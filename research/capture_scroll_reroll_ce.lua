-- Nioh 3 PC v2.00.02, offline research only.
--
-- Capture the native scroll reroll candidate pool, counter increments, and an
-- accepted candidate.  The script is intentionally read-only: it installs code
-- breakpoints and writes dumps, but never writes game memory or save data.
--
-- Attach Cheat Engine to Nioh3.exe, run this script, open one completed scroll,
-- select a rerollable effect, and perform only the requested controlled action.

local MODULE_NAME = 'Nioh3.exe'
local TARGET_SEED = nil -- Set to a decimal Seed to ignore every other scroll.
local MAX_EVENTS = 128

local RVA_CANDIDATE_ENTRY = 0x20C4BD0
local RVA_CANDIDATE_RETURN = 0x20C4F4F
local RVA_REFRESH_INCREMENT = 0x20C1742
local RVA_REFRESH_AFTER = 0x20C1746
local RVA_ACCEPT_READY = 0x20C17B9
local RVA_ACCEPT_AFTER = 0x20C17EE
local RVA_COMPLETION_INCREMENT = 0x20DD050
local RVA_COMPLETION_AFTER = 0x20DD054

local processId = getOpenedProcessID()
if processId == nil or processId == 0 then
  error('Attach Cheat Engine to Nioh3.exe before running this script')
end

local moduleBase = getAddressSafe(MODULE_NAME)
if moduleBase == nil or moduleBase == 0 then
  error('Could not resolve Nioh3.exe module base')
end

local function u32(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x100000000 end
  return value
end

local function readU16(address)
  local value = readSmallInteger(address)
  if value == nil then return nil end
  if value < 0 then return value + 0x10000 end
  return value
end

local function readU32(address)
  return u32(readInteger(address))
end

local function hex(value, width)
  if value == nil then return 'READ_ERROR' end
  return string.format('0x%0' .. tostring(width) .. 'X', value)
end

local function isScrollRecord(record)
  if record == nil or record == 0 then return false end
  local recordType = readU16(record)
  local validType = recordType == 0x1E82 or recordType == 0x516D
    or recordType == 0xE604 or recordType == 0xDD82 or recordType == 0xD523
  if not validType then return false end
  local seed = readU32(record + 0x20)
  return TARGET_SEED == nil or seed == TARGET_SEED
end

local function verifyBytes(rva, expected)
  local actual = readBytes(moduleBase + rva, #expected, true)
  if actual == nil or #actual ~= #expected then
    error('Could not read code at ' .. hex(rva, 8))
  end
  for index = 1, #expected do
    if actual[index] ~= expected[index] then
      error(string.format(
        'PC v2.00.02 code mismatch at %s +%d: expected %02X, got %02X',
        hex(rva, 8), index - 1, expected[index], actual[index]
      ))
    end
  end
end

verifyBytes(RVA_CANDIDATE_ENTRY, {
  0x48,0x8B,0xC4,0x48,0x89,0x58,0x20,0x55,0x56,0x57,
  0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57
})
verifyBytes(RVA_REFRESH_INCREMENT, {0x66,0xFF,0x43,0x0C})
verifyBytes(RVA_COMPLETION_INCREMENT, {0x66,0xFF,0x43,0x0C})

local outputDir = os.getenv('USERPROFILE')
  .. '\\Desktop\\Nioh3_scroll_reroll_capture'
os.execute('mkdir "' .. outputDir .. '" 2>nul')
local manifest = assert(io.open(outputDir .. '\\manifest.tsv', 'w'))
manifest:write('# launch ', os.date('!%Y-%m-%dT%H:%M:%SZ'), '\n')
manifest:write('# game_version\tPC v2.00.02\n')
manifest:write('# event\trva\trecord\tseed\tcounter\tslot\toutput\tcount\tcandidates\tdump\n')
manifest:flush()

local sequence = 0
local activeRecord = nil
local activeOutput = nil
local activeSlot = nil

local function writeDump(address, size, filename)
  if address == nil or address == 0 or size <= 0 then return false end
  local bytes = readBytes(address, size, true)
  if bytes == nil or #bytes ~= size then return false end
  local file = assert(io.open(filename, 'wb'))
  local parts = {}
  for index = 1, #bytes do
    parts[index] = string.char(bytes[index])
  end
  file:write(table.concat(parts))
  file:close()
  return true
end

local function candidateSummary(beginAddress, count)
  local values = {}
  for index = 0, count - 1 do
    local item = beginAddress + index * 0x18
    values[#values + 1] = string.format(
      '%d:%s@%d',
      index + 1,
      hex(readU32(item + 4), 8),
      readBytes(item + 0x0C, 1, false) or -1
    )
  end
  return table.concat(values, ',')
end

local function effectSummary(record)
  if not isScrollRecord(record) then return '' end
  local values = {}
  for index = 0, 6 do
    local item = record + 0x28 + index * 0x18
    values[#values + 1] = string.format(
      '%d:%s@%d',
      index + 1,
      hex(readU32(item + 4), 8),
      readBytes(item + 0x0C, 1, false) or -1
    )
  end
  return table.concat(values, ',')
end

local function logEvent(eventName, rva, record, slot, output, count, summary, dumpName)
  if sequence >= MAX_EVENTS then return end
  sequence = sequence + 1
  local seed = isScrollRecord(record) and readU32(record + 0x20) or nil
  local counter = isScrollRecord(record) and readU16(record + 0x0C) or nil
  if isScrollRecord(record) then
    if dumpName == nil or dumpName == '' then
      dumpName = string.format('%03d_%s_record_e8.bin', sequence, eventName)
      writeDump(record, 0xE8, outputDir .. '\\' .. dumpName)
    end
    local effects = effectSummary(record)
    if summary == nil or summary == '' then
      summary = effects
    else
      summary = summary .. ';effects=' .. effects
    end
  end
  manifest:write(
    eventName, '\t', hex(rva, 8), '\t', hex(record, 16), '\t',
    tostring(seed or 'READ_ERROR'), '\t', tostring(counter or 'READ_ERROR'), '\t',
    tostring(slot or '-'), '\t', hex(output, 16), '\t', tostring(count or 0), '\t',
    summary or '', '\t', dumpName or '', '\n'
  )
  manifest:flush()
  print(string.format(
    '[reroll] %s seed=%s counter=%s slot=%s count=%s',
    eventName, tostring(seed), tostring(counter), tostring(slot), tostring(count)
  ))
end

local stageByRva = {
  [RVA_CANDIDATE_ENTRY] = 'candidate_entry',
  [RVA_CANDIDATE_RETURN] = 'candidate_return',
  [RVA_REFRESH_INCREMENT] = 'refresh_before_increment',
  [RVA_REFRESH_AFTER] = 'refresh_after_increment',
  [RVA_ACCEPT_READY] = 'accept_before_write',
  [RVA_ACCEPT_AFTER] = 'accept_after_increment',
  [RVA_COMPLETION_INCREMENT] = 'completion_before_increment',
  [RVA_COMPLETION_AFTER] = 'completion_after_increment',
}

function debugger_onBreakpoint()
  local rva = RIP - moduleBase
  local stage = stageByRva[rva]
  if stage == nil then
    debug_continueFromBreakpoint(co_run)
    return 1
  end

  if rva == RVA_CANDIDATE_ENTRY then
    if isScrollRecord(RCX) then
      activeRecord = RCX
      activeOutput = R8
      activeSlot = u32(RDX)
      local dumpName = string.format('%03d_candidate_input_record_e8.bin', sequence + 1)
      writeDump(activeRecord, 0xE8, outputDir .. '\\' .. dumpName)
      logEvent(stage, rva, activeRecord, activeSlot, activeOutput, 0, '', dumpName)
    end
  elseif rva == RVA_CANDIDATE_RETURN then
    if activeRecord ~= nil and isScrollRecord(activeRecord) then
      local beginAddress = readPointer(activeOutput)
      local endAddress = readPointer(activeOutput + 8)
      local count = 0
      if beginAddress ~= nil and endAddress ~= nil and endAddress >= beginAddress then
        count = math.floor((endAddress - beginAddress) / 0x18)
      end
      if count < 0 or count > 5 then count = 0 end
      local dumpName = ''
      local summary = ''
      if count > 0 then
        dumpName = string.format('%03d_candidate_output_%d_x18.bin', sequence + 1, count)
        writeDump(beginAddress, count * 0x18, outputDir .. '\\' .. dumpName)
        summary = candidateSummary(beginAddress, count)
      end
      logEvent(stage, rva, activeRecord, activeSlot, activeOutput, count, summary, dumpName)
    end
    activeRecord = nil
    activeOutput = nil
    activeSlot = nil
  elseif rva == RVA_REFRESH_INCREMENT or rva == RVA_REFRESH_AFTER then
    if isScrollRecord(RBX) then
      logEvent(stage, rva, RBX, nil, nil, 0, '', '')
    end
  elseif rva == RVA_ACCEPT_READY or rva == RVA_ACCEPT_AFTER then
    if isScrollRecord(RSI) then
      local summary = ''
      if rva == RVA_ACCEPT_READY and RBP ~= nil and RBP ~= 0 then
        summary = 'selected=' .. hex(readU32(RBP + 4), 8)
          .. '@' .. tostring(readBytes(RBP + 0x0C, 1, false) or -1)
      end
      logEvent(stage, rva, RSI, u32(R14), RBP, 0, summary, '')
    end
  elseif rva == RVA_COMPLETION_INCREMENT or rva == RVA_COMPLETION_AFTER then
    if isScrollRecord(RBX) then
      logEvent(stage, rva, RBX, nil, nil, 0, '', '')
    end
  end

  debug_continueFromBreakpoint(co_run)
  return 1
end

if not debug_isDebugging() then
  debugProcess(1)
end
for rva, _ in pairs(stageByRva) do
  debug_setBreakpoint(moduleBase + rva)
end

print('Scroll reroll capture armed for PC v2.00.02.')
print('Output: ' .. outputDir)
print('Open one completed scroll and perform one controlled reroll action.')
