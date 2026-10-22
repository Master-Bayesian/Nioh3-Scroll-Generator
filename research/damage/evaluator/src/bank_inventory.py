"""Inventory dir holding the item records of a capture's active bank (item bytes only), for rank_outfits '@slot' specs."""
import json
import struct
import sys
from pathlib import Path
sys.path.insert(0, "F:/Nioh3_r16_build/tools/src")
from outfit_model import roles

cap, out = sys.argv[1], Path(sys.argv[2])
inv = roles(cap)["source.two_banks"][1]
active = struct.unpack_from("<I", inv, 0x1EE0)[0]
f0, records = bytearray(), []
for slot in range(17):
    rec = inv[active * 0xF70 + slot * 0xE8:][:0xE8]
    key = struct.unpack_from("<H", rec)[0]
    if key:
        records.append({"F0_offset": len(f0), "item_id_u16": key, "kind": "bank", "physical_armor_slot": slot,
                        "level_raw": None, "plus_raw": None, "rarity_raw": rec[0x30], "bank_slot": slot})
        f0 += rec + b"\0" * 8
pad = [dict(records[0], F0_offset=None, kind="pad") for _ in range(101)]  # rank_outfits finds the list with > 100 rows
out.mkdir(parents=True, exist_ok=True)
(out / "INVENTORY_F0.bin").write_bytes(bytes(f0))
(out / "INVENTORY.json").write_text(json.dumps({"records": records + pad}), encoding="utf-8")
print([(r["bank_slot"], hex(r["F0_offset"])) for r in records])
