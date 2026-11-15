"""Real-binary worker identity acceptance for the opt-in Rust graph (M4.4).

This gate builds the actual Rust workers once, stages them with the real shipped
data, contracts and helper DLLs into an isolated layout on `D:`, and drives
`apps/tauri/verify-worker-identity.mjs` against each role. Nothing is faked: the
handshake and the representative request come from the real binaries, the
recorded digests are the staged files, and no ambient Python is required (the
spawned environment drops `NIOH3_PYTHON` and any Python PATH entry).

It also proves the manifest cannot self-assert success: disallowed flags, paths
that escape the package, a mismatched role, a mismatched binary and a missing
schema are all refused by the acceptance's own expectations, and the shipped
(default) graph reports an unverified worker instead of a fabricated pass.
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"
IDENTITY = ROOT / "apps" / "tauri" / "verify-worker-identity.mjs"


def evidence_path() -> Path:
    """Where the real-role identity evidence is recorded.

    Defaults to the local D: workspace (the delivery volume used for these
    long-running artifacts) and falls back to the platform temp directory when
    that volume is unavailable, so the same gate works on a CI runner and the
    path can be pinned with `NIOH3_M4_IDENTITY_EVIDENCE`.
    """

    configured = os.environ.get("NIOH3_M4_IDENTITY_EVIDENCE", "").strip()
    if configured:
        return Path(configured)
    preferred = Path("D:/Nioh3_v080_deliverables/m4-identity-evidence.json")
    if preferred.drive and Path(preferred.anchor).exists():
        return preferred
    return Path(tempfile.gettempdir()) / "m4-identity-evidence.json"

WORKER_CRATES = (
    ("crates/nioh3-worker/Cargo.toml", "nioh3-readonly-worker"),
    ("crates/nioh3-protected/Cargo.toml", "nioh3-protected-worker"),
)


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
        return Path(tempfile.mkdtemp(prefix="nioh3-identity-"))
    return Path(tempfile.mkdtemp(prefix="nioh3-identity-", dir=preferred))


class RealWorkerIdentityTests(unittest.TestCase):
    """Stage the real runtime once, then exercise it per role."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.target = shared_target_dir()
        cls.profile = "debug"
        environment = dict(os.environ)
        environment["CARGO_TARGET_DIR"] = str(cls.target)
        for manifest, binary in WORKER_CRATES:
            built = cls.target / cls.profile / f"{binary}.exe"
            if built.is_file():
                continue
            build = subprocess.run(
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
            if build.returncode != 0:
                raise AssertionError(
                    f"{binary} did not build: "
                    + build.stderr.decode("utf-8", "replace")[-2000:]
                )
        cls.stage_tool = load_tool("stage_rust_workers")
        cls.packager = load_tool("package_tauri")

    def setUp(self) -> None:
        self.temp = fixture_root()
        self.runtime = self.temp / "runtime"
        workers = self.runtime / "worker"
        workers.mkdir(parents=True)
        manifest = self.stage_tool.stage(self.target, workers, self.profile, ROOT)
        self.layout = self.packager.stage_rust_runtime(ROOT, self.runtime)
        contracts = self.runtime / "packages" / "contracts"
        contracts.mkdir(parents=True)
        for path in (ROOT / "packages" / "contracts").glob("*.schema.json"):
            shutil.copy2(path, contracts / path.name)
        self.workers = workers
        self.manifest = manifest

    def identity(self, role: str, *, opt_in: bool = True, shipped: bool = False) -> tuple[int, str, str, dict | None]:
        out = self.temp / f"identity-{role}{'-shipped' if shipped else ''}.json"
        environment = dict(os.environ)
        environment.pop("NIOH3_WORKER_IDENTITY_OPT_IN", None)
        # The staged manifest never carries the identity selection, so the
        # acceptance harness injects this exact four-part version at spawn.
        environment.setdefault("NIOH3_PARITY_GAME_FILE_VERSION", "2.0.2.0")
        if opt_in:
            environment["NIOH3_WORKER_IDENTITY_OPT_IN"] = "1"
        argv = [
            "node",
            str(IDENTITY),
            "--runtime",
            str(self.runtime),
            "--role",
            role,
            "--out",
            str(out),
        ]
        if shipped:
            argv.append("--shipped")
        completed = subprocess.run(
            argv, cwd=str(ROOT), env=environment, capture_output=True, text=True, timeout=600
        )
        report = json.loads(out.read_text(encoding="utf-8")) if out.is_file() else None
        return completed.returncode, completed.stdout, completed.stderr, report

    def tamper(self, mutate) -> None:
        manifest_path = self.workers / "worker-backend.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        mutate(manifest)
        manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    def test_every_real_role_handshakes_and_answers_a_safe_request(self) -> None:
        evidence = {"roles": {}, "targetDir": str(self.target), "profile": self.profile}
        for role in ("offline_search", "save", "runtime"):
            with self.subTest(role=role):
                code, stdout, stderr, report = self.identity(role)
                self.assertEqual(code, 0, stderr)
                self.assertIn("TAURI_WORKER_IDENTITY_OK", stdout)
                self.assertIsNotNone(report)
                self.assertTrue(report["handshake"], report)
                self.assertTrue(report["representativeOk"], report)
                self.assertEqual(report["backend"], "rust")
                packaged = self.workers / report["binary"]
                self.assertEqual(
                    report["sha256"],
                    __import__("hashlib").sha256(packaged.read_bytes()).hexdigest(),
                )
                if role == "offline_search":
                    self.assertEqual(report["representativeMethod"], "candidate.preview")
                    self.assertRegex(report["contextDigest"] or "", r"^[0-9a-f]{64}$")
                else:
                    self.assertEqual(report["representativeMethod"], f"{role}.discover" if role == "save" else "runtime.status")
                    self.assertTrue(report["stateRootUsed"])
                    self.assertTrue(Path(report["stateRootUsed"]).is_dir())
                self.assertTrue((self.runtime / report["dataRoot"]).is_dir())
                self.assertTrue((self.runtime / report["contractDir"]).is_dir())
                evidence["roles"][role] = {
                    key: report[key]
                    for key in (
                        "binary",
                        "sha256",
                        "representativeMethod",
                        "representativeOk",
                        "dataRoot",
                        "contractDir",
                        "stateRootUsed",
                    )
                }
        target = evidence_path()
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def test_manifest_cannot_self_assert_success(self) -> None:
        cases = {
            "disallowed_flag": lambda manifest: manifest["invocation"]["offline_search"]["argv"].insert(1, "--totally-new-flag"),
            "path_escapes_package": lambda manifest: manifest["invocation"]["offline_search"]["argv"].__setitem__(
                manifest["invocation"]["offline_search"]["argv"].index("--data-root") + 1,
                "../outside/data",
            ),
            "role_mismatch": lambda manifest: manifest["invocation"]["save"]["argv"].__setitem__(
                manifest["invocation"]["save"]["argv"].index("--role") + 1, "runtime"
            ),
            "binary_mismatch": lambda manifest: manifest["invocation"]["offline_search"].__setitem__(
                "binary", "nioh3-protected-worker.exe"
            ),
            "short_digest": lambda manifest: manifest["binaries"][0].__setitem__("sha256", "abc"),
            "manifest_chosen_state_root": lambda manifest: manifest["invocation"]["save"]["argv"].__setitem__(
                manifest["invocation"]["save"]["argv"].index("--state-root") + 1,
                "C:/somewhere/inside/the/package",
            ),
        }
        for name, mutate in cases.items():
            with self.subTest(case=name):
                preserved = (self.workers / "worker-backend.json").read_bytes()
                self.tamper(mutate)
                role = (
                    "save"
                    if name in ("role_mismatch", "manifest_chosen_state_root")
                    else "offline_search"
                )
                code, _, stderr, _ = self.identity(role)
                self.assertNotEqual(code, 0, f"{name} must be refused")
                self.assertTrue(stderr.strip(), f"{name} must explain the refusal")
                (self.workers / "worker-backend.json").write_bytes(preserved)

    def test_missing_contract_schema_is_refused(self) -> None:
        schema = self.runtime / "packages" / "contracts" / "request.schema.json"
        preserved = schema.read_bytes()
        schema.unlink()
        code, _, stderr, _ = self.identity("offline_search")
        if code == 0:
            # The Rust worker itself fails closed on a missing contract file.
            self.fail("a missing request schema must not verify: " + stderr)
        schema.write_bytes(preserved)

    def test_external_state_root_roundtrip_survives_a_process_restart(self) -> None:
        """User state lives outside the package and persists across restarts.

        The broker injects the state root; the manifest only carries the
        placeholder, so the extracted one-file cache can be pruned without losing
        the settings pointer. This drives the real save role twice against the
        same external roots and checks the pointer written by the first process is
        still there after the second.
        """

        state_root = self.temp / "external-state"
        local_app_data = self.temp / "external-localappdata"
        data_dir = self.temp / "data-dir"
        for path in (state_root, local_app_data, data_dir):
            path.mkdir(parents=True, exist_ok=True)

        def run_save(method: str | None, params: dict | None) -> tuple[int, str, str, dict | None]:
            out = self.temp / f"roundtrip-{method or 'none'}.json"
            environment = dict(os.environ)
            environment.setdefault("NIOH3_PARITY_GAME_FILE_VERSION", "2.0.2.0")
            environment["NIOH3_WORKER_IDENTITY_OPT_IN"] = "1"
            argv = [
                "node",
                str(IDENTITY),
                "--runtime",
                str(self.runtime),
                "--role",
                "save",
                "--state-root",
                str(state_root),
                "--local-app-data",
                str(local_app_data),
                "--out",
                str(out),
            ]
            if method:
                argv += ["--request", method, "--params", json.dumps(params or {})]
            completed = subprocess.run(
                argv, cwd=str(ROOT), env=environment, capture_output=True, text=True, timeout=600
            )
            report = json.loads(out.read_text(encoding="utf-8")) if out.is_file() else None
            return completed.returncode, completed.stdout, completed.stderr, report

        pointer = local_app_data / "Nioh3ScrollGenerator" / "settings.json"
        code, stdout, stderr, first = run_save(
            "save.data_directory", {"action": "set", "path": str(data_dir)}
        )
        self.assertEqual(code, 0, stderr)
        self.assertIn("TAURI_WORKER_IDENTITY_OK", stdout)
        self.assertEqual(first["stateRootSource"], "broker-injected-external")
        self.assertEqual(Path(first["stateRootUsed"]), state_root)
        self.assertTrue(pointer.is_file(), "the settings pointer is written outside the package")
        recorded = json.loads(pointer.read_text(encoding="utf-8"))
        self.assertEqual(Path(recorded["data_root"]), data_dir)

        code, _, stderr, second = run_save(
            "save.data_directory", {"action": "inspect", "path": None}
        )
        self.assertEqual(code, 0, stderr)
        self.assertEqual(second["stateRootUsed"], first["stateRootUsed"])
        self.assertTrue(pointer.is_file(), "state survives a worker restart")
        self.assertEqual(
            json.loads(pointer.read_text(encoding="utf-8"))["data_root"], recorded["data_root"]
        )

    def test_contract_digest_mismatch_is_refused_independently(self) -> None:
        preserved = (self.workers / "worker-backend.json").read_bytes()
        self.tamper(
            lambda manifest: manifest["launchContract"]["contractDigests"].__setitem__(
                "offline_search", "0" * 64
            )
        )
        code, _, stderr, _ = self.identity("offline_search")
        self.assertNotEqual(code, 0)
        self.assertIn("contract digest mismatch", stderr)
        (self.workers / "worker-backend.json").write_bytes(preserved)

        schema = self.runtime / "packages" / "contracts" / "request.schema.json"
        original = schema.read_bytes()
        schema.write_bytes(original + b"\n")
        code, _, stderr, _ = self.identity("offline_search")
        self.assertNotEqual(code, 0)
        self.assertIn("contract digest mismatch", stderr)
        schema.write_bytes(original)

    def test_missing_or_malformed_parity_version_is_refused(self) -> None:
        """The harness refuses to spawn a worker with an unvalidated identity."""

        for raw in ("", "2.0.2", "2.0.2.0.0", "2.0.x.0"):
            with self.subTest(version=raw):
                out = self.temp / "identity-invalid.json"
                environment = dict(os.environ)
                environment["NIOH3_WORKER_IDENTITY_OPT_IN"] = "1"
                environment["NIOH3_PARITY_GAME_FILE_VERSION"] = raw
                completed = subprocess.run(
                    [
                        "node",
                        str(IDENTITY),
                        "--runtime",
                        str(self.runtime),
                        "--role",
                        "offline_search",
                        "--out",
                        str(out),
                    ],
                    cwd=str(ROOT),
                    env=environment,
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn("four-part game file version", completed.stderr)
                self.assertFalse(out.is_file(), "a refused spawn writes no report")

    def test_shipped_default_reports_unverified_instead_of_fabricating(self) -> None:
        # Witness the default graph's truthfulness: the shipped worker name holds
        # a Rust binary here, which the shipped (argument-less) invocation cannot
        # start, so the report must be unverified with a reason.
        code, stdout, _, report = self.identity("offline_search", opt_in=False, shipped=True)
        self.assertEqual(code, 0, "the reporting mode always reports")
        self.assertIn("TAURI_WORKER_IDENTITY_UNVERIFIED", stdout)
        self.assertIsNotNone(report)
        self.assertFalse(report["verified"])
        self.assertFalse(report["handshake"])
        self.assertTrue(report["reason"])


if __name__ == "__main__":
    unittest.main()
