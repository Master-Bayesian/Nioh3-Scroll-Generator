-- Shared read-only helpers for PC v2.01 possessed-enemy observers.
return function(schema, sites, maxSeconds, maxEvents)
  local base = assert(getAddressSafe('Nioh3.exe'), 'Nioh3.exe is not attached')
  local pid = assert(getOpenedProcessID(), 'No attached process')
  local function hex(value) return value and string.format('0x%X', value) or nil end
  local function bytesHex(address, count, label)
    local ok, bytes = pcall(readBytes, address, count, true)
    assert(ok and type(bytes) == 'table' and #bytes == count, 'Unreadable ' .. label)
    local out = {}
    for i = 1, count do out[i] = string.format('%02X', bytes[i]) end
    return table.concat(out)
  end
  local function u8(address, label)
    local ok, bytes = pcall(readBytes, address, 1, true)
    assert(ok and type(bytes) == 'table' and #bytes == 1, 'Unreadable ' .. label)
    return bytes[1]
  end
  local function u32(address, label)
    local ok, value = pcall(readInteger, address)
    assert(ok and type(value) == 'number', 'Unreadable ' .. label)
    return value & 0xFFFFFFFF
  end
  local function u64(address, label, allowZero)
    local ok, value = pcall(readQword, address)
    assert(ok and type(value) == 'number', 'Unreadable ' .. label)
    if not allowZero then assert(value ~= 0, 'Null ' .. label) end
    return value
  end
  local function threadId()
    for _, name in ipairs({'getCurrentThreadId', 'getCurrentThreadID'}) do
      local fn = _G[name]
      if type(fn) == 'function' then
        local ok, value = pcall(fn)
        if ok and type(value) == 'number' then return value end
      end
    end
    return nil
  end
  for label, site in pairs(sites) do
    site.address = base + site.rva
    local actual = readBytes(site.address, #site.expected, true)
    assert(actual and #actual == #site.expected, 'Unreadable ' .. label .. ' signature')
    for i, value in ipairs(site.expected) do
      assert(actual[i] == value, 'PC v2.01 ' .. label .. ' signature mismatch')
    end
  end
  assert(not nioh3PossessedCapture or
    (not nioh3PossessedCapture.active and not nioh3PossessedCapture.cleanup_pending),
    'Stop and clean the previous possessed-enemy capture first')
  local debugging = debug_isDebugging()
  local initial = debug_getBreakpointList()
  assert((initial == nil and not debugging) or
    (type(initial) == 'table' and next(initial) == nil),
    'Existing or unknown debugger breakpoints must be preserved')
  local probe = {
    schema = schema, active = false, read_only = true, writes_game_memory = false,
    pid = pid, module_base = hex(base), run_id = NIOH3_POSSESSED_RUN_ID or 'unspecified',
    max_seconds = maxSeconds, max_events = maxEvents, event_sequence = 0, events = {},
  }
  nioh3PossessedCapture = probe
  local helperEnv = {assert=assert,pairs=pairs,pcall=pcall,type=type,tostring=tostring,
    math=math,string=string,table=table}
  local helperPath = NIOH3_BREAKPOINT_LIFECYCLE_PATH or
    'F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua'
  local makeOwner = assert(loadfile(helperPath, 't', helperEnv))()
  local owner = makeOwner({
    list=debug_getBreakpointList, remove=debug_removeBreakpoint,
    remove_id=debug_removeBreakpointByID, timer=createTimer,
    arm=function(address, callback)
      return debug_setBreakpoint(address, 1, bptExecute, bpmDebugRegister, callback)
    end,
  }, probe)
  function probe.retry_cleanup() return owner.retry_cleanup() end
  function probe.stop(reason)
    probe.active = false
    probe.stop_reason = probe.stop_reason or reason or 'manual'
    return owner.stop()
  end
  local api = {base=base,pid=pid,probe=probe,owner=owner,hex=hex,bytesHex=bytesHex,
    u8=u8,u32=u32,u64=u64,threadId=threadId}
  function api.event(site, fields)
    probe.event_sequence = probe.event_sequence + 1
    if probe.event_sequence > probe.max_events then
      probe.error = 'Event budget exceeded'
      probe.stop('event_budget')
      return false
    end
    fields.sequence = probe.event_sequence
    fields.site = site
    fields.thread_id = threadId()
    probe.events[#probe.events + 1] = fields
    return true
  end
  function api.verify()
    assert(getOpenedProcessID() == pid and getAddressSafe('Nioh3.exe') == base,
      'Attached process identity changed')
  end
  function api.arm(callback)
    if not debug_isDebugging() then debugProcess(1) end
    assert(debug_isDebugging(), 'Could not attach Windows debugger')
    for label, site in pairs(sites) do
      local armed, failure = owner.arm(site.address, callback)
      if not armed then
        probe.error = label .. ': ' .. tostring(failure)
        probe.stop('arm_failed')
        error(probe.error)
      end
    end
    probe.active = true
    local ok, failure = pcall(createTimer, maxSeconds * 1000, function()
      if probe.active then probe.stop('time_budget') end
    end)
    if not ok then probe.error=tostring(failure); probe.stop('budget_timer_failed') end
    return {active=probe.active,read_only=true,pid=pid,run_id=probe.run_id,
      schema=schema,owned_breakpoints=probe.owned_breakpoints}
  end
  return api
end
