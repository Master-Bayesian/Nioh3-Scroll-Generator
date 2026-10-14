"""Project PC v2.02 set tiers from retained read-only native captures.

Run through tools/run_python_tests.ps1 with --capture, --native-text and
--runtime-sections. No game process, runtime pointers or community data are
required in the application. Unsupported flags/modes reject the projection.
"""
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

EXE_SHA = "e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130"


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def export(capture_path, text_path, runtime_path):
    capture = json.loads(capture_path.read_text(encoding="utf-8"))
    assert capture["read_only"] and capture["game_writes"] == 0
    assert capture["executable_sha256"].lower() == EXE_SHA
    entries = {row["path"]: row for row in capture["blobs"]}
    def blob(name):
        raw = (capture_path.parent / name).read_bytes()
        assert len(raw) == entries[name]["size"] and sha(raw) == entries[name]["sha256"].lower()
        return raw
    store = blob("candidates/138-store-prefix.bin")
    assert sha(store) == "4158a1a5d793de485522c4ce0564ba4c1f7ca9796e80f108488e337476b0fcbf"
    assert store[:64] == blob("candidates/138-store-header.bin")
    count = struct.unpack_from("<I", store, 4)[0]
    assert count == 91
    rows = [store[8+i*0xD8:8+(i+1)*0xD8] for i in range(count)]
    keys = [struct.unpack_from("<I", row, 4)[0] for row in rows]
    assert len(set(keys)) == count
    # Hash metadata proves +4 is the key and every 0xD8 row is in bounds.
    hashes = blob("candidates/138-hash-entries.bin")
    indexed = {key: index for key, index in struct.iter_unpack("<II", hashes) if index < count}
    assert indexed == {key: index for index, key in enumerate(keys)}
    # The complete adjacent +0x140 store is inside the bounded +0x138 capture.
    offset = int(entries["candidates/140-store-header.bin"]["address"], 16) - int(entries["candidates/138-store-prefix.bin"]["address"], 16)
    assert store[offset:offset+64] == blob("candidates/140-store-header.bin")
    growth_count = struct.unpack_from("<I", store, offset+4)[0]
    assert growth_count == 501 and offset+8+growth_count*10 <= len(store)
    growth = [list(struct.unpack_from("<5H", store, offset+8+i*10)) for i in range(growth_count)]
    from loadout_handoff_code import verify_reviewed_code
    runtime_hash = verify_reviewed_code(runtime_path, EXE_SHA, {
        0x3FA77B: "4869c8d8000000", 0x3FAB0E: "b9ba380000", 0x2FA524: "f7411800002000"})
    text_raw = text_path.read_bytes()
    texts = json.loads(text_raw)
    assert texts["status"] == "complete_native_text_tree_capture" and texts["read_only"] and texts["game_writes"] == 0
    assert texts["executable_sha256"].lower() == EXE_SHA
    labels = {int(key): value["text"] for key, value in texts["entries"].items()}
    tables = load_default_effect_generation_tables()
    groups = {key: tables.resource.table("effect_group").row(group.row_index) for key, group in tables.groups_by_key.items()}
    # Native 0x6D2DA8/0x3FCB38 supplies the weapon-family name for +0x20.
    weapons = blob("candidates/300-store-prefix.bin")
    weapon_count = struct.unpack_from("<I", weapons, 4)[0]
    assert 8+weapon_count*0x80 <= len(weapons)
    weapon_hash = {key: index for key, index in struct.iter_unpack("<II", blob("candidates/300-hash-entries.bin")) if index < weapon_count}
    assert len(weapon_hash) == weapon_count
    weapon_labels = {key: labels[struct.unpack_from("<I", weapons, 8+index*0x80+0x1C)[0]] for key, index in weapon_hash.items()}
    item_rows = {struct.unpack_from("<H", row, 0x152)[0]: row for row in tables.resource.table("item").rows()}
    marker_groups = {key: struct.unpack_from("<I", raw, 8)[0] for key, raw in groups.items() if struct.unpack_from("<I", raw, 8)[0] in keys}
    markers = {str(id): marker_groups[effect.group_key] for id, effect in tables.effects_by_id.items() if effect.group_key in marker_groups}
    set_names = {set_key: labels.get(struct.unpack_from("<I", groups[group], 0x2C)[0]) for group, set_key in marker_groups.items()}
    result = {}
    unit_templates = {}
    for key, row in zip(keys, rows):
        tiers = []
        for index in range(7):
            required, flags, group, mode, base, step = struct.unpack_from("<6I", row, 0x30+index*0x18)
            if not group:
                continue
            assert group in groups and flags in (0, 2) and mode in (0, 1, 2) and required in range(1, 8)
            label = labels.get(struct.unpack_from("<I", groups[group], 0x2C)[0])
            assert label is not None, (key, group)
            for position in (0x38, 0x3C):
                argument = labels.get(struct.unpack_from("<I", groups[group], position)[0])
                if argument:
                    label = label.replace("{}", argument, 1)
            weapon_key = struct.unpack_from("<I", groups[group], 0x20)[0]
            if weapon_key and "{}" in label:
                label = label.replace("{}", weapon_labels[weapon_key], 1)
            item_key = struct.unpack_from("<I", groups[group], 0x18)[0]
            if item_key and "{}" in label:
                item = item_rows[item_key]
                assert struct.unpack_from("<I", item, 0x74)[0] == 0
                label = label.replace("{}", labels[struct.unpack_from("<I", item, 0x68)[0]], 1)
            label = re.sub(r"\^09~(?:BUFF|DEBUFF)~", "", label).replace("^09~~", "")
            unit_id = struct.unpack_from("<I", groups[group], 0x34)[0]
            unit_templates[str(group)] = labels[unit_id] if unit_id else ""
            tiers.append({"index": index, "required": required, "cross_style": bool(flags & 2),
                          "group_id": group, "label_zh": label, "label_resolved": "{}" not in label,
                          "value_mode": mode, "base_raw": base, "growth_raw": step})
        result[str(key)] = {"key": key, "name_zh": set_names.get(key),
                            "members": [id for id in struct.unpack_from("<7I", row, 0x14) if id], "tiers": tiers}
    out = {"schema": "nioh3-loadout-set-catalog/v1", "game_version": "PC v2.02", "executable_sha256": EXE_SHA,
           "sources": {"set_store_sha256": sha(store), "weapon_store_sha256": sha(weapons), "native_text_sha256": sha(text_raw), "runtime_text_sha256": runtime_hash},
           "native_contract": {"set_manager_offset": "0x138", "growth_manager_offset": "0x140", "row_stride": "0xD8",
                               "reduction_group": 0x38BA, "equipment_level_cap": 180, "secondary_guardian_group": 0x8508,
                               "consumers_rva": ["0x3FA34C", "0x3FA6D4", "0x3FAA98", "0x3FA13C", "0x3FA150", "0x5557DC", "0x55764C", "0x2FA524", "0x1BBE2B0", "0x6D2DA8", "0x3FCB38", "0x34E3B8"]},
           "effect_to_set": markers, "sets": result, "growth_by_level": growth, "unit_templates": unit_templates}
    destination = ROOT / "apps/workshop/loadout-sets.json"
    destination.write_text(json.dumps(out, ensure_ascii=False, indent=2)+"\n", encoding="utf-8", newline="\n")
    print(f"Exported {len(result)} native set definitions, {len(markers)} markers, {sum(len(row['tiers']) for row in result.values())} tiers and {len(growth)} growth rows")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--native-text", type=Path, required=True)
    parser.add_argument("--runtime-sections", type=Path, required=True)
    args = parser.parse_args()
    export(args.capture, args.native_text, args.runtime_sections)
