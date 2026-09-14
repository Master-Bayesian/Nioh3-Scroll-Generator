-- PC v2.01 research-only causal capture. Four hardware execute breakpoints.
-- Never calls a game function or writes target memory. Run through the pinned runner.
local identity = assert(NIOH3_POSSESSED_TARGET_IDENTITY, 'Run the verified Python runner first')
assert(identity.executable_sha256 == '4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159', 'Unapproved executable')
assert(identity.image_size == 77814240 and type(identity.creation_filetime) == 'string', 'Missing process-birth attestation')
local expectedSeed = assert(NIOH3_ASSIGNMENT_SEED, 'Explicit target seed required')
assert(expectedSeed >= 0 and expectedSeed <= 0xFFFFFFFF, 'Invalid seed')
local sites = {
  origin_entry={rva=0x10283C0, hex='48895C240848896C241048897424185741544155415641574883EC50488B01440FB6FA0F297424400F297C2430440F294424204C8B304C8B6008'},
  trial_decision={rva=0x1028570, hex='3BC77E50488B3D75D85803'},
  task_copy={rva=0x1BF2D54, hex='0F1187800000008B4310898790000000'},
  task_linked={rva=0x1C24662, hex='41C686F400000001498D9EFC00000041C686E500000001'},
}
local base=assert(getAddressSafe('Nioh3.exe'))
local pid=assert(getOpenedProcessID())
local handle=assert(getOpenedProcessHandle(), 'No CE target handle')
assert(pid==identity.process_id,'Wrong attached PID')
assert(not nioh3PossessedCapture or (not nioh3PossessedCapture.active and not nioh3PossessedCapture.cleanup_pending), 'Previous capture still owns resources')
local function hex(x) return string.format('0x%X',x) end
local function sameProcess()
  return getOpenedProcessID()==pid and getOpenedProcessHandle()==handle and getAddressSafe('Nioh3.exe')==base
end
local function guard() assert(sameProcess(),'Target handle/PID/module changed; no retargeting allowed') end
local readBytesTotal=0
local function bytes(a,n)
  assert(type(a)=='number' and a>0 and type(n)=='number' and n>=0 and n<=65536,
    'Invalid bounded read address='..tostring(a)..' address_type='..type(a)..' size='..tostring(n))
  readBytesTotal=readBytesTotal+n
  assert(readBytesTotal<=8*1024*1024,'Read-byte budget exhausted')
  local b=readBytes(a,n,true)
  assert(type(b)=='table' and #b==n,'Unreadable '..hex(a))
  return b
end
local function readN(a,n)
  local b=bytes(a,n);local v=0
  for i=n,1,-1 do v=(v<<8)|b[i] end
  return v
end
local function u8(a) return readN(a,1) end
local function u16(a) return readN(a,2) end
local function u32(a) return readN(a,4)&0xFFFFFFFF end
local function ptr(a,nullable)
  local p=readN(a,8); assert(nullable or p~=0,'Null pointer at '..hex(a));return p
end
local function raw(a,n)
  local b=bytes(a,n);for i=1,#b do b[i]=string.format('%02X',b[i]) end;return table.concat(b)
end
local function s32(v) v=v&0xFFFFFFFF;if v>=0x80000000 then return v-0x100000000 end;return v end
local function tid()
  for _,name in ipairs({'getCurrentThreadId','getCurrentThreadID'}) do
    if type(_G[name])=='function' then local ok,v=pcall(_G[name]);if ok and type(v)=='number' and v>0 and v%1==0 then return v end end
  end
  error('Debug event thread ID unavailable; refusing RNG route inference')
end
for name,s in pairs(sites) do
  s.address=base+s.rva
  assert(raw(s.address,#s.hex/2)==s.hex,'v2.01 signature mismatch: '..name)
end
local existing=debug_getBreakpointList()
assert((type(existing)=='table' and next(existing)==nil) or (existing==nil and not debug_isDebugging()),'Foreign or unknown breakpoint inventory')
assert(type(NIOH3_POSSESSED_RUN_ID)=='string' and #NIOH3_POSSESSED_RUN_ID>=1 and #NIOH3_POSSESSED_RUN_ID<=512,'Run ID length cap')
local p={schema='nioh3.assignment-origin.v1',run_id=assert(NIOH3_POSSESSED_RUN_ID),
  active=false,read_only=true,writes_game_memory=false,events={},event_sequence=0,
  max_events=256,max_seconds=120,pid=pid,module_base=hex(base),identity=identity,
  target_seed=expectedSeed,conclusion='unvalidated',ignored_hits=0}
nioh3PossessedCapture=p
local owner=assert(loadfile(assert(NIOH3_BREAKPOINT_LIFECYCLE_PATH)))()({
  list=function() guard();return debug_getBreakpointList() end,
  remove=function(a) guard();return debug_removeBreakpoint(a) end,
  remove_id=function(id) guard();return debug_removeBreakpointByID(id) end,
  timer=createTimer,
  arm=function(a,fn) guard();return debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,fn) end,
},p)
local live=nil
local deferred=false
local function finish(reason)
  p.active=false;p.stop_reason=p.stop_reason or reason
  -- Removal on CE GUI timer, not while modifying the stopped debug event.
  p.cleanup_pending=true
  if not deferred then
    deferred=true
    local ok=pcall(createTimer,1,function() deferred=false;owner.stop() end)
    if not ok then deferred=false;owner.stop() end
  end
end
function p.stop(reason) finish(reason or 'manual');return {cleanup_pending=p.cleanup_pending} end
function p.retry_cleanup() return owner.stop() end
function p.clear_captures()
  assert(not p.active and not p.cleanup_pending and #p.owned_breakpoints==0,'Stop/verify cleanup first')
  p.events={};p.event_sequence=0
end
function p.status() return {active=p.active,run_id=p.run_id,cleanup_pending=p.cleanup_pending,
  owned_breakpoints=p.owned_breakpoints,event_count=#p.events,stop_reason=p.stop_reason,
  read_bytes=readBytesTotal,conclusion=p.conclusion,error=p.error} end
local function emit(site,e)
  assert(#p.events<p.max_events,'Event budget exhausted')
  p.event_sequence=p.event_sequence+1;e.sequence=p.event_sequence;e.site=site
  e.rva=hex(sites[site].rva);e.thread_id=tid();e.read_bytes_total=readBytesTotal
  p.events[#p.events+1]=e
end
local function route()
  local thread=tid();local mgr=ptr(base+0x47558C0,true)
  local primaryOwner=mgr~=0 and u32(mgr+0x528) or nil
  local secondaryOwner=u32(base+0x45BFA10)
  local primaryPointer=ptr(base+0x44BFB88,true)
  local secondaryPointer=ptr(base+0x44BFB90,true)
  local addr,which
  if mgr~=0 and primaryOwner==thread then assert(primaryPointer~=0,'Null primary scoped RNG');addr=primaryPointer;which='primary_scoped'
  elseif secondaryOwner==thread then assert(secondaryPointer~=0,'Null secondary scoped RNG');addr=secondaryPointer;which='secondary_scoped'
  else addr=base+0x44BFB84;which='fallback_global' end
  return {route=which,address=hex(addr),state=u32(addr),current_thread=thread,
    manager=mgr~=0 and hex(mgr) or nil,primary_owner_thread=primaryOwner,
    secondary_owner_thread=secondaryOwner,primary_pointer=primaryPointer~=0 and hex(primaryPointer) or nil,
    secondary_pointer=secondaryPointer~=0 and hex(secondaryPointer) or nil},addr
end
-- Only live entries requested by descriptor keys are read. Scanning a bounded
-- hash array is order-independent and avoids calling any native lookup helper.
local function index(ctx,width,limit)
  local store=ptr(ctx);local count=u32(store+4);assert(count<=limit,'Table row cap')
  local h=ptr(ctx+0x20);local b,e=ptr(h+8),ptr(h+0x10)
  local n=(e-b)/8;assert(n>=0 and n%1==0 and n<=limit*2,'Hash cap/stride')
  local map={};local block=bytes(b,n*8)
  local function v(off,len) local z=0;for i=len,1,-1 do z=(z<<8)|block[off+i] end;return z end
  local sentinel=readN(h+4,width)
  for k=0,n-1 do
    local key,row=v(k*8,width),v(k*8+4,4)
    if key~=sentinel and row<count then assert(map[key]==nil,'Duplicate live table key');map[key]=row end
  end
  return {ctx=ctx,store=store,count=count,hash=h,begin=b,finish=e,rows=map}
end
local function row(tableIndex,key,stride)
  local i=tableIndex.rows[key];if i==nil then return nil end
  return tableIndex.store+8+i*stride,i
end
local function descriptor(a,w,o)
  return {address=hex(a),wave_index=w,position=o,raw_hex=raw(a,0x14),
    spawn=u32(a),lookup=u32(a+4),kind=u8(a+0xE),flag=u8(a+0xF),selector_class=u8(a+0x10)}
end
local function snapshotPool(waves)
  local b,e=ptr(waves,true),ptr(waves+8,true);assert(e>=b and (e-b)%0x28==0 and (e-b)/0x28<=32,'Wave vector bounds')
  local list={};local byAddress={}
  for w=0,(e-b)/0x28-1 do
    local wa=b+w*0x28;local a,z=ptr(wa,true),ptr(wa+8,true)
    assert(z>=a and (z-a)%0x14==0,'Descriptor vector stride')
    for j=0,(z-a)/0x14-1 do
      assert(#list<96,'Descriptor cap');local d=descriptor(a+j*0x14,w,j)
      assert(d.spawn>=0xF3C and d.spawn<=0xF96,'Unexpected generated spawn range')
      list[#list+1]=d;byAddress[d.address]=d
    end
  end
  return list,byAddress
end
local function tableEvidence(list,contextKey)
  local m=ptr(base+0x45B5DF0);local selector=u8(m+0xB0A)
  local et=index(ptr(m+(selector~=0 and 0x40 or 0x38)),4,4096)
  local st=index(ptr(m+0x118),2,4096)
  local ct=index(ptr(m+0x230),4,4096)
  local xt=index(ptr(m+0xA98),1,4096)
  local config,ci=row(ct,0x4543,0x20)
  local context,xi=row(xt,contextKey,0x30)
  assert(context~=nil,'Generator context key absent from live special-context table')
  local result={parameter_manager=hex(m),enemy_table_selector=selector,
    enemy_context=hex(et.ctx),subtype_context=hex(st.ctx),config_context=hex(ct.ctx),
    special_context=hex(xt.ctx),
    config_4543={present=config~=nil,row_index=ci,raw_hex=config and raw(config,0x20) or nil},
    generator_context={key=contextKey,row_index=xi,address=hex(context)}}
  for _,d in ipairs(list) do
    local er,ei=row(et,d.lookup,0x398)
    d.eligibility={enemy_row_present=er~=nil,enemy_row_index=ei}
    if er then
      local key=u16(er+0xA8);local sr,si=row(st,key,0x54)
      d.eligibility.enemy_row_address=hex(er);d.eligibility.subtype_key=key
      d.eligibility.enemy_weight244=u32(er+0x244)
      d.eligibility.subtype_row_present=sr~=nil;d.eligibility.subtype_row_index=si
      if sr then d.eligibility.subtype_raw_hex=raw(sr,0x54);d.eligibility.flags14=u8(sr+0x14) end
    end
  end
  return result,{address=hex(context),raw_hex=raw(context,0x30),path=u8(context+0x29),
    configured_counts={u8(context+0x2A),u8(context+0x2B),u8(context+0x2C),u8(context+0x2D),u8(context+0x2E)}}
end
local function originEntry()
  local ret=ptr(RSP)
  if ret~=base+0x102C898 and ret~=base+0x102C8AF then p.ignored_hits=p.ignored_hits+1;return end
  -- These are saved nonvolatile registers/returns in the *validated* parent
  -- prologs, not arbitrary stack searches. Reject UI/preview invocations.
  local frame=RBP
  if ptr(frame+0x1618)~=base+0x20E1943 or ptr(frame+0x16C8)~=base+0x20E19C2 or ptr(frame+0x1778)~=base+0x2237978 then
    p.ignored_hits=p.ignored_hits+1;return
  end
  local seed=u32(frame+0x1608)
  if seed~=expectedSeed then p.ignored_hits=p.ignored_hits+1;return end
  local waves=ptr(RCX);local selector=RDX&0xFF
  if live then assert(live.waves==waves and live.frame==frame and selector==1 and live.selector==0,'Second transaction or invalid pass order') end
  local list,byAddress=snapshotPool(waves);local rng,addr=route()
  local contextKey=u8(frame+0x1640)
  -- The generator reuses [rsp+0x48] later in the function, so by this call it
  -- no longer reliably contains the selected context-row pointer. Resolve the
  -- authored row by the already-validated context key in the native table.
  local tables,contextRow=tableEvidence(list,contextKey)
  local e={seed=seed,selector=selector,source_wrapper=hex(RCX),wave_vector=hex(waves),
    parent_frame=hex(frame),rng=rng,allow_class1=u8(frame+0x1648),
    parent_rng={address=hex(frame+0xC0),state=u32(frame+0xC0)},
    rng_scope_matches_parent=addr==frame+0xC0,
    context_key=contextKey,terrain=u8(waves+0x1F),
    global_request_hex=raw(base+0x45B8400,0xC),
    generator_context_row=contextRow,
    parent_returns={'0x20E1943','0x20E19C2','0x2237978'},descriptors=list}
  e.tables=tables
  if not live then
    live={waves=waves,frame=frame,byAddress=byAddress,source_count=#list,linked={},copied={},trials={},
      seed=seed,pass_rsp=RSP,thread=tid(),selector=selector,table_info=e.tables,
      rng_address=addr,rng_scope_matches_parent=e.rng_scope_matches_parent}
  else
    assert(addr==live.rng_address,'RNG route changed between selector passes')
    for _,d in ipairs(list) do assert(d.flag==0,'Class-1 fallback entered after a preexisting flag') end
    live.selector=selector;live.pass_rsp=RSP;live.table_info=e.tables
  end
  assert(#list>0,'No descriptors in mission origin pass')
  for _,d in ipairs(list) do assert(d.flag==0,'Flag already set before recovered source: earlier writer exists') end
  emit('origin_entry',e)
end
local function decision()
  if not live or tid()~=live.thread then p.ignored_hits=p.ignored_hits+1;return end
  -- helper's 5 pushes + 0x50 frame = 0x78 bytes
  assert(RSP+0x78==live.pass_rsp,'Decision in unrelated invocation')
  local key=hex(RSI);local d=assert(live.byAddress[key],'Descriptor is not from the observed wave vector')
  local rng,addr=route();assert(addr==live.rng_address,'RNG owner changed')
  local ticket,threshold=s32(RAX),s32(RDI)
  local e={seed=live.seed,selector=R15&0xFF,descriptor=descriptor(RSI,d.wave_index,d.position),
    rng=rng,ticket=ticket,threshold=threshold,branch_will_set=ticket<=threshold,
    parent_rng={address=hex(live.frame+0xC0),state=u32(live.frame+0xC0)},
    source_flag_before=u8(RSI+0xF),source_return_rva=hex(ptr(live.pass_rsp)-base)}
  live.trials[#live.trials+1]=e
  assert(#live.trials<=96,'Trial cap')
  emit('trial_decision',e)
end
local function taskCopy()
  if not live or tid()~=live.thread or live.byAddress[hex(RBX)]==nil then p.ignored_hits=p.ignored_hits+1;return end
  -- constructor pushes rdi and subtracts 0x20 => return is rsp+0x28.
  assert(ptr(RSP+0x28)==base+0x1C24635,'Unexpected constructor caller')
  local d=live.byAddress[hex(RBX)]
  assert(u32(RDI+0x20)==d.spawn and u32(RDI+0x24)==0xCC96,'Task identity mismatch')
  assert(ptr(RDI,true)==0 and ptr(RDI+0x18,true)==0,'Constructor did not initialize null sources')
  local e={seed=live.seed,source=descriptor(RBX,d.wave_index,d.position),temporary_task=hex(RDI),
    destination_80_before_hex=raw(RDI+0x80,0x14),task_identity_hex=raw(RDI+0x20,0xC),
    return_rva='0x1C24635',note='breakpoint precedes the 16-byte store; destination is not a post-write observation'}
  live.copied[hex(RBX)]=e;emit('task_copy',e)
end
local function linked()
  if not live or tid()~=live.thread or live.byAddress[hex(RDI)]==nil then p.ignored_hits=p.ignored_hits+1;return end
  local key=hex(RDI);assert(not live.linked[key],'Duplicate task link')
  local d=live.byAddress[key];local rec=R14
  assert(rec~=0 and u32(rec+0x20)==d.spawn and u32(rec+0x28)==d.lookup and u32(rec+0x24)==0xCC96,'Persistent task identity mismatch')
  local root=ptr(ptr(base+0x45B5E00));local mgr=ptr(root+0x48)
  local v=ptr(mgr);local count=u32(mgr+8);assert(count<=1024,'Manager vector cap')
  local membership=false
  for i=0,count-1 do if ptr(v+i*0x10+8,true)==rec then membership=true;break end end
  local e={seed=live.seed,source=descriptor(RDI,d.wave_index,d.position),persistent_task=hex(rec),
    task_identity_hex=raw(rec+0x20,0xC),descriptor_hex=raw(rec+0x80,0x14),
    source0_null=ptr(rec,true)==0,source18_null=ptr(rec+0x18,true)==0,
    flag8f=u8(rec+0x8F),flag_e9=u8(rec+0xE9),flag_ea=u8(rec+0xEA),
    manager=hex(mgr),vector_count=count,manager_membership=membership,
    copy_seen=live.copied[key]~=nil}
  emit('task_linked',e)
  assert(membership,'Object not in actual task manager')
  assert(e.copy_seen,'Existing task reused; not a fresh provenance validation')
  assert(e.descriptor_hex==e.source.raw_hex,'Source descriptor differs from persistent embedded descriptor')
  live.linked[key]=true
  local n=0;for _ in pairs(live.linked) do n=n+1 end
  if n==live.source_count then p.conclusion='capture_complete_requires_offline_causal_validation';finish('all_generated_tasks_linked') end
end
local callbacks={origin_entry=originEntry,trial_decision=decision,task_copy=taskCopy,task_linked=linked}
local function callback()
  if not p.active then debug_continueFromBreakpoint(co_run);return 1 end
  local ok,err=pcall(function()
    guard();local matched=false
    for name,s in pairs(sites) do if RIP==s.address then matched=true;callbacks[name]();break end end
    assert(matched,'Unexpected breakpoint PC')
  end)
  if not ok then p.error=tostring(err):sub(1,1024);p.conclusion='rejected';finish('capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Debugger attachment failed')
for _,name in ipairs({'origin_entry','trial_decision','task_copy','task_linked'}) do
  local ok,err=owner.arm(sites[name].address,callback)
  if not ok then p.error=tostring(err):sub(1,1024);finish('arm_failed');error(p.error) end
end
p.active=true
local ok,err=pcall(createTimer,p.max_seconds*1000,function() if p.active then finish('time_budget') end end)
if not ok then p.error=tostring(err):sub(1,1024);finish('budget_timer_failed');error(p.error) end
return p.status()
