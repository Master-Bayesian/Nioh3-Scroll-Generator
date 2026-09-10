-- Pure mocked lifecycle tests. This file never calls the real CE/debugger API.
-- Execute with dofile from CE Lua; loadfile gives the observer a closed sandbox.
local sourcePath = NIOH3_PROBE_SOURCE_PATH or
  'F:/Nioh3_ScrollEditor/research/capture_live_item_insertion_ce.lua'
local BASE, ENTRY, CALLER = 0x10000000, 0x1054D294, 0x103EED9E
local SOURCE, OUTPUT, SLOT, STACK = 0x20000000, 0x21000000, 0x22000000, 0x23000000
local FOREIGN = 0x30000000
local signature = {0x40,0x55,0x53,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x8D,0xAC}

local function scenario(options)
  options = options or {}
  local state = {sites={}, nextId=1, timers={}, removals={}, continues=0,
    removeMode=options.removeMode or 'immediate', inventoryUnavailable=false, deferred={}}
  -- No __index fallback: an omitted CE function fails instead of reaching CE.
  local env = {assert=assert, error=error, ipairs=ipairs, pairs=pairs, pcall=pcall,
    tostring=tostring, type=type, math=math, string=string, table=table,
    bptExecute=0, bpmDebugRegister=1, co_run=0}
  state.env = env
  env.getAddressSafe = function(name) assert(name=='Nioh3.exe'); return BASE end
  env.getOpenedProcessID = function() return 123 end
  env.readBytes = function(address, count, asTable)
    if asTable then
      local bytes = {}
      for index=1,count do bytes[index] = address==ENTRY and signature[index] or 0 end
      return bytes
    end
    return 0
  end
  env.readSmallInteger = function(address) return address==SOURCE and 0xE604 or 0 end
  env.readInteger = function(address) return address==SLOT and 0xFFFFFFFF or 0 end
  env.readQword = function(address) assert(address==STACK); return CALLER end
  env.debug_isDebugging = function() return true end
  env.debug_getContext = function() end
  env.debug_continueFromBreakpoint = function(mode) assert(mode==0); state.continues=state.continues+1 end
  env.debug_getBreakpointList = function()
    if state.inventoryUnavailable then error('injected inventory failure') end
    local addresses = {}
    for address in pairs(state.sites) do addresses[#addresses+1]=address end
    return addresses
  end
  env.debug_setBreakpoint = function(address, size, trigger, method, callback)
    assert(size==1 and trigger==0 and method==1)
    local id = state.nextId
    state.nextId = id+1
    state.sites[address] = {id=id, callback=callback}
    return true, id
  end
  local function remove(address)
    state.removals[#state.removals+1]=address
    if state.removeMode=='failure' then error('injected removal failure') end
    if state.removeMode=='deferred' then state.deferred[address]=true; return end
    state.sites[address]=nil
  end
  env.debug_removeBreakpoint = remove
  env.debug_removeBreakpointByID = function(id)
    for address, site in pairs(state.sites) do
      if site.id==id then remove(address); return end
    end
  end
  env.createTimer = function(delay, callback)
    if options.failBudgetTimer and delay==300000 then error('injected timer failure') end
    state.timers[#state.timers+1]={delay=delay, callback=callback}
    return {}
  end
  function state.fireTimer(delay)
    for index, timer in ipairs(state.timers) do
      if timer.delay==delay then
        table.remove(state.timers,index)
        timer.callback()
        return true
      end
    end
    return false
  end
  function state.flushDeferred()
    for address in pairs(state.deferred) do state.sites[address]=nil end
    state.deferred={}
  end
  function state.entry()
    env.RCX,env.RDX,env.R8,env.R9,env.RSP = 0x24000000,OUTPUT,SOURCE,SLOT,STACK
    assert(state.sites[ENTRY], 'entry breakpoint missing').callback()
  end
  function state.finish()
    env.RAX,env.RSP = OUTPUT,STACK+8
    assert(state.sites[CALLER], 'return breakpoint missing').callback()
  end
  function state.addForeign(address)
    state.sites[address or FOREIGN]={id=99999, callback=function() error('foreign callback invoked') end}
  end
  local chunk, failure = loadfile(sourcePath,'t',env)
  assert(chunk, failure)
  state.result=chunk()
  state.probe=env.nioh3InsertionProbe
  return state
end

local passed = {}
local function test(name, body)
  local ok, failure = pcall(body)
  if not ok then error(name..': '..tostring(failure)) end
  passed[#passed+1]=name
end

test('Failed removal retains ownership, bounded retries, manual recovery', function()
  local state=scenario({removeMode='failure'})
  state.probe.stop('test_stop')
  assert(not state.probe.active and state.probe.cleanup_pending)
  assert(#state.probe.owned_breakpoints==1 and state.sites[ENTRY])
  local before=state.probe.entry_hits
  state.entry()
  assert(state.probe.entry_hits==before and state.continues==1, 'stopped callback collected new work')
  for _=1,3 do assert(state.fireTimer(100), 'bounded cleanup check missing') end
  assert(not state.fireTimer(100), 'cleanup scheduled forever')
  assert(state.probe.cleanup_pending, 'failed cleanup falsely reported complete')
  state.removeMode='immediate'
  state.probe.retry_cleanup()
  assert(not state.probe.cleanup_pending and #state.probe.owned_breakpoints==0)
end)

test('Deferred return removal and optional signed slot index', function()
  local state=scenario({removeMode='deferred'})
  state.entry()
  state.finish()
  assert(state.probe.active and state.probe.cleanup_pending)
  assert(state.probe.events[2].output_slot_index==-1)
  assert(state.probe.events[2].output_slot_index_address==string.format('0x%X',SLOT))
  assert(state.sites[CALLER], 'mock failed to retain deferred breakpoint')
  state.flushDeferred()
  assert(state.fireTimer(100))
  assert(not state.probe.cleanup_pending and state.sites[ENTRY])
  assert(#state.probe.owned_breakpoints==1)
  state.removeMode='immediate'
  state.probe.stop('complete')
end)

test('Hit budget removes entry and return sites without foreign removals', function()
  local state=scenario()
  state.addForeign()
  state.probe.max_hits=1
  state.entry()
  assert(not state.probe.active and state.probe.stop_reason=='hit_budget')
  assert(not state.probe.cleanup_pending and not state.sites[ENTRY] and not state.sites[CALLER])
  assert(state.sites[FOREIGN] and #state.removals==2)
  for _, address in ipairs(state.removals) do assert(address~=FOREIGN) end
end)

test('Time budget removes owned sites and preserves foreign breakpoint', function()
  local state=scenario()
  state.addForeign()
  assert(state.fireTimer(300000))
  assert(not state.probe.active and state.probe.stop_reason=='time_budget')
  assert(not state.probe.cleanup_pending and state.sites[FOREIGN])
  assert(#state.removals==1 and state.removals[1]==ENTRY)
end)

test('Unavailable inventory never falsely confirms cleanup', function()
  local state=scenario()
  state.inventoryUnavailable=true
  state.probe.stop('test_stop')
  assert(not state.sites[ENTRY] and state.probe.cleanup_pending)
  state.inventoryUnavailable=false
  state.probe.retry_cleanup()
  assert(not state.probe.cleanup_pending and #state.probe.owned_breakpoints==0)
end)

test('Replacing an owned address does not authorize removing a foreign ID', function()
  local state=scenario()
  state.addForeign(ENTRY)
  state.probe.stop('test_stop')
  assert(state.sites[ENTRY].id==99999 and #state.removals==0)
  assert(state.probe.cleanup_pending, 'replacement address was falsely confirmed absent')
  state.sites[ENTRY]=nil -- Simulate its owner's later removal, not observer cleanup.
  state.probe.retry_cleanup()
  assert(not state.probe.cleanup_pending)
end)

test('Budget timer failure stops the observer immediately', function()
  local state=scenario({failBudgetTimer=true})
  assert(not state.result.active and state.probe.stop_reason=='budget_timer_failed')
  assert(not state.probe.cleanup_pending and #state.probe.owned_breakpoints==0)
end)

return {schema='nioh3-live-insertion-probe-mocked-tests/v1', ok=true,
  test_count=#passed, passed=passed, real_debugger_calls=0, real_game_memory_access=false}
