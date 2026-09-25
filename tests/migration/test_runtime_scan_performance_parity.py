"""Matched-workload acceptance for the protected runtime scan and map routes.

Both halves of this gate run the *same* deterministic oracle: a call answers
with a record built by the shipped ``build_source_record`` emitter, so the oracle
cost and the produced bytes are identical on both sides and the only difference
left is host overhead (the shipped Python loops versus the Rust port). That is
what makes the timings comparable rather than a comparison of two different
oracles.

The gate asserts, per route, that the Rust result and the recorded oracle call
sequence match the shipped host exactly - candidate identity, record stage, the
published stage-one record, solver cursor, and the continuation pair for a miss -
and then records both elapsed times. The ratios are written to
``deliverables/m3-protected-host/perf/scan_perf.json`` and bounded so a real
algorithmic regression is caught without turning scheduler noise into a failure.

No game process is involved: the routes run at library level on the Python side
and through the ``scan_bench`` example on the Rust side.
"""

from __future__ import annotations

from dataclasses import asdict
import json
import os
from pathlib import Path
import hashlib
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

from nioh3_scroll_editor.native import build_source_record, scan_next_candidate  # noqa: E402
from nioh3_scroll_editor import native_search_maps  # noqa: E402
from nioh3_scroll_editor.grace_map import build_live_grace_output_map  # noqa: E402
from nioh3_scroll_editor.models import CandidateRecordStage, ScrollCandidate  # noqa: E402
from nioh3_scroll_editor.auxiliary_generation import AuxiliarySearchCriteria  # noqa: E402
from nioh3_scroll_editor.seed_accelerator import (  # noqa: E402
    cuda_seed_acceleration_available,
    native_seed_acceleration_available,
    seed_accelerator_identity,
)

EFFECT_START = 0x34
EFFECT_STRIDE = 0x18
SCROLL_RECORD_SIZE = 0xE8
CATEGORY_TO_TYPE = [0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523]

EVIDENCE = ROOT / "deliverables" / "m3-protected-host" / "perf" / "scan_perf.json"
WORK_ROOT = Path(
    os.environ.get(
        "NIOH3_PROTECTED_PERF_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-protected-host\perf",
    )
)

# A route may not take more than this multiple of the shipped host's time. The
# bound is deliberately loose: it exists to catch a real algorithmic regression,
# not scheduler noise.
MAX_SLOWDOWN = 3.0


def filled_template(category: int, rarity: int) -> bytes:
    """A product-shaped template with a deterministic effect area.

    The shipped emitter copies the effect area, so a populated template is what
    makes every generated row carry resolved slots without hand-written bytes.
    """

    record = bytearray(SCROLL_RECORD_SIZE)
    record[0:2] = CATEGORY_TO_TYPE[category].to_bytes(2, "little")
    record[0x30] = rarity
    record[0x31] = rarity
    for slot in range(7):
        offset = EFFECT_START + slot * EFFECT_STRIDE + 4
        record[offset : offset + 4] = (0x0100 * (slot + 1)).to_bytes(4, "little")
    return bytes(record)


class StubOracle:
    """The Python half of the matched oracle.

    Mirrors ``nioh3_protected::oracle::scripted::ScriptedOracle``: every call is
    answered from the shipped ``build_source_record`` emitter, and the completion
    pass returns its input unchanged, which is exactly what the Rust scripted
    oracle does when no completed row is scripted.
    """

    def __init__(
        self,
        template: bytes,
        rarity: int,
        level: int,
        recommended_level: int,
        transfer_count: int = 0,
        max_batch_size: int = 128,
    ) -> None:
        self.template = template
        self.rarity = rarity
        self.level = level
        self.recommended_level = recommended_level
        self.transfer_count = transfer_count
        self.max_batch_size = max_batch_size
        self.calls: list[dict] = []

    def _row_for_seed(self, seed: int) -> bytes:
        return build_source_record(
            self.template,
            seed=seed,
            rarity=self.rarity,
            level=self.level,
            recommended_level=self.recommended_level,
            transfer_count=self.transfer_count,
        )

    @staticmethod
    def _seed_of(record: bytes) -> int:
        return struct.unpack_from("<I", record, 0x20)[0]

    def generate(self, source_records, *, timeout_ms: int = 60_000) -> list[bytes]:
        seeds = [self._seed_of(record) for record in source_records]
        self.calls.append({"call": "generate", "seeds": seeds})
        return [self._row_for_seed(seed) for seed in seeds]

    def generate_seed_range(
        self,
        template: bytes,
        *,
        start_seed: int,
        seed_step: int,
        count: int,
        playthrough: int | None = None,
        generation_mode: int = 0,
        timeout_ms: int = 60_000,
    ) -> list[bytes]:
        self.calls.append(
            {
                "call": "seed_range",
                "start_seed": start_seed,
                "seed_step": seed_step,
                "count": count,
                "playthrough": playthrough,
            }
        )
        return [
            self._row_for_seed((start_seed + index * seed_step) & 0xFFFFFFFF)
            for index in range(count)
        ]

    def finalize_stage_records_batch(self, source_records, reveal: bool = True):
        seeds = [self._seed_of(record) for record in source_records]
        self.calls.append({"call": "finalize", "seeds": seeds})
        return list(source_records)


class _Context:
    def __init__(self, digest: str) -> None:
        self.context_digest = digest


def template_payload(spec: dict) -> dict:
    return {
        "template_hex": spec["template_hex"],
        "source_sha256": "0" * 64,
        "context_digest": "b" * 64,
        "save_fingerprint": "a" * 64,
    }


def describe_maps(maps: dict) -> dict:
    if not maps:
        return {"maps": "none"}
    if "primary_first_output_map" in maps:
        return {
            "maps": "primary_first",
            "primary_effects": len(maps["primary_first_output_map"].effects),
        }
    if "primary_output_map" in maps:
        return {
            "maps": "joint",
            "grace_ranges": len(maps["grace_output_map"].ranges),
            "primary_effects": len(maps["primary_output_map"].effects),
        }
    return {"maps": "grace", "grace_ranges": len(maps["grace_output_map"].ranges)}


def describe_candidate(candidate: ScrollCandidate | None, last: dict, start_seed: int) -> dict:
    resume_seed = min(0xFFFFFFFF, last.get("current_seed", start_seed) + 1)
    resume_trial = last.get("joint_trial")
    if candidate is None:
        return {"candidate": None, "resume_seed": resume_seed, "resume_trial": resume_trial}
    stage = candidate.record_stage
    stage_name = stage.value if isinstance(stage, CandidateRecordStage) else str(stage)
    return {
        "candidate": {
            "seed": candidate.seed,
            "rarity": candidate.rarity,
            "playthrough": candidate.playthrough,
            "record_stage": stage_name,
            "has_installation_record": candidate.installation_record is not None,
            "cursor": candidate.joint_search_trial,
            "predicted_growth_grace_id": candidate.predicted_growth_grace_id,
        },
        "resume_seed": resume_seed,
        "resume_trial": resume_trial,
    }


def prepare_maps_python(oracle: StubOracle, spec: dict) -> dict:
    return native_search_maps.prepare_maps(
        oracle,
        template_payload(spec),
        spec["playthrough"],
        spec["rarity"],
        spec["level"],
        spec["recommended_level"],
        spec.get("criteria", {}),
        _Context("b" * 64),
        None,
        lambda value: None,
    )


def run_python_route(spec: dict) -> dict:
    template = bytes.fromhex(spec["template_hex"])
    oracle = StubOracle(
        template,
        spec["rarity"],
        spec["level"],
        spec["recommended_level"],
        spec.get("transfer_count", 0),
        spec.get("max_batch_size", 128),
    )
    route = spec["route"]
    last: dict = {}

    def report(update) -> None:
        # `ScanProgress` is a slots dataclass; `prepare_maps` reports a dict.
        payload = update if isinstance(update, dict) else asdict(update)
        last.clear()
        last.update(payload)

    started = time.perf_counter()
    if route == "grace_capture":
        mapping = build_live_grace_output_map(
            oracle,
            template=template,
            category=spec["playthrough"],
            rarity=spec["rarity"],
            level=spec["level"],
            recommended_level=spec["recommended_level"],
        )
        result = {
            "captured": True,
            "rarity": mapping.rarity,
            "effect_slot": mapping.effect_slot,
            "ranges": len(mapping.ranges),
        }
    elif route == "prepare_maps":
        result = describe_maps(prepare_maps_python(oracle, spec))
    else:
        maps = {} if route == "plain" else prepare_maps_python(oracle, spec)
        cursor = (
            {"joint_start_after_trial": spec.get("after_trial", 0)}
            if any(key.startswith("primary_") for key in maps)
            else (
                {"grace_start_after_seed": spec.get("start_seed", 0) - 1}
                if maps and spec.get("start_seed", 0)
                else {}
            )
        )
        criteria = spec.get("criteria", {})
        auxiliary = AuxiliarySearchCriteria(
            **{
                key: tuple(frozenset(group) for group in value)
                if key.endswith("_groups")
                else frozenset(value)
                for key, value in criteria.get("auxiliary", {}).items()
            }
        )
        candidate = scan_next_candidate(
            oracle,
            template=template,
            start_seed=spec.get("start_seed", 0),
            primary_effect_ids=frozenset(criteria.get("primary_effect_ids", [])),
            required_secondary_ids=frozenset(criteria.get("required_secondary_ids", [])),
            required_secondary_id_groups=tuple(
                frozenset(group) for group in criteria.get("required_secondary_id_groups", [])
            ),
            grace_effect_id=criteria.get("grace_effect_id"),
            rarity=spec["rarity"],
            level=spec["level"],
            recommended_level=spec["recommended_level"],
            playthrough=spec.get("playthrough"),
            max_seeds=spec.get("max_seeds", 1),
            accelerate_grace=bool(maps),
            cancel_event=None,
            progress=report,
            auxiliary_criteria=auxiliary,
            **maps,
            **cursor,
        )
        result = describe_candidate(candidate, last, spec.get("start_seed", 0))
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    return {
        "route": route,
        "elapsed_ms": elapsed_ms,
        "calls": oracle.calls,
        "result": result,
    }


class RuntimeScanPerformanceParity(unittest.TestCase):
    """One matched workload per route, timed on both hosts."""

    @classmethod
    def setUpClass(cls) -> None:
        manifest = ROOT / "crates" / "nioh3-protected" / "Cargo.toml"
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build the scan benchmark")
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = resolved_cargo_target_dir("protected-scan-perf")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--offline",
                "--release",
                "--features",
                "test-fake",
                "--manifest-path",
                str(manifest),
                "--example",
                "scan_bench",
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the scan benchmark did not build: "
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.bench = Path(env["CARGO_TARGET_DIR"]) / "release" / "examples" / "scan_bench.exe"
        if not cls.bench.is_file():
            raise AssertionError(f"the scan benchmark binary is missing: {cls.bench}")
        WORK_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(WORK_ROOT))
        cls.root = Path(cls.temp.name)
        cls.evidence: list[dict] = []

    @classmethod
    def accelerator_baseline(cls) -> dict:
        """The accelerator identity the Python baseline actually loaded.

        The manifest carries two different hashes of the same artefact - the
        source/build id exported by the DLL and the DLL's own SHA-256 - so both
        are recorded, and the on-disk file is hashed to prove no changed binary
        was silently blessed into the comparison.
        """

        override = os.environ.get("NIOH3_SEED_ACCELERATOR", "").strip()
        module = Path(override) if override else ROOT / "bin" / "nioh3_seed_accelerator.dll"
        manifest_path = ROOT / "bin" / "nioh3_seed_accelerator.build.json"
        report: dict = {"module": str(module)}
        if module.is_file():
            report["dll_sha256"] = hashlib.sha256(module.read_bytes()).hexdigest()
        if manifest_path.is_file():
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            report["manifest_build_id"] = manifest.get("build_id")
            report["manifest_source_sha256"] = manifest.get("source_sha256")
            report["manifest_binary_sha256"] = manifest.get("binary_sha256")
            report["manifest_abi_version"] = manifest.get("abi_version")
        return report

    @classmethod
    def tearDownClass(cls) -> None:
        EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
        # The shipped `iter_constraint_intersection` enables its native pivot
        # accelerator by default, so the Python baseline is the accelerated one
        # whenever the DLL loads. Recording the identity is what makes that
        # statement checkable instead of assumed.
        identity = seed_accelerator_identity()
        baseline = cls.accelerator_baseline()
        if baseline.get("dll_sha256") and baseline.get("manifest_binary_sha256"):
            # A changed binary must not be blessed into the baseline green.
            if baseline["dll_sha256"] != baseline["manifest_binary_sha256"]:
                raise AssertionError(
                    "the seed accelerator on disk does not match its build "
                    f"manifest: {baseline['dll_sha256']} != "
                    f"{baseline['manifest_binary_sha256']}"
                )
        if identity and baseline.get("manifest_build_id"):
            loaded = identity[1]
            if not loaded.startswith("sha256:"):
                loaded = f"sha256:{loaded}"
            if loaded != baseline["manifest_build_id"]:
                raise AssertionError(
                    "the loaded accelerator build id does not match the manifest: "
                    f"{loaded} != {baseline['manifest_build_id']}"
                )
        EVIDENCE.write_text(
            json.dumps(
                {
                    "max_slowdown": MAX_SLOWDOWN,
                    "max_slowdown_note": (
                        "catastrophic-regression tripwire, not a no-regression "
                        "standard; the measured ratios are in each route entry"
                    ),
                    "oracle": "shipped build_source_record, identical on both hosts",
                    "python_seed_accelerator_available": native_seed_acceleration_available(),
                    "python_seed_accelerator_identity": (
                        {"abi": identity[0], "build_id": identity[1]} if identity else None
                    ),
                    "python_seed_accelerator_baseline": baseline,
                    "routes": cls.evidence,
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        cls.temp.cleanup()

    def run_rust_route(self, spec: dict, name: str, state: Path | None = None) -> dict:
        state = state or self.root / f"{name}-rust-state"
        state.mkdir(parents=True, exist_ok=True)
        payload = dict(spec)
        # A reuse run must see the map the previous run wrote, so the state root
        # is only cleared when the caller asks for a cold one.
        payload["clean_state"] = bool(spec.get("clean_state", True))
        env = dict(os.environ)
        env["NIOH3_STATE_ROOT"] = str(state)
        completed = subprocess.run(
            [str(self.bench)],
            cwd=str(ROOT),
            env=env,
            input=json.dumps(payload),
            text=True,
            capture_output=True,
            timeout=1800,
        )
        if completed.returncode != 0:
            raise AssertionError(
                f"the scan benchmark failed: {completed.stdout} {completed.stderr}"
            )
        report = json.loads(completed.stdout.strip().splitlines()[-1])
        if "error" in report:
            raise AssertionError(f"the scan benchmark reported {report['error']}")
        return report

    def test_measured_maps_interoperate_across_hosts(self) -> None:
        """A map writen by one host must be reusable by the other.

        Both caches use the shipped payload schema, so sharing one state
        directory across the two hosts proves the measured map - not just the
        route result - is byte-compatible.
        """

        shared = self.root / "interop"
        spec = {
            "route": "prepare_maps",
            "label": "interop",
            "template_hex": filled_template(3, 4).hex(),
            "rarity": 4,
            "playthrough": 3,
            "level": 180,
            "recommended_level": 183,
            "max_seeds": 1,
            "criteria": {"grace_effect_id": 0x0500},
        }
        # Python measures into the shared root.
        os.environ["NIOH3_STATE_ROOT"] = str(shared)
        try:
            python_cold = run_python_route(spec)
            python_reuse = run_python_route(spec)
        finally:
            os.environ.pop("NIOH3_STATE_ROOT", None)
        self.assertGreater(len(python_cold["calls"]), 0)
        self.assertEqual(python_reuse["calls"], [], "the Python reuse must read the cache")

        # Rust must accept the map Python wrote, without re-measuring.
        rust_reuse = self.run_rust_route(dict(spec, clean_state=False), "interop", state=shared)
        self.assertEqual(
            rust_reuse["calls"],
            [],
            "the Rust host must reuse the map the shipped host wrote",
        )
        self.assertEqual(rust_reuse["result"], python_reuse["result"])

        # And the reverse: a map Rust writes must be accepted by the shipped host.
        for path in shared.rglob("*.json"):
            path.unlink()
        rust_cold = self.run_rust_route(dict(spec, clean_state=False), "interop", state=shared)
        self.assertGreater(len(rust_cold["calls"]), 0)
        previous = os.environ.get("NIOH3_STATE_ROOT")
        os.environ["NIOH3_STATE_ROOT"] = str(shared)
        try:
            python_from_rust = run_python_route(spec)
        finally:
            if previous is None:
                os.environ.pop("NIOH3_STATE_ROOT", None)
            else:
                os.environ["NIOH3_STATE_ROOT"] = previous
        self.assertEqual(
            python_from_rust["calls"],
            [],
            "the shipped host must reuse the map the Rust host wrote",
        )
        self.assertEqual(python_from_rust["result"], rust_cold["result"])

    def run_python_with_state(self, spec: dict, name: str) -> dict:
        state = self.root / f"{name}-python-state"
        state.mkdir(parents=True, exist_ok=True)
        previous = os.environ.get("NIOH3_STATE_ROOT")
        os.environ["NIOH3_STATE_ROOT"] = str(state)
        try:
            return run_python_route(spec)
        finally:
            if previous is None:
                os.environ.pop("NIOH3_STATE_ROOT", None)
            else:
                os.environ["NIOH3_STATE_ROOT"] = previous

    def compare_route(self, spec: dict) -> dict:
        name = f"{spec['route']}-{spec.get('label', 'case')}"
        python = self.run_python_with_state(spec, name)
        rust = self.run_rust_route(spec, name)
        self.assertEqual(
            rust["result"],
            python["result"],
            f"{name}: the Rust route result must match the shipped host",
        )
        self.assertEqual(
            rust["calls"],
            python["calls"],
            f"{name}: the oracle call sequence must match the shipped host",
        )
        ratio = rust["elapsed_ms"] / max(python["elapsed_ms"], 1e-6)
        entry = {
            "route": spec["route"],
            "label": spec.get("label", "case"),
            "python_ms": round(python["elapsed_ms"], 3),
            "rust_ms": round(rust["elapsed_ms"], 3),
            "rust_over_python": round(ratio, 3),
            "oracle_ms_per_call_note": "identical oracle on both hosts",
            "oracle_calls": len(rust["calls"]),
        }
        self.evidence.append(entry)
        self.assertLess(
            ratio,
            MAX_SLOWDOWN,
            f"{name}: the Rust route is {ratio:.2f}x the shipped host, above the "
            f"{MAX_SLOWDOWN}x limit (python {python['elapsed_ms']:.1f} ms, "
            f"rust {rust['elapsed_ms']:.1f} ms)",
        )
        return entry

    def test_plain_generate_routes_match(self) -> None:
        for label, primary in (("hit", []), ("nohit", [0xDEAD])):
            self.compare_route(
                {
                    "route": "plain",
                    "label": label,
                    "template_hex": filled_template(3, 4).hex(),
                    "rarity": 4,
                    "playthrough": 3,
                    "level": 180,
                    "recommended_level": 183,
                    "max_seeds": 1 if label == "hit" else 4,
                    "start_seed": 0x0002_0001,
                    "criteria": {"primary_effect_ids": primary},
                }
            )

    def test_grace_capture_cold_and_reuse_match(self) -> None:
        capture = {
            "route": "grace_capture",
            "label": "cold",
            "template_hex": filled_template(3, 4).hex(),
            "rarity": 4,
            "playthrough": 3,
            "level": 180,
            "recommended_level": 183,
            "max_seeds": 1,
            "criteria": {"grace_effect_id": 0x0500},
        }
        self.compare_route(capture)

        # A second preparation in the same state root must reuse the cache. Both
        # sides run it in one process per side so the reuse path is timed.
        prepare = dict(capture, route="prepare_maps")
        name = "prepare_maps-reuse"
        python_state = self.root / f"{name}-python-state"
        python_state.mkdir(parents=True, exist_ok=True)
        previous = os.environ.get("NIOH3_STATE_ROOT")
        os.environ["NIOH3_STATE_ROOT"] = str(python_state)
        try:
            python_cold = run_python_route(dict(prepare, label="cold"))
            python_reuse = run_python_route(dict(prepare, label="reuse"))
        finally:
            if previous is None:
                os.environ.pop("NIOH3_STATE_ROOT", None)
            else:
                os.environ["NIOH3_STATE_ROOT"] = previous
        self.assertEqual(python_cold["result"], python_reuse["result"])
        self.assertLess(
            len(python_reuse["calls"]),
            len(python_cold["calls"]),
            "the reuse preparation must not re-measure the map",
        )
        rust_cold = self.run_rust_route(dict(prepare, label="cold"), name)
        rust_reuse = self.run_rust_route(
            dict(prepare, label="reuse", clean_state=False), name
        )
        self.assertEqual(rust_cold["result"], python_cold["result"])
        self.assertEqual(rust_reuse["result"], python_reuse["result"])
        self.assertEqual(rust_reuse["calls"], python_reuse["calls"])
        ratio = rust_reuse["elapsed_ms"] / max(python_reuse["elapsed_ms"], 1e-6)
        self.evidence.append(
            {
                "route": "prepare_maps_reuse",
                "label": "reuse",
                "python_ms": round(python_reuse["elapsed_ms"], 3),
                "rust_ms": round(rust_reuse["elapsed_ms"], 3),
                "rust_over_python": round(ratio, 3),
                "oracle_ms_per_call_note": "identical oracle on both hosts",
                "oracle_calls": len(rust_reuse["calls"]),
            }
        )
        self.assertLess(ratio, MAX_SLOWDOWN)

    def test_accelerated_routes_match(self) -> None:
        if not cuda_seed_acceleration_available():
            # The Python reference refuses bulk CPU pivots without an opt-in,
            # so these accelerated routes only run on a CUDA host.
            self.skipTest("the accelerated Python routes need a CUDA device")
        grace_hit = {
            "route": "grace_accelerated",
            "label": "hit",
            "template_hex": filled_template(3, 4).hex(),
            "rarity": 4,
            "playthrough": 3,
            "level": 180,
            "recommended_level": 183,
            "max_seeds": 1,
            "criteria": {"grace_effect_id": 0x0500},
        }
        self.compare_route(grace_hit)
        self.compare_route(
            dict(grace_hit, label="nohit", max_seeds=4, criteria={"grace_effect_id": 0x0500, "primary_effect_ids": [0xDEAD]})
        )

        primary = {
            "route": "primary_first",
            "label": "hit",
            "template_hex": filled_template(2, 5).hex(),
            "rarity": 5,
            "playthrough": 2,
            "level": 180,
            "recommended_level": 183,
            "max_seeds": 4,
            "criteria": {"primary_effect_ids": [0x0100]},
        }
        self.compare_route(primary)
        self.compare_route(
            dict(
                primary,
                label="nohit",
                # The map only holds 0x0100, and the shipped `runs_for_effects`
                # refuses an absent primary, so the miss is expressed as an
                # unsatisfiable secondary requirement instead.
                criteria={
                    "primary_effect_ids": [0x0100],
                    "required_secondary_ids": [0xDEAD],
                },
            )
        )

        joint = {
            "route": "joint",
            "label": "hit",
            "template_hex": filled_template(3, 5).hex(),
            "rarity": 5,
            "playthrough": 3,
            "level": 180,
            "recommended_level": 183,
            "max_seeds": 4,
            "criteria": {"primary_effect_ids": [0x0100], "grace_effect_id": 0x0600},
        }
        self.compare_route(joint)
        self.compare_route(
            dict(
                joint,
                label="nohit",
                criteria={
                    "primary_effect_ids": [0x0100],
                    "required_secondary_ids": [0xDEAD],
                    "grace_effect_id": 0x0600,
                },
            )
        )
