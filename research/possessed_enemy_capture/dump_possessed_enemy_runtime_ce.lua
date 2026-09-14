-- Read-only fallback dumper for environments where the CE MCP server owns the
-- bridge port. It writes raw process bytes to local files but never writes game
-- memory or calls game code.
local output = assert(NIOH3_POSSESSED_DUMP_DIR, 'NIOH3_POSSESSED_DUMP_DIR is required')
local base = assert(getAddressSafe('Nioh3.exe'), 'Nioh3.exe is not attached')
local pid = assert(getOpenedProcessID(), 'No attached process')
local function hex(value) return string.format('0x%X', value or 0) end
local function u64(address, label)
  local ok,value=pcall(readQword,address)
  assert(ok and type(value)=='number', 'Unreadable '..label)
  return value
end
local function u32(address, label)
  local ok,value=pcall(readInteger,address)
  assert(ok and type(value)=='number', 'Unreadable '..label)
  return value & 0xFFFFFFFF
end
local function writeRange(filename, address, size)
  assert(address and address~=0 and size>=0, 'Invalid range for '..filename)
  local file=assert(io.open(output..'/'..filename,'wb'))
  local ok,failure=pcall(function()
    local offset=0
    while offset<size do
      local count=math.min(0x1000,size-offset)
      local bytes=readBytes(address+offset,count,true)
      assert(bytes and #bytes==count, 'Partial read for '..filename)
      local chars={}
      for i=1,count do chars[i]=string.char(bytes[i]) end
      assert(file:write(table.concat(chars)))
      offset=offset+count
    end
  end)
  file:close()
  assert(ok,failure)
  return {file=filename,address=hex(address),size=size}
end
local contextGlobal=u64(base+0x474D4E0,'scroll context global')
local dataRoot=u64(contextGlobal,'scroll data root')
assert(dataRoot~=0,'Scroll data root is null')
local manager=u64(base+0x45B5DF0,'parameter manager')
assert(manager~=0,'Parameter manager is null')
local result={schema='nioh3-pc-v2.01-possessed-runtime-raw/v2',read_only=true,
  writes_game_memory=false,pid=pid,module_base=hex(base),run_id=NIOH3_POSSESSED_RUN_ID or 'unspecified'}
result.scroll_lookup={context_global_address=hex(base+0x474D4E0),context_address=hex(contextGlobal),
  data_root_address=hex(dataRoot),capacity=400,stride=0xE8,
  container=writeRange('scroll_container_400xE8.bin',dataRoot+0x224A60,400*0xE8),
  metadata=writeRange('scroll_container_metadata.bin',dataRoot+0x16A80,0x80)}
result.enemy_weight_database={manager_global_address=hex(base+0x45B5DF0),manager_address=hex(manager),
  selector=readBytes(manager+0xB0A,1,true)[1],manager=writeRange('parameter_manager.bin',manager,0xC00),contexts={}}
for _,offset in ipairs({0x38,0x40}) do
  local context=u64(manager+offset,'enemy context')
  local rowTable=u64(context,'enemy row table')
  local rowCount=u32(rowTable+4,'enemy row count')
  assert(rowCount<=100000,'Enemy row count exceeds bound')
  local hashObject=u64(context+0x20,'enemy hash object')
  local hashBegin=u64(hashObject+8,'enemy hash begin')
  local hashEnd=u64(hashObject+0x10,'enemy hash end')
  assert(hashEnd>=hashBegin and (hashEnd-hashBegin)%8==0,'Invalid enemy hash range')
  local hashCount=(hashEnd-hashBegin)/8
  assert(hashCount<=500000,'Enemy hash count exceeds bound')
  local prefix=string.format('enemy_context_%02x',offset)
  result.enemy_weight_database.contexts[#result.enemy_weight_database.contexts+1]={
    manager_offset=hex(offset),context_address=hex(context),row_table_address=hex(rowTable),
    row_count=rowCount,row_stride=0x398,hash_object_address=hex(hashObject),
    hash_begin=hex(hashBegin),hash_end=hex(hashEnd),hash_entry_count=hashCount,
    hash_entry_layout='little-endian u32 lookup_key, u32 row_index; one trailing entry retained',
    context=writeRange(prefix..'_context.bin',context,0x80),
    rows=writeRange(prefix..'_rows.bin',rowTable,8+rowCount*0x398),
    hash=writeRange(prefix..'_hash.bin',hashBegin,(hashCount+1)*8),
  }
end
local holder=u64(base+0x45B5E00,'configuration holder')
local configRoot=holder~=0 and u64(holder,'configuration root') or 0
local fallback=u64(manager+0x230,'fallback config context')
result.configuration={holder_global_address=hex(base+0x45B5E00),holder_address=hex(holder),
  root_address=hex(configRoot),lookup_context_offset='0x2820',fallback_context_address=hex(fallback)}
if configRoot~=0 then result.configuration.root=writeRange('configuration_root.bin',configRoot,0x2A20) end
if fallback~=0 then result.configuration.fallback_context=writeRange('configuration_fallback_context.bin',fallback,0x400) end
return result
