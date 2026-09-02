from __future__ import annotations

import argparse
import json
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Sequence

from native_xrefs import containing_runtime_function, parse_runtime_functions


@dataclass(frozen=True, slots=True)
class InstructionRecord:
    rva: str
    bytes_hex: str
    mnemonic: str
    operands: str


def _load_capstone(vendor_path: Path | None):
    if vendor_path is not None:
        sys.path.insert(0, str(vendor_path.resolve()))
    try:
        from capstone import CS_ARCH_X86, CS_MODE_64, Cs
    except ImportError as error:
        raise RuntimeError(
            "Capstone is required for this research helper; pass --vendor to its target directory"
        ) from error
    disassembler = Cs(CS_ARCH_X86, CS_MODE_64)
    disassembler.detail = False
    return disassembler


def disassemble_functions(
    text: bytes,
    pdata: bytes,
    *,
    text_rva: int,
    requested_rvas: Sequence[int],
    vendor_path: Path | None,
) -> dict[str, object]:
    functions = parse_runtime_functions(pdata)
    disassembler = _load_capstone(vendor_path)
    outputs: list[dict[str, object]] = []
    for requested_rva in requested_rvas:
        function = containing_runtime_function(functions, requested_rva)
        if function is None:
            raise ValueError(f"no pdata runtime function contains RVA 0x{requested_rva:X}")
        start = function.begin_rva - text_rva
        end = function.end_rva - text_rva
        if start < 0 or end > len(text):
            raise ValueError(f"runtime function 0x{function.begin_rva:X} is outside text dump")
        instructions = tuple(
            InstructionRecord(
                rva=f"0x{instruction.address:X}",
                bytes_hex=instruction.bytes.hex(" ").upper(),
                mnemonic=instruction.mnemonic,
                operands=instruction.op_str,
            )
            for instruction in disassembler.disasm(
                text[start:end],
                function.begin_rva,
            )
        )
        outputs.append(
            {
                "requested_rva": f"0x{requested_rva:X}",
                "begin_rva": f"0x{function.begin_rva:X}",
                "end_rva": f"0x{function.end_rva:X}",
                "unwind_rva": f"0x{function.unwind_rva:X}",
                "byte_size": function.end_rva - function.begin_rva,
                "instructions": [asdict(item) for item in instructions],
            }
        )
    return {
        "schema": "nioh3-static-function-disassembly/v1",
        "text_rva": f"0x{text_rva:X}",
        "functions": outputs,
    }


def _parse_int(value: str) -> int:
    return int(value, 0)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Disassemble pdata-bounded x64 functions")
    parser.add_argument("--text", type=Path, required=True)
    parser.add_argument("--pdata", type=Path, required=True)
    parser.add_argument("--text-rva", type=_parse_int, default=0x1000)
    parser.add_argument("--function", type=_parse_int, action="append", required=True)
    parser.add_argument("--vendor", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    report = disassemble_functions(
        args.text.read_bytes(),
        args.pdata.read_bytes(),
        text_rva=args.text_rva,
        requested_rvas=args.function,
        vendor_path=args.vendor,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    for function in report["functions"]:
        print(
            f"{function['begin_rva']}..{function['end_rva']} "
            f"({len(function['instructions'])} instructions)"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
