"""Attack and defense terms of every saved samurai capture: which part is outside the weapon stages, and is it per state?

captured record = modelled weapon stages (weapon, scaling, scroll) + player base. The base must not depend on gear.
Attribute seeds, load factor and the +8 field are compared with the model too. argv: out.json
"""
import glob
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from composed_model import ComposedModel, MAPS  # noqa: E402

CACHE = "F:/Nioh3_r16_build/cache"
paths = sorted(p.replace("\\", "/") for p in glob.glob("F:/Nioh3_r16_build/work/**/capture.json", recursive=True))
paths.append("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021/inputs/baseline-1016/capture.json")
rows, seen = [], set()
ATTACK_GROUP = 0x6A7C
for path in paths:
    try:
        m = ComposedModel(CACHE, path)
    except Exception:
        continue
    if bytes(m.inv) in seen:
        continue
    seen.add(bytes(m.inv))
    import base_prefix as BP
    import guardian_stage as G
    try:
        source = m.B.typed_source(m.attr, m.attr_known, m.mode)
        source["map_35d0"], source["map_4728"] = [], []
        source["side_state"]["banks"] = [[], []]
        stat = bytearray(m.stat)
        struct.pack_into("<Q", stat, 8, 0)
        struct.pack_into("<Q", stat, 0x7D8, m.N.ATTR)
        stat[0x7D0] = 0
        source, _ = m._prepared(stat, m.inv, source)
        if struct.unpack_from("<H", m.inv, m.active * 0xF70 + 15 * 0xE8)[0]:
            m._slot15(stat, m.inv, source)
    except Exception as e:
        print("failed", path[-40:], str(e)[:150])
        continue
    u = lambda b, o: struct.unpack_from("<I", b, o)[0]
    aggregate = {e["key_u16"]: e["base_u32"] + e["duplicate_u16"] for e in m.captured_source[MAPS[m.mode]]}
    seeds = BP.U.stat_seeds(m.raw_attrs, m.sync_attrs, m.mode)
    c = m.attr
    guardians = [struct.unpack_from("<IHH", c, 0x58 + i * 8) for i in range(2)]
    guardian = guardians[c[0x70]]
    numeric = G.numeric(guardian[0], guardian[1])
    weapon = struct.unpack_from("<H", m.inv, m.active * 0xF70)[0]
    row = {"path": path[len("F:/Nioh3_r16_build/"):], "form": m.mode, "attrs": m.raw_attrs, "weapon": hex(weapon),
           "records": [u(m.stat, 0x24 + 64 * i) for i in range(6)], "model": [u(stat, 0x24 + 64 * i) for i in range(6)],
           "plus8": [u(m.stat, 0x2C + 64 * i) for i in range(6)], "attack_group": aggregate.get(ATTACK_GROUP), "stat12c": u(m.stat, 0x12C),
           "guardian_attack": numeric["attack_increment_u32"], "guardian_defense": numeric["defense_increment_u32"],
           "core_keys": [hex(struct.unpack_from("<H", c, 0x80 + s * 0xE8)[0]) for s in range(8)],
           "seed_ok": all(u(m.stat, int(k, 16)) == v for k, v in seeds["u32_fields"].items() if k != "0x1c4"),
           "seed1a8_ok": m.stat[0x1A8:0x1AC] == bytes.fromhex(seeds["f32_fields"]["0x1a8"]["bits"])[::-1],
           "loadfactor": [struct.unpack_from("<f", m.stat, 0x7A0)[0], m._load_factor(m.stat)],
           "defense": u(m.stat, 0x1C8)}
    row["base"] = row["records"][0] - row["model"][0]
    row["base_minus_guardian"] = row["base"] - row["guardian_attack"]
    rows.append(row)
    print(row["path"][:42], "form", m.mode, "w", row["weapon"], "rec0", row["records"][0], "model", row["model"][0], "base", row["base"], "-guardian", row["base_minus_guardian"],
          "| +8", row["plus8"][0], row["plus8"][2], "atk", row["attack_group"], "12c", row["stat12c"], "| seeds", row["seed_ok"], row["seed1a8_ok"], "| load %.5f %.5f" % tuple(row["loadfactor"]), "cores", row["core_keys"][:2], flush=True)
Path(sys.argv[1]).write_text(json.dumps(rows, ensure_ascii=False, indent=0), encoding="utf-8")
