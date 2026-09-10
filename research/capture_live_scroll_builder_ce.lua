-- PC v2.01 bounded read-only observation. Hardware execute breakpoints only.
-- A builder call is not evidence of natural acquisition or inventory insertion.
local BASE=assert(getAddressSafe('Nioh3.exe'),'Nioh3.exe is not attached')
local ENTRY,GLOBAL=BASE+0x227C4CC,BASE+0x474D4E0
local function requireSignature(address,expected)
  local actual=readBytes(address,#expected,true)
  assert(actual and #actual==#expected,'Unreadable scroll builder signature')
  for index,value in ipairs(expected) do assert(actual[index]==value,'PC v2.01 scroll builder signature mismatch') end
end
requireSignature(ENTRY,{0x48,0x89,0x5C,0x24,0x08,0x48,0x89,0x6C,0x24,0x10,0x48,0x89,0x74,0x24,0x18,0x57,
  0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xEC,0x20,0x45,0x33,0xED,0xC7})
requireSignature(BASE+0x227C707,{0x45,0x38,0x6E,0x21,0x75,0x1A,0x48,0x8B,0x05,0xCC,0x0D,0x4D,0x02,0x48,0x8B,0x10,
  0x48,0x8B,0x4A,0x08,0x48,0x8D,0x41,0x01,0x48,0x89,0x42,0x08,0x48,0x89,0x4F,0x28})
assert(not nioh3ScrollBuilderProbe or (not nioh3ScrollBuilderProbe.active and not nioh3ScrollBuilderProbe.cleanup_pending),
  'Stop and confirm cleanup of the existing builder observer first')
local initial=debug_getBreakpointList()
assert(type(initial)=='table' and #initial==0,'Existing or unknown debugger breakpoints must be preserved')
local function hex(value) return value and string.format('0x%X',value) or nil end
local function readHex(address,count)
  if not address or address==0 then return nil end
  local ok,bytes=pcall(readBytes,address,count,true)
  if not ok or not bytes or #bytes~=count then return nil end
  local result={}
  for index,value in ipairs(bytes) do result[index]=string.format('%02X',value) end
  return table.concat(result)
end
local function pointer(address)
  if not address or address==0 then return nil end
  local ok,value=pcall(readQword,address)
  if ok then return value end
end
local function byte(address)
  local ok,value=pcall(readBytes,address,1,false)
  if ok then return value end
end
local function requireHex(address,count,label)
  return assert(readHex(address,count),'Unreadable or partial '..label)
end
local function counterState()
  local manager=assert(pointer(GLOBAL),'Unreadable item manager pointer')
  assert(manager~=0,'Item manager pointer is null')
  local data=assert(pointer(manager),'Unreadable inventory data pointer')
  assert(data~=0,'Inventory data pointer is null')
  local serial=requireHex(data+8,8,'serial counter')
  local order=requireHex(data,4,'acquisition order')
  assert(pointer(GLOBAL)==manager and pointer(manager)==data,'Inventory owner changed during counter capture')
  return {manager=hex(manager),data=hex(data),serial_counter_address=hex(data+8),
    serial_counter_le_hex=serial,acquisition_order_le_hex=order}
end
local probe={schema='nioh3-live-scroll-builder-observation/v1',active=false,
  pid=getOpenedProcessID(),module_base=hex(BASE),entry_rva='0x227C4CC',entry_hits=0,
  unmatched_return_hits=0,events={},max_hits=64,max_seconds=120,
  provenance='observed_builder_call',natural_acquisition_confirmed=false,
  action_label=type(NIOH3_BUILDER_ACTION)=='string' and NIOH3_BUILDER_ACTION or 'unspecified',
  writes_item_data=false,invokes_target_function=false}
nioh3ScrollBuilderProbe=probe
local helperPath=NIOH3_BREAKPOINT_LIFECYCLE_PATH or 'F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua'
local helperEnv={assert=assert,pairs=pairs,pcall=pcall,type=type,tostring=tostring,math=math,string=string,table=table}
local makeOwner=assert(loadfile(helperPath,'t',helperEnv))()
local owner=makeOwner({list=debug_getBreakpointList,remove=debug_removeBreakpoint,
  remove_id=debug_removeBreakpointByID,timer=createTimer,
  arm=function(address,callback) return debug_setBreakpoint(address,1,bptExecute,bpmDebugRegister,callback) end},probe)
local pending,returnAddress
function probe.retry_cleanup() return owner.retry_cleanup() end
function probe.stop(reason)
  probe.active=false
  probe.stop_reason=probe.stop_reason or reason or 'manual'
  pending=nil
  return owner.stop()
end
local function onReturn()
  local ok,failure=pcall(function()
    if not probe.active then return end
    debug_getContext(false)
    if not pending or RSP~=pending.expected_rsp then
      probe.unmatched_return_hits=probe.unmatched_return_hits+1
      if probe.unmatched_return_hits>=256 then probe.stop('unmatched_return_budget') end
      return
    end
    local output=requireHex(pending.output,0xE8,'builder output record')
    local descriptor=requireHex(pending.descriptor,0xCC,'builder descriptor after return')
    local counters=counterState()
    assert(counters.manager==pending.counter_before.manager and counters.data==pending.counter_before.data
      and counters.serial_counter_address==pending.counter_before.serial_counter_address,
      'Inventory counter owner changed between builder entry and return')
    probe.events[#probe.events+1]={kind='return',entry_sequence=pending.sequence,
      provenance='observed_builder_call',rax=hex(RAX),rsp=hex(RSP),output_address=hex(pending.output),
      rax_matches_output=RAX==pending.output,output_record_hex=output,
      descriptor_after_hex=descriptor,counter_after=counters,counter_owner_matches_entry=true}
    pending=nil
    owner.remove(returnAddress)
  end)
  if not ok then probe.error=tostring(failure);probe.stop('return_capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end
local function onEntry()
  local ok,failure=pcall(function()
    if not probe.active then return end
    debug_getContext(false)
    probe.entry_hits=probe.entry_hits+1
    assert(RCX and RCX~=0 and RDX and RDX~=0 and RSP and RSP~=0,'Builder argument or stack pointer is null')
    local caller=assert(pointer(RSP),'Unreadable builder return address')
    assert(caller~=0,'Builder return address is null')
    local descriptor=requireHex(RDX,0xCC,'builder descriptor')
    local skip=assert(byte(RDX+0x21),'Unreadable serial allocation flag')
    local descriptorByte22=assert(byte(RDX+0x22),'Unreadable descriptor byte 0x22')
    assert(string.format('%02X',skip)==descriptor:sub(0x21*2+1,0x21*2+2)
      and string.format('%02X',descriptorByte22)==descriptor:sub(0x22*2+1,0x22*2+2),
      'Builder descriptor changed during capture')
    local counters=counterState()
    local event={kind='entry',sequence=probe.entry_hits,provenance='observed_builder_call',
      output_address=hex(RCX),descriptor_address=hex(RDX),rsp=hex(RSP),return_address=hex(caller),
      caller_rva=hex(caller-BASE),descriptor_hex=descriptor,
      skip_serial_allocation=skip,descriptor_byte_22=descriptorByte22,
      counter_before=counters,stack_hex=readHex(RSP,0x80)}
    probe.events[#probe.events+1]=event
    if not pending and not owner.contains(returnAddress) and caller then
      pending={sequence=probe.entry_hits,expected_rsp=RSP+8,output=RCX,descriptor=RDX,counter_before=counters}
      returnAddress=caller
      local armed=owner.arm(caller,onReturn)
      if not armed then probe.return_arm_failed=true;probe.stop('return_arm_failed') end
    else
      event.return_capture_skipped=true
    end
    if probe.entry_hits>=probe.max_hits then probe.stop('hit_budget') end
  end)
  if not ok then probe.error=tostring(failure);probe.stop('entry_capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Could not attach Windows debugger')
local armed=owner.arm(ENTRY,onEntry)
if not armed then probe.stop('entry_arm_failed');error('Builder observer arm failed; inspect cleanup_pending') end
probe.active=true
local timerOk,timerFailure=pcall(createTimer,probe.max_seconds*1000,function()
  if probe.active then probe.stop('time_budget') end
end)
if not timerOk then probe.error=tostring(timerFailure);probe.stop('budget_timer_failed') end
return {active=probe.active,pid=probe.pid,entry_rva=probe.entry_rva,max_hits=probe.max_hits,
  max_seconds=probe.max_seconds,cleanup_pending=probe.cleanup_pending,owned_breakpoints=probe.owned_breakpoints}
