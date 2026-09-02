from __future__ import annotations

"""Find RIP-relative references to selected RVAs in a live PE section dump."""

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from native_xrefs import parse_runtime_functions


def parse_int(value: str) -> int:
    return int(value, 0)


def load_capstone(vendor_path: Path | None):
    if vendor_path is not None:
        sys.path.insert(0, str(vendor_path.resolve()))
    try:
        from capstone import CS_ARCH_X86, CS_MODE_64, Cs
        from capstone.x86_const import X86_OP_MEM, X86_REG_RIP
    except ImportError as error:
        raise RuntimeError(
            "Capstone is required; pass --vendor to its package directory"
        ) from error
    disassembler = Cs(CS_ARCH_X86, CS_MODE_64)
    disassembler.detail = True
    return disassembler, X86_OP_MEM, X86_REG_RIP


def find_xrefs(
    text: bytes,
    pdata: bytes,
    *,
    text_rva: int,
    targets: Sequence[int],
    vendor_path: Path | None,
) -> dict[str, object]:
    disassembler, memory_operand, rip_register = load_capstone(vendor_path)
    target_set = set(targets)
    matches: dict[int, list[dict[str, str]]] = {target: [] for target in targets}
    functions_scanned = 0
    instructions_scanned = 0
    for function in parse_runtime_functions(pdata):
        begin = function.begin_rva - text_rva
        end = function.end_rva - text_rva
        if begin < 0 or end > len(text):
            continue
        functions_scanned += 1
        for instruction in disassembler.disasm(
            text[begin:end], function.begin_rva
        ):
            instructions_scanned += 1
            for operand in instruction.operands:
                if operand.type != memory_operand or operand.mem.base != rip_register:
                    continue
                target = instruction.address + instruction.size + operand.mem.disp
                if target not in target_set:
                    continue
                matches[target].append(
                    {
                        "function_begin_rva": f"0x{function.begin_rva:X}",
                        "instruction_rva": f"0x{instruction.address:X}",
                        "bytes": instruction.bytes.hex(" ").upper(),
                        "mnemonic": instruction.mnemonic,
                        "operands": instruction.op_str,
                    }
                )
    return {
        "schema": "nioh3-runtime-data-xrefs/v1",
        "text_rva": f"0x{text_rva:X}",
        "functions_scanned": functions_scanned,
        "instructions_scanned": instructions_scanned,
        "targets": {
            f"0x{target:X}": matches[target]
            for target in targets
        },
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Find RIP-relative code references to selected data RVAs"
    )
    parser.add_argument("--text", type=Path, required=True)
    parser.add_argument("--pdata", type=Path, required=True)
    parser.add_argument("--text-rva", type=parse_int, default=0x1000)
    parser.add_argument("--target", type=parse_int, action="append", required=True)
    parser.add_argument("--vendor", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    report = find_xrefs(
        args.text.read_bytes(),
        args.pdata.read_bytes(),
        text_rva=args.text_rva,
        targets=args.target,
        vendor_path=args.vendor,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "output": str(args.output.resolve()),
                "match_counts": {
                    target: len(matches)
                    for target, matches in report["targets"].items()
                },
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
