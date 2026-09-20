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

from jsonschema import Draft7Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "packages" / "contracts"
MAX_FRAME_BYTES = 4 * 1024 * 1024
WORKER_ROLE = "offline_search"

sys.path.insert(0, str(ROOT))

from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

REQUEST_SCHEMA = json.loads((SCHEMA_DIR / "request.schema.json").read_text(encoding="utf-8"))
RESPONSE_SCHEMA = json.loads((SCHEMA_DIR / "response.schema.json").read_text(encoding="utf-8"))
RESPONSE_VALIDATOR = Draft7Validator(RESPONSE_SCHEMA)


def schema_methods() -> tuple[str, ...]:
    """Every method the versioned request contract can express."""

    names = {
        branch["properties"]["method"]["const"]
        for branch in REQUEST_SCHEMA["oneOf"]
        if branch.get("properties", {}).get("method", {}).get("const")
    }
    return tuple(sorted(names))


def assert_contract_frame(case: unittest.TestCase, frame: dict, label: str) -> None:
    """Every frame a worker emits must satisfy the versioned response contract."""

    errors = sorted(RESPONSE_VALIDATOR.iter_errors(frame), key=lambda error: list(error.path))
    case.assertFalse(
        errors,
        f"{label} violates response.schema.json: "
        + "; ".join(f"{list(error.path)}: {error.message}" for error in errors),
    )


def search_surface_served(worker: "FramedProcess") -> bool:
    """Whether the worker serves `search.start`.

    A contract-valid body with a deliberately wrong context digest answers an
    error code instead of starting a job, so the probe is cheap and side-effect
    free. `UNSUPPORTED_METHOD` means the surface is not implemented yet, in which
    case no acceleration capability may be advertised for it.
    """

    probe = {
        "query": {
            "playthrough": 3,
            "rarity": 4,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": None,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        },
        "context_digest": "0" * 64,
        "result_count": 1,
        "page_trials": 1,
        "job_trials": 1,
        "allow_cpu_fallback": False,
        "resume_token": None,
    }
    reply = worker.call("search.start", probe)
    if reply.get("ok"):
        return True
    return reply["error"]["code"] != "UNSUPPORTED_METHOD"


def schema_valid_params(method: str, context_digest: str) -> dict:
    """Minimal parameters that satisfy `request.schema.json` for one method.

    These are well-formed on purpose: a schema-invalid body answers
    INVALID_REQUEST for every method and could not separate "validated but not
    implemented" from "served".
    """

    if method == "search.catalog":
        return {"playthrough": 3, "rarity": 4, "locale": "zh-CN"}
    if method == "recommended_level.resolve":
        return {"displayed_level": 180}
    if method == "cache.register":
        return {"cache_json": "{}"}
    if method == "candidate.preview":
        return {"seed": 1, "rarity": 4, "level": 180}
    if method == "search.start":
        return {
            "query": {
                "playthrough": 3,
                "rarity": 4,
                "level": 180,
                "primary_effect_ids": [],
                "required_secondary_ids": [],
                "required_secondary_id_groups": [],
                "grace_effect_id": None,
                "minimum_roll_percent_by_effect_id": [],
                "auxiliary": {
                    "required_terrain_effect_keys": [],
                    "required_terrain_effect_key_groups": [],
                    "required_special_rule_keys": [],
                    "required_special_rule_key_groups": [],
                    "required_enemy_lookup_keys": [],
                    "required_enemy_lookup_key_groups": [],
                },
            },
            "context_digest": context_digest,
            "result_count": 1,
            "page_trials": 1,
            "job_trials": 1,
            "allow_cpu_fallback": False,
            "resume_token": None,
        }
    if method in ("job.snapshot", "job.cancel"):
        return {"job_id": "00000000-0000-0000-0000-000000000000"}
    if method == "candidate.export":
        return {
            "job_id": "00000000-0000-0000-0000-000000000000",
            "candidate_id": "0" * 64,
        }
    return {}

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
# The version-bound proof fields both roles publish once an explicit game file
# version selects a production identity. They are compared field-for-field so a
# handshake cannot silently drop or reinterpret one of them.
PRODUCTION_PROOF_FIELDS = (
    "game_file_version",
    "versioned_resource_dir",
    "bundle_digest",
    "versioned_digest",
    "context_digest",
    "legacy_context_digest",
    "production_authority",
)
# One explicit production version both roles are launched with, so the
# version-bound `context_digest` is a deliberate shared authority rather than
# whatever the host happens to resolve.
PRODUCTION_GAME_FILE_VERSION = "2.0.2.0"
# The other shipped version: a distinct primary identity that must never be
# accepted the way the production identity is.
OTHER_GAME_FILE_VERSION = "2.0.0.2"
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
# Methods the worker must serve with full parameter validation. The set is
# asserted against real behaviour, so adding or removing a method in the worker
# forces a deliberate update here instead of letting a stale list pass.
SUPPORTED_METHODS = (
    "handshake",
    "candidate.preview",
    "search.start",
    "job.snapshot",
    "job.current",
    "job.cancel",
    "candidate.export",
    # M2.3c application methods: both validate parameters before routing, so a
    # wrong-shaped request still answers INVALID_REQUEST.
    "recommended_level.resolve",
    "cache.register",
    # M2.3c catalog: `search.catalog` is served from the shipped tables, so the
    # last schema-known method this slice refused is retired.
    "search.catalog",
    "shutdown",
)
# Schema-known methods this slice deliberately does not implement. They must
# answer UNSUPPORTED_METHOD for a schema-valid request, and full validation still
# applies before that short-circuit.
PENDING_METHODS: tuple[str, ...] = ()
# The read-only preview/search slice implements the D3D11 effect-filter path (the
# partial-effect forward filter with its certified recomposition), so its
# capability is the live probe intersected with that implementation, which is
# what the Rust handshake publishes. The NG4/NG5 cache is a different rule: it
# must not be advertised at all while it is unported.
EFFECT_FILTER_IMPLEMENTED = True

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
        # The read-only worker specifically: `crates/nioh3-protected` also ships a
        # `*-worker` binary, and this gate must never drive the protected host.
        if override and manifest == Path(override):
            return manifest, (bins[0] if bins else name)
        if name != "nioh3-worker" and "nioh3-readonly-worker" not in bins:
            continue
        return manifest, (bins[0] if bins else name)
    raise AssertionError(
        "the development read-only worker crate is required by this gate and was not found under "
        f"{ROOT / 'crates'} (expected a package or bin name containing 'worker'); "
        "set NIOH3_M23_WORKER_MANIFEST to override the manifest path"
    )


def dev_preview_arguments() -> list[str]:
    """The explicit development acknowledgement plus the read-only roots.

    Both roles are launched with the same explicit production game file version
    so the resolved `context_digest` is a deliberate shared authority. Passing no
    identity at all is the fail-closed start both workers must refuse.
    """

    return [
        "--dev-preview-only",
        "--data-root",
        str(ROOT / "nioh3_scroll_editor" / "data"),
        "--contract-dir",
        str(SCHEMA_DIR),
        "--game-file-version",
        PRODUCTION_GAME_FILE_VERSION,
    ]


def python_preview_arguments() -> list[str]:
    """The Python worker argv: same acknowledgement, roots and identity."""

    return [
        sys.executable,
        "-u",
        "-m",
        "nioh3_scroll_editor.search_worker",
        "--game-file-version",
        PRODUCTION_GAME_FILE_VERSION,
    ]


class FramedProcess:
    """Framed JSON stdio client for one worker subprocess."""

    def __init__(
        self,
        argv: list[str],
        *,
        cwd: Path,
        env: dict[str, str],
        name: str = "worker",
    ) -> None:
        self.process = subprocess.Popen(
            argv,
            cwd=str(cwd),
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.name = name
        self.counter = 0
        self.pending_id: str | None = None

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
        frame = json.loads(body.decode("utf-8"))
        # The contract carries the request id back on every reply; a worker
        # that answers the wrong frame would otherwise look healthy.
        if self.pending_id is not None:
            if frame.get("id") != self.pending_id:
                raise AssertionError(
                    f"{self.name} replied to id {frame.get('id')!r} "
                    f"while {self.pending_id!r} was outstanding"
                )
        errors = sorted(
            RESPONSE_VALIDATOR.iter_errors(frame), key=lambda error: list(error.path)
        )
        if errors:
            raise AssertionError(
                f"{self.name} frame violates response.schema.json: "
                + "; ".join(
                    f"{list(error.path)}: {error.message}" for error in errors
                )
            )
        return frame

    def call(self, method: str, params: dict | None = None, *, protocol: int = 1) -> dict:
        self.pending_id = self.send(method, params, protocol=protocol)
        return self.read_frame()

    def close(self) -> int:
        assert self.process.stdin is not None
        self.process.stdin.close()
        return self.process.wait(timeout=60)

    def kill(self) -> None:
        self.process.kill()
        self.process.wait(timeout=60)


def worker_target_dir() -> Path:
    """The cargo target directory this gate builds into.

    The rule lives in one place (`tests/migration/cargo_target.py`):
    `CARGO_TARGET_DIR` always wins, otherwise the platform temp directory is
    used, so a fresh target tree is never written into the checkout.
    """

    return Path(resolved_cargo_target_dir("worker"))


def worktree_env() -> dict[str, str]:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(worker_target_dir())
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
        cls.python_argv = python_preview_arguments()

    def rust_worker(self) -> FramedProcess:
        return FramedProcess(self.rust_argv, cwd=ROOT, env=worktree_env(), name="rust worker")

    def python_worker(self) -> FramedProcess:
        return FramedProcess(
            self.python_argv, cwd=ROOT, env=worktree_env(), name="python worker"
        )

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

    def test_a_missing_identity_is_refused_by_both_roles(self) -> None:
        """Neither role may start, or negotiate a handshake, without an identity.

        Omitting both `--game-file-version` and `--legacy-test-context` is the
        fail-closed production start: the process must exit non-zero with an
        explanation and publish no handshake.
        """

        rust = subprocess.run(
            [
                self.rust_argv[0],
                "--dev-preview-only",
                "--data-root",
                str(ROOT / "nioh3_scroll_editor" / "data"),
                "--contract-dir",
                str(SCHEMA_DIR),
            ],
            cwd=str(ROOT),
            env=worktree_env(),
            input=b"",
            capture_output=True,
            timeout=600,
        )
        self.assertNotEqual(rust.returncode, 0, "the Rust worker started without an identity")
        self.assertIn("--game-file-version", rust.stderr.decode("utf-8", "replace"))
        python = subprocess.run(
            [sys.executable, "-u", "-m", "nioh3_scroll_editor.search_worker"],
            cwd=str(ROOT),
            env=worktree_env(),
            input=b"",
            capture_output=True,
            timeout=600,
        )
        self.assertNotEqual(python.returncode, 0, "the Python worker started without an identity")
        self.assertIn(
            "--game-file-version",
            python.stderr.decode("utf-8", "replace"),
            "the Python refusal must name the missing identity flag",
        )

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
        # Both roles were launched with the same explicit version and the same
        # accelerator policy, so every resolved proof field must agree, and the
        # version-bound `context_digest` is the shared production authority.
        for field in PRODUCTION_PROOF_FIELDS:
            self.assertIn(field, actual, field)
            self.assertIn(field, expected, field)
            self.assertEqual(actual[field], expected[field], field)
        self.assertEqual(actual["game_file_version"], PRODUCTION_GAME_FILE_VERSION)
        self.assertIs(actual["production_authority"], True)
        self.assertIs(expected["production_authority"], True)
        self.assertNotEqual(
            actual["context_digest"],
            actual["legacy_context_digest"],
            "the version-bound authority must not be the pre-version proof digest",
        )

    def test_a_different_explicit_version_is_a_different_authority(self) -> None:
        """A second shipped version must resolve a distinct primary identity.

        The two versions share one pre-version `legacy_context_digest` (same
        profile and same whole-root digest), which is exactly why that field can
        never authorize a candidate, cache, or resume. The version-bound
        `context_digest` must differ.
        """

        first = self.python_worker()
        second = FramedProcess(
            [
                *self.python_argv[:4],
                "--game-file-version",
                OTHER_GAME_FILE_VERSION,
            ],
            cwd=ROOT,
            env=worktree_env(),
            name="python worker (other version)",
        )
        try:
            primary = first.call("handshake")["result"]["context"]
            other = second.call("handshake")["result"]["context"]
        finally:
            first.close()
            second.close()
        self.assertEqual(other["game_file_version"], OTHER_GAME_FILE_VERSION)
        self.assertNotEqual(
            primary["context_digest"],
            other["context_digest"],
            "two explicit versions must not share a version-bound authority",
        )
        self.assertEqual(
            primary["legacy_context_digest"],
            other["legacy_context_digest"],
            "the proof-only pre-version digest is shared, which is why it is not authority",
        )
        self.assertNotEqual(
            primary["versioned_resource_dir"],
            other["versioned_resource_dir"],
            "the two versions must select different versioned resource directories",
        )

    def test_every_contract_method_is_served_with_validation_or_named_pending(self) -> None:
        """Every contract method is either served with validation or named as pending.

        The previous revision of this test asserted a constant against itself,
        which could never fail. The check below derives the contract's method set
        from `request.schema.json` and probes each method twice: malformed
        parameters must return the Python worker's validation code, and a
        schema-valid body must answer UNSUPPORTED_METHOD exactly for the methods
        declared pending. Implementing or removing a method therefore forces a
        deliberate update of these lists.
        """

        worker = self.rust_worker()
        python = self.python_worker()
        try:
            handshake = worker.call("handshake")
            self.assertTrue(handshake.get("ok"), handshake)
            context = handshake["result"]["context"]["context_digest"]
            python.call("handshake")
            for method in schema_methods():
                if method in ("handshake", "shutdown"):
                    continue
                reply = worker.call(method, {"definitely_not_a_parameter": True})
                self.assertFalse(reply.get("ok"), (method, reply))
                expected = python.call(method, {"definitely_not_a_parameter": True})
                self.assertFalse(expected.get("ok"), (method, expected))
                self.assertEqual(
                    reply["error"]["code"],
                    expected["error"]["code"],
                    f"{method} must validate parameters like the Python worker",
                )

                well_formed = worker.call(
                    method, schema_valid_params(method, context)
                )
                code = None if well_formed.get("ok") else well_formed["error"]["code"]
                if method in PENDING_METHODS:
                    self.assertEqual(
                        code,
                        "UNSUPPORTED_METHOD",
                        f"{method} is declared pending but the worker answered {code}",
                    )
                else:
                    self.assertNotEqual(
                        code,
                        "UNSUPPORTED_METHOD",
                        f"{method} is declared served but the worker rejected it as "
                        "unimplemented; update SUPPORTED_METHODS and the migration "
                        "record together",
                    )
            declared = set(SUPPORTED_METHODS) | set(PENDING_METHODS) | {"handshake", "shutdown"}
            self.assertEqual(
                declared,
                set(schema_methods()),
                "every contract method must be declared served or pending",
            )
            self.assertTrue(worker.call("shutdown").get("ok"))
        finally:
            worker.close()
            python.close()

    def test_unknown_methods_are_rejected_like_the_python_worker(self) -> None:
        """A method outside the contract must not fall through to a success."""

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust.call("handshake")
            python.call("handshake")
            invented = "search.not_a_real_method"
            expected = python.call(invented, {})
            actual = rust.call(invented, {})
        finally:
            rust.close()
            python.close()
        self.assertFalse(actual.get("ok"), actual)
        self.assertFalse(expected.get("ok"), expected)
        self.assertEqual(actual["error"]["code"], expected["error"]["code"])

    def test_handshake_capabilities_reflect_the_real_probes(self) -> None:
        """Capabilities are contract data, so each value must be a real probe.

        The Python worker answers these fields from live probes of the same two
        DLLs, so it is the oracle here. A hard-coded `false` is a parity defect
        even when it looks conservative: the broker and desktop renderer use the
        field to decide whether an accelerated search can be offered at all.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_reply = rust.call("handshake")
            python_reply = python.call("handshake")
            served = search_surface_served(rust)
        finally:
            rust.close()
            python.close()
        assert_contract_frame(self, rust_reply, "rust handshake")
        assert_contract_frame(self, python_reply, "python handshake")
        rust_caps = rust_reply["result"]["capabilities"]
        python_caps = python_reply["result"]["capabilities"]

        # The NG4/NG5 save-bound cache is ported, so the advertised playthroughs
        # must be exactly the shipped worker's: the Rust worker compiles the
        # registered map into its native collector (`SearchFactory::cached_collector`)
        # and the search parity gate compares both workers through that map.
        self.assertEqual(
            rust_caps.get("cached_rarity5_playthroughs"),
            python_caps.get("cached_rarity5_playthroughs"),
            "the ported cache route must advertise the playthroughs it serves",
        )

        # Expected capabilities are the measured native availability intersected
        # with what this worker actually implements.
        expected = {
            "playthroughs": python_caps["playthroughs"],
            "rarities": python_caps["rarities"],
            "cpu_exact_replay": python_caps["cpu_exact_replay"],
            "save_write": False,
            "runtime_calls": False,
            # Strict GPU is the default and bulk CPU is the explicit opt-in.
            "bulk_cpu_requires_opt_in": True,
            # Real probe, but only once the search surface that consumes it exists.
            "cuda_pivot_and_auxiliary": bool(python_caps["cuda_pivot_and_auxiliary"])
            and served,
            # Real probe, gated on the effect-filter path being ported.
            "directcompute_effect_filter": bool(
                python_caps["directcompute_effect_filter"]
            )
            and EFFECT_FILTER_IMPLEMENTED,
            # The save-bound NG4/NG5 cached route is ported, so the playthroughs
            # it serves are advertised exactly like the shipped worker's.
            "cached_rarity5_playthroughs": python_caps.get(
                "cached_rarity5_playthroughs"
            ),
        }
        self.assertEqual(
            rust_caps,
            expected,
            "capabilities must equal measured native availability intersected with "
            f"implemented features (search served: {served}, effect filter "
            f"implemented: {EFFECT_FILTER_IMPLEMENTED}); cuda_pivot_and_auxiliary "
            "and bulk_cpu_requires_opt_in must reflect the live probe and the CPU "
            "opt-in policy, not a constant copied from either side",
        )

    def test_accelerator_absence_degrades_without_faking_capability(self) -> None:
        """A missing accelerator must stay visible instead of reporting support."""

        missing = ROOT / "bin" / "definitely-not-present-accelerator.dll"
        self.assertFalse(missing.exists(), "the control path must not exist")
        argv = [
            *self.rust_argv,
            "--accelerator",
            str(missing),
        ]
        worker = FramedProcess(argv, cwd=ROOT, env=worktree_env(), name="rust worker (no accelerator)")
        python = self.python_worker()
        try:
            reply = worker.call("handshake")
            expected = python.call("handshake")["result"]
        finally:
            worker.close()
            python.close()
        self.assertTrue(reply.get("ok"), reply)
        result = reply["result"]
        context = result["context"]
        self.assertIsNone(
            context["seed_accelerator_abi"],
            "a missing accelerator must not report an ABI identity",
        )
        self.assertIsNone(context["seed_accelerator_build_id"])
        # The Python worker still probes its own packaged DLL, so it reports the
        # available value; the Rust worker must report the truth for its own
        # (absent) accelerator rather than copying the Python constant.
        self.assertFalse(result["capabilities"]["cuda_pivot_and_auxiliary"])
        self.assertEqual(result["capabilities"]["playthroughs"], expected["capabilities"]["playthroughs"])
        self.assertEqual(result["capabilities"]["rarities"], expected["capabilities"]["rarities"])

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
                [
                    self.rust_argv[0],
                    "--dev-preview-only",
                    "--data-root",
                    empty,
                    "--contract-dir",
                    str(SCHEMA_DIR),
                    "--game-file-version",
                    PRODUCTION_GAME_FILE_VERSION,
                ],
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
                [
                    self.rust_argv[0],
                    "--dev-preview-only",
                    "--data-root",
                    str(ROOT / "nioh3_scroll_editor" / "data"),
                    "--contract-dir",
                    str(target),
                    "--game-file-version",
                    PRODUCTION_GAME_FILE_VERSION,
                ],
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
