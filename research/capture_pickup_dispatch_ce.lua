-- Bounded observation only: no record writes or native function invocation.
local base=assert(getAddressSafe('Nioh3.exe'))
local signature={0x40,0x53,0x57,0x48,0x83,0xEC,0x38}
local actual=readBytes(base+0x12E6840,#signature,true)
assert(actual and #actual==#signature,'Unreadable dispatch signature')
for i,v in ipairs(signature) do assert(actual[i]==v,'Dispatch signature mismatch') end
assert(not nioh3PickupDispatchProbe or
  (not nioh3PickupDispatchProbe.active and not nioh3PickupDispatchProbe.cleanup_pending),
  'Previous probe active or cleanup unconfirmed')
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Debugger unavailable')
local listed=debug_getBreakpointList()
assert(type(listed)=='table' and next(listed)==nil,'Existing or unknown breakpoints')
local probe={active=true,events={},max_hits=8,started_at=os.time(),read_only=true}
nioh3PickupDispatchProbe=probe
local makeOwner=dofile('F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua')
local schedule=dofile('F:/Nioh3_ScrollEditor/research/ce_main_thread_timer.lua')
local owner=makeOwner({list=function() return debug_getBreakpointList() end,
  remove=function(address) return debug_removeBreakpoint(address) end,
  remove_id=function(id) return debug_removeBreakpointByID(id) end,timer=schedule,
  arm=function(address,callback) return debug_setBreakpoint(address,1,bptExecute,bpmDebugRegister,callback) end},probe)
function probe.stop(reason)
  probe.active=false
  probe.stop_reason=reason or 'manual'
  return owner.stop()
end
function probe.retry_cleanup() return owner.retry_cleanup() end
local function hex(value) return value and string.format('0x%X',value) or nil end
local armed,err=owner.arm(base+0x12E6840,function()
  local ok,failure=pcall(function()
    if not probe.active then return end
    debug_getContext(false)
    local caller=assert(readQword(RSP),'Unreadable return address')
    local first,last=readQword(RCX+0x60),readQword(RCX+0x68)
    assert(first and last and last>=first and (last-first)%8==0,'Invalid pickup pointer range')
    assert(last-first<=8*10000,'Unbounded pickup pointer range')
    probe.events[#probe.events+1]={time=os.time(),rsp=hex(RSP),rcx=hex(RCX),
      caller=hex(caller),caller_rva=hex(caller-base),queued_pointer_count=(last-first)/8}
    if #probe.events>=probe.max_hits then probe.stop('hit_budget') end
  end)
  if not ok then probe.error=tostring(failure);probe.stop('capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end)
if not armed then probe.stop('arm_failed');error(tostring(err)) end
local timerOk,timerError=pcall(schedule,15000,function() if probe.active then probe.stop('time_budget') end end)
if not timerOk then probe.stop('timer_failed');error(tostring(timerError)) end
