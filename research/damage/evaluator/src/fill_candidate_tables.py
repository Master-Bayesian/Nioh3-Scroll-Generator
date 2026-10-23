"""Fill the static parameter-table cells that candidate replays demand, from the same live PID (read-only).

A new baseline's fill covers only the tables its own hit read. Candidates read further rows of the same
startup-loaded tables (readers in demand_fill_loop.STATIC_TABLE_READERS). For every candidate armor piece of the
backpack, replay; while it stops on a static-table read, read that exact span from the live game and retry.
Any other missing read is left UNKNOWN. Output: TABLE_CELLS.json, usable as extra cells / retained cells.
argv: <capture> <request> <filled> <backpack inventory name> <out.json> --port P --session-id S [--form samurai]
"""
import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from demand_fill_loop import STATIC_TABLE_READERS  # noqa: E402

ARMOR_SLOTS = (4, 5, 6, 7, 8)


def main():
    p = argparse.ArgumentParser()
    for name in ("capture", "request", "filled"):
        p.add_argument(name, type=Path)
    p.add_argument("backpack")
    p.add_argument("out", type=Path)
    p.add_argument("--port", type=int, default=5566)
    p.add_argument("--session-id", required=True)
    p.add_argument("--form", default="samurai")
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import CACHE, load_inventory
    sys.path.insert(0, "F:/Nioh3_ScrollEditor/.tools/cheat-engine-mcp/python/src")
    from ce_mcp_server.bridge import CheatEngineBridge
    ev = Evaluator(CACHE, args.capture, args.request, args.filled, armored_baseline=True)
    capture = json.loads(args.capture.read_bytes())
    pid = capture["identity"]["process_id"]
    base = ev.template["scene"]["module_base_u64"]
    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    bridge.start()
    import time
    for _ in range(60):
        if any(s.get("session_id") == args.session_id for s in bridge.list_sessions()):
            break
        time.sleep(1)

    def read(address, size):
        script = ("local t=readBytes(%d,%d,true); local h={} for i,v in ipairs(t or {}) do h[i]=string.format('%%02x',v) end;"
                  "return {pid=getOpenedProcessID(),base=getAddressSafe('Nioh3.exe'),hex=table.concat(h)}") % (address, size)
        r = bridge.call_tool("ce.lua_exec", {"script": script}, args.session_id, 30.0)
        value = r.get("result", r)
        if value.get("pid") != pid or value.get("base") != base:
            raise RuntimeError("CE is no longer attached to the captured process")
        data = bytes.fromhex(value.get("hex") or "")
        return data if len(data) == size else None

    cells = []
    recs, f0 = load_inventory(args.backpack)
    pieces = [(off, r["physical_armor_slot"]) for off, r in sorted(recs.items())
              if r.get("kind") == "armor" and r.get("physical_armor_slot") in ARMOR_SLOTS and r.get(args.form) is True]
    left = 0
    for off, slot in pieces:
        for _ in range(200):
            res = ev.evaluate([(f0[off:off + 0xE8], slot)])
            if res["damage"] is not None:
                break
            missing = (res.get("reason") or {}).get("missing") or {}
            reader = str(missing.get("reader_rva", "")).split(":")[0]
            if reader not in STATIC_TABLE_READERS:
                left += 1
                print(json.dumps({"F0_offset": hex(off), "unknown": res.get("reason")})[:300], flush=True)
                break
            address, size = missing["address_u64"], missing["width_bytes"]
            data = read(address, size)
            if data is None:
                left += 1
                break
            cell = {"name": f"live_table_{address:#x}", "address_u64": address, "value_hex": data.hex(),
                    "known_hex": "ff" * size, "writable": True,
                    "provenance": {"grade": "same-PID read-only static parameter table", "module_image": False,
                                   "demand_reader_rva": reader, "pid": pid}}
            cells.append(cell)
            ev.template["scene"]["memory_regions"].append(cell)
            ev._index()
    args.out.write_text(json.dumps(cells, indent=1) + "\n")
    print(json.dumps({"pieces": len(pieces), "cells": len(cells), "still_unknown": left}))


if __name__ == "__main__":
    main()
