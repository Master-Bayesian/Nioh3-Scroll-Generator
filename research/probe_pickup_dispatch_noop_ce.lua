-- One-shot research dispatch: no-call, native query, isolated assembly, or one
-- explicitly planned insertion. The last mode mutates inventory through native
-- routines. Original executable bytes stay intact in every mode.
local source=debug.getinfo(1,'S').source
local root=assert(source:match('^@(.+[/\\])'),'Cannot resolve executor directory'):gsub('\\','/')
local layout=dofile(root..'live_add_layout_ce.lua')
local base=assert(getAddressSafe('Nioh3.exe'))
local pid=getOpenedProcessID()
local readSlot=nioh3DispatchProbeOptions and nioh3DispatchProbeOptions.read_container_slot==true
local preview=nioh3DispatchProbeOptions and nioh3DispatchProbeOptions.assembly_preview
local insertion=nioh3DispatchProbeOptions and nioh3DispatchProbeOptions.single_insertion
assert(not insertion or preview,'Insertion requires the verified assembly descriptor')
assert(not (readSlot and preview),'Choose one probe mode')
local function parseHex(value,size)
  assert(type(value)=='string' and #value==size*2 and not value:find('[^%x]'),'Invalid bounded byte string')
  local bytes={};for pair in value:gmatch('..') do bytes[#bytes+1]=tonumber(pair,16) end;return bytes
end
local entry=base+layout.dispatch_rva
local original=parseHex(layout.dispatch_signature_hex,7)
local function verifyBytes(address,expected)
  local actual=assert(readBytes(address,#expected,true),'Unreadable instruction range')
  assert(#actual==#expected,'Partial instruction range')
  for i,v in ipairs(expected) do assert(actual[i]==v,'Instruction identity mismatch') end
end
verifyBytes(entry,original)
assert(debug_isDebugging(),'Debugger must already be attached')
local listed=debug_getBreakpointList()
assert(type(listed)=='table' and next(listed)==nil,'Existing or unknown breakpoints')
assert(not nioh3DispatchNoop or nioh3DispatchNoop.released,'Previous allocation requires review')
local manager=assert(readQword(base+layout.manager_pointer_rva))
local data=assert(readQword(manager))
assert(manager~=0 and data~=0 and readQword(data+layout.container_offset+layout.capacity_offset)==400,'Invalid inventory owner')
local container=data+layout.container_offset
if readSlot then
  verifyBytes(base+layout.slot_lookup_rva,{0x8B,0xC2,0x48,0x3B,0x81,0x80,0x6A,0x01,0x00,0x72,0x03,0x33,0xC0,0xC3,
    0x48,0x69,0xC0,0xE8,0,0,0,0x48,0x03,0xC1,0xC3})
end
local descriptor,expectedRecord
if preview then
  descriptor=parseHex(preview.descriptor_hex,0xCC)
  expectedRecord=parseHex(preview.expected_record_hex,0xE8)
  assert(descriptor[0x21+1]==(insertion and 0 or 1),'Unexpected serial-allocation descriptor')
  assert(descriptor[0x10+1]+descriptor[0x11+1]+descriptor[0x12+1]+descriptor[0x13+1]>0,'Preview must supply a seed')
  verifyBytes(base+layout.builder_rva,parseHex(preview.builder_code_hex,layout.builder_size))
end
local plannedContainer
if insertion then insertion.serial_counter_offset=layout.serial_counter_offset end
if insertion then
  assert(insertion.pid==pid and insertion.manager==manager and insertion.data==data,'Insertion owner changed')
  assert(type(insertion.serial)=='number' and insertion.serial>0 and insertion.serial<0x7FFFFFFFFFFFFFFE,'Invalid expected serial')
  assert(readQword(data+layout.serial_counter_offset)==insertion.serial,'Serial changed since planning')
  assert(insertion.function_address==base+layout.insertion_rva,'Unexpected insertion target')
  assert(insertion.slot>=0 and insertion.slot<400 and insertion.slot%1==0,'Invalid planned slot')
  assert(type(insertion.operation_id)=='string' and #insertion.operation_id>=16,'Missing operation identity')
  nioh3InsertionAttempts=nioh3InsertionAttempts or {}
  assert(not nioh3InsertionAttempts[insertion.operation_id],'Operation already attempted')
  plannedContainer=parseHex(insertion.container_hex,400*0xE8)
  verifyBytes(container,plannedContainer)
  assert(plannedContainer[insertion.slot*0xE8+1]==0 and plannedContainer[insertion.slot*0xE8+2]==0,'Planned slot is occupied')
  verifyBytes(base+layout.insertion_rva,parseHex(insertion.insertion_code_hex,layout.insertion_size))
end
-- Resolve local dependencies before acquiring a remote allocation.
local buildCode=dofile(root..'build_dispatch_probe_code.lua')
local makeOwner=dofile(root..'owned_breakpoint_lifecycle_ce.lua')
local schedule=dofile(root..'ce_main_thread_timer.lua')
local memory=assert(allocateMemory(4096,nil,0x40),'Probe allocation failed')
assert(memory~=0,'Null allocation')
local probe={pid=pid,phase='preparing',active=false,allocation=memory,allocation_size=4096,
  started_at=os.time(),item_mutation=false,redirect_count=0,original_code_changed=false,
  mode=insertion and 'single_native_insertion' or preview and 'assembly_preview' or readSlot and 'read_only_slot_lookup' or 'no_call',expected_result=readSlot and container or nil}
if insertion then probe.item_mutation=true;probe.operation_id=insertion.operation_id end
nioh3DispatchNoop=probe
local function hex(v) return v and string.format('0x%X',v) or nil end
local target=entry+#original
local code=buildCode(memory,target,original,preview and base+layout.builder_rva or readSlot and base+layout.slot_lookup_rva or nil,
  preview and memory+0x600 or container,preview and memory+0x400 or nil,insertion)
local owner=makeOwner({list=function() return debug_getBreakpointList() end,
  remove=function(address) return debug_removeBreakpoint(address) end,
  timer=schedule,
  arm=function(address,callback) return debug_setBreakpoint(address,1,bptExecute,bpmDebugRegister,callback) end},probe)
function probe.stop(reason)
  probe.active=false;probe.stop_reason=reason or 'manual';probe.stopped_at=os.time()
  return owner.stop()
end
function probe.retry_cleanup() return owner.retry_cleanup() end
function probe.release()
  if probe.released then return true end
  assert(getOpenedProcessID()==pid,'Process changed; do not release an address in another process')
  owner.retry_cleanup()
  assert(not probe.active and not probe.cleanup_pending and #probe.owned_breakpoints==0,'Cleanup unconfirmed')
  assert(probe.phase=='completed' or probe.redirect_count==0,'Execution completion unconfirmed; retain allocation')
  assert(deAlloc(memory),'Allocation release failed')
  probe.released=true
  return true
end
local registers={'RAX','RBX','RCX','RDX','RSI','RDI','RBP','R8','R9','R10','R11','R12','R13','R14','R15'}
local function context()
  local value={RSP=RSP,RIP=RIP,EFLAGS=EFLAGS}
  for _,name in ipairs(registers) do value[name]=assert(_G[name],'Missing register '..name) end
  return value
end
local function guarded(callback)
  return function()
    local ok,err=pcall(callback)
    if not ok then probe.error=tostring(err);probe.stop('capture_error') end
    debug_continueFromBreakpoint(co_run)
    return 1
  end
end
local function onAck()
  if not probe.active or probe.phase~='redirected' then return end
  debug_getContext(false)
  if RSP~=probe.before.RSP-0x48 or RCX~=probe.before.RCX then return end
  assert(RIP==target and readInteger(memory+0x300)==1,'Probe execution not acknowledged')
  local after=context()
  for _,name in ipairs(registers) do assert(after[name]==probe.before[name],'Register changed: '..name) end
  assert(readQword(RSP+0x38)==probe.before.RDI and readQword(RSP+0x40)==probe.before.RBX,'Prologue saved registers mismatch')
  assert(readQword(RSP+0x48)==probe.return_address,'Caller return address changed')
  if readSlot then
    probe.actual_result=readQword(memory+0x308)
    assert(probe.actual_result==container,'Native slot lookup returned an unexpected pointer')
  end
  if preview then
    assert(readQword(memory+0x308)==memory+0x600,'Builder returned another pointer')
    verifyBytes(memory+0x400,descriptor)
    verifyBytes(memory+0x5F0,probe.canary);verifyBytes(memory+0x6E8,probe.canary)
    probe.output_record=assert(readBytes(memory+0x600,0xE8,true))
    assert(#probe.output_record==0xE8,'Incomplete preview output')
    for offset=0,0xE7 do
      if offset>=0x28 and offset<0x30 then
        local expected=insertion and ((insertion.serial>>(8*(offset-0x28)))&0xFF) or 0xFF
        assert(probe.output_record[offset+1]==expected,'Unexpected builder serial')
      elseif not (offset>=0x24 and offset<0x28 or offset>=0xE4) then
        assert(probe.output_record[offset+1]==expectedRecord[offset+1],'Preview differs at offset '..offset)
      end
    end
    probe.preview_matches_natural_defined_fields=true
    if insertion then
      probe.insertion_status=readInteger(memory+0x318)
      probe.inserted_slot=readInteger(memory+0x320)
      probe.insertion_return=readQword(memory+0x328)
      probe.remainder=assert(readBytes(memory+0x800,0xE8,true))
      assert(probe.insertion_status==3 and probe.inserted_slot==insertion.slot,'Insertion did not acknowledge the planned slot')
      assert(probe.insertion_return==memory+0x800,'Insertion returned another buffer')
      assert(probe.remainder[1]==0 and probe.remainder[2]==0,'Insertion left a remainder')
      assert(readQword(data+layout.serial_counter_offset)==insertion.serial+1,'Unexpected serial-counter result')
      verifyBytes(memory+0x7F0,probe.canary);verifyBytes(memory+0x8E8,probe.canary)
      probe.destination=assert(readBytes(container+insertion.slot*0xE8,0xE8,true))
      assert(readQword(container+insertion.slot*0xE8+0x28)==insertion.serial,'Destination serial differs')
      local afterContainer=assert(readBytes(container,400*0xE8,true))
      assert(#afterContainer==#plannedContainer,'Partial post-insertion container')
      for offset=0,#plannedContainer-1 do
        if offset//0xE8~=insertion.slot then assert(afterContainer[offset+1]==plannedContainer[offset+1],'Another inventory slot changed') end
      end
      probe.other_slots_unchanged=true
    else verifyBytes(data+layout.serial_counter_offset,probe.serial_before) end
  end
  probe.after=after;probe.phase='completed';probe.completed_at=os.time()
  probe.stop('acknowledged')
end
local function onEntry()
  if not probe.active or probe.phase~='armed' then return end
  debug_getContext(false)
  assert(getOpenedProcessID()==pid and RIP==entry,'Process or entry changed')
  assert(RSP%16==8,'Unexpected entry stack alignment')
  assert(readQword(base+layout.manager_pointer_rva)==manager and readQword(manager)==data,'Inventory owner changed')
  if insertion then
    assert(readQword(data+layout.serial_counter_offset)==insertion.serial,'Serial changed before dispatch')
    verifyBytes(container,plannedContainer)
    local scheduler=assert(readQword(base+layout.scheduler_pointer_rva))
    assert(scheduler==insertion.scheduler_owner,'Task owner changed')
    verifyBytes(scheduler+layout.scheduler_pending_offset,{0,0,0,0})
    verifyBytes(scheduler+layout.scheduler_ready_offset,{1})
  end
  if preview then verifyBytes(memory+0x400,descriptor) end
  assert(readQword(RCX+layout.queue_begin_offset)==readQword(RCX+layout.queue_end_offset),'Pickup queue must be empty')
  probe.before=context();probe.return_address=assert(readQword(RSP))
  if preview then probe.serial_before=assert(readBytes(data+layout.serial_counter_offset,8,true));assert(#probe.serial_before==8,'Unreadable serial') end
  assert(probe.return_address==base+layout.dispatch_return_rva,'Unexpected dispatch caller')
  probe.phase='redirected';probe.redirect_count=1
  if insertion then nioh3InsertionAttempts[insertion.operation_id]=true end
  -- No fallible work after changing RIP and before resuming the game.
  RIP=memory
  debug_setContext(false)
end
local ok,err=pcall(function()
  writeBytes(memory,code);writeBytes(memory+0x300,{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0})
  if preview then
    writeBytes(memory+0x400,descriptor)
    local zero={};probe.canary={}
    for i=1,0xE8 do zero[i]=0 end;for i=1,16 do probe.canary[i]=0xA5 end
    writeBytes(memory+0x600,zero);writeBytes(memory+0x5F0,probe.canary);writeBytes(memory+0x6E8,probe.canary)
    if insertion then
      writeBytes(memory+0x800,zero);writeBytes(memory+0x7F0,probe.canary);writeBytes(memory+0x8E8,probe.canary)
      writeBytes(memory+0x318,{0,0,0,0});writeBytes(memory+0x320,{255,255,255,255})
    end
  end
  verifyBytes(memory,code)
  assert(readInteger(memory+0x300)==0,'Probe flag initialization failed')
  probe.code_hex={};for i,v in ipairs(code) do probe.code_hex[i]=string.format('%02X',v) end
  probe.code_hex=table.concat(probe.code_hex)
  assert(owner.arm(target,guarded(onAck)),'Could not arm acknowledgement')
  probe.active=true;probe.phase='armed'
  assert(owner.arm(entry,guarded(onEntry)),'Could not arm entry')
  schedule(10000,function() if probe.active then probe.stop('time_budget') end end)
end)
if not ok then probe.error=tostring(err);probe.stop('setup_failed');error(err) end
