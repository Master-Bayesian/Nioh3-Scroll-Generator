from __future__ import annotations

"""Live parity gate for the game-closed NG3 rarity-3 generator.

The native generator runs only in isolated remote buffers. The script never
reads, decrypts, modifies, or installs a save.
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

from nioh3_scroll_editor.effect_sequence import (  # noqa: E402
    materialize_ng3_rarity3_record,
)
from nioh3_scroll_editor.native import NativeBatchOracle, build_source_record  # noqa: E402
from nioh3_scroll_editor.native import (  # noqa: E402
    NativeRuntimeProfile,
    load_native_runtime_profile,
)
from nioh3_seed_math import is_natural_scroll_id  # noqa: E402


DEFAULT_TEMPLATE = (
    PROJECT_ROOT
    / "audit"
    / "effect_mapping"
    / "babd_manual_capture_20260827"
    / "seed-6096970-before-record.bin"
)

# The native isolated assembly path clears this gameplay/runtime header byte.
# Template-bound saved records preserve it.  It is not part of deterministic
# generation parity, unlike every effect byte and the canonical tuple fields.
RUNTIME_HEADER_OFFSETS = frozenset((0x1B,))


def deterministic_natural_seeds(count: int, rng_seed: int) -> tuple[int, ...]:
    if count <= 0:
        raise ValueError("count must be positive")
    seeds: list[int] = []
    seen: set[int] = set()
    for seed in (1, 6096970, 183696634, 0x0FFFFFFF):
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
    mismatch_count = 0
    runtime_header_mismatch_count = 0
    mismatch_samples: list[dict[str, object]] = []
    token_histogram: dict[str, int] = {}
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
                    rarity=3,
                    level=level,
                    recommended_level=recommended_level,
                )
                for seed in batch_seeds
            ]
            native_records = oracle.generate(sources)
            for seed, native_record in zip(batch_seeds, native_records, strict=True):
                generation_serial = struct.unpack_from("<I", native_record, 0x28)[0]
                transfer_count = struct.unpack_from("<I", native_record, 0xDC)[0]
                offline_record, _ = materialize_ng3_rarity3_record(
                    template,
                    seed=seed,
                    level=level,
                    recommended_level=recommended_level,
                    transfer_count=transfer_count,
                    generation_serial=generation_serial,
                )
                all_offsets = differences(native_record, offline_record)
                runtime_offsets = [
                    offset for offset in all_offsets if offset in RUNTIME_HEADER_OFFSETS
                ]
                offsets = [
                    offset for offset in all_offsets if offset not in RUNTIME_HEADER_OFFSETS
                ]
                if runtime_offsets:
                    runtime_header_mismatch_count += 1
                if offsets:
                    mismatch_count += 1
                    if len(mismatch_samples) < 32:
                        sample: dict[str, object] = {
                            "seed": seed,
                            "seed_hex": f"0x{seed:08X}",
                            "differing_offsets": offsets,
                            "differing_bytes": {
                                f"0x{offset:02X}": {
                                    "native": f"0x{native_record[offset]:02X}",
                                    "offline": f"0x{offline_record[offset]:02X}",
                                }
                                for offset in offsets
                            },
                        }
                        if any(offset >= 0x34 for offset in offsets):
                            sample["native_record_hex"] = native_record.hex().upper()
                            sample["offline_record_hex"] = offline_record.hex().upper()
                        mismatch_samples.append(sample)
                slot5_id = struct.unpack_from("<I", native_record, 0x98)[0]
                token = f"0x{slot5_id:08X}"
                token_histogram[token] = token_histogram.get(token, 0) + 1
                validated += 1
            print(f"validated {validated:,} / {len(seeds):,}", file=sys.stderr)

    return {
        "schema": "nioh3-ng3-rarity3-live-native-parity/v1",
        "game_version": oracle.runtime_profile.display_version,
        "process_id": process_id,
        "module_base": f"0x{module_base:016X}",
        "record_type": "0xE604",
        "playthrough": 3,
        "rarity": 3,
        "level": level,
        "recommended_level": recommended_level,
        "seed_count": len(seeds),
        "seed_set_sha256": hashlib.sha256(
            b"".join(struct.pack("<I", seed) for seed in seeds)
        ).hexdigest(),
        "slot5_token_histogram": token_histogram,
        "full_record_mismatch_count": mismatch_count,
        "runtime_header_mismatch_count": runtime_header_mismatch_count,
        "ignored_runtime_header_offsets": [
            f"0x{offset:02X}" for offset in sorted(RUNTIME_HEADER_OFFSETS)
        ],
        "mismatch_samples": mismatch_samples,
        "full_record_parity_pass": mismatch_count == 0,
        "save_access": "none",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--count", type=int, default=10_000)
    parser.add_argument("--batch-size", type=int, default=2048)
    parser.add_argument("--rng-seed", type=int, default=0x52334649)
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
    payload = validate(
        template=template,
        seeds=deterministic_natural_seeds(args.count, args.rng_seed),
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
    return 0 if payload["full_record_parity_pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
