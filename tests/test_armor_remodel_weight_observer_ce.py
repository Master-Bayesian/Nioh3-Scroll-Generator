"""Closed Lua mocks for the PC v2.02 armor remodel weight observer.

These tests exercise the observer lifecycle and sanitized projection only.  No
Cheat Engine process, game executable, save, or native memory is attached.
"""

from __future__ import annotations

from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).parent))
from lua54_test_runtime import Lua54


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "research" / "armor_remodel_weight_observer_ce.lua"
LIFECYCLE = ROOT / "research" / "owned_breakpoint_lifecycle_ce.lua"
MAIN_THREAD_TIMER = ROOT / "research" / "ce_main_thread_timer.lua"
BASE = 0x140000000
ENTRY = BASE + 0x818018
EXIT = BASE + 0x8180A3
ROW_ORIGINAL = 0x40000000
ROW_ALTERNATE = 0x40001000
RECORD = 0x30000000
IDENTITY = {
    "process_id": 777,
    "creation_filetime": "134357108996277385",
    "image_size": 77830112,
    "executable_sha256": "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130",
    "file_version": "2.0.2.0",
}


MOCK = r'''
B=0x140000000;ROW=0x40000000;ALT=0x40001000;REC=0x30000000
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
  local expected=(NIOH3_ARMOR_WEIGHT_MAX_SECONDS or 30)*1000
  for _,t in ipairs(timers) do if not t.destroyed and t.Enabled and t.Interval==expected then t.OnTimer(t);return true end end
  return false
end
function hit(rva) RIP=B+rva;THREADID=thread;local site=bp[RIP];assert(site,'breakpoint not armed');return site.fn() end
function prepareRow(a,id,weight) fill(a,0x154);put(a+0x152,id,2);put(a+0x98,weight,4) end
function prepareRecord(item,ma,mb,level,plus,rarity)
  fill(REC,0xF0);put(REC,item,2);put(REC+0x06,level,2);put(REC+0x0A,plus,2)
  put(REC+0x30,rarity,1);put(REC+0x31,ma,1);put(REC+0x32,mb,1)
end
function enter(item,ma,mb,level,plus,rarity,stack,row)
  prepareRecord(item,ma,mb,level,plus,rarity);RSP=stack;RCX=row or ROW;RDX=REC
  return hit(0x818018)
end
function leave(raw,stack,row)
  RSP=stack-0x28;R10=row or ROW;RAX=raw;EAX=0;return hit(0x8180A3)
end
bptExecute=0;bpmDebugRegister=1;co_run=0
putHex(B+0x818018,'48895C2408574883EC20');putHex(B+0x8180A3,'4883C420')
prepareRow(ROW,0x609C,27);prepareRow(ALT,0x0F34,27)
'''


def lua_literal(value: object) -> str:
    if isinstance(value, str):
        # Lua has no JSON ``\\uXXXX`` string escape. Encode UTF-8 bytes using
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


def fixture(**changes: object) -> Lua54:
    lua = Lua54()
    lua.run(MOCK)
    identity = dict(IDENTITY)
    identity.update(changes.pop("identity", {}))
    lines = [
        f"NIOH3_ARMOR_WEIGHT_TARGET_IDENTITY={lua_literal(identity)}",
        'NIOH3_ARMOR_WEIGHT_RUN_ID="synthetic-weight"',
        f"NIOH3_BREAKPOINT_LIFECYCLE_PATH={lua_literal('research/owned_breakpoint_lifecycle_ce.lua')}",
        f"NIOH3_ARMOR_WEIGHT_MAIN_THREAD_TIMER_PATH={lua_literal('research/ce_main_thread_timer.lua')}",
    ]
    for name, value in changes.items():
        lines.append(f"{name}={lua_literal(value)}")
    lua.run(";".join(lines))
    return lua


def prepare(lua: Lua54) -> None:
    # Execute the checked-in source bytes directly so the Lua mock does not
    # depend on Windows ANSI handling of the Chinese checkout path.
    lua.run(SOURCE.read_text(encoding="utf-8"))


def arm(lua: Lua54) -> None:
    prepare(lua)
    lua.run("return nioh3ArmorRemodelWeightObserver.arm()")


def probe(lua: Lua54) -> dict:
    return lua.run("return nioh3ArmorRemodelWeightObserver")


def stop_and_drain(lua: Lua54) -> dict:
    lua.run("nioh3ArmorRemodelWeightObserver.stop('test_stop');drain()")
    return probe(lua)


def test_lua_literal_encodes_utf8_without_json_unicode_escapes() -> None:
    value = r"C:\研究\仁王3绘卷生成器项目\research\owned_breakpoint_lifecycle_ce.lua"
    encoded = lua_literal(value)
    assert r"\u" not in encoded
    lua = Lua54()
    assert lua.run(f"return {encoded}") == value


def test_bootstrap_snapshot_arm_stop_expressions() -> None:
    lua = fixture()
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelWeightObserver.status()")
    assert snapshot["prepared"] is True
    assert snapshot["active"] is False

    armed = lua.run("return nioh3ArmorRemodelWeightObserver.arm()")
    assert armed["armed"] is True
    assert armed["active"] is True

    stopping = lua.run("return nioh3ArmorRemodelWeightObserver.stop('snapshot_stop')")
    assert stopping["active"] is False
    assert stopping["cleanup_pending"] is True
    lua.run("drain()")
    closed = lua.run("return nioh3ArmorRemodelWeightObserver.status()")
    assert closed["cleanup_verified"] is True
    assert closed["process_hook_restored"] is True


def test_nonboolean_debug_is_broken_uses_context_fallback_and_records_type() -> None:
    lua = fixture(brokenMode="nonboolean")
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelWeightObserver.status()")
    assert snapshot["debugger_broken"] is False
    assert snapshot["debugger_broken_source"] == "debug_context_fallback:function"
    assert lua.run("return nioh3ArmorRemodelWeightObserver.arm()")["active"] is True
    stop_and_drain(lua)


def test_nonboolean_debug_is_broken_fallback_query_failure_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackFailure=True)
    with pytest.raises(RuntimeError, match="fallback query failed"):
        prepare(lua)


def test_nonboolean_debug_is_broken_true_context_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackContext={})
    with pytest.raises(RuntimeError, match="stopped before prepare"):
        prepare(lua)


def test_positive_and_negative_controls_pair_by_thread_and_stack() -> None:
    lua = fixture()
    prepare(lua)
    assert probe(lua)["prepared"] is True
    assert probe(lua)["active"] is False
    assert lua.run("return armCount") == 0
    lua.run("nioh3ArmorRemodelWeightObserver.arm()")
    lua.run("enter(0x609C,1,0,180,0,4,0x5000,ROW);leave(27,0x5000,ROW)")
    lua.run("enter(0xED50,2,0,180,20,4,0x6000,ROW);leave(55,0x6000,ALT)")
    value = probe(lua)
    assert value["active"] is True
    assert value["arm_inventory"] == ["0x140818018", "0x1408180A3"]
    assert value["events"] and value["events"][1]["thread_id_source"] == "debug_event_context:THREADID"
    assert len(value["events"]) == 2
    negative, positive = value["events"]
    assert (negative["item_id"], negative["control"], negative["mode_label"]) == (
        0x609C,
        "negative",
        "strengthened",
    )
    assert negative["mode"] == "1/0"
    assert negative["thickening_bytes"] == 0
    assert (positive["item_id"], positive["control"], positive["mode_label"]) == (
        0xED50,
        "positive",
        "thickened",
    )
    assert positive["thickening_bytes"] == 1
    assert positive["entry_static_row_id"] == 0x609C
    assert positive["selected_static_row_id"] == 0x0F34
    assert positive["selected_base_weight_raw"] == 27
    assert positive["effective_weight_raw"] == 55
    assert "record_address" not in positive and "raw_record" not in positive
    assert lua.run("return continues") == 4
    closed = stop_and_drain(lua)
    assert closed["cleanup_verified"] is True
    assert closed["global_breakpoints"] in ([], {})
    assert closed["process_hook_restored"] is True
    assert closed["timer_cleanup"]["budget_timer_destroyed"] is True
    assert closed["timer_cleanup"]["cleanup_timer_destroyed"] is True


def test_foreign_item_and_stale_exit_are_continued_without_output() -> None:
    lua = fixture()
    arm(lua)
    lua.run("enter(0x1234,1,2,180,20,4,0x5000,ROW)")
    lua.run("enter(0x609C,1,0,180,20,4,0x6000,ROW);thread=8;leave(27,0x6000,ROW)")
    value = probe(lua)
    assert value["ignored_hits"] == 1
    assert value["stale_pairs"] == 1
    assert value["events"] in ([], {})
    assert lua.run("return continues") == 3
    stop_and_drain(lua)


@pytest.mark.parametrize("mutation", ["attachedPid=778", "attachedHandle=998", "function getAddressSafe(_) return B+0x1000 end"])
def test_callback_rechecks_target_process_identity_and_still_resumes(mutation: str) -> None:
    lua = fixture()
    arm(lua)
    lua.run(f"{mutation};enter(0x609C,1,0,180,20,4,0x5000,ROW)")
    value = probe(lua)
    assert value["stop_reason"] == "process_changed"
    assert value["active"] is False
    assert lua.run("return continues") == 1


def test_callback_does_not_promote_ce_callback_thread_to_target_identity() -> None:
    lua = fixture()
    arm(lua)
    # ``hit`` copies the mock's target-thread source into THREADID. Poison the
    # source itself while leaving the CE callback-thread getters nonzero; the
    # observer must fail closed instead of promoting those getters to target
    # identity.
    lua.run("thread=nil;enter(0x609C,1,0,180,20,4,0x5000,ROW)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "Target debug-event thread ID unavailable" in value["error"]
    assert lua.run("return continues") == 1


def test_each_callback_continues_after_read_error() -> None:
    lua = fixture()
    arm(lua)
    lua.run("prepareRecord(0x609C,1,0,180,20,4);mem[REC+0x06]=nil;RSP=0x5000;RCX=ROW;RDX=REC;hit(0x818018)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "Unreadable record level" in value["error"]
    assert lua.run("return continues") == 1
    closed = stop_and_drain(lua)
    assert closed["cleanup_verified"] is True


def test_ce_nil_arm_return_is_accepted_only_after_inventory_proof() -> None:
    lua = fixture(armReturnsNil=True)
    arm(lua)
    value = probe(lua)
    assert value["active"] is True
    assert value["arm_inventory"] == ["0x140818018", "0x1408180A3"]
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_prepare_does_not_install_process_hook_and_reload_requires_clean_owner() -> None:
    lua = fixture()
    prepare(lua)
    assert lua.run("return onOpenProcess == nil") is True
    lua.run("nioh3ArmorRemodelWeightObserver.arm()")
    with pytest.raises(RuntimeError, match="still active"):
        lua.run(SOURCE.read_text(encoding="utf-8"))
    closed = stop_and_drain(lua)
    assert closed["process_hook_restored"] is True
    assert lua.run("return onOpenProcess == nil") is True
    prepare(lua)
    assert probe(lua)["prepared"] is True


def test_hit_budget_is_bounded_and_still_resumes_stopped_callbacks() -> None:
    lua = fixture()
    arm(lua)
    lua.run("for i=1,64 do enter(0x1234,0,0,1,0,1,0x5000+i,ROW) end")
    value = probe(lua)
    assert value["stop_reason"] == "ignored_hit_budget"
    assert value["total_hits"] == 64
    assert len(value["events"]) == 0
    assert lua.run("return continues") == 64
    lua.run("enter(0x1234,0,0,1,0,1,0x7000,ROW)")
    assert lua.run("return continues") == 65
    assert probe(lua)["total_hits"] == 64
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_time_budget_and_budget_timer_cleanup_are_explicit() -> None:
    lua = fixture()
    arm(lua)
    lua.run("tick=31001;enter(0x1234,0,0,1,0,1,0x5000,ROW)")
    value = probe(lua)
    assert value["stop_reason"] == "time_budget"
    assert value["timer_cleanup"]["budget_timer_destroy_requested"] is True
    assert lua.run("return continues") == 1
    closed = stop_and_drain(lua)
    assert closed["cleanup_verified"] is True


def test_configured_120_second_window_keeps_bounds_and_cleans_up() -> None:
    lua = fixture(NIOH3_ARMOR_WEIGHT_MAX_SECONDS=120)
    arm(lua)
    value = probe(lua)
    assert value["max_seconds"] == 120
    assert lua.run("return timers[1].Interval") == 120000

    lua.run("tick=121001;enter(0x1234,0,0,1,0,1,0x5000,ROW)")
    value = probe(lua)
    assert value["stop_reason"] == "time_budget"
    assert value["active"] is False
    assert value["max_hits"] == 128
    assert value["max_events"] == 64
    closed = stop_and_drain(lua)
    assert closed["cleanup_verified"] is True


@pytest.mark.parametrize("seconds", [0, 121])
def test_invalid_configured_window_is_rejected_before_debugger_arm(seconds: int) -> None:
    lua = fixture(NIOH3_ARMOR_WEIGHT_MAX_SECONDS=seconds, debugging=False)
    with pytest.raises(RuntimeError, match="NIOH3_ARMOR_WEIGHT_MAX_SECONDS"):
        prepare(lua)
    assert lua.run("return attachedWith") == 0
    assert lua.run("return armCount") == 0


def test_foreign_breakpoint_is_refused_and_never_removed() -> None:
    lua = fixture()
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
        lua = fixture(identity={"executable_sha256": "bad"})
    elif mutation == "pid":
        lua = fixture(attachedPid=778)
    elif mutation == "size":
        lua = fixture(identity={"image_size": 77814240})
    elif mutation == "version":
        lua = fixture(identity={"file_version": "2.0.1.0"})
    else:
        lua = fixture()
        lua.run("mem[B+0x8180A3]=0")
    with pytest.raises(RuntimeError, match=pattern):
        arm(lua)
    assert lua.run("return armCount") == 0


def test_existing_non_veh_debugger_and_failed_cleanup_remain_bounded() -> None:
    lua = fixture(iface=1)
    with pytest.raises(RuntimeError, match="VEH"):
        arm(lua)
    assert lua.run("return armCount") == 0

    lua = fixture()
    arm(lua)
    lua.run("removeFail=true;nioh3ArmorRemodelWeightObserver.stop('failed_cleanup');drain()")
    value = probe(lua)
    assert value["cleanup_pending"] is True
    assert value["cleanup_verified"] is False
    lua.run("removeFail=false;nioh3ArmorRemodelWeightObserver.retry_cleanup();drain()")
    assert probe(lua)["cleanup_verified"] is True


def test_source_is_read_only_and_keeps_the_two_site_bound() -> None:
    source = SOURCE.read_text(encoding="utf-8")
    assert "debugProcess(2)" in source
    assert "MAX_EVENTS, MAX_HITS, MAX_TARGET_ENTRIES, MAX_IGNORED_HITS, MAX_SECONDS" in source
    assert "pcall(debug_setBreakpoint, address, 1" in source
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
