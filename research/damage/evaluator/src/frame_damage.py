import json
import struct
import sys
from pathlib import Path

raw = json.loads(Path(sys.argv[1]).read_bytes())["raw_capture"]
for fr in raw["frames"]:
    hit = int(fr["hit"], 16)
    out = {}
    for snap in raw["snapshots"]:
        if snap.get("frame_id") not in (None, fr.get("id")):
            continue
        for r in snap["regions"]:
            if not r.get("bytes_hex") or "address_hex" not in r:
                continue
            a, b = int(r["address_hex"], 16), bytes.fromhex(r["bytes_hex"])
            for name, addr, fmt in (("damage", hit + 0x218, "<f"), ("attack", hit + 8, "<I"), ("stance", hit + 0x148, "<i")):
                if a <= addr and addr + 4 <= a + len(b):
                    out[(snap["kind"], name)] = struct.unpack(fmt, b[addr - a:addr - a + 4])[0]
            if r["role"] == "source.HP_owner" and snap["kind"] == "MAIN_entry_prestate":
                out[("entry", "aggregate_2E3C")] = struct.unpack_from("<f", b, 0x2E3C)[0]
    print(fr.get("id"), {f"{k[0]}.{k[1]}": v for k, v in out.items() if k[1] != "damage" or k[0] == "MAIN_final"})
