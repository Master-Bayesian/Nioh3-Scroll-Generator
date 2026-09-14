"""Execute the complete Lua file with real Lua 5.4 and synthetic CE memory.

These are collector/validator regressions, not new native mission captures.
"""
from pathlib import Path
import copy
import hashlib
import json
import struct
import sys

import pytest

ROOT=Path(__file__).resolve().parents[1]
CAP=ROOT/'research/possessed_enemy_capture'
sys.path[:0]=[str(CAP),str(Path(__file__).parent)]
from test_mode_upstream_lua import fixture as old_fixture, cleanup as old_cleanup, lua_literal
from validate_mode_upstream_sequence import validate, SITES, SCHEMA
from run_possessed_enemy_observer import PHASES, STRICT_V201_PHASES
from run_possessed_enemy_observer import SEQUENCE_OBSERVATION_SECONDS

LOC=json.loads((CAP/'mode_upstream_sequence_v201_locators.json').read_text())


def fixture(extra=0, tail='0000'):
    l=old_fixture(extra=extra,tail=tail)
    q='A8912D05B700010301'+f'{extra:02X}'+tail
    # Reset the whole queue before putting a target node. Session state is a
    # separate object, not a fabricated native mode enum.
    l.run(f'''fill(B+0x45B83E0,4*0x58);put(B+0x45B83E4,0xCC96,4);
    putHex(B+0x45B8400,'{q}');put(S,B+{0xF1E4F1 if extra==0 else 0x21D9E24},8);
    put(B+0x474D480,0x31000000,8);put(0x31000008,0x32000000,8);
    fill(0x32000000,0x44);put(0x32000028,0xCC96,4);
    putHex(0x32000038,'{q}');
    function consume() RSP=F+0x1778;RCX=B+0x45B8400;RDX=RSP+0x70;return hit(0x20E198C) end
    function generated() RSP=F+0x1780;return hit(0x2237978) end
    ''')
    for site in LOC['sites'].values():
        l.run(f"putHex(B+{int(site['rva'],16)},'{site['bytes']}')")
    prepare_output(l, extra)
    return l


def prepare_output(l, extra):
    header=bytearray(0x34)
    count=2 if extra else 1
    struct.pack_into('<3Q',header,0,0x60000000,0x60000028,0x60000028)
    header[0x1E]=0x8E;header[0x1F]=0xD4;header[0x24]=1
    struct.pack_into('<I',header,0x28,183);header[0x30]=3
    wave=bytearray(0x28)
    struct.pack_into('<3Q',wave,0,0x61000000,0x61000000+count*0x14,0x61000000+count*0x14)
    desc=bytearray(count*0x14)
    for i in range(count):
        struct.pack_into('<II',desc,i*0x14,0xF3C+i,0xDCB98)
        desc[i*0x14+0xE]=i+1;desc[i*0x14+0xF]=i;desc[i*0x14+0x10]=i
    l.run(f"putHex(F+0x17E8,'{header.hex()}');putHex(0x60000000,'{wave.hex()}');putHex(0x61000000,'{desc.hex()}')")


def arm(l):
    return l.run('return assert(loadfile('+lua_literal(str(CAP/'mode_upstream_sequence_ce.lua'))+'))()')


def capture(l):
    return l.run('return nioh3PossessedCapture')


def close_window(l, manual=False):
    if manual:
        l.run("nioh3PossessedCapture.stop('entry_observation_complete')")
    else:
        l.run('tick=121000;for _,t in ipairs(timers) do if t.ms==120000 then t.f();break end end')
    c=old_cleanup(l)
    return capture(l),c


def envelope(p):
    addresses=[hex(0x140000000+r) for r in SITES.values()]
    return {'capture_metadata':{'phase':'mode-upstream-sequence','run_id':p['run_id'],'requested_pid':p['pid'],
        'target_seed':p['target_seed'],'target':dict(p['identity'],module_base=None),
        'read_only':True,'writes_game_memory':False,'stop_on_first_final':False,
        'source':{'phase_file':'mode_upstream_sequence_ce.lua',
                  'phase_sha256':hashlib.sha256((CAP/'mode_upstream_sequence_ce.lua').read_bytes()).hexdigest(),
                  'lifecycle_sha256':hashlib.sha256((ROOT/'research/owned_breakpoint_lifecycle_ce.lua').read_bytes()).hexdigest()},
        'fresh_phase_initialization':{'result':{'initialized':True,'debugger_broken':False,'breakpoints':[]}},
        'arm_verification':{'result':{'active':True,'run_id':p['run_id'],'schema':SCHEMA,'debugger_broken':False,
                                    'owned_breakpoints':addresses,'breakpoints':addresses}}},
        'bridge_result':{'result':p}}


def run_first(l):
    arm(l);l.run('source();queued();consume();generated()')


def switch_request(l, extra=1, tail='0200', route=0x21D9E24):
    req=f'A8912D05B700010301{extra:02X}{tail}'
    l.run(f"putHex(C-0x50,'{req}');putHex(B+0x45B8400,'{req}');put(S,B+{route},8)")
    prepare_output(l, extra)


def test_does_not_stop_at_first_return_and_records_second_distinct_invocation():
    l=fixture();run_first(l)
    assert capture(l)['active'] and capture(l)['completed_invocations']==1
    switch_request(l);l.run('tick=2000;source();queued();consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['strictly_sequential_zero_then_nonzero']==[(1,2)]
    assert [g['request_extra'] for g in out['completed_invocations']]==[0,1]
    assert [g['descriptor_count'] for g in out['completed_invocations']]==[1,2]
    assert [g['producer_route'] for g in out['completed_invocations']]==['owned_scroll_branch','session_view_branch']
    assert not out['physical_actor_join_validated'] and not out['product_oracle_accepted']
    assert l.run('return armCount')==4 and l.run('return continues')==8
    assert l.run('return debug_getBreakpointList()')=={} and l.run('return onOpenProcess') is None


def test_identical_request_stack_and_output_address_reuse_after_completion_is_not_conflated():
    l=fixture();run_first(l);l.run('source();queued();consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert [g['invocation_id'] for g in out['completed_invocations']]==[1,2]
    assert [g['request_id'] for g in out['completed_invocations']]==[1,2]
    assert out['strictly_sequential_zero_then_nonzero']==[]


def test_overlapping_pending_identical_payloads_are_unbound_not_fifo_guessed():
    l=fixture();arm(l);l.run('source();queued();source();queued();consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    g=out['completed_invocations'][0]
    assert g['request_id'] is None and g['link_grade']=='ambiguous_pending_payload'
    assert out['pending_request_ids']==[1,2]
    assert out['status']=='complete_window_with_unresolved_links'


def test_two_pending_different_requests_do_not_bind_consume_to_latest_enqueue():
    l=fixture();arm(l);l.run('source();queued()')
    switch_request(l);l.run('source();queued()')
    # First request moves to the head and is consumed before the second one.
    switch_request(l,0,'0000',0xF1E4F1);l.run('consume();generated()')
    switch_request(l);l.run('consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert [g['request_id'] for g in out['completed_invocations']]==[1,2]


def test_unobserved_second_producer_still_records_later_generator_output():
    l=fixture();run_first(l);switch_request(l);l.run('consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['strictly_sequential_zero_then_nonzero']==[(1,2)]
    assert out['completed_invocations'][1]['producer_route'] is None
    assert out['completed_invocations'][1]['link_grade']=='unobserved_or_changed_request'


def test_unobserved_enqueue_can_have_queue_only_identity_but_no_producer_claim():
    l=fixture();arm(l);l.run('queued();consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['completed_invocations'][0]['producer_route'] is None
    assert out['completed_invocations'][0]['request_id']==1


def test_consumer_changed_tail_is_recorded_unbound_not_silently_normalized():
    l=fixture();arm(l);l.run('source();queued();put(B+0x45B840A,2,1);consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['completed_invocations'][0]['tail_hex']=='0200'
    assert out['completed_invocations'][0]['request_id'] is None
    assert out['pending_request_ids']==[1]


def test_requeue_reads_session_copy_and_retains_zero_extra():
    l=fixture();arm(l)
    l.run("""putHex(C-0x29,'A8912D05B700010301000000');put(S,B+0x2233DE8,8);RBP=C;RSP=S;RDX=C-0x29;hit(0x10D9180);
     RSP=S-0x658;R15=C-0x29;R12=B+0x45B83E0;RAX=0;hit(0x10D9368);consume();generated()""")
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['completed_invocations'][0]['producer_route']=='current_session_requeue'
    assert out['completed_invocations'][0]['request_extra']==0


def test_mismatched_session_requeue_is_falsifying_not_corrected():
    l=fixture();arm(l)
    l.run("putHex(C-0x29,'A8912D05B700010301010200');put(S,B+0x2233DE8,8);RBP=C;RSP=S;RDX=C-0x29;hit(0x10D9180)")
    assert 'Requeue source copy mismatch' in capture(l)['error']
    assert capture(l)['events'][0]['request_hex'].endswith('010200')
    old_cleanup(l)


def test_unknown_producer_observed_without_semantic_promotion():
    l=fixture();arm(l);l.run('put(S,B+0x12345,8);source();queued();consume();generated()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['completed_invocations'][0]['producer_route'] is None


@pytest.mark.parametrize('tail',['0000','0200','FFFF','ABCD'])
def test_tail_preserved_without_participant_count_interpretation(tail):
    l=fixture(tail=tail);run_first(l);p,c=close_window(l)
    assert validate(envelope(p),c)['completed_invocations'][0]['tail_hex']==tail.lower()


def test_only_zero_in_window_is_not_proof_no_later_pass_or_normal_solo():
    l=fixture();run_first(l);p,c=close_window(l)
    out=validate(envelope(p),c)
    assert out['observation_validated'] and not out['no_later_request_proven']
    assert not out['native_session_mode_decoded']


def test_no_target_events_is_neutral_incomplete_not_negative_mode_evidence():
    l=fixture();arm(l);p,c=close_window(l)
    out=validate(envelope(p),c)
    assert not out['observation_validated'] and out['status']=='no_target_generation_return'


def test_half_completed_later_invocation_is_reported_pending():
    l=fixture();run_first(l);switch_request(l);l.run('source();queued();consume()')
    p,c=close_window(l);out=validate(envelope(p),c)
    assert out['pending_invocation_ids']==[2]
    assert len(out['completed_invocations'])==1


def test_manual_owner_attested_end_permitted_not_session_finality():
    l=fixture();run_first(l);p,c=close_window(l,manual=True)
    assert validate(envelope(p),c)['observation_validated']


@pytest.mark.parametrize('site',list(LOC['sites']))
def test_all_four_signatures_checked_before_any_breakpoint(site):
    l=fixture();l.run(f"put(B+{int(LOC['sites'][site]['rva'],16)},0,1)")
    with pytest.raises(RuntimeError,match='signature mismatch'):arm(l)
    assert l.run('return armCount')==0


@pytest.mark.parametrize('n',[1,2,3,4])
def test_partial_arm_failure_cleans_only_owned_sites(n):
    l=fixture();l.run(f'armFail={n}')
    with pytest.raises(RuntimeError):arm(l)
    c=old_cleanup(l);assert c['cleanup_metadata']['verified']
    assert l.run('return #removeCalls')<=8


def test_foreign_breakpoint_not_removed():
    l=fixture();l.run('bp[123]=true')
    with pytest.raises(RuntimeError,match='Foreign'):arm(l)
    assert l.run('return #removeCalls')==0


def test_cleanup_failure_and_retry_retains_ownership():
    l=fixture();run_first(l);l.run('removeFail=true')
    p,c=close_window(l);assert p['cleanup_pending'] and not c['cleanup_metadata']['verified']
    with pytest.raises(ValueError,match='cleanup'):validate(envelope(p),c)
    l.run('removeFail=false;nioh3PossessedCapture.retry_cleanup();drain()')
    assert not capture(l)['cleanup_pending']


def test_same_pid_new_handle_never_cleans_other_process():
    l=fixture();arm(l);l.run('attachedHandle=1000;source()');old_cleanup(l)
    assert 'Process instance' in capture(l)['error'] and l.run('return #removeCalls')==0


def test_on_open_process_replacement_stops_and_preserves_owner():
    l=fixture();arm(l);l.run('attachedPid=778;onOpenProcess()');old_cleanup(l)
    assert capture(l)['stop_reason']=='process_changed' and l.run('return #removeCalls')==0


def test_deadline_holds_even_if_timer_callback_delayed():
    l=fixture();arm(l);l.run('tick=121001;source()')
    assert capture(l)['stop_reason']=='observation_window_elapsed'
    assert capture(l)['event_sequence']==0 and l.run('return continues')==1
    old_cleanup(l)


def test_callback_hit_bound_and_resume():
    l=fixture();arm(l);l.run('for i=1,4097 do generated() end')
    assert 'callback hit budget' in capture(l)['error']
    assert l.run('return continues')==4097
    old_cleanup(l)


def test_request_bound_prevents_unlimited_history():
    l=fixture();arm(l);l.run('for i=1,17 do source();queued() end')
    assert 'Request bound' in capture(l)['error'] and capture(l)['request_count']==16
    old_cleanup(l)


@pytest.mark.parametrize('corruption,expected',[
    ('put(F+0x17E8+8,0x60000000+9*0x28,8);put(F+0x17E8+16,0x60000000+9*0x28,8)','Wave count'),
    ('put(0x60000008,0x61000000+97*0x14,8);put(0x60000010,0x61000000+97*0x14,8)','Descriptor count'),
    ('put(0x60000008,0x61000001,8)','Descriptor vector'),
    ('mem[0x61000000]=nil','Unreadable'),
    ('put(B+0x45B8409,1,1)','request/output identity'),
    ('put(F+0x17E8+0x30,4,1)','metadata projection'),
])
def test_falsifying_or_unreadable_generated_output_fails_closed(corruption,expected):
    l=fixture();arm(l);l.run('source();queued();consume();'+corruption+';generated()')
    assert expected in capture(l)['error'];old_cleanup(l)


def test_unrelated_wrapper_not_mislabelled_mission_entry():
    l=fixture();arm(l);l.run('put(F+0x1778,B+0x88888,8);consume();generated()')
    assert capture(l)['event_sequence']==0
    old_cleanup(l)


@pytest.mark.parametrize('mutation',[
    'wrong_birth','wrong_sites','wrong_source','first_stop','no_cleanup','event_order',
    'reused_iid','wrong_request_id','wrong_return_rsp','wrong_output_ptr','wrong_desc_count',
    'wrong_class','wrong_desc_raw','fake_actor','fake_frame_extra','wrong_counter','early_window',
    'clock_backwards','pending_ids','queue_frame',
])
def test_offline_validator_does_not_certify_forged_link_or_empty_assertions(mutation):
    l=fixture();run_first(l);p,c=close_window(l);data=envelope(p)
    if mutation=='wrong_birth':c['cleanup_metadata']['target']['creation_filetime']='other'
    elif mutation=='wrong_sites':data['capture_metadata']['arm_verification']['result']['owned_breakpoints'][3]='0x1'
    elif mutation=='wrong_source':data['capture_metadata']['source']['phase_file']='mode_upstream_ce.lua'
    elif mutation=='first_stop':p['stop_reason']='upstream_request_bound'
    elif mutation=='no_cleanup':c['bridge_result']['result']['probe']['cleanup_pending']=True
    elif mutation=='event_order':p['events'][1]['sequence']=9
    elif mutation=='reused_iid':p['events'][2]['invocation_id']=2
    elif mutation=='wrong_request_id':p['events'][2]['request_id']=2
    elif mutation=='wrong_return_rsp':p['events'][3]['return_rsp']='0x1'
    elif mutation=='wrong_output_ptr':p['events'][3]['output_address']='0x1'
    elif mutation=='wrong_desc_count':p['events'][3]['output']['descriptor_count']=10
    elif mutation=='wrong_class':p['events'][3]['output']['waves'][0]['descriptors'][0]['selector_class']=1
    elif mutation=='wrong_desc_raw':p['events'][3]['output']['waves'][0]['descriptors'][0]['raw_hex']='00'*20
    elif mutation=='fake_actor':p['events'][3]['output']['physical_actor_join']=True
    elif mutation=='fake_frame_extra':p['events'][3]['extra_evidence']='captured F+1648'
    elif mutation=='wrong_counter':p['completed_invocations']=2
    elif mutation=='early_window':p['stopped_elapsed_ms']=1000
    elif mutation=='clock_backwards':p['events'][2]['elapsed_ms']=1;p['events'][3]['elapsed_ms']=0
    elif mutation=='pending_ids':p['events'][2]['matching_request_ids']=[]
    elif mutation=='queue_frame':p['events'][1]['queue_rsp']='0x1'
    with pytest.raises((ValueError,KeyError)):validate(data,c)


def test_legacy_thread_token_is_not_certified_as_native_thread():
    l=fixture();run_first(l);p,c=close_window(l)
    assert not validate(envelope(p),c)['target_thread_id_from_debug_api']


def test_phase_registered_and_identity_gated():
    assert PHASES['mode-upstream-sequence']=='mode_upstream_sequence_ce.lua'
    assert 'mode-upstream-sequence' in STRICT_V201_PHASES


def test_no_target_mutation_or_remote_game_call_api():
    source=(CAP/'mode_upstream_sequence_ce.lua').read_text()
    for api in ['writeBytes(', 'writeInteger(', 'writeQword(', 'autoAssemble(', 'executeCodeEx(', 'createRemoteThread(']:
        assert api not in source


def test_resume_false_is_failure_not_successful_continuation():
    l=fixture();arm(l)
    l.run('function debug_continueFromBreakpoint() return false end;source()')
    assert capture(l)['stop_reason']=='resume_error'
    assert 'Resume unconfirmed' in capture(l)['error']
    old_cleanup(l)


def test_clearing_stopped_capture_resets_all_semantic_counters():
    l=fixture();run_first(l);close_window(l)
    l.run('nioh3PossessedCapture.clear_captures()')
    p=capture(l)
    assert p['conclusion']=='cleared' and p['event_sequence']==0
    assert [p[k] for k in ('request_count','invocation_count','completed_invocations','unbound_invocations')]==[0,0,0,0]


@pytest.mark.parametrize('field,value',[('max_events',129),('max_seconds',121),('read_bytes',4194305),('total_hits',4097)])
def test_validator_rejects_relaxed_or_exceeded_budget(field,value):
    l=fixture();run_first(l);p,c=close_window(l)
    p[field]=value
    with pytest.raises(ValueError): validate(envelope(p),c)


def test_session_forwarded_metadata_not_exported_or_named_mission_key():
    l=fixture();l.run('mem[0x32000030]=nil');run_first(l)
    p,c=close_window(l)
    assert validate(envelope(p),c)['observation_validated']
    for e in p['events']:
        assert e['current_session']['available'] is True
        assert e['current_session']['forwarded_metadata30_not_captured'] is True
        assert 'mission_key' not in e['current_session']


def test_cli_rejects_first_stop_before_starting_bridge(tmp_path):
    import subprocess
    result=subprocess.run([sys.executable,str(CAP/'run_possessed_enemy_observer.py'),
        '--phase','mode-upstream-sequence','--pid','777','--seed','86872488',
        '--run-id','reject-before-bridge','--mode-label','one-person-expedition',
        '--output',str(tmp_path/'capture.json'),'--stop-on-first-final'],
        text=True,capture_output=True,timeout=10)
    assert result.returncode==2
    assert 'must not stop on the first generation' in result.stderr
    assert not (tmp_path/'capture.json').exists()


def test_cli_requires_runner_margin_after_sequence_window(tmp_path):
    import subprocess
    assert SEQUENCE_OBSERVATION_SECONDS == 120.0
    result=subprocess.run([sys.executable,str(CAP/'run_possessed_enemy_observer.py'),
        '--phase','mode-upstream-sequence','--pid','777','--seed','86872488',
        '--run-id','reject-short-runner-window','--mode-label','one-person-expedition',
        '--output',str(tmp_path/'capture.json'),'--timeout-seconds','120'],
        text=True,capture_output=True,timeout=10)
    assert result.returncode==2
    assert 'runner timeout must exceed its 120-second observation window' in result.stderr
    assert not (tmp_path/'capture.json').exists()


def test_runner_has_host_monotonic_sequence_stop_fallback():
    source=(CAP/'run_possessed_enemy_observer.py').read_text()
    assert "nioh3PossessedCapture.stop('observation_window_elapsed')" in source
    assert 'time.monotonic() >= sequence_deadline' in source
