from __future__ import annotations

import argparse
import bisect
import json
import struct
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable, Sequence


@dataclass(frozen=True, slots=True)
class RuntimeFunction:
    begin_rva: int
    end_rva: int
    unwind_rva: int


@dataclass(frozen=True, slots=True)
class DirectCallXref:
    target_rva: str
    call_rva: str
    caller_begin_rva: str | None
    caller_end_rva: str | None
    caller_unwind_rva: str | None
    callsite_aob: str
    caller_entry_signature: str | None


def parse_runtime_functions(pdata: bytes) -> tuple[RuntimeFunction, ...]:
    functions: list[RuntimeFunction] = []
    for offset in range(0, len(pdata) - 11, 12):
        begin, end, unwind = struct.unpack_from("<III", pdata, offset)
        if begin == 0 and end == 0:
            continue
        if begin >= end:
            continue
        functions.append(RuntimeFunction(begin, end, unwind))
    functions.sort(key=lambda item: item.begin_rva)
    return tuple(functions)


def containing_runtime_function(
    functions: Sequence[RuntimeFunction], rva: int
) -> RuntimeFunction | None:
    starts = [item.begin_rva for item in functions]
    index = bisect.bisect_right(starts, rva) - 1
    if index < 0:
        return None
    candidate = functions[index]
    return candidate if rva < candidate.end_rva else None


def _aob_with_relative_call_wildcard(data: bytes, call_offset: int) -> str:
    start = max(0, call_offset - 8)
    end = min(len(data), call_offset + 5 + 12)
    rendered: list[str] = []
    for offset in range(start, end):
        if call_offset + 1 <= offset < call_offset + 5:
            rendered.append("??")
        else:
            rendered.append(f"{data[offset]:02X}")
    return " ".join(rendered)


def find_direct_call_xrefs(
    text: bytes,
    *,
    text_rva: int,
    target_rvas: Iterable[int],
    functions: Sequence[RuntimeFunction],
) -> tuple[DirectCallXref, ...]:
    targets = set(target_rvas)
    xrefs: list[DirectCallXref] = []
    cursor = 0
    while True:
        call_offset = text.find(b"\xE8", cursor)
        if call_offset < 0 or call_offset + 5 > len(text):
            break
        call_rva = text_rva + call_offset
        displacement = struct.unpack_from("<i", text, call_offset + 1)[0]
        target_rva = call_rva + 5 + displacement
        if target_rva in targets:
            caller = containing_runtime_function(functions, call_rva)
            entry_signature = None
            if caller is not None:
                entry_offset = caller.begin_rva - text_rva
                if 0 <= entry_offset < len(text):
                    entry_signature = text[entry_offset:entry_offset + 16].hex(" ").upper()
            xrefs.append(
                DirectCallXref(
                    target_rva=f"0x{target_rva:X}",
                    call_rva=f"0x{call_rva:X}",
                    caller_begin_rva=(f"0x{caller.begin_rva:X}" if caller else None),
                    caller_end_rva=(f"0x{caller.end_rva:X}" if caller else None),
                    caller_unwind_rva=(f"0x{caller.unwind_rva:X}" if caller else None),
                    callsite_aob=_aob_with_relative_call_wildcard(text, call_offset),
                    caller_entry_signature=entry_signature,
                )
            )
        cursor = call_offset + 1
    return tuple(xrefs)


def build_report(
    text_path: Path,
    pdata_path: Path,
    *,
    text_rva: int,
    targets: Sequence[int],
) -> dict[str, object]:
    text = text_path.read_bytes()
    pdata = pdata_path.read_bytes()
    functions = parse_runtime_functions(pdata)
    xrefs = find_direct_call_xrefs(
        text,
        text_rva=text_rva,
        target_rvas=targets,
        functions=functions,
    )
    return {
        "schema": "nioh3-static-direct-call-xrefs/v1",
        "text_rva": f"0x{text_rva:X}",
        "text_size": len(text),
        "pdata_size": len(pdata),
        "runtime_function_count": len(functions),
        "targets": [f"0x{target:X}" for target in targets],
        "xrefs": [asdict(item) for item in xrefs],
        "limitations": [
            "Only direct x64 E8 rel32 calls are included.",
            "Indirect calls, tail jumps, and data-driven dispatch require separate analysis.",
            "AOBs are discovery evidence and require validation in a second process launch.",
        ],
    }


def _parse_int(value: str) -> int:
    return int(value, 0)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Find direct call xrefs in a PE text dump")
    parser.add_argument("--text", type=Path, required=True)
    parser.add_argument("--pdata", type=Path, required=True)
    parser.add_argument("--text-rva", type=_parse_int, default=0x1000)
    parser.add_argument("--target", type=_parse_int, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    report = build_report(
        args.text,
        args.pdata,
        text_rva=args.text_rva,
        targets=args.target,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps({"xref_count": len(report["xrefs"])}))
    for xref in report["xrefs"]:
        print(
            f"{xref['target_rva']} <- {xref['call_rva']} "
            f"caller={xref['caller_begin_rva']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
