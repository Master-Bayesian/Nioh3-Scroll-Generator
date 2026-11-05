"""List weapon, attributes and attack fields of every saved capture, reading JSON only."""
import glob, json, struct
from pathlib import Path
seen = set()
for path in sorted(glob.glob("F:/Nioh3_r16_build/work/**/capture.json", recursive=True)) + ["F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021/inputs/baseline-1016/capture.json"]:
    if "split-" in path or "swap-" in path and False:
        pass
    try:
        raw = json.loads(Path(path).read_bytes())["raw_capture"]
        entry = next(s for s in raw["snapshots"] if s["kind"] == "MAIN_entry_prestate")
        R = {}
        for r in entry["regions"]:
            if r.get("bytes_hex") and r["role"] not in R:
                R[r["role"]] = (int(r["address_hex"], 16), bytes.fromhex(r["bytes_hex"]))
        owner = R["source.HP_owner"][1]
        pre = "source.external_STAT" if struct.unpack_from("<Q", owner, 0x2D90)[0] else "source.inline_STAT"
        stat = R[pre][1]; attr = R[pre + ".attribute_source"][1]; inv = R["source.two_banks"][1]
        active = struct.unpack_from("<I", inv, 0x1EE0)[0]
        key = (struct.unpack_from("<H", inv, active * 0xF70)[0], struct.unpack_from("<7i", attr, 0x14), struct.unpack_from("<4I", stat, 0x24)[:3], stat[0x24:0x2C] and struct.unpack_from("<H", inv, active * 0xF70 + 15 * 0xE8)[0], active)
        if key not in seen:
            seen.add(key)
            print(path[len("F:/Nioh3_r16_build/"):], "weapon", hex(key[0]), "attrs", key[1], "rec0", key[2], "scroll", hex(key[3]), "active", active)
    except Exception as e:
        print("skip", path[-40:], type(e).__name__, str(e)[:60])
