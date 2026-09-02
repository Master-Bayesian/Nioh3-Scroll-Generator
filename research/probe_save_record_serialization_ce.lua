-- Nioh 3 PC v2.00.02, read-only save-record serialization probe.
--
-- This script watches the first four bytes of one known scroll record inside
-- the output buffer constructed immediately before the native file writer.
-- It records every observed transition without modifying game memory or save
-- data. The target offset must be confirmed from a freshly decrypted save.

local MODULE_NAME = 'Nioh3.exe'
local MODULE_VERSION = '2.00.02'
local RVA_SERIALIZE_BUFFER = 0x61E8AC
local RVA_SERIALIZE_RETURN = 0x61D7C6
local EXPECTED_SERIALIZE_BYTES = '48 8B C4 48 89 58 18 55'
local TARGET_RECORD_OFFSET = 0x17984E
local TARGET_RECORD_SIZE = 0xE8
local TARGET_SEED = 203900415
local MAX_WRITE_CAPTURES = 64
local LOCAL_APP_DATA = os.getenv('LOCALAPPDATA') or '.'
local OUTPUT_DIRECTORY = LOCAL_APP_DATA .. '\\Nioh3ScrollGenerator\\research\\save-origin-20260831'

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

local function unsigned32(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x100000000 end
  return value
end

local function bytesToHex(address, count)
  local ok, bytes = pcall(readBytes, address, count, true)
  if not ok or bytes == nil then return nil end
  local result = {}
  for index = 1, #bytes do
    result[index] = string.format('%02X', bytes[index])
  end
  return table.concat(result)
end

local function safeInstruction(address)
  if address == nil or address == 0 then return nil end
  local ok, value = pcall(disassemble, address)
  if not ok then return nil end
  return value
end

openProcess(MODULE_NAME)
local moduleBase = getAddressSafe(MODULE_NAME)
if moduleBase == nil or moduleBase == 0 then
  error('Could not resolve Nioh3.exe module base')
end

local serializeAddress = moduleBase + RVA_SERIALIZE_BUFFER
local serializeReturnAddress = moduleBase + RVA_SERIALIZE_RETURN
if not bytesMatch(serializeAddress, EXPECTED_SERIALIZE_BYTES) then
  error('Serializer signature mismatch; expected Nioh3.exe ' .. MODULE_VERSION)
end

local activeBuffer = nil
local activeWatch = nil
local activeSize = nil
local entries = {}
local writes = {}
local returns = {}
local payloadCaptures = {}
local capturedPayloadSizes = {}
local stopped = false

local function removeWatch()
  if activeWatch ~= nil then
    pcall(debug_removeBreakpoint, activeWatch)
  end
  activeWatch = nil
end

local function snapshot(stage, instructionAddress)
  if activeBuffer == nil then return nil end
  local recordAddress = activeBuffer + TARGET_RECORD_OFFSET
  local seed = unsigned32(readInteger(recordAddress + 0x20))
  local result = {
    stage = stage,
    buffer = activeBuffer,
    bufferSize = activeSize,
    recordAddress = recordAddress,
    seedAtOffset = seed,
    recordHex = bytesToHex(recordAddress, TARGET_RECORD_SIZE),
    instructionAddress = instructionAddress,
    instructionRva = instructionAddress and (instructionAddress - moduleBase) or nil,
    instruction = safeInstruction(instructionAddress),
    previousInstructionAddress = instructionAddress and getPreviousOpcode(instructionAddress) or nil,
    previousInstruction = instructionAddress and safeInstruction(getPreviousOpcode(instructionAddress)) or nil,
  }
  if stage == 'write' then
    result.registers = {
      rax = RAX,
      rcx = RCX,
      rdx = RDX,
      r8 = R8,
      r9 = R9,
      rsp = RSP,
    }
    local destinationDelta = activeWatch - RCX
    result.destinationDelta = destinationDelta
    if destinationDelta >= 0 and destinationDelta < 0x1000 then
      result.mappedSourceAddress = RDX + destinationDelta
      result.mappedSourceSeed = unsigned32(
        readInteger(result.mappedSourceAddress + 0x20)
      )
      result.mappedSourceRecordHex = bytesToHex(
        result.mappedSourceAddress,
        TARGET_RECORD_SIZE
      )
    end
  end
  return result
end

function nioh3GetSaveRecordSerializationStatus()
  return {
    targetSeed = TARGET_SEED,
    targetRecordOffset = TARGET_RECORD_OFFSET,
    entries = entries,
    writes = writes,
    returns = returns,
    payloadCaptures = payloadCaptures,
    activeBuffer = activeBuffer,
    activeWatch = activeWatch,
    stopped = stopped,
  }
end

function nioh3StopSaveRecordSerializationProbe()
  if stopped then return end
  stopped = true
  removeWatch()
  pcall(debug_removeBreakpoint, serializeAddress)
  pcall(debug_removeBreakpoint, serializeReturnAddress)
  activeBuffer = nil
  activeSize = nil
  pcall(detachIfPossible)
  print(string.format(
    '[save record serialization] stopped; entries=%d writes=%d returns=%d',
    #entries,
    #writes,
    #returns
  ))
end

function debugger_onBreakpoint()
  if RIP == serializeAddress then
    removeWatch()
    activeBuffer = RDX
    local sourcePayload = readPointer(RCX + 0xE0)
    local payloadSize = readQword(RCX + 0xE8)
    activeSize = payloadSize and (payloadSize + 0x158) or nil

    local entryCapture = {
      manager = RCX,
      outputBuffer = activeBuffer,
      sourcePayload = sourcePayload,
      payloadSize = payloadSize,
    }
    if sourcePayload ~= nil and payloadSize ~= nil and payloadSize > 0 then
      if payloadSize >= TARGET_RECORD_OFFSET + TARGET_RECORD_SIZE - 0x158 then
        local sourceRecordAddress = sourcePayload + TARGET_RECORD_OFFSET - 0x158
        entryCapture.sourceRecordAddress = sourceRecordAddress
        entryCapture.sourceRecordSeed = unsigned32(
          readInteger(sourceRecordAddress + 0x20)
        )
        entryCapture.sourceRecordHex = bytesToHex(
          sourceRecordAddress,
          TARGET_RECORD_SIZE
        )
      end
      if not capturedPayloadSizes[payloadSize] then
        capturedPayloadSizes[payloadSize] = true
        local filename = string.format(
          '%s\\plaintext-payload-%X.bin',
          OUTPUT_DIRECTORY,
          payloadSize
        )
        local bytesWritten = writeRegionToFile(
          filename,
          sourcePayload,
          payloadSize
        )
        entryCapture.payloadFilename = filename
        entryCapture.payloadBytesWritten = bytesWritten
        payloadCaptures[#payloadCaptures + 1] = {
          filename = filename,
          payloadSize = payloadSize,
          bytesWritten = bytesWritten,
          sourcePayload = sourcePayload,
        }
        print(string.format(
          '[save record serialization] captured plaintext payload size=0x%X bytes=%s',
          payloadSize,
          tostring(bytesWritten)
        ))
      end
    end

    if activeSize ~= nil and activeSize >= TARGET_RECORD_OFFSET + TARGET_RECORD_SIZE then
      local outputSnapshot = snapshot('entry', RIP)
      for key, value in pairs(outputSnapshot) do entryCapture[key] = value end
      entries[#entries + 1] = entryCapture
      activeWatch = activeBuffer + TARGET_RECORD_OFFSET
      local armed = debug_setBreakpoint(
        activeWatch,
        4,
        bptWrite,
        bpmDebugRegister
      )
      if not armed then
        activeWatch = nil
        print('[save record serialization] failed to arm record write watch')
      else
        print(string.format(
          '[save record serialization] armed buffer=0x%X size=0x%X watch=0x%X',
          activeBuffer,
          activeSize,
          activeWatch
        ))
      end
    else
      entries[#entries + 1] = entryCapture
      activeBuffer = nil
      activeSize = nil
    end
  elseif RIP == serializeReturnAddress then
    if activeBuffer ~= nil then
      returns[#returns + 1] = snapshot('return', RIP)
      print(string.format(
        '[save record serialization] return seed-at-offset=%s writes=%d',
        tostring(returns[#returns].seedAtOffset),
        #writes
      ))
      removeWatch()
      activeBuffer = nil
      activeSize = nil
    end
  elseif activeWatch ~= nil and #writes < MAX_WRITE_CAPTURES then
    writes[#writes + 1] = snapshot('write', RIP)
    print(string.format(
      '[save record serialization] write=%d rva=0x%X seed-at-offset=%s',
      #writes,
      RIP - moduleBase,
      tostring(writes[#writes].seedAtOffset)
    ))
    if #writes >= MAX_WRITE_CAPTURES then
      removeWatch()
      print('[save record serialization] safety stop after maximum captures')
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
if not debug_setBreakpoint(serializeAddress) then
  error('Could not arm serializer entry breakpoint')
end
if not debug_setBreakpoint(serializeReturnAddress) then
  error('Could not arm serializer return breakpoint')
end

print(string.format(
  '[save record serialization] armed target Seed %u at decrypted offset 0x%X',
  TARGET_SEED,
  TARGET_RECORD_OFFSET
))
print('[save record serialization] trigger exactly one game save')
