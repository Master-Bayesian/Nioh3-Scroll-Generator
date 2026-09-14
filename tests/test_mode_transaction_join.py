"""Actual Lua 5.4 executed with synthetic CE/memory; not live-game acceptance."""
from pathlib import Path
import copy,hashlib,json,struct,sys
import pytest
ROOT=Path(__file__).resolve().parents[1];CAP=ROOT/'research/possessed_enemy_capture'
LIVE_FIXTURE=ROOT/'tests/fixtures/mode_transaction_join'
sys.path[:0]=[str(CAP),str(Path(__file__).parent)]
from lua54_test_runtime import Lua54
from run_possessed_enemy_observer import lua_literal,PHASES,STRICT_V201_PHASES,WINDOWED_PHASES
from validate_mode_transaction_join import validate,SITES,SCHEMA
LOC=json.loads((CAP/'mode_transaction_join_v201_locators.json').read_text())
IDENT={'process_id':777,'creation_filetime':'12345678','image_size':77814240,
'executable_sha256':'4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'}
MOCK=r'''
B=0x140000000;C=0x50000000;S=0x40000000;G=0x70000000;P=G+0x70
mem={};bp={};timers={};attachedPid=777;attachedHandle=999;debugging=true;thread=7;iface=2
armCount=0;armFail=0;removeCalls={};removeFail=false;timerFail=false;continues=0;tick=1000;attachedWith=0
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
function debug_getCurrentDebuggerInterface() return iface end
function debugProcess(i) assert(i==2);debugging=true;iface=i;attachedWith=i end
function debug_setBreakpoint(a,n,t,m,f) armCount=armCount+1;if armFail==armCount then return false end;assert(n==1 and t==0 and m==1);bp[a]=f;return true,a end
function debug_removeBreakpoint(a) removeCalls[#removeCalls+1]=a;if not removeFail then bp[a]=nil end;return not removeFail end
function debug_removeBreakpointByID(a) return debug_removeBreakpoint(a) end
function debug_continueFromBreakpoint(m) assert(m==0);continues=continues+1;return true end
function createTimer(ms,f) if timerFail then error('timer unavailable') end;timers[#timers+1]={ms=ms,f=f};return true end
function drain() local n=0;while true do local ix=nil;for i,t in ipairs(timers) do if t.ms<=100 then ix=i;break end end;if not ix then break end;local t=table.remove(timers,ix);t.f();n=n+1;assert(n<30) end end
function hit(rva) RIP=B+rva;return assert(bp[RIP],'unarmed')() end
function queued() RSP=S-0x658;R15=C-0x50;R12=B+0x45B83E0;RAX=0;return hit(0x10D9368) end
function consume() put(G,B+0x2237978,8);RSP=G;RCX=B+0x45B8400;RDX=P;return hit(0x20E198C) end
function generated() RSP=G+8;return hit(0x2237978) end
function materialize() put(G,B+0x2237994,8);RSP=G;RCX=0x32000000;RDX=P;R8=0xCC96;return hit(0x1C244E4) end
bptExecute=0;bpmDebugRegister=1;co_run=0
'''


def output(l,n=1,control=1):
    h=bytearray(0x34);struct.pack_into('<3Q',h,0,0x60000000,0x60000028,0x60000028)
    h[0x1E]=0x8E;h[0x1F]=0xD4;h[0x24]=control;h[0x30]=3;struct.pack_into('<I',h,0x28,183)
    w=bytearray(0x28);struct.pack_into('<3Q',w,0,0x61000000,0x61000000+n*20,0x61000000+n*20)
    l.run(f"putHex(P,'{h.hex()}');putHex(0x60000000,'{w.hex()}')")
    for i in range(n):
        d=bytearray(20);struct.pack_into('<II',d,0,0xF3C+i,0xDCB98);d[14]=i+1;d[16]=i%2
        l.run(f"putHex({0x61000000+i*20},'{d.hex()}')")


def fixture(extra=1,ret=0x21D9E24,tail='0000'):
    l=Lua54();l.run(MOCK)
    q='A8912D05B700010301'+f'{extra:02X}'+tail
    lines=[f'NIOH3_POSSESSED_TARGET_IDENTITY={lua_literal(IDENT)}',
           'NIOH3_ASSIGNMENT_SEED=86872488;NIOH3_POSSESSED_RUN_ID="synthetic-join"',
           'NIOH3_BREAKPOINT_LIFECYCLE_PATH='+lua_literal(str(ROOT/'research/owned_breakpoint_lifecycle_ce.lua')),
           'fill(B+0x45B83E0,4*0x58);put(B+0x45B83E4,0xCC96,4)',
           f"putHex(C-0x50,'{q}');putHex(B+0x45B8400,'{q}');put(S,B+{ret},8);put(S-8,C,8)",
           'put(B+0x474D480,0,8);put(B+0x45C44A0,0x20000000,8);fill(0x20001010,0x270)',
           'put(0x20001014,3,1);put(0x20001018,0xCC96,4);put(0x20001278,86872488,4);put(0x2000127C,183,2);put(0x2000127E,1,1);put(0x2000127F,1,1)']
    for s in list(LOC['sites'].values())+list(LOC['extra_signature_checks'].values()):lines.append(f"putHex(B+{int(s['rva'],16)},'{s['bytes']}')")
    disp=0x10D9180-ret
    lines.append(f"putHex(B+{ret-5},'{(bytes([0xE8])+struct.pack('<i',disp)).hex()}')")
    l.run(';'.join(lines));output(l);return l


def arm(l):return l.run('return assert(loadfile('+lua_literal(str(CAP/'mode_transaction_join_ce.lua'))+'))()')
def payload(l):return l.run('return nioh3PossessedCapture')
def close(l):
    l.run("tick=121641;nioh3PossessedCapture.stop('observation_window_elapsed');drain()")
    p=payload(l);a=[hex(0x140000000+r) for r in SITES.values()]
    data={'capture_metadata':{'phase':'mode-transaction-join','run_id':p['run_id'],'requested_pid':777,'target_seed':86872488,
          'target':IDENT,'read_only':True,'writes_game_memory':False,'stop_on_first_final':False,
          'source':{'phase_file':'mode_transaction_join_ce.lua',
          'phase_sha256':hashlib.sha256((CAP/'mode_transaction_join_ce.lua').read_bytes()).hexdigest(),
          'lifecycle_sha256':hashlib.sha256((ROOT/'research/owned_breakpoint_lifecycle_ce.lua').read_bytes()).hexdigest()},
          'fresh_phase_initialization':{'initialized':True,'debugger_broken':False,'breakpoints':[]},
          'arm_verification':{'active':True,'debugger_broken':False,'schema':SCHEMA,'run_id':p['run_id'],'owned_breakpoints':a,'breakpoints':a}},'bridge_result':p}
    cleanup={'cleanup_metadata':{'verified':not p['cleanup_pending'],'target':IDENT,'run_id':p['run_id']},
             'bridge_result':{'probe':{'active':p['active'],'cleanup_pending':p['cleanup_pending'],'owned_breakpoints':p['owned_breakpoints'],
                             'debugger_broken':False,'run_id':p['run_id'],'schema':SCHEMA},'breakpoints':l.run('return debug_getBreakpointList()')}}
    return data,cleanup

def full(l):l.run('queued();consume();generated();materialize()')


def test_native_session_projection_and_complete_input_join():
    l=fixture();arm(l);full(l);assert payload(l)['active'];a,b=close(l);o=validate(a,b)
    assert o['complete_consistent_pipeline_count']==1 and o['materializations'][0]['source_projection']=='session_view_fields_observed'
    assert not o['native_ui_mode_to_producer_decision_recovered'] and not o['persistent_task_join']
    assert len(a['bridge_result']['events'])==4 and l.run('return armCount')==4
    assert l.run('return attachedWith')==0 # existing VEH retained


def test_owned_zero_branch_is_not_relabeled_normal_solo():
    l=fixture(0,0xF1E4F1);arm(l);full(l);o=validate(*close(l))
    assert o['materializations'][0]['producer_route']=='owned_scroll_branch' and o['materializations'][0]['q9']==0
    assert o['complete_consistent_pipeline_count']==1

@pytest.mark.parametrize('tail',['0000','0200','FFFF'])
def test_tail_is_raw_not_player_count(tail):
    l=fixture(tail=tail);arm(l);full(l);o=validate(*close(l));assert o['complete_consistent_pipeline_count']==1


def test_materializer_joins_same_invocation_not_earlier_reused_stack():
    l=fixture();arm(l);full(l);output(l,2);full(l);o=validate(*close(l))
    assert [v['invocation_id'] for v in o['materializations']]==[1,2]
    assert [v['materializer_input_count'] for v in o['materializations']]==[1,2]


def test_late_materializer_does_not_join_superseded_stack_frame():
    l=fixture();arm(l);l.run('queued();consume();generated();queued();consume();materialize()')
    o=validate(*close(l));assert o['unbound_materializer_events']==[6] and o['unfinished_invocations']==[1,2]


def test_same_payload_two_pending_never_guesses_fifo():
    l=fixture();arm(l);l.run('queued();queued();consume();generated();materialize()')
    o=validate(*close(l));assert o['materializations'][0]['join_grade']=='ambiguous_pending_payload'
    assert not o['complete_consistent_pipeline_count']


def test_unobserved_queue_writer_not_lost():
    l=fixture();arm(l);l.run('consume();generated();materialize()')
    o=validate(*close(l));assert o['materializations'][0]['producer_status']=='unbound'
    assert not o['complete_consistent_pipeline_count']


def test_unobserved_generator_materializer_retained():
    l=fixture();arm(l);l.run('materialize()');o=validate(*close(l))
    assert o['unbound_materializer_events']==[1] and not o['complete_consistent_pipeline_count']


def test_writer_contradiction_preserved_then_linked_not_success():
    l=fixture(extra=1,ret=0xF1E4F1);arm(l);full(l);o=validate(*close(l))
    assert o['writer_counterexamples']==[1] and o['materializations'][0]['producer_status']=='contradicted'
    assert not o['complete_consistent_pipeline_count']


def test_unknown_native_caller_not_magic_mode_enum():
    l=fixture(ret=0x234567);arm(l);full(l);o=validate(*close(l));assert o['materializations'][0]['producer_status']=='unbound'


def test_source_projection_mismatch_retains_raw_input():
    l=fixture();l.run('put(0x20001278,123,4)');arm(l);full(l);a,b=close(l);o=validate(a,b)
    assert o['writer_counterexamples']==[1] and a['bridge_result']['events'][0]['session_view']['scroll_seed']==123


def test_expansion_between_return_and_materializer_is_counterexample():
    l=fixture();arm(l);l.run('queued();consume();generated()');output(l,2);l.run('materialize()')
    o=validate(*close(l));x=o['materializations'][0]
    assert (x['generated_count'],x['materializer_input_count'])==(1,2) and not x['descriptor_buffers_equal']
    assert not x['pipeline_consistent']


def test_control24_change_recorded_without_faking_descriptor_change():
    l=fixture();arm(l);l.run('queued();consume();generated();put(P+0x24,0,1);materialize()')
    o=validate(*close(l));assert o['materializations'][0]['header_changed_offsets']==[0x24]
    assert o['materializations'][0]['pipeline_consistent']


def test_queue_change_after_generator_not_silently_bound():
    l=fixture();arm(l);l.run('queued();consume();generated();put(B+0x45B8409,0,1);materialize()')
    o=validate(*close(l));assert not o['materializations'][0]['pipeline_consistent']


def test_unverified_callback_thread_token_is_not_native_identity_gate():
    l=fixture();arm(l);l.run('queued();consume();thread=88;generated();materialize()')
    o=validate(*close(l));assert o['complete_consistent_pipeline_count']==1


def test_no_thread_extension_still_has_scoped_stack_join():
    l=fixture();l.run('getCurrentThreadId=nil');arm(l);full(l);a,b=close(l)
    assert a['bridge_result']['events'][0]['thread_id_source']=='unavailable'
    assert validate(a,b)['complete_consistent_pipeline_count']==1

@pytest.mark.parametrize('rva',[0x10D9180,*SITES.values()])
def test_all_signatures_before_arm(rva):
    l=fixture();l.run(f'put(B+{rva},0,1)')
    with pytest.raises(RuntimeError,match='signature|prologue'):arm(l)
    assert l.run('return armCount')==0


def test_explicit_veh_only():
    l=fixture();l.run('debugging=false;iface=nil');arm(l)
    assert l.run('return attachedWith')==2;close(l)


def test_existing_windows_debugger_rejected_not_switched():
    l=fixture();l.run('iface=1')
    with pytest.raises(RuntimeError,match='VEH'):arm(l)
    assert l.run('return armCount')==0 and l.run('return onOpenProcess') is None

@pytest.mark.parametrize('index',[1,2,3,4])
def test_partial_arming_cleanup_is_owned(index):
    l=fixture();l.run(f'armFail={index}')
    with pytest.raises(RuntimeError):arm(l)
    l.run('drain()');assert l.run('return debug_getBreakpointList()')=={}


def test_foreign_breakpoint_never_removed():
    l=fixture();l.run('bp[123]=true')
    with pytest.raises(RuntimeError,match='Foreign'):arm(l)
    assert l.run('return #removeCalls')==0


def test_process_switch_never_removes_or_resumes_other_target():
    l=fixture();arm(l);l.run('attachedHandle=1000;queued();drain()')
    assert payload(l)['error'] and payload(l)['cleanup_pending']
    assert l.run('return #removeCalls')==0 and l.run('return continues')==0


def test_cleanup_failure_keeps_ownership_for_retry():
    l=fixture();arm(l);full(l);l.run('removeFail=true');a,b=close(l)
    assert a['bridge_result']['cleanup_pending']
    with pytest.raises(ValueError,match='cleanup'):validate(a,b)
    l.run('removeFail=false;nioh3PossessedCapture.retry_cleanup();drain()')
    assert not payload(l)['cleanup_pending']


def test_empty_no_trigger_window_no_mechanism_claim():
    l=fixture();arm(l)
    with pytest.raises(ValueError,match='empty'):validate(*close(l))


def test_session_projection_status_cannot_hide_a_counterexample():
    l=fixture();arm(l);full(l);a,b=close(l);a['bridge_result']['events'][0]['session_view']['metadata']=100
    with pytest.raises(ValueError,match='contradiction'):validate(a,b)


def test_output_comparison_labels_cannot_override_real_bytes():
    l=fixture();arm(l);l.run('queued();consume();generated()');output(l,2);l.run('materialize()');a,b=close(l)
    a['bridge_result']['events'][-1]['descriptor_buffers_equal']=True
    with pytest.raises(ValueError,match='comparison'):validate(a,b)


def test_host_registers_four_site_windowed_phase():
    assert PHASES['mode-transaction-join']=='mode_transaction_join_ce.lua'
    assert 'mode-transaction-join' in STRICT_V201_PHASES and 'mode-transaction-join' in WINDOWED_PHASES
    assert len(SITES)==4 and 0x10D9180 not in SITES.values()


def test_direct_call_target_must_be_real_enqueue_not_just_a_return_rva():
    l=fixture();l.run('put(B+0x21D9E20,0,4)');arm(l);full(l);o=validate(*close(l))
    assert o['writer_counterexamples']==[1] and not o['complete_consistent_pipeline_count']


def test_parameterized_literal_one_is_bounded_not_source_mode_proof():
    l=fixture(ret=0x21DC438);l.run("C=S+0xB8;putHex(C-0x50,'A8912D05B700010301010000')")
    arm(l);full(l);o=validate(*close(l))
    assert o['complete_consistent_pipeline_count']==1
    assert o['materializations'][0]['source_projection']=='literal_one_route_only'
    assert not o['native_ui_mode_to_producer_decision_recovered']


def test_requeue_can_inherit_one_without_being_a_literal_one_writer():
    l=fixture(ret=0x2233DE8)
    l.run("put(S-8,C-0x27,8);put(B+0x474D480,0x33000000,8);put(0x33000008,0x34000000,8);put(0x34000024,2,4);put(0x34000028,0xCC96,4);putHex(0x34000038,'A8912D05B700010301010000')")
    arm(l);full(l);o=validate(*close(l))
    assert o['complete_consistent_pipeline_count']==1
    assert o['materializations'][0]['producer_route']=='current_session_requeue'
    assert o['materializations'][0]['source_projection']=='current_session_copy_observed'


def test_missing_session_view_does_not_invent_zero_input():
    l=fixture();l.run('put(B+0x45C44A0,0,8)');arm(l);full(l);o=validate(*close(l))
    assert o['materializations'][0]['source_projection']=='session_view_unavailable'
    assert not o['native_ui_mode_to_producer_decision_recovered']


def test_tampered_source_projection_grade_is_rejected():
    l=fixture(ret=0x21DC438);l.run("C=S+0xB8;putHex(C-0x50,'A8912D05B700010301010000')")
    arm(l);full(l);a,b=close(l);a['bridge_result']['events'][0]['source_projection']='session_view_fields_observed'
    with pytest.raises(ValueError,match='projection grade'):validate(a,b)


def test_all_event_and_read_vectors_bounded_and_no_target_mutation_api():
    src=(CAP/'mode_transaction_join_ce.lua').read_text()
    assert 'MAX_READ=128,16,16,4096,4*1024*1024' in src
    for token in ['writeBytes(', 'writeInteger(', 'executeCodeEx(', 'autoAssemble(', 'debug_setContext(']:
        assert token not in src


def test_live_parameterized_session_transaction_is_reproducible_from_raw_capture():
    data=json.loads((LIVE_FIXTURE/'live_parameterized_86872488.json').read_text(encoding='utf-8'))
    cleanup=json.loads((LIVE_FIXTURE/'live_parameterized_86872488.cleanup.json').read_text(encoding='utf-8'))
    result=validate(data,cleanup)
    assert result['observation_validated']
    assert result['complete_consistent_pipeline_count']==1
    assert not result['writer_counterexamples']
    assert not result['unfinished_invocations']
    assert not result['unbound_materializer_events']
    materialization=result['materializations'][0]
    assert materialization=={
        'invocation_id':1,
        'request_id':1,
        'producer_route':'parameterized_session_branch',
        'producer_status':'consistent',
        'source_projection':'literal_one_route_only',
        'native_source_projection_bound':False,
        'q9':1,
        'generated_count':10,
        'materializer_input_count':10,
        'descriptor_buffers_equal':True,
        'header_changed_offsets':[],
        'pipeline_consistent':True,
        'join_grade':'unique_observed_queued_payload',
        'native_mode_enum_decoded':False,
    }
    events=data['bridge_result']['events']
    assert [event['site'] for event in events]==[
        'request_queued','mission_consume','mission_generated','materialize_enter'
    ]
    assert events[0]['return_rva']=='0x21DC438'
    assert events[0]['request_hex']=='A8912D05B700010301010200'
    assert events[0]['producer_call']['target_rva']=='0x10D9180'
    descriptors=[item for wave in events[2]['output']['waves'] for item in wave['descriptors']]
    assert (len(descriptors),events[2]['output']['class0_count'],events[2]['output']['class1_count'])==(10,6,4)
    assert [item['spawn'] for item in descriptors if item['source_flag']==1]==[0xF3F]
    assert cleanup['cleanup_metadata']['verified'] is True
