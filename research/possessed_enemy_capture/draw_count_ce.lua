-- Phase 3a: E3A900 inputs and return values for every selector invocation.
local SITES = {
  before = {rva=0xE3B0AF, expected={0xE8,0x4C,0xF8,0xFF,0xFF}},
  after = {rva=0xE3B0B4, expected={0x85,0xC0}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-draw-count/v2', SITES, 180, 128)
local p = api.probe
local pending = {}
local function key() return tostring(api.threadId() or 'unknown') end
local function onBreakpoint()
  local ok, failure = pcall(function()
    api.verify(); debug_getContext(false)
    if RIP == SITES.before.address then
      local event={input_address=api.hex(RCX),input_raw_hex=api.bytesHex(RCX,0x18,'E3A900 input'),
        candidate_count=RDX,selector_context_address=api.hex(RBP)}
      pending[key()]=event; api.event('draw_count_before',event)
    elseif RIP == SITES.after.address then
      local event={return_value=RAX & 0xFFFFFFFF,matched_before_sequence=nil}
      local prior=pending[key()]
      if prior then event.matched_before_sequence=prior.sequence; pending[key()]=nil end
      api.event('draw_count_after',event)
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
