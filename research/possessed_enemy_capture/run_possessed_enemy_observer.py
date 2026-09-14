from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, TYPE_CHECKING

ROOT = Path(__file__).resolve().parents[2]
CE_MCP_PYTHON = ROOT / ".tools" / "cheat-engine-mcp" / "python" / "src"
if CE_MCP_PYTHON.is_dir() and str(CE_MCP_PYTHON) not in sys.path:
    sys.path.insert(0, str(CE_MCP_PYTHON))

if TYPE_CHECKING:
    from ce_mcp_server.bridge import CheatEngineBridge
else:
    def CheatEngineBridge(*args, **kwargs):
        # Preserve the callable export used by capture_postspawn_snapshot.py.
        from ce_mcp_server.bridge import CheatEngineBridge as Implementation
        return Implementation(*args, **kwargs)


def new_bridge(port: int):
    # Keep non-live validation importable without the optional local CE bridge.
    from ce_mcp_server.bridge import CheatEngineBridge
    return CheatEngineBridge(host="127.0.0.1", port=port)


def process_birth_filetime(pid: int) -> str:
    """Identity query only; never request target write/debug rights."""
    if sys.platform != "win32":
        raise RuntimeError("strict v2.01 generator capture requires Windows process-birth validation")
    import ctypes
    from ctypes import wintypes
    k = ctypes.WinDLL("kernel32", use_last_error=True)
    k.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    k.OpenProcess.restype = wintypes.HANDLE
    k.GetProcessTimes.argtypes = (wintypes.HANDLE,) + (ctypes.POINTER(wintypes.FILETIME),) * 4
    k.GetProcessTimes.restype = wintypes.BOOL
    k.CloseHandle.argtypes = (wintypes.HANDLE,)
    k.CloseHandle.restype = wintypes.BOOL
    h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not h:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        times = [wintypes.FILETIME() for _ in range(4)]
        if not k.GetProcessTimes(h, *(ctypes.byref(t) for t in times)):
            raise ctypes.WinError(ctypes.get_last_error())
        return str((times[0].dwHighDateTime << 32) | times[0].dwLowDateTime)
    finally:
        k.CloseHandle(h)


def lua_literal(value: object) -> str:
    """A small explicit encoder, not JSON pasted into Lua table syntax."""
    if isinstance(value, dict):
        return "{" + ",".join("[" + lua_literal(str(k)) + "]=" + lua_literal(v) for k, v in value.items()) + "}"
    if value is None:
        return "nil"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, (int, float)):
        return str(value)
    encoded = str(value).encode("utf-8")
    return '"' + ''.join(chr(b) if 32 <= b < 127 and b not in (34, 92)
                         else "\\" + str(b).zfill(3) for b in encoded) + '"'



CAPTURE_DIR = Path(__file__).resolve().parent
PHASES = {
    "mode-transaction-join": "mode_transaction_join_ce.lua",
    "mode-upstream": "mode_upstream_ce.lua",
    "mode-upstream-sequence": "mode_upstream_sequence_ce.lua",
    "materialization-frontier": "materialization_frontier_ce.lua",
    "assignment-origin": "assignment_origin_ce.lua",
    "late-mask": "late_mask_observer_ce.lua",
    "mission": "mission_entry_ce.lua",
    "selector": "selector_summary_ce.lua",
    "draw-count": "draw_count_ce.lua",
    "mt-selection": "mt_selection_ce.lua",
    "pool-removal": "pool_removal_ce.lua",
    "config": "config_branch_ce.lua",
    "config-draw": "config_and_draw_count_ce.lua",
}
STRICT_V201_PHASES = {"mode-transaction-join", "assignment-origin", "mode-upstream", "mode-upstream-sequence", "materialization-frontier"}
WINDOWED_PHASES = {"mode-transaction-join", "mode-upstream-sequence", "materialization-frontier"}
EXPECTED_EXECUTABLE_SHA256 = "4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159"
SEQUENCE_OBSERVATION_SECONDS = 120.0


def require_ok(operation: str, result: dict[str, Any]) -> dict[str, Any]:
    if result.get("ok") is not True:
        raise RuntimeError(f"{operation} failed: {json.dumps(result, ensure_ascii=False)}")
    return result


def lua_path(path: Path) -> str:
    return path.resolve().as_posix()


def result_value(result: dict[str, Any]) -> object:
    return result.get("result", result.get("value", result))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def require_unused_output(output: Path) -> Path:
    cleanup = output.with_suffix(".cleanup.json")
    cleanup_pending = output.with_suffix(".cleanup-pending.json")
    conflicts = [path for path in (output, cleanup, cleanup_pending) if path.exists()]
    if conflicts:
        names = ", ".join(str(path) for path in conflicts)
        raise FileExistsError(f"Refusing to overwrite existing capture evidence: {names}")
    return cleanup


def write_json(path: Path, payload: object) -> None:
    temporary = path.with_name(f".{path.name}.{time.monotonic_ns()}.tmp")
    temporary.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    temporary.replace(path)


def select_session(sessions: list[dict[str, Any]], requested_session_id: str | None) -> str:
    if requested_session_id is not None:
        if any(session.get("session_id") == requested_session_id for session in sessions):
            return requested_session_id
        raise RuntimeError(f"Requested Cheat Engine session is not connected: {requested_session_id}")
    if len(sessions) == 1:
        return str(sessions[0]["session_id"])
    if len(sessions) > 1:
        ids = ", ".join(str(session.get("session_id")) for session in sessions)
        raise RuntimeError(f"Multiple Cheat Engine sessions are connected; select one with --session-id: {ids}")
    raise RuntimeError("No Cheat Engine session is connected")


def call_session(
    bridge: CheatEngineBridge,
    session_id: str,
    operation: str,
    tool: str,
    payload: dict[str, Any],
    timeout_seconds: float,
) -> dict[str, Any]:
    if not any(session.get("session_id") == session_id for session in bridge.list_sessions()):
        raise RuntimeError(f"Cheat Engine session disconnected before {operation}: {session_id}")
    result = require_ok(operation, bridge.call_tool(tool, payload, session_id, timeout_seconds))
    if result.get("bridge_session_id") != session_id or result.get("session_redirected") is True:
        raise RuntimeError(f"Cheat Engine session changed during {operation}: {json.dumps(result, ensure_ascii=False)}")
    return result


def verify_target(attached: dict[str, Any], requested_pid: int) -> dict[str, Any]:
    if attached.get("attached") is not True:
        raise RuntimeError("Cheat Engine did not report an attached target")
    actual_pid = int(attached.get("process_id", 0) or 0)
    if actual_pid != requested_pid:
        raise RuntimeError(f"Refusing unexpected target PID: {actual_pid} != {requested_pid}")
    process_name = str(attached.get("process_name", ""))
    if process_name.casefold() != "nioh3.exe":
        raise RuntimeError(f"Refusing unexpected target process: {process_name!r}")
    image_path = Path(str(attached.get("image_path", "")))
    if not image_path.is_file():
        raise RuntimeError(f"Target executable path is unavailable: {image_path}")
    executable_sha256 = sha256_file(image_path)
    if executable_sha256 != EXPECTED_EXECUTABLE_SHA256:
        raise RuntimeError(
            "Refusing an unapproved Nioh3.exe build: "
            f"{executable_sha256} != {EXPECTED_EXECUTABLE_SHA256}"
        )
    module_base = attached.get("module_base")
    if isinstance(module_base, int):
        module_base = f"0x{module_base:X}"
    elif module_base is not None:
        module_base = str(module_base)
    return {
        "process_id": actual_pid,
        "process_name": process_name,
        "image_path": str(image_path.resolve()),
        "image_size": image_path.stat().st_size,
        "executable_sha256": executable_sha256,
        "module_base": module_base,
    }


def cleanup_verified(cleanup_result: dict[str, Any]) -> bool:
    value = result_value(cleanup_result)
    if not isinstance(value, dict):
        return False
    probe = value.get("probe")
    breakpoints = value.get("breakpoints")
    if not isinstance(probe, dict):
        return False
    # ``nil`` is an unknown inventory, not proof of cleanup.  Once the runner
    # has armed an observer the bridge must return an explicit empty list/table.
    breakpoints_empty = breakpoints == [] or breakpoints == {}
    return (
        probe.get("active") is False
        and probe.get("cleanup_pending") is False
        and probe.get("owned_breakpoints") in ([], {})
        and breakpoints_empty
        and probe.get("debugger_broken") is False
    )


def fresh_phase_script() -> str:
    """Return the pre-bootstrap CE transaction handshake.

    This intentionally fails closed.  It never removes breakpoints because a
    non-empty inventory may belong to another debugger owner.
    """
    return r"""
local function debuggerStopped()
  local value = debug_isBroken()
  if type(value) == 'boolean' then return value, 'debug_isBroken' end
  assert(type(debug_getCurrentContextTable) == 'function',
         'CE stopped-event fallback API unavailable')
  local ok, context = pcall(debug_getCurrentContextTable)
  assert(ok, 'CE stopped-event fallback query failed')
  return type(context) == 'table', 'debug_context_fallback:' .. type(value)
end
local prior = nioh3PossessedCapture
if prior then
  assert(prior.active == false, 'Previous observer active state is not explicitly clean')
  assert(prior.cleanup_pending == false, 'Previous observer cleanup state is not explicitly complete')
  assert(type(prior.owned_breakpoints) == 'table' and next(prior.owned_breakpoints) == nil,
         'Previous observer breakpoint ownership is unknown or nonempty')
end
assert(type(debug_isDebugging) == 'function', 'CE debugger status API unavailable')
assert(type(debug_isBroken) == 'function', 'CE stopped-event status API unavailable')
assert(type(debug_getBreakpointList) == 'function', 'CE breakpoint inventory API unavailable')
local debugging = debug_isDebugging()
local broken, brokenSource = debuggerStopped()
assert(not broken, 'Debugger event is still stopped')
local inventory = debug_getBreakpointList()
if debugging then
  assert(type(inventory) == 'table', 'Breakpoint inventory is unknown while debugger is active')
  assert(next(inventory) == nil, 'Foreign or stale breakpoint state is present')
else
  assert(inventory == nil or (type(inventory) == 'table' and next(inventory) == nil),
         'Unexpected breakpoint inventory while debugger is inactive')
end
-- Clear only this observer's namespace.  Foreign CE globals are not touched.
nioh3PossessedCapture = nil
NIOH3_POSSESSED_TARGET_IDENTITY = nil
NIOH3_ASSIGNMENT_SEED = nil
NIOH3_MATERIALIZATION_MODE_LABEL = nil
NIOH3_POSSESSED_RUN_ID = nil
NIOH3_POSSESSED_STOP_ON_FIRST_FINAL = nil
NIOH3_BREAKPOINT_LIFECYCLE_PATH = nil
NIOH3_POSSESSED_COMMON_PATH = nil
return {initialized=true, debugger_active=debugging, debugger_broken=broken,
        debugger_broken_source=brokenSource, breakpoints=inventory or {}}
"""


def arm_verification_script(
    expected_pid: int | None = None,
    expected_module_base: str | None = None,
    expected_creation_filetime: str | None = None,
    expected_breakpoint_count: int | None = None,
) -> str:
    """Return a post-bootstrap proof of the observer's ownership and state."""
    return r"""
local function debuggerStopped()
  local value = debug_isBroken()
  if type(value) == 'boolean' then return value, 'debug_isBroken' end
  assert(type(debug_getCurrentContextTable) == 'function',
         'CE stopped-event fallback API unavailable')
  local ok, context = pcall(debug_getCurrentContextTable)
  assert(ok, 'CE stopped-event fallback query failed')
  return type(context) == 'table', 'debug_context_fallback:' .. type(value)
end
local p = assert(nioh3PossessedCapture, 'Observer did not publish a probe')
assert(p.active == true, 'Observer is not active after bootstrap')
assert(p.owned_breakpoints ~= nil, 'Observer did not publish owned breakpoints')
""" + (
        f"assert(p.pid == {expected_pid}, 'Observer PID identity mismatch')\n"
        f"assert(getOpenedProcessID() == {expected_pid}, 'CE attached PID changed after arm')\n"
        if expected_pid is not None else ""
    ) + (
        f"assert(p.module_base == {lua_literal(expected_module_base)}, 'Observer module identity mismatch')\n"
        f"assert(getAddressSafe('Nioh3.exe') == tonumber({lua_literal(expected_module_base)}), 'CE module base changed after arm')\n"
        if expected_module_base is not None else ""
    ) + (
        f"assert(NIOH3_POSSESSED_TARGET_IDENTITY.creation_filetime == {lua_literal(expected_creation_filetime)}, 'Observer birth identity mismatch')\n"
        if expected_creation_filetime is not None else ""
    ) + (
        f"assert(#p.owned_breakpoints == {expected_breakpoint_count}, 'Observer owned-breakpoint count mismatch')\n"
        if expected_breakpoint_count is not None else ""
    ) + r"""
local inventory = assert(debug_getBreakpointList(), 'Breakpoint inventory is unknown after arm')
assert(type(inventory) == 'table', 'Breakpoint inventory is not a table after arm')
local broken, brokenSource = debuggerStopped()
assert(broken == false, 'Debugger event is stopped after arm')
local function normalize(value)
  local out = {}
  for _, item in pairs(value) do
    if type(item) == 'number' then out[#out+1] = string.format('0x%X', item)
    elseif type(item) == 'string' then out[#out+1] = item
    elseif type(item) == 'table' and type(item.address) == 'number' then
      out[#out+1] = string.format('0x%X', item.address)
    else error('Unrecognized breakpoint inventory entry') end
  end
  table.sort(out)
  return out
end
local owned = normalize(p.owned_breakpoints)
local actual = normalize(inventory)
assert(#owned == #actual, 'Global breakpoint inventory does not match observer ownership')
for i = 1, #owned do assert(owned[i] == actual[i], 'Global breakpoint inventory ownership mismatch') end
return {active=p.active, run_id=p.run_id, schema=p.schema,
        owned_breakpoints=owned, breakpoints=actual,
        debugger_broken=broken, debugger_broken_source=brokenSource}
"""


def _as_set(value: object) -> set[str] | None:
    if not isinstance(value, (list, dict)):
        return None
    values = value if isinstance(value, list) else list(value.values())
    try:
        return {str(item) for item in values}
    except Exception:
        return None


def arm_verified(arm_result: dict[str, Any], run_id: str, expected_breakpoint_count: int | None = None) -> bool:
    value = result_value(arm_result)
    if not isinstance(value, dict) or value.get("active") is not True:
        return False
    if value.get("run_id") != run_id or value.get("debugger_broken") is not False:
        return False
    owned = _as_set(value.get("owned_breakpoints"))
    inventory = _as_set(value.get("breakpoints"))
    return (
        owned is not None
        and inventory is not None
        and owned == inventory
        and (expected_breakpoint_count is None or len(owned) == expected_breakpoint_count)
    )


def cleanup_script(run_id: str, reason: str) -> str:
    return (
        "local function debuggerStopped() local value=debug_isBroken(); "
        "if type(value)=='boolean' then return value,'debug_isBroken' end; "
        "assert(type(debug_getCurrentContextTable)=='function','CE stopped-event fallback API unavailable'); "
        "local ok,context=pcall(debug_getCurrentContextTable); assert(ok,'CE stopped-event fallback query failed'); "
        "return type(context)=='table','debug_context_fallback:'..type(value) end; "
        "local p=nioh3PossessedCapture; "
        f"if p then assert(p.run_id=={lua_literal(run_id)},'Unexpected capture owner'); "
        f"if p.active then p.stop({lua_literal(reason)}) end; "
        "if p.cleanup_pending then p.retry_cleanup() end end; "
        "local broken,brokenSource=debuggerStopped(); "
        "return {probe=p and {active=p.active,cleanup_pending=p.cleanup_pending,"
        "debugger_broken=broken,debugger_broken_source=brokenSource,cleanup_errors=p.cleanup_errors,"
        "owned_breakpoints=p.owned_breakpoints,run_id=p.run_id,schema=p.schema,"
        "stop_reason=p.stop_reason} or nil,breakpoints=debug_getBreakpointList(),"
        "debugger_broken=broken,debugger_broken_source=brokenSource}"
    )


def reconnect_cleanup(
    port: int,
    session_id: str,
    run_id: str,
    max_cycles: int = 5,
) -> tuple[dict[str, Any], int]:
    final: dict[str, Any] = {}
    for cycle in range(1, max_cycles + 1):
        time.sleep(0.75)
        bridge = new_bridge(port)
        bridge.start()
        try:
            deadline = time.monotonic() + 20.0
            while time.monotonic() < deadline:
                sessions = bridge.list_sessions()
                if any(session.get("session_id") == session_id for session in sessions):
                    break
                time.sleep(0.25)
            else:
                raise RuntimeError(f"Cheat Engine session did not reconnect for cleanup: {session_id}")
            final = call_session(
                bridge,
                session_id,
                f"post-disconnect cleanup verification {cycle}",
                "ce.lua_exec",
                {"script": cleanup_script(run_id, "runner_cleanup_reconnect")},
                30.0,
            )
        finally:
            bridge.stop()
        if cleanup_verified(final):
            return final, cycle
    return final, max_cycles


def main() -> None:
    parser = argparse.ArgumentParser(description="Arm one read-only PC v2.01 possessed-enemy observer phase.")
    parser.add_argument("--pid", type=int, required=True, help="PID of an already-running Nioh3.exe process")
    parser.add_argument("--phase", choices=sorted(PHASES), required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--seed", type=lambda x: int(x, 0),
                        help="Required for strict v2.01 generator phases; exact displayed seed, decimal or 0x")
    parser.add_argument(
        "--mode-label",
        choices=("normal-solo", "one-person-expedition"),
        help="Optional owner-observed entry mode recorded as capture metadata",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--session-id", help="Exact connected CE bridge session; required when more than one exists")
    parser.add_argument("--port", type=int, default=5556, help="CE bridge port")
    parser.add_argument("--poll-seconds", type=float, default=1.0)
    parser.add_argument("--timeout-seconds", type=float, default=210.0)
    parser.add_argument(
        "--stop-on-first-final",
        action="store_true",
        help="For a late-mask negative control, stop after the first complete final event even when tracked fields are zero",
    )
    args = parser.parse_args()
    if args.phase in STRICT_V201_PHASES and not 1 <= len(args.run_id) <= 128:
        parser.error("strict v2.01 phase run ID must contain 1..128 characters")
    if args.phase in STRICT_V201_PHASES and (args.seed is None or not 0 <= args.seed <= 0xFFFFFFFF):
        parser.error(f"{args.phase} requires --seed in uint32 range")

    if args.phase in WINDOWED_PHASES and args.stop_on_first_final:
        parser.error(f"{args.phase} must not stop on the first generation")
    if args.phase in WINDOWED_PHASES and args.timeout_seconds <= SEQUENCE_OBSERVATION_SECONDS:
        parser.error(f"{args.phase} runner timeout must exceed its 120-second observation window")

    output = args.output.resolve()
    cleanup_output = require_unused_output(output)
    cleanup_pending_output = output.with_suffix(".cleanup-pending.json")
    output.parent.mkdir(parents=True, exist_ok=True)
    bridge = new_bridge(args.port)
    bridge.start()
    session_id: str | None = None
    arm_attempted = False
    primary_failure: BaseException | None = None
    target_identity: dict[str, Any] | None = None
    fresh_initialization: dict[str, Any] | None = None
    arm_verification: dict[str, Any] | None = None
    phase_path = CAPTURE_DIR / PHASES[args.phase]
    source_identity = {
        "phase_file": phase_path.name,
        "phase_sha256": sha256_file(phase_path),
        "common_sha256": (sha256_file(CAPTURE_DIR / "observer_common_ce.lua")
                          if args.phase not in STRICT_V201_PHASES else None),
        "lifecycle_sha256": sha256_file(ROOT / "research" / "owned_breakpoint_lifecycle_ce.lua"),
    }
    try:
        deadline = time.monotonic() + 20.0
        while time.monotonic() < deadline:
            sessions = bridge.list_sessions()
            if sessions:
                session_id = select_session(sessions, args.session_id)
                break
            time.sleep(0.25)
        if session_id is None:
            raise RuntimeError("Cheat Engine bridge did not connect within 20 seconds")

        before_attach = call_session(bridge, session_id, "read current target", "ce.get_attached_process", {}, 30.0)
        if before_attach.get("attached") is True:
            before_pid = int(before_attach.get("process_id", 0) or 0)
            if before_pid != args.pid:
                raise RuntimeError(
                    f"Cheat Engine is already attached to PID {before_pid}; refusing to retarget it to {args.pid}"
                )
        else:
            call_session(
                bridge, session_id, "attach", "ce.attach_process", {"process_id": args.pid}, 30.0
            )
        attached = call_session(bridge, session_id, "target identity", "ce.get_attached_process", {}, 30.0)
        target_identity = verify_target(attached, args.pid)
        if args.phase in STRICT_V201_PHASES:
            if target_identity["image_size"] != 77814240:
                raise RuntimeError("v2.01 executable size mismatch")
            target_identity["creation_filetime"] = process_birth_filetime(args.pid)

        # A phase is a transaction boundary.  Do this after target identity
        # validation and before publishing any new observer globals.
        fresh_initialization = call_session(
            bridge, session_id, "fresh phase initialization", "ce.lua_exec",
            {"script": fresh_phase_script()}, 30.0,
        )

        strict_bootstrap = ""
        if args.phase in STRICT_V201_PHASES:
            strict_bootstrap = ("NIOH3_POSSESSED_TARGET_IDENTITY=" + lua_literal(target_identity) + ";"
                                + f"NIOH3_ASSIGNMENT_SEED={args.seed};")
        if args.mode_label is not None:
            strict_bootstrap += f"NIOH3_MATERIALIZATION_MODE_LABEL={lua_literal(args.mode_label)};"
        bootstrap = (
            strict_bootstrap +
            f"NIOH3_POSSESSED_RUN_ID={lua_literal(args.run_id)};"
            f"NIOH3_POSSESSED_STOP_ON_FIRST_FINAL={'true' if args.stop_on_first_final else 'false'};"
            f"NIOH3_BREAKPOINT_LIFECYCLE_PATH={lua_literal(lua_path(ROOT / 'research' / 'owned_breakpoint_lifecycle_ce.lua'))};"
            f"NIOH3_POSSESSED_COMMON_PATH={lua_literal(lua_path(CAPTURE_DIR / 'observer_common_ce.lua'))};"
            f"return assert(loadfile({lua_literal(lua_path(phase_path))}))()"
        )
        arm_attempted = True
        armed = call_session(
            bridge, session_id, "arm observer", "ce.lua_exec", {"script": bootstrap}, 60.0
        )
        arm_verification = call_session(
            bridge, session_id, "verify observer arm", "ce.lua_exec",
            {"script": arm_verification_script(
                expected_pid=args.pid,
                expected_module_base=target_identity.get("module_base") if target_identity else None,
                expected_creation_filetime=target_identity.get("creation_filetime") if target_identity else None,
                expected_breakpoint_count=4 if args.phase in STRICT_V201_PHASES else None,
            )}, 30.0,
        )
        if not arm_verified(
            arm_verification,
            args.run_id,
            expected_breakpoint_count=4 if args.phase in STRICT_V201_PHASES else None,
        ):
            raise RuntimeError("Observer arm verification failed: identity, ownership, or debugger state mismatch")
        if args.phase in STRICT_V201_PHASES and process_birth_filetime(args.pid) != target_identity["creation_filetime"]:
            raise RuntimeError("Target process replaced during observer arm: refusing capture/retargeting")
        print("ARMED", json.dumps(armed, ensure_ascii=False), flush=True)

        deadline = time.monotonic() + args.timeout_seconds
        sequence_deadline = (
            time.monotonic() + SEQUENCE_OBSERVATION_SECONDS
            if args.phase in WINDOWED_PHASES else None
        )
        while time.monotonic() < deadline:
            if args.phase in STRICT_V201_PHASES:
                if process_birth_filetime(args.pid) != target_identity["creation_filetime"]:
                    raise RuntimeError("Target process replaced: refusing capture/retargeting")
            close_sequence_window = (
                sequence_deadline is not None and time.monotonic() >= sequence_deadline
            )
            snapshot = call_session(
                bridge,
                session_id,
                "close sequence observation window" if close_sequence_window else "read observer",
                "ce.lua_exec",
                {"script": (
                    "if nioh3PossessedCapture and nioh3PossessedCapture.active then "
                    "nioh3PossessedCapture.stop('observation_window_elapsed') end; "
                    "return nioh3PossessedCapture"
                    if close_sequence_window else "return nioh3PossessedCapture"
                )},
                30.0,
            )
            payload = {
                "capture_metadata": {
                    "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                    "phase": args.phase,
                    "run_id": args.run_id,
                    "requested_pid": args.pid,
                    "target_seed": args.seed,
                    "mode_label": args.mode_label,
                    "session_id": session_id,
                    "bridge_port": args.port,
                    "target": target_identity,
                    "source": source_identity,
                    "stop_on_first_final": args.stop_on_first_final,
                    "read_only": True,
                    "writes_game_memory": False,
                    "fresh_phase_initialization": fresh_initialization,
                    "arm_verification": arm_verification,
                },
                "bridge_result": snapshot,
            }
            write_json(output, payload)
            value = result_value(snapshot)
            if isinstance(value, dict) and value.get("active") is False:
                if args.phase in WINDOWED_PHASES and (
                    value.get("error") or value.get("stop_reason") not in {
                        "observation_window_elapsed", "entry_observation_complete"
                    }
                ):
                    raise RuntimeError("sequence observation interrupted; preserve partial evidence: "
                                       + str(value.get("error") or value.get("stop_reason")))
                expected_stop = {
                    "assignment-origin": "all_generated_tasks_linked",
                    "mode-upstream": "upstream_request_bound",
                }.get(args.phase)
                if expected_stop is not None and (
                    value.get("error") or value.get("stop_reason") != expected_stop
                ):
                    raise RuntimeError(f"{args.phase} trace incomplete/rejected: "
                                       + str(value.get("error") or value.get("stop_reason")))
                break
            time.sleep(max(0.1, args.poll_seconds))
        else:
            call_session(
                bridge,
                session_id,
                "stop timed-out observer",
                "ce.lua_exec",
                {
                    "script": "if nioh3PossessedCapture and nioh3PossessedCapture.active then nioh3PossessedCapture.stop('runner_timeout') end; return nioh3PossessedCapture"
                },
                30.0,
            )
            if args.phase in STRICT_V201_PHASES:
                raise TimeoutError(f"{args.phase} capture did not finish before the runner deadline")
        print(f"SAVED {output}", flush=True)
    except BaseException as exc:
        primary_failure = exc
    finally:
        cleanup_failure: Exception | None = None
        initial_cleanup: dict[str, Any] = {}
        initial_cleanup_verified = False
        if arm_attempted and session_id is not None:
            try:
                initial_cleanup = call_session(
                    bridge,
                    session_id,
                    "request cleanup",
                    "ce.lua_exec",
                    {"script": cleanup_script(args.run_id, "runner_complete")},
                    30.0,
                )
                initial_cleanup_verified = cleanup_verified(initial_cleanup)
                if not initial_cleanup_verified:
                    write_json(cleanup_pending_output, {
                        "cleanup_pending_metadata": {
                            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                            "run_id": args.run_id,
                            "phase": args.phase,
                            "session_id": session_id,
                            "bridge_port": args.port,
                            "target": target_identity,
                            "verified": False,
                            "meaning": "Initial state before releasing the bridge for CE GUI-thread cleanup.",
                        },
                        "bridge_result": initial_cleanup,
                    })
            except Exception as exc:
                cleanup_failure = exc
        bridge.stop()
        if arm_attempted and session_id is not None:
            final_cleanup = initial_cleanup
            reconnect_cycles = 0
            if not initial_cleanup_verified:
                try:
                    final_cleanup, reconnect_cycles = reconnect_cleanup(
                        args.port, session_id, args.run_id
                    )
                    cleanup_failure = None
                except Exception as exc:
                    cleanup_failure = exc
            verified = cleanup_verified(final_cleanup)
            write_json(cleanup_output, {
                "cleanup_metadata": {
                    "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                    "run_id": args.run_id,
                    "phase": args.phase,
                    "session_id": session_id,
                    "bridge_port": args.port,
                    "target": target_identity,
                    "verified": verified,
                    "initially_verified": initial_cleanup_verified,
                    "reconnect_cycles": reconnect_cycles,
                    "error": f"{type(cleanup_failure).__name__}: {cleanup_failure}" if cleanup_failure else None,
                },
                "bridge_result": final_cleanup,
            })
            if not verified and cleanup_failure is None:
                cleanup_failure = RuntimeError(f"Observer cleanup is unverified; inspect {cleanup_output}")
        if primary_failure is None and cleanup_failure is not None:
            primary_failure = cleanup_failure
    if primary_failure is not None:
        raise primary_failure


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        raise
