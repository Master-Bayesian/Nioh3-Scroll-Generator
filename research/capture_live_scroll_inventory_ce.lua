-- PC v2.01 read-only inventory evidence. Run through CE Lua on the attached game.
-- Existing inventory is NOT evidence of natural generation: records may be edited.
local base = assert(getAddressSafe('Nioh3.exe'), 'Nioh3.exe is not attached')
local signature = {0x40,0x55,0x53,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x8D,0xAC}
local actual = assert(readBytes(base + 0x54D294, #signature, true), 'Unreadable insertion entry')
for index, value in ipairs(signature) do
  assert(actual[index] == value, 'PC v2.01 insertion signature mismatch')
end
local function hex(value) return string.format('0x%X', value) end
local function recordHex(address)
  local bytes = assert(readBytes(address, 0xE8, true), 'Unreadable scroll record')
  assert(#bytes == 0xE8, 'Partial scroll record')
  local encoded = {}
  for index, value in ipairs(bytes) do encoded[index] = string.format('%02x', value) end
  return table.concat(encoded)
end
local manager = assert(readQword(base + 0x474D4E0), 'Unreadable item manager')
assert(manager ~= 0, 'Item manager is null')
local data = assert(readQword(manager), 'Unreadable inventory data')
assert(data ~= 0, 'Inventory data is null')
local container = data + 0x224A60
assert(readQword(container + 0x16A80) == 400, 'Unexpected scroll capacity')
local entries = {}
for index = 0, 399 do
  local address = container + index * 0xE8
  local kind = assert(readSmallInteger(address), 'Unreadable scroll slot') % 0x10000
  if kind ~= 0 then
    local before = recordHex(address)
    assert(recordHex(address) == before, 'Scroll record changed during capture; retry at rest')
    entries[#entries + 1] = {slot_index=index, address=hex(address), record_hex=before,
      provenance={kind='unknown', basis='Existing inventory may include prior experiments'},
      natural_generation_evidence=false}
  end
end
assert(readQword(base + 0x474D4E0) == manager and readQword(manager) == data,
  'Inventory owner changed during capture')
return {schema='nioh3-live-scroll-inventory-read/v1', pid=getOpenedProcessID(),
  module_base=hex(base), manager=hex(manager), data=hex(data), capacity=400,
  serial_counter=readQword(data+8), read_only=true, entries=entries,
  provenance={kind='unknown', basis='User reports prior experiments among existing scrolls'},
  natural_generation_evidence=false,
  consistency='owner and per-record double-read; not an atomic inventory snapshot',
  scope='Runtime container evidence only; existing records may include experiments'}
