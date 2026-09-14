-- Combined phase: normal-flow configuration branches plus E3A900 results.
-- The E3A900 input structure remains available at RBP-0x48 after the call.
local SITES = {
  key_3b37_return = {rva=0xE668A0, expected={0x84,0xC0}},
  de3fe0_return = {rva=0xE668CC, expected={0x0F,0x28,0xC8}},
  key_ae3_return = {rva=0xE668E9, expected={0x48,0x85,0xC0}},
  draw_count_return = {rva=0xE3B0B4, expected={0x85,0xC0}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-config-and-draw-count/v2',SITES,180,128)
local p=api.probe
p.branch_status={key_3b37='not_executed',de3fe0='not_executed',key_ae3='not_executed'}
local function onBreakpoint()
  local ok,failure=pcall(function()
    api.verify(); debug_getContext(false)
    if RIP==SITES.key_3b37_return.address then
      p.branch_status.key_3b37='executed'
      api.event('config_0x3b37_return',{found=(RAX & 0xFF)~=0,config_root_address=api.hex(RDI),
        config_context_address=api.hex(RDI+0x2820),config_context_raw_hex=api.bytesHex(RDI+0x2820,0x200,'0x3B37 context')})
    elseif RIP==SITES.de3fe0_return.address then
      p.branch_status.de3fe0='executed'
      api.event('de3fe0_return',{xmm_return_capture='unavailable_in bridge; aggregate float bits are captured in selector summary'})
    elseif RIP==SITES.key_ae3_return.address then
      p.branch_status.key_ae3='executed'
      local row=RAX
      api.event('config_0xae3_return',{row_address=api.hex(row),row_raw_hex=row~=0 and api.bytesHex(row,0x40,'0xAE3 row') or nil,
        integer_factor=row~=0 and api.u32(row+0x10,'integer factor') or nil,
        float_factor_bits=row~=0 and api.hex(api.u32(row+0x18,'float factor')) or nil})
    elseif RIP==SITES.draw_count_return.address then
      api.event('draw_count_return',{input_address=api.hex(RBP-0x48),
        input_raw_hex=api.bytesHex(RBP-0x48,0x18,'E3A900 input'),return_value=RAX & 0xFFFFFFFF})
    else error('Unexpected breakpoint') end
  end)
  if not ok then p.error=tostring(failure);p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run);return 1
end
return api.arm(onBreakpoint)
