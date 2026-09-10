-- Observe the native serial counter's writers without changing its value.
local base=assert(getAddressSafe('Nioh3.exe'))
local pid=getOpenedProcessID()
local duration=tonumber(nioh3SerialWriteOptions and nioh3SerialWriteOptions.seconds) or 30
assert(duration>=1 and duration<=120 and duration%1==0,'Invalid observation duration')
local manager=assert(readQword(base+0x474D4E0))
local data=assert(readQword(manager))
assert(manager~=0 and data~=0 and (data+8)%8==0,'Invalid counter owner')
assert(readQword(data+0x224A60+0x16A80)==400,'Unexpected scroll capacity')
assert(not nioh3SerialWriteProbe or
  (not nioh3SerialWriteProbe.active and not nioh3SerialWriteProbe.cleanup_pending),
  'Previous observer needs cleanup')
assert(debug_isDebugging(),'Attach debugger before observing writes')
local listed=debug_getBreakpointList()
assert(type(listed)=='table' and next(listed)==nil,'Existing or unknown breakpoints')
local function bytes(address)
  local raw=assert(readBytes(address,8,true),'Unreadable counter')
  assert(#raw==8,'Incomplete counter')
  local out={};for i,v in ipairs(raw) do out[i]=string.format('%02X',v) end
  return table.concat(out)
end
local function hex(value) return value and string.format('0x%X',value) or nil end
local probe={active=true,events={},started_at=os.time(),counter_address=hex(data+8),
  initial_serial_le_hex=bytes(data+8),read_only=true,pid=pid,max_hits=16,duration_seconds=duration}
nioh3SerialWriteProbe=probe
local makeOwner=dofile('F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua')
local schedule=dofile('F:/Nioh3_ScrollEditor/research/ce_main_thread_timer.lua')
local owner=makeOwner({list=function() return debug_getBreakpointList() end,
  remove=function(address) return debug_removeBreakpoint(address) end,
  remove_id=function(id) return debug_removeBreakpointByID(id) end,timer=schedule,
  arm=function(address,callback) return debug_setBreakpoint(address,8,bptWrite,bpmDebugRegister,callback) end},probe)
function probe.stop(reason)
  probe.active=false;probe.stop_reason=reason or 'manual';probe.stopped_at=os.time()
  return owner.stop()
end
function probe.retry_cleanup() return owner.retry_cleanup() end
local ok,err=owner.arm(data+8,function()
  local success,failure=pcall(function()
    if not probe.active then return end
    assert(getOpenedProcessID()==pid and readQword(base+0x474D4E0)==manager
      and readQword(manager)==data,'Counter owner changed')
    debug_getContext(false)
    -- Data breakpoints report the instruction pointer after the write.
    local rva=RIP-base
    local offsets={[0x54CCFF]=0x28,[0x54DFDD]=0x248,[0x8AFC18]=0x138}
    local caller=offsets[rva] and readQword(RSP+offsets[rva]) or nil
    probe.events[#probe.events+1]={time=os.time(),rip_after_write=hex(RIP),
      rva_after_write=hex(rva),rsp=hex(RSP),serial_le_hex=bytes(data+8),
      caller_stack_offset=offsets[rva],caller_hint=hex(caller),
      caller_rva_hint=caller and hex(caller-base) or nil}
    if #probe.events>=probe.max_hits then probe.stop('hit_budget') end
  end)
  if not success then probe.error=tostring(failure);probe.stop('capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end)
if not ok then probe.stop('arm_failed');error(tostring(err)) end
local scheduled,failure=pcall(schedule,duration*1000,function() if probe.active then probe.stop('time_budget') end end)
if not scheduled then probe.stop('timer_failed');error(tostring(failure)) end
