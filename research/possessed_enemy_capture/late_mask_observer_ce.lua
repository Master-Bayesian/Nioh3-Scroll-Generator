-- Phase 4: observe late candidate-state decisions without writing game memory.
local SITES = {
  mask_test = {rva=0xE669A3, expected={0x42,0x8B,0x84,0x87,0xC4,0x01,0x00,0x00}},
  accepted = {rva=0xE669B0, expected={0xC6,0x82,0xEA,0x00,0x00,0x00,0x01}},
  rejected = {rva=0xE669BD, expected={0x48,0x8B,0x0A}},
  final = {rva=0xE669EC, expected={0x48,0x8B,0x9C,0x24,0x28,0x14,0x00,0x00}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-late-mask/v2', SITES, 300, 4096)
local p = api.probe
local stopOnFirstFinal = NIOH3_POSSESSED_STOP_ON_FIRST_FINAL == true

local function captureRecord(record)
  if not record or record == 0 then return {record_address='0x0'} end
  return {
    record_address=api.hex(record),
    record_raw_hex=api.bytesHex(record,0xF0,'late-mask record'),
    spawn_id=api.hex(api.u32(record+0x20,'spawn id')),
    mission_key=api.hex(api.u32(record+0x24,'mission key')),
    enemy_lookup_key=api.hex(api.u32(record+0x28,'enemy lookup key')),
    field_8e=api.u8(record+0x8E,'record+0x8E'),
    field_8f=api.u8(record+0x8F,'record+0x8F'),
    selector_class=api.u8(record+0x90,'selector class'),
    assigned_index=api.u8(record+0x95,'assigned index'),
    candidate=api.u8(record+0xE9,'candidate flag'),
    field_ea=api.u8(record+0xEA,'record+0xEA'),
  }
end

local function captureOwner(owner)
  local begin=api.u64(owner,'record vector begin',true)
  local count=api.u64(owner+8,'record vector count',true)
  assert(count <= 512,'Record vector count exceeds bound')
  local records={}
  for index=0,count-1 do
    local record=api.u64(begin+index*0x10+8,'record pointer',true)
    local item=captureRecord(record); item.index=index
    records[#records+1]=item
  end
  return {
    owner_address=api.hex(owner),
    selection_mask_hex=api.bytesHex(owner+0x1C4,12,'selection mask'),
    vector_begin=api.hex(begin),vector_count=count,records=records,
  }
end

local function onBreakpoint()
  local ok,failure=pcall(function()
    api.verify(); debug_getContext(false)
    if RIP == SITES.mask_test.address then
      local record=RDX; local owner=RDI
      api.event('mask_test',{
        owner_address=api.hex(owner),record=captureRecord(record),
        assigned_index=RAX & 0xFFFFFFFF,word_index=R8 & 0xFFFFFFFF,
        bit_value=RCX & 0xFFFFFFFF,
        selection_mask_hex=api.bytesHex(owner+0x1C4,12,'selection mask at test'),
      })
    elseif RIP == SITES.accepted.address then
      api.event('mask_accepted',{record=captureRecord(RDX)})
    elseif RIP == SITES.rejected.address then
      api.event('mask_rejected',{record=captureRecord(RDX),selection_mask_hex=api.bytesHex(RDI+0x1C4,12,'selection mask at rejection')})
    elseif RIP == SITES.final.address then
      local snapshot=captureOwner(RDI)
      local field8fNonzero=0
      local fieldEaNonzero=0
      for _,record in ipairs(snapshot.records) do
        if record.field_8f ~= 0 then field8fNonzero=field8fNonzero+1 end
        if record.field_ea ~= 0 then fieldEaNonzero=fieldEaNonzero+1 end
      end
      snapshot.field_8f_nonzero_count=field8fNonzero
      snapshot.field_ea_nonzero_count=fieldEaNonzero
      api.event('late_mask_final',snapshot)
      if stopOnFirstFinal then
        p.stop('first_final_captured')
      elseif field8fNonzero > 0 or fieldEaNonzero > 0 then
        p.stop('candidate_state_captured')
      end
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
