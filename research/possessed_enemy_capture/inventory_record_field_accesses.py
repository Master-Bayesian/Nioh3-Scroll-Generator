"""Inventory exact x64 memory accesses to a structure displacement.

This is a static candidate inventory, not a semantic xref. An instruction that
accesses ``[register + displacement]`` may operate on an unrelated structure or
on stack-local storage. Runtime identity evidence is still required.
"""

from __future__ import annotations

import argparse
import bisect
import hashlib
import json
import struct
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SECTION_DIR = ROOT / "audit/runtime_sections/v2.0.1.0_20260902_title"
DEFAULT_TEXT = DEFAULT_SECTION_DIR / "Nioh3_v2.0.1.0.text.bin"
DEFAULT_PDATA = DEFAULT_SECTION_DIR / "Nioh3_v2.0.1.0.pdata.bin"
DEFAULT_VENDOR = ROOT / "audit/runtime_deps"


def parse_int(value: str) -> int:
    return int(value, 0)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def runtime_functions(data: bytes) -> tuple[list[int], dict[int, tuple[int, int]]]:
    ranges: dict[int, tuple[int, int]] = {}
    for offset in range(0, len(data) - 11, 12):
        begin, end, _ = struct.unpack_from("<III", data, offset)
        if begin and end > begin:
            ranges[begin] = (begin, end)
    return sorted(ranges), ranges


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


def literal_positions(data: bytes, displacement: int) -> list[int]:
    needle = struct.pack("<i", displacement)
    positions: list[int] = []
    cursor = 0
    while True:
        cursor = data.find(needle, cursor)
        if cursor < 0:
            return positions
        positions.append(cursor)
        cursor += 1


def load_capstone(vendor_path: Path):
    sys.path.insert(0, str(vendor_path.resolve()))
    from capstone import CS_AC_READ, CS_AC_WRITE, CS_ARCH_X86, CS_MODE_64, Cs
    from capstone.x86 import X86_OP_MEM

    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    decoder.detail = True
    return decoder, X86_OP_MEM, CS_AC_READ, CS_AC_WRITE


def access_names(access: int, read_flag: int, write_flag: int) -> list[str]:
    names: list[str] = []
    if access & read_flag:
        names.append("read")
    if access & write_flag:
        names.append("write")
    return names or ["unspecified"]


def instruction_entry(
    instruction: Any,
    operand: Any,
    function: tuple[int, int] | None,
    read_flag: int,
    write_flag: int,
    source: str,
) -> dict[str, Any]:
    return {
        "rva": f"0x{instruction.address:X}",
        "bytes_hex": bytes(instruction.bytes).hex(" ").upper(),
        "mnemonic": instruction.mnemonic,
        "operands": instruction.op_str,
        "memory_operand_size": operand.size,
        "base_register": instruction.reg_name(operand.mem.base),
        "index_register": instruction.reg_name(operand.mem.index),
        "access": access_names(operand.access, read_flag, write_flag),
        "function_begin_rva": f"0x{function[0]:X}" if function else None,
        "function_end_rva": f"0x{function[1]:X}" if function else None,
        "validation_source": source,
    }


def inventory(
    text: bytes,
    pdata: bytes,
    text_rva: int,
    displacement: int,
    vendor_path: Path,
) -> dict[str, Any]:
    decoder, memory_operand, read_flag, write_flag = load_capstone(vendor_path)
    starts, ranges = runtime_functions(pdata)
    positions = literal_positions(text, displacement)
    candidate_functions = {
        function
        for position in positions
        if (function := containing_function(text_rva + position, starts, ranges)) is not None
    }
    entries: list[dict[str, Any]] = []
    seen: set[tuple[int, int]] = set()
    for function in sorted(candidate_functions):
        begin, end = function
        block = text[begin - text_rva : end - text_rva]
        for instruction in decoder.disasm(block, begin):
            for operand_index, operand in enumerate(instruction.operands):
                if operand.type != memory_operand or operand.mem.disp != displacement:
                    continue
                key = (instruction.address, operand_index)
                if key in seen:
                    continue
                seen.add(key)
                entries.append(
                    instruction_entry(
                        instruction,
                        operand,
                        function,
                        read_flag,
                        write_flag,
                        "pdata function boundary",
                    )
                )

    # Tiny setters without unwind data are not represented in .pdata. Only add
    # the exact, independently decodable form ``mov byte ptr [rcx+disp],1; ret``.
    leaf_pattern = b"\xC6\x81" + struct.pack("<i", displacement) + b"\x01\xC3"
    cursor = 0
    while True:
        cursor = text.find(leaf_pattern, cursor)
        if cursor < 0:
            break
        address = text_rva + cursor
        if containing_function(address, starts, ranges) is None:
            decoded = list(decoder.disasm(text[cursor : cursor + len(leaf_pattern)], address))
            if len(decoded) == 2 and decoded[0].address == address and decoded[1].mnemonic == "ret":
                operand = decoded[0].operands[0]
                key = (address, 0)
                if key not in seen:
                    seen.add(key)
                    entries.append(
                        instruction_entry(
                            decoded[0],
                            operand,
                            (address, address + len(leaf_pattern)),
                            read_flag,
                            write_flag,
                            "exact unwindless leaf setter pattern",
                        )
                    )
        cursor += 1

    entries.sort(key=lambda item: int(item["rva"], 16))
    writes = [entry for entry in entries if "write" in entry["access"]]
    reads = [entry for entry in entries if "read" in entry["access"]]
    return {
        "schema": "nioh3-static-record-field-access-inventory/v1",
        "text_rva": f"0x{text_rva:X}",
        "field_displacement": f"0x{displacement:X}",
        "raw_literal_occurrence_count": len(positions),
        "candidate_pdata_function_count": len(candidate_functions),
        "validated_access_count": len(entries),
        "validated_write_count": len(writes),
        "validated_read_count": len(reads),
        "entries": entries,
        "limitations": [
            "An exact displacement match does not identify the base object's type.",
            "Stack-local and unrelated-structure accesses remain in the inventory.",
            "The unwindless scan recognizes only one exact leaf-setter form.",
            "Indirect calls and runtime object identity require separate evidence.",
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--text", type=Path, default=DEFAULT_TEXT)
    parser.add_argument("--pdata", type=Path, default=DEFAULT_PDATA)
    parser.add_argument("--text-rva", type=parse_int, default=0x1000)
    parser.add_argument("--field-offset", type=parse_int, required=True)
    parser.add_argument("--vendor-path", type=Path, default=DEFAULT_VENDOR)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite static evidence: {output}")
    text = args.text.read_bytes()
    pdata = args.pdata.read_bytes()
    result = inventory(text, pdata, args.text_rva, args.field_offset, args.vendor_path)
    result["inputs"] = {
        "text": str(args.text.resolve()),
        "text_size": len(text),
        "text_sha256": sha256(args.text),
        "pdata": str(args.pdata.resolve()),
        "pdata_size": len(pdata),
        "pdata_sha256": sha256(args.pdata),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "output": str(output),
        "validated_access_count": result["validated_access_count"],
        "validated_write_count": result["validated_write_count"],
    }))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
