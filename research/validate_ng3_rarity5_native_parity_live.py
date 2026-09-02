from __future__ import annotations

"""Live, read-only-save parity gate for the game-closed NG3 rarity-5 generator.

The script executes a signature-gated native canonicalizer in isolated remote
buffers. It never reads, writes, decrypts, or installs a save file.
"""

import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import sys
from typing import Iterator


PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE  # noqa: E402
from nioh3_scroll_editor.effect_sequence import (  # noqa: E402
    materialize_ng3_rarity5_record,
    serialize_ng3_rarity5_effect_slots,
)
from nioh3_scroll_editor.native import (  # noqa: E402
    NativeBatchOracle,
    NativeRuntimeProfile,
    build_source_record,
    load_native_runtime_profile,
)
from nioh3_seed_math import is_natural_scroll_id  # noqa: E402


DEFAULT_TEMPLATE_EVIDENCE = (
    PROJECT_ROOT
    / "audit"
    / "playthrough-matrix-ng3-current-vs-1-to-5-20260827.json"
)


def iter_dicts(value: object) -> Iterator[dict[str, object]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from iter_dicts(child)
    elif isinstance(value, list):
        for child in value:
            yield from iter_dicts(child)


def load_template(path: Path) -> bytes:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict) and isinstance(payload.get("samples"), list):
        for sample in payload["samples"]:
            if not isinstance(sample, dict):
                continue
            current = sample.get("current")
            if not isinstance(current, dict) or not isinstance(
                current.get("record_hex"), str
            ):
                continue
            record = bytes.fromhex(current["record_hex"])
            if (
                len(record) == SCROLL_RECORD_SIZE
                and struct.unpack_from("<H", record, 0)[0] == 0xE604
                and record[0x30] == 5
            ):
                return record
    for item in iter_dicts(payload):
        raw = item.get("record_hex")
        if not isinstance(raw, str):
            continue
        try:
            record = bytes.fromhex(raw)
        except ValueError:
            continue
        if (
            len(record) == SCROLL_RECORD_SIZE
            and struct.unpack_from("<H", record, 0)[0] == 0xE604
            and record[0x30] == 5
        ):
            return record
    raise ValueError(f"no canonical NG3 rarity-5 template found in {path}")


def deterministic_natural_seeds(count: int, rng_seed: int) -> tuple[int, ...]:
    if count <= 0:
        raise ValueError("count must be positive")
    anchors = (
        1,
        0x0FFFFFFF,
        241719428,
        82212268,
        144466760,
        183696634,
    )
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


def differing_offsets(actual: bytes, expected: bytes) -> list[int]:
    return [
        index
        for index, (actual_byte, expected_byte) in enumerate(
            zip(actual, expected, strict=True)
        )
        if actual_byte != expected_byte
    ]


def differing_bytes(actual: bytes, expected: bytes) -> list[dict[str, object]]:
    return [
        {
            "offset": index,
            "offset_hex": f"0x{index:02X}",
            "native": f"0x{actual_byte:02X}",
            "offline": f"0x{expected_byte:02X}",
        }
        for index, (actual_byte, expected_byte) in enumerate(
            zip(actual, expected, strict=True)
        )
        if actual_byte != expected_byte
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
    effect_slot_mismatches: list[dict[str, object]] = []
    full_record_mismatches: list[dict[str, object]] = []
    effect_slot_mismatch_count = 0
    full_record_mismatch_count = 0
    expected_header_cap_count = 0
    unexpected_record_mismatch_count = 0
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
                    rarity=5,
                    level=level,
                    recommended_level=recommended_level,
                    transfer_count=0,
                )
                for seed in batch_seeds
            ]
            native_records = oracle.generate(sources)
            for seed, native_record in zip(batch_seeds, native_records, strict=True):
                actual_seed = struct.unpack_from("<I", native_record, 0x20)[0]
                if actual_seed != seed:
                    raise RuntimeError(
                        f"native canonicalizer changed Seed {seed} to {actual_seed}"
                    )
                generation_serial = struct.unpack_from("<I", native_record, 0x28)[0]
                transfer_count = struct.unpack_from("<I", native_record, 0xDC)[0]
                offline_record, sequence = materialize_ng3_rarity5_record(
                    template,
                    seed=seed,
                    level=level,
                    recommended_level=recommended_level,
                    transfer_count=transfer_count,
                    generation_serial=generation_serial,
                )
                expected_effect_slots = serialize_ng3_rarity5_effect_slots(sequence)
                actual_effect_slots = native_record[
                    EFFECT_START : EFFECT_START + 7 * EFFECT_STRIDE
                ]
                if actual_effect_slots != expected_effect_slots:
                    effect_slot_mismatch_count += 1
                    if len(effect_slot_mismatches) < 32:
                        effect_slot_mismatches.append(
                            {
                                "seed": seed,
                                "seed_hex": f"0x{seed:08X}",
                                "differing_effect_region_offsets": differing_offsets(
                                    actual_effect_slots,
                                    expected_effect_slots,
                                ),
                            }
                        )
                record_differences = differing_offsets(native_record, offline_record)
                expected_v201_header_cap = (
                    oracle.runtime_profile.display_version == "PC v2.01"
                    and record_differences == [0x30, 0x31]
                    and native_record[0x30:0x32] == b"\x04\x04"
                    and offline_record[0x30:0x32] == b"\x05\x05"
                )
                if record_differences:
                    full_record_mismatch_count += 1
                    if expected_v201_header_cap:
                        expected_header_cap_count += 1
                    else:
                        unexpected_record_mismatch_count += 1
                    if len(full_record_mismatches) < 32:
                        full_record_mismatches.append(
                            {
                                "seed": seed,
                                "seed_hex": f"0x{seed:08X}",
                                "expected_v201_rarity_header_cap": (
                                    expected_v201_header_cap
                                ),
                                "differing_record_offsets": record_differences,
                                "differing_bytes": differing_bytes(
                                    native_record,
                                    offline_record,
                                ),
                            }
                        )
                validated += 1
            print(
                f"validated {validated:,} / {len(seeds):,}",
                file=sys.stderr,
            )

    return {
        "schema": "nioh3-ng3-rarity5-live-native-parity/v1",
        "game_version": oracle.runtime_profile.display_version,
        "process_id": process_id,
        "module_base": f"0x{module_base:016X}",
        "template_type": "0xE604",
        "rarity": 5,
        "level": level,
        "recommended_level": recommended_level,
        "seed_count": len(seeds),
        "seed_set_sha256": hashlib.sha256(
            b"".join(struct.pack("<I", seed) for seed in seeds)
        ).hexdigest(),
        "effect_slot_mismatch_count": effect_slot_mismatch_count,
        "full_record_mismatch_count": full_record_mismatch_count,
        "expected_v201_rarity_header_cap_count": expected_header_cap_count,
        "unexpected_record_mismatch_count": unexpected_record_mismatch_count,
        "effect_slot_mismatches": effect_slot_mismatches,
        "full_record_mismatches": full_record_mismatches,
        "effect_slot_parity_pass": effect_slot_mismatch_count == 0,
        "full_record_parity_pass": full_record_mismatch_count == 0,
        "semantic_record_parity_pass": (
            effect_slot_mismatch_count == 0
            and unexpected_record_mismatch_count == 0
        ),
        "known_version_difference": (
            "PC v2.01 caps requested rarity 5 to rarity 4 in the assembled "
            "record header when feature flag 9 is unavailable; generated "
            "effect slots remain identical."
            if expected_header_cap_count
            else None
        ),
        "save_access": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--count", type=int, default=10_000)
    parser.add_argument("--batch-size", type=int, default=1024)
    parser.add_argument("--rng-seed", type=int, default=0x4E494F48)
    parser.add_argument("--level", type=int, default=180)
    parser.add_argument("--recommended-level", type=int, default=183)
    parser.add_argument("--template-evidence", type=Path, default=DEFAULT_TEMPLATE_EVIDENCE)
    parser.add_argument("--runtime-profile", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    if not 1 <= args.batch_size <= 4096:
        parser.error("--batch-size must be in 1..4096")
    template = load_template(args.template_evidence)
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
    return (
        0
        if payload["effect_slot_parity_pass"]
        and payload["semantic_record_parity_pass"]
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())
