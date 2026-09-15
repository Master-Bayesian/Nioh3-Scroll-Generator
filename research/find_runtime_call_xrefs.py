"""Find validated direct-call references without whole-section disassembly."""

from __future__ import annotations

import argparse
import bisect
import struct
import sys
from pathlib import Path
from typing import Any, Iterable

ROOT = Path("audit/runtime_sections/v2.00.02_20260827_title")
DEFAULT_TEXT = ROOT / "Nioh3_v2.00.02.text.bin"
DEFAULT_PDATA = ROOT / "Nioh3_v2.00.02.pdata.bin"
DEFAULT_VENDOR = Path("audit/runtime_deps")
TEXT_RVA = 0x1000


def parse_int(value: str) -> int:
    return int(value, 0)


def runtime_functions(data: bytes) -> tuple[list[int], dict[int, tuple[int, int]]]:
    ranges: dict[int, tuple[int, int]] = {}
    for offset in range(0, len(data) - 11, 12):
        begin, end, _ = struct.unpack_from("<III", data, offset)
        if begin and end > begin:
            ranges[begin] = (begin, end)
    starts = sorted(ranges)
    return starts, ranges


def containing_function(
    rva: int,
    starts: list[int],
    ranges: dict[int, tuple[int, int]],
) -> tuple[int, int] | None:
    index = bisect.bisect_right(starts, rva) - 1
    if index < 0:
        return None
    begin, end = ranges[starts[index]]
    return (begin, end) if rva < end else None


def direct_rel32_candidate_rvas(
    text: bytes,
    target: int,
    text_rva: int = TEXT_RVA,
) -> Iterable[int]:
    """Yield raw E8-rel32 positions that arithmetically target ``target``."""

    cursor = 0
    while True:
        cursor = text.find(b"\xE8", cursor)
        if cursor < 0 or cursor + 5 > len(text):
            return
        displacement = struct.unpack_from("<i", text, cursor + 1)[0]
        instruction_rva = text_rva + cursor
        if instruction_rva + 5 + displacement == target:
            yield instruction_rva
        cursor += 1


def load_capstone(vendor_path: Path):
    sys.path.insert(0, str(vendor_path.resolve()))
    from capstone import CS_ARCH_X86, CS_MODE_64, Cs
    from capstone.x86 import X86_OP_IMM

    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    decoder.detail = True
    return decoder, X86_OP_IMM


def validated_direct_calls(
    text: bytes,
    pdata: bytes,
    target: int,
    text_rva: int = TEXT_RVA,
    max_function_bytes: int = 0x20000,
    vendor_path: Path = DEFAULT_VENDOR,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], int]:
    """Validate raw candidates only within bounded .pdata function ranges."""

    starts, ranges = runtime_functions(pdata)
    raw_candidates = list(direct_rel32_candidate_rvas(text, target, text_rva))
    candidates_by_function: dict[tuple[int, int], set[int]] = {}
    skipped: list[dict[str, Any]] = []
    for rva in raw_candidates:
        function = containing_function(rva, starts, ranges)
        if function is None:
            skipped.append({"rva": f"0x{rva:X}", "reason": "no pdata function"})
            continue
        if function[1] - function[0] > max_function_bytes:
            skipped.append({
                "rva": f"0x{rva:X}",
                "reason": "containing function exceeds byte limit",
                "function_begin": f"0x{function[0]:X}",
                "function_end": f"0x{function[1]:X}",
            })
            continue
        candidates_by_function.setdefault(function, set()).add(rva)

    decoder, immediate_operand = load_capstone(vendor_path)
    matches: list[dict[str, Any]] = []
    for (begin, end), candidate_rvas in sorted(candidates_by_function.items()):
        block = text[begin - text_rva : end - text_rva]
        unresolved = set(candidate_rvas)
        for instruction in decoder.disasm(block, begin):
            if instruction.address not in unresolved:
                continue
            unresolved.remove(instruction.address)
            if (
                instruction.mnemonic == "call"
                and instruction.operands
                and instruction.operands[0].type == immediate_operand
                and instruction.operands[0].imm == target
            ):
                matches.append({
                    "rva": instruction.address,
                    "bytes_hex": bytes(instruction.bytes).hex(" ").upper(),
                    "mnemonic": instruction.mnemonic,
                    "operands": instruction.op_str,
                    "function_begin": begin,
                    "function_end": end,
                })
            if not unresolved:
                break
        for rva in sorted(unresolved):
            skipped.append({
                "rva": f"0x{rva:X}",
                "reason": "raw rel32 candidate is not a decoded instruction boundary",
                "function_begin": f"0x{begin:X}",
                "function_end": f"0x{end:X}",
            })
    matches.sort(key=lambda item: item["rva"])
    return matches, skipped, len(raw_candidates)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("target", type=parse_int, help="Target function RVA")
    parser.add_argument("--text", type=Path, default=DEFAULT_TEXT)
    parser.add_argument("--pdata", type=Path, default=DEFAULT_PDATA)
    parser.add_argument("--text-rva", type=parse_int, default=TEXT_RVA)
    parser.add_argument("--max-function-bytes", type=parse_int, default=0x20000)
    parser.add_argument("--vendor-path", type=Path, default=DEFAULT_VENDOR)
    args = parser.parse_args()

    matches, skipped, raw_count = validated_direct_calls(
        args.text.read_bytes(),
        args.pdata.read_bytes(),
        args.target,
        args.text_rva,
        args.max_function_bytes,
        args.vendor_path,
    )
    for match in matches:
        print(
            f"0x{match['rva']:X}: {match['mnemonic']} {match['operands']}; "
            f"caller 0x{match['function_begin']:X}..0x{match['function_end']:X}"
        )
    print(
        f"Raw rel32 candidates: {raw_count}; validated direct calls: {len(matches)}; "
        f"skipped or rejected: {len(skipped)}"
    )
    for item in skipped:
        print(f"SKIP {item}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
