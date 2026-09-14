-- PC v2.01 bounded materializer frontier. No target writes or game calls.
local identity=assert(NIOH3_POSSESSED_TARGET_IDENTITY,'Use the verified Python runner')
assert(identity.executable_sha256=='4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159','Unapproved executable')
assert(identity.image_size==77814240 and type(identity.creation_filetime)=='string','Missing birth identity')
local seed=assert(NIOH3_ASSIGNMENT_SEED,'Explicit target seed required')
assert(seed>=0 and seed<=0xFFFFFFFF and seed%1==0,'Invalid seed')
assert(type(NIOH3_POSSESSED_RUN_ID)=='string' and #NIOH3_POSSESSED_RUN_ID>0 and #NIOH3_POSSESSED_RUN_ID<=128,'Run ID bound')
local sites={
-- SIGNATURES_BEGIN
 materialize_enter={rva=0x1c244e4,hex='48895C24205556574154415541564157488DAC24E0FEFFFF'},
 after_prepass={rva=0x1c245d8,hex='488B074D8B6F08498B374C896C2450'},
 task_lookup={rva=0x1c24614,hex='4C8BF04885C07546418A471F488D4DB08B17'},
 task_link={rva=0x1c24662,hex='41C686F400000001498D9EFC00000041C686E500000001'},
-- SIGNATURES_END
}
local MAX_EVENTS,MAX_INVOCATIONS,MAX_HITS,MAX_READ=256,16,4096,4*1024*1024
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
  if type(_G[name])=='function' then local ok,v=pcall(_G[name]);if ok and type(v)=='number' and v>0 then
   threadSource=(name:sub(1,6)=='debug_' and 'debug_event_api:' or 'callback_api_unverified:')..name;return v
  end end
 end;threadSource='unavailable';return nil
end
for name,s in pairs(sites) do s.address=base+s.rva;assert(raw(s.address,#s.hex/2)==s.hex,'v2.01 signature mismatch: '..name) end
local inventory=debug_getBreakpointList()
assert((type(inventory)=='table' and next(inventory)==nil) or (inventory==nil and not debug_isDebugging()),'Foreign or unknown breakpoint inventory')
local p={schema='nioh3.materialization-frontier.v1',run_id=NIOH3_POSSESSED_RUN_ID,
 pid=pid,module_base=hex(base),identity=identity,target_seed=seed,
 mode_label=NIOH3_MATERIALIZATION_MODE_LABEL,read_only=true,writes_game_memory=false,calls_game_functions=false,
 active=false,events={},event_sequence=0,max_events=MAX_EVENTS,max_seconds=120,
 max_hits=MAX_HITS,total_hits=0,max_invocations=MAX_INVOCATIONS,
 started_tick_ms=startedTick,elapsed_ms=0,ignored_hits=0,read_bytes=total,
 invocation_count=0,linked_count=0,
 conclusion='unvalidated',stop_on_first_complete_chain=false,
 coverage='materializer entry/prepass/lookup/link; no whole-manager or actor enumeration; no general factory coverage'}
nioh3PossessedCapture=p
local owner=assert(loadfile(assert(NIOH3_BREAKPOINT_LIFECYCLE_PATH)))()({
 list=function() guard();return debug_getBreakpointList() end,
 remove=function(a) guard();return debug_removeBreakpoint(a) end,
 remove_id=function(id) guard();return debug_removeBreakpointByID(id) end,
 timer=createTimer,
 arm=function(a,f) guard();return debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,f) end,
},p)
local invocations={}
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
 invocation_count=p.invocation_count,linked_count=p.linked_count,error=p.error,conclusion=p.conclusion} end
function p.stop(reason) finish(reason or 'manual');return p.status() end
function p.retry_cleanup() return cleanup() end
function p.clear_captures()
 assert(not p.active and not p.cleanup_pending and #p.owned_breakpoints==0,'Stop and verify cleanup first')
 p.events={};p.event_sequence=0;invocations={};p.conclusion='cleared'
 p.invocation_count=0;p.linked_count=0
end
local function emit(site,e)
 assert(#p.events<MAX_EVENTS,'Event bound');p.event_sequence=p.event_sequence+1
 e.sequence=p.event_sequence;e.site=site;e.rva=hex(sites[site].rva);e.thread_id=thread();e.thread_id_source=threadSource
 e.elapsed_ms=elapsed();e.read_bytes_total=total;p.elapsed_ms=e.elapsed_ms;p.read_bytes=total
 p.events[#p.events+1]=e
end
local function ignore() p.ignored_hits=p.ignored_hits+1 end
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
  control24=header[0x24+1],metadata=val(header,0x28,4),playthrough=header[0x30+1],
  wave_count=n,descriptor_count=count,class0_count=class0,class1_count=class1,waves=waves,
  stage='at_event_boundary',persistent_task_join=false,physical_actor_join=false}
end

local function task_fields(address)
 local identityBytes=bytes(address+0x20,12)
 local embedded=bytes(address+0x80,0x17)
 return {address=hex(address),identity_hex=hx(identityBytes),
  spawn=val(identityBytes,0,4),mission=val(identityBytes,4,4),lookup=val(identityBytes,8,4),
  descriptor_hex=hx({table.unpack(embedded,1,20)}),terrain=embedded[21],index95=embedded[22],value96=embedded[23],
  flags_e5_f5_hex=raw(address+0xE5,0x11),kind14c=uint(address+0x14C,4),
  physical_actor_join=false,whole_manager_enumeration=false}
end
local function flatten(output)
 local result={}
 for _,w in ipairs(output.waves) do for _,d in ipairs(w.descriptors) do result[#result+1]=d end end
 return result
end
local function invocation()
 local entry=RSP+0x258;local found=nil
 for i=#invocations,1,-1 do local g=invocations[i]
  if g.entry_rsp==entry and not g.superseded then found=g;break end
 end
 if not found then ignore();return nil end
 assert(R15==found.output,'Output pointer changed within materializer frame')
 assert(RBP==found.entry_rsp-0x158,'Materializer RBP/RSP relation changed')
 return found
end
local handlers={}
function handlers.materialize_enter()
 assert(#invocations<MAX_INVOCATIONS,'Invocation bound')
 local ret=ptr(RSP)
 if ret==base+0x2237994 then assert(RDX==RSP+0x70,'Known mission caller output ABI') end
 local g={id=#invocations+1,entry_rsp=RSP,wrapper=RCX,root=ptr(RCX),output=RDX,mission_parameter=R8 & 0xFFFFFFFF,lookups=0,links=0}
 g.manager=ptr(g.root+0x48)
 local e={invocation_id=g.id,entry_rsp=hex(RSP),wrapper=hex(RCX),root=hex(g.root),manager=hex(g.manager),
  output_address=hex(RDX),return_rva=relative(ret),return_address=hex(ret),
  caller_grade=ret==base+0x2237994 and 'known_mission_materializer_call' or 'unbound_caller',
  mission_parameter=R8 & 0xFFFFFFFF,output=descriptors(RDX),queue_context_hex=raw(base+0x45B8400,12),
  seed_binding='queue snapshot only; no same-run generator-request join',
  control2bf4=uint(g.root+0x2BF4,1),old_gate2c18=uint(g.root+0x2C18,1)}
 for _,prev in ipairs(invocations) do
  if not prev.superseded and prev.entry_rsp==RSP then
   prev.superseded=true;e.previous_frame_invocation_id=prev.id
   e.previous_frame_complete=prev.post~=nil and prev.links==#prev.post
  end
 end
 invocations[#invocations+1]=g;p.invocation_count=#invocations;emit('materialize_enter',e)
end
function handlers.after_prepass()
 local g=invocation();if not g then return end
 assert(RDI==g.wrapper,'Wrapper identity at prepass boundary')
 assert(not g.post,'Duplicate after-prepass event')
 assert(ptr(g.wrapper)==g.root and ptr(g.root+0x48)==g.manager,'Manager identity changed')
 local output=descriptors(g.output);g.post=flatten(output);g.post_terrain=output.terrain
 emit('after_prepass',{invocation_id=g.id,entry_rsp=hex(g.entry_rsp),output=output,
  gate2c18=uint(g.root+0x2C18,1),helper_span='1C244E4..1C245D8; reset 1BF6EC0, lookup 377B18, E34620, E39D40 twice',
  attribution='span only, not proof that E39D40 alone is the writer'})
end
function handlers.task_lookup()
 local g=invocation();if not g then return end
 assert(g.post,'Lookup without observed prepass boundary')
 assert(RBX==g.manager,'Lookup manager differs from entry')
 local nextIndex=g.lookups+1;assert(nextIndex<=#g.post,'More loop visits than post-prepass descriptors')
 local source=g.post[nextIndex]
 assert(RDI==tonumber(source.address),'Unexpected descriptor pointer/order')
 local current=raw(RDI,0x14)
 local e={invocation_id=g.id,entry_rsp=hex(g.entry_rsp),lookup_ordinal=nextIndex,
  source=source,source_current_hex=current,manager=hex(RBX),lookup_result=hex(RAX),
  branch=RAX==0 and 'new_path' or 'reuse_path'}
 if RAX~=0 then e.existing_task=task_fields(RAX) end
 emit('task_lookup',e)
 assert(current==source.raw_hex,'Descriptor changed after prepass snapshot')
 g.lookups=nextIndex;g.pending={ordinal=nextIndex,source=source,returned=RAX}
end
function handlers.task_link()
 local g=invocation();if not g then return end
 local pending=assert(g.pending,'Task link without the matching lookup')
 assert(RDI==tonumber(pending.source.address) and RBX==g.manager,'Task-link source/manager mismatch')
 assert(type(R14)=='number' and R14>0,'Null task at common link point')
 if pending.returned~=0 then assert(R14==pending.returned,'Reuse returned a different pointer') end
 local task=task_fields(R14)
 local e={invocation_id=g.id,entry_rsp=hex(g.entry_rsp),lookup_ordinal=pending.ordinal,
  source=pending.source,source_current_hex=raw(RDI,0x14),task=task,
  branch=pending.returned==0 and 'new_path' or 'reuse_path',
  identity_matches_source=task.spawn==pending.source.spawn and task.mission==g.mission_parameter and task.lookup==pending.source.lookup,
  terrain_matches_source=task.terrain==g.post_terrain,
  descriptor_matches_source=task.descriptor_hex==pending.source.raw_hex,
  task_stage='before common F4/E5 writes; task pointer observed, not physical actor or entire manager',
  manager=hex(RBX)}
 emit('task_link',e)
 assert(e.source_current_hex==pending.source.raw_hex,'Source changed during task creation/reuse')
 -- Reuse mismatches are the question under investigation: retain, DO NOT repair.
 g.links=g.links+1;p.linked_count=p.linked_count+1;g.pending=nil
 p.conclusion='materializer_frontier_observed_not_global_task_or_actor_finality'
 -- Deliberately remain armed: another materializer invocation may occur later.
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
if not debug_isDebugging() then debugProcess(2) end
assert(debug_isDebugging(),'Debugger attachment failed')
p.active=true
for _,name in ipairs({'materialize_enter','after_prepass','task_lookup','task_link'}) do
 local ok,err=owner.arm(sites[name].address,callback(name));if not ok then p.error=tostring(err):sub(1,4096);finish('arm_failed');error(p.error) end
end
local ok,err=pcall(createTimer,WINDOW_MS,function() if p.active then finish('observation_window_elapsed') end end)
if not ok then p.error=tostring(err):sub(1,4096);finish('timer_failed');error(p.error) end
return p.status()
