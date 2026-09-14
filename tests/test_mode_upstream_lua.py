"""Execute the collector in real Lua 5.4; all CE/process data are synthetic."""
from pathlib import Path
import sys,json,struct,copy
import pytest
ROOT=Path(__file__).resolve().parents[1]
CAP=ROOT/'research/possessed_enemy_capture'
sys.path[:0]=[str(CAP),str(Path(__file__).parent)]
from lua54_test_runtime import Lua54
from run_possessed_enemy_observer import lua_literal,PHASES,STRICT_V201_PHASES
from validate_mode_upstream_capture import validate
LOC=json.loads((CAP/'mode_upstream_v201_locators.json').read_text())
IDENT={'process_id':777,'creation_filetime':'12345678','image_size':77814240,
       'executable_sha256':'4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'}
MOCK=r'''
B=0x140000000;F=0x10000000;C=0x50000000;S=0x40000000
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
function source() RBP=C;RSP=S;RDX=C-0x50;return hit(0x10D9180) end
function queued() RSP=S-0x658;R15=C-0x50;R12=B+0x45B83E0;RAX=0;return hit(0x10D9368) end
function consume() RSP=F+0x1778;RCX=B+0x45B8400;RDX=0x60000000;return hit(0x20E198C) end
function context() RBP=F;RSP=F-0x100;R14=86872488;R15=0xD4;RAX=0x26000008;return hit(0x1029FEE) end
bptExecute=0;bpmDebugRegister=1;co_run=0
'''

def fixture(tail='0200', extra=1):
    l=Lua54();l.run(MOCK)
    req='A8912D05B700010301'+f'{extra:02X}'+tail
    lines=[f'NIOH3_POSSESSED_TARGET_IDENTITY={lua_literal(IDENT)}',
           'NIOH3_ASSIGNMENT_SEED=86872488;NIOH3_POSSESSED_RUN_ID="synthetic-only"',
           'NIOH3_BREAKPOINT_LIFECYCLE_PATH='+lua_literal(str(ROOT/'research/owned_breakpoint_lifecycle_ce.lua'))]
    for s in LOC['sites'].values():lines.append(f"putHex(B+{int(s['rva'],16)},'{s['bytes']}')")
    lines += [f"putHex(C-0x50,'{req}');putHex(B+0x45B8400,'{req}')",'put(S,B+0x21D9E24,8)',
              'put(B+0x45C44A0,0x20000000,8);fill(0x20001010,0x270)',
              'put(0x20001014,3,1);put(0x20001018,0xCC96,4);put(0x20001278,86872488,4);put(0x2000127C,183,2);put(0x2000127E,1,1);put(0x2000127F,1,1)',
              'put(B+0x45B83E4,0xCC96,4);put(F+0x1778,B+0x2237978,8)',
              'put(F+0x1618,B+0x20E1943,8);put(F+0x16C8,B+0x20E19C2,8)',
              f'put(F+0x1640,0x8E,1);put(F+0x1648,{extra},1);put(F-0xD0,3,1)',
              'put(B+0x45B5DF0,0x24000000,8);put(0x24000A98,0x25000000,8);put(0x25000000,0x26000000,8);put(0x26000004,1,4);put(F-0xB8,0x26000008,8)',
              'put(0x24000A90,0x27000000,8);put(0x27000000,0x28000000,8);put(0x27000008,0x28000018,8)']
    row=bytearray(0x30);row[0x28:0x2F]=bytes([0x8E,2,1,1,1,1,0])
    pt=bytearray(0x18);struct.pack_into('<4f',pt,0,1,2,3,4);pt[0x12]=0xD4;pt[0x13]=1
    lines.extend([f"putHex(0x26000008,'{row.hex()}')",f"putHex(0x28000000,'{pt.hex()}')"])
    l.run(';'.join(lines));return l

def arm(l):return l.run('return assert(loadfile('+lua_literal(str(CAP/'mode_upstream_ce.lua'))+'))()')
def capture(l):return l.run('return nioh3PossessedCapture')
def cleanup(l):
    l.run('drain()');p=capture(l)
    return {'cleanup_metadata':{'verified':not p['cleanup_pending'],
            'target':dict(p['identity'],module_base=None),'run_id':p['run_id']},
            'bridge_result':{'result':{'probe':{'active':p['active'],'cleanup_pending':p['cleanup_pending'],
              'owned_breakpoints':p['owned_breakpoints'],'debugger_broken':False,'run_id':p['run_id'],'schema':p['schema']},
               'breakpoints':l.run('return debug_getBreakpointList()')}}}
def envelope(p):
    base=0x140000000
    addresses=[f"0x{base+int(site['rva'],16):X}" for site in LOC['sites'].values()]
    return {'capture_metadata':{'phase':'mode-upstream','run_id':p['run_id'],'requested_pid':p['pid'],
              'target_seed':p['target_seed'],'target':dict(p['identity'],module_base=None),
              'read_only':True,'writes_game_memory':False,
              'source':{'phase_file':'mode_upstream_ce.lua','phase_sha256':'A'*64,'lifecycle_sha256':'B'*64},
              'fresh_phase_initialization':{'result':{'initialized':True,'debugger_broken':False,'breakpoints':[]}},
              'arm_verification':{'result':{'active':True,'run_id':p['run_id'],'schema':p['schema'],
                                    'debugger_broken':False,'owned_breakpoints':addresses,'breakpoints':addresses}}},
            'bridge_result':{'result':p}}
def happy(l):
    arm(l);l.run('source();queued();consume();context()');c=cleanup(l);return capture(l),c

def test_full_actual_lua_path_and_validator():
    l=fixture();p,c=happy(l);out=validate(envelope(p),c)
    assert out['upstream_link_validated'] and not out['product_oracle_accepted']
    assert out['producer_route']=='session_view_branch'
    assert len(p['events'])==4 and l.run('return armCount')==4 and l.run('return continues')==4
    assert not p['cleanup_pending'] and l.run('return onOpenProcess') is None

@pytest.mark.parametrize('tail',['0000','0200','FFFF','1234'])
def test_tail_is_preserved_not_interpreted_as_mode(tail):
    l=fixture(tail=tail);p,c=happy(l)
    assert validate(envelope(p),c)['padding_observed']==tail.lower()

@pytest.mark.parametrize('site',list(LOC['sites']))
def test_every_signature_checked_before_arming(site):
    l=fixture();l.run(f"put(B+{int(LOC['sites'][site]['rva'],16)},0,1)")
    with pytest.raises(RuntimeError,match='signature mismatch'):arm(l)
    assert l.run('return armCount')==0

@pytest.mark.parametrize('n',[1,2,3,4])
def test_partial_arm_cleanup(n):
    l=fixture();l.run(f'armFail={n}')
    with pytest.raises(RuntimeError):arm(l)
    c=cleanup(l);assert c['cleanup_metadata']['verified'];assert l.run('return debug_getBreakpointList()')=={}

def test_foreign_breakpoint_is_never_removed():
    l=fixture();l.run('bp[123]=true')
    with pytest.raises(RuntimeError,match='Foreign'):arm(l)
    assert l.run('return #removeCalls')==0

def test_wrong_literal_source_is_preserved_and_rejected():
    l=fixture(extra=0);arm(l);l.run('source()');p=capture(l)
    assert len(p['events'])==1 and 'Literal-one' in p['error'];assert not p['active'];assert l.run('return continues')==1;cleanup(l)

def test_unknown_caller_preserved_but_not_classified():
    l=fixture();arm(l);l.run('put(S,B+0x12345,8);source()');p=capture(l)
    assert p['events'][0]['route']=='unrecovered' and 'Unrecovered' in p['error'];cleanup(l)

def test_same_pid_new_handle_does_not_remove_new_process_breakpoints():
    l=fixture();arm(l);l.run('attachedHandle=1000;source()');p=capture(l);c=cleanup(l)
    assert 'Process instance' in p['error'] and not c['cleanup_metadata']['verified']
    assert l.run('return #removeCalls')==0

def test_bad_source_request_pointer_rejected():
    l=fixture();arm(l);l.run("putHex(C,'A8912D05B700010301010200');RBP=C;RSP=S;RDX=C;hit(0x10D9180)")
    assert 'request identity' in capture(l)['error'];cleanup(l)

def test_duplicate_target_enqueue_rejected():
    l=fixture();arm(l);l.run('source();source()');assert 'Second target' in capture(l)['error'];cleanup(l)

def test_queue_altered_request_rejected():
    l=fixture();arm(l);l.run('source();put(B+0x45B840A,3,1);queued()')
    assert 'Queue request changed' in capture(l)['error'];cleanup(l)

def test_consumer_altered_request_rejected():
    l=fixture();arm(l);l.run('source();queued();put(B+0x45B8409,0,1);consume()')
    assert 'request changed' in capture(l)['error'];cleanup(l)

def test_wrong_queue_node_rejected():
    l=fixture();arm(l);l.run('source();RSP=S-0x658;R15=C-0x50;R12=B+0x45B83E0;RAX=1;hit(0x10D9368)')
    assert 'Queue node' in capture(l)['error'];cleanup(l)

def test_unrelated_ui_consumer_ignored():
    l=fixture();arm(l);l.run('source();queued();put(F+0x1778,B+0x88888,8);consume()')
    assert capture(l)['active'] and len(capture(l)['events'])==2
    l.run('nioh3PossessedCapture.stop()');cleanup(l)

def test_generator_byte9_mismatch_rejected():
    l=fixture();arm(l);l.run('source();queued();consume();put(F+0x1648,0,1);context()')
    assert 'projection changed' in capture(l)['error'];cleanup(l)

def test_a90_huge_length_rejected():
    l=fixture();arm(l);l.run('source();queued();consume();put(0x27000008,0x28000000+2049*24,8);context()')
    assert 'row count cap' in capture(l)['error'];cleanup(l)

def test_unreadable_row_rejected():
    l=fixture();arm(l);l.run('source();queued();consume();mem[0x28000000]=nil;context()')
    assert 'Unreadable' in capture(l)['error'];cleanup(l)

def test_timeout_never_completes():
    l=fixture();arm(l);l.run('timers[1].f()');p=capture(l);c=cleanup(l)
    with pytest.raises(ValueError,match='final site'):validate(envelope(p),c)

def test_cleanup_failure_can_retry_without_rearming():
    l=fixture();arm(l);l.run('removeFail=true;source();queued();consume();context()');c=cleanup(l)
    assert not c['cleanup_metadata']['verified']
    l.run('removeFail=false;nioh3PossessedCapture.retry_cleanup()');c=cleanup(l)
    assert c['cleanup_metadata']['verified'] and l.run('return armCount')==4

def test_clear_requires_proven_cleanup():
    l=fixture();arm(l)
    with pytest.raises(RuntimeError,match='Stop and verify'):l.run('nioh3PossessedCapture.clear_captures()')
    l.run('nioh3PossessedCapture.stop()');cleanup(l);l.run('nioh3PossessedCapture.clear_captures()')
    assert capture(l)['events']=={}

@pytest.mark.parametrize('change', ['events','sequence','rva','thread','source','birth','cleanup','placement','frame','context','mode'])
def test_validator_rejects_mutated_evidence(change):
    l=fixture();p,c=happy(l)
    if change=='events':p['events']=[]
    elif change=='sequence':p['events'][1]['sequence']=3
    elif change=='rva':p['events'][2]['rva']='0x102A85E'
    elif change=='thread':p['events'][3]['thread_id']=20
    elif change=='source':p['events'][1]['source_address']='0x444'
    elif change=='birth':c['cleanup_metadata']['target']=dict(IDENT,creation_filetime='other')
    elif change=='cleanup':c['bridge_result']['result']['probe']['cleanup_pending']=True
    elif change=='placement':p['events'][3]['placements']['rows_for_terrain']=[]
    elif change=='frame':p['events'][3]['parent_frame']='0xABCD'
    elif change=='context':p['events'][3]['context']['counts']=[0,0,0,0,0]
    elif change=='mode':p['events'][0]['route']='owned_scroll_branch'
    with pytest.raises((ValueError,KeyError)):validate(envelope(p),c)


@pytest.mark.parametrize('route,return_rva,extra,request_offset', [
    ('owned_scroll_branch',0xF1E4F1,0,-0x50),
    ('session_view_branch',0x21D9E24,1,-0x50),
    ('parameterized_session_branch',0x21DC438,1,0x68),
    ('current_session_requeue',0x2233DE8,1,-0x29),
])
def test_validator_checks_each_producer_local_request(route,return_rva,extra,request_offset):
    l=fixture();p,c=happy(l);enq,queued=p['events'][:2]
    if extra != 1:
        request=bytearray.fromhex(enq['request_hex']);request[9]=extra;request_hex=request.hex().upper()
        for event in p['events']:event['request_hex']=request_hex
        p['events'][3]['extra_generation']=extra
    enq['route']=route;enq['return_rva']=f'0x{return_rva:X}'
    anchor=int(enq['caller_rsp'],0) if route=='parameterized_session_branch' else int(enq['caller_rbp'],0)
    address=anchor+request_offset
    enq['request_address']=queued['source_address']=f'0x{address:X}'
    validate(envelope(p),c)
    enq['request_address']=queued['source_address']=f'0x{address+1:X}'
    with pytest.raises(ValueError,match='producer-local'):validate(envelope(p),c)


@pytest.mark.parametrize('change', ['missing','phase','seed','read_only','fresh','arm_state','foreign_site'])
def test_validator_requires_complete_runner_arm_proof(change):
    l=fixture();p,c=happy(l);data=envelope(p)
    if change=='missing':data.pop('capture_metadata')
    elif change=='phase':data['capture_metadata']['phase']='assignment-origin'
    elif change=='seed':data['capture_metadata']['target_seed']+=1
    elif change=='read_only':data['capture_metadata']['read_only']=False
    elif change=='fresh':data['capture_metadata']['fresh_phase_initialization']['result']['breakpoints']=['0x1']
    elif change=='arm_state':data['capture_metadata']['arm_verification']['result']['active']=False
    elif change=='foreign_site':data['capture_metadata']['arm_verification']['result']['breakpoints'][0]='0x1'
    with pytest.raises(ValueError):validate(data,c)


def test_validator_checks_enqueue_stack_frame_link():
    l=fixture();p,c=happy(l);p['events'][1]['queue_rsp']='0x1234'
    with pytest.raises(ValueError,match='queue input/output identity'):validate(envelope(p),c)


def test_unmatched_callbacks_consume_total_hit_budget_and_resume():
    l=fixture();arm(l);l.run('nioh3PossessedCapture.max_hits=3;queued();queued();queued();queued()')
    p=capture(l);assert not p['active'] and 'Total callback hit budget' in p['error']
    assert p['total_hits']==4 and p['ignored_hits']==3 and l.run('return continues')==4
    cleanup(l)


def test_callback_deadline_is_independent_of_timer_delivery():
    l=fixture();arm(l);l.run('tick=121001;queued()')
    p=capture(l);assert not p['active'] and 'Callback deadline' in p['error']
    assert p['total_hits']==1 and l.run('return continues')==1
    cleanup(l)


def test_inactive_callback_still_resumes_and_is_bounded():
    l=fixture();arm(l);l.run("nioh3PossessedCapture.stop('test');queued()")
    p=capture(l);assert not p['active'] and p['total_hits']==1 and l.run('return continues')==1
    cleanup(l)

def test_runner_route_registered_and_strict():
    assert PHASES['mode-upstream']=='mode_upstream_ce.lua' and 'mode-upstream' in STRICT_V201_PHASES


def test_legacy_callback_thread_is_not_claimed_as_game_thread():
    l=fixture();p,c=happy(l)
    assert not validate(envelope(p),c)['target_thread_id_from_debug_api']
    assert all(e['thread_id_source'].startswith('callback_api_unverified:') for e in p['events'])

def test_explicit_debug_event_api_is_preferred_to_legacy_callback():
    l=fixture();l.run('function debug_getCurrentThreadID() return 9900 end')
    p,c=happy(l)
    assert validate(envelope(p),c)['target_thread_id_from_debug_api']
    assert all(e['thread_id']==9900 for e in p['events'])
