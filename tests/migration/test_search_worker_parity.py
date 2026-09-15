"""End-to-end search acceptance gate for the Rust read-only worker (M2.3b).

This gate drives the real Rust worker subprocess over the shipped framed-JSON
protocol and compares it against the shipped Python worker for the search
surface: the canonical v0.7.5 three-rule regression that must finish inside one
continuing job, effect and enemy page identity/order/cursor parity, responsive
cancellation with a valid resume, resume-token rejection, candidate ownership
and export, accelerator-absence behaviour, and the unported NG4/NG5 cache
boundary.

It never skips and never mocks: a missing Rust bin target, a missing method, or
a stale binary is a failure with an owner-actionable message. Every expected
error code is taken from the Python worker's answer to the same request, so the
gate cannot invent a code that the product does not use.
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
import time
import tomllib
import unittest

from jsonschema import Draft7Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "packages" / "contracts"
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
ACCELERATOR = ROOT / "bin" / "nioh3_seed_accelerator.dll"
MAX_FRAME_BYTES = 4 * 1024 * 1024

sys.path.insert(0, str(ROOT))

from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

RESPONSE_VALIDATOR = Draft7Validator(
    json.loads((SCHEMA_DIR / "response.schema.json").read_text(encoding="utf-8"))
)

# Canonical v0.7.5 regression: the three named special rules whose first match
# is pivot trial 158,614,759 (docs/product/releases/v0.7.5.md).
REGRESSION_RULE_KEYS = (64956, 113, 20893)
REGRESSION_SEED = 226061463
REGRESSION_TRIAL = 158614759

TERMINAL_STATES = ("completed", "cancelled", "failed")
POLL_SECONDS = 0.05

# The supported routes are the NG3 auxiliary route (terrain keys, special rule
# keys and groups, enemy lookup keys and groups), the R4 primary route, and the
# shipped full-family replay that serves a rarity-3 primary search and an
# unconstrained query. Rarity-5 effect searches, secondary/roll routes and the
# effect-preimage family stay refused with their own named reasons.
RULE_ROUTE_KEY = 113
PRIMARY_ROUTE_EFFECT = 30543

# The densest complete rarity-3 ordinary set found in the first candidate seeds
# (`deliverables/m23d-preimage/scripts/capture_preimage_vectors.py`). It reaches
# 63 verified matches inside the contract's 100M-trial page cap, which is the
# strongest window the shipped protocol can exercise end to end.
COMPLETE_PREIMAGE_PRIMARY = 17991
COMPLETE_PREIMAGE_SECONDARY_IDS = (19630, 20781, 30543)

# A rarity-5 complete composition the shipped layer accepts: Seed 3 composes
# this ordinary set and terminates in Grace 0x6553, and every requested
# secondary can be drawn into a normal slot.
RARITY5_PRIMARY = 20781
RARITY5_SECONDARY_IDS = (6410, 12028, 28203, 41127)
RARITY5_GRACE_ID = 0x6553

# The same rarity-5 shape with a secondary only the single deep slot can
# produce: the shipped layer refuses it before searching, so the worker must
# refuse it by name too.
RARITY5_DEEP_ONLY_PRIMARY = 41041
RARITY5_DEEP_ONLY_SECONDARY_IDS = (13555, 15994, 44634, 54282)

# Partial-effect forward filter: a rarity-3 request that names only some of its
# ordinary slots. The expected Seed and cursor are the shipped solver's own
# answer for a 100M-trial window
# (`deliverables/m23d-preimage/scripts/probe_forward_filter_route.py`).
PARTIAL_FILTER_PRIMARY = 60020
PARTIAL_FILTER_SECONDARY = 12028
PARTIAL_FILTER_SEED = 90790139
PARTIAL_FILTER_CURSOR = 26885


def complete_rarity5_query() -> dict:
    """A rarity-5 query that names every ordinary slot and its Grace."""

    return base_query(
        rarity=5,
        primary_effect_ids=[RARITY5_PRIMARY],
        required_secondary_ids=list(RARITY5_SECONDARY_IDS),
        grace_effect_id=RARITY5_GRACE_ID,
    )


def one_wildcard_rarity5_query() -> dict:
    """A rarity-5 query with an unrestricted primary and one open ordinary slot."""

    return base_query(
        rarity=5,
        primary_effect_ids=[],
        required_secondary_ids=list(RARITY5_SECONDARY_IDS),
        grace_effect_id=RARITY5_GRACE_ID,
    )


def complete_preimage_query() -> dict:
    """A rarity-3 query that names every ordinary slot."""

    return base_query(
        rarity=3,
        primary_effect_ids=[COMPLETE_PREIMAGE_PRIMARY],
        required_secondary_ids=list(COMPLETE_PREIMAGE_SECONDARY_IDS),
    )


def directcompute_available() -> bool:
    """Whether this machine can run the shipped DirectCompute preimage sweep."""

    from nioh3_scroll_editor.effect_preimage_accelerator import (
        d3d11_effect_acceleration_available,
    )

    return bool(d3d11_effect_acceleration_available())


def composed_roll_percent(query: dict, seed: int) -> int:
    """The roll percent the certified generator gives the queried primary."""

    from nioh3_scroll_editor.effect_sequence import (
        generate_ng3_rarity3_effect_sequence,
    )

    sequence = generate_ng3_rarity3_effect_sequence(seed)
    primary = query["primary_effect_ids"][0]
    for effect in (sequence.primary, *sequence.secondaries):
        if effect.effect_id == primary:
            return int(effect.roll_percent)
    raise AssertionError(f"Seed {seed} does not compose the queried primary")


def contract_digest() -> str:
    digest = hashlib.sha256()
    digest.update((SCHEMA_DIR / "request.schema.json").read_bytes())
    digest.update((SCHEMA_DIR / "response.schema.json").read_bytes())
    return digest.hexdigest()


def develop_worker_target() -> tuple[Path, str]:
    """Return (manifest, bin name) for the development worker; missing = failure."""

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
        "the development worker crate is required by this gate and was not found "
        f"under {ROOT / 'crates'}"
    )


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


class WorkerFailure(AssertionError):
    """A protocol-level failure with the worker's own error code."""

    def __init__(self, label: str, reply: dict) -> None:
        error = reply.get("error", {})
        super().__init__(f"{label} failed: {error.get('code')}: {error.get('message')}")
        self.code = error.get("code")
        self.reply = reply


class FramedProcess:
    """Framed JSON stdio client with contract validation on every reply."""

    def __init__(self, argv: list[str], *, name: str) -> None:
        self.name = name
        self.process = subprocess.Popen(
            argv,
            cwd=str(ROOT),
            env=worktree_env(),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.counter = 0
        self.pending_id: str | None = None

    def call(self, method: str, params: dict | None = None) -> dict:
        self.counter += 1
        request_id = f"m23b-{self.counter}"
        body = json.dumps(
            {
                "protocol": 1,
                "id": request_id,
                "method": method,
                "params": params or {},
            },
            separators=(",", ":"),
        ).encode("utf-8")
        assert self.process.stdin is not None
        self.process.stdin.write(struct.pack("<I", len(body)) + body)
        self.process.stdin.flush()
        self.pending_id = request_id
        return self.read_frame()

    def read_frame(self) -> dict:
        assert self.process.stdout is not None
        header = self.process.stdout.read(4)
        if len(header) != 4:
            raise EOFError(f"{self.name} closed stdout")
        (size,) = struct.unpack("<I", header)
        if size > MAX_FRAME_BYTES:
            raise ValueError(f"{self.name} frame exceeds the contract limit: {size}")
        body = self.process.stdout.read(size)
        if len(body) != size:
            raise EOFError(f"{self.name} frame truncated")
        frame = json.loads(body.decode("utf-8"))
        if self.pending_id is not None and frame.get("id") != self.pending_id:
            raise AssertionError(
                f"{self.name} replied to {frame.get('id')!r} while "
                f"{self.pending_id!r} was outstanding"
            )
        errors = sorted(
            RESPONSE_VALIDATOR.iter_errors(frame), key=lambda error: list(error.path)
        )
        if errors:
            raise AssertionError(
                f"{self.name} frame violates response.schema.json: "
                + "; ".join(f"{list(error.path)}: {error.message}" for error in errors)
            )
        return frame

    def result(self, method: str, params: dict | None = None, *, label: str | None = None) -> dict:
        reply = self.call(method, params)
        if not reply.get("ok"):
            raise WorkerFailure(label or f"{self.name} {method}", reply)
        return reply["result"]

    def error_code(self, method: str, params: dict | None = None) -> str | None:
        reply = self.call(method, params)
        if reply.get("ok"):
            return None
        return reply["error"]["code"]

    def close(self) -> int:
        assert self.process.stdin is not None
        self.process.stdin.close()
        return self.process.wait(timeout=120)

    def kill(self) -> None:
        self.process.kill()
        self.process.wait(timeout=60)


def base_query(**overrides) -> dict:
    query = {
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
    }
    query.update(overrides)
    return query


def supported_auxiliary(**overrides) -> dict:
    """Auxiliary criteria on a route this slice actually supports."""

    auxiliary = {
        "required_terrain_effect_keys": [],
        "required_terrain_effect_key_groups": [],
        "required_special_rule_keys": [RULE_ROUTE_KEY],
        "required_special_rule_key_groups": [],
        "required_enemy_lookup_keys": [],
        "required_enemy_lookup_key_groups": [],
    }
    auxiliary.update(overrides)
    return auxiliary


def supported_query(**overrides) -> dict:
    """A query the M2.3b1 compiler accepts and returns real candidates for."""

    return base_query(auxiliary=supported_auxiliary(), **overrides)


def search_params(
    query: dict,
    context_digest: str,
    *,
    result_count: int = 1,
    page_trials: int = 1_000_000,
    job_trials: int = 1_000_000,
    continue_until_complete: bool = False,
    allow_cpu_fallback: bool = False,
    resume_token: str | None = None,
    cache_id: str | None = None,
) -> dict:
    params = {
        "query": query,
        "context_digest": context_digest,
        "result_count": result_count,
        "page_trials": page_trials,
        "job_trials": job_trials,
        "allow_cpu_fallback": allow_cpu_fallback,
        "resume_token": resume_token,
    }
    if continue_until_complete:
        params["continue_until_complete"] = True
    if cache_id is not None:
        params["cache_id"] = cache_id
    return params


def candidate_seeds(snapshot: dict) -> list[int]:
    return [int(candidate["seed"]) for candidate in snapshot.get("candidates", [])]


def candidate_cursors(snapshot: dict) -> list[int]:
    """The 1-based solver trial each published candidate came from."""

    return [int(candidate["cursor"]) for candidate in snapshot.get("candidates", [])]


def wait_for_terminal(worker: FramedProcess, job_id: str, *, timeout: float) -> dict:
    deadline = time.monotonic() + timeout
    snapshot = worker.result("job.snapshot", {"job_id": job_id}, label="job.snapshot")
    while snapshot["state"] not in TERMINAL_STATES:
        if time.monotonic() > deadline:
            raise AssertionError(
                f"{worker.name} job {job_id} did not finish within {timeout}s "
                f"(state {snapshot['state']}, cursor {snapshot.get('cursor')})"
            )
        time.sleep(POLL_SECONDS)
        snapshot = worker.result("job.snapshot", {"job_id": job_id}, label="job.snapshot")
    return snapshot


class SearchWorkerParityTests(unittest.TestCase):
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
        binary_path = Path(env["CARGO_TARGET_DIR"]) / "debug" / f"{binary}.exe"
        common = [
            "--dev-preview-only",
            "--data-root",
            str(DATA_ROOT),
            "--contract-dir",
            str(SCHEMA_DIR),
        ]
        cls.rust_argv = [str(binary_path), *common, "--accelerator", str(ACCELERATOR)]
        cls.rust_without_accelerator_argv = [
            str(binary_path),
            *common,
            "--accelerator",
            str(ROOT / "bin" / "definitely-not-present-accelerator.dll"),
        ]
        cls.python_argv = [sys.executable, "-u", "-m", "nioh3_scroll_editor.search_worker"]
        cls.search_pending = cls._search_surface_pending()
        cls.cuda_available = cls._cuda_available()

    @classmethod
    def _search_surface_pending(cls) -> bool:
        """True while the Rust worker still answers UNSUPPORTED_METHOD.

        A schema-valid request is used so a parameter-validation answer cannot be
        mistaken for an unimplemented method.
        """

        worker = FramedProcess(cls.rust_argv, name="rust worker")
        try:
            context = worker.result("handshake")["context"]["context_digest"]
            reply = worker.call(
                "search.start",
                search_params(base_query(), context, page_trials=1000, job_trials=1000),
            )
        finally:
            worker.close()
        return reply.get("ok") is False and reply["error"]["code"] == "UNSUPPORTED_METHOD"

    @classmethod
    def _cuda_available(cls) -> bool:
        """Whether the shipped worker's own probe reports CUDA acceleration.

        The acceptance target is the same job result under the policy the
        platform can actually execute: with CUDA the strict default is used,
        without it the documented CPU opt-in keeps the run meaningful on a
        runner that has no GPU. The strict-mode failure itself is asserted
        separately in the accelerator-absence check.
        """

        worker = FramedProcess(cls.python_argv, name="python worker")
        try:
            capabilities = worker.result("handshake")["capabilities"]
        finally:
            worker.close()
        return bool(capabilities.get("cuda_pivot_and_auxiliary"))

    def policy(self) -> dict:
        return {"allow_cpu_fallback": not type(self).cuda_available}

    def setUp(self) -> None:
        if type(self).search_pending:
            raise AssertionError(
                "the Rust worker still answers UNSUPPORTED_METHOD for a valid search.start, "
                "so this M2.3b acceptance check cannot run. Owners: /root/m23b_jobs "
                "(protocol/engine/payload) with /root/m23b_native_search (native page "
                "driver). This gate fails instead of skipping, so an unimplemented search "
                "surface can never look like a passing acceptance result."
            )

    def rust_worker(self) -> FramedProcess:
        return FramedProcess(self.rust_argv, name="rust worker")

    def python_worker(self) -> FramedProcess:
        return FramedProcess(self.python_argv, name="python worker")

    def handshake_context(self, worker: FramedProcess) -> dict:
        return worker.result("handshake")["context"]

    @staticmethod
    def regression_auxiliary(shape: str) -> dict:
        """The canonical rule selection in both shipped request shapes.

        The desktop UI sends one singleton group per selected rule; the flat key
        list is the older equivalent. Both must produce the same job.
        """

        keys = list(REGRESSION_RULE_KEYS)
        if shape == "flat":
            special_keys, special_groups = keys, []
        elif shape == "singleton_groups":
            special_keys, special_groups = [], [[key] for key in keys]
        else:
            raise AssertionError(f"unknown rule shape {shape!r}")
        return {
            "required_terrain_effect_keys": [],
            "required_terrain_effect_key_groups": [],
            "required_special_rule_keys": special_keys,
            "required_special_rule_key_groups": special_groups,
            "required_enemy_lookup_keys": [],
            "required_enemy_lookup_key_groups": [],
        }

    def test_regression_query_completes_inside_one_continuing_job(self) -> None:
        """The canonical v0.7.5 case must finish in one job, not stop at 10M.

        Both the bounded control and the continuing job are compared against the
        Python worker on the same parameters. The terminal cursor is asserted
        exactly: this is a deterministic non-cancelled job, so a different chunk
        boundary is a real finding and must be escalated with resume evidence
        (no skipped and no duplicated matches) rather than accepted here.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)
            policy = self.policy()

            for shape in ("singleton_groups", "flat"):
                with self.subTest(rule_shape=shape):
                    query = base_query(auxiliary=self.regression_auxiliary(shape))
                    bounded_params = search_params(
                        query,
                        rust_context,
                        page_trials=1_000_000,
                        job_trials=10_000_000,
                        **policy,
                    )
                    python_bounded = python.result(
                        "search.start",
                        {**bounded_params, "context_digest": python_context},
                        label=f"python bounded {shape}",
                    )
                    python_bounded_snapshot = wait_for_terminal(
                        python, python_bounded["job_id"], timeout=300
                    )
                    rust_bounded = rust.result(
                        "search.start", bounded_params, label=f"rust bounded {shape}"
                    )
                    rust_bounded_snapshot = wait_for_terminal(
                        rust, rust_bounded["job_id"], timeout=300
                    )
                    self.assertEqual(
                        rust_bounded_snapshot["stop_reason"],
                        "budget_reached",
                        "the bounded API must keep its old stop instead of continuing",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_bounded_snapshot),
                        [],
                        "the bounded 10M budget must not reach the 158,614,759 trial match",
                    )
                    self.assertEqual(
                        rust_bounded_snapshot["stop_reason"],
                        python_bounded_snapshot["stop_reason"],
                    )
                    self.assertEqual(
                        rust_bounded_snapshot["cursor"],
                        python_bounded_snapshot["cursor"],
                        f"{shape}: bounded cursor differs from the Python worker",
                    )

                    continuing_params = search_params(
                        query,
                        rust_context,
                        page_trials=100_000_000,
                        job_trials=200_000_000,
                        continue_until_complete=True,
                        **policy,
                    )
                    python_started = python.result(
                        "search.start",
                        {**continuing_params, "context_digest": python_context},
                        label=f"python continuing {shape}",
                    )
                    python_final = wait_for_terminal(
                        python, python_started["job_id"], timeout=900
                    )
                    rust_started = rust.result(
                        "search.start", continuing_params, label=f"rust continuing {shape}"
                    )
                    started = time.monotonic()
                    rust_final = wait_for_terminal(rust, rust_started["job_id"], timeout=900)
                    elapsed = time.monotonic() - started

                    self.assertEqual(rust_final["state"], "completed", rust_final.get("error"))
                    self.assertEqual(rust_final["stop_reason"], "result_limit")
                    self.assertEqual(candidate_seeds(rust_final), [REGRESSION_SEED])
                    self.assertGreaterEqual(
                        rust_final["cursor"],
                        REGRESSION_TRIAL,
                        "the job must pass the known pivot trial inside the same job",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_final),
                        candidate_seeds(python_final),
                        f"{shape}: candidate identity differs from the Python worker",
                    )
                    self.assertEqual(
                        rust_final["cursor"],
                        python_final["cursor"],
                        f"{shape}: the terminal cursor of a deterministic continuing job "
                        "must match the Python worker exactly; a different chunk boundary "
                        "is a finding that needs resume evidence (no skipped or duplicated "
                        "matches) before it can be accepted",
                    )
                    self.assertEqual(
                        rust_final["stop_reason"], python_final["stop_reason"]
                    )
                    print(
                        f"# regression {shape}: seed {REGRESSION_SEED} at cursor "
                        f"{rust_final['cursor']} in {elapsed:.2f}s "
                        f"(python {python_final['cursor']})"
                    )
        finally:
            rust.close()
            python.close()

    def test_effect_and_enemy_pages_match_the_python_worker(self) -> None:
        """Page identity, order and cursor must match for real native queries."""

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)

            # Real enemy lookup keys come from the shipped preview of a known
            # Seed, so the enemy route is exercised with keys the roster really
            # contains instead of an empty-auxiliary query.
            preview = rust.result(
                "candidate.preview",
                {"seed": REGRESSION_SEED, "rarity": 4, "level": 180},
                label="candidate.preview",
            )["candidate"]
            lookup_keys = {
                variant: sorted(
                    {
                        occurrence["lookup_key"]
                        for occurrence in (
                            (preview.get("enemy_states") or {}).get(variant) or {}
                        ).get("occurrences", [])
                    }
                )
                for variant in ("solo", "expedition")
            }
            for variant, keys in lookup_keys.items():
                self.assertTrue(
                    keys,
                    f"the shipped preview for seed {REGRESSION_SEED} carries no "
                    f"{variant} occurrence, so the enemy route cannot be exercised",
                )

            queries = {
                "effect_primary": base_query(primary_effect_ids=[PRIMARY_ROUTE_EFFECT]),
                "rule_route": supported_query(),
            }
            for variant in ("solo", "expedition"):
                queries[f"enemy_{variant}"] = base_query(
                    enemy_variant=variant,
                    auxiliary=supported_auxiliary(
                        required_special_rule_keys=[],
                        required_enemy_lookup_keys=[lookup_keys[variant][0]],
                    ),
                )
            for name, query in queries.items():
                params = search_params(
                    query,
                    rust_context,
                    result_count=25,
                    page_trials=2_000_000,
                    job_trials=2_000_000,
                    **self.policy(),
                )
                python_started = python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label=f"python {name}",
                )
                python_snapshot = wait_for_terminal(python, python_started["job_id"], timeout=600)
                self.assertTrue(
                    python_snapshot["candidates"],
                    f"{name}: the Python oracle found nothing in one 2M page, so the "
                    "page parity check would be vacuous; widen the window or pick a "
                    "denser query before trusting this gate",
                )
                rust_started = rust.result(
                    "search.start", params, label=f"rust {name}"
                )
                rust_snapshot = wait_for_terminal(rust, rust_started["job_id"], timeout=600)
                self.assertEqual(
                    candidate_seeds(rust_snapshot),
                    candidate_seeds(python_snapshot),
                    f"{name}: candidate identity or order differs",
                )
                self.assertEqual(
                    rust_snapshot["cursor"],
                    python_snapshot["cursor"],
                    f"{name}: cursor differs",
                )
                self.assertEqual(
                    rust_snapshot["stop_reason"],
                    python_snapshot["stop_reason"],
                    f"{name}: stop reason differs",
                )
        finally:
            rust.close()
            python.close()

    def test_r4_primary_route_matches_the_python_worker(self) -> None:
        """The promised primary route is the R4 default primary id.

        Rarity 3 primary search is a separate unported route and is asserted
        rejected below, so this gate must not accept an R3 answer as if it were
        the promised one.
        """

        query = base_query(primary_effect_ids=[PRIMARY_ROUTE_EFFECT])
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=5,
                page_trials=2_000_000,
                job_trials=2_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust primary")["job_id"],
                timeout=600,
            )
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python primary",
                )["job_id"],
                timeout=600,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertTrue(rust_snapshot["candidates"], "the primary route found nothing")
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "R4 primary candidate identity or order differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            "R4 primary cursor differs from the Python worker",
        )

    def test_r4_primary_with_auxiliary_criteria_matches_the_python_worker(self) -> None:
        """The combined route is part of the promised surface and stays gated.

        A temporary fail-closed refusal is not acceptance: this check requires
        the real combination result, so it fails until the combination route is
        wired rather than quietly passing on a refusal.
        """

        query = base_query(
            primary_effect_ids=[PRIMARY_ROUTE_EFFECT],
            auxiliary=supported_auxiliary(),
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=5,
                page_trials=2_000_000,
                job_trials=2_000_000,
                **self.policy(),
            )
            reply = rust.call("search.start", params)
            self.assertTrue(
                reply.get("ok"),
                "the R4 primary + auxiliary combination is part of the promised "
                f"surface and must not be refused: {reply.get('error')}",
            )
            rust_snapshot = wait_for_terminal(rust, reply["result"]["job_id"], timeout=600)
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python combined",
                )["job_id"],
                timeout=600,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "combined-route candidate identity or order differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            "combined-route cursor differs from the Python worker",
        )

    def test_partial_effect_forward_filter_matches_the_python_worker(self) -> None:
        """The partial-effect forward filter serves a request the pivots cannot.

        A rarity-3 request that names only some ordinary slots has no
        complete-composition preimage, so the shipped worker sweeps the full seed
        family with the DirectCompute constraint mask and certifies every
        survivor with the forward generator. The expected cursor and candidate
        below come from the shipped solver itself
        (`deliverables/m23d-preimage/scripts/probe_forward_filter_route.py`), so
        this gate pins the route even if both workers drift together.
        """

        query = base_query(
            rarity=3,
            primary_effect_ids=[PARTIAL_FILTER_PRIMARY],
            required_secondary_ids=[PARTIAL_FILTER_SECONDARY],
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)
            params = search_params(
                query,
                rust_context,
                result_count=1,
                page_trials=10_000_000,
                job_trials=10_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust partial")["job_id"],
                timeout=900,
            )
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python partial",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            [PARTIAL_FILTER_SEED],
            "the partial-effect route must publish the probed Seed",
        )
        self.assertEqual(candidate_seeds(rust_snapshot), candidate_seeds(python_snapshot))
        self.assertEqual(candidate_cursors(rust_snapshot), candidate_cursors(python_snapshot))
        self.assertEqual(
            rust_snapshot["cursor"],
            PARTIAL_FILTER_CURSOR,
            "the partial-effect cursor must be the accepted candidate's pivot trial",
        )
        self.assertEqual(rust_snapshot["cursor"], python_snapshot["cursor"])
        self.assertEqual(rust_snapshot["stop_reason"], python_snapshot["stop_reason"])

    def test_partial_effect_secondary_only_query_matches_the_python_worker(self) -> None:
        """A partial request with no primary still runs the forward filter.

        The criterion groups of a primary-less request use the shipped
        `ordinary_kind = 2`, so this pins that packing separately from the
        primary-bearing case above.
        """

        query = base_query(
            rarity=3,
            primary_effect_ids=[],
            required_secondary_ids=[12028, 16437],
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=1,
                page_trials=10_000_000,
                job_trials=10_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust partial secondary")["job_id"],
                timeout=900,
            )
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python partial secondary",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            [232216827],
            "the secondary-only route must publish the probed Seed",
        )
        self.assertEqual(candidate_seeds(rust_snapshot), candidate_seeds(python_snapshot))
        self.assertEqual(candidate_cursors(rust_snapshot), candidate_cursors(python_snapshot))
        self.assertEqual(rust_snapshot["cursor"], python_snapshot["cursor"])
        self.assertEqual(rust_snapshot["stop_reason"], python_snapshot["stop_reason"])

    def test_grace_filtered_partial_rarity5_matches_the_python_worker(self) -> None:
        """A rarity-5 partial request with a selected Grace inverts that Grace.

        The shipped solver's pivot is the Grace's draw-1 runs, not the whole seed
        family, so this pins the cursor space itself: the expected Seeds and
        cursors come from the shipped solver
        (`deliverables/m23d-preimage/scripts/probe_forward_filter_route.py`), and
        a natural-family substitute would land on a different trial.
        """

        cases = (
            {
                "name": "primary_and_secondaries",
                "query": base_query(
                    rarity=5,
                    primary_effect_ids=[RARITY5_PRIMARY],
                    required_secondary_ids=[6410, 12028],
                    grace_effect_id=RARITY5_GRACE_ID,
                ),
                "expected_seed": 88364494,
                "expected_cursor": 393530,
            },
            {
                "name": "secondaries_only",
                "query": base_query(
                    rarity=5,
                    primary_effect_ids=[],
                    required_secondary_ids=[6410, 12028],
                    grace_effect_id=RARITY5_GRACE_ID,
                ),
                "expected_seed": 163797243,
                "expected_cursor": 1695,
            },
            {
                "name": "grace_only",
                "query": base_query(
                    rarity=5,
                    primary_effect_ids=[],
                    required_secondary_ids=[],
                    grace_effect_id=RARITY5_GRACE_ID,
                ),
                "expected_seed": 162486523,
                "expected_cursor": 15,
            },
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)
            for case in cases:
                with self.subTest(case=case["name"]):
                    params = search_params(
                        case["query"],
                        rust_context,
                        result_count=1,
                        page_trials=4_000_000,
                        job_trials=4_000_000,
                        **self.policy(),
                    )
                    rust_snapshot = wait_for_terminal(
                        rust,
                        rust.result(
                            "search.start", params, label=f"rust {case['name']}"
                        )["job_id"],
                        timeout=900,
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python {case['name']}",
                        )["job_id"],
                        timeout=900,
                    )
                    self.assertEqual(
                        rust_snapshot["state"], "completed", rust_snapshot.get("error")
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        [case["expected_seed"]],
                        f"{case['name']}: expected the probed Grace-context Seed",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        candidate_seeds(python_snapshot),
                        f"{case['name']}: candidate identity differs from Python",
                    )
                    self.assertEqual(
                        candidate_cursors(rust_snapshot),
                        candidate_cursors(python_snapshot),
                        f"{case['name']}: candidate cursor differs from Python",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"],
                        case["expected_cursor"],
                        f"{case['name']}: the Grace draw-1 pivot cursor must match",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"], python_snapshot["cursor"]
                    )
                    self.assertEqual(
                        rust_snapshot["stop_reason"], python_snapshot["stop_reason"]
                    )
        finally:
            rust.close()
            python.close()

    def test_grace_filtered_partial_rarity4_matches_the_python_worker(self) -> None:
        """The rarity-4 finalizer route inverts the same draw-1 Grace runs."""

        grace = 25939
        cases = (
            {
                "name": "primary_and_secondary",
                "query": base_query(
                    rarity=4,
                    primary_effect_ids=[PRIMARY_ROUTE_EFFECT],
                    required_secondary_ids=[12028],
                    grace_effect_id=grace,
                ),
                "expected_seed": 83888569,
                "expected_cursor": 33956,
            },
            {
                "name": "secondary_only",
                "query": base_query(
                    rarity=4,
                    primary_effect_ids=[],
                    required_secondary_ids=[12028],
                    grace_effect_id=grace,
                ),
                "expected_seed": 116283643,
                "expected_cursor": 688,
            },
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)
            for case in cases:
                with self.subTest(case=case["name"]):
                    params = search_params(
                        case["query"],
                        rust_context,
                        result_count=1,
                        page_trials=4_000_000,
                        job_trials=4_000_000,
                        **self.policy(),
                    )
                    rust_snapshot = wait_for_terminal(
                        rust,
                        rust.result(
                            "search.start", params, label=f"rust {case['name']}"
                        )["job_id"],
                        timeout=900,
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python {case['name']}",
                        )["job_id"],
                        timeout=900,
                    )
                    self.assertEqual(
                        rust_snapshot["state"], "completed", rust_snapshot.get("error")
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        [case["expected_seed"]],
                        f"{case['name']}: expected the probed Grace-context Seed",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        candidate_seeds(python_snapshot),
                        f"{case['name']}: candidate identity differs from Python",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"],
                        case["expected_cursor"],
                        f"{case['name']}: the Grace draw-1 pivot cursor must match",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"], python_snapshot["cursor"]
                    )
                    self.assertEqual(
                        rust_snapshot["stop_reason"], python_snapshot["stop_reason"]
                    )
        finally:
            rust.close()
            python.close()

    def test_partial_effect_cancel_and_resume_continue_without_replay(self) -> None:
        """A cancelled partial-effect page resumes exactly where it stopped.

        The forward-filter route scans the whole seed family, so a cancel lands
        inside a long page rather than after a short one. The union of the
        cancelled page and the resumed job must equal the shipped worker's
        single continuing run, which is only possible if the checkpoint really
        is the accepted-match cursor and nothing is replayed.
        """

        query = base_query(
            rarity=3,
            primary_effect_ids=[PARTIAL_FILTER_PRIMARY],
            required_secondary_ids=[PARTIAL_FILTER_SECONDARY],
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                context,
                result_count=25,
                page_trials=100_000_000,
                job_trials=400_000_000,
                continue_until_complete=True,
                **self.policy(),
            )
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python continuing partial",
                )["job_id"],
                timeout=900,
            )
            started = rust.result("search.start", params, label="rust partial start")
            time.sleep(0.25)
            inflight = rust.result(
                "job.snapshot", {"job_id": started["job_id"]}, label="job.snapshot"
            )
            self.assertNotIn(
                inflight["state"],
                TERMINAL_STATES,
                "the partial page finished before the cancel landed, so this check "
                "measured nothing",
            )
            cancel_started = time.monotonic()
            rust.result("job.cancel", {"job_id": started["job_id"]}, label="job.cancel")
            cancelled = wait_for_terminal(rust, started["job_id"], timeout=60)
            cancel_seconds = time.monotonic() - cancel_started
            self.assertEqual(cancelled["state"], "cancelled", cancelled)
            self.assertLess(
                cancel_seconds,
                2.0,
                "a cancel inside a 100M-trial partial page must be responsive",
            )
            token = cancelled.get("resume_token")
            self.assertIsNotNone(
                token, "a cancelled partial job must publish its checkpoint token"
            )
            resumed = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    result_count=25,
                    page_trials=100_000_000,
                    job_trials=400_000_000,
                    continue_until_complete=True,
                    resume_token=token,
                    **self.policy(),
                ),
                label="resume search.start",
            )
            self.assertGreaterEqual(
                resumed["cursor"],
                cancelled["cursor"],
                "a resume must not rewind the partial-effect cursor",
            )
            resumed_snapshot = wait_for_terminal(rust, resumed["job_id"], timeout=900)
            union_seeds = candidate_seeds(cancelled) + candidate_seeds(resumed_snapshot)
            self.assertEqual(
                len(union_seeds),
                len(set(union_seeds)),
                "a resumed partial page must not replay candidates",
            )
            python_seeds = candidate_seeds(python_snapshot)
            self.assertTrue(union_seeds, "the cancelled run published nothing to check")
            # A resumed job carries its own `job_trials` budget from the
            # checkpoint, so the two runs need not cover the same number of
            # trials; what must hold is that both enumerate the same accepted
            # candidates in the same order as far as both went, with nothing
            # replayed and nothing invented.
            overlap = min(len(union_seeds), len(python_seeds))
            self.assertEqual(
                union_seeds[:overlap],
                python_seeds[:overlap],
                "cancel plus resume must enumerate the shipped run's candidates in "
                f"the same order (rust {len(union_seeds)}, shipped {len(python_seeds)})",
            )
            if resumed_snapshot["candidates"]:
                self.assertGreater(
                    candidate_cursors(resumed_snapshot)[0],
                    cancelled["cursor"],
                    "the resumed page must continue after the checkpoint",
                )
            print(
                f"# partial cancel: {cancel_seconds * 1000:.0f} ms at cursor "
                f"{cancelled['cursor']} ({len(cancelled['candidates'])} published), "
                f"resumed {len(resumed_snapshot['candidates'])} candidates to cursor "
                f"{resumed_snapshot['cursor']} ({resumed_snapshot['stop_reason']}), "
                f"shipped run {len(python_seeds)} to cursor {python_snapshot['cursor']} "
                f"({python_snapshot['stop_reason']})"
            )
        finally:
            rust.close()
            python.close()

    def test_complete_preimage_route_matches_the_python_worker(self) -> None:
        """The complete-composition preimage route is a real GPU route now.

        A rarity-3 request that names every ordinary slot is inverted through the
        shipped effect-preimage accelerator on both sides, so this gate compares
        the composed candidates and their per-candidate cursors against the
        shipped Python worker.

        The window is wide enough to reach the shipped page boundary
        (`max(64, 8 * pending)` verified matches), so the assertion covers the
        scan-boundary cursor as well as the candidate list: the Rust collector
        mirrors the wider scan and publishes only the requested candidates.
        """

        query = complete_preimage_query()
        has_directcompute = directcompute_available()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=2,
                page_trials=100_000_000,
                job_trials=100_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust complete preimage")[
                    "job_id"
                ],
                timeout=900,
            )
            if not has_directcompute:
                # Without a DirectCompute device this route must fail closed by
                # name instead of quietly answering from a different scanner.
                self.assertEqual(rust_snapshot["state"], "failed", rust_snapshot)
                self.assertEqual(
                    rust_snapshot["error"]["code"],
                    "SEARCH_BACKEND_UNAVAILABLE",
                    rust_snapshot["error"],
                )
                self.assertIn(
                    "effect-preimage",
                    rust_snapshot["error"]["message"].lower(),
                    "the refusal must name the helper it could not use",
                )
                return
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python complete preimage",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertTrue(
            rust_snapshot["candidates"],
            "the complete-composition preimage route found nothing",
        )
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "complete-preimage candidate identity or order differs from the Python worker",
        )
        self.assertEqual(
            candidate_cursors(rust_snapshot),
            candidate_cursors(python_snapshot),
            "complete-preimage candidate trials differ from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            "complete-preimage page cursor differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["stop_reason"],
            python_snapshot["stop_reason"],
            "complete-preimage stop reason differs from the Python worker",
        )

    def test_complete_rarity5_preimage_matches_the_python_worker(self) -> None:
        """A rarity-5 complete composition runs the same GPU inverse with Grace.

        The draw-1 Grace preimage is the route's shared constraint, so this also
        covers the Grace integration: candidate identity, per-candidate cursor and
        the final page cursor must match the shipped Python worker exactly.
        """

        query = complete_rarity5_query()
        has_directcompute = directcompute_available()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=2,
                page_trials=100_000_000,
                job_trials=100_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust rarity5 preimage")[
                    "job_id"
                ],
                timeout=900,
            )
            if not has_directcompute:
                self.assertEqual(rust_snapshot["state"], "failed", rust_snapshot)
                self.assertIn(
                    "effect-preimage",
                    rust_snapshot["error"]["message"].lower(),
                    "the refusal must name the helper it could not use",
                )
                return
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python rarity5 preimage",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertTrue(
            rust_snapshot["candidates"],
            "the rarity-5 complete-composition route found nothing",
        )
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "rarity-5 candidate identity or order differs from the Python worker",
        )
        self.assertEqual(
            candidate_cursors(rust_snapshot),
            candidate_cursors(python_snapshot),
            "rarity-5 candidate trials differ from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            "rarity-5 page cursor differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["stop_reason"],
            python_snapshot["stop_reason"],
            "rarity-5 stop reason differs from the Python worker",
        )
        for candidate in rust_snapshot["candidates"]:
            slots = {
                int(effect["slot"]): int(effect["effect_id"])
                for effect in candidate["effects"]
            }
            self.assertEqual(
                slots.get(6),
                RARITY5_GRACE_ID,
                "every candidate must terminate in the requested Grace",
            )

    def test_one_wildcard_rarity5_route_matches_the_python_worker(self) -> None:
        """The one-wildcard route must serve the same Seeds as the shipped worker.

        The requested ordinary effects have to be present and one ordinary slot is
        free, so this also proves the containment acceptance agrees with
        `verify_one_wildcard_matches` rather than with a stricter exact-set match.
        """

        query = one_wildcard_rarity5_query()
        has_directcompute = directcompute_available()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=2,
                page_trials=100_000_000,
                job_trials=100_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust one wildcard")[
                    "job_id"
                ],
                timeout=900,
            )
            if not has_directcompute:
                self.assertEqual(rust_snapshot["state"], "failed", rust_snapshot)
                self.assertIn(
                    "effect-preimage",
                    rust_snapshot["error"]["message"].lower(),
                )
                return
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python one wildcard",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertTrue(
            rust_snapshot["candidates"],
            "the one-wildcard route found nothing",
        )
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "one-wildcard candidate identity or order differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            "one-wildcard page cursor differs from the Python worker",
        )
        self.assertEqual(
            rust_snapshot["stop_reason"],
            python_snapshot["stop_reason"],
            "one-wildcard stop reason differs from the Python worker",
        )
        for candidate in rust_snapshot["candidates"]:
            slots = {
                int(effect["slot"]): int(effect["effect_id"])
                for effect in candidate["effects"]
            }
            ordinary = {slots.get(slot) for slot in range(1, 6)}
            for effect_id in RARITY5_SECONDARY_IDS:
                self.assertIn(
                    effect_id,
                    ordinary,
                    "every returned candidate must contain the required ordinary effects",
                )
            self.assertEqual(
                slots.get(6),
                RARITY5_GRACE_ID,
                "every returned candidate must terminate in the requested Grace",
            )
            self.assertEqual(
                len(ordinary - set(RARITY5_SECONDARY_IDS)),
                1,
                "exactly one ordinary slot is free in a one-wildcard search",
            )


    def test_complete_rarity5_deep_slot_only_set_is_refused_by_both_workers(self) -> None:
        """A structurally impossible rarity-5 set must fail closed, not answer."""

        query = base_query(
            rarity=5,
            primary_effect_ids=[RARITY5_DEEP_ONLY_PRIMARY],
            required_secondary_ids=list(RARITY5_DEEP_ONLY_SECONDARY_IDS),
            grace_effect_id=RARITY5_GRACE_ID,
        )
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                rust_context,
                result_count=2,
                page_trials=100_000_000,
                job_trials=100_000_000,
                **self.policy(),
            )
            reply = rust.call("search.start", params)
            self.assertFalse(reply.get("ok"), "the worker must refuse this combination")
            self.assertEqual(
                reply["error"]["code"],
                "INVALID_REQUEST",
                reply["error"],
            )
            self.assertIn(
                "deep slot",
                reply["error"]["message"].lower(),
                "the refusal must name the reason it cannot be searched",
            )
            python_reply = python.call(
                "search.start", {**params, "context_digest": python_context}
            )
            self.assertFalse(
                python_reply.get("ok"),
                "the shipped worker refuses this combination too, so a search answer "
                "here would be a parity break",
            )
            self.assertEqual(python_reply["error"]["code"], "INVALID_REQUEST")
        finally:
            rust.close()
            python.close()


    def test_complete_preimage_roll_and_occurrence_filters_match_the_python_worker(self) -> None:
        """Roll minimums and effect occurrences stay enforced on the GPU route.

        Plain roll minimums are the one post-acceptance criterion the job layer
        does not re-check, so the route decides them from its own certified
        composition; occurrences are decided by the job layer. Both must agree
        with the shipped Python worker, including when the filter removes a seed
        the unfiltered run returned.
        """

        query = complete_preimage_query()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            baseline = search_params(
                query, rust_context, result_count=3, page_trials=100_000_000, job_trials=100_000_000,
                **self.policy(),
            )
            baseline_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", baseline, label="preimage baseline")["job_id"],
                timeout=900,
            )
            self.assertTrue(baseline_snapshot["candidates"], "baseline found no candidate")
            seed = candidate_seeds(baseline_snapshot)[0]
            roll = composed_roll_percent(query, seed)
            cases = {
                "roll_at_observed_minimum": [[COMPLETE_PREIMAGE_PRIMARY, roll]],
                "roll_above_observed": [
                    [COMPLETE_PREIMAGE_PRIMARY, min(roll + 5, 100)]
                ],
            }
            for name, minimum_rolls in cases.items():
                filtered = base_query(
                    rarity=3,
                    primary_effect_ids=[COMPLETE_PREIMAGE_PRIMARY],
                    required_secondary_ids=list(COMPLETE_PREIMAGE_SECONDARY_IDS),
                    minimum_roll_percent_by_effect_id=minimum_rolls,
                )
                params = search_params(
                    filtered, rust_context, result_count=3, page_trials=100_000_000,
                    job_trials=100_000_000, **self.policy(),
                )
                rust_snapshot = wait_for_terminal(
                    rust,
                    rust.result("search.start", params, label=f"{name} rust")["job_id"],
                    timeout=900,
                )
                python_snapshot = wait_for_terminal(
                    python,
                    python.result(
                        "search.start",
                        {**params, "context_digest": python_context},
                        label=f"{name} python",
                    )["job_id"],
                    timeout=900,
                )
                self.assertEqual(
                    rust_snapshot["state"],
                    "completed",
                    f"{name}: {rust_snapshot.get('error')}",
                )
                self.assertEqual(
                    candidate_seeds(rust_snapshot),
                    candidate_seeds(python_snapshot),
                    f"{name}: candidate identity or order differs from the Python worker",
                )
                self.assertEqual(
                    rust_snapshot["cursor"],
                    python_snapshot["cursor"],
                    f"{name}: cursor differs from the Python worker",
                )
                if name == "roll_at_observed_minimum":
                    self.assertIn(
                        seed,
                        candidate_seeds(rust_snapshot),
                        f"{name}: the unfiltered seed must survive its own roll minimum",
                    )
                else:
                    self.assertNotIn(
                        seed,
                        candidate_seeds(rust_snapshot),
                        f"{name}: a roll minimum above the observed roll must filter that seed",
                    )
        finally:
            rust.close()
            python.close()


    def test_complete_preimage_cancel_and_resume_do_not_replay(self) -> None:
        """The preimage route must cancel responsively and resume exactly.

        The first job is cancelled while a 200M-trial page is in flight. The
        checkpoint it publishes must be usable without replaying a candidate and
        without rewinding the cursor, which is the property the shipped page's
        scan-boundary cursor exists to guarantee.
        """

        query = complete_preimage_query()
        rust = self.rust_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            started = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    result_count=100,
                    page_trials=100_000_000,
                    job_trials=100_000_000,
                    continue_until_complete=True,
                    **self.policy(),
                ),
                label="preimage cancel start",
            )
            time.sleep(0.25)
            inflight = rust.result(
                "job.snapshot", {"job_id": started["job_id"]}, label="job.snapshot"
            )
            self.assertNotIn(
                inflight["state"],
                TERMINAL_STATES,
                "the preimage job finished before the cancel could be issued, so this "
                "check measured nothing; raise result_count so a page stays in flight",
            )
            rust.result("job.cancel", {"job_id": started["job_id"]}, label="preimage cancel")
            cancelled = wait_for_terminal(rust, started["job_id"], timeout=900)
            self.assertEqual(cancelled["state"], "cancelled", cancelled.get("error"))
            self.assertEqual(cancelled["stop_reason"], "cancelled")
            token = cancelled.get("resume_token")
            self.assertIsNotNone(
                token, "a cancelled preimage job must publish a resume token"
            )
            first_seeds = set(candidate_seeds(cancelled))
            resumed = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    result_count=100,
                    page_trials=100_000_000,
                    job_trials=100_000_000,
                    continue_until_complete=True,
                    resume_token=token,
                    **self.policy(),
                ),
                label="preimage resume start",
            )
            self.assertGreaterEqual(
                resumed["cursor"],
                cancelled["cursor"],
                "a resume must not rewind the cursor",
            )
            resumed_snapshot = wait_for_terminal(rust, resumed["job_id"], timeout=900)
        finally:
            rust.close()
        self.assertEqual(resumed_snapshot["state"], "completed", resumed_snapshot.get("error"))
        self.assertTrue(resumed_snapshot["candidates"], "the resumed preimage job found nothing")
        for candidate in resumed_snapshot["candidates"]:
            self.assertGreater(
                int(candidate["cursor"]),
                int(cancelled["cursor"]),
                "a resumed preimage job must not return a trial at or before the checkpoint",
            )
            self.assertNotIn(
                int(candidate["seed"]),
                first_seeds,
                "a resumed preimage job must not replay a published candidate",
            )

    def test_effect_routes_match_or_refuse_like_the_python_worker(self) -> None:
        """Effect routes either answer with the Python worker's result or refuse.

        The shipped full-family replay serves a rarity-3 primary search and an
        unconstrained query (batched primary ids, then the auxiliary masks), and
        the partial-effect forward filter serves a request that names only some
        ordinary slots at rarity 3, 4 and 5. Every served case carries the
        shipped solver's own cursor and Seed
        (`deliverables/m23d-preimage/scripts/probe_forward_filter_route.py`), so
        the gate pins the route even if both workers drift together. Routes that
        remain unported still reject with INVALID_REQUEST and their own reason.
        """

        served = {
            "unconstrained": {
                "query": base_query(),
            },
            "r3_primary": {
                "query": base_query(
                    rarity=3, primary_effect_ids=[PRIMARY_ROUTE_EFFECT]
                ),
            },
            "r4_primary_plus_secondary": {
                "query": base_query(
                    primary_effect_ids=[PRIMARY_ROUTE_EFFECT],
                    required_secondary_ids=[12028],
                ),
                "result_count": 1,
                "expected_seed": 74209531,
                "expected_cursor": 1110,
            },
            "r5_primary": {
                "query": base_query(
                    rarity=5, primary_effect_ids=[PRIMARY_ROUTE_EFFECT]
                ),
                "result_count": 1,
                "expected_seed": 8411387,
                "expected_cursor": 626,
            },
        }
        refused = {
            # The shared structural preflight refuses an effect id outside the
            # native table before any route is chosen, exactly like the shipped
            # worker, so this is a query-level refusal rather than a route gap.
            "unknown_effect_id": {
                "query": base_query(
                    primary_effect_ids=[PRIMARY_ROUTE_EFFECT],
                    required_secondary_ids=[0x1234],
                ),
                "reason": "native parameter table",
                "python": True,
            },
        }
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            for name, case in served.items():
                with self.subTest(route=name):
                    params = search_params(
                        case["query"],
                        context,
                        result_count=case.get("result_count", 2),
                        page_trials=2_000_000,
                        job_trials=2_000_000,
                        **self.policy(),
                    )
                    rust_snapshot = wait_for_terminal(
                        rust,
                        rust.result("search.start", params, label=f"rust {name}")["job_id"],
                        timeout=600,
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python {name}",
                        )["job_id"],
                        timeout=600,
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        candidate_seeds(python_snapshot),
                        f"{name}: candidate identity or order differs from Python",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"],
                        python_snapshot["cursor"],
                        f"{name}: cursor differs from the Python worker",
                    )
                    self.assertEqual(
                        rust_snapshot["stop_reason"],
                        python_snapshot["stop_reason"],
                        f"{name}: stop reason differs from the Python worker",
                    )
                    if "expected_seed" in case:
                        self.assertEqual(
                            candidate_seeds(rust_snapshot),
                            [case["expected_seed"]],
                            f"{name}: the served route must publish the probed Seed",
                        )
                        self.assertEqual(
                            rust_snapshot["cursor"],
                            case["expected_cursor"],
                            f"{name}: the served cursor must be the probed cursor",
                        )
            for name, case in refused.items():
                with self.subTest(route=name):
                    reply = rust.call(
                        "search.start",
                        search_params(
                            case["query"],
                            context,
                            page_trials=1_000_000,
                            job_trials=1_000_000,
                        )
                    )
                    self.assertFalse(
                        reply.get("ok"),
                        f"{name} must be refused, not silently searched: {reply}",
                    )
                    self.assertEqual(reply["error"]["code"], "INVALID_REQUEST")
                    self.assertIn(
                        case["reason"],
                        reply["error"]["message"],
                        f"{name} must name the missing route it actually needs "
                        f"(expected {case['reason']!r}): {reply['error']['message']}",
                    )
                    if case.get("python"):
                        # A structural refusal is not a Rust-only gap: the shipped
                        # worker refuses the same query, so both codes must match
                        # and neither worker may answer it.
                        python_reply = python.call(
                            "search.start",
                            search_params(
                                case["query"],
                                python_context,
                                page_trials=1_000_000,
                                job_trials=1_000_000,
                            ),
                        )
                        self.assertFalse(python_reply.get("ok"), python_reply)
                        self.assertEqual(
                            python_reply["error"]["code"],
                            reply["error"]["code"],
                            f"{name}: the shipped worker refuses this query with a "
                            "different code",
                        )
        finally:
            rust.close()
            python.close()

    def test_r3_and_r5_auxiliary_only_routes_match_the_python_worker(self) -> None:
        """Auxiliary-only pages are served at every certified rarity.

        The fused auxiliary pivot is selected purely from non-empty auxiliary
        criteria with no effect constraint; neither the compiler nor the job
        layer gates that route on rarity, and the shipped preview composes
        certified rarity-3/4/5 sequences. So a rarity-3 or rarity-5 auxiliary
        page is part of the advertised surface and must be evidenced by a real
        subprocess run, not assumed from the refusal of the effect routes.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            for rarity in (3, 5):
                with self.subTest(rarity=rarity):
                    query = base_query(rarity=rarity, auxiliary=supported_auxiliary())
                    params = search_params(
                        query,
                        rust_context,
                        result_count=5,
                        page_trials=2_000_000,
                        job_trials=2_000_000,
                        **self.policy(),
                    )
                    reply = rust.call("search.start", params)
                    self.assertTrue(
                        reply.get("ok"),
                        f"rarity {rarity} auxiliary-only is part of the advertised "
                        f"surface and must not be refused: {reply.get('error')}",
                    )
                    rust_snapshot = wait_for_terminal(
                        rust, reply["result"]["job_id"], timeout=600
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python rarity {rarity} auxiliary",
                        )["job_id"],
                        timeout=600,
                    )
                    self.assertEqual(
                        rust_snapshot["state"],
                        python_snapshot["state"],
                        f"rarity {rarity}: state differs from the Python worker",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        candidate_seeds(python_snapshot),
                        f"rarity {rarity}: auxiliary-only candidate identity or order "
                        "differs from the Python worker",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"],
                        python_snapshot["cursor"],
                        f"rarity {rarity}: auxiliary-only cursor differs from the "
                        "Python worker",
                    )
        finally:
            rust.close()
            python.close()

    def test_nonzero_descriptor_selector_candidate_still_composes(self) -> None:
        """A candidate with a nonzero descriptor selector must not be dropped.

        Regression for the bounded selector fix: the auxiliary half still
        composes through the selector-aware roster stage and the enemy-state
        half reports the shipped fallback, so the candidate is returned and
        matches the Python worker field for field.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                supported_query(),
                rust_context,
                result_count=100,
                page_trials=2_000_000,
                job_trials=2_000_000,
                **self.policy(),
            )
            rust_snapshot = wait_for_terminal(
                rust,
                rust.result("search.start", params, label="rust selector page")["job_id"],
                timeout=900,
            )
            python_snapshot = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    {**params, "context_digest": python_context},
                    label="python selector page",
                )["job_id"],
                timeout=900,
            )
        finally:
            rust.close()
            python.close()
        self.assertEqual(rust_snapshot["state"], "completed", rust_snapshot.get("error"))
        self.assertEqual(
            candidate_seeds(rust_snapshot),
            candidate_seeds(python_snapshot),
            "the selector candidate page differs from the Python worker",
        )
        fallbacks = [
            candidate
            for candidate in rust_snapshot["candidates"]
            if not ((candidate.get("enemy_states") or {}).get("solo") or {}).get(
                "possessed_complete", True
            )
        ]
        self.assertTrue(
            fallbacks,
            "the page contains no nonzero-selector candidate, so the fallback "
            "branch was not exercised; pick a result_count that reaches one",
        )
        for candidate in fallbacks:
            python_match = [
                other
                for other in python_snapshot["candidates"]
                if other["seed"] == candidate["seed"]
            ]
            self.assertTrue(python_match, f"seed {candidate['seed']} missing from Python")
            self.assertEqual(
                candidate["auxiliary"],
                python_match[0]["auxiliary"],
                f"seed {candidate['seed']}: auxiliary half differs from Python",
            )
            self.assertEqual(
                candidate["enemy_states"],
                python_match[0]["enemy_states"],
                f"seed {candidate['seed']}: enemy-state fallback differs from Python",
            )
            self.assertIsNone(
                candidate["enemy_states"]["solo"]["terrain"],
                "the fallback must report a null terrain, not an invented one",
            )

    def test_supported_post_acceptance_filters_are_enforced(self) -> None:
        """Every supported post-acceptance filter must narrow real candidates.

        The native compiler names `enemy_occurrence_groups`, `effect_occurrences`
        and `initial_challenge_counts` as filters applied after materialization.
        Carrying the metadata without enforcing it would return false matches, so
        each case below is calibrated from the Python worker's unfiltered page,
        must return at least one candidate, must differ from that unfiltered page,
        must match the Python worker exactly, and every returned candidate must
        satisfy the filter when its own payload is inspected.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)
            policy = self.policy()
            base_params = search_params(
                supported_query(),
                rust_context,
                result_count=25,
                page_trials=2_000_000,
                job_trials=2_000_000,
                **policy,
            )
            base = rust.result("search.start", base_params, label="base search.start")
            base_snapshot = wait_for_terminal(rust, base["job_id"], timeout=600)
            base_candidates = base_snapshot["candidates"]
            self.assertTrue(
                base_candidates,
                "the unfiltered page is empty, so no filter case could be calibrated",
            )

            capacities = sorted(
                {
                    candidate["initial_challenge_capacity"]
                    for candidate in base_candidates
                    if candidate.get("initial_challenge_capacity") is not None
                }
            )
            effect_ids = sorted(
                {
                    effect["effect_id"]
                    for candidate in base_candidates
                    for effect in candidate["effects"]
                    if effect["effect_id"] not in (0, 0xFFFF_FFFF)
                }
            )
            lookup_keys = sorted(
                {
                    occurrence["lookup_key"]
                    for candidate in base_candidates
                    for variant in ("solo", "expedition")
                    for occurrence in (
                        (candidate.get("enemy_states") or {}).get(variant) or {}
                    ).get("occurrences", [])
                }
            )
            self.assertTrue(capacities, "no candidate reported a challenge capacity")
            self.assertTrue(effect_ids, "no candidate carried an ordinary effect")
            self.assertTrue(lookup_keys, "no candidate carried an enemy occurrence")

            chosen_capacity = capacities[0]
            chosen_effect = effect_ids[0]
            chosen_lookup = lookup_keys[0]
            base_seeds = set(candidate_seeds(base_snapshot))

            cases = (
                (
                    "initial_challenge_counts",
                    {"initial_challenge_counts": [chosen_capacity]},
                    lambda candidate: candidate["initial_challenge_capacity"]
                    == chosen_capacity,
                    "enforced",
                ),
                (
                    "effect_occurrences",
                    {
                        "effect_occurrences": [
                            {
                                "scope": "any",
                                "alternatives": [
                                    {
                                        "effect_id": chosen_effect,
                                        "minimum_roll_percent": 0,
                                    }
                                ],
                            }
                        ]
                    },
                    lambda candidate: any(
                        effect["effect_id"] == chosen_effect
                        for effect in candidate["effects"]
                    ),
                    # The occurrence filter rides on the partial-effect route,
                    # which composes nothing for it and lets the job layer decide
                    # it, exactly like the shipped solver.
                    "enforced",
                ),
                (
                    "enemy_occurrence_groups",
                    {
                        "enemy_occurrence_groups": [
                            [
                                {
                                    "lookup_keys": [chosen_lookup],
                                    "state": "any",
                                    "availability": "any",
                                }
                            ]
                        ]
                    },
                    lambda candidate: any(
                        occurrence["lookup_key"] == chosen_lookup
                        for variant in ("solo", "expedition")
                        for occurrence in (
                            (candidate.get("enemy_states") or {}).get(variant) or {}
                        ).get("occurrences", [])
                    ),
                    "enforced_or_fail_closed",
                ),
            )

            for name, extra, satisfied, mode in cases:
                with self.subTest(filter=name):
                    query = supported_query(**extra)
                    params = search_params(
                        query,
                        rust_context,
                        result_count=25,
                        page_trials=2_000_000,
                        job_trials=2_000_000,
                        **policy,
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python {name}",
                        )["job_id"],
                        timeout=600,
                    )
                    if mode == "rejected":
                        reply = rust.call("search.start", params)
                        self.assertFalse(
                            reply.get("ok"),
                            f"{name} must be refused as an unported route instead of "
                            f"returning candidates that ignore it: {reply}",
                        )
                        self.assertEqual(reply["error"]["code"], "INVALID_REQUEST")
                        self.assertIn(
                            "effect-preimage",
                            reply["error"]["message"],
                            "the refusal must name the missing route",
                        )
                        self.assertTrue(
                            python_snapshot["candidates"],
                            "the Python worker serves this route, so the refusal is a "
                            "documented M2.3b1 boundary rather than a shared rule",
                        )
                        continue
                    started = rust.result(
                        "search.start", params, label=f"{name} search.start"
                    )
                    snapshot = wait_for_terminal(rust, started["job_id"], timeout=600)
                    if mode == "enforced_or_fail_closed" and snapshot["state"] == "failed":
                        self.assertEqual(
                            (snapshot.get("error") or {}).get("code"),
                            "UNSUPPORTED_CONTEXT",
                            f"{name}: a failure must name the preview-composition limit",
                        )
                        for candidate in snapshot["candidates"]:
                            self.assertTrue(
                                satisfied(candidate),
                                f"{name}: a candidate published before the failure does "
                                "not satisfy the filter it was searched with",
                            )
                        print(
                            f"# filter {name}: accepted but failed closed with "
                            f"UNSUPPORTED_CONTEXT after {len(snapshot['candidates'])} "
                            "candidates"
                        )
                        continue
                    self.assertEqual(
                        snapshot["state"],
                        "completed",
                        f"{name}: {snapshot.get('error')}",
                    )
                    candidates = snapshot["candidates"]
                    self.assertTrue(
                        candidates,
                        f"{name}: the filter returned no candidate in the window, so "
                        "enforcement could not be observed; widen the window or pick "
                        "another calibration value",
                    )
                    for candidate in candidates:
                        self.assertTrue(
                            satisfied(candidate),
                            f"{name}: seed {candidate['seed']} was returned but does not "
                            "satisfy the filter it was searched with",
                        )
                    self.assertTrue(
                        set(candidate_seeds(snapshot)) - base_seeds,
                        f"{name}: the filtered page returned only candidates the "
                        "unfiltered page already had, so the filter looks like a no-op",
                    )
                    self.assertEqual(
                        candidate_seeds(snapshot),
                        candidate_seeds(python_snapshot),
                        f"{name}: candidate identity or order differs from the Python worker",
                    )
                    self.assertEqual(
                        snapshot["cursor"],
                        python_snapshot["cursor"],
                        f"{name}: cursor differs from the Python worker",
                    )
                    print(
                        f"# filter {name}: {len(candidates)} candidates, cursor "
                        f"{snapshot['cursor']} (unfiltered {base_snapshot['cursor']})"
                    )
        finally:
            rust.close()
            python.close()

    def test_cancel_is_responsive_and_resume_does_not_replay(self) -> None:
        # The canonical sparse query keeps one 100M-trial page busy for about
        # two seconds on this machine, so a cancel issued after 250 ms is
        # genuinely in flight (v0.7.5 measured 31 ms to cancel such a page).
        query = base_query(
            auxiliary={
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": list(REGRESSION_RULE_KEYS),
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            }
        )
        rust = self.rust_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            started = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    result_count=1,
                    page_trials=100_000_000,
                    job_trials=400_000_000,
                    continue_until_complete=True,
                    **self.policy(),
                ),
                label="search.start",
            )
            time.sleep(0.25)
            inflight = rust.result(
                "job.snapshot", {"job_id": started["job_id"]}, label="job.snapshot"
            )
            self.assertNotIn(
                inflight["state"],
                TERMINAL_STATES,
                "the job finished before the cancel could be issued, so this check "
                "measured nothing; raise job_trials so one page stays in flight",
            )
            self.assertLess(
                inflight["cursor"],
                REGRESSION_TRIAL,
                "the cancel must be issued before the known match is reached",
            )
            cancel_started = time.monotonic()
            rust.result("job.cancel", {"job_id": started["job_id"]}, label="job.cancel")
            cancelled = wait_for_terminal(rust, started["job_id"], timeout=30)
            cancel_seconds = time.monotonic() - cancel_started
            self.assertEqual(cancelled["state"], "cancelled", cancelled)
            self.assertLess(
                cancel_seconds,
                2.0,
                "a cancel inside a 100M-trial page must be responsive (v0.7.5 measured 31 ms)",
            )
            token = cancelled.get("resume_token")
            self.assertIsNotNone(
                token,
                "a cancelled job must publish the checkpoint token used for resume",
            )
            resumed = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    result_count=1,
                    page_trials=100_000_000,
                    job_trials=400_000_000,
                    continue_until_complete=True,
                    resume_token=token,
                    **self.policy(),
                ),
                label="resume search.start",
            )
            self.assertGreaterEqual(
                resumed["cursor"],
                cancelled["cursor"],
                "a resume must not rewind the cursor",
            )
            replay_window = wait_for_terminal(rust, resumed["job_id"], timeout=900)
            seeds = candidate_seeds(replay_window)
            self.assertEqual(
                len(seeds),
                len(set(seeds)),
                "a resume must not replay candidates already published",
            )
            self.assertEqual(
                seeds,
                [REGRESSION_SEED],
                "a resumed job must still reach the candidate the cancelled one was chasing",
            )
            print(f"# cancel: {cancel_seconds * 1000:.0f} ms at cursor {cancelled['cursor']}")
        finally:
            rust.close()

    def test_resume_tokens_reject_forgery_and_changed_policy(self) -> None:
        query = supported_query()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(context, python_context)
            mint = self.policy()
            bounded = rust.result(
                "search.start",
                search_params(
                    query, context, page_trials=100_000, job_trials=100_000, **mint
                ),
                label="token mint",
            )
            final = wait_for_terminal(rust, bounded["job_id"], timeout=120)
            token = final.get("resume_token")
            self.assertIsNotNone(token, "a bounded stop must publish a resume token")

            changed_policy = {"allow_cpu_fallback": not mint["allow_cpu_fallback"]}
            variants = {
                "forged": search_params(query, context, page_trials=100_000, job_trials=100_000, resume_token="m23b-forged.token", **mint),
                "changed_query": search_params(base_query(level=170), context, page_trials=100_000, job_trials=100_000, resume_token=token, **mint),
                "changed_context": search_params(query, "0" * 64, page_trials=100_000, job_trials=100_000, resume_token=token, **mint),
                "changed_policy": search_params(query, context, page_trials=100_000, job_trials=100_000, resume_token=token, **changed_policy),
                "changed_continuation": search_params(query, context, page_trials=100_000, job_trials=100_000, continue_until_complete=True, resume_token=token, **mint),
            }
            for name, params in variants.items():
                expected = python.error_code("search.start", {**params, "context_digest": python_context if name != "changed_context" else params["context_digest"]})
                actual = rust.error_code("search.start", params)
                self.assertIsNotNone(expected, f"{name}: the Python worker accepted a bad request")
                self.assertEqual(actual, expected, f"{name}: {actual} != python {expected}")
        finally:
            rust.close()
            python.close()

    def test_resume_tokens_do_not_cross_a_process_boundary(self) -> None:
        query = supported_query()
        first = self.rust_worker()
        second = self.rust_worker()
        try:
            context = self.handshake_context(first)["context_digest"]
            bounded = first.result(
                "search.start",
                search_params(
                    query,
                    context,
                    page_trials=100_000,
                    job_trials=100_000,
                    **self.policy(),
                ),
                label="token mint",
            )
            final = wait_for_terminal(first, bounded["job_id"], timeout=120)
            token = final.get("resume_token")
            self.assertIsNotNone(token)
            second_context = self.handshake_context(second)["context_digest"]
            code = second.error_code(
                "search.start",
                search_params(
                    query,
                    second_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    resume_token=token,
                    **self.policy(),
                ),
            )
            self.assertIsNotNone(
                code,
                "a token minted in another worker process must be rejected",
            )
        finally:
            first.close()
            second.close()

    def test_candidate_export_respects_job_ownership(self) -> None:
        query = supported_query()
        rust = self.rust_worker()
        python = self.python_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            params = search_params(
                query,
                context,
                result_count=5,
                page_trials=2_000_000,
                job_trials=2_000_000,
                **self.policy(),
            )
            started = rust.result("search.start", params, label="search.start")
            snapshot = wait_for_terminal(rust, started["job_id"], timeout=300)
            candidates = snapshot["candidates"]
            self.assertTrue(
                candidates,
                "the ownership check needs at least one candidate; raise job_trials",
            )
            candidate_id = candidates[0]["candidate_id"]
            exported = rust.result(
                "candidate.export",
                {"job_id": started["job_id"], "candidate_id": candidate_id},
                label="candidate.export",
            )
            self.assertEqual(exported["candidate_id"], candidate_id)
            wrong_job = rust.error_code(
                "candidate.export",
                {"job_id": "00000000-0000-0000-0000-000000000000", "candidate_id": candidate_id},
            )
            python_wrong_job = python.error_code(
                "candidate.export",
                {
                    "job_id": "00000000-0000-0000-0000-000000000000",
                    "candidate_id": candidate_id,
                },
            )
            self.assertIsNotNone(wrong_job, "a foreign job id must not export a candidate")
            self.assertEqual(wrong_job, python_wrong_job)
            foreign_candidate = rust.error_code(
                "candidate.export",
                {"job_id": started["job_id"], "candidate_id": "0" * 64},
            )
            self.assertIsNotNone(foreign_candidate, "an unknown candidate must not export")
        finally:
            rust.close()
            python.close()

    def test_search_fails_closed_without_the_accelerator(self) -> None:
        worker = FramedProcess(self.rust_without_accelerator_argv, name="rust worker (no accelerator)")
        python = self.python_worker()
        try:
            handshake = worker.result("handshake")
            capabilities = handshake["capabilities"]
            self.assertFalse(
                capabilities["cuda_pivot_and_auxiliary"],
                "an absent accelerator must not advertise CUDA acceleration",
            )
            self.assertIsNone(handshake["context"]["seed_accelerator_abi"])
            context = handshake["context"]["context_digest"]
            query = base_query()
            params = search_params(
                query, context, page_trials=1_000_000, job_trials=1_000_000
            )
            strict = self.terminal_failure_code(worker, "search.start", params)
            # Accepted deviation (recorded in the migration record): the shipped
            # Python worker loads its own packaged accelerator, so it cannot be
            # made backend-unavailable here. The Rust contract is that a missing
            # accelerator starts a job that fails closed with this exact code
            # instead of silently running bulk CPU, so the code is asserted
            # directly rather than compared field-for-field with Python.
            self.assertEqual(
                strict,
                "SEARCH_BACKEND_UNAVAILABLE",
                "an exact search must fail closed with the backend-unavailable code "
                "when the accelerator is absent instead of silently running the CPU "
                "path (a job that starts and then fails is the supported shape)",
            )
            python_context = self.handshake_context(python)["context_digest"]
            python_strict = self.terminal_failure_code(
                python,
                "search.start",
                search_params(
                    query,
                    python_context,
                    page_trials=1_000_000,
                    job_trials=1_000_000,
                ),
            )
            if python_strict is not None and strict is not None:
                self.assertEqual(strict, python_strict)
        finally:
            worker.close()
            python.close()

    def test_execution_policy_does_not_leak_into_the_next_job(self) -> None:
        """One job's CPU opt-in must not become the next job's default.

        The observable at the protocol boundary depends on the platform: where
        the shipped probe reports CUDA, both policies can run, so the stronger
        evidence is the native owner's RAII guard unit test and this check then
        only proves the following strict job still completes normally. Where
        CUDA is unavailable, a leaked opt-in would let the strict job run on
        bulk CPU, so the strict job must fail with the backend-unavailable code.
        """

        rust = self.rust_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            query = supported_query()
            opted_in = rust.result(
                "search.start",
                search_params(
                    query,
                    context,
                    page_trials=100_000,
                    job_trials=100_000,
                    allow_cpu_fallback=True,
                ),
                label="opted-in search.start",
            )
            opted_snapshot = wait_for_terminal(rust, opted_in["job_id"], timeout=300)
            self.assertIn(
                opted_snapshot["state"],
                ("completed", "failed"),
                "the opt-in job must reach a terminal state before the strict job",
            )
            strict_code = self.terminal_failure_code(
                rust,
                "search.start",
                search_params(
                    query,
                    context,
                    page_trials=100_000,
                    job_trials=100_000,
                    allow_cpu_fallback=False,
                ),
            )
            if type(self).cuda_available:
                self.assertIsNone(
                    strict_code,
                    "with CUDA available a strict job must run normally after an "
                    "opted-in job; the policy restoration itself is proven by the "
                    "native guard unit test",
                )
            else:
                self.assertEqual(
                    strict_code,
                    "SEARCH_BACKEND_UNAVAILABLE",
                    "a strict job must not inherit the previous job's CPU opt-in",
                )
        finally:
            rust.close()

    def terminal_failure_code(
        self, worker: FramedProcess, method: str, params: dict
    ) -> str | None:
        """Return a request-level or job-level failure code.

        The shipped worker answers either shape: a request that is rejected
        outright, or a job that is accepted and then fails closed with a typed
        error. Both are honest failures; only silence would not be.
        """

        reply = worker.call(method, params)
        if not reply.get("ok"):
            return reply["error"]["code"]
        snapshot = wait_for_terminal(worker, reply["result"]["job_id"], timeout=120)
        if snapshot["state"] == "failed":
            return (snapshot.get("error") or {}).get("code")
        return None

    # Deterministic synthetic draw-1 partitions. Every one is a labeled fixture,
    # not a capture: dense two-range, an interleaved four-range map, and a "late"
    # map whose composable range sits after the sparse one so the first pages
    # cannot fill from easy hits.
    SYNTHETIC_RANGES = {
        "two_range": [
            {"start": 0, "end": 32767, "grace_id": 5858},
            {"start": 32768, "end": 65535, "grace_id": 25939},
        ],
        "interleaved": [
            {"start": 0, "end": 8191, "grace_id": 25939},
            {"start": 8192, "end": 16383, "grace_id": 5858},
            {"start": 16384, "end": 49151, "grace_id": 25939},
            {"start": 49152, "end": 65535, "grace_id": 5858},
        ],
        "late": [
            {"start": 0, "end": 61439, "grace_id": 25939},
            {"start": 61440, "end": 65535, "grace_id": 5858},
        ],
    }

    @classmethod
    def synthetic_grace_cache(
        cls,
        playthrough: int,
        generation_digest: str,
        shape: str = "two_range",
    ) -> str:
        """A clearly synthetic but structurally valid NG4/NG5 rarity-5 map.

        No genuine NG4/NG5 capture exists in the tree, so this fixture is
        labeled as synthetic: a dense partition of the draw-1 buckets whose
        grace ids are real effect rows, registered against the worker's own
        generation-context digest. Both workers must accept it and agree on the
        route's whole output; live capture acceptance stays an explicit open
        gate.
        """

        payload = {
            "schema": "nioh3-grace-output-map-cache/v2",
            "game_version": "2.00.02",
            "generation_context_digest": generation_digest,
            "draw_index": 1,
            "record_type": "0xDD82" if playthrough == 4 else "0xD523",
            "rarity": 5,
            "playthrough": f"synthetic-ng{playthrough}-{shape}",
            "effect_slot": 6,
            "ranges": cls.SYNTHETIC_RANGES[shape],
        }
        return json.dumps(payload, separators=(",", ":"), sort_keys=True)

    def register_grace_cache(self, worker: FramedProcess, cache_json: str) -> str:
        return worker.result("cache.register", {"cache_json": cache_json})["cache_id"]

    @staticmethod
    def normalise_payload(value):
        """Drop only genuinely nondeterministic run metadata."""

        if isinstance(value, dict):
            return {
                key: SearchWorkerParityTests.normalise_payload(item)
                for key, item in value.items()
                if key not in ("elapsed_ms",)
            }
        if isinstance(value, list):
            return [SearchWorkerParityTests.normalise_payload(item) for item in value]
        return value

    def assert_cached_case_matches(
        self,
        rust: FramedProcess,
        python: FramedProcess,
        *,
        name: str,
        playthrough: int,
        query: dict,
        rust_context: str,
        python_context: str,
        shape: str = "two_range",
        result_count: int = 2,
        page_trials: int = 200_000,
        job_trials: int | None = None,
        continue_until_complete: bool = False,
        compare_exports: bool = True,
    ) -> dict:
        """Run one cached case on both workers and compare the whole output."""

        rust_cache = self.register_grace_cache(
            rust, self.synthetic_grace_cache(playthrough, rust_context, shape)
        )
        python_cache = self.register_grace_cache(
            python, self.synthetic_grace_cache(playthrough, python_context, shape)
        )
        self.assertEqual(
            rust_cache, python_cache, f"{name}: the registered map id must match"
        )
        params = search_params(
            query,
            rust_context,
            result_count=result_count,
            page_trials=page_trials,
            job_trials=job_trials or page_trials,
            continue_until_complete=continue_until_complete,
            cache_id=rust_cache,
            **self.policy(),
        )
        rust_snapshot = wait_for_terminal(
            rust,
            rust.result("search.start", params, label=f"rust {name}")["job_id"],
            timeout=900,
        )
        python_snapshot = wait_for_terminal(
            python,
            python.result(
                "search.start",
                {**params, "context_digest": python_context},
                label=f"python {name}",
            )["job_id"],
            timeout=900,
        )
        self.assertEqual(
            rust_snapshot["state"], "completed", f"{name}: {rust_snapshot.get('error')}"
        )
        self.assertEqual(
            self.normalise_payload(rust_snapshot["candidates"]),
            self.normalise_payload(python_snapshot["candidates"]),
            f"{name}: the whole candidate payload must match the shipped worker",
        )
        self.assertEqual(
            rust_snapshot["cursor"],
            python_snapshot["cursor"],
            f"{name}: page cursor differs from Python",
        )
        self.assertEqual(
            rust_snapshot["stop_reason"],
            python_snapshot["stop_reason"],
            f"{name}: stop reason differs from Python",
        )
        if compare_exports:
            for candidate in rust_snapshot["candidates"]:
                rust_export = rust.result(
                    "candidate.export",
                    {
                        "job_id": rust_snapshot["job_id"],
                        "candidate_id": candidate["candidate_id"],
                    },
                    label=f"rust {name} export",
                )
                python_export = python.result(
                    "candidate.export",
                    {
                        "job_id": python_snapshot["job_id"],
                        "candidate_id": candidate["candidate_id"],
                    },
                    label=f"python {name} export",
                )
                self.assertEqual(
                    self.normalise_payload(rust_export),
                    self.normalise_payload(python_export),
                    f"{name}: the private export payload must match the shipped worker",
                )
        return rust_snapshot

    def test_cached_ng4_ng5_rarity5_route_matches_the_python_worker(self) -> None:
        """The save-bound NG4/NG5 route now serves the registered map.

        Both workers register the same synthetic valid map, must agree on its
        content-addressed id, and must return the same ordered candidates, page
        cursor and stop reason for a rarity-5 request through that map. The
        fixture is synthesized (no genuine 0xDD82/0xD523 capture exists), so this
        proves the ported route and its context binding, not a live save.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_handshake = rust.result("handshake")
            python_handshake = python.result("handshake")
            rust_context = rust_handshake["context"]["context_digest"]
            python_context = python_handshake["context"]["context_digest"]
            self.assertEqual(rust_context, python_context)
            self.assertEqual(
                rust_handshake["capabilities"].get("cached_rarity5_playthroughs"),
                [4, 5],
                "the ported save-bound route must advertise the playthroughs it serves",
            )
            self.assertEqual(
                rust_handshake["capabilities"].get("cached_rarity5_playthroughs"),
                python_handshake["capabilities"].get("cached_rarity5_playthroughs"),
            )

            for playthrough in (4, 5):
                with self.subTest(playthrough=playthrough):
                    rust_cache = self.register_grace_cache(
                        rust, self.synthetic_grace_cache(playthrough, rust_context)
                    )
                    python_cache = self.register_grace_cache(
                        python, self.synthetic_grace_cache(playthrough, python_context)
                    )
                    self.assertEqual(
                        rust_cache,
                        python_cache,
                        "the registered map id must be the same content hash",
                    )
                    query = base_query(
                        playthrough=playthrough,
                        rarity=5,
                        grace_effect_id=5858,
                    )
                    params = search_params(
                        query,
                        rust_context,
                        result_count=1,
                        page_trials=200_000,
                        job_trials=200_000,
                        cache_id=rust_cache,
                        **self.policy(),
                    )
                    rust_snapshot = wait_for_terminal(
                        rust,
                        rust.result(
                            "search.start", params, label=f"rust ng{playthrough} cached"
                        )["job_id"],
                        timeout=600,
                    )
                    python_snapshot = wait_for_terminal(
                        python,
                        python.result(
                            "search.start",
                            {**params, "context_digest": python_context},
                            label=f"python ng{playthrough} cached",
                        )["job_id"],
                        timeout=600,
                    )
                    self.assertEqual(
                        rust_snapshot["state"], "completed", rust_snapshot.get("error")
                    )
                    self.assertTrue(
                        rust_snapshot["candidates"],
                        f"the NG{playthrough} cached route found nothing",
                    )
                    self.assertEqual(
                        candidate_seeds(rust_snapshot),
                        candidate_seeds(python_snapshot),
                        f"NG{playthrough}: candidate identity differs from Python",
                    )
                    self.assertEqual(
                        candidate_cursors(rust_snapshot),
                        candidate_cursors(python_snapshot),
                        f"NG{playthrough}: candidate cursor differs from Python",
                    )
                    self.assertEqual(
                        rust_snapshot["cursor"],
                        python_snapshot["cursor"],
                        f"NG{playthrough}: page cursor differs from Python",
                    )
                    self.assertEqual(
                        rust_snapshot["stop_reason"],
                        python_snapshot["stop_reason"],
                        f"NG{playthrough}: stop reason differs from Python",
                    )
                    for candidate in rust_snapshot["candidates"]:
                        self.assertEqual(
                            candidate.get("rarity"), 5, "the cached route serves rarity 5"
                        )

            # Refusals: without a registered map, with a map of the wrong
            # playthrough, and with an unknown cache id, both workers must refuse
            # the same way instead of silently searching the bundled map.
            ng4_cache = self.register_grace_cache(
                rust, self.synthetic_grace_cache(4, rust_context)
            )
            unknown = "0" * 64
            cases = {
                "ng4_without_map": search_params(
                    base_query(playthrough=4, rarity=5),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                ),
                "ng4_with_unknown_cache": search_params(
                    base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    cache_id=unknown,
                ),
                "ng4_rarity4_query": search_params(
                    base_query(playthrough=4, rarity=4),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    cache_id=ng4_cache,
                ),
            }
            for name, params in cases.items():
                with self.subTest(refusal=name):
                    rust_code = self.terminal_failure_code(rust, "search.start", params)
                    python_code = self.terminal_failure_code(
                        python,
                        "search.start",
                        {**params, "context_digest": python_context},
                    )
                    self.assertEqual(
                        rust_code,
                        "INVALID_REQUEST",
                        f"{name}: the cached route must fail closed",
                    )
                    self.assertEqual(
                        python_code,
                        rust_code,
                        f"{name}: the shipped worker refuses this with a different code",
                    )
        finally:
            rust.close()
            python.close()

    def test_cached_ng4_ng5_whole_payload_matches_the_python_worker(self) -> None:
        """The cached route must match the shipped worker beyond candidate ids.

        The cached NG4/NG5 path composes a full payload (effects, auxiliary,
        enemy-state half, installation metadata, context and the private export),
        so identity/order/cursor agreement is not enough: every returned
        candidate and its `candidate.export` block must equal Python's, on
        several synthetic partitions, levels, caller filters and page depths,
        including a no-hit budget, cancel/resume, and negative map bindings.

        A real defect this gate found: the Rust payload published a composed
        `enemy_states` half for NG4/NG5, while the shipped
        `worker_contracts.candidate_payload` publishes `enemy_states: null` for
        every playthrough other than NG3 and never generates those previews. The
        port now composes the enemy half only for NG3, exactly like the shipped
        worker.
        """

        rust = self.rust_worker()
        python = self.python_worker()
        try:
            rust_context = self.handshake_context(rust)["context_digest"]
            python_context = self.handshake_context(python)["context_digest"]
            self.assertEqual(rust_context, python_context)

            cases = (
                (
                    "ng4_grace_only",
                    4,
                    base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                    "two_range",
                    2,
                    200_000,
                    False,
                ),
                (
                    "ng5_grace_only",
                    5,
                    base_query(playthrough=5, rarity=5, grace_effect_id=25939),
                    "two_range",
                    2,
                    200_000,
                    False,
                ),
                (
                    "ng4_late_map_level60",
                    4,
                    base_query(playthrough=4, rarity=5, grace_effect_id=25939, level=60),
                    "late",
                    3,
                    40_000,
                    False,
                ),
                (
                    "ng5_filtered_interleaved",
                    5,
                    base_query(
                        playthrough=5,
                        rarity=5,
                        required_secondary_ids=[6410],
                        grace_effect_id=5858,
                    ),
                    "interleaved",
                    3,
                    60_000,
                    False,
                ),
                (
                    "ng4_deep_pagination",
                    4,
                    base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                    "two_range",
                    25,
                    200_000,
                    True,
                ),
            )
            for name, playthrough, query, shape, count, page, complete in cases:
                with self.subTest(case=name):
                    snapshot = self.assert_cached_case_matches(
                        rust,
                        python,
                        name=name,
                        playthrough=playthrough,
                        query=query,
                        rust_context=rust_context,
                        python_context=python_context,
                        shape=shape,
                        result_count=count,
                        page_trials=page,
                        continue_until_complete=complete,
                    )
                    self.assertTrue(
                        snapshot["candidates"],
                        f"{name}: the case must publish candidates to compare",
                    )
                    for candidate in snapshot["candidates"]:
                        self.assertIsNone(
                            candidate.get("enemy_states"),
                            f"{name}: a cached NG4/NG5 payload publishes no enemy-state half",
                        )
                        self.assertEqual(candidate.get("playthrough"), playthrough)

            # A budget with no hit at all must still agree exactly.
            with self.subTest(case="ng4_no_hit"):
                nohit = self.assert_cached_case_matches(
                    rust,
                    python,
                    name="ng4_no_hit",
                    playthrough=4,
                    query=base_query(
                        playthrough=4,
                        rarity=5,
                        primary_effect_ids=[60020],
                        required_secondary_ids=[12028],
                        grace_effect_id=5858,
                    ),
                    rust_context=rust_context,
                    python_context=python_context,
                    result_count=3,
                    page_trials=2_000,
                )
                self.assertFalse(
                    nohit["candidates"], "the tiny fixed budget is a genuine no-hit case"
                )
                self.assertEqual(nohit["stop_reason"], "budget_reached")

            # Cancel and resume through the same registered map.
            resume_query = base_query(playthrough=4, rarity=5, grace_effect_id=5858)
            rust_cache = self.register_grace_cache(
                rust, self.synthetic_grace_cache(4, rust_context)
            )
            python_cache = self.register_grace_cache(
                python, self.synthetic_grace_cache(4, python_context)
            )
            self.assertEqual(rust_cache, python_cache)
            shipped = wait_for_terminal(
                python,
                python.result(
                    "search.start",
                    search_params(
                        resume_query,
                        python_context,
                        result_count=25,
                        page_trials=20_000_000,
                        job_trials=400_000_000,
                        continue_until_complete=True,
                        cache_id=python_cache,
                        **self.policy(),
                    ),
                    label="python cached continuing",
                )["job_id"],
                timeout=900,
            )
            started = rust.result(
                "search.start",
                search_params(
                    resume_query,
                    rust_context,
                    result_count=25,
                    page_trials=20_000_000,
                    job_trials=400_000_000,
                    continue_until_complete=True,
                    cache_id=rust_cache,
                    **self.policy(),
                ),
                label="rust cached continuing",
            )
            time.sleep(0.25)
            inflight = rust.result("job.snapshot", {"job_id": started["job_id"]})
            self.assertNotIn(
                inflight["state"],
                TERMINAL_STATES,
                "the cached page finished before the cancel landed",
            )
            rust.result("job.cancel", {"job_id": started["job_id"]})
            cancelled = wait_for_terminal(rust, started["job_id"], timeout=60)
            self.assertEqual(cancelled["state"], "cancelled")
            token = cancelled.get("resume_token")
            self.assertIsNotNone(token, "a cancelled cached job publishes its token")
            resumed = rust.result(
                "search.start",
                search_params(
                    resume_query,
                    rust_context,
                    result_count=25,
                    page_trials=20_000_000,
                    job_trials=400_000_000,
                    continue_until_complete=True,
                    resume_token=token,
                    cache_id=rust_cache,
                    **self.policy(),
                ),
                label="resume cached search.start",
            )
            resumed_snapshot = wait_for_terminal(rust, resumed["job_id"], timeout=900)
            union = candidate_seeds(cancelled) + candidate_seeds(resumed_snapshot)
            self.assertEqual(len(union), len(set(union)), "a resume must not replay")
            shipped_seeds = candidate_seeds(shipped)
            overlap = min(len(union), len(shipped_seeds))
            self.assertEqual(
                union[:overlap],
                shipped_seeds[:overlap],
                "cancel plus resume must enumerate the shipped candidates in order",
            )

            # Negative and stale map bindings: a register-time context mismatch,
            # a map of another playthrough, a rarity-4 query through the cache,
            # an unknown id and no map at all must all refuse identically.
            stale = self.synthetic_grace_cache(4, "b" * 64)
            for worker, label in ((rust, "rust"), (python, "python")):
                reply = worker.call("cache.register", {"cache_json": stale})
                self.assertFalse(
                    reply.get("ok"), f"{label}: a stale generation context must refuse"
                )
                self.assertEqual(reply["error"]["code"], "INVALID_REQUEST")
            ng5_cache = self.register_grace_cache(
                rust, self.synthetic_grace_cache(5, rust_context)
            )
            refusals = {
                "ng4_query_with_ng5_map": search_params(
                    base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    cache_id=ng5_cache,
                ),
                "ng4_rarity4_query": search_params(
                    base_query(playthrough=4, rarity=4),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    cache_id=rust_cache,
                ),
                "ng4_unknown_cache": search_params(
                    base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                    cache_id="0" * 64,
                ),
                "ng4_without_map": search_params(
                    base_query(playthrough=4, rarity=5),
                    rust_context,
                    page_trials=100_000,
                    job_trials=100_000,
                ),
            }
            for name, params in refusals.items():
                with self.subTest(refusal=name):
                    rust_code = self.terminal_failure_code(rust, "search.start", params)
                    python_code = self.terminal_failure_code(
                        python,
                        "search.start",
                        {**params, "context_digest": python_context},
                    )
                    self.assertEqual(rust_code, "INVALID_REQUEST", f"{name}: fail closed")
                    self.assertEqual(python_code, rust_code, f"{name}: code parity")

            # `candidate.preview` carries no cache parameter in the shipped
            # contract, so a cached Seed previews against the bundled NG3 map on
            # both sides. This pins that the preview path is not silently
            # cache-aware and that both workers compose it identically.
            cached_seed = self.assert_cached_case_matches(
                rust,
                python,
                name="ng4_preview_source",
                playthrough=4,
                query=base_query(playthrough=4, rarity=5, grace_effect_id=5858),
                rust_context=rust_context,
                python_context=python_context,
                result_count=1,
                page_trials=200_000,
            )["candidates"][0]["seed"]
            rust_preview = rust.result(
                "candidate.preview", {"seed": cached_seed, "rarity": 5, "level": 180}
            )
            python_preview = python.result(
                "candidate.preview", {"seed": cached_seed, "rarity": 5, "level": 180}
            )
            self.assertEqual(
                self.normalise_payload(rust_preview),
                self.normalise_payload(python_preview),
                "candidate.preview must match the shipped worker for a cached Seed",
            )
        finally:
            rust.close()
            python.close()


if __name__ == "__main__":
    unittest.main()
