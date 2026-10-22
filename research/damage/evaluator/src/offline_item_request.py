"""Offline item-swap request: baseline filled request + item record -> predicted request (no after-swap STAT/D values).

D comes from predict_swap_d.py (baseline + the item's affixes, retained materializer). STAT+0x7B8 (native equip
weight, v5 'weight_native') becomes baseline + the item's table weight: item table row (key = record u16 +0), and
when record rarity (u8 +0x30) >= 4 the row named by the original row's u16 +0x168 redirect (native F4DBC -> 8180AC);
weight is that row's u16 +0xA0 (native 818018..8180A9, pre-source, no skill weight deltas).
Retained static parameter-table cells from earlier same-PID fills are merged by reader RVA (STATIC_TABLE_READERS).
argv: <baseline_request> <baseline_capture> <predicted_D_bin> <record_capture> <record_offset_hex> <item_table_dir>
      <out_request> [<retained FILLED_CELLS.json> ...]
"""
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from demand_fill_loop import STATIC_TABLE_READERS  # noqa: E402

BASE_REQ, BASE_CAP, D_BIN, REC_CAP, REC_OFF, TABLE, OUT = sys.argv[1:8]
RETAINED = sys.argv[8:]
ROW, WEIGHT, REDIRECT, RARITY = 416, 0xA0, 0x168, 0x30

request = json.loads(Path(BASE_REQ).read_bytes())
regions = request["scene"]["memory_regions"]


def roles(path):
    raw = json.loads(Path(path).read_bytes())["raw_capture"]
    entry = next(s for s in raw["snapshots"] if s["kind"] == "MAIN_entry_prestate")
    out = {}
    for r in entry["regions"]:
        if r.get("bytes_hex"):
            out.setdefault(r["role"], (int(r["address_hex"], 16), bytes.fromhex(r["bytes_hex"])))
    return out


def write(address, data):
    done = 0
    for r in regions:
        start = r["address_u64"]
        value, known = bytearray.fromhex(r["value_hex"]), bytearray.fromhex(r["known_hex"])
        for i, b in enumerate(data):
            at = address + i - start
            if 0 <= at < len(value):
                value[at], known[at] = b, 0xFF
                done += 1
        r["value_hex"], r["known_hex"] = value.hex(), known.hex()
    return done


def item_weight(record):
    rows = (Path(TABLE) / "rows.bin").read_bytes()
    entries = (Path(TABLE) / "hash_entries.bin").read_bytes()
    index = {struct.unpack_from("<I", entries, i)[0]: struct.unpack_from("<I", entries, i + 4)[0]
             for i in range(0, len(entries), 8)}
    key, rarity = struct.unpack_from("<H", record, 0)[0], record[RARITY]
    row = index[key]
    selected = struct.unpack_from("<H", rows, row * ROW + REDIRECT)[0] if rarity >= 4 else key
    srow = index[selected]
    return struct.unpack_from("<H", rows, srow * ROW + WEIGHT)[0], {
        "item_key": hex(key), "rarity": rarity, "row": row, "selected_key": hex(selected), "selected_row": srow}


base = roles(BASE_CAP)
prefix = "source.external_STAT" if struct.unpack_from("<Q", base["source.HP_owner"][1], 0x2D90)[0] else "source.inline_STAT"
stat_addr, stat = base[prefix]
d_addr, d_old = base[prefix + ".D"]
record = roles(REC_CAP)["source.two_banks"][1][int(REC_OFF, 16):][:0xE8]

changes = []
d_new = Path(D_BIN).read_bytes()
for i in range(0, len(d_new), 4):
    if d_new[i:i + 4] != d_old[i:i + 4]:
        changes.append({"field": f"D+{i:#x}", "old": d_old[i:i + 4].hex(), "new": d_new[i:i + 4].hex(),
                        "source": "predict_swap_d (baseline + item affixes)", "bytes_written": write(d_addr + i, d_new[i:i + 4])})
weight, how = item_weight(record)
old_w = struct.unpack_from("<I", stat, 0x7B8)[0]
new_w = struct.pack("<I", old_w + weight)
changes.append({"field": "STAT+0x7b8", "old": stat[0x7B8:0x7BC].hex(), "new": new_w.hex(), "item_weight": weight,
                "source": "item table weight", **how, "bytes_written": write(stat_addr + 0x7B8, new_w)})

have = {(r["address_u64"], len(r["value_hex"])) for r in regions}
merged = []
for path in RETAINED:
    for cell in json.loads(Path(path).read_bytes()):
        reader = cell["provenance"]["demand_reader_rva"].split(":")[0]
        if reader in STATIC_TABLE_READERS and (cell["address_u64"], len(cell["value_hex"])) not in have:
            regions.append(cell)
            have.add((cell["address_u64"], len(cell["value_hex"])))
            merged.append({"address": hex(cell["address_u64"]), "reader": reader, "from": path})
Path(OUT + ".changes.json").write_text(json.dumps({"changes": changes, "merged_static_cells": merged}, indent=1) + "\n")
Path(OUT).write_text(json.dumps(request) + "\n")
print(json.dumps({"changes": changes, "merged_static_cells": len(merged)}, indent=1))
