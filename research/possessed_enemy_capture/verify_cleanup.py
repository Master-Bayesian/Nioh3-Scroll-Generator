from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from run_possessed_enemy_observer import call_session, cleanup_verified, select_session, write_json


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Recheck and retry cleanup only for an identified possessed-enemy capture owner."
    )
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int, default=5556)
    parser.add_argument("--timeout-seconds", type=float, default=3.0)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite cleanup evidence: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)

    from ce_mcp_server.bridge import CheatEngineBridge

    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    bridge.start()
    final: dict[str, Any] = {}
    attempts = 0
    try:
        deadline = time.monotonic() + 20.0
        session_id: str | None = None
        while time.monotonic() < deadline:
            sessions = bridge.list_sessions()
            if sessions:
                session_id = select_session(sessions, args.session_id)
                break
            time.sleep(0.25)
        if session_id is None:
            raise RuntimeError("Cheat Engine bridge did not connect within 20 seconds")

        run_id_json = json.dumps(args.run_id)
        script = (
            "local p=nioh3PossessedCapture; "
            f"assert(p and p.run_id=={run_id_json},'Unexpected capture owner'); "
            "if p.active then p.stop('cleanup_recheck') end; "
            "if p.cleanup_pending then p.retry_cleanup() end; "
            "return {probe={active=p.active,cleanup_pending=p.cleanup_pending,"
            "cleanup_errors=p.cleanup_errors,owned_breakpoints=p.owned_breakpoints,"
            "run_id=p.run_id,schema=p.schema,stop_reason=p.stop_reason},"
            "breakpoints=debug_getBreakpointList()}"
        )
        cleanup_deadline = time.monotonic() + args.timeout_seconds
        while True:
            attempts += 1
            final = call_session(
                bridge,
                session_id,
                f"cleanup recheck {attempts}",
                "ce.lua_exec",
                {"script": script},
                30.0,
            )
            if cleanup_verified(final) or time.monotonic() >= cleanup_deadline:
                break
            time.sleep(0.15)
    finally:
        bridge.stop()

    verified = cleanup_verified(final)
    write_json(output, {
        "cleanup_recheck_metadata": {
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "run_id": args.run_id,
            "session_id": args.session_id,
            "bridge_port": args.port,
            "attempts": attempts,
            "verified": verified,
            "read_only": True,
            "writes_game_memory": False,
        },
        "bridge_result": final,
    })
    if not verified:
        raise RuntimeError(f"Cleanup remains unverified; inspect {output}")
    print(json.dumps({"output": str(output), "verified": True, "attempts": attempts}))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        raise
