-- PC v2.01 read-only observer of game-owned per-slot completion wrapper calls.
-- ABI at 0x227DCD0: RCX output, RDX source, R8D slot 0..6, R9B reveal.
-- Hardware execution breakpoints only; no target calls, patches, or register writes.
local BASE=assert(getAddressSafe('Nioh3.exe'),'Nioh3.exe is not attached')
local ENTRY,GLOBAL=BASE+0x227DCD0,BASE+0x474D4E0
local signature={0x40,0x53,0x55,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x81,0xEC,0x78,0x01,0x00,0x00}
local actual=readBytes(ENTRY,#signature,true)
assert(actual and #actual==#signature,'Unreadable finalizer entry signature')
for i,v in ipairs(signature) do assert(actual[i]==v,'PC v2.01 finalizer signature mismatch') end
assert(not nioh3ScrollFinalizerProbe or (not nioh3ScrollFinalizerProbe.active and
  not nioh3ScrollFinalizerProbe.cleanup_pending and #nioh3ScrollFinalizerProbe.owned_breakpoints==0),
  'Existing finalizer observer must finish cleanup first')
local initial=debug_getBreakpointList()
assert(type(initial)=='table' and #initial==0,'Preserve existing or unknown debugger breakpoints')
local config=NIOH3_FINALIZER_OPTIONS or {}
assert(type(config)=='table','Finalizer options must be a table')
local function bounded(value,default,maximum)
  value=value or default
  assert(type(value)=='number' and value%1==0 and value>=1 and value<=maximum,'Invalid observer budget')
  return value
end
local filter=config.serial_le_hex
if filter~=nil then
  assert(type(filter)=='string' and #filter==16 and filter:match('^%x+$'),'Serial filter must contain exactly eight little-endian hex bytes')
  filter=filter:upper()
end
local function hex(value) return value and string.format('0x%X',value) or nil end
local function pointer(address)
  assert(type(address)=='number' and address~=0,'Null pointer location')
  local value=readQword(address)
  assert(type(value)=='number' and value~=0,'Unreadable or null pointer')
  return value
end
local function readHex(address,count)
  assert(type(address)=='number' and address~=0,'Null record address')
  local bytes=readBytes(address,count,true)
  assert(type(bytes)=='table' and #bytes==count,'Unreadable or partial record')
  local out={}
  for i,v in ipairs(bytes) do out[i]=string.format('%02X',v) end
  return table.concat(out)
end
local function stableHex(address,count)
  local value=readHex(address,count)
  assert(value==readHex(address,count),'Record changed during double-read')
  return value
end
local function serial(value) return value:sub(0x28*2+1,0x30*2) end
local function byte(value,offset) return tonumber(value:sub(offset*2+1,offset*2+2),16) end
local pid=getOpenedProcessID()
local function inventoryOwner()
  assert(getOpenedProcessID()==pid,'Attached process changed')
  local manager=pointer(GLOBAL)
  local data=pointer(manager)
  local counter=stableHex(data+8,8)
  assert(pointer(GLOBAL)==manager and pointer(manager)==data,'Inventory owner changed during read')
  return {manager=hex(manager),data=hex(data),serial_counter_le_hex=counter}
end
-- CE has no documented current-target-thread getter. Never use GetCurrentThreadID.
-- Optional externally obtained TEB rows are revalidated against live PID/TID/Self.
local stackRows=config.stack_owners or {}
assert(type(stackRows)=='table' and #stackRows<=1024,'Invalid stack owner table')
local function threadOwner(sp)
  local result={status='unresolved',rsp=hex(sp)}
  for _,row in ipairs(stackRows) do
    local ok,found=pcall(function()
      local teb=tonumber(row.teb)
      local tid=tonumber(row.thread_id)
      if not teb or not tid then return nil end
      if pointer(teb+0x30)~=teb or pointer(teb+0x40)~=pid or pointer(teb+0x48)~=tid then return nil end
      local base,limit=pointer(teb+8),pointer(teb+0x10)
      if limit<=sp and sp<base then
        return {status='validated_teb_stack',thread_id=tid,teb=hex(teb),stack_base=hex(base),stack_limit=hex(limit),rsp=hex(sp)}
      end
    end)
    if ok and found then
      if result.thread_id then return {status='ambiguous',rsp=hex(sp)} end
      result=found
    end
  end
  return result
end
local probe={schema='nioh3-live-scroll-finalizer-observation/v1',active=false,accepting=false,
  pid=pid,module_base=hex(BASE),entry_rva='0x227DCD0',entry_hits=0,matched_entries=0,
  unmatched_return_hits=0,filtered_entries=0,events={},serial_filter_le_hex=filter,
  max_hits=bounded(config.max_hits,256,4096),max_seconds=bounded(config.max_seconds,300,3600),
  max_matches=bounded(config.max_matches,64,128),
  action_label=config.action_label or 'unspecified game-owned completion observation',
  writes_item_data=false,writes_target_code=false,invokes_target_function=false,
  scope='Per-slot candidate observations; accepted outer-loop result still requires inventory/UI comparison'}
nioh3ScrollFinalizerProbe=probe
local helperPath=NIOH3_BREAKPOINT_LIFECYCLE_PATH or 'F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua'
local helperEnv={assert=assert,pairs=pairs,pcall=pcall,type=type,tostring=tostring,math=math,string=string,table=table}
local makeOwner=assert(loadfile(helperPath,'t',helperEnv))()
local owner=makeOwner({list=debug_getBreakpointList,remove=debug_removeBreakpoint,
  remove_id=debug_removeBreakpointByID,timer=createTimer,
  arm=function(address,callback) return debug_setBreakpoint(address,1,bptExecute,bpmDebugRegister,callback) end},probe)
local pending,returnAddress
function probe.retry_cleanup() return owner.retry_cleanup() end
function probe.stop(reason)
  probe.active,probe.accepting=false,false
  probe.stop_reason=probe.stop_reason or reason or 'manual'
  if pending then pending.event.return_capture_status='incomplete';pending.event.incomplete_reason=probe.stop_reason end
  pending=nil
  return owner.stop()
end
local function drain(reason)
  probe.accepting=false
  probe.drain_reason=reason
  owner.remove(ENTRY)
  if not pending then probe.stop(reason) end
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
    local sample=pending
    local thread=threadOwner(RSP)
    if sample.thread.thread_id and thread.thread_id then assert(sample.thread.thread_id==thread.thread_id,'Thread owner changed between entry and return') end
    local after=inventoryOwner()
    assert(after.manager==sample.inventory.manager and after.data==sample.inventory.data,'Inventory owner changed between entry and return')
    local output=stableHex(sample.output,0xE8)
    local source=stableHex(sample.source,0xE8)
    assert(RAX==sample.output,'Finalizer return RAX does not identify the supplied output buffer')
    assert(serial(source)==sample.serial and serial(output)==sample.serial,'Finalizer source/output serial changed')
    local flags=byte(output,0x42+sample.slot*24)
    probe.events[#probe.events+1]={kind='return',entry_sequence=sample.event.sequence,
      caller_rva=sample.event.caller_rva,rsp=hex(RSP),thread=thread,rax=hex(RAX),rax_matches_output=true,
      source_after_hex=source,output_record_hex=output,output_address=hex(sample.output),
      inventory_owner=after,source_unchanged=source==sample.event.source_record_hex,
      effect_index=sample.slot,reveal_argument_byte=sample.event.reveal_argument_byte,
      selected_slot_effect_flags=flags,candidate_completed_flag_set=math.floor(flags/4)%2==1,
      scope='Candidate completed bit is observed; outer-loop acceptance is not inferred'}
    sample.event.return_capture_status='paired'
    pending=nil
    -- Retain the return site across the rapid outer-loop calls from one caller.
    -- Removing it here can be deferred by CE and lose the next eligible slot.
    if not probe.accepting then probe.stop(probe.drain_reason) end
  end)
  if not ok then probe.error=tostring(failure);probe.stop('return_capture_error') end
  debug_continueFromBreakpoint(co_run)
  return 1
end
local function onEntry()
  local ok,failure=pcall(function()
    if not probe.active or not probe.accepting then return end
    debug_getContext(false)
    probe.entry_hits=probe.entry_hits+1
    assert(RCX and RCX~=0 and RDX and RDX~=0 and RSP and RSP~=0,'Null finalizer argument')
    local source=stableHex(RDX,0xE8)
    local matches=filter==nil or serial(source)==filter
    if not matches then
      probe.filtered_entries=probe.filtered_entries+1
      if probe.entry_hits>=probe.max_hits then drain('hit_budget') end
      return
    end
    local slot=R8%4294967296
    assert(slot<=6,'Unexpected signed R8D effect index')
    local reveal=R9%256
    local caller=pointer(RSP)
    local inventory=inventoryOwner()
    local thread=threadOwner(RSP)
    local event={kind='entry',sequence=probe.entry_hits,effect_index=slot,reveal_argument_byte=reveal,
      reveal_nonzero=reveal~=0,r8_raw=hex(R8),r9_raw=hex(R9),source_address=hex(RDX),output_address=hex(RCX),
      source_record_hex=source,output_buffer_before_hex=stableHex(RCX,0xE8),
      output_before_is_final_result=false,source_serial_le_hex=serial(source),
      rsp=hex(RSP),thread=thread,return_address=hex(caller),caller_rva=hex(caller-BASE),
      inventory_owner=inventory,stack_hex=stableHex(RSP,0x80),return_capture_status='pending'}
    probe.events[#probe.events+1]=event
    probe.matched_entries=probe.matched_entries+1
    if pending then
      event.return_capture_status='skipped_pending_pair'
    else
      if owner.contains(returnAddress) and returnAddress~=caller then owner.remove(returnAddress) end
      if probe.cleanup_pending then owner.retry_cleanup() end
      if probe.cleanup_pending or (owner.contains(returnAddress) and returnAddress~=caller) then
        event.return_capture_status='skipped_pending_cleanup'
      else
        local retained=owner.contains(returnAddress) and returnAddress==caller
        if not retained then
          -- Do not let owner.arm adopt/remove an unrelated breakpoint at this site.
          local sites=debug_getBreakpointList()
          assert(type(sites)=='table','Cannot check return breakpoint ownership')
          for _,address in pairs(sites) do assert(address~=caller,'Return address already has a breakpoint') end
        end
        pending={event=event,expected_rsp=RSP+8,source=RDX,output=RCX,slot=slot,
          inventory=inventory,thread=thread,serial=serial(source)}
        event.retained_return_site=retained
        returnAddress=caller
        if not retained and not owner.arm(caller,onReturn) then probe.stop('return_arm_failed') end
      end
    end
    if probe.active and (probe.entry_hits>=probe.max_hits or probe.matched_entries>=probe.max_matches) then
      drain(probe.entry_hits>=probe.max_hits and 'hit_budget' or 'match_budget')
    end
  end)
  if not ok then
    local last=probe.events[#probe.events]
    if last and last.kind=='entry' and last.return_capture_status=='pending' and not pending then
      last.return_capture_status='incomplete';last.incomplete_reason='entry_capture_error'
    end
    probe.error=tostring(failure);probe.stop('entry_capture_error')
  end
  debug_continueFromBreakpoint(co_run)
  return 1
end
if not debug_isDebugging() then debugProcess(1) end
assert(debug_isDebugging(),'Could not attach Windows debugger')
if not owner.arm(ENTRY,onEntry) then probe.stop('entry_arm_failed');error('Finalizer observer arm failed; inspect cleanup_pending') end
probe.active,probe.accepting=true,true
local ok,failure=pcall(createTimer,probe.max_seconds*1000,function()
  if probe.active then probe.stop('time_budget') end
end)
if not ok then probe.error=tostring(failure);probe.stop('budget_timer_failed') end
return {active=probe.active,pid=pid,entry_rva=probe.entry_rva,max_seconds=probe.max_seconds,
  max_hits=probe.max_hits,serial_filter_le_hex=filter,cleanup_pending=probe.cleanup_pending,
  owned_breakpoints=probe.owned_breakpoints}
