-- Phase 3b: MT samples, range reduction/rejection, chosen records, and removal.
local SITES = {
  raw_fast = {rva=0xE3B24A, expected={0x41,0x8B,0x4C,0x82,0x04}},
  raw_reject = {rva=0xE3B3D9, expected={0x41,0x8B,0x4C,0x82,0x04}},
  reduced = {rva=0xE3B43D, expected={0x48,0x8B,0x5D,0xC8}},
  chosen = {rva=0xE3B471, expected={0x48,0x8B,0x33}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-mt-selection/v2', SITES, 180, 512)
local p = api.probe
local function onBreakpoint()
  local ok, failure = pcall(function()
    api.verify(); debug_getContext(false)
    if RIP == SITES.raw_fast.address or RIP == SITES.raw_reject.address then
      local mt=R10; local cursor=RAX & 0xFFFFFFFF
      api.event(RIP == SITES.raw_fast.address and 'mt_raw_fast' or 'mt_raw_rejection_path',
        {mt_address=api.hex(mt),cursor=cursor,raw_state_word=api.hex(api.u32(mt+cursor*4+4,'MT state word')),
         mt_cursor_and_state_raw_hex=api.bytesHex(mt,0x9C4,'MT cursor and 624-word state before draw'),
         mt_extended_context_raw_hex=api.bytesHex(mt,0x1388,'extended MT context before draw')})
    elseif RIP == SITES.reduced.address then
      api.event('mt_range_reduced',{range=RSI & 0xFFFFFFFF,reduced_value=RDX & 0xFFFFFFFF,
        rejection_limit=RDI & 0xFFFFFFFF,tempered_value=R8 & 0xFFFFFFFF})
    elseif RIP == SITES.chosen.address then
      local record=api.u64(RBX,'weighted pool record',true)
      api.event('weighted_record_chosen',{pool_entry_address=api.hex(RBX),
        pool_entry_raw_hex=api.bytesHex(RBX,0x10,'weighted pool entry'),record_address=api.hex(record),
        record_raw_hex=record ~= 0 and api.bytesHex(record,0xF0,'chosen record') or nil,
        chosen_offset=RDX & 0xFFFFFFFF,total_weight=RSI & 0xFFFFFFFF})
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
