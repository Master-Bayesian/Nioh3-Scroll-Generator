-- Nioh 3 PC v2.00.02, experimental runtime auxiliary override proof.
--
-- This script proves whether a completed auxiliary descriptor can be safely
-- rewritten after the native constructor. It does not edit the save file.
-- The default profile replaces the first three generated enemy groups with
-- Ichimokuren while preserving each group's native mode and encounter fields.
--
-- Edit TARGET_SEED before running. The target scroll must naturally allocate
-- at least three non-empty groups because this first proof deliberately avoids
-- calling an unverified native allocator.

local MODULE_NAME = 'Nioh3.exe'
local MODULE_VERSION = '2.00.02'
local RVA_DESCRIPTOR_COMPLETE = 0x20DD558
local EXPECTED_BYTES = '48 8B 54 24 60'

local TARGET_SEED = 203900415
local TARGET_ENEMY_KEYS = {
  0x0006DE91, -- Ichimokuren
  0x0006DE91, -- Ichimokuren
  0x0006DE91, -- Ichimokuren
}

-- Leave these nil to preserve the native result during the enemy proof.
local TARGET_RULE_KEYS = nil -- Example: { 0x6171, 0x0000, 0x0000 }
local TARGET_TERRAIN_ENUM = nil -- Example: 0x74
local lastOverride = nil

local function unsigned32(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x100000000 end
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

local function applyOverride(descriptor)
  local outerBegin = readPointer(descriptor + 0x00)
  local outerEnd = readPointer(descriptor + 0x08)
  local outerCapacity = readPointer(descriptor + 0x10)
  if not validVector(outerBegin, outerEnd, outerCapacity, 0x28) then
    return false, 'invalid outer vector'
  end

  local currentGroupCount = (outerEnd - outerBegin) / 0x28
  local requestedGroupCount = #TARGET_ENEMY_KEYS
  if requestedGroupCount > currentGroupCount then
    return false, string.format(
      'profile needs %d groups but native descriptor has only %d',
      requestedGroupCount,
      currentGroupCount
    )
  end

  local snapshot = {
    descriptor = descriptor,
    originalGroupCount = currentGroupCount,
    requestedGroupCount = requestedGroupCount,
    groups = {},
  }

  for index = 1, requestedGroupCount do
    local group = outerBegin + (index - 1) * 0x28
    local innerBegin = readPointer(group + 0x00)
    local innerEnd = readPointer(group + 0x08)
    local innerCapacity = readPointer(group + 0x10)
    if not validVector(innerBegin, innerEnd, innerCapacity, 0x14) then
      return false, string.format('invalid inner vector for group %d', index)
    end
    if innerEnd - innerBegin < 0x14 then
      return false, string.format('group %d has no reusable enemy entry', index)
    end


    local originalEntryCount = (innerEnd - innerBegin) / 0x14
    local originalKey = unsigned32(readInteger(innerBegin + 0x04))

    -- Preserve +0x00, +0x08, +0x0C, +0x0E, and +0x10 until live challenge
    -- evidence proves their complete semantics. The verified UI consumer and
    -- native resolver consume the lookup key at +0x04.
    writeInteger(innerBegin + 0x04, TARGET_ENEMY_KEYS[index])
    writeQword(group + 0x08, innerBegin + 0x14)
    snapshot.groups[index] = {
      originalEntryCount = originalEntryCount,
      originalKey = originalKey,
      replacementKey = TARGET_ENEMY_KEYS[index],
      outerRaw20 = readByte(group + 0x20),
      outerRaw21 = readByte(group + 0x21),
    }
  end
  writeQword(descriptor + 0x08, outerBegin + requestedGroupCount * 0x28)

  if TARGET_RULE_KEYS ~= nil then
    if #TARGET_RULE_KEYS ~= 3 then
      return false, 'TARGET_RULE_KEYS must contain exactly three keys'
    end
    for index = 1, 3 do
      writeSmallInteger(descriptor + 0x18 + (index - 1) * 2, TARGET_RULE_KEYS[index])
    end
  end
  if TARGET_TERRAIN_ENUM ~= nil then
    writeByte(descriptor + 0x1F, TARGET_TERRAIN_ENUM)
  end
  lastOverride = snapshot
  return true, string.format('applied %d enemy groups', requestedGroupCount)
end

openProcess(MODULE_NAME)
local moduleBase = getAddressSafe(MODULE_NAME)
if moduleBase == nil or moduleBase == 0 then
  error('Could not resolve Nioh3.exe module base')
end

local hookAddress = moduleBase + RVA_DESCRIPTOR_COMPLETE
if not bytesMatch(hookAddress, EXPECTED_BYTES) then
  error('Signature mismatch; expected Nioh3.exe ' .. MODULE_VERSION)
end

local hitCount = 0
local failureCount = 0
local stopped = false

function nioh3GetIllegalAuxiliaryOverrideStatus()
  return {
    targetSeed = TARGET_SEED,
    hookAddress = hookAddress,
    applied = hitCount,
    failures = failureCount,
    stopped = stopped,
    lastOverride = lastOverride,
  }
end

function nioh3StopIllegalAuxiliaryOverride()
  if stopped then return end
  stopped = true
  pcall(debug_removeBreakpoint, hookAddress)
  pcall(detachIfPossible)
  print(string.format(
    '[illegal auxiliary] stopped; applied=%d failures=%d',
    hitCount,
    failureCount
  ))
end

function debugger_onBreakpoint()
  if RIP == hookAddress then
    local seed = unsigned32(R12)
    if seed == TARGET_SEED then
      local ok, detail = applyOverride(RBP)
      if ok then
        hitCount = hitCount + 1
        print(string.format(
          '[illegal auxiliary] seed=%u %s (hit %d)',
          seed,
          detail,
          hitCount
        ))
      else
        failureCount = failureCount + 1
        print(string.format(
          '[illegal auxiliary] seed=%u rejected: %s',
          seed,
          detail
        ))
      end
    end
  end
  debug_continueFromBreakpoint(co_run)
  return 1
end

if not debug_isDebugging() then
  -- The established v2.00.02 capture uses the Windows debugger backend.
  debugProcess(1)
end
for _, address in ipairs(debug_getBreakpointList() or {}) do
  pcall(debug_removeBreakpoint, address)
end
if not debug_setBreakpoint(hookAddress) then
  error('Could not arm the descriptor-complete breakpoint')
end

print(string.format(
  '[illegal auxiliary] armed for Seed %u at %s+0x%X',
  TARGET_SEED,
  MODULE_NAME,
  RVA_DESCRIPTOR_COMPLETE
))
print('[illegal auxiliary] open the target scroll detail, then start its challenge')
print('[illegal auxiliary] stop with nioh3StopIllegalAuxiliaryOverride()')
