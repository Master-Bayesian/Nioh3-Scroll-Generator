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

# The M2.3b1 supported routes are the NG3 auxiliary route (terrain keys, special
# rule keys and groups, enemy lookup keys and groups) and the R4 primary route.
# An unconstrained query is an effect-constraint search, which runs through the
# effect-preimage accelerator and is deliberately unsupported in this slice.
RULE_ROUTE_KEY = 113
PRIMARY_ROUTE_EFFECT = 30543


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
        if "worker" not in name and not any("worker" in entry for entry in bins):
            continue
        return manifest, (bins[0] if bins else name)
    raise AssertionError(
        "the development worker crate is required by this gate and was not found "
        f"under {ROOT / 'crates'}"
    )


def worktree_env() -> dict[str, str]:
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / ".codex_tmp" / "m23-worker-target"))
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

    def test_unported_effect_and_r3_routes_are_rejected(self) -> None:
        """Unsupported routes must reject explicitly, never run silently.

        Recorded as an accepted M2.3b1 boundary, each arm naming its own actual
        reason: an unconstrained query needs the effect-constraint replay route,
        and a rarity-3 *primary* query needs the batched primary/replay route
        (the native evidence shows it is not the effect-preimage DLL), so both
        are refused with INVALID_REQUEST while the Python worker still serves
        them. The auxiliary-only rarity-3 and rarity-5 routes are a different,
        served surface and are gated separately.
        """

        # Each arm names the reason it actually misses. The rarity-3 primary
        # message must not be the rarity-5 effect-preimage text: the native
        # evidence shows that route is the full-family/batched-primary replay,
        # so this arm requires its own reason word and forbids the wrong one.
        cases = {
            "unconstrained": {
                "query": base_query(),
                "reason": "effect",
                "forbidden": None,
            },
            "r3_primary": {
                "query": base_query(
                    rarity=3, primary_effect_ids=[PRIMARY_ROUTE_EFFECT]
                ),
                "reason": "primary",
                "forbidden": "effect-preimage",
            },
        }
        rust = self.rust_worker()
        try:
            context = self.handshake_context(rust)["context_digest"]
            for name, case in cases.items():
                with self.subTest(route=name):
                    reply = rust.call(
                        "search.start",
                        search_params(
                            case["query"],
                            context,
                            page_trials=1_000_000,
                            job_trials=1_000_000,
                        ),
                    )
                    self.assertFalse(
                        reply.get("ok"),
                        f"{name} must be refused, not silently searched: {reply}",
                    )
                    self.assertEqual(reply["error"]["code"], "INVALID_REQUEST")
                    self.assertTrue(
                        case["reason"] in reply["error"]["message"],
                        f"{name} must name the missing route it actually needs "
                        f"(expected {case['reason']!r}): {reply['error']['message']}",
                    )
                    if case["forbidden"] is not None:
                        self.assertNotIn(
                            case["forbidden"],
                            reply["error"]["message"],
                            f"{name} must not report the rarity-5 effect-preimage "
                            "text for a route the native evidence shows is a "
                            "different one",
                        )
        finally:
            rust.close()

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
                    # Effect constraints run through the effect-preimage
                    # accelerator, which this slice does not port, so the route
                    # must refuse rather than return unaffected candidates.
                    "rejected",
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

    def test_unported_ng4_ng5_search_is_unadvertised_and_fails_closed(self) -> None:
        rust = self.rust_worker()
        try:
            handshake = rust.result("handshake")
            self.assertNotIn(
                "cached_rarity5_playthroughs",
                handshake["capabilities"],
                "the NG4/NG5 cache is not ported, so it must not be advertised",
            )
            context = handshake["context"]["context_digest"]
            for playthrough in (4, 5):
                query = base_query(playthrough=playthrough, rarity=5)
                params = search_params(query, context, page_trials=100_000, job_trials=100_000)
                actual = self.terminal_failure_code(rust, "search.start", params)
                # Accepted boundary (documented, not a parity claim): the NG4/NG5
                # cache is deliberately unported, so this worker rejects the route
                # with INVALID_REQUEST while the Python worker can still accept it
                # through its own save-bound map. Rejection is the contract here.
                self.assertEqual(
                    actual,
                    "INVALID_REQUEST",
                    f"playthrough {playthrough} without a save-bound rarity-5 map "
                    "must be rejected, never silently ignored",
                )
        finally:
            rust.close()


if __name__ == "__main__":
    unittest.main()
