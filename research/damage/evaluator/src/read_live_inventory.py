"""Read the live F0 item pool (read-only CE readBytes) into the INVENTORY.json/INVENTORY_F0.bin layout rank tools use.

Roots as in Codex's capture_full_inventory.py: manager = Q(base+0x4751530), data = Q(manager),
count = Q(data+0x927D0), pool = data+0x10 (stride 0xF0, E8 payload). Read twice; both passes must agree.
Classification uses the v2.01 catalog names (work/item_names.json): armor slot from the catalog body part, and
samurai/ninja from its class, because samurai and ninja equipment are not interchangeable.
argv: <port> <session_id> <pid> <out_dir>
"""
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, "F:/Nioh3_ScrollEditor/.tools/cheat-engine-mcp/python/src")
from ce_mcp_server.bridge import CheatEngineBridge  # noqa: E402

PORT, SESSION, PID, OUT = int(sys.argv[1]), sys.argv[2], int(sys.argv[3]), Path(sys.argv[4])
SLOT = {"头部": 4, "身体": 5, "手臂": 6, "腿部": 7, "足部": 8}
names = json.loads(Path("F:/Nioh3_r16_build/work/item_names.json").read_bytes())
bridge = CheatEngineBridge(host="127.0.0.1", port=PORT)
bridge.start()


def lua(script):
    r = bridge.call_tool("ce.lua_exec", {"script": script}, SESSION, 60.0)
    return r.get("result", r)


def read(address, size):
    out = b""
    for at in range(address, address + size, 0x10000):
        n = min(0x10000, address + size - at)
        v = lua("local t=readBytes(%d,%d,true); local h={} for i,b in ipairs(t or {}) do h[i]=string.format('%%02x',b) end;"
                "return {pid=getOpenedProcessID(),hex=table.concat(h)}" % (at, n))
        if v.get("pid") != PID:
            raise RuntimeError("CE is not attached to PID %d" % PID)
        chunk = bytes.fromhex(v.get("hex") or "")
        if len(chunk) != n:
            raise RuntimeError("unreadable at %#x" % at)
        out += chunk
    return out


try:
    import time
    for _ in range(60):
        if any(s.get("session_id") == SESSION for s in bridge.list_sessions()):
            break
        time.sleep(1)
    if lua("return {v=getOpenedProcessID()}").get("v") != PID:
        lua("openProcess(%d); return getOpenedProcessID()" % PID)
    base = lua("return {v=getAddressSafe('Nioh3.exe')}")["v"]
    passes = []
    for _ in range(2):
        manager = struct.unpack("<Q", read(base + 0x4751530, 8))[0]
        data = struct.unpack("<Q", read(manager, 8))[0]
        count = struct.unpack("<Q", read(data + 0x927D0, 8))[0]
        assert 0 < count <= 2500, count
        passes.append((manager, data, count, read(data + 0x10, count * 0xF0)))
    assert passes[0] == passes[1], "pool changed between passes"
    manager, data, count, pool = passes[0]
    records = []
    for i in range(count):
        rec = pool[i * 0xF0:(i + 1) * 0xF0]
        key = struct.unpack_from("<H", rec, 0)[0]
        if not key:
            continue
        name, major, minor, part = (names.get(str(key)) or [None] * 4)
        kind = "armor" if major == "防具" else "native_dispatch_1" if major == "武器" else "other_or_unresolved"
        records.append({"F0_offset": i * 0xF0, "item_id_u16": key, "name": name, "catalog_class": minor, "catalog_part": part,
                        "quantity_raw": struct.unpack_from("<H", rec, 2)[0], "level_raw": struct.unpack_from("<H", rec, 6)[0],
                        "plus_raw": struct.unpack_from("<H", rec, 8)[0], "rarity_raw": rec[0x30], "kind": kind,
                        "physical_armor_slot": SLOT.get(part) if kind == "armor" else None,
                        "samurai": minor in ("武士防具", "武士武器"), "ninja": minor in ("忍者防具", "忍者武器")})
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "INVENTORY_F0.bin").write_bytes(pool)
    (OUT / "INVENTORY.json").write_text(json.dumps({
        "schema": "claude-live-inventory/v1", "pid": PID, "module_base": hex(base), "manager": hex(manager), "data": hex(data),
        "pool_rows": count, "two_pass_equal": True, "pool_sha256": hashlib.sha256(pool).hexdigest(),
        "records": records}, ensure_ascii=False, indent=1), encoding="utf-8")
    from collections import Counter
    print(json.dumps({"rows": count, "nonzero": len(records), "kinds": Counter(r["kind"] for r in records),
                      "classes": Counter(r["catalog_class"] for r in records)}, ensure_ascii=False))
finally:
    bridge.stop()
