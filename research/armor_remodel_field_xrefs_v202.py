"""Bounded offline inventory of PC v2.02 equipment-field read candidates.

This helper searches private ``.text``/``.pdata`` section dumps captured from
the PC 2.0.2.0 image.  A raw ModRM/displacement prefilter selects only pdata
functions that might contain a memory operand at the requested displacement;
Capstone then validates the instruction boundary, operand width, and read
access.  The result is an evidence inventory, not a semantic getter claim.

The default target fields are equipment-record ``+0x31`` and ``+0x32``.
Optional record-neighbour fields ``+0x30``/``+0x33`` and static-row ``+0x98``
are separately labelled context so a caller can rank nearby code without
silently treating it as an equipment-record read.
"""

from __future__ import annotations

import argparse
import bisect
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import struct
import sys
from typing import Any, Iterable, Sequence


TEXT_RVA = 0x1000
DEFAULT_TARGET_OFFSETS = (0x31, 0x32)
# Optional context is available through the CLI/API, but the default run is
# deliberately limited to the two requested equipment-record fields.  A
# nearby/context field can otherwise dominate a raw disp8 scan before the
# target fields have been covered.
DEFAULT_NEARBY_RECORD_OFFSETS: tuple[int, ...] = ()
DEFAULT_CONTEXT_OFFSETS: tuple[int, ...] = ()
DEFAULT_MAX_RAW_CANDIDATES = 500_000
DEFAULT_MAX_FUNCTION_BYTES = 0x20_000
DEFAULT_MAX_TOTAL_DECODED_BYTES = 0x8_000_000
DEFAULT_MAX_OUTPUT_COUNT = 10_000
DEFAULT_SKIP_EXAMPLES = 32


@dataclass(frozen=True, slots=True)
class RuntimeFunction:
    begin_rva: int
    end_rva: int
    unwind_rva: int


@dataclass(frozen=True, slots=True)
class RawFieldCandidate:
    """A displacement-shaped byte sequence accepted by the raw prefilter."""

    displacement_rva: int
    field_offset: int
    encoding: str
    modrm_rva: int


def parse_int(value: str) -> int:
    return int(value, 0)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def parse_runtime_functions(data: bytes) -> tuple[RuntimeFunction, ...]:
    """Parse a bounded exception-directory section dump.

    The section is a sequence of 12-byte ``RUNTIME_FUNCTION`` rows.  A
    zero-filled suffix is permitted because a section dump can include PE
    alignment padding; a non-zero row after that suffix is rejected instead
    of being treated as an inferred function boundary.
    """

    if not data or len(data) % 12:
        raise ValueError(".pdata size must be a non-empty multiple of 12 bytes")

    functions: list[RuntimeFunction] = []
    previous_begin = -1
    zero_suffix_at: int | None = None
    for offset in range(0, len(data), 12):
        begin, end, unwind = struct.unpack_from("<III", data, offset)
        if begin == end == unwind == 0:
            zero_suffix_at = offset
            break
        if begin == 0 or end <= begin:
            raise ValueError(f"invalid .pdata row at byte 0x{offset:X}")
        if begin <= previous_begin:
            raise ValueError(f"unsorted .pdata row at byte 0x{offset:X}")
        functions.append(RuntimeFunction(begin, end, unwind))
        previous_begin = begin

    if zero_suffix_at is not None and any(data[zero_suffix_at:]):
        raise ValueError("non-zero bytes follow zero-filled .pdata suffix")
    if not functions:
        raise ValueError(".pdata contains no runtime functions")
    return tuple(functions)


def containing_function(
    functions: Sequence[RuntimeFunction],
    rva: int,
) -> RuntimeFunction | None:
    """Return the pdata function containing ``rva`` when one exists."""

    starts = [item.begin_rva for item in functions]
    index = bisect.bisect_right(starts, rva) - 1
    if index < 0:
        return None
    function = functions[index]
    return function if rva < function.end_rva else None


def _iter_occurrences(data: bytes, needle: bytes) -> Iterable[int]:
    cursor = 0
    while True:
        cursor = data.find(needle, cursor)
        if cursor < 0:
            return
        yield cursor
        cursor += 1


def _modrm_rva_for_disp8(data: bytes, displacement_offset: int) -> int | None:
    """Return a nearby ModRM byte for a one-byte positive displacement."""

    if displacement_offset <= 0:
        return None
    direct = displacement_offset - 1
    modrm = data[direct]
    if modrm & 0xC0 == 0x40 and modrm & 0x07 != 0x04:
        return direct

    # An SIB byte follows ModRM when rm=100.  The displacement follows SIB.
    if displacement_offset < 2:
        return None
    modrm_offset = displacement_offset - 2
    modrm = data[modrm_offset]
    if modrm & 0xC0 == 0x40 and modrm & 0x07 == 0x04:
        return modrm_offset
    return None


def _modrm_rva_for_disp32(data: bytes, displacement_offset: int) -> int | None:
    """Return a nearby ModRM byte for a four-byte displacement."""

    if displacement_offset <= 0:
        return None
    direct = displacement_offset - 1
    modrm = data[direct]
    if modrm & 0xC0 == 0x80 and modrm & 0x07 != 0x04:
        return direct

    # The SIB byte is immediately before the four-byte displacement.
    if displacement_offset < 2:
        return None
    modrm_offset = displacement_offset - 2
    modrm = data[modrm_offset]
    if modrm & 0xC0 == 0x80 and modrm & 0x07 == 0x04:
        return modrm_offset
    return None


def raw_field_candidates(
    text: bytes,
    *,
    text_rva: int = TEXT_RVA,
    field_offsets: Iterable[int],
    max_candidates: int = DEFAULT_MAX_RAW_CANDIDATES,
) -> tuple[list[RawFieldCandidate], dict[str, Any]]:
    """Find bounded ModRM/displacement-shaped candidates without decoding text.

    Positive displacements below ``0x80`` commonly use ModRM ``disp8`` and
    larger values such as ``+0x98`` use ``disp32``.  The prefilter only accepts
    the corresponding ModRM forms; Capstone remains the authority for an
    actual instruction boundary and memory operand semantics.
    """

    if max_candidates <= 0:
        raise ValueError("max_candidates must be positive")
    # Preserve the caller's order.  ``scan`` puts target record fields first so
    # a noisy nearby/context displacement cannot consume the entire raw budget
    # before +0x31/+0x32 are examined.
    offsets = tuple(dict.fromkeys(field_offsets))
    if not offsets or any(offset < 0 or offset > 0x7FFF_FFFF for offset in offsets):
        raise ValueError("field offsets must be non-empty non-negative int32 values")

    candidates: list[RawFieldCandidate] = []
    seen: set[tuple[int, int, str, int]] = set()
    form_counts: Counter[str] = Counter()
    raw_occurrence_counts: Counter[str] = Counter()
    form_counts_by_offset: Counter[int] = Counter()
    raw_occurrence_counts_by_offset: Counter[int] = Counter()
    truncated = False

    def add_candidate(
        displacement_offset: int,
        field_offset: int,
        encoding: str,
        modrm_offset: int,
    ) -> bool:
        nonlocal truncated
        raw_occurrence_counts[encoding] += 1
        key = (displacement_offset, field_offset, encoding, modrm_offset)
        if key in seen:
            return True
        raw_occurrence_counts_by_offset[field_offset] += 1
        if len(candidates) >= max_candidates:
            truncated = True
            return False
        seen.add(key)
        candidates.append(
            RawFieldCandidate(
                displacement_rva=text_rva + displacement_offset,
                field_offset=field_offset,
                encoding=encoding,
                modrm_rva=text_rva + modrm_offset,
            )
        )
        form_counts[encoding] += 1
        form_counts_by_offset[field_offset] += 1
        return True

    for field_offset in offsets:
        if field_offset <= 0x7F:
            needle = bytes((field_offset,))
            for displacement_offset in _iter_occurrences(text, needle):
                modrm_offset = _modrm_rva_for_disp8(text, displacement_offset)
                if modrm_offset is None:
                    continue
                if not add_candidate(
                    displacement_offset, field_offset, "disp8", modrm_offset
                ):
                    break
            if truncated:
                break

        needle = struct.pack("<i", field_offset)
        for displacement_offset in _iter_occurrences(text, needle):
            modrm_offset = _modrm_rva_for_disp32(text, displacement_offset)
            if modrm_offset is None:
                continue
            if not add_candidate(
                displacement_offset, field_offset, "disp32", modrm_offset
            ):
                break
        if truncated:
            break

    candidates.sort(key=lambda item: (item.displacement_rva, item.field_offset, item.encoding))
    return candidates, {
        "requested_field_offsets": [f"0x{offset:X}" for offset in offsets],
        "raw_candidate_count": len(candidates),
        "raw_candidate_limit": max_candidates,
        "raw_candidate_limit_reached": truncated,
        "raw_occurrence_counts_by_encoding": dict(sorted(raw_occurrence_counts.items())),
        "accepted_candidate_counts_by_encoding": dict(sorted(form_counts.items())),
        "raw_occurrence_counts_by_offset": {
            f"0x{offset:X}": count
            for offset, count in sorted(raw_occurrence_counts_by_offset.items())
        },
        "accepted_candidate_counts_by_offset": {
            f"0x{offset:X}": count
            for offset, count in sorted(form_counts_by_offset.items())
        },
        "prefilter_text_bytes_examined": len(text),
    }


def _load_capstone(vendor_path: Path | None):
    if vendor_path is not None:
        sys.path.insert(0, str(vendor_path.resolve()))
    try:
        from capstone import CS_AC_READ, CS_ARCH_X86, CS_MODE_64, Cs
        from capstone.x86 import X86_OP_MEM
    except ImportError as error:  # pragma: no cover - environment failure
        raise RuntimeError(
            "Capstone is required; pass --vendor-path or use the prepared research Python"
        ) from error
    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    decoder.detail = True
    return decoder, X86_OP_MEM, CS_AC_READ


def _access_names(access: int, read_flag: int) -> list[str]:
    return ["read"] if access & read_flag else []


def _hex_rva(value: int | None) -> str | None:
    return None if value is None else f"0x{value:X}"


def _field_class(
    displacement: int,
    target_offsets: set[int],
    nearby_offsets: set[int],
    context_offsets: set[int],
) -> str | None:
    if displacement in target_offsets:
        return "target_record_field"
    if displacement in nearby_offsets:
        return "nearby_record_field"
    if displacement in context_offsets:
        return "secondary_static_row_context"
    return None


def _record_width_allowed(field_class: str, operand_size: int) -> bool:
    if field_class == "secondary_static_row_context":
        return operand_size in {1, 2, 4, 8}
    return operand_size in {1, 2}


def scan(
    text: bytes,
    pdata: bytes,
    *,
    text_rva: int = TEXT_RVA,
    target_offsets: Iterable[int] = DEFAULT_TARGET_OFFSETS,
    nearby_record_offsets: Iterable[int] = DEFAULT_NEARBY_RECORD_OFFSETS,
    context_offsets: Iterable[int] = DEFAULT_CONTEXT_OFFSETS,
    max_raw_candidates: int = DEFAULT_MAX_RAW_CANDIDATES,
    max_function_bytes: int = DEFAULT_MAX_FUNCTION_BYTES,
    max_total_decoded_bytes: int = DEFAULT_MAX_TOTAL_DECODED_BYTES,
    max_output_count: int = DEFAULT_MAX_OUTPUT_COUNT,
    max_skip_examples: int = DEFAULT_SKIP_EXAMPLES,
    excluded_base_registers: Iterable[str] = ("rsp", "esp"),
    vendor_path: Path | None = None,
) -> dict[str, Any]:
    """Run the bounded candidate inventory over already loaded section bytes."""

    if max_function_bytes <= 0 or max_total_decoded_bytes <= 0 or max_output_count <= 0:
        raise ValueError("scan limits must be positive")
    if max_skip_examples < 0:
        raise ValueError("max_skip_examples cannot be negative")

    target_values = tuple(target_offsets)
    nearby_values = tuple(nearby_record_offsets)
    context_values = tuple(context_offsets)
    target_set = set(target_values)
    nearby_set = set(nearby_values)
    context_set = set(context_values)
    # Keep targets ahead of optional context in the bounded prefilter.  Sets
    # below are used only for membership/classification.
    all_offsets = tuple(
        dict.fromkeys(
            target_values + nearby_values + context_values
        )
    )
    if not target_set:
        raise ValueError("at least one target record offset is required")
    if any(offset < 0 or offset > 0x7FFF_FFFF for offset in all_offsets):
        raise ValueError("field offsets must be non-negative int32 values")

    functions = parse_runtime_functions(pdata)
    raw_candidates, prefilter_stats = raw_field_candidates(
        text,
        text_rva=text_rva,
        field_offsets=all_offsets,
        max_candidates=max_raw_candidates,
    )
    starts = [function.begin_rva for function in functions]
    ranges = {function.begin_rva: function for function in functions}

    candidates_by_function: dict[tuple[int, int], list[RawFieldCandidate]] = {}
    skip_counts: Counter[str] = Counter()
    skip_examples: list[dict[str, Any]] = []

    def skip(reason: str, **details: Any) -> None:
        skip_counts[reason] += 1
        if len(skip_examples) < max_skip_examples:
            item: dict[str, Any] = {"reason": reason}
            item.update(details)
            skip_examples.append(item)

    for candidate in raw_candidates:
        index = bisect.bisect_right(starts, candidate.displacement_rva) - 1
        function = ranges[starts[index]] if index >= 0 else None
        if function is None or candidate.displacement_rva >= function.end_rva:
            skip("no_pdata_function", displacement_rva=_hex_rva(candidate.displacement_rva))
            continue
        size = function.end_rva - function.begin_rva
        if size > max_function_bytes:
            skip(
                "function_exceeds_max_function_bytes",
                displacement_rva=_hex_rva(candidate.displacement_rva),
                function_begin_rva=_hex_rva(function.begin_rva),
                function_end_rva=_hex_rva(function.end_rva),
                function_byte_size=size,
            )
            continue
        if function.begin_rva < text_rva or function.end_rva > text_rva + len(text):
            skip(
                "function_outside_text_dump",
                function_begin_rva=_hex_rva(function.begin_rva),
                function_end_rva=_hex_rva(function.end_rva),
            )
            continue
        candidates_by_function.setdefault(
            (function.begin_rva, function.end_rva), []
        ).append(candidate)

    decoder, memory_operand, read_flag = _load_capstone(vendor_path)
    excluded_bases = set(excluded_base_registers)
    decoded_function_bytes = 0
    decoded_instruction_bytes = 0
    decoded_instruction_count = 0
    scanned_function_count = 0
    output_limit_reached = False
    accesses: list[dict[str, Any]] = []
    function_reports: dict[tuple[int, int], dict[str, Any]] = {}

    for function_key in sorted(candidates_by_function):
        begin, end = function_key
        size = end - begin
        if decoded_function_bytes + size > max_total_decoded_bytes:
            skip(
                "total_decoded_function_bytes_limit",
                function_begin_rva=_hex_rva(begin),
                function_end_rva=_hex_rva(end),
                function_byte_size=size,
            )
            continue
        if len(accesses) >= max_output_count:
            output_limit_reached = True
            skip("max_output_count_limit", function_begin_rva=_hex_rva(begin))
            continue

        decoded_function_bytes += size
        scanned_function_count += 1
        block = text[begin - text_rva : end - text_rva]
        report = {
            "function_begin_rva": _hex_rva(begin),
            "function_end_rva": _hex_rva(end),
            "function_unwind_rva": _hex_rva(
                ranges[begin].unwind_rva
            ),
            "function_byte_size": size,
            "prefilter_candidate_count": len(candidates_by_function[function_key]),
            "accesses": [],
        }
        function_reports[function_key] = report

        for instruction in decoder.disasm(block, begin):
            decoded_instruction_count += 1
            decoded_instruction_bytes += instruction.size
            for operand_index, operand in enumerate(instruction.operands):
                if operand.type != memory_operand:
                    continue
                displacement = operand.mem.disp
                field_class = _field_class(
                    displacement, target_set, nearby_set, context_set
                )
                if field_class is None:
                    continue
                if not (operand.access & read_flag):
                    continue
                base_register = instruction.reg_name(operand.mem.base)
                if base_register in excluded_bases:
                    skip(
                        "stack_base_operand",
                        instruction_rva=_hex_rva(instruction.address),
                        base_register=base_register,
                    )
                    continue
                if not _record_width_allowed(field_class, operand.size):
                    continue
                evidence = {
                    "rva": _hex_rva(instruction.address),
                    "bytes_hex": bytes(instruction.bytes).hex(" ").upper(),
                    "mnemonic": instruction.mnemonic,
                    "operands": instruction.op_str,
                    "memory_operand_index": operand_index,
                    "memory_operand_size": operand.size,
                    "base_register": base_register,
                    "index_register": instruction.reg_name(operand.mem.index),
                    "displacement": _hex_rva(displacement),
                    "field_class": field_class,
                    "access": _access_names(operand.access, read_flag),
                    "function_begin_rva": _hex_rva(begin),
                    "function_end_rva": _hex_rva(end),
                    "function_unwind_rva": _hex_rva(ranges[begin].unwind_rva),
                    "validation_source": "raw ModRM prefilter + pdata-bounded Capstone instruction",
                }
                accesses.append(evidence)
                report["accesses"].append(evidence)
                if len(accesses) >= max_output_count:
                    output_limit_reached = True
                    break
            if output_limit_reached:
                break

        if output_limit_reached:
            # Remaining instructions in this already bounded function are not
            # decoded after the output cap; the function itself was accounted.
            for remaining_key in sorted(candidates_by_function):
                if remaining_key > function_key:
                    skip(
                        "max_output_count_limit",
                        function_begin_rva=_hex_rva(remaining_key[0]),
                    )
            break

    accesses.sort(key=lambda item: int(item["rva"], 16))
    for report in function_reports.values():
        report["accesses"].sort(key=lambda item: int(item["rva"], 16))
    record_candidate_functions = [
        function_reports[key]
        for key in sorted(function_reports)
        if any(
            access["field_class"]
            in {"target_record_field", "nearby_record_field"}
            for access in function_reports[key]["accesses"]
        )
    ]
    exact_accesses = [
        item for item in accesses if item["field_class"] == "target_record_field"
    ]
    nearby_accesses = [
        item for item in accesses if item["field_class"] == "nearby_record_field"
    ]
    context_accesses = [
        item for item in accesses if item["field_class"] == "secondary_static_row_context"
    ]
    target_accesses_by_function: dict[str, set[str]] = {}
    for item in exact_accesses:
        target_accesses_by_function.setdefault(item["function_begin_rva"], set()).add(
            item["displacement"]
        )
    paired_target_functions = sorted(
        (
            function_rva
            for function_rva, observed in target_accesses_by_function.items()
            if {f"0x{offset:X}" for offset in target_set}.issubset(observed)
        ),
        key=lambda value: int(value, 16),
    )
    context_function_rvas = {
        item["function_begin_rva"]
        for item in context_accesses
    }
    paired_target_context_functions = sorted(
        (
            function_rva
            for function_rva in paired_target_functions
            if function_rva in context_function_rvas
        ),
        key=lambda value: int(value, 16),
    )

    return {
        "schema": "nioh3-static-equipment-record-field-xrefs/v1",
        "scope": "offline PC v2.0.2.0 live .text/.pdata section dump",
        "text_rva": _hex_rva(text_rva),
        "target_record_offsets": [_hex_rva(value) for value in sorted(target_set)],
        "nearby_record_offsets": [_hex_rva(value) for value in sorted(nearby_set)],
        "secondary_context_offsets": [_hex_rva(value) for value in sorted(context_set)],
        # Context-only functions are intentionally omitted from this primary
        # list; their validated accesses remain in the flat evidence list and
        # the counts below, where consumers can join them by function RVA.
        "candidate_functions": record_candidate_functions,
        "accesses": accesses,
        "candidate_function_rvas": [
            report["function_begin_rva"] for report in record_candidate_functions
        ],
        "paired_target_functions": paired_target_functions,
        "paired_target_function_evidence": [
            report
            for report in record_candidate_functions
            if report["function_begin_rva"] in paired_target_functions
        ],
        "paired_target_context_functions": paired_target_context_functions,
        "paired_target_context_function_evidence": [
            report
            for report in record_candidate_functions
            if report["function_begin_rva"] in paired_target_context_functions
        ],
        "raw_prefilter": prefilter_stats,
        "scan_limits": {
            "max_raw_candidates": max_raw_candidates,
            "max_function_bytes": max_function_bytes,
            "max_total_decoded_bytes": max_total_decoded_bytes,
            "max_output_count": max_output_count,
            "max_skip_examples": max_skip_examples,
        },
        "scan_statistics": {
            "pdata_runtime_function_count": len(functions),
            "raw_candidates_after_prefilter": len(raw_candidates),
            "candidate_function_count": len(candidates_by_function),
            "scanned_function_count": scanned_function_count,
            "decoded_function_bytes": decoded_function_bytes,
            "decoded_instruction_bytes": decoded_instruction_bytes,
            "decoded_instruction_count": decoded_instruction_count,
            "validated_access_count": len(accesses),
            "target_record_access_count": len(exact_accesses),
            "nearby_record_access_count": len(nearby_accesses),
            "secondary_context_access_count": len(context_accesses),
            "paired_target_function_count": len(paired_target_functions),
            "paired_target_context_function_count": len(
                paired_target_context_functions
            ),
            "validated_u8_count": sum(item["memory_operand_size"] == 1 for item in accesses),
            "validated_u16_count": sum(item["memory_operand_size"] == 2 for item in accesses),
            "skipped": dict(sorted(skip_counts.items())),
            "skip_examples": skip_examples,
            "output_limit_reached": output_limit_reached,
            "excluded_base_registers": sorted(excluded_bases),
        },
        "limitations": [
            "The displacement and memory operand do not establish the base object's type.",
            "A candidate function may be unrelated code that happens to use the same offset.",
            "The raw prefilter recognizes ModRM disp8/disp32 shapes; unusual encodings may be missed.",
            "The default scan excludes rsp/esp-based operands and reports them as explicit skips.",
            "The static-row +0x98 value is labelled secondary context and is not an equipment-record field.",
            "No getter, caller role, menu path, or runtime object identity is inferred.",
            "Offline section bytes provide no live trigger, process identity, or semantic validation.",
        ],
    }


def build_report(
    text_path: Path,
    pdata_path: Path,
    *,
    text_rva: int = TEXT_RVA,
    target_offsets: Iterable[int] = DEFAULT_TARGET_OFFSETS,
    nearby_record_offsets: Iterable[int] = DEFAULT_NEARBY_RECORD_OFFSETS,
    context_offsets: Iterable[int] = DEFAULT_CONTEXT_OFFSETS,
    max_raw_candidates: int = DEFAULT_MAX_RAW_CANDIDATES,
    max_function_bytes: int = DEFAULT_MAX_FUNCTION_BYTES,
    max_total_decoded_bytes: int = DEFAULT_MAX_TOTAL_DECODED_BYTES,
    max_output_count: int = DEFAULT_MAX_OUTPUT_COUNT,
    max_skip_examples: int = DEFAULT_SKIP_EXAMPLES,
    vendor_path: Path | None = None,
) -> dict[str, Any]:
    """Load input files, scan them, and attach immutable input identity."""

    text = text_path.read_bytes()
    pdata = pdata_path.read_bytes()
    result = scan(
        text,
        pdata,
        text_rva=text_rva,
        target_offsets=target_offsets,
        nearby_record_offsets=nearby_record_offsets,
        context_offsets=context_offsets,
        max_raw_candidates=max_raw_candidates,
        max_function_bytes=max_function_bytes,
        max_total_decoded_bytes=max_total_decoded_bytes,
        max_output_count=max_output_count,
        max_skip_examples=max_skip_examples,
        vendor_path=vendor_path,
    )
    result["inputs"] = {
        "text": str(text_path.resolve()),
        "text_size": len(text),
        "text_sha256": sha256(text_path),
        "pdata": str(pdata_path.resolve()),
        "pdata_size": len(pdata),
        "pdata_sha256": sha256(pdata_path),
        "offline_only": True,
    }
    return result


def _parse_offsets(values: Sequence[str] | None, defaults: Iterable[int]) -> tuple[int, ...]:
    return tuple(parse_int(value) for value in values) if values else tuple(defaults)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--text", type=Path, required=True)
    parser.add_argument("--pdata", type=Path, required=True)
    parser.add_argument("--text-rva", type=parse_int, default=TEXT_RVA)
    parser.add_argument("--target-offset", action="append")
    parser.add_argument("--nearby-record-offset", action="append")
    parser.add_argument("--context-offset", action="append")
    parser.add_argument("--max-raw-candidates", type=int, default=DEFAULT_MAX_RAW_CANDIDATES)
    parser.add_argument("--max-function-bytes", type=parse_int, default=DEFAULT_MAX_FUNCTION_BYTES)
    parser.add_argument(
        "--max-total-decoded-bytes",
        type=parse_int,
        default=DEFAULT_MAX_TOTAL_DECODED_BYTES,
    )
    parser.add_argument("--max-output-count", type=int, default=DEFAULT_MAX_OUTPUT_COUNT)
    parser.add_argument("--max-skip-examples", type=int, default=DEFAULT_SKIP_EXAMPLES)
    parser.add_argument("--vendor-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)

    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite static evidence: {output}")
    result = build_report(
        args.text,
        args.pdata,
        text_rva=args.text_rva,
        target_offsets=_parse_offsets(args.target_offset, DEFAULT_TARGET_OFFSETS),
        nearby_record_offsets=_parse_offsets(
            args.nearby_record_offset, DEFAULT_NEARBY_RECORD_OFFSETS
        ),
        context_offsets=_parse_offsets(args.context_offset, DEFAULT_CONTEXT_OFFSETS),
        max_raw_candidates=args.max_raw_candidates,
        max_function_bytes=args.max_function_bytes,
        max_total_decoded_bytes=args.max_total_decoded_bytes,
        max_output_count=args.max_output_count,
        max_skip_examples=args.max_skip_examples,
        vendor_path=args.vendor_path,
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    statistics = result["scan_statistics"]
    print(
        json.dumps(
            {
                "output": str(output),
                "candidate_function_count": statistics["candidate_function_count"],
                "validated_access_count": statistics["validated_access_count"],
                "target_record_access_count": statistics["target_record_access_count"],
                "skipped": statistics["skipped"],
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
