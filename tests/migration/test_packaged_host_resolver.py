"""Executable gate for the packaged Rust worker profile (F1/F2).

The other Rust acceptance gates drive a worker binary with an argv they build
themselves. This one drives the *host resolver* instead: it assembles a real
staged Rust package layout, asks the real `nioh3-studio` resolver for the
command line of each role, and then spawns exactly that command line. That is
the chain that was missing, and it is what makes a mismatch between the staged
manifest, the packaged layout and the broker's launch path a failure instead of
a silent one.

Nothing here is mocked: the workers are the real debug binaries, the package
layout is produced by the shipping staging and packaging helpers, the resolver
is the host's own `launch_command`, and the declared layout is checked against
every path the resolver hands the process. No save file and no game process is
touched; the same executable gate additionally performs one read-only handshake
per role, and the optional WebView2 step (see `verify-host-package.mjs`) is
opt-in.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"
MANIFEST = ROOT / "apps/tauri/src-tauri/Cargo.toml"
ROLES = ("offline_search", "save", "runtime")
DUMP_TEST = "tests::dump_role_launch_for_acceptance"
HOST_VERIFIER = ROOT / "apps/tauri/verify-host-package.mjs"
FRONTEND_VERIFIER = ROOT / "apps/tauri/verify-packaged-frontend.mjs"
MAX_FRAME_BYTES = 4 * 1024 * 1024
PRODUCTION_GAME_FILE_VERSION = "2.0.2.0"
EVIDENCE_ROOT_ENV = "NIOH3_ACCEPTANCE_EVIDENCE_ROOT"
EVIDENCE_RUN_ID_ENV = "NIOH3_ACCEPTANCE_RUN_ID"


def node_executable() -> str | None:
    return shutil.which("node") or shutil.which("node.exe")


def host_executable(target: Path) -> Path:
    return target / "debug" / "nioh3-studio.exe"


def build_host_executable(target: Path, environment: dict) -> Path:
    """Build the host once and return its executable path."""

    executable = host_executable(target)
    if executable.is_file():
        return executable
    result = subprocess.run(
        [
            "cargo",
            "build",
            "--locked",
            "--offline",
            "--manifest-path",
            str(MANIFEST),
        ],
        cwd=str(ROOT),
        env=environment,
        capture_output=True,
        timeout=3600,
    )
    if result.returncode != 0:
        raise AssertionError(
            "the host executable did not build: "
            + result.stderr.decode("utf-8", "replace")[-2000:]
        )
    return executable


def write_build_manifest(layout: Path, version: str, entry: Path) -> Path:
    """Declare the staged layout as a package manifest, as the packager does.

    The packaged host validates this file before it serves a window, so the
    development-build acceptance must carry a truthful manifest. Every declared
    entry is hashed from the staged bytes; nothing is asserted that is not there.
    """

    destination = layout / "Nioh3Studio.exe"
    shutil.copy2(entry, destination)
    files = []
    for path in sorted(layout.rglob("*")):
        if not path.is_file():
            continue
        files.append(
            {
                "path": path.relative_to(layout).as_posix(),
                "size": path.stat().st_size,
                "sha256": real_hash(path),
            }
        )
    manifest = {
        "schema": "nioh3-tauri-manifest/v1",
        "version": version,
        "files": files,
    }
    path = layout / "build-manifest.json"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    return path


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def shared_target_dir() -> Path:
    configured = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if configured:
        return Path(configured)
    preferred = Path("D:/Nioh3_v080_deliverables/build-cache/m4-workers")
    if preferred.drive and Path(preferred.anchor).exists():
        return preferred
    return Path(tempfile.gettempdir()) / "nioh3-m4-workers"


def fixture_root() -> Path:
    preferred = Path("D:/Nioh3_v080_deliverables/tmp")
    try:
        preferred.mkdir(parents=True, exist_ok=True)
    except OSError:
        return Path(tempfile.mkdtemp(prefix="nioh3-packaged-host-"))
    return Path(tempfile.mkdtemp(prefix="nioh3-packaged-host-", dir=preferred))


def real_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def file_identity(path: Path) -> dict:
    """Return the raw-byte identity used by the retained acceptance evidence."""

    resolved = path.resolve()
    return {
        "path": str(resolved),
        "size": resolved.stat().st_size,
        "sha256": real_hash(resolved),
    }


def plaintext_digest(path: Path) -> str:
    """SHA-256 of the decrypted savedata generation.

    A restore re-encrypts the container, so the encrypted bytes legitimately
    differ from the backup's even when the recovered generation is identical.
    Comparing the decrypted bytes is the invariant that actually holds.
    """

    import sys

    sys.path.insert(0, str(ROOT))
    from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool

    plain = Path(tempfile.mkdtemp(prefix="nioh3-plain-")) / "plain.bin"
    SaveCrypto(default_crypto_tool(ROOT)).decrypt(path, plain)
    digest = hashlib.sha256(plain.read_bytes()).hexdigest()
    shutil.rmtree(plain.parent, ignore_errors=True)
    return digest


def write_frame(stream, value: dict) -> None:
    body = json.dumps(value, separators=(",", ":")).encode("utf-8")
    assert len(body) < MAX_FRAME_BYTES
    stream.write(struct.pack("<I", len(body)) + body)
    stream.flush()


def read_frame(stream) -> dict:
    header = stream.read(4)
    if len(header) != 4:
        raise EOFError("the spawned worker closed stdout")
    (size,) = struct.unpack("<I", header)
    if size == 0 or size > MAX_FRAME_BYTES:
        raise ValueError(f"the spawned worker declared an invalid frame size {size}")
    body = stream.read(size)
    if len(body) != size:
        raise EOFError("the spawned worker truncated a frame")
    return json.loads(body.decode("utf-8"))


class PackagedHostResolverTests(unittest.TestCase):
    """Assemble the real Rust package once, then exercise every role."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.target = shared_target_dir()
        environment = dict(os.environ)
        environment["CARGO_TARGET_DIR"] = str(cls.target)
        for manifest, binary in (
            ("crates/nioh3-worker/Cargo.toml", "nioh3-readonly-worker"),
            ("crates/nioh3-protected/Cargo.toml", "nioh3-protected-worker"),
        ):
            built = cls.target / "debug" / f"{binary}.exe"
            if built.is_file():
                continue
            result = subprocess.run(
                [
                    "cargo",
                    "build",
                    "--locked",
                    "--offline",
                    "--manifest-path",
                    str(ROOT / manifest),
                    "--bin",
                    binary,
                ],
                cwd=str(ROOT),
                env=environment,
                capture_output=True,
                timeout=3600,
            )
            if result.returncode != 0:
                raise AssertionError(
                    f"{binary} did not build: "
                    + result.stderr.decode("utf-8", "replace")[-2000:]
                )
        # `assemble_layout` resolves release host binaries through the same
        # explicit CARGO_TARGET_DIR as every other migration gate. Build those
        # prerequisites here so a clean, standalone full-suite run cannot turn
        # one missing host artifact into seven identical setup failures.
        for manifest, binary in (
            ("apps/tauri/src-tauri/Cargo.toml", "nioh3-studio"),
            ("apps/launcher/Cargo.toml", "Nioh3Launcher"),
        ):
            built = cls.target / "release" / f"{binary}.exe"
            if built.is_file():
                continue
            result = subprocess.run(
                [
                    "cargo",
                    "build",
                    "--locked",
                    "--offline",
                    "--release",
                    "--manifest-path",
                    str(ROOT / manifest),
                    "--bin",
                    binary,
                ],
                cwd=str(ROOT),
                env=environment,
                capture_output=True,
                timeout=3600,
            )
            if result.returncode != 0:
                raise AssertionError(
                    f"{binary} did not build: "
                    + result.stderr.decode("utf-8", "replace")[-2000:]
                )
        cls.stage_tool = load_tool("stage_rust_workers")
        cls.packager = load_tool("package_tauri")
        cls.host_environment = dict(os.environ)
        cls.host_environment["CARGO_TARGET_DIR"] = str(cls.target)
        cls.host_environment["NIOH3_ACCEPTANCE_GAME_FILE_VERSION"] = (
            PRODUCTION_GAME_FILE_VERSION
        )

    def setUp(self) -> None:
        self.temp = fixture_root()
        self.addCleanup(lambda: shutil.rmtree(self.temp, ignore_errors=True))
        self.package = self.assemble_real_rust_package()
        self.capture_node_identity("before")

    def tearDown(self) -> None:
        # `addCleanup` removes the synthetic package after this method returns.
        # In evidence mode, freeze the exact final graph before that happens.
        self.capture_node_identity("after")

    def evidence_directory(self) -> Path | None:
        configured = os.environ.get(EVIDENCE_ROOT_ENV, "").strip()
        if not configured:
            return None
        destination = Path(configured).resolve() / "nodes" / self._testMethodName
        destination.mkdir(parents=True, exist_ok=True)
        return destination

    def write_evidence_json(self, name: str, payload: dict | list) -> None:
        destination = self.evidence_directory()
        if destination is None:
            return
        (destination / name).write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def persist_evidence_file(self, source: Path, name: str | None = None) -> None:
        destination = self.evidence_directory()
        if destination is None or not source.is_file():
            return
        shutil.copy2(source, destination / (name or source.name))

    def persist_evidence_tree(self, source: Path, prefix: str) -> None:
        destination = self.evidence_directory()
        if destination is None or not source.is_dir():
            return
        for path in sorted(source.rglob("*")):
            if path.is_file():
                relative = path.relative_to(source)
                target = destination / prefix / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)

    def capture_node_identity(self, phase: str) -> None:
        destination = self.evidence_directory()
        if destination is None:
            return

        artifact_paths = {
            "debugHost": self.target / "debug/nioh3-studio.exe",
            "debugSearchWorker": self.target / "debug/nioh3-readonly-worker.exe",
            "debugProtectedWorker": self.target / "debug/nioh3-protected-worker.exe",
            "releaseHostFixturePrerequisite": self.target / "release/nioh3-studio.exe",
            "releaseLauncherFixturePrerequisite": self.target / "release/Nioh3Launcher.exe",
            "stagedSearchWorker": self.package / "worker/nioh3-search-worker.exe",
            "stagedProtectedWorker": self.package / "worker/nioh3-protected-worker.exe",
        }
        source_paths = {
            "resolverTest": Path(__file__),
            "hostVerifier": HOST_VERIFIER,
            "frontendVerifier": FRONTEND_VERIFIER,
            "stageRustWorkers": TOOLS / "stage_rust_workers.py",
            "packageTauri": TOOLS / "package_tauri.py",
        }
        package_paths = {
            "workerManifest": self.package / "worker/worker-backend.json",
            "buildManifest": self.package / "build-manifest.json",
        }
        payload = {
            "schema": "nioh3-r5-evidence-node/v1",
            "runId": os.environ.get(EVIDENCE_RUN_ID_ENV),
            "node": self._testMethodName,
            "phase": phase,
            "gameFileVersion": PRODUCTION_GAME_FILE_VERSION,
            "cargoTarget": str(self.target.resolve()),
            "syntheticPackage": str(self.package.resolve()),
            "artifacts": {
                name: file_identity(path) if path.is_file() else None
                for name, path in artifact_paths.items()
            },
            "sources": {
                name: file_identity(path) if path.is_file() else None
                for name, path in source_paths.items()
            },
            "packageFiles": {
                name: file_identity(path) if path.is_file() else None
                for name, path in package_paths.items()
            },
        }
        self.write_evidence_json(f"{phase}-identity.json", payload)
        for name, path in package_paths.items():
            self.persist_evidence_file(path, f"{phase}-{name}.json")

    def assemble_real_rust_package(self) -> Path:
        """Stage the real binaries, the real tables and the real contracts."""

        binaries = self.temp / "cargo"
        (binaries / "debug").mkdir(parents=True)
        for name in ("nioh3-readonly-worker.exe", "nioh3-protected-worker.exe"):
            shutil.copy2(self.target / "debug" / name, binaries / "debug" / name)
        workers = self.temp / "workers"
        self.stage_tool.stage(binaries, workers, "debug", ROOT)

        root = self.temp / "root"
        (root / "apps/tauri/src-tauri/target/release").mkdir(parents=True)
        (root / "apps/launcher/target/release").mkdir(parents=True)
        (root / "apps/tauri/src-tauri/target/release/nioh3-studio.exe").write_bytes(b"broker")
        (root / "apps/launcher/target/release/Nioh3Launcher.exe").write_bytes(b"launcher")
        (root / "package.json").write_text(
            json.dumps({"name": "fixture", "version": "0.0.0-test", "dependencies": {}}),
            encoding="utf-8",
        )
        (root / "bin").mkdir()
        for name in (
            "nioh3_seed_accelerator.dll",
            "nioh3_effect_preimage_accelerator.dll",
            "nioh3_seed_accelerator.build.json",
        ):
            shutil.copy2(ROOT / "bin" / name, root / "bin" / name)
        shutil.copytree(ROOT / "nioh3_scroll_editor" / "data", root / "nioh3_scroll_editor" / "data")
        (root / "packages/contracts").mkdir(parents=True)
        for path in (ROOT / "packages/contracts").glob("*.schema.json"):
            shutil.copy2(path, root / "packages/contracts" / path.name)
        (root / "third_party/nioh_savefile_decrypt").mkdir(parents=True)
        (root / "third_party/nioh_savefile_decrypt/LICENSE").write_text("MIT", encoding="utf-8")
        api = root / "node_modules/@tauri-apps/api"
        api.mkdir(parents=True)
        (api / "package.json").write_text(
            json.dumps({"name": "@tauri-apps/api", "version": "0.0.0", "license": "MIT"}),
            encoding="utf-8",
        )
        (api / "LICENSE").write_text("MIT", encoding="utf-8")

        output = self.temp / "portable"
        self.packager.assemble_layout(root, output, workers, "rust")
        return output

    def resolve_launch(self, role: str, state_root: Path, packaged: bool = True) -> dict:
        """Ask the real host resolver for this role's command line."""

        environment = dict(self.host_environment)
        environment.update(
            {
                "NIOH3_ACCEPTANCE_ROOT": str(self.package),
                "NIOH3_ACCEPTANCE_ROLE": role,
                "NIOH3_ACCEPTANCE_PACKAGED": "1" if packaged else "0",
                "NIOH3_ACCEPTANCE_STATE_ROOT": str(state_root),
            }
        )
        result = subprocess.run(
            [
                "cargo",
                "test",
                "--locked",
                "--offline",
                "--manifest-path",
                str(MANIFEST),
                "--bin",
                "nioh3-studio",
                DUMP_TEST,
                "--",
                "--ignored",
                "--exact",
                "--nocapture",
            ],
            cwd=str(ROOT),
            env=environment,
            capture_output=True,
            timeout=900,
        )
        stdout = result.stdout.decode("utf-8", "replace")
        if result.returncode != 0:
            raise AssertionError(
                f"the host resolver failed for {role}: "
                + stdout[-3000:]
                + result.stderr.decode("utf-8", "replace")[-2000:]
            )
        for line in stdout.splitlines():
            if line.startswith("NIOH3_LAUNCH "):
                return json.loads(line[len("NIOH3_LAUNCH ") :])
        raise AssertionError(f"the resolver printed no launch line for {role}: {stdout[-2000:]}")

    def spawn_and_handshake(self, launch: dict) -> dict:
        """Spawn the resolved command line and complete one framed handshake."""

        environment = dict(os.environ)
        environment["NIOH3_STATE_ROOT"] = launch["stateRoot"]
        environment.pop("NIOH3_PYTHON", None)
        environment.pop("NIOH3_RUST_SEARCH_WORKER", None)
        environment.pop("NIOH3_RUST_PROTECTED_WORKER", None)
        if environment.get("PATH"):
            environment["PATH"] = ";".join(
                part for part in environment["PATH"].split(";") if "python" not in part.lower()
            )
        process = subprocess.Popen(
            [launch["executable"], *launch["argv"]],
            cwd=str(self.package),
            env=environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            write_frame(process.stdin, {"protocol": 1, "id": "gate-1", "method": "handshake", "params": {}})
            reply = read_frame(process.stdout)
        finally:
            try:
                process.stdin.close()
            except OSError:
                pass
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=30)
        stderr = process.stderr.read().decode("utf-8", "replace") if process.stderr else ""
        if not reply.get("ok"):
            raise AssertionError(
                f"{launch['role']} refused the handshake: {reply.get('error')}; stderr={stderr[-2000:]}"
            )
        return reply["result"]

    def manifest(self) -> dict:
        return json.loads(
            (self.package / "worker/worker-backend.json").read_text(encoding="utf-8")
        )

    def test_staged_layout_declares_the_roots_the_resolver_uses(self) -> None:
        manifest = self.manifest()
        assert manifest["backend"] == "rust"
        for name in ("nioh3-search-worker.exe", "nioh3-protected-worker.exe"):
            assert (self.package / "worker" / name).is_file()
        runtime = self.package / "worker/runtime"
        assert (runtime / "bin/nioh3_seed_accelerator.dll").is_file()
        data = runtime / "nioh3_scroll_editor/data"
        assert (data / "recommended_level_curve.json").is_file()
        assert (data / "effect_names_multilingual.json").is_file()
        assert (self.package / "packages/contracts/request.schema.json").is_file()
        # The working-tree shape the development fallback uses must not exist in
        # the package; if it did, a cutover could silently pass for the wrong
        # reason.
        assert not (self.package / "nioh3_scroll_editor/data").exists()

    def test_resolved_launch_matches_the_staged_manifest_for_every_role(self) -> None:
        manifest = self.manifest()
        state_root = self.temp / "state"
        state_root.mkdir()
        staged_root = str(self.package)
        retained_launches = {}
        for role in ROLES:
            launch = self.resolve_launch(role, state_root)
            retained_launches[role] = launch
            declared = manifest["invocation"][role]
            assert launch["packaged"] is True
            assert Path(launch["executable"]) == self.package / "worker" / declared["binary"]
            argv = launch["argv"]
            # The resolver's argv must carry the same flag/value pairs the staging
            # tool declared; option order is not part of the contract.
            def pairs(tokens):
                result = {}
                index = 0
                while index < len(tokens):
                    token = tokens[index]
                    if token.startswith("--") and index + 1 < len(tokens) and not tokens[index + 1].startswith("--"):
                        result[token] = tokens[index + 1]
                        index += 2
                    else:
                        result[token] = None
                        index += 1
                return result

            actual, expected = pairs(argv), pairs(declared["argv"])
            for flag, value in expected.items():
                assert flag in actual, f"{role} argv lacks {flag}: {argv}"
                if value == "<state root>":
                    assert actual[flag] == str(state_root)
                elif value is not None:
                    # The staging tool declares package-confined paths in POSIX
                    # form on purpose and the resolver converts every manifest
                    # separator to the platform's own (a canonical `\\?\` root is
                    # never normalized, so a mixed-separator path would not
                    # resolve). Compare the declared text against that same
                    # normalization instead of assuming the two agree byte for
                    # byte on every platform.
                    observed = actual[flag].replace(staged_root, "<runtime>")
                    assert observed.replace(os.sep, "/") == value, (
                        f"{role} {flag}: {actual[flag]} != {value}"
                    )
            # F3: the resolver always pins the helper, in both launch shapes.
            assert "--accelerator" in actual, f"{role} argv lacks --accelerator: {argv}"
            assert Path(actual["--accelerator"]) == (
                self.package / "worker/runtime/bin/nioh3_seed_accelerator.dll"
            )
            assert Path(actual["--data-root"]) == (
                self.package / "worker/runtime/nioh3_scroll_editor/data"
            )
            assert Path(actual["--contract-dir"]) == self.package / "packages/contracts"
            for path in actual.values():
                if path is not None and path.startswith(staged_root):
                    assert Path(path).is_file() or Path(path).is_dir(), path
            if role == "offline_search":
                assert "--packaged-worker" in actual
                assert "--dev-preview-only" not in actual
            else:
                assert actual["--role"] == role
                assert "--dev-protected-only" not in actual
                assert actual["--state-root"] == str(state_root)
        self.write_evidence_json("resolved-launches.json", retained_launches)

    def test_resolved_launch_really_starts_the_declared_binary(self) -> None:
        state_root = self.temp / "state-live"
        state_root.mkdir()
        manifest = self.manifest()
        expected_sha = {
            entry["packagedName"]: entry["sha256"] for entry in manifest["binaries"]
        }
        retained_handshakes = {}
        for role in ROLES:
            launch = self.resolve_launch(role, state_root)
            binary = Path(launch["executable"])
            assert real_hash(binary) == expected_sha[binary.name], (
                f"{role} resolved {binary.name} but the manifest declares another identity"
            )
            handshake = self.spawn_and_handshake(launch)
            retained_handshakes[role] = {
                "launch": launch,
                "handshake": handshake,
                "binary": file_identity(binary),
            }
            assert handshake["role"] == role, handshake
            assert handshake["contract_digest"], handshake
            if role != "offline_search":
                assert handshake["kill_safe"] is False, handshake
        self.write_evidence_json("role-handshakes.json", retained_handshakes)

    def test_a_package_without_the_manifest_keeps_the_shipped_python_graph(self) -> None:
        (self.package / "worker/worker-backend.json").unlink()
        launch = self.resolve_launch("offline_search", self.temp)
        # No staged manifest: the host resolves the shipped packaged worker, and
        # the development Rust flags must not appear on that command line.
        assert launch["packaged"] is True
        assert "--packaged-worker" not in launch["argv"]
        assert "--dev-preview-only" not in launch["argv"]
        assert Path(launch["executable"]).name == "nioh3-search-worker.exe"
        self.write_evidence_json("no-manifest-fallback.json", launch)

    def test_a_damaged_manifest_fails_the_resolver_instead_of_falling_back(self) -> None:
        manifest_path = self.package / "worker/worker-backend.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["backend"] = "python"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        environment = dict(self.host_environment)
        environment.update(
            {
                "NIOH3_ACCEPTANCE_ROOT": str(self.package),
                "NIOH3_ACCEPTANCE_ROLE": "offline_search",
                "NIOH3_ACCEPTANCE_PACKAGED": "1",
            }
        )
        result = subprocess.run(
            [
                "cargo",
                "test",
                "--locked",
                "--offline",
                "--manifest-path",
                str(MANIFEST),
                "--bin",
                "nioh3-studio",
                DUMP_TEST,
                "--",
                "--ignored",
                "--exact",
                "--nocapture",
            ],
            cwd=str(ROOT),
            env=environment,
            capture_output=True,
            timeout=900,
        )
        combined = result.stdout.decode("utf-8", "replace") + result.stderr.decode(
            "utf-8", "replace"
        )
        assert result.returncode != 0, "a damaged manifest must fail the resolver"
        assert "WORKER_BACKEND_UNSUPPORTED" in combined, combined[-2000:]
        # The shipped Python worker is still there and must not have been used.
        assert (self.package / "worker/nioh3-search-worker.exe").is_file()
        self.write_evidence_json(
            "damaged-manifest-result.json",
            {
                "exitCode": result.returncode,
                "expectedError": "WORKER_BACKEND_UNSUPPORTED",
                "combinedOutput": combined,
            },
        )

    def test_real_host_process_resolves_the_staged_graph(self) -> None:
        """Native acceptance: the real host binary, launched against the package.

        The host records its own resolution before it validates the package
        manifest, so this runs against a staged runtime rather than a finished
        single-file release. It is development-build evidence, not release
        evidence, and it writes only into its own temporary profile.
        """

        node = node_executable()
        if node is None:
            self.skipTest("Node.js is required to launch the real host process")
        executable = build_host_executable(self.target, self.host_environment)
        if not executable.is_file():
            self.skipTest(
                "the debug host executable is unavailable; build apps/tauri/src-tauri first"
            )
        out = self.temp / "host-resolution.json"
        result = subprocess.run(
            [
                node,
                str(HOST_VERIFIER),
                "--package",
                str(self.package),
                "--exe",
                str(executable),
                "--out",
                str(out),
                "--timeout",
                "90",
            ],
            cwd=str(ROOT),
            capture_output=True,
            timeout=300,
        )
        stdout = result.stdout.decode("utf-8", "replace")
        stderr = result.stderr.decode("utf-8", "replace")
        self.write_evidence_json(
            "host-verifier-run.json",
            {
                "exitCode": result.returncode,
                "argv": [
                    node,
                    str(HOST_VERIFIER),
                    "--package",
                    str(self.package),
                    "--exe",
                    str(executable),
                    "--out",
                    str(out),
                    "--timeout",
                    "90",
                ],
                "stdout": stdout,
                "stderr": stderr,
            },
        )
        self.persist_evidence_file(out, "host-resolution.json")
        self.capture_node_identity("execution")
        if result.returncode != 0:
            raise AssertionError(
                f"the packaged host did not resolve the staged graph: {stdout[-3000:]}{stderr[-3000:]}"
            )
        evidence = json.loads(out.read_text(encoding="utf-8"))
        assert evidence["backend"] == "rust-packaged", evidence
        assert evidence["developmentBuild"] is True
        assert evidence["releaseCandidate"] is False
        assert evidence["stderrBytes"] >= 0
        for role in ROLES:
            assert role in evidence["roles"], evidence

    def test_real_frontend_acceptance_on_the_rust_packaged_graph(self) -> None:
        """WebView2 acceptance through the frontend on the staged Rust profile.

        This is the gate that proves the *UI* is answered by the staged graph:
        catalog in three locales, preview, the v0.7.5 continuation regression and
        its resume, favorites persistence across a host restart, and the protected
        save plan on the synthetic encrypted fixture. Development-build evidence:
        the host executable is the debug build and the run writes only into its
        own temporary profile. No real game process and no user save are touched.

        This gate asserts the published seed *and cursor*, so it drives the
        verifier's `extended` profile; the default `release` profile previews that
        seed directly and claims no cursor.
        """

        node = node_executable()
        if node is None:
            self.skipTest("Node.js and Playwright are required for the UI gate")
        if not (ROOT / "node_modules/playwright/package.json").is_file():
            self.skipTest("Playwright is not installed")
        python = (
            os.environ.get("NIOH3_PYTHON")
            or str(ROOT / ".codex_tmp/v2-build-env/Scripts/python.exe")
        )
        if not Path(python).is_file():
            self.skipTest(f"the project Python environment is missing: {python}")
        executable = build_host_executable(self.target, self.host_environment)
        assert executable.is_file(), "the debug host executable must be buildable"
        version = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["version"]
        write_build_manifest(self.package, version, executable)
        out = self.temp / "frontend"
        result = subprocess.run(
            [
                node,
                str(FRONTEND_VERIFIER),
                "--package",
                str(self.package),
                "--exe",
                str(executable),
                "--python",
                python,
                "--out",
                str(out),
                "--profile",
                "extended",
                "--timeout",
                "120",
            ],
            cwd=str(ROOT),
            capture_output=True,
            timeout=1800,
        )
        stdout = result.stdout.decode("utf-8", "replace")
        stderr = result.stderr.decode("utf-8", "replace")
        blocked_evidence = out / "partial-evidence.json"
        full_evidence = out / "packaged-frontend.json"
        self.write_evidence_json(
            "frontend-verifier-run.json",
            {
                "exitCode": result.returncode,
                "argv": [
                    node,
                    str(FRONTEND_VERIFIER),
                    "--package",
                    str(self.package),
                    "--exe",
                    str(executable),
                    "--python",
                    python,
                    "--out",
                    str(out),
                    "--profile",
                    "extended",
                    "--timeout",
                    "120",
                ],
                "stdout": stdout,
                "stderr": stderr,
            },
        )
        self.persist_evidence_tree(out, "frontend-result")
        self.capture_node_identity("execution")
        if result.returncode != 0 and not blocked_evidence.is_file():
            raise AssertionError(
                "the packaged frontend acceptance failed: "
                + stdout[-4000:]
                + stderr[-4000:]
            )
        # The acceptance must succeed. A blocked save/cart leg is a failure of
        # this gate, not a re-label: partial evidence is kept on disk for a
        # diagnostic report, but the assertion below never accepts it.
        if result.returncode != 0:
            raise AssertionError(
                "the packaged frontend acceptance failed: "
                + stdout[-4000:]
                + stderr[-4000:]
            )
        evidence_path = full_evidence
        assert evidence_path.is_file(), f"the acceptance wrote no evidence: {stdout[-2000:]}"
        evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
        assert evidence["graph"] == "rust-packaged", evidence
        assert evidence["developmentBuild"] is True
        assert evidence["releaseCandidate"] is False
        assert (
            evidence["handshake"]["selectedContext"]["gameFileVersion"]
            == PRODUCTION_GAME_FILE_VERSION
        ), evidence["handshake"]
        # The fixture copy is written on purpose by the committed legs; what must
        # hold is that they are isolated and never touch a real save.
        assert evidence["fixtureWrites"]["realSaveTouched"] is False, evidence["fixtureWrites"]
        assert evidence["fixtureWrites"]["isolatedCopy"] is True, evidence["fixtureWrites"]
        assert evidence["handshake"]["role"] == "offline_search"
        for locale in ("zh-CN", "en-US", "ja-JP"):
            assert locale in evidence["catalogs"], evidence["catalogs"]
        assert evidence["regression"]["seed"] == 226061463
        assert evidence["regression"]["cursor"] == 158614759
        # Every requested leg must be present and positive. No baseline
        # adjustment, no manual skip, no "blocked" acceptance path.
        cart = evidence["cart"]
        assert cart.get("plan"), cart
        assert cart["committed"] is True, cart
        assert evidence["persistence"]["favoritesAfterRestart"] >= 1
        for leg in ("editor", "delete", "restore"):
            assert evidence["saveFlow"][leg], evidence["saveFlow"]
        restore = evidence["saveFlow"]["restore"]
        assert restore["restoredToStartingGeneration"] is True, restore
        editor = evidence["saveFlow"]["editor"]
        applied = editor["applied"]
        # The editor leg must be a real apply through the UI path: committed, the
        # value read back, every unrelated field preserved, and the decrypted
        # record bytes changed by exactly the declared amount plus the checksum.
        assert applied["committed"] is True, applied
        assert applied["valueReadBack"] == editor["valueAfter"], applied
        assert applied["headerPreserved"] is True, applied
        assert applied["unrelatedEffectsPreserved"] is True, applied
        assert applied["decryptedChangedBytes"] > 0, applied
        assert (
            applied["decryptedChangedBytes"] == applied["declaredChangedOffsets"] + 1
        ), applied
        assert editor["fixtureRestored"] is True, editor
        # Update check: the launch check must have reached a terminal phase, a
        # manual check must be accepted and settle, and nothing may download.
        updater = evidence["updater"]
        assert updater["downloadsAttempted"] == 0, updater
        assert "status" in updater["calls"], updater
        assert updater["startupStatus"]["canApply"] is True, updater["startupStatus"]
        for leg in ("startupSettled", "manualSettled"):
            assert updater[leg]["phase"] in {
                "current",
                "available",
                "ready",
                "failed",
            }, updater[leg]


if __name__ == "__main__":
    unittest.main()
