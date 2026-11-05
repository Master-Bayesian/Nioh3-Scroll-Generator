"""All guardian definitions at one level from the game's own table: name, player attack/defense, effect entries.

Checked against four guardians read live on 11-05 (all five entries and the displayed 强化值). argv: out.json [level]
"""
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from composed_model import ComposedModel  # noqa: E402

CACHE = "F:/Nioh3_r16_build/cache"
HOME = Path(__file__).resolve().parents[2] / "data"
out = sys.argv[1]
level = int(sys.argv[2]) if len(sys.argv) > 2 else 50
ComposedModel(CACHE, "F:/Nioh3_r16_build/work/blind-1023/base/capture.json")
import guardian_stage as G  # noqa: E402

texts = json.loads(Path("F:/Nioh3_r16_build/work/live-1104/ALL_TEXTS.json").read_bytes())
gnames = json.loads(Path("F:/Nioh3_r16_build/work/live-1104/SOURCES.json").read_bytes())["gname"]
rows = []
for key in sorted(G.KEYS):
    row = G.definition(key)
    names = []
    for off in range(0, 0x100 - 3, 4):
        t = texts.get(hex(struct.unpack_from("<I", row, off)[0]))
        if t and len(t) < 16:
            names.append((hex(off), t))
    n = G.numeric(key, level)
    effects = []
    for i in range(6):
        d = G.descriptor(key, i)
        if d["key_u16"]:
            effects.append({"group": hex(d["key_u16"]), "name": gnames.get(hex(d["key_u16"])), "value": G.skill_value(key, i, level)[0],
                            "required": d["required_u16"]})
    rows.append({"key": hex(key), "names": names, "attack": n["attack_increment_u32"], "defense": n["defense_increment_u32"], "effects": effects})
    print(hex(key), names[:2], n["attack_increment_u32"], n["defense_increment_u32"], [(e["name"], e["value"]) for e in effects])
Path(out).write_text(json.dumps({"level": level, "rows": rows}, ensure_ascii=False, indent=0), encoding="utf-8")
