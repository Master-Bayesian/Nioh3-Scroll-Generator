-- Closed mock environment: no real debugger or game calls are available.
local root=NIOH3_RESEARCH_TEST_ROOT or 'F:/Nioh3_ScrollEditor/research/'
local BASE,ENTRY,SERIAL_SITE,GLOBAL=0x10000000,0x1227C4CC,0x1227C707,0x1474D4E0
local OUTPUT,DESC,STACK,CALLER,MANAGER,DATA=0x20000000,0x21000000,0x22000000,0x23000000,0x24000000,0x25000000
local signatures={
  [ENTRY]={0x48,0x89,0x5C,0x24,0x08,0x48,0x89,0x6C,0x24,0x10,0x48,0x89,0x74,0x24,0x18,0x57,
    0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xEC,0x20,0x45,0x33,0xED,0xC7},
  [SERIAL_SITE]={0x45,0x38,0x6E,0x21,0x75,0x1A,0x48,0x8B,0x05,0xCC,0x0D,0x4D,0x02,0x48,0x8B,0x10,
    0x48,0x8B,0x4A,0x08,0x48,0x8D,0x41,0x01,0x48,0x89,0x42,0x08,0x48,0x89,0x4F,0x28}}
local function scenario(skip)
  local state={sites={},timers={},removals={},nextId=1,mode='immediate',counter=50,
    skip=skip or 0,outputSerial=0,continues=0,deferred={},manager=MANAGER,data=DATA}
  local env={assert=assert,error=error,ipairs=ipairs,pairs=pairs,pcall=pcall,type=type,
    tostring=tostring,math=math,string=string,table=table,loadfile=loadfile,
    bptExecute=0,bpmDebugRegister=1,co_run=0,
    NIOH3_BREAKPOINT_LIFECYCLE_PATH=root..'owned_breakpoint_lifecycle_ce.lua',NIOH3_BUILDER_ACTION='mock menu preview'}
  state.env=env
  env.getAddressSafe=function() return BASE end
  env.getOpenedProcessID=function() return 123 end
  env.readBytes=function(address,count,asTable)
    if address==state.readFailureAddress then return nil end
    if address==state.partialReadAddress then return {0} end
    if not asTable then
      if address==DESC+0x21 then return state.skip end
      if address==DESC+0x22 then return 3 end
      return 0
    end
    local result={}
    for index=1,count do
      local offset=index-1
      result[index]=0
      if signatures[address] then result[index]=signatures[address][index]
      elseif address==DESC and offset==0x21 then result[index]=state.skip
      elseif address==DESC and offset==0x22 then result[index]=3
      elseif address==DATA+8 and offset==0 then result[index]=state.counter
      elseif address==OUTPUT and offset>=0x28 and offset<0x30 then
        result[index]=state.outputSerial==-1 and 0xFF or (offset==0x28 and state.outputSerial or 0)
      end
    end
    if address==DATA+8 and state.replaceOwnerDuringCounterRead then state.manager=MANAGER+0x100 end
    return result
  end
  env.readQword=function(address)
    if address==GLOBAL then return state.manager end
    if address==state.manager then return state.data end
    if address==STACK then return CALLER end
    error('unmapped mock pointer')
  end
  env.debug_isDebugging=function() return true end
  env.debug_getContext=function() end
  env.debug_continueFromBreakpoint=function() state.continues=state.continues+1 end
  env.debug_getBreakpointList=function()
    local list={}
    for address in pairs(state.sites) do list[#list+1]=address end
    return list
  end
  env.debug_setBreakpoint=function(address,size,trigger,method,callback)
    assert(size==1 and trigger==0 and method==1)
    local id=state.nextId
    state.nextId=id+1
    state.sites[address]={id=id,callback=callback}
    return true,id
  end
  local function remove(address)
    state.removals[#state.removals+1]=address
    if state.mode=='failure' then error('injected remove failure') end
    if state.mode=='deferred' then state.deferred[address]=true;return end
    state.sites[address]=nil
  end
  env.debug_removeBreakpoint=remove
  env.debug_removeBreakpointByID=function(id)
    for address,site in pairs(state.sites) do if site.id==id then remove(address);return end end
  end
  env.createTimer=function(delay,callback)
    state.timers[#state.timers+1]={delay=delay,callback=callback}
    return {}
  end
  function state.fire(delay)
    for index,timer in ipairs(state.timers) do
      if timer.delay==delay then table.remove(state.timers,index);timer.callback();return true end
    end
    return false
  end
  function state.entry()
    env.RCX,env.RDX,env.RSP=OUTPUT,DESC,STACK
    state.sites[ENTRY].callback()
  end
  function state.finish(rsp,rax)
    env.RSP,env.RAX=rsp or STACK+8,rax or OUTPUT
    state.sites[CALLER].callback()
  end
  local chunk,failure=loadfile(root..'capture_live_scroll_builder_ce.lua','t',env)
  assert(chunk,failure)
  state.result=chunk()
  state.probe=env.nioh3ScrollBuilderProbe
  return state
end
local passed={}
local function test(name,body)
  local ok,failure=pcall(body)
  if not ok then error(name..': '..tostring(failure)) end
  passed[#passed+1]=name
end

test('Matched return captures zero skip flag, raw counters and output',function()
  local state=scenario(0)
  state.entry()
  state.finish(STACK+16)
  assert(#state.probe.events==1 and state.probe.unmatched_return_hits==1)
  -- Values are supplied by the mock; this verifies observation, not generation.
  state.outputSerial,state.counter=50,51
  state.finish()
  local entry,result=state.probe.events[1],state.probe.events[2]
  assert(entry.skip_serial_allocation==0 and entry.descriptor_byte_22==3 and entry.playthrough_input==nil)
  assert(#entry.descriptor_hex==0xCC*2 and entry.output_record_hex==nil)
  assert(entry.counter_before.serial_counter_le_hex=='3200000000000000')
  assert(result.counter_after.serial_counter_le_hex=='3300000000000000')
  assert(result.output_record_hex:sub(0x28*2+1,0x30*2)=='3200000000000000')
  assert(result.rax_matches_output and result.entry_sequence==1)
  assert(state.probe.provenance=='observed_builder_call' and not state.probe.natural_acquisition_confirmed)
  state.probe.stop('complete')
end)

test('Skip flag preserves observed sentinel and does not imply natural acquisition',function()
  local state=scenario(1)
  state.entry()
  state.outputSerial=-1
  state.finish()
  local result=state.probe.events[2]
  assert(state.probe.events[1].skip_serial_allocation==1)
  assert(result.counter_after.serial_counter_le_hex=='3200000000000000')
  assert(result.output_record_hex:sub(0x28*2+1,0x30*2)=='FFFFFFFFFFFFFFFF')
  assert(result.provenance=='observed_builder_call' and state.probe.action_label=='mock menu preview')
  state.probe.stop('complete')
end)

test('RAX mismatch is reported rather than accepted as output identity',function()
  local state=scenario()
  state.entry()
  state.finish(nil,OUTPUT+8)
  assert(state.probe.events[2].rax_matches_output==false)
  state.probe.stop('complete')
end)

test('Missing or partial required entry reads stop without an accepted event',function()
  for _,address in ipairs({DESC,DATA+8,DATA,DESC+0x21,DESC+0x22}) do
    local state=scenario()
    state.readFailureAddress=address
    state.entry()
    assert(state.probe.stop_reason=='entry_capture_error' and not state.probe.active)
    assert(#state.probe.events==0 and not state.probe.cleanup_pending and next(state.sites)==nil)
    assert(state.continues==1)
  end
  local state=scenario()
  state.partialReadAddress=DESC
  state.entry()
  assert(state.probe.stop_reason=='entry_capture_error' and #state.probe.events==0 and next(state.sites)==nil)
end)

test('Missing return readback cannot produce a completed pair',function()
  for _,address in ipairs({OUTPUT,DESC,DATA+8,DATA}) do
    local state=scenario()
    state.entry()
    state.readFailureAddress=address
    state.finish()
    assert(state.probe.stop_reason=='return_capture_error' and not state.probe.active)
    assert(#state.probe.events==1 and not state.probe.cleanup_pending and next(state.sites)==nil)
    assert(state.continues==2)
  end
end)

test('Counter owner changes during a read or between a pair fail closed',function()
  local during=scenario()
  during.replaceOwnerDuringCounterRead=true
  during.entry()
  assert(during.probe.stop_reason=='entry_capture_error' and #during.probe.events==0)
  assert(next(during.sites)==nil and during.continues==1)
  for _,field in ipairs({'manager','data'}) do
    local between=scenario()
    between.entry()
    between[field]=between[field]+0x100
    between.finish()
    assert(between.probe.stop_reason=='return_capture_error' and #between.probe.events==1)
    assert(not between.probe.cleanup_pending and next(between.sites)==nil and between.continues==2)
  end
end)

test('Null owner pointers are never recorded as valid counter locations',function()
  for _,field in ipairs({'manager','data'}) do
    local state=scenario()
    state[field]=0
    state.entry()
    assert(state.probe.stop_reason=='entry_capture_error' and #state.probe.events==0)
    assert(next(state.sites)==nil and state.continues==1)
  end
end)

test('Failed cleanup is bounded and manually retryable without foreign removal',function()
  local state=scenario()
  local foreign=0x30000000
  state.sites[foreign]={id=9999}
  state.mode='failure'
  state.probe.stop('manual')
  assert(state.probe.cleanup_pending and not state.probe.active)
  state.entry()
  assert(state.probe.entry_hits==0 and state.continues==1)
  for _=1,3 do assert(state.fire(100)) end
  assert(not state.fire(100) and state.probe.cleanup_pending)
  state.mode='immediate'
  state.probe.retry_cleanup()
  assert(not state.probe.cleanup_pending and state.sites[foreign])
  for _,address in ipairs(state.removals) do assert(address~=foreign) end
end)

test('Deferred return cleanup waits for inventory confirmation',function()
  local state=scenario()
  state.entry()
  state.mode='deferred'
  state.finish()
  assert(state.probe.cleanup_pending and state.sites[CALLER])
  state.sites[CALLER]=nil
  assert(state.fire(100))
  assert(not state.probe.cleanup_pending and state.sites[ENTRY])
  state.mode='immediate'
  state.probe.stop('complete')
end)

test('Entry hit and time budgets stop all owned sites',function()
  local hits=scenario()
  hits.probe.max_hits=1
  hits.entry()
  assert(hits.probe.stop_reason=='hit_budget' and not hits.probe.active and not hits.probe.cleanup_pending)
  assert(next(hits.sites)==nil)
  local timed=scenario()
  assert(timed.fire(120000))
  assert(timed.probe.stop_reason=='time_budget' and not timed.probe.active and not timed.probe.cleanup_pending)
  assert(next(timed.sites)==nil)
end)

return {schema='nioh3-scroll-builder-probe-mocked-tests/v1',ok=true,test_count=#passed,
  passed=passed,real_debugger_calls=0,real_game_memory_access=false}
