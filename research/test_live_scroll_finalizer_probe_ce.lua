-- Closed mock harness. No real debugger, target memory, or OS process API exists here.
local root=NIOH3_RESEARCH_TEST_ROOT or 'F:/Nioh3_ScrollEditor/research/'
local BASE,ENTRY,GLOBAL=0x10000000,0x1227DCD0,0x1474D4E0
local SOURCE,OUTPUT,STACK,CALLER,MANAGER,DATA,TEB=0x20000000,0x21000000,0x22000000,0x23000000,0x24000000,0x25000000,0x26000000
local signature={0x40,0x53,0x55,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x81,0xEC,0x78,0x01,0x00,0x00}
local function zeroes(count) local out={} for i=1,count do out[i]=0 end return out end
local function copy(value) local out={} for i,v in ipairs(value) do out[i]=v end return out end
local function scenario(options)
  options=options or {}
  local state={sites={},next_id=1,timers={},removals={},arms={},mode='immediate',continues=0,
    manager=MANAGER,data=DATA,pid=123,tid=456,caller=CALLER,serial=0x34,read_counts={}}
  state.source=zeroes(232);state.source[1]=4;state.source[2]=0xE6
  state.source[0x28+1]=state.serial;state.source[0x30+1]=4
  state.output=copy(state.source)
  local env={assert=assert,error=error,ipairs=ipairs,pairs=pairs,pcall=pcall,type=type,tonumber=tonumber,
    tostring=tostring,math=math,string=string,table=table,
    bptExecute=0,bpmDebugRegister=1,co_run=0,
    NIOH3_BREAKPOINT_LIFECYCLE_PATH=root..'owned_breakpoint_lifecycle_ce.lua',
    NIOH3_FINALIZER_OPTIONS=options.config or {serial_le_hex='3400000000000000'}}
  env.loadfile=function(path,mode,scope)
    assert(path==root..'owned_breakpoint_lifecycle_ce.lua','Unexpected helper path')
    return loadfile(path,mode,scope)
  end
  state.env=env
  env.getAddressSafe=function() return BASE end
  env.getOpenedProcessID=function() return state.pid end
  env.readBytes=function(address,count,asTable)
    assert(asTable==true,'Unexpected read mode')
    state.read_counts[address]=(state.read_counts[address] or 0)+1
    if address==state.fail_read then return nil end
    if address==state.partial_read then return {0} end
    local bytes
    if address==ENTRY then bytes=copy(signature);if options.bad_signature then bytes[1]=0 end
    elseif address==SOURCE then bytes=copy(state.source)
    elseif address==OUTPUT then bytes=copy(state.output)
    else bytes=zeroes(count) end
    if state.unstable_read==address and state.read_counts[address]%2==0 then bytes[1]=(bytes[1]+1)%256 end
    if address==DATA+8 and state.owner_changes_during_read then state.manager=MANAGER+0x100 end
    return bytes
  end
  env.readQword=function(address)
    if address==GLOBAL then return state.manager end
    if address==state.manager then return state.data end
    if address==STACK then return state.caller end
    if address==TEB+0x30 then return TEB end
    if address==TEB+0x40 then return state.teb_pid or state.pid end
    if address==TEB+0x48 then return state.tid end
    if address==TEB+8 then return STACK+0x10000 end
    if address==TEB+0x10 then return STACK-0x10000 end
    error('Unmapped mock pointer')
  end
  env.debug_isDebugging=function() return true end
  env.debug_getContext=function() end
  env.debug_continueFromBreakpoint=function() state.continues=state.continues+1 end
  env.debug_getBreakpointList=function()
    if state.list_failure then error('Injected list failure') end
    local out={} for address in pairs(state.sites) do out[#out+1]=address end return out
  end
  env.debug_setBreakpoint=function(address,size,trigger,method,callback)
    assert(size==1 and trigger==0 and method==1,'Only hardware execution sites are permitted')
    assert(not state.sites[address],'Foreign or retained site must not be overwritten')
    local id=state.next_id;state.next_id=id+1
    state.sites[address]={id=id,callback=callback};state.arms[#state.arms+1]=address
    if state.arm_fail==address then return false,id end
    return true,id
  end
  local function remove(address)
    state.removals[#state.removals+1]=address
    if state.mode=='failure' then error('Injected remove failure') end
    if state.mode=='deferred' then return end
    state.sites[address]=nil
  end
  env.debug_removeBreakpoint=remove
  env.debug_removeBreakpointByID=function(id)
    for address,site in pairs(state.sites) do if site.id==id then remove(address);return end end
  end
  env.createTimer=function(delay,callback)
    if options.timer_failure then error('Injected timer failure') end
    state.timers[#state.timers+1]={delay=delay,callback=callback};return {}
  end
  function state.fire(delay)
    for i,timer in ipairs(state.timers) do
      if timer.delay==delay then table.remove(state.timers,i);timer.callback();return true end
    end
    return false
  end
  function state.entry(slot,reveal)
    env.RCX,env.RDX,env.RSP=OUTPUT,SOURCE,STACK
    env.R8,env.R9=slot or 2,reveal or 1
    assert(state.sites[ENTRY],'Entry breakpoint is absent').callback()
  end
  function state.finish(sp,rax)
    env.RSP,env.RAX=sp or STACK+8,rax or OUTPUT
    assert(state.sites[state.caller],'Return breakpoint is absent').callback()
  end
  if options.foreign then state.sites[options.foreign]={id=9999} end
  local chunk=assert(loadfile(root..'capture_live_scroll_finalizer_ce.lua','t',env))
  state.loaded,state.result=pcall(chunk)
  state.probe=env.nioh3ScrollFinalizerProbe
  return state
end
local passed={}
local function test(name,body)
  local ok,failure=pcall(body)
  if not ok then error(name..': '..tostring(failure)) end
  passed[#passed+1]=name
end

test('Exact ABI captures source/output before and paired return without target calls',function()
  local s=scenario();assert(s.loaded and s.probe.active)
  s.entry(2,0xFF000001);s.output[0x42+2*24+1]=4;s.finish()
  local a,b=s.probe.events[1],s.probe.events[2]
  assert(a.effect_index==2 and a.reveal_argument_byte==1 and a.reveal_nonzero)
  assert(#a.source_record_hex==464 and #a.output_buffer_before_hex==464)
  assert(a.return_capture_status=='paired' and b.entry_sequence==a.sequence)
  assert(b.source_unchanged and b.candidate_completed_flag_set and b.rax_matches_output)
  assert(a.thread.status=='unresolved' and a.thread.thread_id==nil)
  assert(not s.probe.invokes_target_function and not s.probe.writes_item_data)
  s.probe.stop('complete');assert(next(s.sites)==nil)
end)

test('Three rapid same-caller slots retain return site despite deferred removal',function()
  local s=scenario();s.mode='deferred'
  for _,slot in ipairs({1,3,4}) do s.entry(slot);s.finish() end
  assert(#s.probe.events==6 and #s.arms==2 and #s.removals==0)
  for i=1,5,2 do assert(s.probe.events[i].return_capture_status=='paired') end
  assert(s.probe.events[3].retained_return_site and s.probe.events[5].retained_return_site)
  s.probe.stop('complete');assert(s.probe.cleanup_pending)
  s.mode='immediate';s.probe.retry_cleanup();assert(not s.probe.cleanup_pending and next(s.sites)==nil)
end)

test('Last hit drains its matched return before stopping',function()
  local s=scenario({config={max_hits=1}});s.entry()
  assert(s.probe.active and not s.probe.accepting and s.sites[CALLER])
  assert(s.sites[ENTRY]==nil);s.finish()
  assert(s.probe.stop_reason=='hit_budget' and #s.probe.events==2 and next(s.sites)==nil)
end)

test('Serial filters skip records without reading unrelated output and still enforce cap',function()
  local s=scenario({config={serial_le_hex='9900000000000000',max_hits=2}})
  s.fail_read=OUTPUT;s.entry();s.entry()
  assert(s.probe.filtered_entries==2 and #s.probe.events==0)
  assert(s.probe.stop_reason=='hit_budget' and next(s.sites)==nil)
end)

test('Wrong return SP does not pair and nested entry cannot overwrite pending identity',function()
  local s=scenario();s.entry(1);s.entry(3);s.finish(STACK+16)
  assert(#s.probe.events==2 and s.probe.events[2].return_capture_status=='skipped_pending_pair')
  s.finish();assert(s.probe.events[3].effect_index==1 and s.probe.unmatched_return_hits==1)
  s.probe.stop('complete')
end)

test('Changed caller waits for confirmed cleanup and next entry can retry',function()
  local s=scenario();s.entry();s.finish();s.caller=CALLER+0x100;s.mode='deferred';s.entry()
  assert(s.probe.events[3].return_capture_status=='skipped_pending_cleanup')
  assert(s.probe.cleanup_pending and s.sites[s.caller]==nil)
  s.sites[CALLER]=nil;s.mode='immediate';s.entry();s.finish()
  assert(s.probe.events[4].return_capture_status=='paired' and #s.probe.events==5)
  s.probe.stop('complete')
end)

test('Foreign return breakpoint and same-address replacement are never removed',function()
  local s=scenario();s.sites[CALLER]={id=9999};s.entry()
  assert(s.probe.stop_reason=='entry_capture_error' and s.sites[CALLER].id==9999)
  for _,address in ipairs(s.removals) do assert(address~=CALLER) end
  local replaced=scenario();replaced.sites[ENTRY]={id=9999}
  replaced.probe.stop('manual');assert(replaced.sites[ENTRY].id==9999 and replaced.probe.cleanup_pending)
end)

test('Failed cleanup remains owned after bounded retries and late callbacks collect nothing',function()
  local s=scenario();s.entry();local callback=s.sites[CALLER].callback;s.mode='failure'
  s.probe.stop('manual');assert(s.probe.cleanup_pending and not s.probe.active)
  callback();assert(#s.probe.events==1 and s.probe.events[1].return_capture_status=='incomplete')
  for _=1,3 do assert(s.fire(100)) end
  assert(not s.fire(100) and s.probe.cleanup_pending)
  s.mode='immediate';s.probe.retry_cleanup();assert(next(s.sites)==nil and not s.probe.cleanup_pending)
end)

test('Missing partial and unstable reads fail closed and continue the game',function()
  for _,field in ipairs({'fail_read','partial_read','unstable_read'}) do
    local s=scenario();s[field]=SOURCE;s.entry()
    assert(s.probe.stop_reason=='entry_capture_error' and #s.probe.events==0 and s.continues==1)
    assert(next(s.sites)==nil)
  end
  for _,address in ipairs({OUTPUT,SOURCE,DATA+8}) do
    local s=scenario();s.entry();s.fail_read=address;s.finish()
    assert(s.probe.stop_reason=='return_capture_error' and #s.probe.events==1)
    assert(s.probe.events[1].return_capture_status=='incomplete' and s.continues==2)
  end
end)

test('RAX, serial, PID, and inventory owner mismatches cannot produce paired output',function()
  for _,which in ipairs({'rax','serial','pid','owner','during_read'}) do
    local s=scenario()
    if which=='during_read' then s.owner_changes_during_read=true end
    s.entry()
    if which=='during_read' then assert(s.probe.stop_reason=='entry_capture_error')
    else
      if which=='serial' then s.output[0x28+1]=99 end
      if which=='pid' then s.pid=999 end
      if which=='owner' then s.data=DATA+0x100 end
      s.finish(nil,which=='rax' and OUTPUT+8 or nil)
      assert(s.probe.stop_reason=='return_capture_error' and #s.probe.events==1)
    end
    assert(next(s.sites)==nil)
  end
end)

test('TEB lookup validates live PID TID Self and stack bounds',function()
  local config={stack_owners={{thread_id=456,teb=TEB}}}
  local s=scenario({config=config});s.entry();s.finish()
  assert(s.probe.events[1].thread.thread_id==456 and s.probe.events[2].thread.thread_id==456)
  s.probe.stop('complete')
  local stale=scenario({config=config});stale.teb_pid=999;stale.entry()
  assert(stale.probe.events[1].thread.status=='unresolved')
  stale.probe.stop('complete')
end)

test('Time and unmatched return budgets stop owned sites without false pairs',function()
  local timed=scenario();timed.entry();assert(timed.fire(300000))
  assert(timed.probe.stop_reason=='time_budget' and timed.probe.events[1].return_capture_status=='incomplete')
  assert(next(timed.sites)==nil)
  local s=scenario();s.entry()
  for _=1,256 do s.finish(STACK+16) end
  assert(s.probe.stop_reason=='unmatched_return_budget' and #s.probe.events==1 and next(s.sites)==nil)
end)

test('Signature options existing sites and timer failure reject arming safely',function()
  assert(not scenario({bad_signature=true}).loaded)
  assert(not scenario({config={serial_le_hex='unsafe'}}).loaded)
  assert(not scenario({config={max_seconds=0}}).loaded)
  local foreign=scenario({foreign=CALLER});assert(not foreign.loaded and foreign.sites[CALLER])
  local timed=scenario({timer_failure=true})
  assert(timed.probe.stop_reason=='budget_timer_failed' and next(timed.sites)==nil)
end)

return {schema='nioh3-scroll-finalizer-probe-mocked-tests/v1',ok=true,test_count=#passed,
  passed=passed,real_debugger_calls=0,real_game_memory_access=false}
