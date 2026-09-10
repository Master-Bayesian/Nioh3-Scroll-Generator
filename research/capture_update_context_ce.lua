-- Read-only bounded observation of the enclosing game-update entry.
local base=assert(getAddressSafe('Nioh3.exe'))
local pid=getOpenedProcessID()
local entry=base+0xE64480
local signature={0x48,0x8B,0xC4,0x55,0x53,0x56,0x57,0x48,0x8D,0xA8,0x98,0xF5,0xFF,0xFF,0x48,0x81,0xEC,0x48,0x0B,0,0}
local actual=assert(readBytes(entry,#signature,true))
assert(#actual==#signature,'Incomplete update signature')
for i,v in ipairs(signature) do assert(actual[i]==v,'Update entry signature mismatch') end
assert(debug_isDebugging(),'Debugger must be attached')
local listed=debug_getBreakpointList()
assert(type(listed)=='table' and next(listed)==nil,'Existing or unknown breakpoints')
assert(not nioh3UpdateContextProbe or (not nioh3UpdateContextProbe.active and not nioh3UpdateContextProbe.cleanup_pending),'Previous observer requires cleanup')
local makeOwner=dofile('F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua')
local schedule=dofile('F:/Nioh3_ScrollEditor/research/ce_main_thread_timer.lua')
local p={schema='nioh3-update-context/v1',pid=pid,active=true,read_only=true,events={},started_at=os.time()}
nioh3UpdateContextProbe=p
local owner=makeOwner({list=function() return debug_getBreakpointList() end,
  remove=function(a) return debug_removeBreakpoint(a) end,timer=schedule,
  arm=function(a,cb) return debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,cb) end},p)
function p.stop(reason) p.active=false;p.stop_reason=reason;p.stopped_at=os.time();return owner.stop() end
function p.retry_cleanup() return owner.retry_cleanup() end
local function hex(v) return v and string.format('0x%X',v) end
local ok,err=owner.arm(entry,function()
  local success,failure=pcall(function()
    if not p.active then return end
    assert(getOpenedProcessID()==pid,'Process changed')
    debug_getContext(false)
    assert(RIP==entry,'Unexpected entry')
    local caller=assert(readQword(RSP),'Unreadable caller')
    -- Entry may be close to StackBase. Stop at the readable boundary rather
    -- than losing the validated caller when a larger optional read is partial.
    local stack={}
    for offset=0,0x1F8,8 do
      local value=readQword(RSP+offset)
      if value==nil then break end
      stack[#stack+1]={offset=offset,value=hex(value)}
    end
    p.events[#p.events+1]={rsp=hex(RSP),rcx=hex(RCX),rdx=hex(RDX),caller=hex(caller),caller_rva=hex(caller-base),
      object_first_qword=hex(readQword(RCX)),stack_qwords=stack,time=os.time()}
    if #p.events>=2 then p.stop('hit_budget') end
  end)
  if not success then p.error=tostring(failure);p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run);return 1
end)
if not ok then p.stop('arm_failed');error(tostring(err)) end
local scheduled,failure=pcall(schedule,15000,function() if p.active then p.stop('time_budget') end end)
if not scheduled then p.stop('timer_failed');error(tostring(failure)) end
