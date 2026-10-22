"""Read the live equipped banks (read-only) at the address a capture of the same process recorded, into an inventory dir.

Waits until the active bank's slot-15 key differs from <old_key_hex> (the owner swapped the scroll), then writes the
active bank's item records like bank_inventory.py does.
argv: <port> <session> <pid> <capture_of_same_pid> <old_slot15_key_hex> <out_dir>
"""
import json
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, "F:/Nioh3_ScrollEditor/.tools/cheat-engine-mcp/python/src")
sys.path.insert(0, str(Path(__file__).parent))
from ce_mcp_server.bridge import CheatEngineBridge  # noqa: E402
from outfit_model import roles  # noqa: E402

PORT, SESSION, PID, CAP, OLD, OUT = int(sys.argv[1]), sys.argv[2], int(sys.argv[3]), sys.argv[4], int(sys.argv[5], 16), Path(sys.argv[6])
address, old_banks, _ = roles(CAP)["source.two_banks"]
OLD_RECORD = old_banks[struct.unpack_from("<I", old_banks, 0x1EE0)[0] * 0xF70 + 15 * 0xE8:][:0xE8]
names = json.loads(Path("F:/Nioh3_r16_build/work/item_names.json").read_bytes())
bridge = CheatEngineBridge(host="127.0.0.1", port=PORT)
bridge.start()
try:
    for _ in range(60):
        if any(s.get("session_id") == SESSION for s in bridge.list_sessions()):
            break
        time.sleep(1)

    def read(at, n):
        r = bridge.call_tool("ce.lua_exec", {"script": "local t=readBytes(%d,%d,true); local h={} for i,b in ipairs(t or {}) do "
                                             "h[i]=string.format('%%02x',b) end; return {pid=getOpenedProcessID(),hex=table.concat(h)}" % (at, n)},
                             SESSION, 30.0)
        v = r.get("result", r)
        assert v.get("pid") == PID, "CE not attached to the game"
        return bytes.fromhex(v["hex"])

    for _ in range(600):
        inv = read(address, 0x2018)
        active = struct.unpack_from("<I", inv, 0x1EE0)[0]
        slot15 = inv[active * 0xF70 + 15 * 0xE8:][:0xE8]
        key = struct.unpack_from("<H", slot15)[0]
        # A different copy of the same scroll keeps the key; compare the whole record with the old one.
        if key and slot15 != OLD_RECORD and read(address, 0x2018) == inv:
            break
        time.sleep(2)
    else:
        raise SystemExit("slot 15 did not change")
    f0, records = bytearray(), []
    for slot in range(17):
        rec = inv[active * 0xF70 + slot * 0xE8:][:0xE8]
        k = struct.unpack_from("<H", rec)[0]
        if k:
            records.append({"F0_offset": len(f0), "item_id_u16": k, "kind": "bank", "physical_armor_slot": slot,
                            "level_raw": None, "plus_raw": None, "rarity_raw": rec[0x30], "bank_slot": slot,
                            "name": (names.get(str(k)) or [None])[0]})
            f0 += rec + b"\0" * 8
    pad = [dict(records[0], F0_offset=None, kind="pad") for _ in range(101)]
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "INVENTORY_F0.bin").write_bytes(bytes(f0))
    (OUT / "INVENTORY.json").write_text(json.dumps({"records": records + pad}, ensure_ascii=False), encoding="utf-8")
    print(json.dumps([(r["bank_slot"], hex(r["F0_offset"]), hex(r["item_id_u16"]), r["name"]) for r in records], ensure_ascii=False))
finally:
    bridge.stop()
