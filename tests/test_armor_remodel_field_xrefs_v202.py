from __future__ import annotations

import importlib.util
from pathlib import Path
import struct
import sys

import pytest


ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "research" / "armor_remodel_field_xrefs_v202.py"
SPEC = importlib.util.spec_from_file_location("armor_remodel_field_xrefs_v202", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_scan_validates_u8_u16_and_separates_static_context() -> None:
    code = bytes.fromhex(
        "0F B6 41 31 "  # movzx eax, byte ptr [rcx + 0x31]
        "0F B7 41 32 "  # movzx eax, word ptr [rcx + 0x32]
        "8A 41 30 "     # mov al, byte ptr [rcx + 0x30]
        "8B 81 98 00 00 00 "  # mov eax, dword ptr [rcx + 0x98]
        "0F B6 44 24 31 "  # stack local: excluded by the default base filter
        "88 41 31 "     # store: must not be reported as a read
        "C3"
    )
    pdata = struct.pack("<III", 0x1000, 0x1000 + len(code), 0x4000)

    result = MODULE.scan(
        code,
        pdata,
        target_offsets=(0x31, 0x32),
        nearby_record_offsets=(0x30, 0x33),
        context_offsets=(0x98,),
        max_total_decoded_bytes=len(code),
    )

    assert result["scan_statistics"]["validated_access_count"] == 4
    assert result["scan_statistics"]["target_record_access_count"] == 2
    assert result["scan_statistics"]["nearby_record_access_count"] == 1
    assert result["scan_statistics"]["secondary_context_access_count"] == 1
    assert result["scan_statistics"]["validated_u8_count"] == 2
    assert result["scan_statistics"]["validated_u16_count"] == 1
    assert result["scan_statistics"]["skipped"]["stack_base_operand"] == 1
    assert result["scan_statistics"]["paired_target_function_count"] == 1
    assert result["scan_statistics"]["paired_target_context_function_count"] == 1
    assert [item["displacement"] for item in result["accesses"]] == [
        "0x31",
        "0x32",
        "0x30",
        "0x98",
    ]
    assert result["candidate_function_rvas"] == ["0x1000"]


def test_scan_records_total_function_byte_skip() -> None:
    code = bytes.fromhex("0F B6 41 31 C3")
    pdata = struct.pack("<III", 0x1000, 0x1000 + len(code), 0x4000)

    result = MODULE.scan(
        code,
        pdata,
        max_total_decoded_bytes=len(code) - 1,
    )

    assert result["accesses"] == []
    assert result["scan_statistics"]["scanned_function_count"] == 0
    assert result["scan_statistics"]["skipped"]["total_decoded_function_bytes_limit"] == 1


def test_raw_prefilter_is_bounded() -> None:
    with pytest.raises(ValueError, match="max_candidates"):
        MODULE.raw_field_candidates(
            b"\x31" * 32,
            field_offsets=(0x31,),
            max_candidates=0,
        )
