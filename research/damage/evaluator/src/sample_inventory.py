"""Merge the test backpack and players' saves into one inventory, so the path compiler sees what real backpacks hold.

Saves' item records are the live pool's rows. Writes INVENTORY.json + INVENTORY_F0.bin for path_outfits.py, and spaces.json: each save's items per
slot as (start, count) in the order path_outfits.py flattens them, so a GPU run can search one player's backpack.
argv: base_inventory_dir out_dir names.json class tag=decrypted_save...
"""
import collections
import json
import struct
import sys
from pathlib import Path

ITEM, ROW = 0xE8, 0xF0
PART = {"头部": 4, "身体": 5, "手臂": 6, "腿部": 7, "足部": 8}
base, out, names_path, kind = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4]
out.mkdir(parents=True, exist_ok=True)
names = json.loads(names_path.read_bytes())
catalog = json.loads((base / "INVENTORY.json").read_bytes())
records = next(v for v in catalog.values() if isinstance(v, list) and len(v) > 100)
f0 = (base / "INVENTORY_F0.bin").read_bytes()
by_slot = {slot: [] for slot in PART.values()}
for r in records:
    if r["kind"] == "armor" and r.get("physical_armor_slot") is not None:
        by_slot[r["physical_armor_slot"]].append(f0[r["F0_offset"]:r["F0_offset"] + ITEM])
own = {slot: len(rows) for slot, rows in by_slot.items()}
marks = {}
for pair in sys.argv[5:]:
    tag, path = pair.split("=", 1)
    d = Path(path).read_bytes()

    def is_item(at):
        key = struct.unpack_from("<H", d, at)[0]
        return str(key) in names and struct.unpack_from("<H", d, at + 2)[0] == key and d[at + 0x30] <= 5

    phase = collections.Counter(at % ROW for at in range(0x200000, 0x320000) if is_item(at)).most_common(1)[0][0]
    marks[tag] = {}
    found = {slot: [] for slot in PART.values()}
    for at in range(0x200000 + (phase - 0x200000) % ROW, 0x320000, ROW):
        if is_item(at):
            n = names[str(struct.unpack_from("<H", d, at)[0])]
            if n[1] == "防具" and n[2] == kind and n[3] in PART:
                found[PART[n[3]]].append(d[at:at + ITEM])
    for slot, rows in found.items():
        marks[tag][slot] = (len(by_slot[slot]), len(rows))
        by_slot[slot].extend(rows)
blob, rows, starts, flat = bytearray(), [], {}, 0
for slot in sorted(by_slot):
    starts[slot] = flat
    flat += len(by_slot[slot])
    for item in by_slot[slot]:
        rows.append({"F0_offset": len(blob), "kind": "armor", "physical_armor_slot": slot})
        blob += item + bytes(ROW - ITEM)
rows += [{"F0_offset": 0, "kind": "other"}] * max(0, 101 - len(rows))
(out / "INVENTORY_F0.bin").write_bytes(blob)
(out / "INVENTORY.json").write_text(json.dumps({"schema": "merged", "records": rows}), encoding="utf-8")
spaces = {"own": {slot: (starts[slot], own[slot]) for slot in by_slot}}
for tag, per in marks.items():
    spaces[tag] = {slot: (starts[slot] + at, n) for slot, (at, n) in per.items()}
(out / "spaces.json").write_text(json.dumps(spaces, indent=1), encoding="ascii")
print({tag: [v[s][1] for s in sorted(v)] for tag, v in spaces.items()})
