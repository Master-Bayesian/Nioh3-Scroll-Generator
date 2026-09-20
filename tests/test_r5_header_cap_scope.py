"""Narrow scope tests for the version-scoped rarity-5 header cap.

The PC v2.01/PC v2.02 known difference is accepted only for the exact two
header offsets with the exact native/offline values. Any extra differing byte,
any other value, and any unlisted version must stay "unexpected".

These tests are offline: they import the research helper and never touch the
game process.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
HELPER_PATH = PROJECT_ROOT / "research" / "validate_ng3_rarity5_native_parity_live.py"


def load_helper():
    spec = importlib.util.spec_from_file_location("r5_parity_helper", HELPER_PATH)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def records(native_header: bytes, offline_header: bytes, tail_diff: bool = False):
    native = bytearray(0xE8)
    offline = bytearray(0xE8)
    native[0x30:0x32] = native_header
    offline[0x30:0x32] = offline_header
    if tail_diff:
        native[0x40] = 0x01
        offline[0x40] = 0x02
    return bytes(native), bytes(offline)


def test_scope_accepts_only_the_documented_two_offsets() -> None:
    helper = load_helper()
    for version in ("PC v2.01", "PC v2.02"):
        native, offline = records(b"\x04\x04", b"\x05\x05")
        assert helper.expected_known_rarity_header_cap(
            version, [0x30, 0x31], native, offline
        )


def test_scope_rejects_extra_differing_bytes() -> None:
    helper = load_helper()
    native, offline = records(b"\x04\x04", b"\x05\x05", tail_diff=True)
    assert not helper.expected_known_rarity_header_cap(
        "PC v2.02", [0x30, 0x31, 0x40], native, offline
    )


def test_scope_rejects_other_offsets_and_values() -> None:
    helper = load_helper()
    native, offline = records(b"\x04\x04", b"\x05\x05")
    assert not helper.expected_known_rarity_header_cap(
        "PC v2.02", [0x30], native, offline
    )
    assert not helper.expected_known_rarity_header_cap(
        "PC v2.02", [0x31, 0x30], native, offline
    )

    native_other, offline_other = records(b"\x03\x03", b"\x05\x05")
    assert not helper.expected_known_rarity_header_cap(
        "PC v2.02", [0x30, 0x31], native_other, offline_other
    )
    native_other, offline_other = records(b"\x04\x04", b"\x04\x05")
    assert not helper.expected_known_rarity_header_cap(
        "PC v2.02", [0x30, 0x31], native_other, offline_other
    )


def test_scope_rejects_unlisted_versions_and_lists_exactly_two() -> None:
    helper = load_helper()
    native, offline = records(b"\x04\x04", b"\x05\x05")
    assert not helper.expected_known_rarity_header_cap(
        "PC v9.99", [0x30, 0x31], native, offline
    )
    assert set(helper.KNOWN_RARITY5_HEADER_CAP_VERSIONS) == {"PC v2.01", "PC v2.02"}
    assert helper.HEADER_CAP_OFFSETS == [0x30, 0x31]
    assert helper.HEADER_CAP_NATIVE_BYTES == b"\x04\x04"
    assert helper.HEADER_CAP_OFFLINE_BYTES == b"\x05\x05"
