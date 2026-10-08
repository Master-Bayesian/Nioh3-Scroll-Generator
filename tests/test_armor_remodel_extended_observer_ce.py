"""Offline Lua mocks for the bounded armor extended CE observer.

The harness has no Cheat Engine process or game image. It exercises the
requirements observer and the combined weight/toughness observer with native
register, breakpoint, timer, and lifecycle stand-ins.
"""

from __future__ import annotations

from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).parent))
from lua54_test_runtime import Lua54


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "research" / "armor_remodel_extended_observer_ce.lua"
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
WEIGHT_ENTRY = BASE + 0x818018
WEIGHT_EXIT = BASE + 0x8180A3
TOUGH_ENTRY = BASE + 0x817FD4
TOUGH_EXIT = BASE + 0x81800B


MOCK = r'''
B=0x140000000;ROW=0x40000000;ALT=0x40001000;REC=0x30000000
mem={};bp={};timers={};attachedPid=777;attachedHandle=999;debugging=true;iface=2
targetThread=7;THREADID=7;ceThread=7007;tick=1000;armCount=0;armFail=0;armReturnsNil=false;removeCalls={};removeFail=false
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
  for a,s in pairs(bp) do if s.id==id then return debug_removeBreakpoint(a) end end;return true
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
  local expected=(NIOH3_ARMOR_EXTENDED_MAX_SECONDS or 120)*1000
  for _,t in ipairs(timers) do if not t.destroyed and t.Enabled and t.Interval==expected then t.OnTimer(t);return true end end
  return false
end
function hit(rva) RIP=B+rva;THREADID=targetThread;local site=bp[RIP];assert(site,'breakpoint not armed');return site.fn() end
function prepareRow(a,id,weight)
  fill(a,0x300,0);put(a+0x152,id,2);put(a+0x98,weight,4)
end
function prepareRecord(item,ma,mb,level,plus,rarity,flags)
  fill(REC,0xF0);put(REC,item,2);put(REC+0x06,level,2);put(REC+0x0A,plus,2)
  put(REC+0x18,flags or 0,4);put(REC+0x30,rarity,1);put(REC+0x31,ma,1);put(REC+0x32,mb,1)
end
function enterReq(item,ma,mb,level,plus,rarity,stack,row,flags,index)
  prepareRecord(item,ma,mb,level,plus,rarity,flags);prepareRow(row or ROW,0x609C,0x1111)
  RSP=stack;RCX=row or ROW;RDX=REC;R8=index or 0;return hit(0x2F9E30)
end
function leaveReq(raw,stack)
  RAX=raw;EAX=0;R8=0xDEADBEEF;RSP=stack-0x28;return hit(0x2F9E73)
end
function enterWeight(item,ma,mb,level,plus,rarity,stack,row,flags)
  prepareRecord(item,ma,mb,level,plus,rarity,flags);prepareRow(row or ROW,0x609C,0x1111)
  RSP=stack;RCX=row or ROW;RDX=REC;R8=0xAAAA;return hit(0x818018)
end
function leaveWeight(raw,stack,selected)
  RAX=raw;EAX=0;R8=0xDEADBEEF;R10=selected or ALT;RSP=stack-0x28;return hit(0x8180A3)
end
function enterTough(item,ma,mb,level,plus,rarity,stack,row,flags)
  prepareRecord(item,ma,mb,level,plus,rarity,flags);prepareRow(row or ROW,0x609C,0x1111)
  RSP=stack;RCX=row or ROW;RDX=REC;R8=0xBBBB;return hit(0x817FD4)
end
function leaveTough(raw,stack)
  RAX=raw;EAX=0;R8=0xDEADBEEF;RSP=stack-0x28;return hit(0x81800B)
end
prepareRow(ROW,0x609C,0x1111);prepareRow(ALT,0xED50,0x2222)
bptExecute=0;bpmDebugRegister=1;co_run=0
putHex(B+0x2F9E30,'4883EC284183F806');putHex(B+0x2F9E73,'4883C428C3')
putHex(B+0x818018,'48895C2408574883EC20');putHex(B+0x8180A3,'4883C420')
putHex(B+0x817FD4,'48895C24084889742410574883EC20');putHex(B+0x81800B,'488B5C24304883C4205FC3')
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


def fixture(kind: str = "requirements", **changes: object) -> Lua54:
    lua = Lua54()
    lua.run(MOCK)
    identity = dict(IDENTITY)
    identity.update(changes.pop("identity", {}))
    lines = [
        f"NIOH3_ARMOR_EXTENDED_TARGET_IDENTITY={lua_literal(identity)}",
        'NIOH3_ARMOR_EXTENDED_RUN_ID="synthetic-extended"',
        f"NIOH3_ARMOR_EXTENDED_KIND={lua_literal(kind)}",
        f"NIOH3_ARMOR_EXTENDED_LIFECYCLE_PATH={lua_literal('research/owned_breakpoint_lifecycle_ce.lua')}",
        f"NIOH3_ARMOR_EXTENDED_MAIN_THREAD_TIMER_PATH={lua_literal('research/ce_main_thread_timer.lua')}",
    ]
    for name, value in changes.items():
        lines.append(f"{name}={lua_literal(value)}")
    lua.run(";".join(lines))
    return lua


def prepare(lua: Lua54) -> None:
    # Execute checked-in source bytes directly so the mock does not depend on
    # Windows ANSI handling of the Chinese checkout path.
    lua.run(SOURCE.read_text(encoding="utf-8"))


def arm(lua: Lua54) -> None:
    prepare(lua)
    lua.run("return nioh3ArmorRemodelExtendedObserver.arm()")


def probe(lua: Lua54) -> dict:
    return lua.run("return nioh3ArmorRemodelExtendedObserver")


def stop_and_drain(lua: Lua54) -> dict:
    lua.run("nioh3ArmorRemodelExtendedObserver.stop('test_stop');drain()")
    return probe(lua)


def test_lua_literal_encodes_utf8_without_json_unicode_escapes() -> None:
    value = r"C:\研究\仁王3绘卷生成器项目\research\owned_breakpoint_lifecycle_ce.lua"
    encoded = lua_literal(value)
    assert r"\u" not in encoded
    lua = Lua54()
    assert lua.run(f"return {encoded}") == value


@pytest.mark.parametrize("kind", ["requirements", "weight_toughness"])
def test_bootstrap_snapshot_arm_stop_expressions(kind: str) -> None:
    lua = fixture(kind)
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelExtendedObserver.status()")
    assert snapshot["prepared"] is True
    assert snapshot["active"] is False
    assert snapshot["observer_kind"] == kind
    assert snapshot["site_count"] == (2 if kind == "requirements" else 4)

    armed = lua.run("return nioh3ArmorRemodelExtendedObserver.arm()")
    assert armed["armed"] is True
    assert armed["active"] is True
    expected = (
        [f"0x{REQ_ENTRY:X}", f"0x{REQ_EXIT:X}"]
        if kind == "requirements"
        else sorted(
            [
                f"0x{WEIGHT_ENTRY:X}",
                f"0x{WEIGHT_EXIT:X}",
                f"0x{TOUGH_ENTRY:X}",
                f"0x{TOUGH_EXIT:X}",
            ],
            key=lambda value: int(value, 16),
        )
    )
    assert probe(lua)["arm_inventory"] == expected
    stopping = lua.run("return nioh3ArmorRemodelExtendedObserver.stop('snapshot_stop')")
    assert stopping["active"] is False
    assert stopping["cleanup_pending"] is True
    lua.run("drain()")
    closed = lua.run("return nioh3ArmorRemodelExtendedObserver.status()")
    assert closed["cleanup_verified"] is True
    assert closed["process_hook_restored"] is True


def test_nonboolean_debug_is_broken_uses_context_fallback_and_records_type() -> None:
    lua = fixture(brokenMode="nonboolean")
    prepare(lua)
    snapshot = lua.run("return nioh3ArmorRemodelExtendedObserver.status()")
    assert snapshot["debugger_broken"] is False
    assert snapshot["debugger_broken_source"] == "debug_context_fallback:function"
    assert lua.run("return nioh3ArmorRemodelExtendedObserver.arm()")["active"] is True
    stop_and_drain(lua)


def test_nonboolean_debug_is_broken_fallback_query_failure_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackFailure=True)
    with pytest.raises(RuntimeError, match="fallback query failed"):
        prepare(lua)


def test_nonboolean_debug_is_broken_true_context_fails_closed() -> None:
    lua = fixture(brokenMode="nonboolean", fallbackContext={})
    with pytest.raises(RuntimeError, match="stopped before prepare"):
        prepare(lua)


def test_requirements_controls_indexes_and_zero_result_use_entry_r8() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run(
        "enterReq(0x609C,0,0,180,20,4,0x5000,ROW,0,0);leaveReq(0x100000005,0x5000);"
        "enterReq(0x609C,1,0,180,20,4,0x6000,ROW,0,1);leaveReq(6,0x6000);"
        "enterReq(0xED50,1,1,180,20,4,0x7000,ROW,0,2);leaveReq(7,0x7000);"
        "enterReq(0xED50,1,2,180,20,4,0x8000,ROW,0,3);leaveReq(8,0x8000);"
        "enterReq(0x609C,2,0,180,20,4,0x9000,ROW,0,4);leaveReq(0,0x9000);"
        "enterReq(0x609C,2,2,180,20,4,0xA000,ROW,0,6);leaveReq(9,0xA000)"
    )
    value = probe(lua)
    assert value["max_hits"] == 256
    assert value["max_events"] == 128
    assert value["max_target_entries"] == 128
    assert value["max_read_bytes"] == 8192
    assert [(event["mode"], event["control"]) for event in value["events"]] == [
        ("0/0", "negative"),
        ("1/0", "positive"),
        ("1/1", "positive"),
        ("1/2", "positive"),
        ("2/0", "negative"),
        ("2/2", "negative"),
    ]
    assert [event["stat_index"] for event in value["events"]] == [0, 1, 2, 3, 4, 6]
    assert value["events"][0]["requirement_raw"] == 5
    assert value["events"][0]["effective_value_raw"] == 5
    assert value["events"][5]["requirement_raw"] == 9
    assert lua.run("return continues") == 12
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_requirements_rejects_invalid_stat_index_and_resumes() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("enterReq(0x609C,1,0,180,20,4,0x5000,ROW,0,7)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "outside 0..6" in value["error"]
    assert lua.run("return continues") == 1
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_combined_interleaving_uses_kind_in_pair_key_and_avoids_stale_aliases() -> None:
    lua = fixture("weight_toughness")
    arm(lua)
    lua.run(
        "enterWeight(0x609C,1,2,180,20,4,0x5000,ROW,0x1234);"
        "enterTough(0xED50,1,2,180,20,4,0x5000,ROW,0x5678);"
        "leaveWeight(0x10000002A,0x5000,ALT);"
        "leaveTough(0x100000037,0x5000)"
    )
    value = probe(lua)
    assert value["max_hits"] == 256
    assert value["max_events"] == 128
    assert value["max_read_bytes"] == 16384
    assert len(value["events"]) == 2
    weight, toughness = value["events"]
    assert weight["call_kind"] == "weight"
    assert weight["thickening_bytes"] == 1
    assert weight["weight_raw"] == 0x2A
    assert weight["effective_weight_raw"] == 0x2A
    assert weight["entry_static_row_id"] == 0x609C
    assert weight["entry_base_weight_raw"] == 0x1111
    assert weight["selected_static_row_id"] == 0xED50
    assert weight["selected_base_weight_raw"] == 0x2222
    assert toughness["call_kind"] == "toughness"
    assert toughness["toughness_raw"] == 0x37
    assert toughness["toughness_flags_0x18"] == 0x5678
    assert "selected_static_row_id" not in toughness
    assert "selected_base_weight_raw" not in toughness
    assert weight["thread_id_source"] == "debug_event_context:THREADID"
    assert lua.run("return continues") == 4
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_toughness_wrong_0x20_stack_delta_does_not_pair() -> None:
    lua = fixture("weight_toughness")
    arm(lua)
    lua.run(
        "enterTough(0x609C,1,0,180,20,4,0x5000,ROW,0x40000);"
        "RSP=0x5000-0x20;RAX=7;EAX=0;hit(0x81800B);"
        "leaveTough(0x10000003B,0x5000)"
    )
    value = probe(lua)
    assert value["stale_pairs"] == 1
    assert len(value["events"]) == 1
    assert value["events"][0]["toughness_raw"] == 0x3B
    assert lua.run("return continues") == 3
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_fixed_six_targets_include_known_control_and_ignore_foreign_item() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run(
        "enterReq(0x53A0,1,0,180,20,4,0x5000,ROW,0,0);leaveReq(11,0x5000);"
        "enterReq(0x1234,0,0,180,20,4,0x6000,ROW,0,0)"
    )
    value = probe(lua)
    assert len(value["events"]) == 1
    assert value["events"][0]["item_id"] == 0x53A0
    assert value["ignored_hits"] == 1
    assert lua.run("return continues") == 3
    assert stop_and_drain(lua)["cleanup_verified"] is True


@pytest.mark.parametrize(
    "expression,pattern,expected_continues",
    [
        (
            "prepareRecord(0x609C,1,0,180,20,4,0);mem[REC+0x06]=nil;RSP=0x5000;RCX=ROW;RDX=REC;hit(0x818018)",
            "Unreadable record level",
            1,
        ),
        (
            "enterWeight(0x609C,1,0,180,20,4,0x5000,ROW,0);R10=0;RSP=0x5000-0x28;hit(0x8180A3)",
            "Weight selected row register",
            2,
        ),
        (
            "prepareRecord(0x609C,1,0,180,20,4,0);mem[REC+0x06]=nil;RSP=0x5000;RCX=ROW;RDX=REC;hit(0x817FD4)",
            "Unreadable record level",
            1,
        ),
        (
            "enterTough(0x609C,1,0,180,20,4,0x5000,ROW,0);RAX=nil;RSP=0x5000-0x28;hit(0x81800B)",
            "Exit effective stat register",
            2,
        ),
    ],
)
def test_each_combined_callback_error_still_resumes(
    expression: str, pattern: str, expected_continues: int
) -> None:
    lua = fixture("weight_toughness")
    arm(lua)
    lua.run(expression)
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and pattern in value["error"]
    assert lua.run("return continues") == expected_continues
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_callback_requires_target_thread_and_does_not_use_ce_callback_thread() -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run("targetThread=nil;enterReq(0x609C,1,0,180,20,4,0x5000,ROW,0,0)")
    value = probe(lua)
    assert value["stop_reason"] == "capture_error"
    assert value["error"] and "Target debug-event thread ID unavailable" in value["error"]
    assert lua.run("return continues") == 1


@pytest.mark.parametrize(
    "mutation",
    [
        "attachedPid=778",
        "attachedHandle=998",
        "function getAddressSafe(_) return B+0x1000 end",
    ],
)
def test_callback_rechecks_process_identity_and_resumes(mutation: str) -> None:
    lua = fixture("requirements")
    arm(lua)
    lua.run(f"{mutation};enterReq(0x609C,1,0,180,20,4,0x5000,ROW,0,0)")
    value = probe(lua)
    assert value["stop_reason"] == "process_changed"
    assert value["active"] is False
    assert lua.run("return continues") == 1


def test_ce_nil_arm_return_is_accepted_only_after_inventory_proof() -> None:
    lua = fixture("weight_toughness", armReturnsNil=True)
    arm(lua)
    value = probe(lua)
    assert value["active"] is True
    assert value["arm_inventory"] == sorted(
        [
            f"0x{WEIGHT_ENTRY:X}",
            f"0x{WEIGHT_EXIT:X}",
            f"0x{TOUGH_ENTRY:X}",
            f"0x{TOUGH_EXIT:X}",
        ],
        key=lambda value: int(value, 16),
    )
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_prepare_does_not_install_hook_and_reload_requires_clean_owner() -> None:
    lua = fixture("requirements")
    prepare(lua)
    assert lua.run("return onOpenProcess == nil") is True
    lua.run("nioh3ArmorRemodelExtendedObserver.arm()")
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
    lua.run("for i=1,64 do enterReq(0x1234,0,0,1,0,1,0x5000+i,ROW,0,0) end")
    value = probe(lua)
    assert value["stop_reason"] == "ignored_hit_budget"
    assert value["total_hits"] == 64
    assert len(value["events"]) == 0
    assert lua.run("return continues") == 64
    lua.run("enterReq(0x1234,0,0,1,0,1,0x7000,ROW,0,0)")
    assert lua.run("return continues") == 65
    assert probe(lua)["total_hits"] == 64
    assert stop_and_drain(lua)["cleanup_verified"] is True


def test_configured_120_second_combined_window_uses_timer_and_cleans_up() -> None:
    lua = fixture("weight_toughness", NIOH3_ARMOR_EXTENDED_MAX_SECONDS=120)
    arm(lua)
    value = probe(lua)
    assert value["max_seconds"] == 120
    assert value["max_hits"] == 256
    assert value["max_events"] == 128
    assert value["max_read_bytes"] == 16384
    assert lua.run("return timers[1].Interval") == 120000
    assert lua.run("return fireBudget()") is True
    value = probe(lua)
    assert value["stop_reason"] == "time_budget"
    assert value["active"] is False
    assert stop_and_drain(lua)["cleanup_verified"] is True


@pytest.mark.parametrize("seconds", [0, 121])
def test_invalid_window_is_rejected_before_debugger_arm(seconds: int) -> None:
    lua = fixture(
        "requirements", NIOH3_ARMOR_EXTENDED_MAX_SECONDS=seconds, debugging=False
    )
    with pytest.raises(RuntimeError, match="NIOH3_ARMOR_EXTENDED_MAX_SECONDS"):
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

    lua = fixture("weight_toughness")
    arm(lua)
    lua.run("removeFail=true;nioh3ArmorRemodelExtendedObserver.stop('failed_cleanup');drain()")
    value = probe(lua)
    assert value["cleanup_pending"] is True
    assert value["cleanup_verified"] is False
    lua.run("removeFail=false;nioh3ArmorRemodelExtendedObserver.retry_cleanup();drain()")
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
