"""Focused tests for the PC v2.02 live-add code-identity resource derivation."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
V201_RESOURCE = REPO / "nioh3_scroll_editor/data/live_add_pc_v201_identity.json"
V202_RESOURCE = REPO / "nioh3_scroll_editor/data/live_add_pc_v202_identity.json"
MAPPING_EVIDENCE = (
    REPO
    / "tests/fixtures/game-version-update-20260919"
    / "identity-mapping.json"
)
EXPORTER = REPO / "tools/export_v202_identity_resource.py"
V202_TEXT = Path(
    r"D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919"
    r"\sections-live\Nioh3_v2.0.2.0.text.bin"
)

V201_RESOURCE_SHA = "03EB144A66E36A15CA3B8262463B9DB211F7C3ECA7B7910F753E38849ECE27AD"
V202_MAPPING_SHA = "1F0DB8EB22D8300A561B5FA921D26343431AD9BAF9C2676074C5C696EDE20588"


def _load_exporter():
    spec = importlib.util.spec_from_file_location("export_live_add_identity", EXPORTER)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def test_v201_identity_resource_is_untouched():
    payload = json.loads(V201_RESOURCE.read_text(encoding="utf-8"))
    assert _sha256(V201_RESOURCE) == V201_RESOURCE_SHA
    assert payload["profile_id"] == "pc-v2.01-live-add-r1"
    assert len(payload["ranges"]) == 60
    for entry in payload["ranges"]:
        assert set(entry) == {"rva", "size", "sha256"}


def test_v202_resource_is_complete_or_honest_about_what_it_covers():
    """Every v2.01 range is either verified or explicitly recorded as unverified."""
    assert V202_RESOURCE.exists(), "the candidate identity resource is not shipped"
    payload = json.loads(V202_RESOURCE.read_text(encoding="utf-8"))
    assert payload["profile_id"] == "pc-v2.02-live-add-candidate"
    assert payload["executable_sha256"] == (
        "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130"
    )
    assert len(payload["ranges"]) + len(payload["unverified_ranges"]) == 60
    assert payload["ranges"], "a resource with no verified range is not acceptable"
    for entry in payload["ranges"]:
        assert set(entry) == {"rva", "size", "sha256"}
    for entry in payload["unverified_ranges"]:
        assert set(entry) == {"v201_rva", "size", "reason"}
    assert payload["derivation"]["mapped_from_accepted_mapping"] >= 16


def test_mapping_evidence_records_the_gap_precisely():
    assert _sha256(MAPPING_EVIDENCE) == V202_MAPPING_SHA
    mapping = json.loads(MAPPING_EVIDENCE.read_text(encoding="utf-8"))
    assert mapping["schema"] == "nioh3-live-add-identity-mapping/v1"
    assert mapping["profile_id"] == "pc-v2.02-live-add-candidate"
    assert mapping["sources"]["installed_executable"].startswith("E22C4A63")
    assert len(mapping["entries"]) == 60
    totals = mapping["totals"]
    assert totals["ranges"] == 60
    assert totals["mapped"] + totals["unmapped"] == 60
    assert totals["unmapped"] == len(mapping["unmapped"])
    for item in mapping["unmapped"]:
        assert item["reason"] in {
            "ambiguous-shape-match",
            "no-match",
            "decode-mismatch",
            "no-anchor",
        }
        if item["reason"] == "ambiguous-shape-match":
            assert len(item["candidates"]) >= 2
    for entry in mapping["entries"]:
        if entry["v202_rva"] is not None:
            assert entry["method"] in {"accepted-mapping", "unique-shape-match"}
            assert len(entry["v202_sha256"]) == 64
        else:
            assert entry["v202_rva"] is None


def test_shape_helpers_mask_values_and_keep_widths():
    module = _load_exporter()
    machine = module.decoder()
    blob = bytes.fromhex("488b4a084881c1604a2200c3")  # mov rcx,[rdx+8]; add rcx,0x224a60; ret
    insns = list(machine.disasm(blob, 0x1000))
    # capstone reports the sign-extended imm32 of `add rcx, imm32` as an 8-byte
    # operand, which is exactly the width the shape comparison keeps.
    assert module.instruction_shape(insns) == "mov|reg,mem:8;add|reg,imm:8;ret|"
    mask = module.value_mask(insns)
    assert len(mask) == len(blob)
    # The displacement and the immediate bytes are masked; opcodes are not.
    assert mask[3] and mask[8] and not mask[0]
    offset, run = module.longest_unmasked_run(mask)
    assert run >= 1 and offset >= 0


@pytest.mark.skipif(not V202_TEXT.is_file(), reason="v2.02 lane image not present")
def test_exporter_emits_the_honest_resource_and_can_require_completeness():
    module = _load_exporter()
    mapping = module.build_mapping()
    assert mapping["totals"]["ranges"] == 60
    assert mapping["totals"]["mapped"] + mapping["totals"]["unmapped"] == 60
    assert module.main() == 0
    if mapping["totals"]["unmapped"]:
        # The strict gate stays available for the day all ranges relocate.
        assert module.main(["--require-complete"]) == 2
