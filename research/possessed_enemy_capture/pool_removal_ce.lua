-- Phase 3c: weighted selection assignment and removal. Captures every iteration.
local SITES = {
  chosen = {rva=0xE3B471, expected={0x48,0x8B,0x33}},
  assigned = {rva=0xE3B4A6, expected={0xC6,0x86,0xE9,0x00,0x00,0x00,0x01}},
  removal = {rva=0xE3B4C2, expected={0x48,0x83,0xC3,0x10}},
  shrunk = {rva=0xE3B4F2, expected={0x48,0x83,0x6D,0x18,0x01}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-pool-removal/v2', SITES, 180, 512)
local p = api.probe
local function poolSnapshot(beginAddress, endAddress)
  if beginAddress == 0 or endAddress < beginAddress or (endAddress-beginAddress)%0x10 ~= 0 then return nil end
  local count=(endAddress-beginAddress)/0x10
  if count > 512 then return nil end
  return {begin_address=api.hex(beginAddress),end_address=api.hex(endAddress),count=count,
    raw_hex=api.bytesHex(beginAddress,count*0x10,'weighted pool')}
end
local function onBreakpoint()
  local ok, failure = pcall(function()
    api.verify(); debug_getContext(false)
    local beginAddress=api.u64(RBP-0x38,'pool begin',true)
    local endAddress=api.u64(RBP-0x30,'pool end',true)
    if RIP == SITES.chosen.address then
      local record=api.u64(RBX,'chosen pool entry',true)
      api.event('pool_chosen',{pool=poolSnapshot(beginAddress,endAddress),entry_address=api.hex(RBX),
        entry_raw_hex=api.bytesHex(RBX,0x10,'chosen entry'),record_address=api.hex(record),
        record_raw_hex=record~=0 and api.bytesHex(record,0xF0,'chosen record') or nil,
        reduced_value=RDX & 0xFFFFFFFF,total_weight=RSI & 0xFFFFFFFF})
    elseif RIP == SITES.assigned.address then
      api.event('pool_record_assigned',{pool=poolSnapshot(beginAddress,endAddress),record_address=api.hex(RSI),
        record_before_raw_hex=api.bytesHex(RSI,0xF0,'record before assignment')})
    elseif RIP == SITES.removal.address then
      api.event('pool_removal_begin',{pool=poolSnapshot(beginAddress,endAddress),next_entry_address=api.hex(RBX)})
    elseif RIP == SITES.shrunk.address then
      api.event('pool_removal_complete',{pool_before_end_address=api.hex(endAddress),
        pool_after_end_address=api.hex(endAddress-0x10),remaining_draws=api.u64(RBP+0x18,'remaining draws',true)})
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
