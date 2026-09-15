"""Find possible relocated function pointers in a captured runtime section."""

from __future__ import annotations

import argparse
import json
import struct
from collections import Counter, defaultdict
from pathlib import Path
from typing import Iterable


def parse_int(value: str) -> int:
    return int(value, 0)


def find_pointer_xrefs(
    data: bytes,
    targets: Iterable[int],
    *,
    data_rva: int,
    pointer_alignment: int = 8,
    module_alignment: int = 0x10000,
    max_matches: int = 10000,
) -> dict[str, object]:
    """Find qwords whose subtraction from a target yields an aligned image base.

    Runtime `.rdata` captures contain loader-relocated absolute pointers, while
    an older capture manifest may omit its process module base. The image base
    can therefore be inferred per candidate, but a match remains only a possible
    pointer until its surrounding table and consumers are typed.
    """

    if pointer_alignment <= 0 or module_alignment <= 0 or max_matches <= 0:
        raise ValueError("scan limits and alignments must be positive")
    target_values = sorted(set(targets))
    targets_by_low: dict[int, list[int]] = defaultdict(list)
    for target in target_values:
        targets_by_low[target % module_alignment].append(target)

    matches: list[dict[str, str]] = []
    base_counts: Counter[int] = Counter()
    for offset in range(0, len(data) - 7, pointer_alignment):
        value = struct.unpack_from("<Q", data, offset)[0]
        for target in targets_by_low.get(value % module_alignment, ()):
            base = value - target
            if base < 0x100000000 or base > 0x0000FFFFFFFF0000:
                continue
            if base % module_alignment:
                continue
            matches.append({
                "target_rva": f"0x{target:X}",
                "data_offset": f"0x{offset:X}",
                "data_rva": f"0x{data_rva + offset:X}",
                "pointer_value": f"0x{value:X}",
                "inferred_module_base": f"0x{base:X}",
            })
            base_counts[base] += 1
            if len(matches) > max_matches:
                raise RuntimeError(f"Refusing to exceed match limit ({max_matches})")

    return {
        "schema": "nioh3-runtime-relocated-pointer-xrefs/v1",
        "data_rva": f"0x{data_rva:X}",
        "data_size": len(data),
        "pointer_alignment": pointer_alignment,
        "module_alignment": f"0x{module_alignment:X}",
        "targets": [f"0x{target:X}" for target in target_values],
        "inferred_module_base_counts": {
            f"0x{base:X}": count for base, count in base_counts.most_common()
        },
        "matches": matches,
        "limitations": [
            "Matches are aligned absolute-pointer candidates, not typed xrefs.",
            "Unaligned pointers, encoded pointers, and pointers outside this section are not found.",
            "An inferred module base requires corroboration from surrounding table structure or another known pointer.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--data-rva", type=parse_int, required=True)
    parser.add_argument("--target", type=parse_int, action="append", required=True)
    parser.add_argument("--pointer-alignment", type=parse_int, default=8)
    parser.add_argument("--module-alignment", type=parse_int, default=0x10000)
    parser.add_argument("--max-matches", type=int, default=10000)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    report = find_pointer_xrefs(
        args.data.read_bytes(),
        args.target,
        data_rva=args.data_rva,
        pointer_alignment=args.pointer_alignment,
        module_alignment=args.module_alignment,
        max_matches=args.max_matches,
    )
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite existing evidence: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "output": str(output),
        "match_count": len(report["matches"]),
        "inferred_module_base_counts": report["inferred_module_base_counts"],
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
