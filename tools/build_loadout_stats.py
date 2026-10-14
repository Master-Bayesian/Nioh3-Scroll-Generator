"""Project reviewed armor getters and revalidate the captured capacity table."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--armor-root", type=Path, required=True)
    parser.add_argument("--capacity-root", type=Path, required=True)
    parser.add_argument("--validation-output", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    armor_module = load_module(args.armor_root / "tools/armor_remodel_batch_v202.py", "reviewed_armor_stats")
    model = armor_module.ArmorModel()
    validation = model.validate_live()
    exporter = load_module(args.capacity_root / "scripts/export_capacity_table.py", "reviewed_capacity_export")
    captures = args.capacity_root / "captures"
    sys.argv = [str(exporter.__file__), "--table", str(captures / "20261008-5-samurai-attribute-table.json"),
                "--native", str(captures / "20261008-5-native-capacity-final-r2.json"),
                "--recovery", str(captures / "20261008-5-native-capacity-final-r2-recovery.json"),
                "--output", str(args.validation_output)]
    exporter.main()
    capacity = json.loads(args.validation_output.read_text())
    assert capacity["executable_sha256"].upper() == armor_module.EXE_SHA
    assert capacity["source_table_sha256"] == "b77f17ee934768cc915e57f98a809f0c99c0d79b37b7364386a1cff2358f49f7"
    item_path = args.armor_root / "nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables/item.bin"
    raw = item_path.read_bytes()
    items = {}
    for key, item in sorted(model.armor.items()):
        alternate = model.items.get(item.alternate_id, item)
        flag = struct.unpack_from("<I", raw, 8 + item.index * 416 + 0xB0)[0]
        cold_index = model.cold_index(item.cold_key)
        items[str(key)] = [item.weight, alternate.weight, item.toughness, alternate.toughness,
                           cold_index if cold_index is not None else -1, bool(flag & 0x100)]
    accessories = [item for key, item in model.items.items() if model.names.get(str(key), ["", ""])[1] == "饰品"]
    assert accessories and all(item.toughness == 0 for item in accessories)
    for item in accessories:
        alternate = model.items.get(item.alternate_id, item)
        assert alternate.toughness == 0
        items[str(item.item_id)] = [item.weight, alternate.weight, 0, 0, -1, False]
    asset = {"schema": "nioh3-loadout-stats/v1", "game_version": "2.0.2.0",
             "armor_provenance": model.provenance, "armor_validation": validation,
             "capacity_table_sha256": capacity["source_table_sha256"], "capacity_checks": capacity["checks"],
             "accessory_rows": len(accessories),
             "capacity_raw": {style: [row[f"{style}_raw_tenths"] for row in capacity["rows"][:150]]
                              for style in ("samurai", "ninja")},
             "stage_caps": list(armor_module.STAGE_CAPS.values()),
             "weight_coefficients": [row[1] for row in model.curve],
             "cold_rows": model.cold_rows, "items": items}
    args.output.write_text(json.dumps(asset, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"armor_rows": len(items), "capacity_rows": 150,
                      "sha256": hashlib.sha256(args.output.read_bytes()).hexdigest()}))


if __name__ == "__main__":
    main()
