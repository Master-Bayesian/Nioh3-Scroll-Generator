-- PC v2.01: bounded SEQUENTIAL request/queue/consumer/output observation.
-- Four FIXED execute hardware breakpoints. No target writes or native calls.
-- The fourth site is the generator RETURN, not the old pre-generation context.
local identity=assert(NIOH3_POSSESSED_TARGET_IDENTITY,'Use the verified Python runner')
assert(identity.executable_sha256=='4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159','Unapproved executable')
assert(identity.image_size==77814240 and type(identity.creation_filetime)=='string','Missing birth identity')
local seed=assert(NIOH3_ASSIGNMENT_SEED,'Explicit target seed required')
assert(seed>=0 and seed<=0xFFFFFFFF and seed%1==0,'Invalid seed')
assert(type(NIOH3_POSSESSED_RUN_ID)=='string' and #NIOH3_POSSESSED_RUN_ID>0 and #NIOH3_POSSESSED_RUN_ID<=128,'Run ID bound')
local sites={
-- SIGNATURES_BEGIN
 request_enqueue={rva=0x10D9180,hex='48895C240855565741544155415641574881EC20060000488B0592AD3D03'},
 request_queued={rva=0x10D9368,hex='4A895C203042896C20386642896C203C'},
 mission_consume={rva=0x20E198C,hex='48895C24084889742410574881ECA00000008A4109488BF2'},
 mission_generated={rva=0x2237978,hex='488B0D81E43702488D542468448BC7'},
-- SIGNATURES_END
}
local MAX_EVENTS,MAX_REQUESTS,MAX_INVOCATIONS,MAX_HITS,MAX_READ=128,16,16,4096,4*1024*1024
local WINDOW_MS=120000
local base=assert(getAddressSafe('Nioh3.exe'));local pid=assert(getOpenedProcessID())
local handle=assert(getOpenedProcessHandle());assert(pid==identity.process_id,'Wrong PID')
assert(not nioh3PossessedCapture or (nioh3PossessedCapture.active==false and nioh3PossessedCapture.cleanup_pending==false),'Previous owner unresolved')
assert(type(getTickCount)=='function','Monotonic tick API unavailable')
local startedTick=getTickCount()
local function hex(x) return string.format('0x%X',x) end
local function relative(a) if a>=base and a<base+0x5000000 then return hex(a-base) end;return nil end
local function guard()
 assert(getOpenedProcessID()==pid and getOpenedProcessHandle()==handle and getAddressSafe('Nioh3.exe')==base,'Process instance/handle/module changed')
end
local total=0
local function bytes(a,n)
 guard();assert(type(a)=='number' and a>0 and a%1==0 and n>=0 and n<=65536 and n%1==0,'Invalid read')
 total=total+n;assert(total<=MAX_READ,'Read budget exhausted')
 local b=readBytes(a,n,true);assert(type(b)=='table' and #b==n,'Unreadable '..hex(a));return b
end
local function val(b,o,n) local v=0;for i=n,1,-1 do v=(v<<8)|b[o+i] end;return v end
local function uint(a,n) return val(bytes(a,n),0,n) end
local function ptr(a) return uint(a,8) end
local function hx(b) local t={};for i,v in ipairs(b) do t[i]=string.format('%02X',v) end;return table.concat(t) end
local function raw(a,n) return hx(bytes(a,n)) end
local function elapsed()
 local n=getTickCount()-startedTick;if n<0 then n=n+0x100000000 end;return n
end
local threadSource='unavailable'
local function thread()
 for _,name in ipairs({'debug_getCurrentThreadId','debug_getCurrentThreadID','getCurrentThreadId','getCurrentThreadID'}) do
  if type(_G[name])=='function' then local ok,v=pcall(_G[name]);if ok and type(v)=='number' and v>0 and v%1==0 then
   threadSource=(name:sub(1,6)=='debug_' and 'debug_event_api:' or 'callback_api_unverified:')..name;return v
  end end
 end;error('Callback/debug thread identity unavailable')
end
for name,s in pairs(sites) do s.address=base+s.rva;assert(raw(s.address,#s.hex/2)==s.hex,'v2.01 signature mismatch: '..name) end
local inventory=debug_getBreakpointList()
assert((type(inventory)=='table' and next(inventory)==nil) or (inventory==nil and not debug_isDebugging()),'Foreign or unknown breakpoint inventory')
local p={schema='nioh3.mode-upstream-sequence.v1',run_id=NIOH3_POSSESSED_RUN_ID,
 pid=pid,module_base=hex(base),identity=identity,target_seed=seed,
 mode_label=NIOH3_MATERIALIZATION_MODE_LABEL,read_only=true,writes_game_memory=false,calls_game_functions=false,
 active=false,events={},event_sequence=0,max_events=MAX_EVENTS,max_seconds=120,
 max_hits=MAX_HITS,total_hits=0,max_requests=MAX_REQUESTS,max_invocations=MAX_INVOCATIONS,
 started_tick_ms=startedTick,elapsed_ms=0,ignored_hits=0,read_bytes=total,
 request_count=0,invocation_count=0,completed_invocations=0,unbound_invocations=0,
 conclusion='unvalidated',stop_on_first_complete_chain=false,
 coverage='fixed-site observation window; not an all-writers trace or final playable-session proof'}
nioh3PossessedCapture=p
local owner=assert(loadfile(assert(NIOH3_BREAKPOINT_LIFECYCLE_PATH)))()({
 list=function() guard();return debug_getBreakpointList() end,
 remove=function(a) guard();return debug_removeBreakpoint(a) end,
 remove_id=function(id) guard();return debug_removeBreakpointByID(id) end,
 timer=createTimer,
 arm=function(a,f) guard();return debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,f) end,
},p)
local requests,generations={},{}
local deferred=false
local oldSwitch,switched=onOpenProcess,nil
local function cleanup()
 local result=owner.stop();if onOpenProcess==switched then onOpenProcess=oldSwitch end;return result
end
local function finish(reason)
 if not p.stop_reason then p.stop_reason=reason;p.stopped_elapsed_ms=elapsed() end
 p.elapsed_ms=elapsed();p.read_bytes=total;p.active=false;p.cleanup_pending=true
 if not deferred then deferred=true
  local ok=pcall(createTimer,1,function() deferred=false;cleanup() end)
  if not ok then deferred=false;cleanup() end
 end
end
function p.status() return {active=p.active,run_id=p.run_id,cleanup_pending=p.cleanup_pending,
 owned_breakpoints=p.owned_breakpoints,event_count=#p.events,stop_reason=p.stop_reason,
 read_bytes=total,ignored_hits=p.ignored_hits,total_hits=p.total_hits,elapsed_ms=elapsed(),
 request_count=p.request_count,invocation_count=p.invocation_count,completed_invocations=p.completed_invocations,
 unbound_invocations=p.unbound_invocations,error=p.error,conclusion=p.conclusion} end
function p.stop(reason) finish(reason or 'manual');return p.status() end
function p.retry_cleanup() return cleanup() end
function p.clear_captures()
 assert(not p.active and not p.cleanup_pending and #p.owned_breakpoints==0,'Stop and verify cleanup first')
 p.events={};p.event_sequence=0;requests={};generations={};p.conclusion='cleared'
 p.request_count=0;p.invocation_count=0;p.completed_invocations=0;p.unbound_invocations=0
end
local function emit(site,e)
 assert(#p.events<MAX_EVENTS,'Event bound');p.event_sequence=p.event_sequence+1
 e.sequence=p.event_sequence;e.site=site;e.rva=hex(sites[site].rva);e.thread_id=thread();e.thread_id_source=threadSource
 e.elapsed_ms=elapsed();e.read_bytes_total=total;p.elapsed_ms=e.elapsed_ms;p.read_bytes=total
 p.events[#p.events+1]=e
end
local function ignore() p.ignored_hits=p.ignored_hits+1 end
local returns={ [0xF1E4F1]='owned_scroll_branch', [0x21D9E24]='session_view_branch',
 [0x21DC438]='parameterized_session_branch', [0x2233DE8]='current_session_requeue' }
-- Observational snapshots, NOT native mode enums. Unavailable auxiliary state
-- is recorded, not defaulted to zero. Only request/ABI/output reads are required.
local function optional(f)
 local ok,v=pcall(f);if ok then return v end
 -- Never use optional-state handling to evade a hard budget or process guard.
 guard();assert(total<=MAX_READ,'Read budget exhausted')
 return {available=false,error=tostring(v):sub(1,512)}
end
local function session_view()
 local root=ptr(base+0x45C44A0);if root==0 then return {available=false,reason='null_root'} end
 local b=root+0x1010
 return {available=true,root=hex(root),view=hex(b),kind=uint(b,4),playthrough=uint(b+4,1),
  mission_type=uint(b+8,4),scroll_seed=uint(b+0x268,4),metadata=uint(b+0x26C,2),
  byte6=uint(b+0x26E,1),byte8_source=uint(b+0x26F,1)}
end
local function current_session()
 local root=ptr(base+0x474D480);if root==0 then return {available=false,reason='null_root'} end
 local s=ptr(root+8);if s==0 then return {available=false,root=hex(root),reason='null_object'} end
 return {available=true,root=hex(root),object=hex(s),state24=uint(s+0x24,4),mission_type=uint(s+0x28,4),
  forwarded_metadata30_not_captured=true,request_address=hex(s+0x38),request_hex=raw(s+0x38,12)}
end
local function queue_state()
 local a={}
 for i=0,3 do
  local n=base+0x45B83E0+i*0x58;local t=uint(n+4,4);local b=uint(n+8,1)
  local e={index=i,node=hex(n),mission_type=t,byte8=b,occupied=t~=0 or b~=0}
  if t==0xCC96 then e.request_hex=raw(n+0x20,12) end
  a[#a+1]=e
 end
 return a
end
local function snapshots(e)
 e.current_session=optional(current_session);e.session_view=optional(session_view);e.queue_snapshot=optional(queue_state)
end
local function add_request(e,phase,source,stack)
 assert(#requests<MAX_REQUESTS,'Request bound')
 local r={id=#requests+1,phase=phase,request=e.request_hex,source=source,enqueue_rsp=stack,
  producer_thread=e.thread_id,route=e.route}
 requests[#requests+1]=r;p.request_count=#requests;e.request_id=r.id;return r
end
local handlers={}
function handlers.request_enqueue()
 local req=bytes(RDX,12);if val(req,0,4)~=seed then ignore();return end
 local ret=ptr(RSP);local route=returns[ret-base]
 local e={request_address=hex(RDX),request_hex=hx(req),return_rva=relative(ret),
  caller_rbp=hex(RBP),caller_rsp=hex(RSP),route=route or 'unrecovered',
  padding_observed=hx({req[11],req[12]})}
 snapshots(e);emit('request_enqueue',e)
 if route=='owned_scroll_branch' or route=='session_view_branch' then assert(RDX==RBP-0x50,'Unexpected caller-local request identity')
 elseif route=='parameterized_session_branch' then assert(RDX==RSP+0x68,'Unexpected parameterized request identity')
 elseif route=='current_session_requeue' then assert(RDX==RBP-0x29,'Unexpected requeue request identity') end
 if route=='owned_scroll_branch' then assert(req[10]==0,'Literal-zero source contradicted')
 elseif route=='session_view_branch' or route=='parameterized_session_branch' then assert(req[10]==1,'Literal-one source contradicted') end
 if route=='session_view_branch' then
  local v=e.session_view;assert(v.available,'Session-view producer input unavailable')
  assert(v.mission_type==0xCC96 and v.scroll_seed==seed and v.metadata==val(req,4,2) and v.byte6==req[7]
   and v.playthrough==req[8] and (v.byte8_source~=0 and 1 or 0)==req[9],'Session-to-request projection mismatch')
 elseif route=='current_session_requeue' then
  local s=e.current_session;assert(s.available and s.mission_type==0xCC96 and s.request_hex==e.request_hex,'Requeue source copy mismatch')
 end
 for _,r in ipairs(requests) do assert(not(r.phase=='enqueued' and r.source==RDX and r.enqueue_rsp==RSP),'Ambiguous live enqueue frame reuse') end
 add_request(e,'enqueued',RDX,RSP)
end
function handlers.request_queued()
 -- At this PC R15 is still the source request, unlike later code that reuses it.
 local req=bytes(R15,12);if val(req,0,4)~=seed then ignore();return end
 local node=R12+RAX
 assert(R12==base+0x45B83E0 and RAX>=0 and RAX%0x58==0 and RAX//0x58<3,'Queue node identity mismatch')
 local e={request_hex=raw(node+0x20,12),source_hex=hx(req),source_address=hex(R15),
  request_address=hex(node+0x20),queue_node=hex(node),queue_index=RAX//0x58,queue_rsp=hex(RSP),
  mission_type=uint(node+4,4)}
 local matches={}
 for _,r in ipairs(requests) do if r.phase=='enqueued' and r.source==R15 and r.enqueue_rsp==RSP+0x658 then matches[#matches+1]=r end end
 assert(#matches<=1,'Ambiguous queue copy')
 if #matches==1 then e.request_id=matches[1].id;e.enqueue_link='frame_and_source'
 else e.enqueue_link='unobserved_enqueue' end
 snapshots(e);emit('request_queued',e)
 assert(e.request_hex==e.source_hex and e.mission_type==0xCC96,'Queue request changed')
 local r=matches[1]
 if r then
  assert(r.request==e.request_hex and r.producer_thread==e.thread_id,'Enqueue-to-copy mismatch')
  r.phase='queued'
 else r=add_request(e,'queued',R15,nil) end
 r.queue_node=node;r.queue_event=e.sequence
end
function handlers.mission_consume()
 local ret=ptr(RSP);if ret~=base+0x2237978 then ignore();return end
 assert(RCX==base+0x45B8400,'Mission request is not queue-head payload')
 local req=bytes(RCX,12);if val(req,0,4)~=seed then ignore();return end
 assert(p.invocation_count<MAX_INVOCATIONS,'Invocation bound')
 local matches={};for _,r in ipairs(requests) do if r.phase=='queued' and r.request==hx(req) then matches[#matches+1]=r end end
 -- Identical pending requests must NOT be assigned FIFO by guess. Observe the
 -- invocation independently, retain every candidate ID and stop linking them.
 local ids={};for _,r in ipairs(matches) do ids[#ids+1]=r.id end
 local e={invocation_id=p.invocation_count+1,request_hex=hx(req),request_address=hex(RCX),
  return_rva=relative(ret),consumer_rsp=hex(RSP),output_address=hex(RDX),queue_head=hex(base+0x45B83E0),
  matching_request_ids=ids,link_grade=#matches==1 and 'unique_pending_payload' or (#matches==0 and 'unobserved_or_changed_request' or 'ambiguous_pending_payload')}
 if #matches==1 then e.request_id=matches[1].id end
 snapshots(e);emit('mission_consume',e)
 assert(RDX==RSP+0x70,'Mission output/caller stack identity mismatch')
 for _,g in ipairs(generations) do assert(g.done or g.consumer_rsp~=RSP,'Overlapping generator frame reuse') end
 local g={id=e.invocation_id,request_id=e.request_id,consumer_rsp=RSP,thread=e.thread_id,
  output=RDX,request=e.request_hex,request_bytes=req,done=false}
 generations[#generations+1]=g;p.invocation_count=g.id
 if #matches==1 then matches[1].phase='consumed'
 else p.unbound_invocations=p.unbound_invocations+1
  for _,r in ipairs(matches) do r.phase='ambiguous_consumption' end
 end
end
local function descriptors(output)
 local header=bytes(output,0x34)
 local b,e,c=val(header,0,8),val(header,8,8),val(header,16,8)
 assert(e>=b and c>=e and (e-b)%0x28==0 and (c-b)%0x28==0,'Wave vector geometry')
 local n=(e-b)//0x28;assert(n<=8 and (b>0 or (e==0 and c==0)),'Wave count/null bound')
 local waves={};local count=0;local class0,class1=0,0
 for i=0,n-1 do
  local w=bytes(b+i*0x28,0x28);local db,de,dc=val(w,0,8),val(w,8,8),val(w,16,8)
  assert(de>=db and dc>=de and (de-db)%0x14==0 and (dc-db)%0x14==0,'Descriptor vector geometry')
  local size=(de-db)//0x14;assert(size<=96 and count+size<=96 and (db>0 or (de==0 and dc==0)),'Descriptor count/null bound')
  local ds={}
  for j=0,size-1 do
   local d=bytes(db+j*0x14,0x14);local cl=d[0x10+1]
   ds[#ds+1]={address=hex(db+j*0x14),raw_hex=hx(d),wave_index=i,position=j,
    spawn=val(d,0,4),lookup=val(d,4,4),point_key=d[0xE+1],source_flag=d[0xF+1],selector_class=cl}
   if cl==0 then class0=class0+1 elseif cl==1 then class1=class1+1 end
  end
  waves[#waves+1]={wave_index=i,address=hex(b+i*0x28),raw_hex=hx(w),descriptors=ds};count=count+size
 end
 return {address=hex(output),header_hex=hx(header),context_key=header[0x1E+1],terrain=header[0x1F+1],
  byte8_copy=header[0x24+1],metadata=val(header,0x28,4),playthrough=header[0x30+1],
  wave_count=n,descriptor_count=count,class0_count=class0,class1_count=class1,waves=waves,
  stage='post_20E198C_pre_1C244E4',persistent_task_join=false,physical_actor_join=false}
end
function handlers.mission_generated()
 local matches={}
 for _,g in ipairs(generations) do if not g.done and g.consumer_rsp+8==RSP then matches[#matches+1]=g end end
 if #matches==0 then ignore();return end
 assert(#matches==1,'Ambiguous generator return frame')
 local g=matches[1]
 local e={invocation_id=g.id,request_id=g.request_id,consumer_rsp=hex(g.consumer_rsp),return_rsp=hex(RSP),
  output_address=hex(g.output),request_hex=raw(base+0x45B8400,12),
  extra_from_request=g.request_bytes[10],extra_evidence='request_projection_static; not a new F+1648 observation',
  output=descriptors(g.output)}
 snapshots(e);emit('mission_generated',e)
 assert(e.thread_id==g.thread,'Generator callback thread token changed')
 assert(g.output==RSP+0x68 and e.request_hex==g.request,'Generator request/output identity changed')
 assert(e.output.playthrough==g.request_bytes[8] and e.output.metadata==val(g.request_bytes,4,2)
  and e.output.byte8_copy==g.request_bytes[9],'Completed output metadata projection mismatch')
 g.done=true;p.completed_invocations=p.completed_invocations+1
 p.conclusion='generation_returns_observed_not_session_finality'
 -- Deliberately DO NOT stop. A completed invocation is not the end of an entry.
end
local function callback(name)
 return function()
  local ok,err=xpcall(function()
   guard();p.total_hits=p.total_hits+1;assert(p.total_hits<=MAX_HITS,'Total callback hit budget')
   p.elapsed_ms=elapsed()
   if p.elapsed_ms>=WINDOW_MS then if p.active then finish('observation_window_elapsed') end;return end
   assert(RIP==sites[name].address,'Unexpected breakpoint PC')
   if p.active then handlers[name]() end
  end,debug.traceback)
  if not ok then p.error=tostring(err):sub(1,4096);finish('capture_error') end
  local continued,why=pcall(debug_continueFromBreakpoint,co_run)
  if not continued or why==false then p.error=('Resume unconfirmed: '..tostring(why)):sub(1,4096);finish('resume_error') end
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
for _,name in ipairs({'request_enqueue','request_queued','mission_consume','mission_generated'}) do
 local ok,err=owner.arm(sites[name].address,callback(name));if not ok then p.error=tostring(err):sub(1,4096);finish('arm_failed');error(p.error) end
end
local ok,err=pcall(createTimer,WINDOW_MS,function() if p.active then finish('observation_window_elapsed') end end)
if not ok then p.error=tostring(err):sub(1,4096);finish('timer_failed');error(p.error) end
return p.status()
