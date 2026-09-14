-- Serialize the current read-only capture directly from CE to avoid MCP output
-- truncation. Functions and unsupported userdata are omitted.
local path = assert(NIOH3_POSSESSED_OUTPUT_PATH, 'NIOH3_POSSESSED_OUTPUT_PATH is required')
local probe = assert(nioh3PossessedCapture, 'No possessed-enemy capture exists')
local function escape(value)
  return value:gsub('[%z\1-\31\\"]', function(character)
    local replacements={['\\']='\\\\',['"']='\\"',['\b']='\\b',['\f']='\\f',['\n']='\\n',['\r']='\\r',['\t']='\\t'}
    return replacements[character] or string.format('\\u%04X',string.byte(character))
  end)
end
local function encode(value, seen)
  local kind=type(value)
  if kind=='nil' then return 'null' end
  if kind=='boolean' then return value and 'true' or 'false' end
  if kind=='number' then
    if value~=value or value==math.huge or value==-math.huge then return 'null' end
    return string.format('%.17g',value)
  end
  if kind=='string' then return '"'..escape(value)..'"' end
  if kind~='table' then return nil end
  assert(not seen[value],'Cycle in capture table')
  seen[value]=true
  local count,maxIndex,isArray=0,0,true
  for key,item in pairs(value) do
    if type(item)~='function' then
      count=count+1
      if type(key)~='number' or key<1 or key%1~=0 then isArray=false
      else if key>maxIndex then maxIndex=key end end
    end
  end
  if isArray and maxIndex~=count then isArray=false end
  local parts={}
  if isArray then
    for index=1,maxIndex do parts[#parts+1]=encode(value[index],seen) or 'null' end
    seen[value]=nil
    return '['..table.concat(parts,',')..']'
  end
  local keys={}
  for key,item in pairs(value) do
    if type(item)~='function' and (type(key)=='string' or type(key)=='number') then keys[#keys+1]=key end
  end
  table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
  for _,key in ipairs(keys) do
    local encoded=encode(value[key],seen)
    if encoded then parts[#parts+1]='"'..escape(tostring(key))..'":'..encoded end
  end
  seen[value]=nil
  return '{'..table.concat(parts,',')..'}'
end
local envelope={
  capture_metadata={
    captured_at_utc=os.date('!%Y-%m-%dT%H:%M:%SZ'),
    phase=NIOH3_POSSESSED_PHASE or 'unknown',
    run_id=probe.run_id,
    requested_pid=probe.pid,
  },
  probe=probe,
  breakpoints=debug_getBreakpointList(),
}
local document=assert(encode(envelope,{}),'Could not encode capture')
local file=assert(io.open(path,'wb'))
assert(file:write(document))
file:close()
return {path=path,bytes=#document,event_count=#probe.events,active=probe.active,
  cleanup_pending=probe.cleanup_pending,stop_reason=probe.stop_reason}
