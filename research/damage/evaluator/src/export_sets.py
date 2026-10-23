"""Export every armor/accessory set: name (membership effect), and per reward row the piece threshold and effects."""
import json
import struct
import sys
from pathlib import Path

P = Path("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021")
sys.path.insert(0, str(P / "src"))
sys.path.insert(0, str(P))
from outfit_model import OutfitModel  # noqa: E402
from regress_fast import BASE, CACHE  # noqa: E402
from zone_table import NAMES, effect_names  # noqa: E402

m = OutfitModel(CACHE, BASE / "capture.json", observe_only=True)
T = m.T
from side_sources import table  # noqa: E402
side = table()
group_names = effect_names(m, NAMES)
loc = json.loads(Path("F:/Nioh3_ScrollEditor/nioh3_scroll_editor/data/effect_names_multilingual.json").read_bytes())["effects"]


def zh(eid):
    return loc.get(f"0x{eid:08X}", {}).get("names", {}).get("zh-CN")


group_zh = {}
for eid in T.EFFECTS.indices:
    g = struct.unpack_from("<H", T.EFFECTS.row(eid), 2)[0]
    n = zh(eid)
    if g and n and "^" not in n:
        group_zh.setdefault(g, set()).add(n)
# membership effects: effect -> group -> side key
members = {}
for eid in T.EFFECTS.indices:
    group = struct.unpack_from("<H", T.EFFECTS.row(eid), 2)[0]
    if group and group in T.GROUPS.indices:
        key = struct.unpack_from("<I", T.GROUPS.row(group), 8)[0]
        if key:
            members.setdefault(key, set()).add(zh(eid) or hex(eid))
sets = []
for key in sorted(side.indices):
    row = side.row(key)
    rewards = []
    for i in range(7):
        r = row[0x30 + 24 * i:0x48 + 24 * i]
        threshold, flags, group = struct.unpack_from("<I", r, 0)[0], r[4], struct.unpack_from("<H", r, 8)[0]
        if group == 0:
            continue
        rewards.append({"pieces": threshold, "group": hex(group), "flags": flags, "mode": struct.unpack_from("<I", r, 12)[0],
                        "base": struct.unpack_from("<I", r, 16)[0], "scale": struct.unpack_from("<i", r, 20)[0],
                        "effects": sorted(group_zh.get(group, set()) | set(group_names.get(group, [])))[:4]})
    sets.append({"side_key": hex(key), "names": sorted(members.get(key, [])), "rewards": rewards})
out = Path("F:/Nioh3_r16_build/work/sets/SETS.json")
out.parent.mkdir(exist_ok=True)
out.write_text(json.dumps(sets, ensure_ascii=False, indent=1), encoding="utf-8")
lines = []
for s in sets:
    if not s["rewards"]:
        continue
    lines.append(f"{s['side_key']} {' / '.join(s['names'][:3])}")
    for r in s["rewards"]:
        lines.append(f"    {r['pieces']} pieces (flags {r['flags']}): {' / '.join(r['effects']) or r['group']}")
out.with_suffix(".txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(len(sets), "sets;", sum(bool(s["rewards"]) for s in sets), "with rewards")
