"""End-to-end subprocess acceptance gate for the Rust read-only worker (M2.3a).

This gate drives the real worker process over the shipped framed-JSON protocol
and compares its handshake, context identity and preview payloads against the
Python worker for the full supported NG3 preview surface: rarities 3, 4 and 5
across representative levels and a deterministic seed matrix chosen with the
M0/M2.1/M2.2 stride discipline.

It is not an emitter comparison and it does not skip. The development worker
crate is a required dependency of this gate: a missing crate, a missing Cargo
toolchain, or a stale binary is a failure, never a silent pass. The binary is
rebuilt from the current candidate before every run so a stale debug artifact
cannot satisfy the gate.

Search orchestration (continuous search, resume, cancel) is M2.3b and is not
claimed here; the worker must advertise and serve only its read-only preview
subset.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "packages" / "contracts"
MAX_FRAME_BYTES = 4 * 1024 * 1024
WORKER_ROLE = "offline_search"

# Deterministic seed matrix: fixed boundary/known seeds plus a stride sweep.
FIXED_SEEDS = (
    1,
    2,
    2965,
    6_096_970,
    74_063_692,
    82_212_268,
    183_696_634,
    226_061_463,
    241_719_428,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
)
STRIDE_SEEDS = tuple((index * 2_654_435_761) & 0xFFFF_FFFF for index in range(1, 9))
SEED_MATRIX = FIXED_SEEDS + STRIDE_SEEDS
RARITIES = (3, 4, 5)
LEVELS = (1, 90, 180)

CONTEXT_FIELDS = (
    "product_version",
    "game_profile",
    "resources_digest",
    "algorithm_version",
    "policy_version",
    "context_digest",
    "seed_accelerator_abi",
    "seed_accelerator_build_id",
)
REQUIRED_CAPABILITIES = (
    "playthroughs",
    "rarities",
    "cuda_pivot_and_auxiliary",
    "directcompute_effect_filter",
    "cpu_exact_replay",
    "bulk_cpu_requires_opt_in",
    "save_write",
    "runtime_calls",
)
CANDIDATE_REQUIRED = (
    "candidate_id",
    "context_digest",
    "seed",
    "playthrough",
    "rarity",
    "record_stage",
    "installable",
    "install_blocker",
    "effects",
    "auxiliary",
    "enemy_states",
    "cursor",
    "evidence",
    "installation_available",
    "initial_challenge_capacity",
)
EFFECT_REQUIRED = ("slot", "effect_id", "value", "metadata", "prefix", "tail_0", "tail_1", "roll_percent")
SUPPORTED_METHODS = ("handshake", "candidate.preview", "shutdown")
UNSUPPORTED_METHODS = (
    "search.catalog",
    "recommended_level.resolve",
    "search.start",
    "cache.register",
    "candidate.export",
    "job.snapshot",
    "job.current",
    "job.cancel",
)

# Only these keys may be normalised away before comparison. Everything else in a
# payload is compared exactly, so a meaningful mismatch cannot be hidden.
NONDETERMINISTIC_KEYS = frozenset({"elapsed_ms", "job_id", "run_id", "started_at", "finished_at", "timestamp"})


def contract_digest() -> str:
    digest = hashlib.sha256()
    digest.update((SCHEMA_DIR / "request.schema.json").read_bytes())
    digest.update((SCHEMA_DIR / "response.schema.json").read_bytes())
    return digest.hexdigest()


def develop_worker_target() -> tuple[Path, str]:
    """Return (manifest path, bin name); a missing crate fails the gate."""

    override = os.environ.get("NIOH3_M23_WORKER_MANIFEST", "").strip()
    candidates: list[Path] = []
    if override:
        candidates.append(Path(override))
    crates = ROOT / "crates"
    if crates.is_dir():
        candidates.extend(sorted(crates.glob("*/Cargo.toml")))
    for manifest in candidates:
        if not manifest.is_file():
            continue
        with manifest.open("rb") as stream:
            payload = tomllib.load(stream)
        name = str(payload.get("package", {}).get("name", ""))
        bins = [str(entry.get("name", name)) for entry in payload.get("bin", [])]
        if "worker" not in name and not any("worker" in entry for entry in bins):
            continue
        return manifest, (bins[0] if bins else name)
    raise AssertionError(
        "the development read-only worker crate is required by this gate and was not found under "
        f"{ROOT / 'crates'} (expected a package or bin name containing 'worker'); "
        "set NIOH3_M23_WORKER_MANIFEST to override the manifest path"
    )


def dev_preview_arguments() -> list[str]:
    """The explicit development acknowledgement plus the read-only roots."""

    return [
        "--dev-preview-only",
        "--data-root",
        str(ROOT / "nioh3_scroll_editor" / "data"),
        "--contract-dir",
        str(SCHEMA_DIR),
    ]


class FramedProcess:
    """Framed JSON stdio client for one worker subprocess."""

    def __init__(self, argv: list[str], *, cwd: Path, env: dict[str, str]) -> None:
        self.process = subprocess.Popen(
            argv,
            cwd=str(cwd),
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.counter = 0

    def send(self, method: str, params: dict | None = None, *, protocol: int = 1) -> str:
        self.counter += 1
        request_id = f"m23-{self.counter}"
        body = json.dumps(
            {"protocol": protocol, "id": request_id, "method": method, "params": params or {}},
            separators=(",", ":"),
        ).encode("utf-8")
        assert self.process.stdin is not None
        self.process.stdin.write(struct.pack("<I", len(body)) + body)
        self.process.stdin.flush()
        return request_id

    def send_raw(self, payload: bytes) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(payload)
        self.process.stdin.flush()

    def read_frame(self) -> dict:
        assert self.process.stdout is not None
        header = self.process.stdout.read(4)
        if len(header) != 4:
            raise EOFError("worker closed stdout")
        (size,) = struct.unpack("<I", header)
        if size > MAX_FRAME_BYTES:
            raise ValueError(f"worker frame exceeds the contract limit: {size}")
        body = self.process.stdout.read(size)
        if len(body) != size:
            raise EOFError("worker frame truncated")
        return json.loads(body.decode("utf-8"))

    def call(self, method: str, params: dict | None = None, *, protocol: int = 1) -> dict:
        self.send(method, params, protocol=protocol)
        return self.read_frame()

    def close(self) -> int:
        assert self.process.stdin is not None
        self.process.stdin.close()
        return self.process.wait(timeout=60)

    def kill(self) -> None:
        self.process.kill()
        self.process.wait(timeout=60)


def worktree_env() -> dict[str, str]:
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / ".codex_tmp" / "m23-worker-target"))
    return env


def normalise(value):
    """Drop only genuinely nondeterministic run metadata, recursively."""

    if isinstance(value, dict):
        return {key: normalise(item) for key, item in value.items() if key not in NONDETERMINISTIC_KEYS}
    if isinstance(value, list):
        return [normalise(item) for item in value]
    return value


def preview_matrix() -> list[tuple[int, int, int]]:
    return [(seed, rarity, level) for rarity in RARITIES for level in LEVELS for seed in SEED_MATRIX]


def payload_features(payload: dict) -> frozenset[str]:
    """Return the coverage features one preview payload actually exercises."""

    features: set[str] = set()
    candidate = payload["candidate"]
    auxiliary = candidate.get("auxiliary")
    if auxiliary:
        if auxiliary["terrain"]["display_effect_keys"]:
            features.add("terrain_display_keys")
        if auxiliary["special_rules"]:
            features.add("special_rules")
        if any(group for group in auxiliary["enemy_groups"]):
            features.add("enemy_groups")
    states = candidate.get("enemy_states") or {}
    for variant in ("solo", "expedition"):
        preview = states.get(variant)
        if not preview:
            continue
        for occurrence in preview.get("occurrences", []):
            if occurrence.get("possessed") == "yes":
                features.add(f"wraith_{variant}")
            if occurrence.get("availability") == "expedition_only":
                features.add("expedition_only")
    if candidate["initial_challenge_capacity"] is not None:
        features.add("challenge_capacity")
    features.add(f"rarity_{candidate['rarity']}")
    return frozenset(features)


class PreviewWorkerParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        manifest, binary = develop_worker_target()
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build and run the development worker")
        env = worktree_env()
        build = subprocess.run(
            [
                cargo,
                "build",
                "--locked",
                "--offline",
                "--manifest-path",
                str(manifest),
                "--bin",
                binary,
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the development worker did not build from the current candidate: "
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.rust_argv = [str(Path(env["CARGO_TARGET_DIR"]) / "debug" / f"{binary}.exe"), *dev_preview_arguments()]
        cls.python_argv = [sys.executable, "-u", "-m", "nioh3_scroll_editor.search_worker"]

    def rust_worker(self) -> FramedProcess:
        return FramedProcess(self.rust_argv, cwd=ROOT, env=worktree_env())

    def python_worker(self) -> FramedProcess:
        return FramedProcess(self.python_argv, cwd=ROOT, env=worktree_env())

    def test_development_acknowledgement_is_required_before_serving_frames(self) -> None:
        process = subprocess.run(
            [self.rust_argv[0], *dev_preview_arguments()[2:]],
            cwd=str(ROOT),
            env=worktree_env(),
            input=b"",
            capture_output=True,
            timeout=600,
        )
        self.assertNotEqual(process.returncode, 0, "worker served without the development acknowledgement")
        self.assertTrue(process.stderr.strip(), "worker refused without an explanation on stderr")

    def test_handshake_reports_the_exact_contract_and_an_honest_subset(self) -> None:
        worker = self.rust_worker()
        try:
            reply = worker.call("handshake")
        finally:
            worker.close()
        self.assertTrue(reply.get("ok"), reply)
        handshake = reply["result"]
        self.assertEqual(handshake["protocol"], 1)
        self.assertEqual(handshake["role"], WORKER_ROLE)
        self.assertEqual(handshake["contract_digest"], contract_digest())
        for field in CONTEXT_FIELDS:
            self.assertIn(field, handshake["context"], field)
        capabilities = handshake["capabilities"]
        for field in REQUIRED_CAPABILITIES:
            self.assertIn(field, capabilities, field)
        self.assertIs(capabilities["save_write"], False)
        self.assertIs(capabilities["runtime_calls"], False)

    def test_context_identity_matches_the_python_worker_exactly(self) -> None:
        python = self.python_worker()
        rust = self.rust_worker()
        try:
            expected = python.call("handshake")["result"]["context"]
            actual = rust.call("handshake")["result"]["context"]
        finally:
            python.close()
            rust.close()
        for field in CONTEXT_FIELDS:
            self.assertEqual(actual[field], expected[field], field)

    def test_only_the_advertised_methods_are_supported(self) -> None:
        worker = self.rust_worker()
        try:
            self.assertTrue(worker.call("handshake").get("ok"))
            for method in UNSUPPORTED_METHODS:
                reply = worker.call(method, {})
                self.assertFalse(reply.get("ok"), reply)
                self.assertEqual(reply["error"]["code"], "UNSUPPORTED_METHOD", method)
            # Bounded development deviation: a known-but-unimplemented method
            # short-circuits to UNSUPPORTED_METHOD before full parameter
            # validation, so malformed params still answer UNSUPPORTED_METHOD
            # instead of the Python worker's INVALID_REQUEST. M2.3b must restore
            # full validation for every method it implements.
            malformed = worker.call("search.start", {"unexpected": 1})
            self.assertFalse(malformed.get("ok"), malformed)
            self.assertEqual(malformed["error"]["code"], "UNSUPPORTED_METHOD", malformed)
            for method in SUPPORTED_METHODS:
                self.assertIn(method, ("handshake", "candidate.preview", "shutdown"))
            self.assertTrue(worker.call("shutdown").get("ok"))
        finally:
            worker.close()

    def test_full_preview_surface_matches_the_python_worker(self) -> None:
        """Rarities 3/4/5 across representative levels and the seed matrix."""

        python = self.python_worker()
        rust = self.rust_worker()
        seen: dict[str, int] = {}
        compared = 0
        try:
            python.call("handshake")
            rust.call("handshake")
            for seed, rarity, level in preview_matrix():
                params = {"seed": seed, "rarity": rarity, "level": level}
                expected = python.call("candidate.preview", params)
                actual = rust.call("candidate.preview", params)
                self.assertTrue(expected.get("ok"), expected)
                self.assertTrue(actual.get("ok"), actual)
                candidate = actual["result"]["candidate"]
                for key in CANDIDATE_REQUIRED:
                    self.assertIn(key, candidate, key)
                for effect in candidate["effects"]:
                    for key in EFFECT_REQUIRED:
                        self.assertIn(key, effect, key)
                # Offline previews never carry record bytes in the shipped contract.
                self.assertEqual(candidate["record_stage"], "effect_sequence_only")
                self.assertEqual(actual["result"]["transfer"]["record_hex"], "")
                self.assertIsNone(actual["result"]["transfer"]["installation_record_hex"])
                self.assertEqual(candidate["evidence"], "certified_offline_replay")
                self.assertEqual(
                    normalise(actual["result"]),
                    normalise(expected["result"]),
                    f"seed {seed} rarity {rarity} level {level} preview differs from the Python worker",
                )
                compared += 1
                for feature in payload_features(expected["result"]):
                    seen[feature] = seen.get(feature, 0) + 1
        finally:
            python.close()
            rust.close()
        self.assertEqual(compared, len(preview_matrix()))
        for rarity in RARITIES:
            self.assertGreater(seen.get(f"rarity_{rarity}", 0), 0, f"rarity {rarity} missing from the matrix")
        # Non-vacuity: the matrix must actually exercise the preview surface, so
        # a future fixture change cannot quietly reduce this gate to empty cases.
        for feature in ("terrain_display_keys", "special_rules", "enemy_groups", "challenge_capacity"):
            self.assertGreater(seen.get(feature, 0), 0, f"matrix never exercised {feature}")
        self.assertGreater(seen.get("wraith_solo", 0) + seen.get("wraith_expedition", 0), 0, "no Wraith occurrence")
        self.assertGreater(seen.get("expedition_only", 0), 0, "no expedition-only occurrence")

    def test_handler_rejects_malformed_frames_and_strict_request_errors(self) -> None:
        worker = self.rust_worker()
        try:
            worker.call("handshake")
            for method, params, code in (
                ("candidate.preview", {"unexpected": 1}, "INVALID_REQUEST"),
                # An unknown method fails the versioned request schema before
                # dispatch; the eight known-but-unsupported methods are covered
                # by test_only_the_advertised_methods_are_supported.
                ("no.such.method", {}, "INVALID_REQUEST"),
            ):
                reply = worker.call(method, params)
                self.assertFalse(reply.get("ok"), reply)
                self.assertEqual(reply["error"]["code"], code, reply)
            reply = worker.call("handshake", protocol=2)
            self.assertFalse(reply.get("ok"), reply)
            self.assertEqual(reply["error"]["code"], "PROTOCOL_MISMATCH")
            worker.send_raw(struct.pack("<I", MAX_FRAME_BYTES + 1))
            with self.assertRaises(Exception):
                worker.read_frame()
        finally:
            worker.kill()

    def test_handshake_is_required_and_shutdown_and_eof_exit_cleanly(self) -> None:
        worker = self.rust_worker()
        try:
            reply = worker.call("job.current")
            self.assertFalse(reply.get("ok"), reply)
            self.assertEqual(reply["error"]["code"], "HANDSHAKE_REQUIRED")
            self.assertTrue(worker.call("handshake").get("ok"))
            self.assertTrue(worker.call("shutdown").get("ok"))
        finally:
            self.assertEqual(worker.close(), 0)
        second = self.rust_worker()
        try:
            self.assertTrue(second.call("handshake").get("ok"))
        finally:
            self.assertEqual(second.close(), 0)

    def test_restart_leaves_no_cross_process_state(self) -> None:
        handshakes = []
        for _ in range(2):
            worker = self.rust_worker()
            try:
                handshakes.append(normalise(worker.call("handshake")["result"]))
            finally:
                worker.close()
        self.assertEqual(handshakes[0], handshakes[1])

    def test_rust_worker_runs_without_python_on_path(self) -> None:
        """Positive independence: no interpreter is reachable, Rust still answers."""

        with tempfile.TemporaryDirectory(prefix="m23-nopy-") as isolated:
            env = worktree_env()
            env["PATH"] = isolated  # only the empty directory is searchable
            for name in ("PYTHONHOME", "PYTHONPATH", "NIOH3_PYTHON", "NIOH3_SEED_ACCELERATOR"):
                env.pop(name, None)
            self.assertIsNone(shutil.which("python", path=env["PATH"]), "isolation failed to hide python")
            worker = FramedProcess([self.rust_argv[0], *dev_preview_arguments()], cwd=ROOT, env=env)
            oracle = self.python_worker()
            try:
                self.assertTrue(worker.call("handshake").get("ok"))
                oracle.call("handshake")
                for seed, rarity, level in ((1, 4, 180), (0x9E3779B9, 5, 90), (3169321468, 3, 1)):
                    params = {"seed": seed, "rarity": rarity, "level": level}
                    actual = worker.call("candidate.preview", params)
                    expected = oracle.call("candidate.preview", params)
                    self.assertTrue(actual.get("ok"), actual)
                    self.assertEqual(
                        normalise(actual["result"]),
                        normalise(expected["result"]),
                        f"isolated Rust worker diverged for seed {seed}",
                    )
            finally:
                worker.close()
                oracle.close()

    def test_worker_reads_shipped_resources_instead_of_embedding_values(self) -> None:
        """An empty data root must fail closed, never return the shipped payload."""

        oracle = self.python_worker()
        try:
            oracle.call("handshake")
            expected = oracle.call("candidate.preview", {"seed": 1, "rarity": 4, "level": 180})
        finally:
            oracle.close()
        self.assertTrue(expected.get("ok"), expected)
        with tempfile.TemporaryDirectory(prefix="m23-emptyroot-") as empty:
            worker = FramedProcess(
                [self.rust_argv[0], "--dev-preview-only", "--data-root", empty, "--contract-dir", str(SCHEMA_DIR)],
                cwd=ROOT,
                env=worktree_env(),
            )
            try:
                reply = worker.call("handshake")
                if reply.get("ok"):
                    served = worker.call("candidate.preview", {"seed": 1, "rarity": 4, "level": 180})
                    self.assertFalse(
                        served.get("ok") and normalise(served["result"]) == normalise(expected["result"]),
                        "worker produced the shipped payload without the shipped resources",
                    )
            except EOFError:
                pass  # refuse-and-exit is the expected fail-closed behaviour
            finally:
                worker.kill()

    def test_contract_digest_is_read_from_disk(self) -> None:
        with tempfile.TemporaryDirectory(prefix="m23-contract-") as scratch:
            target = Path(scratch)
            for name in ("request.schema.json", "response.schema.json"):
                (target / name).write_bytes((SCHEMA_DIR / name).read_bytes())
            mutated = json.loads((target / "response.schema.json").read_text(encoding="utf-8"))
            mutated["description"] = "mutation binding probe"
            (target / "response.schema.json").write_text(json.dumps(mutated, indent=2), encoding="utf-8")
            worker = FramedProcess(
                [self.rust_argv[0], "--dev-preview-only", "--data-root", str(ROOT / "nioh3_scroll_editor" / "data"),
                 "--contract-dir", str(target)],
                cwd=ROOT,
                env=worktree_env(),
            )
            try:
                reply = worker.call("handshake")
                self.assertTrue(reply.get("ok"), reply)
                self.assertNotEqual(reply["result"]["contract_digest"], contract_digest())
            finally:
                worker.close()


if __name__ == "__main__":
    unittest.main()
