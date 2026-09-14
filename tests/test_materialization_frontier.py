"""Real Lua 5.4 + synthetic CE memory; NOT live/native game acceptance."""
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
from lua54_test_runtime import Lua54
from run_possessed_enemy_observer import lua_literal, PHASES, STRICT_V201_PHASES, WINDOWED_PHASES
from validate_materialization_frontier import validate, SITES, SCHEMA
LOC=json.loads((CAP/'materialization_frontier_v201_locators.json').read_text())
IDENT={'process_id':777,'creation_filetime':'12345678','image_size':77814240,
 'executable_sha256':'4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'}
MOCK=r'''
B=0x140000000;E=0x40000000;W=0x30000000;ROOTPTR=0x31000000;M=0x32000000;P=E+0x70
mem={};bp={};timers={};attachedPid=777;attachedHandle=999;debugging=true;thread=7
armCount=0;armFail=0;removeCalls={};removeFail=false;timerFail=false;continues=0;tick=1000
function fill(a,n,v) for i=0,n-1 do mem[a+i]=v or 0 end end
function put(a,v,n) for i=0,n-1 do mem[a+i]=v&255;v=v>>8 end end
function putHex(a,h) for i=1,#h,2 do mem[a+(i-1)/2]=tonumber(h:sub(i,i+1),16) end end
function readBytes(a,n,_) local t={} for i=0,n-1 do if mem[a+i]==nil then return nil end;t[#t+1]=mem[a+i] end;return t end
function getAddressSafe(_) return B end
function getOpenedProcessID() return attachedPid end
function getOpenedProcessHandle() return attachedHandle end
function getCurrentThreadId() return thread end
function getTickCount() return tick end
function debug_getBreakpointList() local t={} for a in pairs(bp) do t[#t+1]=a end;return t end
function debug_isDebugging() return debugging end
function debugProcess(_) debugging=true end
function debug_setBreakpoint(a,n,t,m,f) armCount=armCount+1;if armFail==armCount then return false end;assert(n==1 and t==0 and m==1);bp[a]=f;return true,a end
function debug_removeBreakpoint(a) removeCalls[#removeCalls+1]=a;if not removeFail then bp[a]=nil end;return not removeFail end
function debug_removeBreakpointByID(a) return debug_removeBreakpoint(a) end
function debug_continueFromBreakpoint(m) assert(m==0);continues=continues+1;return true end
function createTimer(ms,f) if timerFail then error('timer unavailable') end;timers[#timers+1]={ms=ms,f=f};return true end
function drain() local n=0;while true do local ix=nil;for i,t in ipairs(timers) do if t.ms<=100 then ix=i;break end end;if not ix then break end;local t=table.remove(timers,ix);t.f();n=n+1;assert(n<30) end end
function hit(rva) RIP=B+rva;return assert(bp[RIP],'unarmed')() end
function enter() RSP=E;RCX=W;RDX=P;R8=0xCC96;return hit(0x1C244E4) end
function pre() RSP=E-0x258;RBP=E-0x158;R15=P;RDI=W;return hit(0x1C245D8) end
function lookup(i,existing) RDI=0x61000000+i*0x14;RBX=M;RAX=existing or 0;return hit(0x1C24614) end
function link(i,task) RDI=0x61000000+i*0x14;RBX=M;R14=task or 0x62000000+i*0x200;return hit(0x1C24662) end
bptExecute=0;bpmDebugRegister=1;co_run=0
'''


def fixture(n=1):
    l=Lua54();l.run(MOCK)
    lines=[f'NIOH3_POSSESSED_TARGET_IDENTITY={lua_literal(IDENT)}',
        'NIOH3_ASSIGNMENT_SEED=86872488;NIOH3_POSSESSED_RUN_ID="synthetic-frontier"',
        'NIOH3_BREAKPOINT_LIFECYCLE_PATH='+lua_literal(str(ROOT/'research/owned_breakpoint_lifecycle_ce.lua')),
        'put(E,B+0x2237994,8);put(W,ROOTPTR,8);put(ROOTPTR+0x48,M,8);put(ROOTPTR+0x2BF4,0,1);put(ROOTPTR+0x2C18,1,1)',
        "putHex(B+0x45B8400,'A8912D05B700010301000000')"]
    for s in LOC['sites'].values():lines.append(f"putHex(B+{int(s['rva'],16)},'{s['bytes']}')")
    l.run(';'.join(lines));prepare(l,n);return l


def prepare(l,n,control24=0):
    header=bytearray(0x34);struct.pack_into('<3Q',header,0,0x60000000,0x60000028,0x60000028)
    header[0x1E]=0x8E;header[0x1F]=0xD4;header[0x24]=control24;header[0x30]=3
    struct.pack_into('<I',header,0x28,183)
    wave=bytearray(0x28);struct.pack_into('<3Q',wave,0,0x61000000,0x61000000+n*20,0x61000000+n*20)
    l.run(f"putHex(P,'{header.hex()}');putHex(0x60000000,'{wave.hex()}')")
    for i in range(n):
        d=bytearray(20);struct.pack_into('<II',d,0,0xF3C+i,0xDCB98);d[0xE]=i+1;d[0x10]=i%2
        t=bytearray(0x158);struct.pack_into('<III',t,0x20,0xF3C+i,0xCC96,0xDCB98);t[0x80:0x94]=d;t[0x94]=0xD4
        l.run(f"putHex({0x61000000+i*20},'{d.hex()}');putHex({0x62000000+i*0x200},'{t.hex()}')")


def arm(l):return l.run('return assert(loadfile('+lua_literal(str(CAP/'materialization_frontier_ce.lua'))+'))()')
def capture(l):return l.run('return nioh3PossessedCapture')
def cleanup(l):
    l.run('drain()');p=capture(l)
    return {'cleanup_metadata':{'verified':not p['cleanup_pending'],'target':p['identity'],'run_id':p['run_id']},
        'bridge_result':{'probe':{'active':p['active'],'cleanup_pending':p['cleanup_pending'],
          'owned_breakpoints':p['owned_breakpoints'],'debugger_broken':False,'run_id':p['run_id'],'schema':p['schema']},
          'breakpoints':l.run('return debug_getBreakpointList()')}}
def envelope(p):
    a=[hex(0x140000000+r) for r in SITES.values()]
    return {'capture_metadata':{'phase':'materialization-frontier','run_id':p['run_id'],'requested_pid':p['pid'],
        'target_seed':p['target_seed'],'target':p['identity'],'read_only':True,'writes_game_memory':False,
        'stop_on_first_final':False,'source':{'phase_file':'materialization_frontier_ce.lua',
        'phase_sha256':hashlib.sha256((CAP/'materialization_frontier_ce.lua').read_bytes()).hexdigest(),
        'lifecycle_sha256':hashlib.sha256((ROOT/'research/owned_breakpoint_lifecycle_ce.lua').read_bytes()).hexdigest()},
        'fresh_phase_initialization':{'initialized':True,'debugger_broken':False,'breakpoints':[]},
        'arm_verification':{'active':True,'run_id':p['run_id'],'schema':SCHEMA,'debugger_broken':False,'owned_breakpoints':a,'breakpoints':a}},
        'bridge_result':p}
def close(l):
    l.run("tick=121641;nioh3PossessedCapture.stop('observation_window_elapsed')")
    c=cleanup(l);return envelope(capture(l)),c

def complete(l,n=1,reuse=False):
    l.run('enter();pre()')
    for i in range(n):l.run(f'lookup({i},{0x62000000+i*0x200 if reuse else 0});link({i})')


def test_known_single_entry_has_no_fake_global_or_actor_finality():
    l=fixture();arm(l);complete(l)
    assert capture(l)['active'];data,c=close(l);o=validate(data,c)
    assert o['invocations'][0]['all_post_descriptors_linked']
    assert o['invocations'][0]['control24_before']==0 # Q[8] is one: no forged equality
    assert not o['whole_manager_state_known'] and not o['physical_actor_join']
    assert not o['product_oracle_accepted'] and not o['unobserved_generator_excluded']
    assert l.run('return armCount')==4 and l.run('return continues')==4


def test_prepass_can_expand_six_to_ten_without_fabricating_a_new_seed_request():
    l=fixture(6);arm(l);l.run('enter()');prepare(l,10);l.run('pre()')
    for i in range(10):l.run(f'lookup({i});link({i})')
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert (o['before_descriptor_count'],o['after_descriptor_count'])==(6,10)
    assert o['helper_span_result']=='descriptor_expansion_in_pre_iteration_span'
    assert o['queue_context_is_not_seed_proof']


def test_reused_old_descriptor_mismatch_is_evidence_not_silently_overwritten():
    l=fixture();arm(l);l.run('enter();pre();put(0x62000090,1,1);put(0x6200008F,1,1);lookup(0,0x62000000);link(0)')
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert o['reuse_mismatches']==[1] and o['new_path_copy_mismatches']==[]
    assert l.run('return mem[0x62000090]')==1


def test_new_path_descriptor_change_is_explicit_model_falsification():
    l=fixture();arm(l);l.run('enter();pre();lookup(0);put(0x62000090,1,1);link(0)')
    data,c=close(l);assert validate(data,c)['invocations'][0]['new_path_copy_mismatches']==[1]


def test_second_materializer_call_same_stack_and_payload_keeps_distinct_ids():
    l=fixture();arm(l);complete(l);prepare(l,2);complete(l,2)
    data,c=close(l);o=validate(data,c)
    assert [x['after_descriptor_count'] for x in o['invocations']]==[1,2]
    assert data['bridge_result']['events'][4]['previous_frame_complete'] is True


def test_alternate_caller_recorded_without_misattributing_queue_snapshot_seed():
    l=fixture();l.run('put(E,B+0x2236501,8);put(B+0x45B8400,7,4)');arm(l);complete(l)
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert o['caller_grade']=='unbound_caller' and o['queue_context_seed']==7
    assert o['queue_context_is_not_seed_proof']


def test_missing_link_is_incomplete_not_success():
    l=fixture(2);arm(l);l.run('enter();pre();lookup(0);lookup(1);link(1)')
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert o['unlinked_lookup_ordinals']==[1] and not o['all_post_descriptors_linked']


def test_gate_before_prepass_is_not_empty_success():
    l=fixture();arm(l);l.run('enter()');data,c=close(l)
    o=validate(data,c)['invocations'][0];assert o['after_descriptor_count'] is None and not o['all_post_descriptors_linked']


def test_empty_capture_rejected():
    l=fixture();arm(l);data,c=close(l)
    with pytest.raises(ValueError,match='empty'):validate(data,c)


@pytest.mark.parametrize('site',list(SITES))
def test_every_site_signature_before_arm(site):
    l=fixture();l.run(f'put(B+{SITES[site]},0,1)')
    with pytest.raises(RuntimeError,match='signature'):arm(l)
    assert l.run('return armCount')==0


@pytest.mark.parametrize('n',[1,2,3,4])
def test_partial_arm_cleans_only_owned(n):
    l=fixture();l.run(f'armFail={n}')
    with pytest.raises(RuntimeError):arm(l)
    c=cleanup(l);assert c['cleanup_metadata']['verified'];assert l.run('return debug_getBreakpointList()')=={}


def test_foreign_breakpoint_not_removed():
    l=fixture();l.run('bp[123]=true')
    with pytest.raises(RuntimeError,match='Foreign'):arm(l)
    assert l.run('return #removeCalls')==0


def test_same_pid_different_handle_stops_without_touching_new_process():
    l=fixture();arm(l);l.run('attachedHandle=1000;enter()');c=cleanup(l)
    assert not c['cleanup_metadata']['verified'] and l.run('return #removeCalls')==0


def test_cleanup_failure_retained_then_retry_succeeds():
    l=fixture();arm(l);complete(l);l.run('removeFail=true');data,c=close(l)
    with pytest.raises(ValueError,match='cleanup'):validate(data,c)
    l.run('removeFail=false;nioh3PossessedCapture.retry_cleanup()');c=cleanup(l)
    assert c['cleanup_metadata']['verified']


def test_timer_failure_does_not_leave_unowned_breakpoints():
    l=fixture();l.run('timerFail=true')
    with pytest.raises(RuntimeError):arm(l)
    assert not capture(l)['active'];assert l.run('return debug_getBreakpointList()')=={}


def test_old_timer_cannot_stop_new_owner():
    l=fixture();arm(l);complete(l);close(l);old=capture(l)
    # old closure is inactive; a fresh arm publishes a new probe.
    l.run('tick=200000');arm(l)
    l.run('for _,t in ipairs(timers) do if t.ms==120000 then t.f();break end end')
    assert capture(l)['active'];close(l)


@pytest.mark.parametrize('expr,match',[
    ('R15=P+8;hit(0x1C245D8)','Output pointer'),
    ('RBP=E;hit(0x1C245D8)','RBP/RSP'),
    ('RDI=W+8;hit(0x1C245D8)','Wrapper identity'),
])
def test_wrong_frame_or_object_fails(expr,match):
    l=fixture();arm(l);l.run('enter();RSP=E-0x258;RBP=E-0x158;R15=P;RDI=W;'+expr)
    assert match in capture(l)['error'];cleanup(l)


def test_reuse_pointer_change_fails():
    l=fixture();arm(l);l.run('enter();pre();lookup(0,0x62000000);link(0,0x62000100)')
    assert 'different pointer' in capture(l)['error'];cleanup(l)


def test_descriptor_mutation_after_prepass_retains_failed_event():
    l=fixture();arm(l);l.run('enter();pre();put(0x6100000F,1,1);lookup(0)')
    p=capture(l);assert 'Descriptor changed' in p['error'] and len(p['events'])==3;cleanup(l)


def test_unavailable_thread_api_is_not_guessed():
    l=fixture();l.run('getCurrentThreadId=nil');arm(l);complete(l);data,c=close(l)
    assert validate(data,c)['observation_validated']
    assert data['bridge_result']['events'][0]['thread_id_source']=='unavailable'


def test_host_window_closes_even_if_lua_window_timer_did_not_fire():
    l=fixture();arm(l);complete(l);data,c=close(l)
    assert validate(data,c)['window_ms']==120641
    assert 'materialization-frontier' in WINDOWED_PHASES & STRICT_V201_PHASES
    assert PHASES['materialization-frontier']=='materialization_frontier_ce.lua'


@pytest.mark.parametrize('field,value',[('descriptor_count',2),('class0_count',99),('control24',1)])
def test_validator_rejects_forged_interpretation(field,value):
    l=fixture();arm(l);complete(l);data,c=close(l)
    data['bridge_result']['events'][0]['output'][field]=value
    with pytest.raises(ValueError):validate(data,c)


def test_validator_cannot_accept_other_process_cleanup():
    l=fixture();arm(l);complete(l);data,c=close(l)
    c=copy.deepcopy(c);c['cleanup_metadata']['target']['creation_filetime']='999'
    with pytest.raises(ValueError,match='identity'):validate(data,c)


def test_event_limit_is_hard():
    l=fixture(96);arm(l);l.run('enter();pre()')
    # 2 entries + 192 visits, then next batch exhausts fixed event capacity.
    for i in range(96):l.run(f'lookup({i});link({i})')
    l.run('enter();pre()')
    for i in range(31):
        if not capture(l)['active']:break
        l.run(f'lookup({i});link({i})')
    assert not capture(l)['active'] and 'Event bound' in capture(l)['error'];cleanup(l)


def test_after_deadline_no_new_reads_or_events():
    l=fixture();arm(l);l.run('tick=122000;enter()')
    assert capture(l)['stop_reason']=='observation_window_elapsed' and capture(l)['event_sequence']==0;cleanup(l)


def test_different_collector_source_is_not_accepted_as_this_phase():
    l=fixture();arm(l);complete(l);data,c=close(l)
    data['capture_metadata']['source']['phase_sha256']='A'*64
    with pytest.raises(ValueError,match='source hash'):validate(data,c)


def test_source_order_not_inferred_from_count_only():
    l=fixture(2);arm(l);l.run('enter();pre();lookup(1)')
    assert 'pointer/order' in capture(l)['error'];cleanup(l)


def test_rejects_false_task_join_boolean():
    l=fixture();arm(l);l.run('enter();pre();put(0x62000090,1,1);lookup(0,0x62000000);link(0)')
    data,c=close(l);data['bridge_result']['events'][-1]['descriptor_matches_source']=True
    with pytest.raises(ValueError,match='compare'):validate(data,c)


def test_data_from_different_invocation_cannot_be_relabelled():
    l=fixture();arm(l);complete(l);complete(l);data,c=close(l)
    data['bridge_result']['events'][-1]['invocation_id']=1
    with pytest.raises(ValueError):validate(data,c)


def test_unknown_pointer_read_never_fills_zero_task():
    l=fixture();arm(l);l.run('enter();pre();lookup(0,0x70000000)')
    assert 'Unreadable' in capture(l)['error'] and capture(l)['linked_count']==0;cleanup(l)


def test_manual_stop_at_first_link_does_not_call_it_function_return():
    l=fixture();arm(l);complete(l);l.run("nioh3PossessedCapture.stop('entry_observation_complete')")
    c=cleanup(l);o=validate(envelope(capture(l)),c)
    assert o['window_ms']==0 and not o['invocations'][0]['function_return_observed']


def test_reused_same_spawn_and_descriptor_wrong_mission_is_mismatch():
    l=fixture();arm(l);l.run('enter();pre();put(0x62000024,0xDEAD,4);lookup(0,0x62000000);link(0)')
    data,c=close(l);o=validate(data,c)
    assert o['invocations'][0]['reuse_mismatches']==[1]
    assert not data['bridge_result']['events'][-1]['identity_matches_source']


def test_reused_same_descriptor_wrong_terrain_is_mismatch():
    l=fixture();arm(l);l.run('enter();pre();put(0x62000094,0x12,1);lookup(0,0x62000000);link(0)')
    data,c=close(l);o=validate(data,c)
    assert o['invocations'][0]['reuse_mismatches']==[1]
    assert not data['bridge_result']['events'][-1]['terrain_matches_source']


def test_same_flat_bytes_but_different_wave_partition_is_not_no_change():
    l=fixture(2);arm(l);l.run('enter()')
    # Keep both descriptors byte-identical; move the second to a new wave.
    w=struct.pack('<3Q',0x61000014,0x61000028,0x61000028)+bytes(16)
    l.run(f"putHex(0x60000028,'{w.hex()}');put(P+8,0x60000050,8);put(P+16,0x60000050,8);"
          'put(0x60000008,0x61000014,8);put(0x60000010,0x61000014,8);pre()')
    for i in range(2):l.run(f'lookup({i});link({i})')
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert o['before_descriptor_count']==o['after_descriptor_count']==2
    assert o['helper_span_result']=='descriptor_change_in_pre_iteration_span'


@pytest.mark.parametrize('extra',[
    ['--stop-on-first-final'],
    ['--timeout-seconds','120'],
    ['--timeout-seconds','119.9'],
])
def test_new_phase_rejects_truncated_host_window_before_bridge(tmp_path,monkeypatch,extra):
    import run_possessed_enemy_observer as runner
    def forbidden(*a,**k):raise AssertionError('must reject before bridge access')
    monkeypatch.setattr(runner,'new_bridge',forbidden)
    monkeypatch.setattr(sys,'argv',['runner','--pid','777','--phase','materialization-frontier',
        '--run-id','bound-window','--seed','86872488','--output',str(tmp_path/'never.json'),*extra])
    with pytest.raises(SystemExit) as exc:runner.main()
    assert exc.value.code==2 and not (tmp_path/'never.json').exists()



def test_other_materializer_mission_parameter_not_silently_filtered():
    l=fixture();l.run('put(E,B+0x2236501,8);put(0x62000024,0xDEAD,4)');arm(l)
    l.run('RSP=E;RCX=W;RDX=P;R8=0xDEAD;hit(0x1C244E4);pre();lookup(0);link(0)')
    data,c=close(l);o=validate(data,c)['invocations'][0]
    assert o['mission_parameter']==0xDEAD and o['caller_grade']=='unbound_caller'
    assert o['queue_context_is_not_seed_proof'] and not o['new_path_copy_mismatches']


def test_frontier_starts_the_user_mode_debugger_with_veh():
    source=(ROOT/'research/possessed_enemy_capture/materialization_frontier_ce.lua').read_text()
    assert "if not debug_isDebugging() then debugProcess(2) end" in source
