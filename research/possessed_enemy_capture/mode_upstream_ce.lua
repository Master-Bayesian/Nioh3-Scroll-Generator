-- PC v2.01: one upstream enqueue -> queue copy -> mission consume -> context.
-- No target writes or native calls; four owned execute hardware breakpoints.
local identity=assert(NIOH3_POSSESSED_TARGET_IDENTITY,'Use the verified Python runner')
assert(identity.executable_sha256=='4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159','Unapproved executable')
assert(identity.image_size==77814240 and type(identity.creation_filetime)=='string','Missing birth identity')
local seed=assert(NIOH3_ASSIGNMENT_SEED,'Explicit target seed required')
assert(seed>=0 and seed<=0xFFFFFFFF and seed%1==0,'Invalid seed')
assert(type(NIOH3_POSSESSED_RUN_ID)=='string' and #NIOH3_POSSESSED_RUN_ID>0 and #NIOH3_POSSESSED_RUN_ID<=128,'Run ID bound')
local sites={
-- SIGNATURES_BEGIN
 request_enqueue={rva=0x10d9180,hex='48895C240855565741544155415641574881EC20060000488B0592AD3D03'},
 request_queued={rva=0x10d9368,hex='4A895C203042896C20386642896C203C'},
 mission_consume={rva=0x20e198c,hex='48895C24084889742410574881ECA00000008A4109488BF2'},
 context_resolved={rva=0x1029fee,hex='0F84E12800004181E6FFFFFF0F'},
-- SIGNATURES_END
}
local base=assert(getAddressSafe('Nioh3.exe'));local pid=assert(getOpenedProcessID())
local handle=assert(getOpenedProcessHandle());assert(pid==identity.process_id,'Wrong PID')
assert(not nioh3PossessedCapture or (nioh3PossessedCapture.active==false and nioh3PossessedCapture.cleanup_pending==false),'Previous owner unresolved')
assert(type(getTickCount)=='function','Monotonic tick API unavailable')
local startedTick=getTickCount()
local function hex(x) return string.format('0x%X',x) end
local function guard()
 assert(getOpenedProcessID()==pid and getOpenedProcessHandle()==handle and getAddressSafe('Nioh3.exe')==base,'Process instance/handle/module changed')
end
local total=0
local function bytes(a,n)
 guard();assert(type(a)=='number' and a>0 and a%1==0 and n>=0 and n<=65536 and n%1==0,'Invalid read')
 total=total+n;assert(total<=2*1024*1024,'Read budget exhausted')
 local b=readBytes(a,n,true);assert(type(b)=='table' and #b==n,'Unreadable '..hex(a));return b
end
local function val(b,o,n) local v=0;for i=n,1,-1 do v=(v<<8)|b[o+i] end;return v end
local function uint(a,n) return val(bytes(a,n),0,n) end
local function ptr(a) return uint(a,8) end
local function hx(b) local t={};for i,v in ipairs(b) do t[i]=string.format('%02X',v) end;return table.concat(t) end
local function raw(a,n) return hx(bytes(a,n)) end
-- Prefer an explicit debugger-event API. Legacy getCurrentThreadId can be
-- the CE callback thread, as the packaged Run D diagnostic illustrates. Never
-- use that legacy value to choose a target RNG stream or claim game ownership.
local threadSource='unavailable'
local function thread()
 for _,name in ipairs({'debug_getCurrentThreadId','debug_getCurrentThreadID','getCurrentThreadId','getCurrentThreadID'}) do
  if type(_G[name])=='function' then local ok,v=pcall(_G[name]);if ok and type(v)=='number' and v>0 and v%1==0 then
   threadSource=(name:sub(1,6)=='debug_' and 'debug_event_api:' or 'callback_api_unverified:')..name
   return v
  end end
 end;error('Callback/debug thread identity unavailable')
end
local function relative(a) if a>=base and a<base+0x5000000 then return hex(a-base) end;return nil end
for name,s in pairs(sites) do s.address=base+s.rva;assert(raw(s.address,#s.hex/2)==s.hex,'v2.01 signature mismatch: '..name) end
local inventory=debug_getBreakpointList()
assert((type(inventory)=='table' and next(inventory)==nil) or (inventory==nil and not debug_isDebugging()),'Foreign or unknown breakpoint inventory')
local p={schema='nioh3.mode-upstream.v1',run_id=NIOH3_POSSESSED_RUN_ID,
 pid=pid,module_base=hex(base),identity=identity,target_seed=seed,
 mode_label=NIOH3_MATERIALIZATION_MODE_LABEL,read_only=true,writes_game_memory=false,
 active=false,events={},event_sequence=0,max_events=16,max_seconds=120,
 max_hits=2048,total_hits=0,started_tick_ms=startedTick,elapsed_ms=0,
 conclusion='unvalidated',ignored_hits=0,read_bytes=total}
nioh3PossessedCapture=p
local owner=assert(loadfile(assert(NIOH3_BREAKPOINT_LIFECYCLE_PATH)))()({
 list=function() guard();return debug_getBreakpointList() end,
 remove=function(a) guard();return debug_removeBreakpoint(a) end,
 remove_id=function(id) guard();return debug_removeBreakpointByID(id) end,
 timer=createTimer,
 arm=function(a,f) guard();return debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,f) end,
},p)
local deferred=false
local oldSwitch,switched=onOpenProcess,nil
local function cleanup()
 local result=owner.stop()
 if onOpenProcess==switched then onOpenProcess=oldSwitch end
 return result
end
local function finish(reason)
 p.active=false;p.stop_reason=p.stop_reason or reason;p.cleanup_pending=true
 if not deferred then deferred=true
  local ok=pcall(createTimer,1,function() deferred=false;cleanup() end)
  if not ok then deferred=false;cleanup() end
 end
end
function p.stop(reason) finish(reason or 'manual');return p.status() end
function p.retry_cleanup() return cleanup() end
function p.clear_captures()
 assert(not p.active and not p.cleanup_pending and #p.owned_breakpoints==0,'Stop and verify cleanup first')
 p.events={};p.event_sequence=0;p.conclusion='cleared'
end
function p.status() return {active=p.active,run_id=p.run_id,cleanup_pending=p.cleanup_pending,
 owned_breakpoints=p.owned_breakpoints,event_count=#p.events,stop_reason=p.stop_reason,
 read_bytes=total,ignored_hits=p.ignored_hits,total_hits=p.total_hits,elapsed_ms=p.elapsed_ms,
 error=p.error,conclusion=p.conclusion} end
local function emit(site,e)
 assert(#p.events<p.max_events,'Event bound');p.event_sequence=p.event_sequence+1
 e.sequence=p.event_sequence;e.site=site;e.rva=hex(sites[site].rva);e.thread_id=thread();e.thread_id_source=threadSource
 e.read_bytes_total=total;p.read_bytes=total;p.events[#p.events+1]=e
end
local state=nil
local function ignore() p.ignored_hits=p.ignored_hits+1 end
local returns={ [0xF1E4F1]='owned_scroll_branch', [0x21D9E24]='session_view_branch',
 [0x21DC438]='parameterized_session_branch', [0x2233DE8]='current_session_requeue' }
local function sessionView()
 local root=ptr(base+0x45C44A0);assert(root~=0,'Missing session root')
 local b=root+0x1010
 return {root=hex(root),view=hex(b),kind=uint(b,4),playthrough=uint(b+4,1),
  mission_type=uint(b+8,4),scroll_seed=uint(b+0x268,4),metadata=uint(b+0x26C,2),
  byte6=uint(b+0x26E,1),byte8_source=uint(b+0x26F,1)}
end
local function a90(terrain)
 local manager=ptr(base+0x45B5DF0);assert(manager~=0,'Null parameter manager')
 local ctx=ptr(manager+0xA90);assert(ctx~=0,'Missing A90 table')
 local begin,ending=ptr(ctx),ptr(ctx+8)
 assert(begin>0 and ending>=begin and (ending-begin)%0x18==0,'A90 vector bounds')
 local count=(ending-begin)//0x18;assert(count>0 and count<=2048,'A90 row count cap')
 local block=bytes(begin,count*0x18);local rows={}
 for i=0,count-1 do if block[i*0x18+0x12+1]==terrain then
  assert(#rows<128,'Terrain placement row cap')
  local r={};for j=1,0x18 do r[j]=block[i*0x18+j] end
  rows[#rows+1]={row_index=i,address=hex(begin+i*0x18),terrain=terrain,
   slot=r[0x13+1],raw_hex=hx(r)}
 end end
 assert(#rows>0,'No authored placement rows for terrain')
 return {context=hex(ctx),begin=hex(begin),finish=hex(ending),row_count=count,
  stride=0x18,rows_for_terrain=rows,identity_join='not_live_actor_validated'}
end
local handlers={}
function handlers.request_enqueue()
 local req=bytes(RDX,12);if val(req,0,4)~=seed then ignore();return end
 assert(state==nil,'Second target enqueue is ambiguous; preserve evidence')
 local ret=ptr(RSP);local route=returns[ret-base]
 local e={request_address=hex(RDX),request_hex=hx(req),return_rva=relative(ret),
  caller_rbp=hex(RBP),caller_rsp=hex(RSP),route=route or 'unrecovered',padding_observed=hx({req[11],req[12]})}
 -- Record the source before checking the recovered route, so falsifying data survive.
 if route=='session_view_branch' then e.session=sessionView() end
 emit('request_enqueue',e)
 assert(route~=nil,'Unrecovered enqueue caller')
 if route=='owned_scroll_branch' or route=='session_view_branch' then assert(RDX==RBP-0x50,'Unexpected caller-local request identity')
 elseif route=='parameterized_session_branch' then assert(RDX==RSP+0x68,'Unexpected parameterized request identity')
 elseif route=='current_session_requeue' then assert(RDX==RBP-0x29,'Unexpected requeue request identity') end
 if route=='owned_scroll_branch' then assert(req[10]==0,'Literal-zero source contradicted')
 elseif route=='session_view_branch' or route=='parameterized_session_branch' then assert(req[10]==1,'Literal-one source contradicted') end
 if e.session then
  assert(e.session.mission_type==0xCC96 and e.session.scroll_seed==seed,'Session source identity mismatch')
  assert(e.session.playthrough==req[8] and e.session.metadata==val(req,4,2) and e.session.byte6==req[7], 'Session-to-request fields differ')
  assert((e.session.byte8_source~=0 and 1 or 0)==req[9],'Session bool conversion mismatch')
 end
 state={phase=1,request=hx(req),source=RDX,enqueue_rsp=RSP,producer_thread=thread(),route=route}
end
function handlers.request_queued()
 if not state then ignore();return end
 assert(state.phase==1,'Unexpected second queue copy')
 assert(thread()==state.producer_thread,'Queue copy changed producer thread')
 assert(R15==state.source and RSP+0x658==state.enqueue_rsp,'Enqueue frame/source mismatch')
 assert(R12==base+0x45B83E0 and RAX>=0 and RAX%0x58==0 and RAX//0x58<3,'Queue node identity mismatch')
 local node=R12+RAX;local q=raw(node+0x20,12)
 emit('request_queued',{queue_node=hex(node),queue_index=RAX//0x58,queue_rsp=hex(RSP),request_hex=q,
  request_address=hex(node+0x20),mission_type=uint(node+4,4),source_address=hex(R15)})
 assert(q==state.request and uint(node+4,4)==0xCC96,'Queue request changed')
 state.phase=2;state.queue_node=node
end
function handlers.mission_consume()
 if not state then ignore();return end
 local ret=ptr(RSP);if ret~=base+0x2237978 then ignore();return end
 assert(state.phase==2,'Mission request consumed before matching enqueue')
 assert(RCX==base+0x45B8400,'Mission request is not queue-head payload')
 local q=raw(RCX,12)
 emit('mission_consume',{request_address=hex(RCX),request_hex=q,output_address=hex(RDX),
  return_rva=relative(ret),consumer_rsp=hex(RSP),queue_head=hex(base+0x45B83E0)})
 assert(q==state.request,'Queue-to-mission request changed')
 state.phase=3;state.consumer_thread=thread();state.consumer_rsp=RSP
end
function handlers.context_resolved()
 if not state or state.phase~=3 then ignore();return end
 -- Only the child of the accepted mission call. Do not label UI contexts.
 if ptr(RBP+0x1618)~=base+0x20E1943 or ptr(RBP+0x16C8)~=base+0x20E19C2 or ptr(RBP+0x1778)~=base+0x2237978 then ignore();return end
 assert(thread()==state.consumer_thread and RBP+0x1778==state.consumer_rsp,'Generator frame/thread mismatch')
 assert((R14&0xFFFFFFFF)==seed,'Generator seed changed')
 local key=uint(RBP+0x1640,1);local extra=uint(RBP+0x1648,1);local terrain=R15&255
 local manager=ptr(base+0x45B5DF0);local ctx=ptr(manager+0xA98);local store=ptr(ctx)
 local count=uint(store+4,4);assert(count>0 and count<=256,'A98 row cap')
 local selected=ptr(RSP+0x48)
 assert(selected==RAX and selected>=store+8 and (selected-store-8)%0x30==0 and (selected-store-8)//0x30<count,'Selected A98 row identity')
 local r=bytes(selected,0x30);assert(r[0x28+1]==key,'Context key/row mismatch')
 local req=bytes(base+0x45B8400,12)
 local e={request_hex=hx(req),seed=R14&0xFFFFFFFF,parent_frame=hex(RBP),context_key=key,
  extra_generation=extra,terrain=terrain,playthrough=uint(RSP+0x30,1),
  ancestry_rvas={'0x20E1943','0x20E19C2','0x2237978'},
  context={manager=hex(manager),table=hex(ctx),store=hex(store),row_count=count,
   row_index=(selected-store-8)//0x30,address=hex(selected),raw_hex=hx(r),path=r[0x29+1],
   counts={r[0x2A+1],r[0x2B+1],r[0x2C+1],r[0x2D+1],r[0x2E+1]}},
  placements=a90(terrain)}
 emit('context_resolved',e)
 assert(e.request_hex==state.request and extra==req[10] and e.playthrough==req[8],'Generator request projection changed')
 state.phase=4;p.conclusion='upstream_link_captured_not_product_acceptance';finish('upstream_request_bound')
end
local function callback(name)
 return function()
  local ok,err=xpcall(function()
   guard();p.total_hits=p.total_hits+1;assert(p.total_hits<=p.max_hits,'Total callback hit budget')
   local now=getTickCount();local elapsed=now-startedTick;if elapsed<0 then elapsed=elapsed+0x100000000 end
   assert(elapsed>=0 and elapsed<=p.max_seconds*1000,'Callback deadline');p.elapsed_ms=elapsed
   assert(RIP==sites[name].address,'Unexpected breakpoint PC');assert(p.ignored_hits<1024,'Unrelated-hit budget')
   if p.active then handlers[name]() end
  end,debug.traceback)
  if not ok then p.error=tostring(err):sub(1,4096);finish('capture_error') end
  local continued,why=pcall(debug_continueFromBreakpoint,co_run)
  if not continued then p.error=tostring(why):sub(1,4096);finish('resume_error') end
  return 1
 end
end
switched=function(...)
 if not (getOpenedProcessID()==pid and getOpenedProcessHandle()==handle) then p.error='Attached process switched';finish('process_changed') end
 if type(oldSwitch)=='function' then return oldSwitch(...) end
end
onOpenProcess=switched
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Debugger attachment failed')
p.active=true
for _,name in ipairs({'request_enqueue','request_queued','mission_consume','context_resolved'}) do
 local ok,err=owner.arm(sites[name].address,callback(name));if not ok then p.error=tostring(err):sub(1,4096);finish('arm_failed');error(p.error) end
end
local ok,err=pcall(createTimer,120000,function() if p.active then finish('timeout') end end)
if not ok then p.error=tostring(err):sub(1,4096);finish('timer_failed');error(p.error) end
return p.status()

