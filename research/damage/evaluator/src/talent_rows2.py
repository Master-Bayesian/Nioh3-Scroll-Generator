"""Decode source7c0/source7c8 rows as source_talents.py reads them: key +8, group (+0xC / +0x18), value per rank."""
import json, struct, sys
from pathlib import Path
P = Path("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021")
sys.path.insert(0, str(P / "src")); sys.path.insert(0, str(P))
from outfit_model import OutfitModel  # noqa: E402
from regress_fast import BASE, CACHE  # noqa: E402
m = OutfitModel(CACHE, BASE / "capture.json", observe_only=True)
T = m.T
loc = json.loads(Path("F:/Nioh3_ScrollEditor/nioh3_scroll_editor/data/effect_names_multilingual.json").read_bytes())["effects"]
gname = {}
for eid in T.EFFECTS.indices:
    g = struct.unpack_from("<H", T.EFFECTS.row(eid), 2)[0]
    n = loc.get(f"0x{eid:08X}", {}).get("names", {}).get("zh-CN")
    if g and n and "^" not in n:
        gname.setdefault(g, n)
live = json.loads(Path(sys.argv[1]).read_bytes())
ranks = {lab: {s["key"]: s["rank"] for s in live[lab]["skills"]} for lab in ("map5d98", "map5de8")}
ids = {}
out = []
for family, keyoff, valoff, mp in (("source7c0", 0xC, 0x20, "map5d98"), ("source7c8", 0x18, 0x28, "map5de8")):
    t = live[family]
    rows, stride, count = bytes.fromhex(t["rows_hex"]), t["stride"], t["count"]
    for i in range(count):
        r = rows[8 + i * stride:8 + (i + 1) * stride]
        key, group = struct.unpack_from("<I", r, 8)[0], struct.unpack_from("<H", r, keyoff)[0]
        rank = ranks[mp].get(key, 0)
        values = [struct.unpack_from("<I", r, valoff + 8 * s)[0] for s in range(10) if valoff + 8 * s + 4 <= stride]
        tid = hex(struct.unpack_from("<I", T.GROUPS.row(group), 44)[0]) if group in T.GROUPS.indices else None
        ids[group] = tid
        out.append({"family": family, "row": i, "key": hex(key), "group": hex(group), "rank": rank,
                    "mode_u8": r[0x14] if family == "source7c8" else None, "values_by_rank": values,
                    "current_value": values[rank] if rank < len(values) else None, "name": gname.get(group), "text_id": tid})
Path(sys.argv[2]).write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")
json.dump(sorted({v for v in ids.values() if v}), open(sys.argv[3], "w"))
for o in out:
    print(o["family"], o["row"], o["group"], "rank", o["rank"], "now", o["current_value"], o["name"] or o["text_id"], o["values_by_rank"][:10])
