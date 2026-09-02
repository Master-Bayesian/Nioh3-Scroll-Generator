-- Nioh 3 PC v2.00.02, read-only save-write origin probe.
--
-- This script observes save file opens and writes. It records paths, handles,
-- buffer sizes, a short buffer prefix, the direct caller, and values found on
-- the stack that point into Nioh3.exe. It never writes game memory or save data.

local MODULE_NAME = 'Nioh3.exe'
local MODULE_VERSION = '2.00.02'
local MAX_OPEN_CAPTURES = 64
local MAX_WRITE_CAPTURES = 256
local UNKNOWN_WRITE_MINIMUM = 0x10000
local PREFIX_BYTES = 64
local STACK_QWORDS = 48

local function unsigned32(value)
  if value == nil then return nil end
  if value < 0 then return value + 0x100000000 end
  return value
end

local function addressText(address)
  if address == nil then return 'nil' end
  return string.format('0x%X', address)
end

local function safeReadPointer(address)
  local ok, value = pcall(readPointer, address)
  if not ok then return nil end
  return value
end

local function safeReadWideString(address)
  if address == nil or address == 0 then return nil end
  local ok, value = pcall(readString, address, 1024, true)
  if not ok then return nil end
  return value
end

local function bytesToHex(address, count)
  if address == nil or address == 0 or count <= 0 then return nil end
  local ok, bytes = pcall(readBytes, address, count, true)
  if not ok or bytes == nil then return nil end
  local result = {}
  for index = 1, #bytes do
    result[index] = string.format('%02X', bytes[index])
  end
  return table.concat(result)
end

local function isSavePath(path)
  if path == nil then return false end
  local normalized = string.upper(path)
  return string.find(normalized, 'SAVEDATA', 1, true) ~= nil
end

openProcess(MODULE_NAME)
local moduleBase = getAddressSafe(MODULE_NAME)
local moduleSize = getModuleSize(MODULE_NAME)
if moduleBase == nil or moduleBase == 0 or moduleSize == nil then
  error('Could not resolve Nioh3.exe module range')
end
local moduleEnd = moduleBase + moduleSize

local createFileAddress = getAddressSafe('KERNELBASE.CreateFileW')
local writeFileAddress = getAddressSafe('KERNELBASE.WriteFile')
local ntWriteFileAddress = getAddressSafe('ntdll.NtWriteFile')
if createFileAddress == nil or writeFileAddress == nil or ntWriteFileAddress == nil then
  error('Could not resolve required Windows file APIs')
end

local openCaptures = {}
local writeCaptures = {}
local saveHandles = {}
local pendingReturns = {}
local stopped = false

local function isGameAddress(address)
  return address ~= nil and address >= moduleBase and address < moduleEnd
end

local function describeAddress(address)
  if address == nil then return nil end
  local result = {
    address = address,
    text = addressText(address),
    symbol = getNameFromAddress(address, true, true, false),
  }
  if isGameAddress(address) then
    result.module = MODULE_NAME
    result.rva = address - moduleBase
  end
  return result
end

local function captureGameStackCandidates(stackPointer)
  local result = {}
  local seen = {}
  for index = 0, STACK_QWORDS - 1 do
    local value = safeReadPointer(stackPointer + index * 8)
    if isGameAddress(value) and not seen[value] then
      seen[value] = true
      result[#result + 1] = {
        stackOffset = index * 8,
        address = value,
        rva = value - moduleBase,
        symbol = getNameFromAddress(value, true, true, false),
      }
    end
  end
  return result
end

local function addWriteCapture(apiName, handle, buffer, byteCount, stackPointer)
  if #writeCaptures >= MAX_WRITE_CAPTURES then return end
  local path = saveHandles[handle]
  if path == nil and byteCount < UNKNOWN_WRITE_MINIMUM then return end

  local caller = safeReadPointer(stackPointer)
  local prefixCount = math.min(byteCount, PREFIX_BYTES)
  local capture = {
    api = apiName,
    handle = handle,
    handleText = addressText(handle),
    path = path,
    knownSaveHandle = path ~= nil,
    buffer = buffer,
    bufferText = addressText(buffer),
    byteCount = byteCount,
    prefixHex = bytesToHex(buffer, prefixCount),
    caller = describeAddress(caller),
    gameStackCandidates = captureGameStackCandidates(stackPointer),
  }
  writeCaptures[#writeCaptures + 1] = capture

  print(string.format(
    '[save write] %s handle=%s bytes=%d path=%s caller=%s',
    apiName,
    addressText(handle),
    byteCount,
    tostring(path),
    addressText(caller)
  ))
end

local function armCreateFileReturn(returnAddress, path)
  if returnAddress == nil or returnAddress == 0 then return end
  local queue = pendingReturns[returnAddress]
  if queue == nil then
    queue = {}
    pendingReturns[returnAddress] = queue
    if not debug_setBreakpoint(returnAddress) then
      pendingReturns[returnAddress] = nil
      return
    end
  end
  queue[#queue + 1] = path
end

function nioh3GetSaveWriteOriginStatus()
  return {
    moduleVersion = MODULE_VERSION,
    moduleBase = moduleBase,
    moduleSize = moduleSize,
    apiAddresses = {
      createFileW = createFileAddress,
      writeFile = writeFileAddress,
      ntWriteFile = ntWriteFileAddress,
    },
    openCaptures = openCaptures,
    writeCaptures = writeCaptures,
    saveHandles = saveHandles,
    stopped = stopped,
  }
end

function nioh3ClearSaveWriteOriginCaptures()
  openCaptures = {}
  writeCaptures = {}
  print('[save write] captures cleared')
end

function nioh3StopSaveWriteOriginProbe()
  if stopped then return end
  stopped = true
  pcall(debug_removeBreakpoint, createFileAddress)
  pcall(debug_removeBreakpoint, writeFileAddress)
  pcall(debug_removeBreakpoint, ntWriteFileAddress)
  for returnAddress, _ in pairs(pendingReturns) do
    pcall(debug_removeBreakpoint, returnAddress)
  end
  pendingReturns = {}
  pcall(detachIfPossible)
  print(string.format(
    '[save write] stopped; opens=%d writes=%d',
    #openCaptures,
    #writeCaptures
  ))
end

function debugger_onBreakpoint()
  if RIP == createFileAddress then
    local path = safeReadWideString(RCX)
    if isSavePath(path) and #openCaptures < MAX_OPEN_CAPTURES then
      local returnAddress = safeReadPointer(RSP)
      openCaptures[#openCaptures + 1] = {
        stage = 'entry',
        path = path,
        returnAddress = describeAddress(returnAddress),
      }
      armCreateFileReturn(returnAddress, path)
      print(string.format('[save open] path=%s return=%s', path, addressText(returnAddress)))
    end
  elseif RIP == writeFileAddress then
    addWriteCapture('WriteFile', RCX, RDX, unsigned32(R8), RSP)
  elseif RIP == ntWriteFileAddress then
    local buffer = safeReadPointer(RSP + 0x30)
    local byteCount = unsigned32(readInteger(RSP + 0x38))
    addWriteCapture('NtWriteFile', RCX, buffer, byteCount, RSP)
  else
    local queue = pendingReturns[RIP]
    if queue ~= nil then
      local path = table.remove(queue, 1)
      if path ~= nil then
        saveHandles[RAX] = path
        if #openCaptures < MAX_OPEN_CAPTURES then
          openCaptures[#openCaptures + 1] = {
            stage = 'return',
            path = path,
            handle = RAX,
            handleText = addressText(RAX),
          }
        end
        print(string.format('[save open] handle=%s path=%s', addressText(RAX), path))
      end
      if #queue == 0 then
        pendingReturns[RIP] = nil
        pcall(debug_removeBreakpoint, RIP)
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
if not debug_setBreakpoint(createFileAddress) then
  error('Could not arm KERNELBASE.CreateFileW breakpoint')
end
if not debug_setBreakpoint(writeFileAddress) then
  error('Could not arm KERNELBASE.WriteFile breakpoint')
end
if not debug_setBreakpoint(ntWriteFileAddress) then
  error('Could not arm ntdll.NtWriteFile breakpoint')
end

print('[save write] armed read-only CreateFileW/WriteFile/NtWriteFile probe')
print('[save write] trigger exactly one game save, then inspect nioh3GetSaveWriteOriginStatus()')
