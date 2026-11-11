"""Project verified native group identities/formatting for read-only loadout totals."""
import argparse
import hashlib
import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.effect_generation_tables import load_default_effect_generation_tables


def complete_native_texts(text_path, capture_path, tables):
    """Reuse the set projection's native text and weapon/item argument contract."""
    raw = text_path.read_bytes()
    source = json.loads(raw)
    expected = json.loads((ROOT / "apps/workshop/loadout-sets.json").read_text(encoding="utf-8"))
    assert hashlib.sha256(raw).hexdigest() == expected["sources"]["native_text_sha256"]
    assert source["status"] == "complete_native_text_tree_capture" and source["read_only"] and source["game_writes"] == 0
    assert source["executable_sha256"].lower() == expected["executable_sha256"]
    labels = {int(key): entry["text"] for key, entry in source["entries"].items()}
    capture = json.loads(capture_path.read_text(encoding="utf-8"))
    assert capture["read_only"] and capture["game_writes"] == 0
    assert capture["executable_sha256"].lower() == expected["executable_sha256"]
    entries = {row["path"]: row for row in capture["blobs"]}
    def blob(name):
        data = (capture_path.parent / name).read_bytes()
        assert len(data) == entries[name]["size"] and hashlib.sha256(data).hexdigest() == entries[name]["sha256"].lower()
        return data
    weapons = blob("candidates/300-store-prefix.bin")
    assert hashlib.sha256(weapons).hexdigest() == expected["sources"]["weapon_store_sha256"]
    count = struct.unpack_from("<I", weapons, 4)[0]
    assert 8 + count * 0x80 <= len(weapons)
    indices = {key: index for key, index in struct.iter_unpack("<II", blob("candidates/300-hash-entries.bin")) if index < count}
    assert len(indices) == count
    weapon_labels = {key: labels[struct.unpack_from("<I", weapons, 8 + index * 0x80 + 0x1C)[0]] for key, index in indices.items()}
    items = {struct.unpack_from("<H", row, 0x152)[0]: row for row in tables.resource.table("item").rows()}
    def resolve(row, offset):
        label = labels.get(struct.unpack_from("<I", row, offset)[0])
        if label is None:
            return None
        for position in (0x38, 0x3C):
            argument = labels.get(struct.unpack_from("<I", row, position)[0])
            if argument:
                label = label.replace("{}", argument, 1)
        weapon_key = struct.unpack_from("<I", row, 0x20)[0]
        if weapon_key in weapon_labels and "{}" in label:
            label = label.replace("{}", weapon_labels[weapon_key], 1)
        item = items.get(struct.unpack_from("<I", row, 0x18)[0])
        if item is not None and "{}" in label and struct.unpack_from("<I", item, 0x74)[0] == 0:
            argument = labels.get(struct.unpack_from("<I", item, 0x68)[0])
            if argument:
                label = label.replace("{}", argument, 1)
        return re.sub(r"\^09~(?:BUFF|DEBUFF)~", "", label).replace("^09~~", "")
    return labels, resolve, {"name": text_path.name, "sha256": hashlib.sha256(raw).hexdigest(), "executable_sha256": source["executable_sha256"]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--guardian-catalog", type=Path, required=True)
    parser.add_argument("--clan-catalog", type=Path, required=True)
    parser.add_argument("--native-text", type=Path, required=True)
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--runtime-sections", type=Path, required=True)
    args = parser.parse_args()
    tables = load_default_effect_generation_tables()
    native_groups = {key: tables.resource.table("effect_group").row(row.row_index)
                     for key, row in tables.groups_by_key.items()}
    units, known, sources = {}, {}, []
    grades = None
    for path in (args.guardian_catalog, args.clan_catalog):
        raw = path.read_bytes()
        source = json.loads(raw)
        assert source["source_policy"]["native_only"]
        assert source["game_identity"]["game_version"] == "PC v2.02"
        sources.append({"name": path.name, "sha256": hashlib.sha256(raw).hexdigest(),
                        "executable_sha256": source["game_identity"]["executable_sha256"]})
        if grades is None:
            grades = source["grade_labels_by_index"]
        else:
            assert grades == source["grade_labels_by_index"]
        for group in source["effect_groups"]:
            # Captured formatter metadata must match the shipped pointer-free row.
            assert bytes.fromhex(group["raw_hex"]) == native_groups[group["id"]]
            tid = group["text_ids"]["unit_template"]
            if tid:
                assert tid not in units or units[tid] == group["unit_template"]
                units[tid] = group["unit_template"]
            if group["id"] in known:
                assert known[group["id"]]["label_zh"] == group["label_zh"]
            known[group["id"]] = group
    labels, resolve, text_source = complete_native_texts(args.native_text, args.capture, tables)
    sources.append(text_source)
    expected = json.loads((ROOT / "apps/workshop/loadout-sets.json").read_text(encoding="utf-8"))
    from loadout_handoff_code import verify_reviewed_code
    runtime_hash = verify_reviewed_code(args.runtime_sections, expected["executable_sha256"], {
        0x1BC1B0B: "b8fe5400006639410c756e"})
    assert runtime_hash == expected["sources"]["runtime_text_sha256"]
    assert labels[0x035D4DAF] == "{}<dot>{}"
    groups = {}
    for key, row in native_groups.items():
        unit_id = struct.unpack_from("<I", row, 0x34)[0]
        flags = struct.unpack_from("<I", row, 0x50)[0]
        template = labels.get(unit_id) if unit_id else ""
        if unit_id and template is not None:
            assert unit_id not in units or units[unit_id] == template
            units[unit_id] = template
        spirit = struct.unpack_from("<H", row, 0xC)[0] == 0x54FE
        # Keep unknown templates explicit instead of guessing units.
        supported = ((bool(flags & 4) or template is not None)
                     and row[0x6C] in (0, 1, 2))
        groups[str(key)] = {"id": key, "grade": bool(flags & 4), "decimal_mode": row[0x6C],
                            "unit_template": template, "format_supported": supported,
                            "format_kind": "spirit_integer_decimal" if spirit else "native",
                            "raw_cap_i32": struct.unpack_from("<i", row, 4)[0],
                            "merge_kind": "max" if key == 0x38BA else "grade" if flags & 4 else "sum",
                            "label_zh": resolve(row, 0x2C) or known.get(key, {}).get("label_zh"),
                            "description_zh": resolve(row, 0x30) or known.get(key, {}).get("description")}
    output = {"schema": "nioh3-loadout-effect-catalog/v1", "game_version": "PC v2.02",
              "sources": sources, "grade_labels": grades,
              "effect_to_group": {str(key): row.group_key for key, row in tables.effects_by_id.items()},
              "groups": groups,
              "scope": "Native group-value accumulation and display only; not final combat formulas, triggered-buff stacking, skill-effect mapping or set tiers."}
    destination = ROOT / "apps/workshop/loadout-effect-catalog.json"
    destination.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"Exported {len(output['effect_to_group'])} effects, {len(groups)} groups, {len(units)} native unit templates; "
          f"{sum(not group['format_supported'] for group in groups.values())} groups need formatting follow-up")


if __name__ == "__main__":
    main()
