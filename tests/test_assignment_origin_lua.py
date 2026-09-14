from pathlib import Path
import sys,json,struct,copy
import pytest
ROOT=Path(__file__).resolve().parents[1]
CAP=ROOT/'research'/'possessed_enemy_capture'
sys.path.insert(0,str(CAP));sys.path.insert(0,str(Path(__file__).parent))
from lua54_test_runtime import Lua54
from assignment_origin_reference import draw_10000
from validate_assignment_origin_capture import validate

BASE=0x140000000;FRAME=0x10000000;D=0x22000000
IDENT={'process_id':777,'creation_filetime':'123456789012345678','image_size':77814240,
       'executable_sha256':'4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'}
LOC=json.loads((CAP/'assignment_origin_v201_locators.json').read_text())
MOCK='''
B=0x140000000;F=0x10000000;mem={};bp={};timers={};attachedPid=777;attachedHandle=999
armedCount=0;removeCalls={};debugging=true;thread=7;armFail=0;removeFail=false;timerFail=false;continues=0
function fill(a,n,v) for i=0,n-1 do mem[a+i]=v or 0 end end
function put(a,v,n) for i=0,n-1 do mem[a+i]=v&255;v=v>>8 end end
function putHex(a,h) for i=1,#h,2 do mem[a+(i-1)/2]=tonumber(h:sub(i,i+1),16) end end
function readBytes(a,n,asTable) local t={} for i=0,n-1 do if mem[a+i]==nil then return nil end;t[#t+1]=mem[a+i] end;return t end
function getAddressSafe(s) return B end
function getOpenedProcessID() return attachedPid end
function getOpenedProcessHandle() return attachedHandle end
function getCurrentThreadId() return thread end
function debug_getBreakpointList() local t={} for a in pairs(bp) do t[#t+1]=a end;table.sort(t);return t end
function debug_isDebugging() return debugging end
function debugProcess(_) debugging=true end
function debug_setBreakpoint(a,n,t,m,fn) armedCount=armedCount+1;if armFail==armedCount then return false end;bp[a]=fn;return true,a end
function debug_removeBreakpoint(a) removeCalls[#removeCalls+1]=a;if not removeFail then bp[a]=nil end;return not removeFail end
function debug_removeBreakpointByID(id) return debug_removeBreakpoint(id) end
function debug_continueFromBreakpoint(mode) assert(mode==co_run);continues=continues+1;return true end
function createTimer(ms,fn) if timerFail then error('mock timer failure') end;timers[#timers+1]={ms=ms,fn=fn};return true end
function drainSmall() local n=0;while true do local ix=nil;for i,t in ipairs(timers) do if t.ms<=100 then ix=i;break end end;if not ix then break end;local t=table.remove(timers,ix);t.fn();n=n+1;assert(n<=20,'unbounded cleanup') end;return n end
function hit(rva) RIP=B+rva;assert(bp[RIP],'site not armed');return bp[RIP]() end
function beginPass(sel)
 RSP=F-0x108;RBP=F;RCX=F-0x20;RDX=sel
 put(RSP,B+(sel==0 and 0x102C898 or 0x102C8AF),8)
 return hit(0x10283C0)
end
function trial(pos,sel,state,ticket,threshold)
 RSP=F-0x180;RSI=0x22000000+pos*20;R15=sel;RAX=ticket;RDI=threshold;put(F+0xC0,state,4)
 return hit(0x1028570)
end
function copyTask(pos)
 local src=0x22000000+pos*20;local task=0x34000000+pos*0x200
 fill(task,0x158);put(task+0x20,0xF3C+pos,4);put(task+0x24,0xCC96,4);put(task+0x28,0xDCB98,4)
 RSP=0x36000000+pos*0x100;put(RSP+0x28,B+0x1C24635,8);RBX=src;RDI=task
 return hit(0x1BF2D54)
end
function linkTask(pos)
 local src=0x22000000+pos*20;local task=0x35000000+pos*0x200
 fill(task,0x158);put(task+0x20,0xF3C+pos,4);put(task+0x24,0xCC96,4);put(task+0x28,0xDCB98,4)
 for i=0,19 do mem[task+0x80+i]=mem[src+i] end
 RDI=src;R14=task;return hit(0x1C24662)
end
function makeTable(ctx,store,h,entries,width,key,stride,data)
 fill(ctx,0x28);fill(store,8+stride);fill(h,0x18);fill(entries,16,255)
 put(ctx,store,8);put(ctx+0x20,h,8);put(store+4,1,4)
 put(h+4,0xFFFFFFFF,width);put(h+8,entries,8);put(h+0x10,entries+16,8)
 put(entries,key,width);put(entries+4,0,4);putHex(store+8,data)
end
bptExecute=0;bpmDebugRegister=1;co_run=0
'''

def fixture(state=8495488, threshold=5000,classes=(0,1),extra=1):
    L=Lua54();L.run(MOCK)
    s=[]
    for site in LOC['sites'].values():s.append(f"putHex(B+{int(site['rva'],16)},'{site['bytes']}')")
    from run_possessed_enemy_observer import lua_literal
    s += [f'NIOH3_POSSESSED_TARGET_IDENTITY={lua_literal(IDENT)}',
          'NIOH3_ASSIGNMENT_SEED=86872488;NIOH3_POSSESSED_RUN_ID="synthetic-only"',
          'NIOH3_BREAKPOINT_LIFECYCLE_PATH='+json.dumps(str(ROOT/'research/owned_breakpoint_lifecycle_ce.lua')),
          'put(F+0x1618,B+0x20E1943,8);put(F+0x16C8,B+0x20E19C2,8);put(F+0x1778,B+0x2237978,8)',
          f'put(F+0x1608,86872488,4);put(F+0x1648,{extra},1);put(F+0x1640,0x8E,1);put(F+0xC0,{state},4)',
          "putHex(F-0xB8,'FFFFFFFFFFFFFFFF')",
          'put(F-0x20,0x20000000,8);fill(0x20000000,0x28);put(0x20000000,0x21000000,8);put(0x20000008,0x21000028,8)',
          f'fill(0x21000000,0x28);put(0x21000000,0x22000000,8);put(0x21000008,0x22000000+{len(classes)*20},8)',
          'put(B+0x47558C0,0x28000000,8);fill(0x28000000,0x530);put(0x28000528,7,4);put(B+0x44BFB88,F+0xC0,8);put(B+0x44BFB90,0,8);put(B+0x45BFA10,0,4);put(B+0x44BFB84,0,4)',
          'fill(B+0x45B8400,12);put(B+0x45B8400,86872488,4);put(B+0x45B8409,1,1)',
          'put(B+0x45B5DF0,0x23000000,8);fill(0x23000000,0xB20)',
          'put(0x23000038,0x24000000,8);put(0x23000118,0x24100000,8);put(0x23000230,0x24200000,8);put(0x23000A98,0x24300000,8)',
          f'put(B+0x45B5E00,0x30000000,8);put(0x30000000,0x31000000,8);put(0x31000048,0x32000000,8);put(0x32000000,0x33000000,8);put(0x32000008,{len(classes)},4)']
    for i,cls in enumerate(classes):
        raw=bytearray(20);struct.pack_into('<II',raw,0,0xF3C+i,0xDCB98);raw[16]=cls
        s += [f"putHex(0x22000000+{i*20},'{raw.hex()}')",f'put(0x33000000+{i*16+8},0x35000000+{i*0x200},8)']
    enemy=bytearray(0x398);struct.pack_into('<H',enemy,0xA8,7);struct.pack_into('<I',enemy,0x244,1)
    subtype=bytearray(0x54)
    cfg=bytearray(32);struct.pack_into('<i',cfg,16,threshold);struct.pack_into('<f',cfg,24,1)
    for off,width,key,stride,raw in [(0,4,0xDCB98,0x398,enemy),(0x100000,2,7,0x54,subtype),(0x200000,4,0x4543,32,cfg)]:
        s.append(f"makeTable({0x24000000+off},{0x25000000+off},{0x26000000+off},{0x27000000+off},{width},{key},{stride},'{raw.hex()}')")
    context=bytearray(0x30);context[0x2A:0x2F]=bytes((1,2,3,4,5))
    s.append(f"makeTable(0x24300000,0x25300000,0x26300000,0x27300000,1,0x8E,0x30,'{context.hex()}')")
    L.run(';'.join(s));return L

def arm(L):return L.run(f"return assert(loadfile({json.dumps(str(CAP/'assignment_origin_ce.lua'))}))()")
def snapshot(L):return L.run('return nioh3PossessedCapture')
def conclude(L):
    L.run('drainSmall()');p=snapshot(L)
    cleanup={'cleanup_metadata':{'verified':not p['cleanup_pending'],'run_id':p['run_id'],'target':p['identity']},
             'bridge_result':{'result':{'probe':{k:p[k] for k in ('active','cleanup_pending','owned_breakpoints','run_id','schema')},
                                         'breakpoints':L.run('return debug_getBreakpointList()')}}}
    return p,cleanup

def happy(L):
    arm(L);L.run('beginPass(0)');s,t=draw_10000(8495488);L.run(f'trial(0,0,{s},{t},5000)')
    L.run('beginPass(1)');s,t=draw_10000(s);L.run(f'trial(1,1,{s},{t},5000);put(0x22000000+20+15,1,1)')
    L.run('copyTask(0);linkTask(0);copyTask(1);linkTask(1)')
    return conclude(L)

def test_actual_lua_four_sites_and_causal_fixture_validates():
    L=fixture();p,c=happy(L)
    assert L.run('return armedCount')==4 and not p['active'] and not p['cleanup_pending']
    assert L.run('return continues')==8
    r=validate(p,c);assert r['selected_spawn']==0xF3D and r['trials']==2 and r['tasks']==2
    assert len(p['events'])==8

def test_signature_mismatch_before_arming():
    L=fixture();L.run('put(B+0x10283C0,0,1)')
    with pytest.raises(RuntimeError,match='signature mismatch'):arm(L)
    assert L.run('return armedCount')==0

def test_missing_exe_identity_before_arming():
    L=fixture();L.run('NIOH3_POSSESSED_TARGET_IDENTITY=nil')
    with pytest.raises(RuntimeError,match='runner'):arm(L)
    assert L.run('return armedCount')==0

def test_foreign_breakpoint_not_deleted():
    L=fixture();L.run('bp[123]=true')
    with pytest.raises(RuntimeError,match='Foreign'):arm(L)
    assert L.run('return bp[123]') is True and L.run('return #removeCalls')==0

def test_partial_arm_failure_own_only_cleanup():
    L=fixture();L.run('armFail=3')
    with pytest.raises(RuntimeError):arm(L)
    L.run('drainSmall()');assert not snapshot(L)['cleanup_pending'] and L.run('return #debug_getBreakpointList()')==0

def test_cleanup_failure_remains_owned_then_explicit_retry():
    L=fixture();arm(L);L.run('removeFail=true;nioh3PossessedCapture.stop("test");drainSmall()')
    assert snapshot(L)['cleanup_pending'] and len(snapshot(L)['owned_breakpoints'])==4
    assert L.run('return #removeCalls')<=20
    L.run('removeFail=false;nioh3PossessedCapture.retry_cleanup();drainSmall()')
    assert not snapshot(L)['cleanup_pending']

def test_process_replacement_never_removes_new_process_breakpoints():
    L=fixture();arm(L);L.run('attachedHandle=1000;beginPass(0);drainSmall()')
    assert snapshot(L)['conclusion']=='rejected' and snapshot(L)['cleanup_pending']
    assert L.run('return #removeCalls')==0

def test_wrong_parent_call_ignored_without_guessing_semantics():
    L=fixture();arm(L);L.run('put(F+0x1618,B+0x1234,8);beginPass(0)')
    assert snapshot(L)['ignored_hits']==1 and snapshot(L)['events']=={}

def test_thread_route_diagnostic_does_not_override_exact_parent_rng_chain():
    L=fixture(state=8495488,threshold=10000,classes=(0,),extra=0);arm(L)
    L.run('put(B+0x44BFB88,F+0xD0,8);put(F+0xD0,8495488,4);beginPass(0)')
    state,ticket=draw_10000(8495488)
    L.run(f'put(F+0xD0,{state},4);trial(0,0,{state},{ticket},10000);put(0x22000000+15,1,1);copyTask(0);linkTask(0)')
    p,c=conclude(L);result=validate(p,c)
    assert result['rng_route_diagnostic']=='primary_scoped'
    assert result['rng_route_diagnostic_matches_parent'] is False
    assert result['confirmed_rng_source']=='generator parent frame +0xC0'

def test_preexisting_flag_falsifies_earliest_source():
    L=fixture();arm(L);L.run('put(0x2200000F,1,1);beginPass(0);drainSmall()')
    assert 'already set' in snapshot(L)['error']

def test_existing_task_without_ctor_not_accepted():
    L=fixture();arm(L);L.run('beginPass(0);linkTask(0);drainSmall()')
    assert 'Existing task reused' in snapshot(L)['error']

def test_unreadable_source_is_error_not_zero_data():
    L=fixture();arm(L);L.run('mem[0x22000000]=nil;beginPass(0);drainSmall()')
    assert 'Unreadable' in snapshot(L)['error']

def test_oversized_vector_is_bounded_before_reads():
    L=fixture();arm(L);L.run('put(0x21000008,0x22000000+97*20,8);beginPass(0);drainSmall()')
    assert snapshot(L)['conclusion']=='rejected'

def test_timeout_closes_without_success():
    L=fixture();arm(L);L.run('for _,t in ipairs(timers) do if t.ms==120000 then t.fn();break end end;drainSmall()')
    assert snapshot(L)['stop_reason']=='time_budget' and not snapshot(L)['active']

def test_clear_requires_cleanup_then_empties():
    L=fixture();arm(L)
    with pytest.raises(RuntimeError,match='Stop'):L.run('nioh3PossessedCapture.clear_captures()')
    L.run('nioh3PossessedCapture.stop();drainSmall();nioh3PossessedCapture.clear_captures()')
    assert snapshot(L)['events']=={}

@pytest.mark.parametrize('mutation',['empty','rng','threshold','flag','copy','missing-origin','cleanup','cleanup-inventory','thread','config','context-table'])
def test_offline_validator_rejects_bad_or_vacuous_evidence(mutation):
    L=fixture();p,c=happy(L)
    if mutation=='empty':p['events']=[]
    elif mutation=='rng':p['events'][1]['parent_rng']['state']^=1
    elif mutation=='threshold':p['events'][1]['threshold']+=1
    elif mutation=='flag':p['events'][-1]['flag8f']=0
    elif mutation=='copy':p['events'][-1]['descriptor_hex']='00'*20
    elif mutation=='missing-origin':p['events'].pop(2)
    elif mutation=='cleanup':c['cleanup_metadata']['verified']=False
    elif mutation=='cleanup-inventory':c['bridge_result']['result']['breakpoints']=None
    elif mutation=='thread':p['events'][-1]['thread_id']=8
    elif mutation=='config':p['events'][0]['tables']['config_4543'].pop('present')
    elif mutation=='context-table':p['events'][0]['tables']['generator_context']['address']='0x1'
    with pytest.raises((ValueError,KeyError)):validate(p,c)


def test_unknown_debug_event_thread_stops_instead_of_falling_back():
    L=fixture();arm(L);L.run('thread=0;beginPass(0);drainSmall()')
    assert 'thread ID unavailable' in snapshot(L)['error']

def test_budget_timer_failure_releases_already_armed_sites():
    L=fixture();L.run('timerFail=true')
    with pytest.raises(RuntimeError):arm(L)
    assert not snapshot(L)['cleanup_pending'] and L.run('return #debug_getBreakpointList()')==0
