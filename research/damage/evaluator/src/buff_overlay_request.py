"""Buff overlay: set the source actor's class0 status aggregate (owner+0x2DC0+0x7C = +0x2E3C, f32) in a request.

The aggregate is where applied class0 statuses multiply their magnitude (e.g. Counterattack Deflect 0x1AE,
f32 1.1); MAIN folds it into the boost/cut pair natively, so no displayed-damage multiplication is done here.
argv: <request> <capture_for_roles> <out_request> <aggregate_f32>
"""
import json
import struct
import sys
from pathlib import Path

REQ, CAP, OUT, VALUE = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4])
raw = json.loads(Path(CAP).read_bytes())["raw_capture"]
entry = next(s for s in raw["snapshots"] if s["kind"] == "MAIN_entry_prestate")
owner = next(int(r["address_hex"], 16) for r in entry["regions"] if r["role"] == "source.HP_owner" and r.get("bytes_hex"))
request = json.loads(Path(REQ).read_bytes())
address, data, old = owner + 0x2E3C, struct.pack("<f", VALUE), None
for r in request["scene"]["memory_regions"]:
    at = address - r["address_u64"]
    if 0 <= at and at + 4 <= len(r["value_hex"]) // 2:
        value, known = bytearray.fromhex(r["value_hex"]), bytearray.fromhex(r["known_hex"])
        old = (value[at:at + 4].hex(), known[at:at + 4].hex())
        value[at:at + 4], known[at:at + 4] = data, b"\xff" * 4
        r["value_hex"], r["known_hex"] = value.hex(), known.hex()
assert old is not None, "aggregate not inside the request's captured regions"
Path(OUT).write_text(json.dumps(request) + "\n")
print(json.dumps({"address": hex(address), "old_value_known": old, "new": data.hex(), "f32": struct.unpack("<f", data)[0]}))
