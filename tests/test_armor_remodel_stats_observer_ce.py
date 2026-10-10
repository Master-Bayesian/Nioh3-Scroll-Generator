"""Closed Lua mocks for the PC v2.02 armor stats observer.

These tests exercise the requirements and toughness observer lifecycle and
sanitized projection only. No Cheat Engine process, game executable, save, or
native memory is attached.
"""

from __future__ import annotations

from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).parent))
from lua54_test_runtime import Lua54


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "research" / "armor_remodel_stats_observer_ce.lua"
IDENTITY = {
    "process_id": 777,
    "creation_filetime": "134357108996277385",
    "image_size": 77830112,
    "executable_sha256": "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130",
    "file_version": "2.0.2.0",
}
BASE = 0x140000000
REQ_ENTRY = BASE + 0x2F9E30
REQ_EXIT = BASE + 0x2F9E73
TOUGH_ENTRY = BASE + 0x817FD4
TOUGH_EXIT = BASE + 0x81800B


MOCK = r'''
B=0x140000000;ROW=0x40000000;REC=0x30000000
mem={};bp={};timers={};attachedPid=777;attachedHandle=999;debugging=true;iface=2
thread=7;THREADID=7;ceThread=7007;tick=1000;armCount=0;armFail=0;armReturnsNil=false;removeCalls={};removeFail=false
brokenMode='boolean';fallbackContext=nil;fallbackFailure=false
timerFail=false;continueFail=false;continues=0;attachedWith=0
RAX=0;RBX=0;RCX=0;RDX=0;RSI=0;RDI=0;RBP=0;R8=0;R9=0;R10=0;R11=0;R12=0;R13=0;R14=0;R15=0;RSP=0;RIP=0;EAX=0
function fill(a,n,v) for i=0,n-1 do mem[a+i]=v or 0 end end
function put(a,v,n) for i=0,n-1 do mem[a+i]=v&255;v=v>>8 end end
function putHex(a,h) for i=1,#h,2 do mem[a+(i-1)/2]=tonumber(h:sub(i,i+1),16) end end
function readBytes(a,n,_) local t={} for i=0,n-1 do if mem[a+i]==nil then return nil end;t[#t+1]=mem[a+i] end;return t end
function getAddressSafe(_) return B end
function getOpenedProcessID() return attachedPid end
function getOpenedProcessHandle() return attachedHandle end
function getCurrentThreadId() return ceThread end
function debug_getCurrentThreadId() return ceThread end
function getTickCount() return tick end
function debug_getBreakpointList() if not debugging then return nil end;local t={} for a in pairs(bp) do t[#t+1]=a end;table.sort(t);return t end
function debug_isDebugging() return debugging end
function debug_isBroken()
  if brokenMode=='nonboolean' then return function() end end
  return false
end
function debug_getCurrentContextTable()
  if fallbackFailure then error('injected fallback query failure') end
  return fallbackContext
end
function debug_getCurrentDebuggerInterface() return iface end
function debugProcess(i) assert(i==2);debugging=true;iface=i;attachedWith=i end
function debug_getContext(_) end
function debug_setBreakpoint(a,n,t,m,fn)
  armCount=armCount+1;if armFail==armCount then return false,'injected arm failure' end
  assert(n==1 and t==0 and m==1);bp[a]={id=armCount,fn=fn};if armReturnsNil then return nil end;return true,armCount
end
function debug_removeBreakpoint(a)
  removeCalls[#removeCalls+1]=a;if removeFail then return false end;bp[a]=nil;return true
end
function debug_removeBreakpointByID(id)
  for a,s in pairs(bp) do if s.id==id then return debug_removeBreakpoint(a) end end
  return true
end
function debug_continueFromBreakpoint(m) assert(m==0);continues=continues+1;if continueFail then return false end;return true end
function synchronize(fn) fn() end
function createTimer(ms,fn)
  if timerFail then error('injected timer failure') end
  local timer={ms=ms or 0,fn=type(fn)=='function' and fn or nil,Interval=ms or 0,Enabled=false,destroyed=false}
  function timer.destroy(_) timer.destroyed=true;return true end
  timers[#timers+1]=timer;return timer
end
function drain()
  local count=0
  while true do
    local index=nil
    for i,t in ipairs(timers) do
      if not t.destroyed and t.Enabled and t.Interval<=100 then index=i;break end
    end
    if not index then break end
    local t=table.remove(timers,index);if t.OnTimer then t.OnTimer(t) elseif t.fn then t.fn() end;count=count+1;assert(count<30,'unbounded cleanup')
  end
  return count
end
function fireBudget()
  local expected=(NIOH3_ARMOR_STATS_MAX_SECONDS or 30)*1000
  for _,t in ipairs(timers) do if not t.destroyed and t.Enabled and t.Interval==expected then t.OnTimer(t);return true end end
  return false
end
function hit(rva) RIP=B+rva;THREADID=thread;local site=bp[RIP];assert(site,'breakpoint not armed');return site.fn() end
function prepareRecord(item,ma,mb,level,plus,rarity,flags)
  fill(REC,0xF0);put(REC,item,2);put(REC+0x06,level,2);put(REC+0x0A,plus,2)
  put(REC+0x18,flags or 0,4);put(REC+0x30,rarity,1);put(REC+0x31,ma,1);put(REC+0x32,mb,1)
end
function enter(item,ma,mb,level,plus,rarity,stack,row,flags,index)
  prepareRecord(item,ma,mb,level,plus,rarity,flags);RSP=stack;RCX=row or ROW;RDX=REC;R8=index or 0
  if NIOH3_ARMOR_STATS_KIND=='requirements' then return hit(0x2F9E30) end
  return hit(0x817FD4)
end
function leave(raw,stack)
  RAX=raw;EAX=0;R8=0xDEADBEEF
  if NIOH3_ARMOR_STATS_KIND=='requirements' then RSP=stack-0x28;return hit(0x2F9E73) end
  RSP=stack-0x28;return hit(0x81800B)
end
bptExecute=0;bpmDebugRegister=1;co_run=0
putHex(B+0x2F9E30,'4883EC284183F806');putHex(B+0x2F9E73,'4883C428C3')
putHex(B+0x817FD4,'48895C24084889742410574883EC20');putHex(B+0x81800B,'488B5C24304883C4205FC3')
'''


def lua_literal(value: object) -> str:
    if isinstance(value, str):
        # Lua has no JSON ``\uXXXX`` string escape. Encode UTF-8 bytes using
        # Lua's three-digit decimal escape, matching the live observer runner.
        encoded = value.encode("utf-8")
        return '"' + "".join(
            chr(byte)
            if 32 <= byte < 127 and byte not in (ord('"'), ord("\\"))
            else f"\\{byte:03d}"
            for byte in encoded
        ) + '"'
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return hex(value)
    if isinstance(value, dict):
        return "{" + ",".join(
            f"[{lua_literal(key)}]={lua_literal(item)}" for key, item in value.items()
        ) + "}"
    raise TypeError(value)


def fixture(kind: str = "requirements", **changes: object) -> Lua54:
    lua = Lua54()
    lua.run(MOCK)
    identity = dict(IDENTITY)
    identity.update(changes.pop("identity", {}))
    lines = [
        f"NIOH3_ARMOR_STATS_TARGET_IDENTITY={lua_literal(identity)}",
        'NIOH3_ARMOR_STATS_RUN_ID="synthetic-stats"',
        f"NIOH3_ARMOR_STATS_KIND={lua_literal(kind)}",
        f"NIOH3_BREAKPOINT_LIFECYCLE_PATH={lua_literal('research/owned_breakpoint_lifecycle_ce.lua')}",
        f"NIOH3_ARMOR_STATS_MAIN_THREAD_TIMER_PATH={lua_literal('research/ce_main_thread_timer.lua')}",
    ]
    for name, value in changes.items():
        lines.append(f"{name}={lua_literal(value)}")
    lua.run(";".join(lines))
    return lua


def prepare(lua: Lua54) -> None:
    # Execute checked-in source bytes directly so the Lua mock does not depend
    # on Windows ANSI handling of the Chinese checkout path.
    lua.run(SOURCE.read_text(encoding="utf-8"))


def arm(lua: Lua54) -> None:
    prepare(lua)
    lua.run("return nioh3ArmorRemodelStatsObserver.arm()")


def probe(lua: Lua54) -> dict:
    return lua.run("return nioh3ArmorRemodelStatsObserver")


def stop_and_drain(lua: Lua54) -> dict:
    lua.run("nioh3ArmorRemodelStatsObserver.stop('test_stop');drain()")
    return probe(lua)


def test_lua_literal_encodes_utf8_without_json_unicode_escapes() -> None:
    value = r"C:\研究\仁王3绘卷生成器项目\research\owned_breakpoint_lifecycle_ce.lua"
    encoded = lua_literal(value)
    assert r"\u" not in encoded
    lua = Lua54()
    assert lua.run(f"return {encoded}") == value


@pytest.mark.parametrize("kind", ["requirements", "toughness"])
def test_bootstrap_snapshot_arm_stop_expressions(kind: str) -> None:
    lua = fixture(kind)
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelStatsObserver.status()")
    assert snapshot["prepared"] is True
    assert snapshot["active"] is False
    assert snapshot["observer_kind"] == kind

    armed = lua.run("return nioh3ArmorRemodelStatsObserver.arm()")
    assert armed["armed"] is True
    assert armed["active"] is True
    expected = (
        [f"0x{REQ_ENTRY:X}", f"0x{REQ_EXIT:X}"]
        if kind == "requirements"
        else [f"0x{TOUGH_ENTRY:X}", f"0x{TOUGH_EXIT:X}"]
    )
    assert probe(lua)["arm_inventory"] == expected

    stopping = lua.run("return nioh3ArmorRemodelStatsObserver.stop('snapshot_stop')")
    assert stopping["active"] is False
    assert stopping["cleanup_pending"] is True
    lua.run("drain()")
    closed = lua.run("return nioh3ArmorRemodelStatsObserver.status()")
    assert closed["cleanup_verified"] is True
    assert closed["process_hook_restored"] is True


def test_nonboolean_debug_is_broken_uses_context_fallback_and_records_type() -> None:
    lua = fixture(brokenMode="nonboolean")
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelStatsObserver.status()")
    assert snapshot["debugger_broken"] is False
    assert snapshot["debugger_broken_source"] == "debug_context_fallback:function"
    assert lua.run("return nioh3ArmorRemodelStatsObserver.arm()")["active"] is True
    stop_and_drain(lua)


def test_nonboolean_debug_is_broken_fallback_query_failure_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackFailure=True)
    with pytest.raises(RuntimeError, match="fallback query failed"):
        prepare(lua)


def test_nonboolean_debug_is_broken_true_context_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackContext={})
    with pytest.raises(RuntimeError, match="stopped before prepare"):
        prepare(lua)


def test_requirements_capture_five_controls_pair_index_and_low32_result() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run(
        "enter(0x609C,1,0,180,20,4,0x5000,ROW,0,0);leave(0x100000005,0x5000);"
        "enter(0x609C,1,1,180,20,4,0x6000,ROW,0,1);leave(6,0x6000);"
        "enter(0xED50,1,2,180,20,4,0x7000,ROW,0,2);leave(7,0x7000);"
        "enter(0xED50,2,0,180,20,4,0x8000,ROW,0,3);leave(0,0x8000);"
        "enter(0x609C,2,2,180,20,4,0x9000,ROW,0,4);leave(8,0x9000)"
    )
    value = probe(lua)
    assert value["observer_kind"] == "requirements"
    assert value["max_hits"] == 256
    assert value["max_events"] == 128
    assert value["max_target_entries"] == 128
    assert value["max_read_bytes"] == 8192
    assert len(value["events"]) == 5
    modes = [(event["mode"], event["control"]) for event in value["events"]]
    assert modes == [
        ("1/0", "positive"),
        ("1/1", "positive"),
        ("1/2", "positive"),
        ("2/0", "negative"),
        ("2/2", "negative"),
    ]
    assert value["events"][0]["stat_index"] == 0
    assert value["events"][0]["requirement_raw"] == 5
    assert value["events"][0]["effective_value_raw"] == 5
    assert value["events"][0]["stack_relation"] == "exit_rsp_is_entry_rsp_minus_0x28"
    assert value["events"][4]["stat_index"] == 4
    assert value["events"][4]["requirement_raw"] == 8
    assert all("selected_row" not in event and "base_raw" not in event for event in value["events"])
    assert lua.run("return continues") == 10
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_requirements_zero_result_is_captured_at_common_exit() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("enter(0x609C,2,0,180,20,4,0x5000,ROW,0,6);leave(0,0x5000)")
    event = probe(lua)["events"][0]
    assert event["stat_index"] == 6
    assert event["requirement_raw"] == 0
    assert event["effective_value_raw"] == 0
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_toughness_capture_uses_0x28_stack_pair_and_exploratory_controls() -> None:
    lua = fixture("toughness")
    arm(lua)
    lua.run(
        "enter(0x609C,1,0,180,20,4,0x5000,ROW,0x40000,99);leave(0x100000037,0x5000);"
        "enter(0x609C,1,1,180,20,4,0x6000,ROW,0x40000,99);leave(41,0x6000);"
        "enter(0xED50,1,2,180,20,4,0x7000,ROW,0x40000,99);leave(42,0x7000);"
        "enter(0xED50,2,0,180,20,4,0x8000,ROW,0x40000,99);leave(43,0x8000);"
        "enter(0xED50,2,2,180,20,4,0x9000,ROW,0x40000,99);leave(47,0x9000)"
    )
    value = probe(lua)
    assert value["observer_kind"] == "toughness"
    assert value["max_hits"] == 128
    assert value["max_events"] == 64
    assert value["max_read_bytes"] == 4096
    assert len(value["events"]) == 5
    assert [event["mode"] for event in value["events"]] == ["1/0", "1/1", "1/2", "2/0", "2/2"]
    assert all(event["control"] == "exploratory_equal" for event in value["events"])
    first, second = value["events"][0], value["events"][-1]
    assert first["toughness_raw"] == 0x37
    assert first["effective_value_raw"] == 0x37
    assert first["stack_relation"] == "exit_rsp_is_entry_rsp_minus_0x28"
    assert second["toughness_raw"] == 47
    assert all(event["flags_0x18"] == 0x40000 for event in value["events"])
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_toughness_wrong_0x20_stack_delta_does_not_pair() -> None:
    lua = fixture("toughness")
    arm(lua)
    lua.run(
        "enter(0x609C,1,0,180,20,4,0x5000,ROW,0x40000,0);"
        "RSP=0x5000-0x20;RAX=7;EAX=0;hit(0x81800B)"
    )
    value = probe(lua)
    assert value["events"] in ([], {})
    assert value["stale_pairs"] == 1
    assert lua.run("return continues") == 2
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_foreign_item_and_stale_exit_are_continued_without_output() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("enter(0x1234,1,2,180,20,4,0x5000,ROW,0,0)")
    lua.run("enter(0x609C,1,0,180,20,4,0x6000,ROW,0,0);thread=8;leave(27,0x6000)")
    value = probe(lua)
    assert value["ignored_hits"] == 1
    assert value["stale_pairs"] == 1
    assert value["events"] in ([], {})
    assert lua.run("return continues") == 3
    assert stop_and_drain(lua)["cleanup_verified"] is True


@pytest.mark.parametrize("mutation", ["attachedPid=778", "attachedHandle=998", "function getAddressSafe(_) return B+0x1000 end"])
def test_callback_rechecks_target_process_identity_and_still_resumes(mutation: str) -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run(f"{mutation};enter(0x609C,1,0,180,20,4,0x5000,ROW,0,0)")
    value = probe(lua)
    assert value["stop_reason"] == "process_changed"
    assert value["active"] is False
    assert lua.run("return continues") == 1


def test_callback_does_not_promote_ce_callback_thread_to_target_identity() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("thread=nil;enter(0x609C,1,0,180,20,4,0x5000,ROW,0,0)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "Target debug-event thread ID unavailable" in value["error"]
    assert lua.run("return continues") == 1


@pytest.mark.parametrize("kind,rva", [("requirements", "0x2F9E30"), ("toughness", "0x817FD4")])
def test_each_entry_callback_continues_after_read_error(kind: str, rva: str) -> None:
    lua = fixture(kind)
    arm(lua)
    lua.run(f"prepareRecord(0x609C,1,0,180,20,4,0);mem[REC+0x06]=nil;RSP=0x5000;RCX=ROW;RDX=REC;R8=0;hit({rva})")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "Unreadable record level" in value["error"]
    assert lua.run("return continues") == 1
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_requirements_rejects_invalid_stat_index_and_resumes() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("enter(0x609C,1,0,180,20,4,0x5000,ROW,0,7)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "outside 0..6" in value["error"]
    assert lua.run("return continues") == 1
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_ce_nil_arm_return_is_accepted_only_after_inventory_proof() -> None:
    lua = fixture("requirements", armReturnsNil=True)
    arm(lua)
    value = probe(lua)
    assert value["active"] is True
    assert value["arm_inventory"] == [f"0x{REQ_ENTRY:X}", f"0x{REQ_EXIT:X}"]
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_prepare_does_not_install_process_hook_and_reload_requires_clean_owner() -> None:
    lua = fixture("requirements")
    prepare(lua)
    assert lua.run("return onOpenProcess == nil") is True
    lua.run("nioh3ArmorRemodelStatsObserver.arm()")
    with pytest.raises(RuntimeError, match="still active"):
        lua.run(SOURCE.read_text(encoding="utf-8"))
    closed = stop_and_drain(lua)
    assert closed["process_hook_restored"] is True
    assert lua.run("return onOpenProcess == nil") is True
    prepare(lua)
    assert probe(lua)["prepared"] is True


def test_ignored_hit_budget_is_bounded_and_resumes_callbacks() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("for i=1,64 do enter(0x1234,0,0,1,0,1,0x5000+i,ROW,0,0) end")
    value = probe(lua)
    assert value["stop_reason"] == "ignored_hit_budget"
    assert value["total_hits"] == 64
    assert len(value["events"]) == 0
    assert lua.run("return continues") == 64
    lua.run("enter(0x1234,0,0,1,0,1,0x7000,ROW,0,0)")
    assert lua.run("return continues") == 65
    assert probe(lua)["total_hits"] == 64
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_configured_120_second_requirements_window_keeps_limits_and_cleans_up() -> None:
    lua = fixture("requirements", NIOH3_ARMOR_STATS_MAX_SECONDS=120)
    arm(lua)
    value = probe(lua)
    assert value["max_seconds"] == 120
    assert value["max_hits"] == 256
    assert value["max_events"] == 128
    assert value["max_read_bytes"] == 8192
    assert lua.run("return timers[1].Interval") == 120000
    lua.run("tick=121001;enter(0x1234,0,0,1,0,1,0x5000,ROW,0,0)")
    value = probe(lua)
    assert value["stop_reason"] == "time_budget"
    assert value["active"] is False
    assert stop_and_drain(lua)["cleanup_verified"] is True


@pytest.mark.parametrize("seconds", [0, 121])
def test_invalid_configured_window_is_rejected_before_debugger_arm(seconds: int) -> None:
    lua = fixture("requirements", NIOH3_ARMOR_STATS_MAX_SECONDS=seconds, debugging=False)
    with pytest.raises(RuntimeError, match="NIOH3_ARMOR_STATS_MAX_SECONDS"):
        prepare(lua)
    assert lua.run("return attachedWith") == 0
    assert lua.run("return armCount") == 0


def test_foreign_breakpoint_is_refused_and_never_removed() -> None:
    lua = fixture("requirements")
    lua.run("bp[0xDEAD]={id=900,fn=function() end}")
    with pytest.raises(RuntimeError, match="Foreign"):
        arm(lua)
    assert lua.run("return bp[0xDEAD]~=nil") is True
    assert lua.run("return #removeCalls") == 0


@pytest.mark.parametrize(
    "mutation,pattern",
    [
        ("identity", "identity"),
        ("pid", "Wrong process"),
        ("signature", "Signature mismatch"),
        ("size", "image_size"),
        ("version", "file_version"),
    ],
)
def test_identity_and_signature_validation_happen_before_arming(
    mutation: str, pattern: str
) -> None:
    if mutation == "identity":
        lua = fixture("requirements", identity={"executable_sha256": "bad"})
    elif mutation == "pid":
        lua = fixture("requirements", attachedPid=778)
    elif mutation == "size":
        lua = fixture("requirements", identity={"image_size": 77814240})
    elif mutation == "version":
        lua = fixture("requirements", identity={"file_version": "2.0.1.0"})
    else:
        lua = fixture("requirements")
        lua.run("mem[B+0x2F9E73]=0")
    with pytest.raises(RuntimeError, match=pattern):
        arm(lua)
    assert lua.run("return armCount") == 0


def test_existing_non_veh_debugger_and_failed_cleanup_remain_bounded() -> None:
    lua = fixture("requirements", iface=1)
    with pytest.raises(RuntimeError, match="VEH"):
        arm(lua)
    assert lua.run("return armCount") == 0

    lua = fixture("requirements")
    arm(lua)
    lua.run("removeFail=true;nioh3ArmorRemodelStatsObserver.stop('failed_cleanup');drain()")
    value = probe(lua)
    assert value["cleanup_pending"] is True
    assert value["cleanup_verified"] is False
    lua.run("removeFail=false;nioh3ArmorRemodelStatsObserver.retry_cleanup();drain()")
    assert probe(lua)["cleanup_verified"] is True


def test_source_rejects_game_mutation_apis() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    for forbidden in (
        "writeBytes(",
        "writeInteger(",
        "executeCodeEx(",
        "autoAssemble(",
        "debug_setContext(",
        "debug_setBreakpoint(address, 8",
        "bptWrite",
        "bptAccess",
    ):
        assert forbidden not in source
