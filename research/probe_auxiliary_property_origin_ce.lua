-- Nioh 3 PC v2.00.02, read-only auxiliary origin probe.
--
-- This script compares the descriptor copied from property 0x1B2 with the
-- descriptor immediately before and after the canonical Seed constructor.
-- It never writes game memory or save data.

local MODULE_NAME = 'Nioh3.exe'
local MODULE_VERSION = '2.00.02'
local TARGET_SEED = 203900415

local STAGES = {
  {
    name = 'property_copy_return',
    rva = 0x1F0B630,
    expected = '41 8B 4C 24 18',
  },
  {
    name = 'before_seed_constructor',
    rva = 0x1F0C632,
    expected = '48 8D 8D 70 02 00 00',
  },
  {
    name = 'after_seed_constructor',
    rva = 0x1F0C650,
    expected = '44 38 B5 90 02 00 00',
  },
}

local function unsigned32(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x100000000 end
  return value
end

local function unsigned16(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x10000 end
  return value
end

local function parseHexBytes(text)
  local result = {}
  for token in string.gmatch(text, '%x%x') do
    result[#result + 1] = tonumber(token, 16)
  end
  return result
end

local function bytesMatch(address, expectedText)
  local expected = parseHexBytes(expectedText)
  local actual = readBytes(address, #expected, true)
  if actual == nil or #actual ~= #expected then return false end
  for index = 1, #expected do
    if actual[index] ~= expected[index] then return false end
  end
  return true
end

local function validVector(beginAddress, endAddress, capacityAddress, stride)
  return beginAddress ~= nil
    and endAddress ~= nil
    and capacityAddress ~= nil
    and beginAddress ~= 0
    and endAddress >= beginAddress
    and capacityAddress >= endAddress
    and ((endAddress - beginAddress) % stride) == 0
end

local function snapshotDescriptor(descriptor)
  local result = {
    descriptor = descriptor,
    rules = {
      unsigned16(readSmallInteger(descriptor + 0x18)),
      unsigned16(readSmallInteger(descriptor + 0x1A)),
      unsigned16(readSmallInteger(descriptor + 0x1C)),
    },
    raw1E = readByte(descriptor + 0x1E),
    terrain = readByte(descriptor + 0x1F),
    raw20 = readInteger(descriptor + 0x20),
    vectorValid = false,
    groups = {},
  }

  local outerBegin = readPointer(descriptor + 0x00)
  local outerEnd = readPointer(descriptor + 0x08)
  local outerCapacity = readPointer(descriptor + 0x10)
  result.outerBegin = outerBegin
  result.outerEnd = outerEnd
  result.outerCapacity = outerCapacity
  if not validVector(outerBegin, outerEnd, outerCapacity, 0x28) then
    return result
  end

  result.vectorValid = true
  result.groupCount = (outerEnd - outerBegin) / 0x28
  local captureCount = math.min(result.groupCount, 16)
  for groupIndex = 0, captureCount - 1 do
    local group = outerBegin + groupIndex * 0x28
    local innerBegin = readPointer(group + 0x00)
    local innerEnd = readPointer(group + 0x08)
    local innerCapacity = readPointer(group + 0x10)
    local groupResult = {
      innerBegin = innerBegin,
      innerEnd = innerEnd,
      innerCapacity = innerCapacity,
      vectorValid = false,
      entries = {},
    }
    if validVector(innerBegin, innerEnd, innerCapacity, 0x14) then
      groupResult.vectorValid = true
      groupResult.entryCount = (innerEnd - innerBegin) / 0x14
      local entryCaptureCount = math.min(groupResult.entryCount, 16)
      for entryIndex = 0, entryCaptureCount - 1 do
        local entry = innerBegin + entryIndex * 0x14
        groupResult.entries[entryIndex + 1] = {
          raw00 = unsigned32(readInteger(entry + 0x00)),
          enemyKey = unsigned32(readInteger(entry + 0x04)),
          mode = unsigned32(readInteger(entry + 0x08)),
          raw0C = unsigned32(readInteger(entry + 0x0C)),
          raw10 = unsigned32(readInteger(entry + 0x10)),
        }
      end
    end
    result.groups[groupIndex + 1] = groupResult
  end
  return result
end

openProcess(MODULE_NAME)
local moduleBase = getAddressSafe(MODULE_NAME)
if moduleBase == nil or moduleBase == 0 then
  error('Could not resolve Nioh3.exe module base')
end

local stageByAddress = {}
for _, stage in ipairs(STAGES) do
  stage.address = moduleBase + stage.rva
  if not bytesMatch(stage.address, stage.expected) then
    error(string.format(
      'Signature mismatch at %s; expected Nioh3.exe %s',
      stage.name,
      MODULE_VERSION
    ))
  end
  stageByAddress[stage.address] = stage
end

local captures = {}
local failures = {}
local stopped = false

function nioh3GetAuxiliaryPropertyOriginStatus()
  return {
    targetSeed = TARGET_SEED,
    captures = captures,
    failures = failures,
    stopped = stopped,
  }
end

function nioh3StopAuxiliaryPropertyOriginProbe()
  if stopped then return end
  stopped = true
  for _, stage in ipairs(STAGES) do
    pcall(debug_removeBreakpoint, stage.address)
  end
  pcall(detachIfPossible)
  print('[auxiliary origin] stopped')
end

function debugger_onBreakpoint()
  local stage = stageByAddress[RIP]
  if stage ~= nil then
    local seedOk, seedValue = pcall(readInteger, R12 + 0x20)
    local seed = seedOk and unsigned32(seedValue) or nil
    if seed == TARGET_SEED then
      local ok, result = pcall(snapshotDescriptor, RBP + 0x270)
      if ok then
        captures[stage.name] = result
        print(string.format(
          '[auxiliary origin] captured %s for Seed %u',
          stage.name,
          seed
        ))
      else
        failures[#failures + 1] = stage.name .. ': ' .. tostring(result)
      end
    end
  end
  debug_continueFromBreakpoint(co_run)
  return 1
end

if not debug_isDebugging() then
  debugProcess(1)
end
for _, address in ipairs(debug_getBreakpointList() or {}) do
  pcall(debug_removeBreakpoint, address)
end
for _, stage in ipairs(STAGES) do
  if not debug_setBreakpoint(stage.address) then
    error('Could not arm breakpoint for ' .. stage.name)
  end
end

print(string.format(
  '[auxiliary origin] armed three read-only stages for Seed %u',
  TARGET_SEED
))
print('[auxiliary origin] switch away from and back to the target scroll')
