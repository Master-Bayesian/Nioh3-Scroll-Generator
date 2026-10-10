"""Getter boundary checks and acceptance against retained native returns."""

from copy import deepcopy
import csv
import importlib.util
import json
import shutil
from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("armor_remodel_batch_v202", ROOT / "tools/armor_remodel_batch_v202.py")
batch = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = batch
SPEC.loader.exec_module(batch)


@pytest.fixture
def model():
    result = batch.ArmorModel.__new__(batch.ArmorModel)
    # Different alternate requirements and a second alternate expose wrong-row
    # requirement reads and accidental recursive row selection.
    original = batch.Item(0, 1, 0x0DF9, 2, 56, 47, 15, (0, 0, 0x0915, 0x0600, 0x0814, 0, 0))
    alternate = batch.Item(1, 2, 0x2B2F, 3, 55, 50, 16, (99,) * 7)
    further = batch.Item(2, 3, 0x09A9, 0, 88, 500, 17, (88,) * 7)
    result.items = {item.item_id: item for item in (original, alternate, further)}
    result.armor = dict(result.items)
    result.internal = {2, 3}
    result.curve = [(index, index) for index in range(502)]
    result.curve[20] = (15, 9)
    result.names = {"1": ["Example", "Armor", "Head", ""]}
    result.live_fields = set()
    result.cold_rows = [(batch.f32(3.17), batch.f32(8.88)), (batch.f32(6.88), batch.f32(19.36)),
                        (batch.f32(9.64), batch.f32(39.74))]
    result.empty_key = 255
    result.map_entries = [(255, 0xFFFFFFFF), (16, 1), (255, 0xFFFFFFFF), (255, 0xFFFFFFFF),
                          (15, 0), (17, 2), (255, 0xFFFFFFFF)]
    return result


@pytest.mark.parametrize("mode,weight,requirements", [
    ("0/0", 55, [0, 0, 21, 0, 20, 0, 0]),
    ("1/0", 55, [0, 0, 28, 0, 27, 0, 0]),
    ("2/0", 59, [0, 0, 21, 0, 20, 0, 0]),
    ("1/1", 55, [0, 0, 36, 0, 35, 0, 0]),
    ("2/2", 64, [0, 0, 21, 0, 20, 0, 0]),
    ("1/2", 59, [0, 0, 28, 0, 27, 0, 0]),
])
def test_six_modes_use_original_requirements_and_one_hop_alternate(model, mode, weight, requirements):
    value = model.calculate(1, mode=mode)
    assert (value["weight_raw"], value["requirements"], value["toughness"]) == (weight, requirements, 50)
    assert value["selected_row_id"] == 2
    assert value["cross_slot_alternate"] is True


@pytest.mark.parametrize("rarity,weight,requirements,toughness", [
    (0, 56, [0, 0, 21, 0, 20, 0, 0], 47),
    (1, 56, [0, 0, 21, 0, 20, 0, 0], 47),
    (2, 56, [0, 0, 21, 0, 20, 0, 0], 47),
    (3, 60, [0, 0, 28, 0, 27, 0, 0], 47),
    (4, 59, [0, 0, 28, 0, 27, 0, 0], 50),
    (5, 59, [0, 0, 16, 13, 15, 0, 0], 50),
])
def test_rarity_gates_and_high_byte_zero_rule(model, rarity, weight, requirements, toughness):
    result = model.calculate(1, rarity=rarity, mode="1/2")
    assert (result["weight_raw"], result["requirements"], result["toughness"]) == (weight, requirements, toughness)
    assert ("rarity5_requirement_selector" in result["unverified_branches"]) == (rarity == 5)


@pytest.mark.parametrize("stage,cap", [(1, 5), (2, 15), (3, 30), (4, 45), (5, 120)])
def test_stage_cap_clamps_plus_before_table_lookup(model, stage, cap):
    result = model.calculate(1, plus=65535, stage=stage, mode="2/2")
    assert result["effective_plus"] == cap
    assert result["weight_raw"] == 55 + cap
    assert model.calculate(1, plus=0, stage=stage, mode="1/2")["requirement_delta_nonzero"] == 0


def test_cold_lookup_uses_original_key_and_caps_level(model):
    assert [model.cold_index(key) for key in (15, 16, 17, 18, -1)] == [0, 1, 2, None, None]
    value = model.calculate(1, flags=0x40000, level=65535)
    assert (value["cold_table_index"], value["cold_candidate"], value["toughness"]) == (0, 14, 14)
    assert value["toughness"] == model.calculate(1, flags=0x40000, level=180)["toughness"]
    assert model.calculate(1, flags=0x240000)["toughness"] == 8
    model.map_entries = [(255, 0xFFFFFFFF)] * 7
    assert model.calculate(1, flags=0x40000)["toughness"] == 50


def test_cold_candidate_preserves_float32_operation_rounding(model):
    slope = batch.f32(0.06944444)
    model.cold_rows[0] = (slope, 12.875)
    assert int(180 * slope * 0.01 + 12.875) == 12
    assert model.calculate(1, flags=0x40000)["toughness"] == 13
    model.cold_rows[0] = (100.0, 100.0)
    assert model.calculate(1, flags=0x40000)["toughness"] == 50


def test_lookup_full_collision_map_terminates_and_handles_signed_key(model):
    model.map_entries = [(100, 0)] * 7
    assert model.cold_index(-128) is None
    model.map_entries[0] = (128, 2)
    assert model.cold_index(-128) == 2
    model.map_entries[0] = (128, 999)
    assert model.cold_index(-128) is None


@pytest.mark.parametrize("arguments", [
    {"item_id": 999}, {"item_id": True}, {"level": -1}, {"plus": 65536},
    {"rarity": 6}, {"rarity": True}, {"stage": 0}, {"flags": -1}, {"mode": "2/1"},
])
def test_rejects_unsupported_inputs(model, arguments):
    with pytest.raises(ValueError):
        model.calculate(**{"item_id": 1, **arguments})


def test_live_grade_does_not_transfer_to_an_unobserved_configuration(model):
    value = model.calculate(1)
    model.live_fields = {model.field_key(value, field) for field in ["weight", "toughness"] + [f"requirement_{i}" for i in range(7)]}
    assert model.calculate(1, flags=0x20084)["verification"] == "live-matched-configuration"
    assert model.calculate(1, plus=21)["verification"] == "computed-only"
    special = model.calculate(1, flags=0x40000)
    assert "toughness" not in special["live_matched_fields"]
    assert special["verification"] == "computed-only"


@pytest.fixture
def live_model():
    return batch.ArmorModel()


def test_retained_native_corpus_and_catalog(live_model):
    report = live_model.validate_live()
    assert (report["native_returns"], report["distinct_item_mode_configurations"], report["distinct_verified_fields"], report["mismatches"]) == (1352, 52, 468, 0)
    assert report["paired_native_returns"] == 1190
    assert report["direct_native_queries"] == 162
    assert len(live_model.armor) == 806
    assert len(set(live_model.armor) - live_model.internal) == 636
    assert live_model.curve[20] == (15, 9)
    assert live_model.curve[120] == (42, 20)
    assert live_model.calculate(0x609C, mode="1/0")["verification_method"] == "natural-ui-getter-observation"
    assert live_model.calculate(0xC18E, mode="1/0")["verification_method"] == "native-query-on-temporary-record"


def test_public_file_integrity_is_checked_before_model_load(tmp_path):
    evidence = tmp_path / "public-evidence"
    shutil.copytree(batch.PUBLIC_EVIDENCE, evidence)
    path = evidence / "native-returns.json"
    path.write_bytes(path.read_bytes() + b" ")
    with pytest.raises(ValueError, match="Public evidence hash mismatch"):
        batch.ArmorModel(evidence)


@pytest.mark.parametrize("corruption", ["return", "pair", "cleanup"])
def test_rejects_tampered_native_evidence(live_model, monkeypatch, corruption):
    live_model.validate_live()
    assert live_model.live_fields
    original_capture = batch.load_json

    def altered(path):
        data = original_capture(path)
        if path.name == "native-returns.json":
            data = deepcopy(data)
            if corruption == "return":
                data["events"][0]["actual"] += 1
            elif corruption == "pair":
                data["runs"][0]["callbacks"] -= 1
            else:
                data["runs"][0]["cleanup_verified"] = False
        return data

    monkeypatch.setattr(batch, "load_json", altered)
    with pytest.raises(ValueError):
        live_model.validate_live()
    assert live_model.live_fields == set()


def test_rejects_corrupted_item_table(tmp_path):
    (tmp_path / "batch-dependencies").mkdir()
    (tmp_path / "batch-dependencies/item-live.bin").write_bytes(b"unsupported table")
    with pytest.raises(ValueError, match="Unsupported or corrupted"):
        batch.ArmorModel(tmp_path, ROOT / "apps/workshop/item-names.json")


def test_export_preserves_values_names_grades_and_existing_files(live_model, tmp_path):
    live_model.names = {**live_model.names, "24732": ["=Example", "Armor", "Head", ""]}
    output = tmp_path / "batch"
    summary = batch.export(live_model, output, rarity=4, level=180, plus_values=[20, 20], stage=3, flags=0, item_ids=[0x609C])
    assert summary["exported_configurations"] == 6
    assert summary["live_matched_configurations"] == 5
    with (output / "armor-remodel.csv").open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))
    assert len(rows) == 6
    assert rows[0]["name"] == "'=Example"
    assert rows[2]["weight_raw"] == "31"
    document = json.loads((output / "armor-remodel.json").read_text(encoding="utf-8"))
    assert document["rows"][0]["name"] == "=Example"
    before = (output / "armor-remodel.json").read_bytes()
    with pytest.raises(FileExistsError):
        batch.export(live_model, output, rarity=4, level=180, plus_values=[20], stage=3, flags=0)
    assert (output / "armor-remodel.json").read_bytes() == before
    invalid = tmp_path / "invalid"
    with pytest.raises(ValueError):
        batch.export(live_model, invalid, rarity=4, level=180, plus_values=[20], stage=3, flags=0, item_ids=[0xFFFF])
    assert not invalid.exists()
