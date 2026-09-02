"""Search a decrypted save for direct fixed-stride auxiliary companion tables.

This is a correlation probe, not a proof that every encoded/indexed layout is
absent. It compares independently generated terrain, first-rule, and first-
enemy values across multiple occupied physical scroll slots.
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.auxiliary_generation import generate_complete_auxiliary
from nioh3_scroll_editor.savegame import (
    SaveCrypto,
    SaveInventory,
    default_crypto_tool,
    read_local_scroll_header,
)


@dataclass(frozen=True, slots=True)
class Sample:
    slot_index: int
    ordinal: int
    terrain: int
    first_rule: int
    first_enemy: int


@dataclass(frozen=True, slots=True)
class FieldSpec:
    name: str
    width: int
    value: Callable[[Sample], int]

    def encode(self, sample: Sample) -> bytes:
        return self.value(sample).to_bytes(self.width, "little")


FIELDS = (
    FieldSpec("terrain_u8", 1, lambda sample: sample.terrain),
    FieldSpec("first_rule_u16", 2, lambda sample: sample.first_rule),
    FieldSpec("first_enemy_u32", 4, lambda sample: sample.first_enemy),
)


def find_all(data: bytes, needle: bytes) -> list[int]:
    positions: list[int] = []
    start = 0
    while True:
        position = data.find(needle, start)
        if position < 0:
            return positions
        positions.append(position)
        start = position + 1


def load_samples(save_path: Path, project_root: Path) -> tuple[bytes, list[Sample]]:
    crypto = SaveCrypto(default_crypto_tool(project_root))
    with tempfile.TemporaryDirectory(prefix="nioh3-aux-companion-") as directory:
        decrypted_path = Path(directory) / "decrypted.bin"
        crypto.decrypt(save_path, decrypted_path)
        decrypted = decrypted_path.read_bytes()

    inventory = SaveInventory.load(save_path, decrypted)
    samples: list[Sample] = []
    for ordinal, entry in enumerate(inventory.scroll_entries()):
        header = read_local_scroll_header(entry.record)
        try:
            auxiliary = generate_complete_auxiliary(header.seed, header.playthrough)
        except Exception:
            continue
        samples.append(
            Sample(
                slot_index=entry.slot_index,
                ordinal=ordinal,
                terrain=auxiliary.terrain.value,
                first_rule=auxiliary.special_rules.keys[0],
                first_enemy=auxiliary.enemies.groups[0].entries[0].lookup_key,
            )
        )
    return decrypted, samples


def contiguous_matches(data: bytes, samples: list[Sample], field: FieldSpec) -> list[int]:
    encoded = b"".join(field.encode(sample) for sample in samples)
    return find_all(data, encoded)


def fixed_stride_matches(
    data: bytes,
    samples: list[Sample],
    field: FieldSpec,
    *,
    index_attribute: str,
    maximum_stride: int,
) -> list[tuple[int, int]]:
    if not samples:
        return []
    occurrences_by_sample = {
        sample: find_all(data, field.encode(sample)) for sample in samples
    }
    anchor = min(samples, key=lambda sample: len(occurrences_by_sample[sample]))
    anchor_positions = occurrences_by_sample[anchor]
    if not anchor_positions:
        return []

    ordered_checks = sorted(
        (sample for sample in samples if sample != anchor),
        key=lambda sample: len(occurrences_by_sample[sample]),
    )
    matches: list[tuple[int, int]] = []
    anchor_index = int(getattr(anchor, index_attribute))
    for stride in range(field.width, maximum_stride + 1):
        for anchor_position in anchor_positions:
            base = anchor_position - anchor_index * stride
            if base < 0:
                continue
            for sample in ordered_checks:
                index = int(getattr(sample, index_attribute))
                position = base + index * stride
                expected = field.encode(sample)
                if data[position:position + field.width] != expected:
                    break
            else:
                matches.append((base, stride))
    return matches


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--save", type=Path, required=True)
    parser.add_argument(
        "--scan-blob",
        type=Path,
        help=(
            "Optional plaintext blob to scan instead of the decrypted character "
            "save. The character save is still used to enumerate scroll samples."
        ),
    )
    parser.add_argument("--project-root", type=Path, default=PROJECT_ROOT)
    parser.add_argument("--maximum-stride", type=int, default=0x400)
    args = parser.parse_args()

    decrypted, samples = load_samples(args.save.resolve(), args.project_root.resolve())
    if args.scan_blob is None:
        scan_path = args.save.resolve()
        scan_data = decrypted
        scan_kind = "decrypted_character_save"
    else:
        scan_path = args.scan_blob.resolve()
        scan_data = scan_path.read_bytes()
        scan_kind = "external_plaintext_blob"
    print(
        f"scan_kind={scan_kind} scan_path={scan_path} "
        f"scan_size={len(scan_data)} samples={len(samples)}"
    )
    for sample in samples:
        print(
            f"slot={sample.slot_index:03d} ordinal={sample.ordinal:03d} "
            f"terrain=0x{sample.terrain:02X} rule=0x{sample.first_rule:04X} "
            f"enemy=0x{sample.first_enemy:08X}"
        )

    for field in FIELDS:
        contiguous = contiguous_matches(scan_data, samples, field)
        print(
            f"{field.name} dense_contiguous_matches="
            f"{[hex(position) for position in contiguous]}"
        )
        for index_attribute in ("slot_index", "ordinal"):
            matches = fixed_stride_matches(
                scan_data,
                samples,
                field,
                index_attribute=index_attribute,
                maximum_stride=args.maximum_stride,
            )
            print(
                f"{field.name} {index_attribute} fixed_stride_matches="
                f"{[(hex(base), hex(stride)) for base, stride in matches[:64]]} "
                f"total={len(matches)}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
