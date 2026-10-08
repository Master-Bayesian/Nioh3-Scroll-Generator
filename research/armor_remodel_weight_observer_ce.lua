-- PC v2.02 bounded, read-only armor remodel weight observer.
--
-- This file is an offline-ready CE observer.  It is intentionally limited to
-- two execute breakpoints in one known native function.  The runner must
-- provide NIOH3_ARMOR_WEIGHT_TARGET_IDENTITY and NIOH3_ARMOR_WEIGHT_RUN_ID.
-- The identity is evidence supplied by the runner; CE cannot calculate an
-- executable hash from this script.  No game-function calls, writes, context
-- changes, or complete record reads are performed here.

local SCHEMA = 'nioh3.armor-remodel-weight-observer.v1'
local identity = assert(NIOH3_ARMOR_WEIGHT_TARGET_IDENTITY,
  'Expected verified PC v2.02 executable identity')
assert(type(identity) == 'table', 'Executable identity must be a table')
assert(type(identity.process_id) == 'number' and identity.process_id > 0 and
  identity.process_id % 1 == 0, 'Executable identity is missing process_id')
assert(type(identity.creation_filetime) == 'string' and
  identity.creation_filetime:match('^%d+$') ~= nil and
  #identity.creation_filetime <= 32,
  'Executable identity is missing creation_filetime')
assert(type(identity.image_size) == 'number' and identity.image_size > 0 and
  identity.image_size % 1 == 0, 'Executable identity is missing image_size')
assert(type(identity.executable_sha256) == 'string' and
  #identity.executable_sha256 == 64 and
  identity.executable_sha256:match('^%x+$'),
  'Executable identity is missing executable_sha256')
assert(identity.image_size == 77830112,
  'Unsupported PC v2.02 executable image_size')
assert(identity.executable_sha256:upper() ==
  'E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130',
  'Unsupported PC v2.02 executable SHA-256')
if identity.file_version ~= nil then
  assert(identity.file_version == '2.0.2.0',
    'Unsupported PC v2.02 executable file_version')
end

local run_id = assert(NIOH3_ARMOR_WEIGHT_RUN_ID,
  'Explicit armor weight observer run ID required')
assert(type(run_id) == 'string' and #run_id > 0 and #run_id <= 128,
  'Invalid armor weight observer run ID')

local sites = {
  entry = {
    rva = 0x818018,
    -- mov [rsp+8],rbx; push rdi; sub rsp,20h
    hex = '48895C2408574883EC20',
  },
  exit = {
    rva = 0x8180A3,
    -- add rsp,20h; the breakpoint is before the epilogue continues.
    hex = '4883C420',
  },
}

-- A native call hits both sites, so a 64-callback limit only permits 32
-- calls and can be consumed by unrelated records. Keep separate, explicit
-- bounds: 128 callbacks (at most 64 paired calls), 64 target entries, and
-- 64 unrelated entry callbacks, all inside the configured 30- or 120-second
-- window. The longer window is an explicit owner choice for bridge latency;
-- every hit, event, and read bound remains unchanged.
local configured_max_seconds = rawget(_G, 'NIOH3_ARMOR_WEIGHT_MAX_SECONDS')
if configured_max_seconds == nil then configured_max_seconds = 30 end
assert(type(configured_max_seconds) == 'number' and
  configured_max_seconds % 1 == 0 and
  (configured_max_seconds == 30 or configured_max_seconds == 120),
  'NIOH3_ARMOR_WEIGHT_MAX_SECONDS must be integer 30 or 120')
local MAX_EVENTS, MAX_HITS, MAX_TARGET_ENTRIES, MAX_IGNORED_HITS, MAX_SECONDS =
  64, 128, 64, 64, configured_max_seconds
local MAX_READ_BYTES, CLEANUP_RETRIES_MS = 4096, 100
local TARGETS = {[0x609C] = true, [0xED50] = true}
local MODE_LABELS = {
  ['0/0'] = 'unmodified',
  ['1/0'] = 'strengthened',
  ['2/0'] = 'thickened',
  ['1/1'] = 'extreme_strengthened',
  ['2/2'] = 'extreme_thickened',
  ['1/2'] = 'strengthened_plus_thickened',
}
local NEGATIVE_MODES = {
  ['0/0'] = true,
  ['1/0'] = true,
  ['1/1'] = true,
}

local base = assert(getAddressSafe('Nioh3.exe'), 'Nioh3.exe module is unavailable')
local pid = assert(getOpenedProcessID(), 'Opened process ID is unavailable')
assert(pid == identity.process_id, 'Wrong process for supplied executable identity')
assert(type(getOpenedProcessHandle) == 'function',
  'Opened process handle API is unavailable')
local handle = assert(getOpenedProcessHandle(), 'Opened process handle is unavailable')

local function hex(value)
  assert(type(value) == 'number', 'Expected numeric address')
  return string.format('0x%X', value)
end

local function copy_identity(value)
  local result = {}
  for key, item in pairs(value) do result[key] = item end
  return result
end

local total_read_bytes = 0
local function guard_process()
  assert(getOpenedProcessID() == pid, 'Attached process changed')
  assert(getOpenedProcessHandle() == handle, 'Attached process handle changed')
  assert(getAddressSafe('Nioh3.exe') == base, 'Nioh3.exe module base changed')
end

local function read_bytes(address, count, label)
  guard_process()
  assert(type(address) == 'number' and address > 0 and address % 1 == 0,
    'Invalid read address: '..tostring(label))
  assert(type(count) == 'number' and count >= 0 and count <= 64 and
    count % 1 == 0, 'Invalid read size: '..tostring(label))
  total_read_bytes = total_read_bytes + count
  assert(total_read_bytes <= MAX_READ_BYTES, 'Read budget exhausted')
  local bytes = readBytes(address, count, true)
  assert(type(bytes) == 'table' and #bytes == count,
    'Unreadable '..tostring(label))
  return bytes
end

local function little(bytes, count)
  local value = 0
  for index = count, 1, -1 do
    local byte = bytes[index]
    assert(type(byte) == 'number' and byte >= 0 and byte <= 255,
      'Malformed native byte')
    value = (value << 8) | byte
  end
  return value
end

local function u8(address, label)
  return little(read_bytes(address, 1, label), 1)
end

local function u16(address, label)
  return little(read_bytes(address, 2, label), 2)
end

local function u32(address, label)
  return little(read_bytes(address, 4, label), 4)
end

local function signature(address, expected, label)
  local actual = read_bytes(address, #expected / 2, label)
  for index = 1, #actual do
    local wanted = tonumber(expected:sub(index * 2 - 1, index * 2), 16)
    assert(actual[index] == wanted,
      'Signature mismatch at '..tostring(label))
  end
end

local function verify_signatures()
  for name, site in pairs(sites) do
    signature(base + site.rva, site.hex, name)
  end
end

local function inventory()
  assert(type(debug_isDebugging) == 'function',
    'Debugger status API unavailable')
  if not debug_isDebugging() then
    -- CE commonly returns nil before a debugger has started. That is an
    -- unknown inventory only while inactive; callers must not treat it as an
    -- empty active inventory.
    return nil
  end
  assert(type(debug_getBreakpointList) == 'function',
    'Breakpoint inventory API unavailable')
  local ok, result = pcall(debug_getBreakpointList)
  assert(ok and type(result) == 'table',
    'Breakpoint inventory unavailable: '..tostring(result))
  local addresses = {}
  for key, item in pairs(result) do
    local address = item
    if type(item) == 'table' then address = item.address end
    if type(item) == 'boolean' and type(key) == 'number' then address = key end
    if type(address) == 'string' then address = tonumber(address) end
    assert(type(address) == 'number' and address > 0 and address % 1 == 0,
      'Unrecognized breakpoint inventory entry')
    addresses[#addresses + 1] = address
  end
  table.sort(addresses)
  return addresses
end

local function inventory_hex(addresses)
  local result = {}
  for _, address in ipairs(addresses or {}) do result[#result + 1] = hex(address) end
  return result
end

local function empty_inventory()
  local addresses = inventory()
  assert(addresses ~= nil,
    'Breakpoint inventory unavailable while debugger is inactive')
  assert(#addresses == 0, 'Foreign or unknown breakpoint inventory')
  return addresses
end

local function current_interface()
  for _, name in ipairs({
    'debug_getCurrentDebuggerInterface',
    'getCurrentDebuggerInterface',
  }) do
    if type(_G[name]) == 'function' then
      local ok, result = pcall(_G[name])
      if ok then return result end
    end
  end
  return nil
end

local function thread_id()
  -- THREADID is populated by CE's stopped-event context. The similarly
  -- named legacy current-thread getter identifies CE's callback thread, not
  -- the stopped target thread, and must never silently become target identity.
  local event_thread = rawget(_G, 'THREADID')
  if type(event_thread) == 'number' and event_thread > 0 and
    event_thread % 1 == 0 then
    return event_thread, 'debug_event_context:THREADID'
  end
  error('Target debug-event thread ID unavailable')
end

local function debugger_stopped()
  if type(debug_isBroken) == 'function' then
    local ok, result = pcall(debug_isBroken)
    assert(ok, 'CE stopped-event status query failed')
    if type(result) == 'boolean' then return result, 'debug_isBroken' end
    -- Some installed CE builds expose a callable debug_isBroken symbol whose
    -- result is a serialized function object. Match the existing observer
    -- fallback: query the current context and record the returned type.
    assert(type(debug_getCurrentContextTable) == 'function',
      'CE stopped-event fallback API unavailable')
    local context_ok, context = pcall(debug_getCurrentContextTable)
    assert(context_ok, 'CE stopped-event fallback query failed')
    return type(context) == 'table', 'debug_context_fallback:'..type(result)
  end
  assert(type(debug_getCurrentContextTable) == 'function',
    'CE stopped-event status API unavailable')
  local ok, context = pcall(debug_getCurrentContextTable)
  assert(ok, 'CE stopped-event fallback query failed')
  return type(context) == 'table', 'debug_context_fallback:'..type(context)
end

local function sibling_path(path, name)
  local slash = path:match('^.*()[/\\]')
  if slash ~= nil then return path:sub(1, slash)..name end
  return name
end

local function load_timer_adapter()
  local path = NIOH3_ARMOR_WEIGHT_MAIN_THREAD_TIMER_PATH or
    NIOH3_MAIN_THREAD_TIMER_PATH
  if path == nil and type(NIOH3_BREAKPOINT_LIFECYCLE_PATH) == 'string' then
    path = sibling_path(NIOH3_BREAKPOINT_LIFECYCLE_PATH,
      'ce_main_thread_timer.lua')
  end
  assert(type(path) == 'string' and #path > 0 and #path <= 1024,
    'Main-thread timer adapter path required')
  local chunk, failure = loadfile(path)
  assert(type(chunk) == 'function',
    'Main-thread timer adapter unavailable: '..tostring(failure))
  local ok, adapter = pcall(chunk)
  assert(ok and type(adapter) == 'function',
    'Main-thread timer adapter is invalid: '..tostring(adapter))
  return adapter, path
end

local prior_observer = rawget(_G, 'nioh3ArmorRemodelWeightObserver')
if prior_observer ~= nil then
  assert(type(prior_observer) == 'table' and prior_observer.schema == SCHEMA,
    'Previous armor weight observer namespace is not recognized')
  assert(prior_observer.active == false,
    'Previous armor weight observer is still active')
  assert(prior_observer.cleanup_pending == false and
    prior_observer.cleanup_verified == true,
    'Previous armor weight observer cleanup is unconfirmed')
  assert(type(prior_observer.owned_breakpoints) == 'table' and
    next(prior_observer.owned_breakpoints) == nil,
    'Previous armor weight observer still owns breakpoints')
end

-- Prove active global debugger state before replacing the observer namespace.
-- An inactive debugger may legitimately report a nil breakpoint inventory.
if type(debug_isDebugging) == 'function' and debug_isDebugging() then
  assert(type(debug_getBreakpointList) == 'function',
    'Breakpoint inventory API unavailable')
  local ok, listed = pcall(debug_getBreakpointList)
  assert(ok and type(listed) == 'table' and next(listed) == nil,
    'Foreign or unknown breakpoint inventory')
end

local function control_for(item_id, mode_a, mode_b)
  local mode = tostring(mode_a)..'/'..tostring(mode_b)
  local label = MODE_LABELS[mode]
  local thickening_bytes = (mode_a == 2 and 1 or 0) +
    (mode_b == 2 and 1 or 0)
  if label == nil then
    return 'target_unclassified', 'unknown', mode, thickening_bytes
  end
  if NEGATIVE_MODES[mode] then
    return 'negative', label, mode, thickening_bytes
  end
  if thickening_bytes > 0 then
    return 'positive', label, mode, thickening_bytes
  end
  return 'target_unclassified', 'unknown', mode, thickening_bytes
end

local p = {
  schema = SCHEMA,
  run_id = run_id,
  pid = pid,
  module_base = hex(base),
  identity = copy_identity(identity),
  read_only = true,
  writes_game_memory = false,
  calls_game_functions = false,
  register_changes = false,
  active = false,
  prepared = false,
  armed = false,
  cleanup_ready = false,
  events = {},
  event_sequence = 0,
  max_events = MAX_EVENTS,
  max_hits = MAX_HITS,
  max_target_entries = MAX_TARGET_ENTRIES,
  max_ignored_hits = MAX_IGNORED_HITS,
  max_seconds = MAX_SECONDS,
  total_hits = 0,
  target_entry_hits = 0,
  late_hits = 0,
  ignored_hits = 0,
  stale_pairs = 0,
  read_bytes = 0,
  cleanup_pending = false,
  cleanup_errors = {},
  owned_breakpoints = {},
  global_breakpoints = {},
  cleanup_verified = false,
  process_hook_installed = false,
  process_hook_restored = true,
  debugger_broken = nil,
  debugger_broken_source = nil,
  fresh_inventory = {},
  arm_inventory = {},
  timer_cleanup = {
    main_thread_adapter = false,
    budget_timer_created = false,
    budget_timer_destroy_requested = false,
    budget_timer_destroyed = false,
    cleanup_timer_created = false,
    cleanup_timer_destroyed = false,
  },
  controls = {
    positive = 'known remodel mode 2/0, 2/2, or 1/2 with a thickening byte',
    negative = 'known remodel mode 0/0, 1/0, or 1/1 with zero thickening bytes',
  },
}

-- Export before arming so a bounded arm error still leaves a status object.
nioh3ArmorRemodelWeightObserver = p

local function record_cleanup_error(message)
  if #p.cleanup_errors < 32 then
    p.cleanup_errors[#p.cleanup_errors + 1] = tostring(message)
  end
end

local function refresh_inventory()
  local ok, result = pcall(inventory)
  if not ok then
    record_cleanup_error(result)
    p.global_breakpoints = nil
    return nil
  end
  if result == nil then
    p.global_breakpoints = nil
    p.cleanup_verified = false
    return nil
  end
  p.global_breakpoints = inventory_hex(result)
  return result
end

local function run_on_main_thread(action)
  assert(type(action) == 'function', 'Main-thread action is not callable')
  if type(inMainThread) == 'function' then
    local ok, result = pcall(inMainThread)
    assert(ok, 'CE main-thread status query failed')
    if result == true then return action() end
  end
  if type(synchronize) ~= 'function' then return action() end
  local completed, ok, result = false, false, nil
  synchronize(function()
    ok, result = pcall(action)
    completed = true
  end)
  assert(completed, 'CE main-thread synchronization did not complete')
  assert(ok, result)
  return result
end

local function destroy_timer(timer, field)
  if timer == nil then return false end
  p.timer_cleanup.budget_timer_destroy_requested =
    p.timer_cleanup.budget_timer_destroy_requested or field == 'budget'
  local destroyed = false
  if type(destroyTimer) == 'function' then
    local ok, result = pcall(function()
      return run_on_main_thread(function() return destroyTimer(timer) end)
    end)
    destroyed = ok and result ~= false
  end
  -- Standard CE timers are userdata, while the closed Lua harness uses a
  -- table. Index both uniformly and accept either CE's `timer.destroy()`
  -- method shape or a conventional self-taking Lua method.
  if not destroyed then
    local ok, result = pcall(function()
      return run_on_main_thread(function()
        local method = timer.destroy
        if type(method) ~= 'function' then return false end
        local called, value = pcall(method, timer)
        if called and value ~= false then return true end
        local called_without_self, value_without_self = pcall(method)
        return called_without_self and value_without_self ~= false
      end)
    end)
    if ok then
      destroyed = result == true
    end
  end
  if field == 'budget' then p.timer_cleanup.budget_timer_destroyed = destroyed end
  if field == 'cleanup' then p.timer_cleanup.cleanup_timer_destroyed = destroyed end
  return destroyed
end

local budget_timer, cleanup_timer
local cleanup_scheduled = false
local owner
local timer_create, timer_adapter_path
local old_on_open_process, switched_on_open_process

local function publish_owner_state(result)
  if type(result) == 'table' then
    p.cleanup_pending = result.cleanup_pending == true
    if type(result.owned_breakpoints) == 'table' then
      p.owned_breakpoints = result.owned_breakpoints
    end
  end
  local addresses = refresh_inventory()
  local debugger_ok, debugger_failure = pcall(function()
    local broken, source = debugger_stopped()
    p.debugger_broken, p.debugger_broken_source = broken, source
    assert(not broken, 'Debugger event is still stopped during cleanup')
    assert(debug_isDebugging(), 'Debugger is no longer running during cleanup')
  end)
  if not debugger_ok then record_cleanup_error(debugger_failure) end
  local timers_ok = (not p.timer_cleanup.budget_timer_created or
      p.timer_cleanup.budget_timer_destroyed) and
    (not p.timer_cleanup.cleanup_timer_created or
      p.timer_cleanup.cleanup_timer_destroyed)
  local hook_ok = p.process_hook_restored == true and
    (switched_on_open_process == nil or onOpenProcess ~= switched_on_open_process)
  local inventory_ok = addresses ~= nil and #addresses == 0
  local debugger_running = type(debug_isDebugging) == 'function' and
    debug_isDebugging() == true
  p.cleanup_verified = (not p.cleanup_pending and inventory_ok and
    debugger_ok and debugger_running and timers_ok and hook_ok and
    type(p.owned_breakpoints) == 'table' and
    next(p.owned_breakpoints) == nil)
  if not p.cleanup_verified then p.cleanup_pending = true end
  p.read_bytes = total_read_bytes
end

local function cleanup_now()
  cleanup_scheduled = false
  if cleanup_timer ~= nil then
    destroy_timer(cleanup_timer, 'cleanup')
    cleanup_timer = nil
  end
  local ok, result = pcall(owner.stop)
  if not ok then
    record_cleanup_error(result)
    p.cleanup_pending = true
    p.cleanup_verified = false
    return
  end
  publish_owner_state(result)
end

local function schedule_cleanup()
  if cleanup_scheduled or owner == nil then return end
  cleanup_scheduled = true
  p.timer_cleanup.cleanup_timer_created = true
  local function run_cleanup()
    -- ce_main_thread_timer destroys its userdata before invoking us. Mark it
    -- here and clear the reference so cleanup_now does not depend on a second
    -- destroy call on an already-destroyed CE object.
    p.timer_cleanup.cleanup_timer_destroyed = true
    cleanup_timer = nil
    cleanup_now()
  end
  local ok, result = pcall(timer_create, CLEANUP_RETRIES_MS, run_cleanup)
  if ok then
    cleanup_timer = result
    if cleanup_timer == nil then
      cleanup_scheduled = false
      p.timer_cleanup.cleanup_timer_destroyed = true
      record_cleanup_error('Main-thread cleanup timer returned no timer')
      cleanup_now()
    end
  else
    cleanup_scheduled = false
    p.timer_cleanup.cleanup_timer_destroyed = true
    record_cleanup_error(result)
    cleanup_now()
  end
end

local function finish(reason)
  if p.stop_reason == nil then
    p.stop_reason = reason
    p.stopped_elapsed_ms = type(getTickCount) == 'function' and
      p.started_tick_ms ~= nil and (getTickCount() - p.started_tick_ms) or nil
  end
  p.active = false
  p.read_bytes = total_read_bytes
  p.cleanup_pending = true
  if budget_timer ~= nil then
    destroy_timer(budget_timer, 'budget')
    budget_timer = nil
  end
  if switched_on_open_process ~= nil and onOpenProcess == switched_on_open_process then
    onOpenProcess = old_on_open_process
    p.process_hook_restored = true
  end
  schedule_cleanup()
end

local function elapsed_ms()
  if type(getTickCount) ~= 'function' or p.started_tick_ms == nil then return 0 end
  local result = getTickCount() - p.started_tick_ms
  if result < 0 then result = result + 0x100000000 end
  return result
end

local pending = {}
local function pair_key(thread, stack)
  return tostring(thread)..':'..tostring(stack)
end

local function emit(event)
  assert(#p.events < MAX_EVENTS, 'Event budget exhausted')
  p.event_sequence = p.event_sequence + 1
  event.sequence = p.event_sequence
  event.elapsed_ms = elapsed_ms()
  p.events[#p.events + 1] = event
end

local function entry_observation()
  assert(type(RCX) == 'number' and RCX > 0, 'Entry static row is null')
  assert(type(RDX) == 'number' and RDX > 0, 'Entry armor record is null')
  assert(type(RSP) == 'number' and RSP > 0, 'Entry stack is unavailable')
  local thread, source = thread_id()
  local item_id = u16(RDX + 0x00, 'record item id')
  local mode_a = u8(RDX + 0x31, 'record remodel byte 31')
  local mode_b = u8(RDX + 0x32, 'record remodel byte 32')
  local level = u16(RDX + 0x06, 'record level')
  local plus = u16(RDX + 0x0A, 'record plus')
  local rarity = u8(RDX + 0x30, 'record rarity')
  if not TARGETS[item_id] then
    p.ignored_hits = p.ignored_hits + 1
    if p.ignored_hits >= MAX_IGNORED_HITS then
      finish('ignored_hit_budget')
    end
    return
  end
  p.target_entry_hits = p.target_entry_hits + 1
  if p.target_entry_hits > MAX_TARGET_ENTRIES then
    finish('target_entry_budget')
    return
  end
  local control, mode_label, mode, thickening_bytes = control_for(item_id, mode_a, mode_b)
  local key = pair_key(thread, RSP)
  if pending[key] ~= nil then p.stale_pairs = p.stale_pairs + 1 end
  pending[key] = {
    thread_id = thread,
    thread_source = source,
    entry_rsp = RSP,
    item_id = item_id,
    mode_a = mode_a,
    mode_b = mode_b,
    mode = mode,
    mode_label = mode_label,
    thickening_bytes = thickening_bytes,
    control = control,
    level = level,
    plus = plus,
    rarity = rarity,
    static_row_id = u16(RCX + 0x152, 'entry static row id'),
    base_weight_raw = u32(RCX + 0x98, 'entry static base weight'),
  }
end

local function exit_observation()
  assert(type(RSP) == 'number' and RSP > 0, 'Exit stack is unavailable')
  assert(type(R10) == 'number' and R10 > 0, 'Exit selected static row is null')
  local thread, source = thread_id()
  local entry_rsp = RSP + 0x28
  local key = pair_key(thread, entry_rsp)
  local entry = pending[key]
  if entry == nil then
    p.stale_pairs = p.stale_pairs + 1
    return
  end
  pending[key] = nil
  assert(entry.entry_rsp == entry_rsp, 'Entry/exit stack relation changed')
  -- The native return is the low 32 bits of RAX. EAX can be a stale mock/API
  -- alias after a context refresh, so never prefer it over the full register.
  assert(type(RAX) == 'number' and RAX % 1 == 0,
    'Exit effective weight register unavailable')
  local selected_id = u16(R10 + 0x152, 'exit selected static row id')
  local selected_weight = u32(R10 + 0x98, 'exit selected static base weight')
  emit({
    site = 'paired_entry_exit',
    rva_entry = hex(sites.entry.rva),
    rva_exit = hex(sites.exit.rva),
    thread_id = entry.thread_id,
    thread_id_source = entry.thread_source or source,
    stack_relation = 'exit_rsp_is_entry_rsp_minus_0x28',
    item_id = entry.item_id,
    mode_a = entry.mode_a,
    mode_b = entry.mode_b,
    mode = entry.mode,
    mode_label = entry.mode_label,
    thickening_bytes = entry.thickening_bytes,
    control = entry.control,
    level = entry.level,
    plus = entry.plus,
    rarity = entry.rarity,
    entry_static_row_id = entry.static_row_id,
    entry_base_weight_raw = entry.base_weight_raw,
    selected_static_row_id = selected_id,
    selected_base_weight_raw = selected_weight,
    effective_weight_raw = RAX & 0xFFFFFFFF,
  })
  if #p.events >= MAX_EVENTS then finish('event_budget') end
end

local function callback(name)
  return function()
    local ok, failure = xpcall(function()
      guard_process()
      verify_signatures()
      assert(RIP == sites[name].address, 'Unexpected breakpoint PC')
      if not p.active then return end
      if p.total_hits >= MAX_HITS then
        p.late_hits = p.late_hits + 1
        finish('hit_budget')
        return
      end
      p.total_hits = p.total_hits + 1
      if elapsed_ms() >= MAX_SECONDS * 1000 then
        finish('time_budget')
        return
      end
      assert(type(debug_getContext) == 'function',
        'Debug-event context API unavailable')
      local context_ok, context_failure = pcall(debug_getContext, false)
      assert(context_ok, 'Debug-event context refresh failed: '..tostring(context_failure))
      if name == 'entry' then entry_observation() else exit_observation() end
      if p.total_hits >= MAX_HITS and p.active then finish('hit_budget') end
    end, debug.traceback)
    if not ok then
      local text = tostring(failure)
      p.error = text:sub(1, 4096)
      if text:find('Attached process changed', 1, true) or
        text:find('process handle changed', 1, true) or
        text:find('module base changed', 1, true) then
        finish('process_changed')
      else
        finish('capture_error')
      end
    end
    -- Resume every stopped event, including inactive, bounds, and error
    -- paths. The stop reason remains process_changed if attachment proof
    -- failed; a failed resume is retained as an additional error.
    local continued, why = pcall(debug_continueFromBreakpoint, co_run)
    if not continued or why == false then
      p.error = ('Resume unconfirmed: '..tostring(why)):sub(1, 4096)
      finish('resume_error')
    end
    p.read_bytes = total_read_bytes
    return 1
  end
end

local function status()
  p.elapsed_ms = elapsed_ms()
  p.read_bytes = total_read_bytes
  return {
    active = p.active,
    prepared = p.prepared,
    armed = p.armed,
    cleanup_ready = p.cleanup_ready,
    process_hook_installed = p.process_hook_installed,
    process_hook_restored = p.process_hook_restored,
    run_id = p.run_id,
    pid = p.pid,
    schema = p.schema,
    cleanup_pending = p.cleanup_pending,
    cleanup_verified = p.cleanup_verified,
    owned_breakpoints = p.owned_breakpoints,
    global_breakpoints = p.global_breakpoints,
    event_count = #p.events,
    total_hits = p.total_hits,
    target_entry_hits = p.target_entry_hits,
    late_hits = p.late_hits,
    ignored_hits = p.ignored_hits,
    stale_pairs = p.stale_pairs,
    stop_reason = p.stop_reason,
    error = p.error,
    debugger_broken = p.debugger_broken,
    debugger_broken_source = p.debugger_broken_source,
    elapsed_ms = p.elapsed_ms,
    read_bytes = p.read_bytes,
    timer_cleanup = p.timer_cleanup,
  }
end

function p.status() return status() end
function p.stop(reason)
  if p.active then finish(reason or 'manual')
  elseif p.cleanup_pending then schedule_cleanup() end
  return status()
end
function p.retry_cleanup()
  assert(owner ~= nil, 'Observer owner is unavailable')
  local ok, result = pcall(owner.retry_cleanup)
  if not ok then
    record_cleanup_error(result)
    p.cleanup_pending = true
    p.cleanup_verified = false
  else
    publish_owner_state(result)
  end
  return status()
end
function p.clear_captures()
  assert(not p.active and not p.cleanup_pending and p.cleanup_verified,
    'Stop and verify cleanup first')
  p.events = {}; p.event_sequence = 0; pending = {}; p.stale_pairs = 0
end

local prepared, arm_started = false, false

local function do_prepare()
  if prepared then return status() end

  -- Bootstrap/prepare validates all external state and creates the owned
  -- lifecycle object.  It intentionally does not arm a breakpoint or start a
  -- capture timer; the caller may now present its controlled menu state.
  for name, site in pairs(sites) do site.address = base + site.rva end
  verify_signatures()
  local lifecycle_path = assert(NIOH3_BREAKPOINT_LIFECYCLE_PATH,
    'Owned breakpoint lifecycle helper path required')
  local lifecycle_chunk = assert(loadfile(lifecycle_path))
  local make_owner = lifecycle_chunk()
  assert(type(make_owner) == 'function', 'Owned breakpoint lifecycle helper is invalid')
  timer_create, timer_adapter_path = load_timer_adapter()
  p.timer_cleanup.main_thread_adapter = true
  local initial_inventory = {}
  if debug_isDebugging() then
    local broken, broken_source = debugger_stopped()
    assert(not broken, 'Debugger event is still stopped before prepare')
    p.debugger_broken, p.debugger_broken_source = broken, broken_source
    initial_inventory = empty_inventory()
  end
  p.fresh_inventory = inventory_hex(initial_inventory)

  if debug_isDebugging() then
    local interface = current_interface()
    assert(interface == nil or interface == 2,
      'An existing non-VEH debugger is active')
  else
    assert(type(debugProcess) == 'function', 'VEH debugger API unavailable')
    debugProcess(2)
  end
  assert(debug_isDebugging(), 'VEH debugger attachment failed')
  local interface = current_interface()
  assert(interface == nil or interface == 2, 'VEH debugger interface not active')
  local broken, broken_source = debugger_stopped()
  assert(not broken, 'Debugger event is still stopped before arm')
  p.debugger_broken, p.debugger_broken_source = broken, broken_source
  assert(#empty_inventory() == 0, 'Breakpoint inventory changed before arm')

  owner = make_owner({
    list = function() guard_process(); return debug_getBreakpointList() end,
    remove = function(address) guard_process(); return debug_removeBreakpoint(address) end,
    remove_id = function(id) guard_process(); return debug_removeBreakpointByID(id) end,
    timer = timer_create,
    arm = function(address, fn)
      guard_process()
      local ok, result, id = pcall(debug_setBreakpoint, address, 1,
        bptExecute, bpmDebugRegister, fn)
      if not ok then return false, result end
      if result == false then return false, 'CE rejected breakpoint arm' end
      -- CE's debug_setBreakpoint may return nil on success. Trust only the
      -- explicit active inventory, never the return value alone.
      local listed = inventory()
      if listed == nil then
        return false, 'Breakpoint inventory unavailable after arm'
      end
      local present = false
      for _, listed_address in ipairs(listed) do
        if listed_address == address then present = true; break end
      end
      if not present then
        return false, 'Breakpoint arm was not present in active inventory'
      end
      return true, result, id
    end,
  }, p)

  prepared = true
  p.prepared = true
  p.cleanup_ready = true
  p.global_breakpoints = p.fresh_inventory
  p.cleanup_verified = true
  return status()
end

local function fail_arm(message)
  p.error = tostring(message):sub(1, 4096)
  finish('arm_failed')
  error(p.error)
end

local function do_arm()
  assert(prepared, 'Call prepare before arm')
  assert(not arm_started, 'This observer run has already been armed')
  assert(not p.active, 'Observer is already active')
  arm_started = true
  guard_process()
  verify_signatures()
  local broken, broken_source = debugger_stopped()
  assert(not broken, 'Debugger event is stopped before arm')
  p.debugger_broken, p.debugger_broken_source = broken, broken_source
  assert(#empty_inventory() == 0, 'Foreign or unknown breakpoint inventory')

  -- Install attachment-change tracking only for an armed transaction. A
  -- prepare-only phase therefore cannot leave a stale closure chained into a
  -- later observer reload.
  old_on_open_process = onOpenProcess
  switched_on_open_process = function(...)
    local ok, failure = pcall(guard_process)
    if not ok then
      p.error = tostring(failure):sub(1, 4096)
      finish('process_changed')
    end
    if type(old_on_open_process) == 'function' then
      return old_on_open_process(...)
    end
  end
  onOpenProcess = switched_on_open_process
  p.process_hook_installed = true
  p.process_hook_restored = false

  local arm_ok, arm_error = true, nil
  for _, name in ipairs({'entry', 'exit'}) do
    local ok, result = owner.arm(sites[name].address, callback(name))
    if not ok then arm_ok, arm_error = false, tostring(result); break end
  end
  if not arm_ok then
    p.error = arm_error
    finish('arm_failed')
    error(arm_error)
  end
  local arm_addresses = inventory()
  if arm_addresses == nil then fail_arm('Breakpoint inventory unavailable after arm') end
  p.arm_inventory = inventory_hex(arm_addresses)
  local started_tick = type(getTickCount) == 'function' and getTickCount() or nil
  if type(started_tick) ~= 'number' then fail_arm('Monotonic timer API unavailable') end
  p.started_tick_ms = started_tick
  p.armed = true
  p.active = true

  local function budget_expired()
    -- ce_main_thread_timer destroys the timer before invoking this callback.
    p.timer_cleanup.budget_timer_destroyed = true
    budget_timer = nil
    if p.active then finish('time_budget') end
  end
  local timer_ok, timer_result = pcall(timer_create, MAX_SECONDS * 1000, budget_expired)
  if not timer_ok then
    p.error = tostring(timer_result):sub(1, 4096)
    finish('timer_failed')
    error(p.error)
  end
  if timer_result == nil then fail_arm('Main-thread timer adapter returned no timer') end
  budget_timer = timer_result
  p.timer_cleanup.budget_timer_created = true
  local after_ok, after_broken, after_broken_source = pcall(debugger_stopped)
  if not after_ok then fail_arm(after_broken) end
  if after_broken then fail_arm('Debugger event is stopped after arm') end
  p.debugger_broken, p.debugger_broken_source = after_broken, after_broken_source
  return status()
end

function p.prepare() return do_prepare() end
function p.arm() return do_arm() end

-- Loading the script performs the bounded bootstrap/prepare handshake.  The
-- owner explicitly calls p.arm() only after the controlled trigger is ready.
do_prepare()
return status()
