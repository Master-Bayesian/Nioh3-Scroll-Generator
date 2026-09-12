"""Run the actual Lua observer against a CE API mock, plus raw-binary assertions."""
from __future__ import annotations
import ctypes
import ctypes.util
import json
import os
from pathlib import Path
import re
import subprocess
import pytest

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'research/title_save_v201'


def run_lua(body: str) -> None:
    script = ('ROOT=' + json.dumps(str(ROOT)) + '\n' +
              (ROOT/'tests/title_save_lua_harness.lua').read_text() + '\n' + body).encode()
    executable = os.environ.get('TITLE_SAVE_LUA_EXECUTABLE')
    if executable:
        result = subprocess.run([executable, '-'], input=script, capture_output=True, timeout=15)
        assert result.returncode == 0, result.stderr.decode(errors='replace')
        return
    try:
        from lupa.lua54 import LuaRuntime
    except ImportError:
        LuaRuntime = None
    if LuaRuntime is not None:
        LuaRuntime(unpack_returned_tuples=True).execute(script.decode())
        return
    library = (os.environ.get('TITLE_SAVE_LUA_LIBRARY') or
               ctypes.util.find_library('lua5.4') or ctypes.util.find_library('lua54'))
    if not library:
        pytest.skip('Lua 5.4 unavailable; set TITLE_SAVE_LUA_EXECUTABLE or TITLE_SAVE_LUA_LIBRARY')
    lua = ctypes.CDLL(library)
    lua.luaL_newstate.restype = ctypes.c_void_p
    lua.luaL_openlibs.argtypes = [ctypes.c_void_p]
    lua.luaL_loadstring.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    lua.luaL_loadstring.restype = ctypes.c_int
    lua.lua_pcallk.argtypes = [ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_longlong,ctypes.c_void_p]
    lua.lua_pcallk.restype = ctypes.c_int
    lua.lua_tolstring.argtypes = [ctypes.c_void_p,ctypes.c_int,ctypes.POINTER(ctypes.c_size_t)]
    lua.lua_tolstring.restype = ctypes.c_char_p
    lua.lua_close.argtypes = [ctypes.c_void_p]
    state = lua.luaL_newstate()
    try:
        lua.luaL_openlibs(state)
        status = lua.luaL_loadstring(state, script)
        if not status:
            status = lua.lua_pcallk(state, 0, 0, 0, 0, None)
        assert status == 0, (lua.lua_tolstring(state,-1,None) or b'Lua failure').decode(errors='replace')
    finally:
        lua.lua_close(state)


@pytest.mark.parametrize('profile', ['ownership','snapshots','serialization','files','file_handles','loading','registry','worker'])
def test_four_site_profiles_arm_and_cleanup(profile):
    run_lua(f"local x=mock('{profile}');assert(x.start());assert(count(x.bp)==4);x.probe.stop();assert(count(x.bp)==0);assert(not x.status().cleanup_pending)")


def test_signature_mismatch_arms_nothing():
    run_lua("local x=mock();x.memory[base+loc.sites.writer_entry.signature_rva]=0;assert(not pcall(x.start));assert(x.arms==0)")


def test_unknown_executable_and_veh_rejected_before_arm():
    run_lua("local x=mock();x.api.identity=function() return {pid=10,creation_filetime='123',sha256='BAD',size=loc.disk_size,version='2.0.1.0'} end;assert(not pcall(x.start));assert(x.arms==0);local y=mock();y.veh=true;assert(not pcall(y.start));assert(y.arms==0)")


def test_unrelated_breakpoint_is_not_removed():
    run_lua("local x=mock();x.bp[0x777777]=function()end;assert(not pcall(x.start));assert(x.arms==0);assert(#x.removed==0);assert(x.bp[0x777777])")


def test_arming_failure_after_partial_side_effect_is_cleaned():
    run_lua("local x=mock();x.failarm=2;local ok=x.start();assert(not ok);assert(count(x.bp)==0);assert(not x.status().active)")


def test_failed_cleanup_retains_owner_and_retry_recovers():
    run_lua("local x=mock();assert(x.start());x.refuseremoval=true;x.probe.stop();assert(x.status().cleanup_pending);assert(#x.status().owned_breakpoints==4);x.refuseremoval=false;x.probe.retry_cleanup();assert(not x.status().cleanup_pending);assert(count(x.bp)==0)")


def test_changed_attachment_never_cleans_new_process_breakpoints():
    run_lua("local x=mock();assert(x.start());x.handle='new_process_same_pid';x.onlost();assert(not x.status().active);assert(x.status().cleanup_pending);assert(#x.removed==0);assert(count(x.bp)==4)")


def test_changed_pid_callback_resumes_and_preserves_unknown_ownership():
    run_lua("local x=mock();assert(x.start());x.pid=11;x.hit('request_entry');assert(x.continued==1);assert(not x.status().active);assert(x.status().cleanup_pending);assert(#x.removed==0)")


def test_busy_rejection_is_captured_without_guessing_eax():
    run_lua("local x=mock();x.fill(0x300000,0x80);x.write(0x300038,1,1);assert(x.start());x.hit('request_entry');x.regs.RSP=x.regs.RSP-0x28;x.regs.RAX=12345;x.hit('request_exit');local e=x.status().events[2];assert(e.disposition=='busy_rejected');assert(e.entry_sequence==1);assert(e.context.address=='0x300000')")


def test_free_request_exit_is_not_acceptance_proof():
    run_lua("local x=mock();x.fill(0x300000,0x80);assert(x.start());x.hit('request_entry');x.regs.RSP=x.regs.RSP-0x28;x.hit('request_exit');assert(x.status().events[2].disposition=='submitted_attempt_not_native_acceptance_proof');assert(x.status().release=='BLOCK')")


def test_request_pairing_uses_thread_and_frame():
    run_lua("local x=mock();x.fill(0x300000,0x80);assert(x.start());x.hit('request_entry');x.regs.RSP=x.regs.RSP-0x28;x.regs.THREADID=18;x.hit('request_exit');assert(x.status().events[2].unpaired)")


@pytest.mark.parametrize('al',[0,1])
def test_common_writer_epilogue_captures_early_failure_and_success(al):
    run_lua(f"local x=mock('files');assert(x.start());x.regs.RAX={al};x.hit('writer_exit');assert(x.status().events[1].return_al=={al});assert(x.continued==1)")


def test_file_write_count_and_rename_are_distinct_events():
    run_lua("local x=mock('files');x.write(0x200030,999,4);assert(x.start());x.regs.RAX=1;x.regs.RSI=0x8888;x.hit('writer_write_result');x.hit('writer_rename_result');assert(x.status().events[1].bytes_written==999);assert(x.status().events[2].return_u32==1)")


def test_completion_uses_rbx_not_operation_code_in_rdi():
    run_lua("local x=mock();x.fill(0x300000,0x80);x.regs.RDI=1;assert(x.start());x.hit('completion_consume');assert(x.status().events[1].context.address=='0x300000')")


def test_event_limit_stops_and_read_bounds_hold():
    run_lua("local x=mock('files',{max_events=4});assert(x.start());for i=1,5 do x.hit('writer_exit') end;assert(#x.status().events==4);assert(x.status().dropped==1);assert(not x.status().active);assert(count(x.bp)==0);assert(x.maxread<=512)")


def test_unreadable_fields_remain_unknown_not_zero():
    run_lua("local x=mock('serialization');assert(x.start());x.hit('serializer_entry');local t=x.status().events[1].task;assert(t.payload==nil);assert(t.payload_bytes==nil);assert(x.continued==1)")


def test_sink_failure_stops_without_masking_resume():
    run_lua("local x=mock('files');assert(x.start());x.failsink=true;x.hit('writer_exit');assert(not x.status().active);assert(count(x.bp)==0);assert(x.continued==1)")


def test_missing_thread_is_fatal_but_resumes():
    run_lua("local x=mock('files');assert(x.start());x.regs.THREADID=nil;x.hit('writer_exit');assert(x.status().stop_reason=='callback_failure');assert(x.continued==1)")


def test_clear_captures_requires_confirmed_cleanup():
    run_lua("local x=mock('files');assert(x.start());assert(not pcall(x.probe.clear_captures));x.refuseremoval=true;x.probe.stop();assert(not pcall(x.probe.clear_captures));x.refuseremoval=false;x.probe.retry_cleanup();x.probe.clear_captures();assert(x.status().epoch==1)")


def test_snapshot_digest_excludes_salt_and_never_claims_generation():
    run_lua("local x=mock('snapshots',{hash_payloads=true});assert(x.start());x.hit('snapshot_ready');local h=x.status().events[1].fingerprint;assert(h.bytes==0x900000);assert(h.purpose=='investigative_content_fingerprint_not_generation_ack')")


def test_paths_and_accounts_are_pseudonymized():
    run_lua(r"""local x=mock('files');x.fill(0x400000,512);x.fill(0x401000,512)
local d='C:\\Users\\Secret\\Savedata\\76561199999999999\\SAVEDATA00\\'
for i=1,#d do x.write(0x400000+(i-1)*2,d:byte(i),2) end
local f='SAVEDATA.BIN';for i=1,#f do x.write(0x401000+(i-1)*2,f:byte(i),2) end
x.regs.RCX=0x400000;x.regs.RDX=0x401000;assert(x.start());x.hit('writer_entry')
assert(not x.logs[1]:find('Secret'));assert(not x.logs[1]:find('76561199999999999'));assert(x.logs[1]:find('SAVEDATA00'))""")


def test_malformed_queue_cannot_cause_unbounded_reads():
    run_lua("local x=mock('files');x.write(base+0x45c4448,0x500000,8);x.fill(0x500000,0x40);x.write(0x500018,0x600000,8);x.write(0x500020,0x700000,8);assert(x.start());x.hit('writer_exit');assert(x.status().events[1].globals.queue_readable==false);assert(x.maxread<=512)")


def test_observer_contains_no_target_write_or_arbitrary_execution_api():
    source=(ROOT/'research/probe_title_save_ownership_ce.lua').read_text()
    uncommented='\n'.join(line.split('--')[0] for line in source.splitlines())
    assert not re.search(r'\b(?:writeBytes|writeInteger|writeQword|writePointer|writeString|autoAssemble|executeCodeEx|executeMethod|createRemoteThread|injectDLL|pause|unpause|debug_setContext|detachIfPossible|openProcess)\s*\(', uncommented)
    assert 'bpmDebugRegister' in source
    assert 'owned_breakpoint_lifecycle_ce.lua' in source
    assert not re.search(r'function\s+debugger_onBreakpoint', source)


def test_static_instruction_signatures_against_exact_provided_dump():
    directory=os.environ.get('TITLE_SAVE_SECTIONS')
    if not directory:
        pytest.skip('Set TITLE_SAVE_SECTIONS to the supplied raw sections; no bundled game binary')
    raw=(Path(directory)/'Nioh3_v2.0.1.0.text.bin').read_bytes()
    import hashlib
    assert hashlib.sha256(raw).hexdigest().upper()=='F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'
    loc=json.loads((RESEARCH/'locators.json').read_text())
    for name,site in loc['sites'].items():
        expected=bytes.fromhex(site['expected_hex']); mask=bytes.fromhex(site['mask_hex'])
        off=site['signature_rva']-0x1000
        assert raw[off:off+len(expected)]==expected,name
        assert site['rva'] in {x['rva'] for x in site['instructions']},name
        pattern=b''.join(re.escape(bytes([v])) if m else b'.' for v,m in zip(expected,mask))
        hits=[m.start()+0x1000 for m in re.finditer(pattern,raw,re.DOTALL)]
        assert hits==[site['signature_rva']],name
    # Semantic guardrails: callback uses RBX; read-exit is not the retry loop.
    assert loc['sites']['read_exit']['rva']==0x1fe1a4b
    assert raw[0x1b2d60-0x1000:0x1b2d60-0x1000+4].hex()=='44887338'
    assert raw[0x5b8093-0x1000:0x5b8093-0x1000+5].hex()=='e888c05d00'


def test_unknown_busy_byte_is_not_a_proven_rejection():
    run_lua("local x=mock();assert(x.start());x.hit('request_entry');x.regs.RSP=x.regs.RSP-0x28;x.hit('request_exit');assert(x.status().events[2].disposition=='unknown_busy_field_unreadable')")


def test_duplicate_request_frame_stops_without_replacing_evidence():
    run_lua("local x=mock();assert(x.start());x.hit('request_entry');x.hit('request_entry');assert(not x.status().active);assert(#x.status().events==1);assert(x.continued==2)")


def test_production_adapter_accepts_nil_arm_success_and_restores_handler():
    run_lua("local x=mock('files');local p,ok=use_production_adapter(x);assert(ok,'start');assert(count(x.bp)==4,'breakpoints');assert((x.synchronize_calls or 0)>=1,'watch synchronization');x.hit('writer_exit');assert(#p.status().events==1,'event');assert(x.continued==1,'continue');p.stop();assert(count(x.bp)==0,'cleanup');assert(MainForm.OnProcessOpened==x.previous_handler,'handler restoration');assert((x.synchronize_calls or 0)>=2,'unwatch synchronization')")


def test_identity_attestation_does_not_require_get_file_hash_cmdlet():
    source = (ROOT / 'research/probe_title_save_ownership_ce.lua').read_text(encoding='utf-8')
    assert 'Get-FileHash' not in source
    assert '[Security.Cryptography.SHA256]::Create()' in source


def test_production_adapter_same_pid_reattach_retains_unknown_owner():
    run_lua("local x=mock('files');local p,ok=use_production_adapter(x);assert(ok);x.handle='same_pid_new_handle';MainForm.OnProcessOpened();assert(x.previous_handler_calls==1);assert(not p.status().active);assert(p.status().cleanup_pending);assert(#x.removed==0)")


def test_production_adapter_does_not_replace_a_new_process_open_handler():
    run_lua("local x=mock('files');local p,ok=use_production_adapter(x);assert(ok);local other=function()end;MainForm.OnProcessOpened=other;p.stop();assert(MainForm.OnProcessOpened==other)")


def test_actual_lua_jsonl_export_refuses_overwrite(tmp_path):
    target=tmp_path/'C0.files.jsonl'
    run_lua(f"local x=mock('files');local p,ok=use_production_adapter(x);assert(ok);RAX=1;x.hit('writer_exit');p.stop();assert(M.export({json.dumps(str(target))})==1);assert(not pcall(M.export,{json.dumps(str(target))}))")
    rows=[json.loads(line) for line in target.read_text().splitlines()]
    assert rows[0]['process']['creation_filetime']=='134336000000000000'
    assert rows[1]['return_al']==1
    assert rows[-1]['active'] is False
    assert rows[-1]['continue_failed'] is False
    assert rows[-1]['owned_breakpoints'] in ([], {})
    assert rows[-1]['release']=='BLOCK'


def test_runtime_lua_locators_match_json_masks_and_offsets(tmp_path):
    target=tmp_path/'locators-runtime.json'
    run_lua(f"local f=assert(io.open({json.dumps(str(target))},'w'));assert(f:write(M.encode_json(loc)));f:close()")
    actual=json.loads(target.read_text())
    reference=json.loads((RESEARCH/'locators.json').read_text())
    assert actual['disk_sha256']==reference['disk_executable_sha256']
    assert actual['text_size']==reference['text_size']
    assert len(actual['sites'])==len(reference['sites'])==28
    for key,entry in reference['sites'].items():
        assert actual['sites'][key]=={'rva':entry['rva'],'signature_rva':entry['signature_rva'],'bytes':entry['expected_hex'],'mask':entry['mask_hex']}


def test_writer_rename_and_success_exit_do_not_reuse_clobbered_size_or_handle():
    run_lua("""local x=mock('files');assert(x.start());x.regs.R9=0x9001b0;x.hit('writer_entry')
x.regs.RSP=x.regs.RSP-0x8d8;x.regs.RDI=0x9001b0;x.regs.RSI=0x8888;x.regs.RAX=1;x.hit('writer_write_result')
x.regs.RDI=-1;x.regs.RSI=0x104;x.hit('writer_rename_result');x.hit('writer_exit')
local es=x.status().events;assert(es[3].bytes==0x9001b0);assert(es[3].handle==nil)
assert(es[3].previously_observed_handle=='0x8888');assert(es[4].bytes==0x9001b0)
assert(es[4].writer_start_sequence==1);assert(es[4].return_al==1)""")


def test_handle_profile_retains_size_from_open_for_common_exit():
    run_lua("""local x=mock('file_handles');assert(x.start());x.regs.RDI=1234;x.regs.RAX=0x8888;x.hit('writer_open_result')
x.regs.RDI=-1;x.regs.RSI=0x104;x.regs.RAX=1;x.hit('writer_exit')
local e=x.status().events[2];assert(e.bytes==1234);assert(e.writer_start_origin=='open_result');assert(e.handle==nil)""")


def test_unpaired_writer_exit_keeps_size_unknown():
    run_lua("local x=mock('files');assert(x.start());x.regs.RDI=-1;x.regs.RAX=1;x.hit('writer_exit');local e=x.status().events[1];assert(e.unpaired);assert(e.bytes==nil);assert(e.return_al==1)")


def test_async_cleanup_restores_attachment_handler_when_verified():
    run_lua("local x=mock('files');assert(x.start());x.refuseremoval=true;x.probe.stop();assert(not x.unwatched);x.refuseremoval=false;x.drain();assert(x.unwatched);assert(not x.status().cleanup_pending);assert(count(x.bp)==0)")


def test_cleanup_error_is_bounded_and_exported(tmp_path):
    target=tmp_path/'failed-cleanup.jsonl'
    run_lua(f"local x=mock('files');local p,ok=use_production_adapter(x);assert(ok);x.refuseremoval=true;p.stop();p.status().cleanup_errors={{string.rep('x',10000)}};M.export({json.dumps(str(target))})")
    status=json.loads(target.read_text().splitlines()[-1])
    assert status['cleanup_pending'] is True
    assert len(status['owned_breakpoints'])==4
    assert len(status['cleanup_errors'][0])==512


def test_attachment_change_during_slow_identity_query_arms_nothing():
    run_lua("local x=mock();local original=x.api.identity;x.api.identity=function() x.handle='replacement';return original() end;assert(not pcall(x.start));assert(x.arms==0);assert(#x.removed==0)")
