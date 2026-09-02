from __future__ import annotations

"""Live parity gate for the game-closed NG3 rarity-4 generation chain.

The script calls signature-gated native stage generation and completion
finalization only in isolated remote buffers. It never reads or writes a save.
"""

import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import sys


PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from emaki_exchange import EFFECT_COUNT, EFFECT_START, EFFECT_STRIDE  # noqa: E402
from nioh3_scroll_editor.effect_sequence import (  # noqa: E402
    materialize_ng3_rarity4_final_record,
    materialize_ng3_rarity4_stage_one_record,
)
from nioh3_scroll_editor.native import (  # noqa: E402
    NativeBatchOracle,
    NativeRuntimeProfile,
    build_source_record,
    load_native_runtime_profile,
)
from nioh3_seed_math import is_natural_scroll_id  # noqa: E402


DEFAULT_TEMPLATE = (
    PROJECT_ROOT
    / "audit"
    / "r4_finalizer_capture"
    / "r4_native_corpus_distributed_20260827"
    / "sample_01_seed_1_stage.bin"
)


def deterministic_natural_seeds(count: int, rng_seed: int) -> tuple[int, ...]:
    if count <= 0:
        raise ValueError("count must be positive")
    anchors = (1, 2965, 183696634, 0x0FFFFFFF)
    seeds: list[int] = []
    seen: set[int] = set()
    for seed in anchors:
        if is_natural_scroll_id(seed) and seed not in seen:
            seeds.append(seed)
            seen.add(seed)
            if len(seeds) == count:
                return tuple(seeds)
    rng = random.Random(rng_seed)
    while len(seeds) < count:
        seed = rng.randrange(1, 0x10000000)
        if is_natural_scroll_id(seed) and seed not in seen:
            seeds.append(seed)
            seen.add(seed)
    return tuple(seeds)


def requires_finalization(record: bytes, effect_index: int) -> bool:
    offset = EFFECT_START + effect_index * EFFECT_STRIDE
    return (
        struct.unpack_from("<H", record, offset)[0] != 0
        and not record[offset + 0x0D] & 0x40
        and not record[offset + 0x0E] & 0x04
    )


def complete_native_batch(
    oracle: NativeBatchOracle,
    stage_records: list[bytes],
) -> tuple[list[bytes], list[int | None]]:
    outputs_by_index: list[dict[int, bytes]] = []
    for effect_index in range(EFFECT_COUNT):
        source_indices = [
            index
            for index, record in enumerate(stage_records)
            if requires_finalization(record, effect_index)
        ]
        if not source_indices:
            outputs_by_index.append({})
            continue
        outputs = oracle.finalize_effect_stage_batch(
            [stage_records[index] for index in source_indices],
            effect_index=effect_index,
        )
        outputs_by_index.append(dict(zip(source_indices, outputs, strict=True)))

    finals: list[bytes] = []
    accepted_indices: list[int | None] = []
    for record_index, stage in enumerate(stage_records):
        final = stage
        accepted_index = None
        for effect_index in range(EFFECT_COUNT):
            candidate = outputs_by_index[effect_index].get(record_index)
            if candidate is None:
                continue
            offset = EFFECT_START + effect_index * EFFECT_STRIDE
            if candidate[offset + 0x0E] & 0x04:
                final = candidate
                accepted_index = effect_index
                break
        finals.append(final)
        accepted_indices.append(accepted_index)
    return finals, accepted_indices


def differences(actual: bytes, expected: bytes) -> list[int]:
    return [
        index
        for index, (left, right) in enumerate(zip(actual, expected, strict=True))
        if left != right
    ]


def validate(
    *,
    template: bytes,
    seeds: tuple[int, ...],
    batch_size: int,
    level: int,
    recommended_level: int,
    runtime_profile: NativeRuntimeProfile | None = None,
) -> dict[str, object]:
    stage_mismatch_count = 0
    final_mismatch_count = 0
    accepted_index_mismatch_count = 0
    mismatch_samples: list[dict[str, object]] = []
    accepted_histogram: dict[str, int] = {}
    validated = 0

    oracle_options = {"max_batch_size": batch_size}
    if runtime_profile is not None:
        oracle_options["runtime_profile"] = runtime_profile
    with NativeBatchOracle(**oracle_options) as oracle:
        process_id = oracle.pid
        module_base = oracle.module_base
        for start in range(0, len(seeds), batch_size):
            batch_seeds = seeds[start : start + batch_size]
            sources = [
                build_source_record(
                    template,
                    seed=seed,
                    rarity=4,
                    level=level,
                    recommended_level=recommended_level,
                )
                for seed in batch_seeds
            ]
            native_stages = oracle.generate(sources)
            native_finals, native_indices = complete_native_batch(
                oracle,
                native_stages,
            )
            for seed, native_stage, native_final, native_index in zip(
                batch_seeds,
                native_stages,
                native_finals,
                native_indices,
                strict=True,
            ):
                generation_serial = struct.unpack_from("<I", native_stage, 0x28)[0]
                transfer_count = struct.unpack_from("<I", native_stage, 0xDC)[0]
                kwargs = {
                    "seed": seed,
                    "level": level,
                    "recommended_level": recommended_level,
                    "transfer_count": transfer_count,
                    "generation_serial": generation_serial,
                }
                offline_stage, _ = materialize_ng3_rarity4_stage_one_record(
                    template,
                    **kwargs,
                )
                offline_final, _ = materialize_ng3_rarity4_final_record(
                    template,
                    **kwargs,
                )
                stage_differences = differences(native_stage, offline_stage)
                final_differences = differences(native_final, offline_final)
                offline_index = None
                for effect_index in range(EFFECT_COUNT):
                    offset = EFFECT_START + effect_index * EFFECT_STRIDE
                    if (
                        native_stage[offset + 0x0E] & 0x04 == 0
                        and offline_final[offset + 0x0E] & 0x04
                    ):
                        offline_index = effect_index
                        break
                if stage_differences:
                    stage_mismatch_count += 1
                if final_differences:
                    final_mismatch_count += 1
                if native_index != offline_index:
                    accepted_index_mismatch_count += 1
                label = "none" if native_index is None else str(native_index)
                accepted_histogram[label] = accepted_histogram.get(label, 0) + 1
                if (
                    stage_differences
                    or final_differences
                    or native_index != offline_index
                ) and len(mismatch_samples) < 32:
                    mismatch_samples.append(
                        {
                            "seed": seed,
                            "seed_hex": f"0x{seed:08X}",
                            "native_accepted_index": native_index,
                            "offline_accepted_index": offline_index,
                            "stage_differing_offsets": stage_differences,
                            "final_differing_offsets": final_differences,
                        }
                    )
                validated += 1
            print(f"validated {validated:,} / {len(seeds):,}", file=sys.stderr)

    return {
        "schema": "nioh3-ng3-rarity4-live-native-parity/v1",
        "game_version": oracle.runtime_profile.display_version,
        "process_id": process_id,
        "module_base": f"0x{module_base:016X}",
        "record_type": "0xE604",
        "playthrough": 3,
        "rarity": 4,
        "level": level,
        "recommended_level": recommended_level,
        "seed_count": len(seeds),
        "seed_set_sha256": hashlib.sha256(
            b"".join(struct.pack("<I", seed) for seed in seeds)
        ).hexdigest(),
        "stage_mismatch_count": stage_mismatch_count,
        "final_mismatch_count": final_mismatch_count,
        "accepted_index_mismatch_count": accepted_index_mismatch_count,
        "accepted_index_histogram": accepted_histogram,
        "mismatch_samples": mismatch_samples,
        "stage_parity_pass": stage_mismatch_count == 0,
        "final_parity_pass": final_mismatch_count == 0,
        "accepted_index_parity_pass": accepted_index_mismatch_count == 0,
        "save_access": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--count", type=int, default=10_000)
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--rng-seed", type=int, default=0x52344649)
    parser.add_argument("--level", type=int, default=180)
    parser.add_argument("--recommended-level", type=int, default=183)
    parser.add_argument("--template", type=Path, default=DEFAULT_TEMPLATE)
    parser.add_argument("--runtime-profile", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    if not 1 <= args.batch_size <= 4096:
        parser.error("--batch-size must be in 1..4096")
    template = args.template.read_bytes()
    if len(template) != 0xE8 or struct.unpack_from("<H", template, 0)[0] != 0xE604:
        parser.error("--template must be one 0xE8 E604 record")
    seeds = deterministic_natural_seeds(args.count, args.rng_seed)
    payload = validate(
        template=template,
        seeds=seeds,
        batch_size=args.batch_size,
        level=args.level,
        recommended_level=args.recommended_level,
        runtime_profile=(
            load_native_runtime_profile(args.runtime_profile)
            if args.runtime_profile is not None
            else None
        ),
    )
    text = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if all(
        payload[key]
        for key in (
            "stage_parity_pass",
            "final_parity_pass",
            "accepted_index_parity_pass",
        )
    ) else 1


if __name__ == "__main__":
    raise SystemExit(main())
