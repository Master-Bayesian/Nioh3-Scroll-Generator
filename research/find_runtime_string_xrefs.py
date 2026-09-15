"""Find RIP-relative string references with bounded function disassembly."""

from __future__ import annotations

import argparse
import bisect
import re
import struct
import sys
from pathlib import Path
from typing import Any


ROOT = Path("audit/runtime_sections/v2.00.02_20260827_title")
DEFAULT_TEXT = ROOT / "Nioh3_v2.00.02.text.bin"
DEFAULT_RDATA = ROOT / "Nioh3_v2.00.02.rdata.bin"
DEFAULT_PDATA = ROOT / "Nioh3_v2.00.02.pdata.bin"
DEFAULT_VENDOR = Path("audit/runtime_deps")
TEXT_RVA = 0x1000
RDATA_RVA = 0x38DA000
DEFAULT_MODULE_BASE = 0x7FF7A02A0000


def _parse_int(value: str) -> int:
    return int(value, 0)


def _runtime_functions(data: bytes) -> tuple[list[int], dict[int, tuple[int, int]]]:
    ranges: dict[int, tuple[int, int]] = {}
    for offset in range(0, len(data) - 11, 12):
        begin, end, _ = struct.unpack_from("<III", data, offset)
        if begin and end > begin:
            ranges[begin] = (begin, end)
    starts = sorted(ranges)
    return starts, ranges


def _containing_function(
    rva: int,
    starts: list[int],
    ranges: dict[int, tuple[int, int]],
) -> tuple[int, int] | None:
    index = bisect.bisect_right(starts, rva) - 1
    if index < 0:
        return None
    begin, end = ranges[starts[index]]
    return (begin, end) if rva < end else None


def _load_capstone(vendor_path: Path):
    sys.path.insert(0, str(vendor_path.resolve()))
    from capstone import CS_ARCH_X86, CS_MODE_64, Cs
    from capstone.x86 import X86_OP_MEM, X86_REG_RIP

    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    decoder.detail = True
    decoder.skipdata = True
    return decoder, X86_OP_MEM, X86_REG_RIP


_COMMON_RIP_MEMORY_PATTERN = re.compile(
    rb"(?:(?:\x66|\xF2|\xF3)?[\x40-\x4F]?)"
    rb"(?:\x8B|\x8D)"
    rb"[\x05\x0D\x15\x1D\x25\x2D\x35\x3D]"
    rb"(?P<displacement>.{4})",
    re.DOTALL,
)


def _raw_common_rip_relative_candidates(
    text: bytes,
    targets: set[int],
    text_rva: int = TEXT_RVA,
    max_candidates: int = 10000,
) -> list[tuple[int, int]]:
    """Prefilter common MOV/LEA RIP-relative references in raw bytes."""

    candidates: set[tuple[int, int]] = set()
    for match in _COMMON_RIP_MEMORY_PATTERN.finditer(text):
        displacement = struct.unpack("<i", match.group("displacement"))[0]
        target = text_rva + match.end() + displacement
        if target in targets:
            candidates.add((text_rva + match.start(), target))
            if len(candidates) > max_candidates:
                raise RuntimeError(
                    f"Refusing to exceed raw candidate limit ({max_candidates})."
                )
    return sorted(candidates)


def _validated_rip_relative_references(
    text: bytes,
    pdata: bytes,
    targets: set[int],
    text_rva: int = TEXT_RVA,
    max_function_bytes: int = 0x20000,
    max_total_function_bytes: int = 0x8000000,
    max_matches: int = 10000,
    vendor_path: Path = DEFAULT_VENDOR,
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    """Disassemble one bounded .pdata function at a time.

    Capstone's Python binding can retain extreme amounts of memory when a large
    game section is passed to one ``disasm`` call, especially with skip-data
    enabled. This scanner therefore never submits the complete section and
    refuses unbounded function coverage or output.
    """

    if max_function_bytes <= 0 or max_total_function_bytes <= 0 or max_matches <= 0:
        raise ValueError("resource limits must be positive")

    starts, ranges = _runtime_functions(pdata)
    raw_candidates = _raw_common_rip_relative_candidates(
        text,
        targets,
        text_rva,
        max_matches,
    )
    candidates_by_function: dict[tuple[int, int], set[tuple[int, int]]] = {}
    skipped_no_function = 0
    skipped_large_functions = 0
    skipped_outside_text = 0
    for rva, target in raw_candidates:
        function = _containing_function(rva, starts, ranges)
        if function is None:
            skipped_no_function += 1
            continue
        begin, end = function
        if end - begin > max_function_bytes:
            skipped_large_functions += 1
            continue
        if begin < text_rva or end > text_rva + len(text):
            skipped_outside_text += 1
            continue
        candidates_by_function.setdefault(function, set()).add((rva, target))

    scanned_bytes = sum(end - begin for begin, end in candidates_by_function)
    if scanned_bytes > max_total_function_bytes:
        raise RuntimeError(
            "Refusing to exceed --max-total-function-bytes while scanning "
            f"candidate .pdata functions (required: 0x{scanned_bytes:X})."
        )

    decoder, memory_operand, rip_register = _load_capstone(vendor_path)
    matches_by_key: dict[tuple[int, int], dict[str, Any]] = {}
    rejected_instruction_boundaries = 0
    for (begin, end), function_candidates in sorted(candidates_by_function.items()):
        block = text[begin - text_rva : end - text_rva]
        unresolved = set(function_candidates)
        for instruction in decoder.disasm(block, begin):
            if instruction.id == 0:
                continue
            instruction_candidates = {
                item for item in unresolved if item[0] == instruction.address
            }
            if not instruction_candidates:
                continue
            for operand in instruction.operands:
                if operand.type != memory_operand or operand.mem.base != rip_register:
                    continue
                target = instruction.address + instruction.size + operand.mem.disp
                key = (instruction.address, target)
                if key not in instruction_candidates:
                    continue
                unresolved.discard(key)
                matches_by_key[key] = {
                    "rva": instruction.address,
                    "bytes_hex": bytes(instruction.bytes).hex(" ").upper(),
                    "mnemonic": instruction.mnemonic,
                    "operands": instruction.op_str,
                    "target": target,
                    "function_begin": begin,
                    "function_end": end,
                }
                if len(matches_by_key) > max_matches:
                    raise RuntimeError(
                        f"Refusing to exceed --max-matches ({max_matches})."
                    )
            if not unresolved:
                break
        rejected_instruction_boundaries += len(unresolved)

    matches = list(matches_by_key.values())
    matches.sort(key=lambda item: (item["rva"], item["target"]))
    statistics = {
        "raw_candidates": len(raw_candidates),
        "scanned_functions": len(candidates_by_function),
        "scanned_bytes": scanned_bytes,
        "skipped_no_function": skipped_no_function,
        "skipped_large_functions": skipped_large_functions,
        "skipped_outside_text": skipped_outside_text,
        "rejected_instruction_boundaries": rejected_instruction_boundaries,
    }
    return matches, statistics


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("needle", nargs="+", help="ASCII string(s) to locate")
    parser.add_argument("--text", type=Path, default=DEFAULT_TEXT)
    parser.add_argument("--rdata", type=Path, default=DEFAULT_RDATA)
    parser.add_argument("--pdata", type=Path, default=DEFAULT_PDATA)
    parser.add_argument(
        "--text-rva",
        type=_parse_int,
        default=TEXT_RVA,
        help="RVA of the captured .text section",
    )
    parser.add_argument(
        "--rdata-rva",
        type=_parse_int,
        default=RDATA_RVA,
        help="RVA of the captured .rdata section",
    )
    parser.add_argument(
        "--module-base",
        type=_parse_int,
        default=DEFAULT_MODULE_BASE,
        help="Module base used when the runtime .rdata section was captured",
    )
    parser.add_argument("--max-function-bytes", type=_parse_int, default=0x20000)
    parser.add_argument("--max-total-function-bytes", type=_parse_int, default=0x8000000)
    parser.add_argument("--max-string-matches", type=int, default=10000)
    parser.add_argument("--max-matches", type=int, default=10000)
    parser.add_argument("--vendor-path", type=Path, default=DEFAULT_VENDOR)
    args = parser.parse_args()

    text = args.text.read_bytes()
    rdata = args.rdata.read_bytes()
    offsets: list[int] = []
    for needle_text in args.needle:
        needle = needle_text.encode("utf-8")
        cursor = 0
        while True:
            cursor = rdata.find(needle, cursor)
            if cursor < 0:
                break
            offsets.append(cursor)
            if len(offsets) > args.max_string_matches:
                raise RuntimeError(
                    "Refusing to exceed --max-string-matches "
                    f"({args.max_string_matches})."
                )
            cursor += 1
    if not offsets:
        print("No matching strings found.")
        return 1

    targets = {args.rdata_rva + offset: offset for offset in offsets}
    print("Strings:")
    for rva in sorted(targets):
        print(f"  RVA 0x{rva:X}")

    print("Data pointers:")
    data_pointer_count = 0
    for rva in sorted(targets):
        pointer = struct.pack("<Q", args.module_base + rva)
        cursor = 0
        while True:
            cursor = rdata.find(pointer, cursor)
            if cursor < 0:
                break
            pointer_rva = args.rdata_rva + cursor
            print(f"  RVA 0x{pointer_rva:X} -> string RVA 0x{rva:X}")
            data_pointer_count += 1
            cursor += 1
    if not data_pointer_count:
        print("  None")

    matches, statistics = _validated_rip_relative_references(
        text,
        args.pdata.read_bytes(),
        set(targets),
        args.text_rva,
        args.max_function_bytes,
        args.max_total_function_bytes,
        args.max_matches,
        args.vendor_path,
    )
    print("Code references:")
    for match in matches:
        print(
            f"  0x{match['rva']:X}: {match['mnemonic']} {match['operands']} "
            f"-> 0x{match['target']:X}; function "
            f"0x{match['function_begin']:X}..0x{match['function_end']:X}"
        )
    if not matches:
        print(
            "  No validated common MOV/LEA RIP-relative reference "
            "(the string may use another encoding or a data table)."
        )
    print(
        "Scan bounds: "
        f"{statistics['raw_candidates']} raw candidates / "
        f"{statistics['scanned_functions']} bounded functions / "
        f"0x{statistics['scanned_bytes']:X} bytes; "
        f"skipped no function: {statistics['skipped_no_function']}; "
        f"skipped large: {statistics['skipped_large_functions']}; "
        f"skipped outside .text: {statistics['skipped_outside_text']}; "
        f"rejected boundaries: {statistics['rejected_instruction_boundaries']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
