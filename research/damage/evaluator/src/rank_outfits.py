"""Damage of real backpack armor (single pieces or outfits) for one captured hit, via outfit_model + native MAIN.

Per candidate: outfit_model predicts every STAT/D word change from the baseline capture and the records; the
changes are written into the baseline filled request, and demand_fill_loop.py --offline replays MAIN. A replay
that needs memory outside the request, or a model Unknown, makes the candidate UNKNOWN (never zero).
argv: <cache_dir> <baseline_capture> <baseline_filled_request> <inventory_dir> <out_dir> <retained FILLED_CELLS.json>
      [--outfit name=F0off,F0off,...]... [--only key,key] [--limit N] [--buff 1.1]
"""
import argparse
import json
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
from outfit_model import OutfitModel, Unknown  # noqa: E402

p = argparse.ArgumentParser()
for name in ("cache", "capture", "request", "inventory", "out", "retained"):
    p.add_argument(name, type=Path)
p.add_argument("--outfit", action="append", default=[])
p.add_argument("--only", default="")
p.add_argument("--limit", type=int, default=0)
p.add_argument("--buff", type=float, default=None, help="owner +0x2E3C class0 aggregate overlay")
p.add_argument("--jobs", type=int, default=6)
p.add_argument("--weapon-kind", type=int, default=None,
               help="rank backpack weapons whose item row u8 +0x183 equals this kind, each placed in slot 0")
p.add_argument("--weapon-type", default=None,
               help="also require this catalog weapon type (item_names.json, e.g. 刀); +0x183 groups 刀 with 忍刀")
p.add_argument("--names", type=Path, default=Path("F:/Nioh3_r16_build/work/item_names.json"))
p.add_argument("--extra-cells", type=Path, action="append", default=[], help="static-resource cell files")
args = p.parse_args()

model = OutfitModel(args.cache, args.capture)
NAMES = json.loads(args.names.read_bytes()) if args.names.exists() else {}
from demand_fill_loop import STATIC_TABLE_READERS  # noqa: E402  (after prepare: same sys.path)

template = json.loads(args.request.read_bytes())
have = {(r["address_u64"], len(r["value_hex"])) for r in template["scene"]["memory_regions"]}
for cell in json.loads(args.retained.read_bytes()):
    if cell["provenance"]["demand_reader_rva"].split(":")[0] in STATIC_TABLE_READERS and \
            (cell["address_u64"], len(cell["value_hex"])) not in have:
        template["scene"]["memory_regions"].append(cell)
import bisect  # noqa: E402
spans = sorted((r["address_u64"], r["address_u64"] + len(r["value_hex"]) // 2) for r in template["scene"]["memory_regions"])
starts = [s for s, _ in spans]
for path in args.extra_cells:  # static-resource reconstructions (reconstruct_grade_table.py); never overlap captured bytes
    for cell in json.loads(path.read_bytes()):
        a, data = cell["address_u64"], bytes.fromhex(cell["value_hex"])
        k = bisect.bisect_right(starts, a + len(data)) - 1
        covered = set()
        while k >= 0 and spans[k][1] > a - 0x10000:
            covered.update(range(max(a, spans[k][0]), min(a + len(data), spans[k][1])))
            k -= 1
        run = []
        for i in range(len(data) + 1):  # emit maximal runs of bytes no captured region covers
            if i < len(data) and a + i not in covered:
                run.append(i)
                continue
            if run:
                piece = data[run[0]:run[-1] + 1]
                template["scene"]["memory_regions"].append({**cell, "name": cell["name"] + f"+{run[0]}",
                                                            "address_u64": a + run[0], "value_hex": piece.hex(),
                                                            "known_hex": "ff" * len(piece)})
                run = []
owner_addr = model.R["source.HP_owner"][0]


def write(regions, address, data):
    hit = 0
    for r in regions:
        start = r["address_u64"]
        if not (start <= address + len(data) and address < start + len(r["value_hex"]) // 2):
            continue
        value, known = bytearray.fromhex(r["value_hex"]), bytearray.fromhex(r["known_hex"])
        for i, b in enumerate(data):
            at = address + i - start
            if 0 <= at < len(value):
                value[at], known[at] = b, 0xFF
                hit += 1
        r["value_hex"], r["known_hex"] = value.hex(), known.hex()
    return hit


catalog = json.loads((args.inventory / "INVENTORY.json").read_bytes())
records = next(v for v in catalog.values() if isinstance(v, list) and len(v) > 100)
f0 = (args.inventory / "INVENTORY_F0.bin").read_bytes()
armor = [r for r in records if r["kind"] == "armor"]
by_offset = {r["F0_offset"]: r for r in armor}
candidates = []
for spec in args.outfit:
    name, offs = spec.split("=", 1)
    # "F0off" for armor (slot from the catalog) or "F0off@slot" for any record, e.g. a weapon in slot 0
    every = {r["F0_offset"]: r for r in records}
    rows = [by_offset[int(o, 16)] if "@" not in o else dict(every[int(o.split("@")[0], 16)], physical_armor_slot=int(o.split("@")[1]))
            for o in offs.split(",") if o]
    assert len({r["physical_armor_slot"] for r in rows}) == len(rows), "one record per slot"
    candidates.append((name, rows))
if not args.outfit:
    pool = armor
    if args.only:
        keys = {int(k, 16) for k in args.only.split(",")}
        pool = [r for r in pool if r["item_id_u16"] in keys]
    candidates = [(f"{n:03d}_s{r['physical_armor_slot']}_{r['item_id_u16']:04x}_{r['F0_offset']:x}", [r]) for n, r in enumerate(pool)]
if args.weapon_kind is not None:
    T = model.T
    weapons = [dict(r, physical_armor_slot=0) for r in records if r["kind"] == "native_dispatch_1"
               and r["item_id_u16"] in T.ITEMS.indices and T.ITEMS.row(r["item_id_u16"])[0x183] == args.weapon_kind]
    if args.weapon_type:
        names = json.loads(args.names.read_bytes())
        weapons = [r for r in weapons if names.get(str(r["item_id_u16"]), [None] * 4)[3] == args.weapon_type]
    current = model.inv[model.active * 0xF70:][:0xE8]
    candidates = [("control_current_weapon", [{"F0_offset": None, "physical_armor_slot": 0, "item_id_u16": struct.unpack_from("<H", current)[0],
                                               "level_raw": None, "plus_raw": None, "rarity_raw": current[0x30], "_record": current}])]
    candidates += [(f"w{n:03d}_{r['item_id_u16']:04x}_{r['F0_offset']:x}", [r]) for n, r in enumerate(weapons)]
if args.limit:
    candidates = candidates[:args.limit]
args.out.mkdir(parents=True, exist_ok=True)
results = []
for tag, rows in candidates:
    item = {"tag": tag, "pieces": [{"slot": r["physical_armor_slot"], "item_key": hex(r["item_id_u16"]), "F0_offset": hex(r["F0_offset"]) if r["F0_offset"] is not None else None,
                                    "level": r["level_raw"], "plus": r["plus_raw"], "rarity": r["rarity_raw"]} for r in rows]}
    for piece in item["pieces"]:  # catalog display name, for the owner to find the item in game
        piece["name"] = NAMES.get(str(int(piece["item_key"], 16)), [None])[0]
    run_dir = args.out / f"{tag}.run"
    try:
        pred = model.predict([(r.get("_record") or f0[r["F0_offset"]:r["F0_offset"] + 0xE8], r["physical_armor_slot"]) for r in rows])
    except (Unknown, ValueError, KeyError) as error:
        item.update(damage=None, status="UNKNOWN", unknown_reason={"model": str(error)[:400]})
        results.append(item)
        print(json.dumps({"tag": tag, "status": "UNKNOWN", "reason": str(error)[:160]}), flush=True)
        continue
    request = json.loads(json.dumps(template))
    regions = request["scene"]["memory_regions"]
    for name, address, actual in (("stat", model.stat_addr, model.stat), ("d", model.d_addr, model.d)):
        for at in pred[name + "_changed_words"]:
            write(regions, address + at, pred[name][at:at + 4])
    # The attack packet is built from STAT record 0 when the swing starts (builder 2B30A0 / COPY 78A194), so a
    # changed weapon record must also reach the packet. Fields MAIN reads, each equal in all 10 captured hits:
    # +8 = record0+0 + record0+8, +C = record0+C, +14 = record0+18.
    if any(0x24 <= a < 0x64 for a in pred["stat_changed_words"]):
        r_old, r_new = model.stat[0x24:0x64], pred["stat"][0x24:0x64]
        u = lambda b, o: struct.unpack_from("<I", b, o)[0]
        main = model.main_addr
        packet = {8: (u(model.main, 8) + (u(r_new, 0) + u(r_new, 8)) - (u(r_old, 0) + u(r_old, 8))) & 0xFFFFFFFF,
                  0xC: u(r_new, 0xC), 0x14: u(r_new, 0x18)}
        for off, value in packet.items():
            assert write(regions, main + off, struct.pack("<I", value)) == 4
        item["packet_changes"] = {hex(o): [u(model.main, o), v] for o, v in packet.items() if u(model.main, o) != v}
    if args.buff is not None:
        assert write(regions, owner_addr + 0x2E3C, struct.pack("<f", args.buff)) == 4
    stat_new = {hex(a): [struct.unpack_from("<I", model.stat, a)[0], struct.unpack_from("<I", pred["stat"], a)[0]]
                for a in pred["stat_changed_words"]}
    item.update(weight=struct.unpack_from("<I", pred["stat"], 0x7B8)[0], stat_changes=stat_new,
                d_changed_words=[hex(a) for a in pred["d_changed_words"]], approximate_words=pred["approximate_words"],
                set_rewards=pred["set_rewards"], requirement_gates=pred["requirement_gates"])
    item["_run_dir"], item["_request"] = run_dir, json.dumps(request)  # kept serialized: about 2 MB each
    results.append(item)


def replay(item):
    run_dir = item.pop("_run_dir", None)
    request = item.pop("_request", None)
    if run_dir is None:
        return item
    if not run_dir.exists():
        req_path = args.out / (item["tag"] + ".request.json")
        req_path.write_text(request + "\n")
        subprocess.run([sys.executable, str(HERE / "demand_fill_loop.py"), "--capture", str(args.capture), "--cache-dir",
                        str(args.cache), "--output-dir", str(run_dir), "--offline", "--iterations", "2",
                        "--resume-request", str(req_path)], capture_output=True, text=True)
        req_path.unlink()
    loop = [json.loads(x) for x in (run_dir / "LOOP.jsonl").read_text().splitlines()] if (run_dir / "LOOP.jsonl").exists() else []
    last = loop[-1] if loop else {}
    damage = last.get("damage") if last.get("normal_return") else None
    item.update(damage=damage, status="computed" if damage is not None else "UNKNOWN",
                unknown_reason=None if damage is not None else {"replay": {"missing": last.get("missing"), "next_rva": last.get("next_rva")}})
    print(json.dumps({"tag": item["tag"], "weight": item["weight"], "damage": damage, "status": item["status"],
                      "set": bool(item["set_rewards"]), "approx": item["approximate_words"]}), flush=True)
    return item


from concurrent.futures import ThreadPoolExecutor  # noqa: E402
with ThreadPoolExecutor(max_workers=args.jobs) as pool:
    results = list(pool.map(replay, results))
(args.out / "RANKING.json").write_text(json.dumps({
    "model": "outfit_model (body 2FB2BC + collectors + set expansion 3FAA98 + load factor + materializer) + native MAIN offline",
    "scope": "baseline no-armor capture (PID 53660), katana mid-stance first light hit, training target",
    "buff_aggregate": args.buff,
    "not_modelled": ["slot15 stage", "set-requested refresh pass (reported UNKNOWN)", "context deltas 8175F4",
                     "guardian/passive stages beyond baseline", "ownership/legality"],
    "results": results}, indent=1) + "\n")
