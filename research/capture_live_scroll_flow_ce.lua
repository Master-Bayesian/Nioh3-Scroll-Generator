-- PC v2.01 combined builder/insertion observer. No target invocation or item writes.
-- Two entry sites and one retained return site per channel: at most four HW sites.
local BASE=assert(getAddressSafe('Nioh3.exe'),'Nioh3.exe is not attached')
local PID=assert(getOpenedProcessID(),'No attached process')
local GLOBAL=BASE+0x474D4E0
local function requireSignature(rva,expected)
  local actual=readBytes(BASE+rva,#expected,true)
  assert(actual and #actual==#expected,'Unreadable scroll-flow signature')
  for index,value in ipairs(expected) do assert(actual[index]==value,'PC v2.01 scroll-flow signature mismatch') end
end
requireSignature(0x227C4CC,{0x48,0x89,0x5C,0x24,0x08,0x48,0x89,0x6C,0x24,0x10,0x48,0x89,0x74,0x24,0x18,0x57,
  0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xEC,0x20,0x45,0x33,0xED,0xC7})
requireSignature(0x227C707,{0x45,0x38,0x6E,0x21,0x75,0x1A,0x48,0x8B,0x05,0xCC,0x0D,0x4D,0x02,0x48,0x8B,0x10,
  0x48,0x8B,0x4A,0x08,0x48,0x8D,0x41,0x01,0x48,0x89,0x42,0x08,0x48,0x89,0x4F,0x28})
requireSignature(0x54D294,{0x40,0x55,0x53,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x8D,0xAC})
requireSignature(0x54DC33,{0x41,0x80,0xF9,0x12,0x0F,0x85,0xB8,0x00,0x00,0x00,0x48,0x81,0xC1,0x60,0x4A,0x22,
  0x00,0x41,0x83,0xCF,0xFF,0xE8,0x6F,0x53})
assert(not nioh3ScrollFlowProbe or (not nioh3ScrollFlowProbe.active and not nioh3ScrollFlowProbe.cleanup_pending),
  'Stop and confirm cleanup of the existing combined observer first')
local initial=debug_getBreakpointList()
assert(type(initial)=='table' and next(initial)==nil,'Existing or unknown debugger breakpoints must be preserved')
local function hex(value) return value and string.format('0x%X',value) or nil end
local function requiredPointer(address,label)
  assert(address and address~=0,'Null address for '..label)
  local ok,value=pcall(readQword,address)
  assert(ok and type(value)=='number' and value~=0,'Unreadable or null '..label)
  return value
end
local function requiredHex(address,count,label)
  assert(address and address~=0,'Null address for '..label)
  local ok,bytes=pcall(readBytes,address,count,true)
  assert(ok and type(bytes)=='table' and #bytes==count,'Unreadable or partial '..label)
  local result={}
  for index=1,count do
    local value=bytes[index]
    assert(type(value)=='number' and value>=0 and value<=255 and value%1==0,'Invalid byte in '..label)
    result[index]=string.format('%02X',value)
  end
  return table.concat(result)
end
local function rawField(record,offset,size) return record:sub(offset*2+1,(offset+size)*2) end
local function unsignedLE(record,offset,size)
  local parts={}
  for index=size-1,0,-1 do parts[#parts+1]=rawField(record,offset+index,1) end
  return assert(tonumber(table.concat(parts),16),'Invalid little-endian field')
end
local boundOwner
local function verifyProcess()
  assert(getOpenedProcessID()==PID and getAddressSafe('Nioh3.exe')==BASE,'Attached process identity changed')
end
local function counterState()
  verifyProcess()
  local manager=requiredPointer(GLOBAL,'item manager pointer')
  local data=requiredPointer(manager,'inventory data pointer')
  if boundOwner then assert(manager==boundOwner.manager and data==boundOwner.data,'Inventory owner changed') end
  local serial=requiredHex(data+8,8,'uint64 serial counter')
  local order=requiredHex(data,4,'acquisition-order counter')
  assert(requiredPointer(GLOBAL,'item manager pointer')==manager
    and requiredPointer(manager,'inventory data pointer')==data,'Inventory owner changed during capture')
  boundOwner=boundOwner or {manager=manager,data=data}
  return {manager=hex(manager),data=hex(data),serial_counter_address=hex(data+8),
    serial_counter_le_hex=serial,acquisition_order_le_hex=order}
end
local startingCounters=counterState()
local probe={schema='nioh3-live-scroll-flow-observation/v1',active=false,pid=PID,module_base=hex(BASE),
  entry_hits=0,builder_hits=0,insertion_hits=0,scroll_insertion_hits=0,unmatched_return_hits=0,
  max_hits=512,max_seconds=180,max_unmatched_returns=512,max_hardware_sites=4,events={},
  starting_counters=startingCounters,natural_acquisition_confirmed=false,
  action_label=type(NIOH3_SCROLL_FLOW_ACTION)=='string' and NIOH3_SCROLL_FLOW_ACTION or 'unspecified',
  writes_item_data=false,invokes_target_function=false}
nioh3ScrollFlowProbe=probe
local helperEnv={assert=assert,pairs=pairs,pcall=pcall,type=type,tostring=tostring,math=math,string=string,table=table}
local helper=NIOH3_BREAKPOINT_LIFECYCLE_PATH or 'F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua'
local makeOwner=assert(loadfile(helper,'t',helperEnv))()
local owner=makeOwner({list=debug_getBreakpointList,remove=debug_removeBreakpoint,
  remove_id=debug_removeBreakpointByID,timer=createTimer,
  arm=function(address,callback) return debug_setBreakpoint(address,1,bptExecute,bpmDebugRegister,callback) end},probe)
local channels={
  {name='builder',entry=BASE+0x227C4CC,provenance='observed_builder_call'},
  {name='insertion',entry=BASE+0x54D294,provenance='observed_insertion_call'}}
local scrollTypes={[0x1E82]=true,[0x516D]=true,[0xE604]=true,[0xDD82]=true,[0xD523]=true}
function probe.retry_cleanup() return owner.retry_cleanup() end
function probe.stop(reason)
  probe.active=false
  probe.stop_reason=probe.stop_reason or reason or 'manual'
  for _,channel in ipairs(channels) do
    if channel.pending then
      local sample=channel.pending
      sample.event.return_capture_status='unpaired'
      probe.events[#probe.events+1]={kind='unpaired',channel=channel.name,entry_sequence=sample.sequence,
        provenance=channel.provenance,reason=probe.stop_reason,return_address=hex(channel.return_address)}
      channel.pending=nil
    end
  end
  return owner.stop()
end
local function safeArm(address,callback)
  -- Inspect before reserving ownership: a rejected foreign address must never
  -- become a site that the cleanup helper might remove.
  local listed=debug_getBreakpointList()
  assert(type(listed)=='table','Breakpoint inventory unavailable before arm')
  local allowed,count={},0
  for _,ownedAddress in ipairs(probe.owned_breakpoints) do allowed[tonumber(ownedAddress:sub(3),16)]=true end
  for _,present in pairs(listed) do
    assert(allowed[present],'Foreign breakpoint appeared during capture')
    assert(present~=address,'Breakpoint address already in use')
    count=count+1
  end
  assert(count<4 and #probe.owned_breakpoints<4,'Four hardware sites already owned')
  return owner.arm(address,callback)
end
local function errorStop(channel,phase,failure)
  probe.error=tostring(failure)
  probe.events[#probe.events+1]={kind='capture_error',channel=channel.name,phase=phase,error=probe.error,
    entry_sequence=channel.pending and channel.pending.sequence or nil}
  probe.stop(phase..'_capture_error')
end
local function finishReturn(channel)
  if not probe.active then return end
  verifyProcess()
  debug_getContext(false)
  local sample=channel.pending
  if not sample or RSP~=sample.expected_rsp then
    probe.unmatched_return_hits=probe.unmatched_return_hits+1
    if probe.unmatched_return_hits>=probe.max_unmatched_returns then probe.stop('unmatched_return_budget') end
    return
  end
  local output=requiredHex(sample.output,0xE8,'return output record')
  local counters=counterState()
  local event={kind='return',channel=channel.name,entry_sequence=sample.sequence,provenance=channel.provenance,
    rax=hex(RAX),rsp=hex(RSP),output_address=hex(sample.output),rax_matches_output=RAX==sample.output,
    output_record_hex=output,output_serial_le_hex=rawField(output,0x28,8),counter_after=counters,
    counter_owner_matches_entry=counters.manager==sample.counter_before.manager and counters.data==sample.counter_before.data}
  assert(event.counter_owner_matches_entry,'Inventory owner changed between entry and return')
  if channel.name=='builder' then
    event.descriptor_after_hex=requiredHex(sample.source,0xCC,'builder descriptor after return')
  else
    event.source_after_hex=requiredHex(sample.source,0xE8,'insertion source after return')
    event.output_slot_index_address=hex(sample.slot_output)
    if sample.slot_output~=0 then
      local bytes=requiredHex(sample.slot_output,4,'optional slot-index output')
      local index=unsignedLE(bytes,0,4)
      event.output_slot_index=index>=0x80000000 and index-0x100000000 or index
    end
  end
  -- Required reads are complete before this pair can be published.
  counterState()
  probe.events[#probe.events+1]=event
  sample.event.return_capture_status='paired'
  channel.pending=nil
  -- Keep this site's breakpoint for the next same-caller entry. Removing it
  -- after every return loses rapid pickup pairs while CE removal is deferred.
end
local function skipped(event,reason)
  event.return_capture_skipped=true
  event.return_capture_status='skipped'
  event.return_capture_skip_reason=reason
end
local function pairEntry(channel,sample,caller)
  if channel.pending then skipped(sample.event,'channel_call_pending');return end
  if channel.retiring then
    -- CE can finish removal between rapid callbacks before its Lua timer runs.
    -- Reconcile actual absence now; never infer removal from the earlier request.
    owner.retry_cleanup()
    if owner.contains(channel.return_address) then skipped(sample.event,'return_site_cleanup_pending');return end
    channel.retiring,channel.return_address=false,nil
  end
  if channel.return_address and channel.return_address~=caller then
    channel.retiring=true
    owner.remove(channel.return_address)
    if owner.contains(channel.return_address) then skipped(sample.event,'return_site_replacement_pending');return end
    channel.retiring,channel.return_address=false,nil
  end
  if not channel.return_address then
    if owner.contains(caller) then skipped(sample.event,'return_address_owned_by_other_site');return end
    local armed=safeArm(caller,channel.on_return)
    if not armed then skipped(sample.event,'return_arm_failed');probe.stop('return_arm_failed');return end
    channel.return_address=caller
  end
  assert(owner.contains(channel.return_address),'Retained return breakpoint ownership was lost')
  sample.event.return_capture_status='pending'
  channel.pending=sample
end
local function captureEntry(channel)
  if not probe.active then return end
  verifyProcess()
  debug_getContext(false)
  probe.entry_hits=probe.entry_hits+1
  probe[channel.name..'_hits']=probe[channel.name..'_hits']+1
  assert(RCX and RCX~=0 and RDX and RDX~=0 and RSP and RSP~=0,'Null entry argument or stack pointer')
  local caller=requiredPointer(RSP,'return address')
  local counters=counterState()
  local event={kind='entry',channel=channel.name,sequence=probe.entry_hits,provenance=channel.provenance,
    rcx=hex(RCX),rdx=hex(RDX),r8=hex(R8),r9=hex(R9),rsp=hex(RSP),return_address=hex(caller),
    caller_rva=hex(caller-BASE),counter_before=counters,return_capture_status='unpaired'}
  local sample={sequence=event.sequence,event=event,expected_rsp=RSP+8,counter_before=counters}
  if channel.name=='builder' then
    local descriptor=requiredHex(RDX,0xCC,'builder descriptor')
    event.output_address,event.descriptor_address=hex(RCX),hex(RDX)
    event.descriptor_hex=descriptor
    event.skip_serial_allocation=unsignedLE(descriptor,0x21,1)
    event.descriptor_byte_22=unsignedLE(descriptor,0x22,1)
    sample.output,sample.source=RCX,RDX
  else
    assert(RCX==boundOwner.manager,'Insertion manager does not match bound global owner')
    assert(R8 and R8~=0 and R9~=nil,'Null insertion source or unknown optional slot pointer')
    local source=requiredHex(R8,0xE8,'insertion source record')
    local recordType=unsignedLE(source,0,2)
    event.source_hex,event.source_serial_le_hex=source,rawField(source,0x28,8)
    event.record_type,event.is_scroll=hex(recordType),scrollTypes[recordType]==true
    event.fifth_argument=unsignedLE(requiredHex(RSP+0x28,1,'fifth argument'),0,1)
    if event.is_scroll then
      probe.scroll_insertion_hits=probe.scroll_insertion_hits+1
      event.seed=unsignedLE(source,0x20,4)
    end
    sample.output,sample.source,sample.slot_output=RDX,R8,R9
  end
  counterState()
  probe.events[#probe.events+1]=event
  pairEntry(channel,sample,caller)
  if probe.entry_hits>=probe.max_hits then probe.stop('hit_budget') end
end
for _,channel in ipairs(channels) do
  local channel=channel
  channel.on_return=function()
    local ok,failure=pcall(finishReturn,channel)
    if not ok then errorStop(channel,'return',failure) end
    debug_continueFromBreakpoint(co_run)
    return 1
  end
  channel.on_entry=function()
    local ok,failure=pcall(captureEntry,channel)
    if not ok then errorStop(channel,'entry',failure) end
    debug_continueFromBreakpoint(co_run)
    return 1
  end
end
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Could not attach Windows debugger')
local armOk,armFailure=pcall(function()
  for _,channel in ipairs(channels) do assert(safeArm(channel.entry,channel.on_entry),'Entry arm failed') end
end)
if not armOk then probe.error=tostring(armFailure);probe.stop('entry_arm_failed');error(probe.error) end
probe.active=true
local timerOk,timerFailure=pcall(createTimer,probe.max_seconds*1000,function()
  if probe.active then probe.stop('time_budget') end
end)
if not timerOk then probe.error=tostring(timerFailure);probe.stop('budget_timer_failed') end
return {active=probe.active,pid=probe.pid,module_base=probe.module_base,max_hits=probe.max_hits,
  max_seconds=probe.max_seconds,max_hardware_sites=4,cleanup_pending=probe.cleanup_pending,
  owned_breakpoints=probe.owned_breakpoints}
