-- PC v2.01 bounded observation of the candidate item insertion entry.
-- Run through CE Lua after verifying the process. This never calls the target
-- function, changes registers, or writes item/save data. Hardware execution
-- breakpoints auto-continue; a timer and hit budget remove only owned sites.

local BASE = assert(getAddressSafe('Nioh3.exe'), 'Nioh3.exe is not attached')
local ENTRY = BASE + 0x54D294
local SIGNATURE = {0x40,0x55,0x53,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x8D,0xAC}
local actual = readBytes(ENTRY, #SIGNATURE, true)
assert(actual and #actual == #SIGNATURE, 'Unreadable insertion entry')
for i, value in ipairs(SIGNATURE) do assert(actual[i] == value, 'PC v2.01 insertion signature mismatch') end
assert(not nioh3InsertionProbe or (not nioh3InsertionProbe.active and not nioh3InsertionProbe.cleanup_pending),
  'Stop and confirm cleanup of the existing owned probe first')
local initialBreakpoints = debug_getBreakpointList()
assert(type(initialBreakpoints) == 'table', 'Could not inspect debugger breakpoint ownership')
assert(#initialBreakpoints == 0, 'Existing debugger breakpoints must be preserved; use a separate capture session')

local function hex(value)
  return value and string.format('0x%X', value) or nil
end
local function readHex(address, count)
  if not address or address == 0 then return nil end
  local ok, bytes = pcall(readBytes, address, count, true)
  if not ok or not bytes or #bytes ~= count then return nil end
  local result = {}
  for i, value in ipairs(bytes) do result[i] = string.format('%02X', value) end
  return table.concat(result)
end
local function u16(address)
  local value = address and readSmallInteger(address)
  return value and value % 0x10000 or nil
end
local function u32(address)
  local value = address and readInteger(address)
  return value and value % 0x100000000 or nil
end
local function i32(address)
  if not address or address == 0 then return nil end
  local value = u32(address)
  if value and value >= 0x80000000 then value = value - 0x100000000 end
  return value
end
local scrollTypes = {[0x1E82]=true,[0x516D]=true,[0xE604]=true,[0xDD82]=true,[0xD523]=true}
local probe = {schema='nioh3-live-insertion-observation/v1', active=false,
  pid=getOpenedProcessID(), module_base=hex(BASE), entry_rva='0x54D294',
  entry_hits=0, scroll_hits=0, events={}, caller_counts={}, max_hits=2048, max_seconds=300,
  writes_item_data=false, invokes_target_function=false, thread_identity='not yet resolved',
  cleanup_pending=false, cleanup_errors={}, owned_breakpoints={}}
nioh3InsertionProbe = probe
local returnAddress, pending
local owned = {}
local cleanupTimerScheduled = false
local cleanupChecksRemaining = 0
local cleanupPass

local function cleanupError(message)
  if #probe.cleanup_errors < 32 then probe.cleanup_errors[#probe.cleanup_errors+1] = tostring(message) end
end

local function publishOwnership()
  local addresses, removalPending = {}, false
  for address, site in pairs(owned) do
    addresses[#addresses+1] = hex(address)
    if site.remove_requested then removalPending = true end
  end
  table.sort(addresses)
  probe.owned_breakpoints = addresses
  probe.cleanup_pending = removalPending
end

local function inspectBreakpoints()
  local ok, addresses = pcall(debug_getBreakpointList)
  if not ok or type(addresses) ~= 'table' then
    cleanupError('Breakpoint inventory unavailable: '..tostring(addresses))
    return nil
  end
  local present = {}
  for _, address in pairs(addresses) do present[address] = true end
  return present
end

local function reconcileOwnership(present)
  if not present then return end
  for address, site in pairs(owned) do
    if site.remove_requested and not present[address] then
      owned[address] = nil
      if address == returnAddress then returnAddress, pending = nil, nil end
    end
  end
end

local function scheduleCleanupCheck()
  if cleanupTimerScheduled or not probe.cleanup_pending or cleanupChecksRemaining <= 0 then return end
  cleanupTimerScheduled = true
  local ok, failure = pcall(createTimer, 100, function()
    cleanupTimerScheduled = false
    cleanupChecksRemaining = cleanupChecksRemaining - 1
    cleanupPass()
  end)
  if not ok then
    cleanupTimerScheduled = false
    cleanupChecksRemaining = 0
    cleanupError('Cleanup timer unavailable: '..tostring(failure))
  end
end

cleanupPass = function()
  reconcileOwnership(inspectBreakpoints())
  for address, site in pairs(owned) do
    if site.remove_requested then
      local ok, result
      if site.id ~= nil and type(debug_removeBreakpointByID) == 'function' then
        ok, result = pcall(debug_removeBreakpointByID, site.id)
      else
        ok, result = pcall(debug_removeBreakpoint, address)
      end
      if not ok or result == false then
        cleanupError('Removal not confirmed for '..hex(address)..': '..tostring(result))
      end
    end
  end
  -- CE may defer removal until the stopped thread continues. A successful API
  -- return is not proof that the hardware breakpoint has been removed.
  reconcileOwnership(inspectBreakpoints())
  publishOwnership()
  scheduleCleanupCheck()
end

local function requestRemoval(address)
  if owned[address] then owned[address].remove_requested = true end
  cleanupChecksRemaining = math.max(cleanupChecksRemaining, 3)
  cleanupPass()
end

function probe.retry_cleanup()
  cleanupChecksRemaining = 3
  cleanupPass()
  return {cleanup_pending=probe.cleanup_pending, owned_breakpoints=probe.owned_breakpoints}
end

function probe.stop(reason)
  probe.active = false
  probe.stop_reason = probe.stop_reason or reason or 'manual'
  pending = nil
  for _, site in pairs(owned) do site.remove_requested = true end
  return probe.retry_cleanup()
end

local function onReturn()
  local ok, failure = pcall(function()
    if not probe.active then return end
    debug_getContext(false)
    if pending and RSP == pending.expected_rsp then
      probe.events[#probe.events+1] = {kind='return', entry_sequence=pending.sequence,
        rax=hex(RAX), rsp=hex(RSP), output_address=hex(pending.output),
        output_record_hex=readHex(pending.output, 0xE8),
        source_after_hex=readHex(pending.source, 0xE8),
        output_slot_index_address=hex(pending.slot_output),
        output_slot_index=i32(pending.slot_output)}
      pending = nil
      requestRemoval(returnAddress)
    end
  end)
  if not ok then probe.error=tostring(failure); probe.stop('return_capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end

local function onEntry()
  local ok, failure = pcall(function()
    if not probe.active then return end
    debug_getContext(false)
    probe.entry_hits = probe.entry_hits + 1
    local recordType = u16(R8)
    local scroll = scrollTypes[recordType] == true
    if scroll then probe.scroll_hits = probe.scroll_hits + 1 end
    local caller = readQword(RSP)
    local callerKey = hex(caller) or 'unreadable'
    probe.caller_counts[callerKey] = (probe.caller_counts[callerKey] or 0) + 1
    if scroll or (probe.caller_counts[callerKey] <= 4 and #probe.events < 120) then
      local event = {kind='entry', sequence=probe.entry_hits, is_scroll=scroll,
        record_type=hex(recordType), seed=scroll and u32(R8+0x20) or nil,
        rcx=hex(RCX), rdx=hex(RDX), r8=hex(R8), r9=hex(R9), rsp=hex(RSP),
        fifth_argument=readBytes(RSP+0x28,1,false), return_address=hex(caller),
        caller_rva=caller and hex(caller-BASE) or nil,
        source_hex=readHex(R8,0xE8), stack_hex=readHex(RSP,0x80)}
      probe.events[#probe.events+1] = event
      if not returnAddress and caller then
        pending = {sequence=probe.entry_hits, expected_rsp=RSP+8, source=R8, output=RDX, slot_output=R9}
        returnAddress = caller
        owned[caller] = {remove_requested=false}
        local armOk, armed, id = pcall(debug_setBreakpoint, caller, 1, bptExecute, bpmDebugRegister, onReturn)
        owned[caller].id = id
        publishOwnership()
        if not armOk or not armed then
          pending=nil
          probe.return_arm_failed=true
          probe.stop('return_arm_failed')
        end
      end
    end
    if probe.entry_hits >= probe.max_hits then probe.stop('hit_budget') end
  end)
  if not ok then probe.error=tostring(failure); probe.stop('entry_capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end

if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(), 'Could not attach Windows debugger')
owned[ENTRY] = {remove_requested=false}
local armOk, armed, id = pcall(debug_setBreakpoint, ENTRY, 1, bptExecute, bpmDebugRegister, onEntry)
owned[ENTRY].id = id
publishOwnership()
if not armOk or not armed then
  probe.stop('entry_arm_failed')
  error('Could not arm insertion observer; inspect nioh3InsertionProbe.cleanup_pending')
end
probe.active = true
local timerOk, timerFailure = pcall(createTimer, probe.max_seconds*1000, function()
  if probe.active then probe.stop('time_budget') end
end)
if not timerOk then
  probe.error=tostring(timerFailure)
  probe.stop('budget_timer_failed')
end
return {active=probe.active, pid=probe.pid, max_hits=probe.max_hits, max_seconds=probe.max_seconds,
  entry_rva=probe.entry_rva, cleanup_pending=probe.cleanup_pending, owned_breakpoints=probe.owned_breakpoints}
