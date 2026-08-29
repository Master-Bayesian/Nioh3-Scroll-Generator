from __future__ import annotations

import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).parent / "research" / "probe_scroll_auxiliary_text_catalog.py"
SPEC = importlib.util.spec_from_file_location("probe_scroll_auxiliary_text_catalog", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def _entry(text_id: int, text: str) -> bytes:
    encoded = text.encode("utf-16-le") + b"\x00\x00"
    code_units = len(encoded) // 2
    return text_id.to_bytes(4, "little") + code_units.to_bytes(4, "little") + encoded


def test_iter_text_entries_and_substring_matching() -> None:
    block = b"\x00\x00" + _entry(0x03606B44, "一難横行（足部防具）{1}")
    block += _entry(0x03E80C4C, "地獄")
    entries = list(MODULE.iter_text_entries_from_block(block, 0x1000))

    matches = MODULE.match_entries(entries, ["一難横行", "地獄"], exact=False)

    assert [entry.text_id for entry in matches["一難横行"]] == [0x03606B44]
    assert [entry.text_id for entry in matches["地獄"]] == [0x03E80C4C]


def test_exact_matching_does_not_accept_format_string() -> None:
    entries = [
        MODULE.TextEntry(1, 0x1000, 5, "地獄"),
        MODULE.TextEntry(2, 0x2000, 8, "地獄（常世）"),
    ]

    matches = MODULE.match_entries(entries, ["地獄"], exact=True)

    assert [entry.text_id for entry in matches["地獄"]] == [1]
