-- Phase 1: task lookups and scoped-LCG placement. No target filtering.
local SITES = {
  seed = {rva=0x2237A53, expected={0x8B,0x3D,0xA7,0x09,0x38,0x02}},
  lookup1 = {rva=0x2237A6B, expected={0x48,0x85,0xC0}},
  placement = {rva=0x215DF5D, expected={0xE8,0xAE,0x75,0x12,0xFE}},
  lookup2 = {rva=0x2237AB0, expected={0x48,0x85,0xC0}},
}
local commonPath = NIOH3_POSSESSED_COMMON_PATH or
  'F:/Nioh3_ScrollEditor/research/possessed_enemy_capture/observer_common_ce.lua'
local api = assert(loadfile(commonPath))()('nioh3-possessed-mission-entry/v2', SITES, 180, 32)
local p, base = api.probe, api.base
p.required_sites = {seed=false,lookup1=false,placement=false,lookup2=false}
local firstKey
local function onBreakpoint()
  local ok, failure = pcall(function()
    api.verify(); debug_getContext(false)
    local rip = RIP
    if rip == SITES.seed.address then
      firstKey = api.u32(base + 0x45B8410, 'first lookup key')
      p.required_sites.seed = true
      api.event('seed', {seed=api.u32(base+0x45B8400,'seed global'),
        first_lookup_key=api.hex(firstKey),descriptor_global_raw_hex=api.bytesHex(base+0x45B8400,0x40,'descriptor globals')})
    elseif rip == SITES.lookup1.address then
      p.required_sites.lookup1 = true
      local row = RAX
      api.event('lookup1_return', {lookup_key=api.hex(firstKey),row_address=api.hex(row),
        row_raw_hex=row ~= 0 and api.bytesHex(row,0xE8,'first task row') or nil,
        rng_modifier=row ~= 0 and api.u8(row+0x33,'row+0x33') or nil})
    elseif rip == SITES.placement.address then
      p.required_sites.placement = true
      local slot = RBX
      local address = RDX + slot * 4 + 0x1A0
      api.event('placement', {slot_index=slot,value=api.u32(address,'placement'),
        value_address=api.hex(address),rng_owner_address=api.hex(RDX),
        rng_owner_raw_hex=api.bytesHex(RDX,0x220,'scoped RNG owner')})
    elseif rip == SITES.lookup2.address then
      p.required_sites.lookup2 = true
      local key = api.u32(RBX+0x248,'second lookup key')
      local row = RAX
      api.event('lookup2_return', {lookup_key=api.hex(key),row_address=api.hex(row),
        row_raw_hex=row ~= 0 and api.bytesHex(row,0xE8,'second task row') or nil,
        possession_mask=row ~= 0 and api.u8(row+0x0E,'row+0x0E') or nil})
    else error('Unexpected breakpoint') end
    local done = true
    for _, hit in pairs(p.required_sites) do if not hit then done=false end end
    if done then p.stop('all_required_sites_captured') end
  end)
  if not ok then p.error=tostring(failure); p.stop('capture_error') end
  debug_continueFromBreakpoint(co_run); return 1
end
return api.arm(onBreakpoint)
