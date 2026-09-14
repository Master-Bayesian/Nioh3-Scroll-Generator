-- Phase 2: both selector passes, complete MT state, candidates, masks, outcomes.
local SITES = {
  entry = {rva=0xE66815, expected={0x85,0xD2}},
  pass0 = {rva=0xE6694C, expected={0xE8,0x9F,0x44,0xFD,0xFF}},
  pass1 = {rva=0xE66958, expected={0xE8,0x93,0x44,0xFD,0xFF}},
  final = {rva=0xE669EC, expected={0x48,0x8B,0x9C,0x24,0x28,0x14,0x00,0x00}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-selector-summary/v3', SITES, 180, 16)
local p = api.probe
local parameterAddress
local function captureRecord(record, index)
  if record == 0 then return {index=index,record_address='0x0'} end
  local object0 = api.u64(record,'record+0',true)
  local object18 = api.u64(record+0x18,'record+0x18',true)
  local fieldE9 = api.u8(record+0xE9,'record+0xE9')
  return {
    index=index,record_address=api.hex(record),record_raw_hex=api.bytesHex(record,0xF0,'record'),
    object0_address=api.hex(object0),
    object0_raw_hex=object0 ~= 0 and api.bytesHex(object0,0x100,'record object0') or nil,
    object18_address=api.hex(object18),
    object18_raw_hex=object18 ~= 0 and api.bytesHex(object18,0xA8,'record object18') or nil,
    spawn_id=api.hex(api.u32(record+0x20,'spawn id')),
    mission_key=api.hex(api.u32(record+0x24,'mission key')),
    enemy_lookup_key=api.hex(api.u32(record+0x28,'enemy lookup key')),
    field_80=api.u8(record+0x80,'record+0x80'),
    field_8e=api.u8(record+0x8E,'record+0x8E'), field_8f=api.u8(record+0x8F,'record+0x8F'),
    selector_class=api.u8(record+0x90,'selector class'), assigned_index=api.u8(record+0x95,'assigned index'),
    field_df=api.u8(record+0xDF,'record+0xDF'), candidate=fieldE9, field_e9=fieldE9,
    field_ea=api.u8(record+0xEA,'record+0xEA'),
  }
end
local function captureOwner(owner, label)
  assert(owner and owner ~= 0, 'Null candidate owner at ' .. label)
  local begin = api.u64(owner,'record vector begin',true)
  local count = api.u64(owner+8,'record vector count',true)
  assert(count <= 512, 'Record vector count exceeds bound')
  local entries = {}
  for index=0,count-1 do
    local entry = begin + index*0x10
    local record = api.u64(entry+8,'record pointer',true)
    entries[#entries+1] = {entry_address=api.hex(entry),entry_raw_hex=api.bytesHex(entry,0x10,'record entry'),
      record=captureRecord(record,index)}
  end
  return {owner_address=api.hex(owner),owner_raw_hex=api.bytesHex(owner,0x1D0,'candidate owner'),
    vector_begin=api.hex(begin),vector_count=count,entries=entries,
    selection_mask_hex=api.bytesHex(owner+0x1C4,12,'selection mask')}
end
local function captureSelector(label, param, selector)
  parameterAddress = param
  local owner = api.u64(param,'parameter owner',false)
  local mt = api.u64(param+0x30,'MT context',false)
  return {selector_class=selector,parameter_address=api.hex(param),
    parameter_raw_hex=api.bytesHex(param,0x38,'selector parameters'),
    float_input_bits=api.hex(api.u32(param+0x20,'float input bits')),
    mt_address=api.hex(mt),
    mt_cursor_and_state_raw_hex=api.bytesHex(mt,0x9C4,'MT cursor and 624-word state'),
    mt_extended_context_raw_hex=api.bytesHex(mt,0x1388,'extended MT context'),
    candidates=captureOwner(owner,label)}
end
local function onBreakpoint()
  local ok, failure = pcall(function()
    api.verify(); debug_getContext(false)
    if RIP == SITES.entry.address then
      api.event('selector_entry',{placement=RDX & 0xFFFFFFFF,mode=api.u8(RBP+0x116,'mode gate'),
        generation_context_address=api.hex(RBP),generation_context_raw_hex=api.bytesHex(RBP,0x138,'generation context')})
    elseif RIP == SITES.pass0.address then
      api.event('selector_pass_0',captureSelector('pass0',RCX,0))
    elseif RIP == SITES.pass1.address then
      api.event('selector_pass_1',captureSelector('pass1',RCX,1))
    elseif RIP == SITES.final.address then
      local owner = RDI
      local event = {parameter_address=api.hex(parameterAddress),candidates=captureOwner(owner,'final')}
      if parameterAddress then
        local mt=api.u64(parameterAddress+0x30,'final MT context',false)
        event.mt_address=api.hex(mt)
        event.mt_cursor_and_state_raw_hex=api.bytesHex(mt,0x9C4,'final MT cursor and 624-word state')
        event.mt_extended_context_raw_hex=api.bytesHex(mt,0x1388,'final extended MT context')
      end
      api.event('selector_final',event); p.stop('selector_final_captured')
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
