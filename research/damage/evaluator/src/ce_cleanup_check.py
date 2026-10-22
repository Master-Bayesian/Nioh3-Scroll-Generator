"""Reconnect to the CE session, report observer cleanup and the global breakpoint inventory.

With --stop, ask the owned observer to stop once more (it removes only its own
breakpoints). Never touches breakpoints the observer does not own.
"""
import argparse
import json
import sys
import time
from pathlib import Path

ROOT = Path("F:/Nioh3_ScrollEditor")
NAMESPACE = "Nioh3CoherentPhase_20261016_v4"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=5566)
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--stop", action="store_true")
    parser.add_argument("--clear-stale", action="store_true")
    args = parser.parse_args()
    sys.path.insert(0, str(ROOT / ".tools/cheat-engine-mcp/python/src"))
    from ce_mcp_server.bridge import CheatEngineBridge
    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    bridge.start()
    try:
        for _ in range(60):
            if any(s.get("session_id") == args.session_id for s in bridge.list_sessions()):
                break
            time.sleep(1)

        def lua(script):
            return bridge.call_tool("ce.lua_exec", {"script": script}, args.session_id, 30.0)

        if args.stop:
            print(json.dumps(lua("local p=rawget(_G,'%s'); if p then p.stop('claude_cleanup_recheck') end; return true" % NAMESPACE))[:400])
            time.sleep(3)
        if args.clear_stale:
            # Only when CE's global breakpoint list is explicitly empty: the
            # stale namespace then owns nothing real and may be released.
            print(json.dumps(lua(
                "local list=debug_getBreakpointList and debug_getBreakpointList() or nil;"
                "if type(list)~='table' or #list~=0 then return {cleared=false,reason='breakpoints present or unknown'} end;"
                "rawset(_G,'%s',nil); return {cleared=true}" % NAMESPACE))[:400])
        report = lua(
            "local p=rawget(_G,'%s'); local s=p and p.status() or {observer='absent'};"
            "local list=debug_getBreakpointList and debug_getBreakpointList() or {};"
            "local out={} for i,v in ipairs(list) do out[i]=v end;"
            "return {status=s,global_breakpoints=out,debugging=debug_isDebugging()}" % NAMESPACE)
        print(json.dumps(report)[:3000])
    finally:
        bridge.stop()


if __name__ == "__main__":
    main()
