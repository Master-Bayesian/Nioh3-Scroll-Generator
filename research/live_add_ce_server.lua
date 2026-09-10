-- Optional typed local executor. No request is evaluated as Lua or a file path.
-- Start explicitly: dofile(thisFile).start(pipeName, secretToken, scriptRoot).
local M={}
local function json(value,seen)
  local t=type(value)
  if t=='nil' then return 'null' end
  if t=='boolean' then return value and 'true' or 'false' end
  if t=='number' then return tostring(value) end
  if t=='string' then
    return '"'..value:gsub('[%z\1-\31\\"]',function(c)
      if c=='"' or c=='\\' then return '\\'..c end
      return string.format('\\u%04x',c:byte()) end)..'"'
  end
  assert(t=='table','Unsupported response value')
  seen=seen or {};assert(not seen[value],'Cyclic response');seen[value]=true
  local out={};local array=#value>0
  if array then for _,v in ipairs(value)do out[#out+1]=json(v,seen)end
  else for k,v in pairs(value)do out[#out+1]=json(tostring(k))..':'..json(v,seen)end end
  seen[value]=nil
  return (array and '[' or '{')..table.concat(out,',')..(array and ']' or '}')
end
local function hex(bytes)
  if not bytes then return nil end
  local out={};for i,v in ipairs(bytes)do out[i]=string.format('%02X',v)end
  return table.concat(out)
end
local function parse(raw)
  assert(#raw<=512*1024 and raw:sub(-1)=='\n','Invalid request frame')
  local result={}
  for line in raw:gmatch('([^\n]+)\n')do
    local k,v=line:match('^([a-z_]+)=([%w_.%-]+)$')
    assert(k and not result[k],'Invalid or duplicate request field');result[k]=v
  end
  return result
end
function M.start(pipeName,token,root)
  assert(type(pipeName)=='string' and pipeName:match('^nioh3%-live%-add%-%x+$') and #pipeName==47)
  assert(type(token)=='string' and token:match('^%x+$') and #token==64)
  assert(not nioh3LiveAddServer or nioh3LiveAddServer.stopped,'Executor already running')
  assert(root:sub(-1)=='/','Expected a local script directory')
  local server=assert(createPipe(pipeName,512*1024,512*1024,1))
  assert(server.valid,'Cannot create executor pipe')
  server.Timeout=5
  local state={stopped=false,requests=0,pipe=server,operation=nil,receipts={}}
  nioh3LiveAddServer=state
  local function snapshot(p)
    local b=debug_getBreakpointList()
    return {operation_id=p.operation_id,phase=p.phase,mode=p.mode,pid=p.pid,
      active=p.active,released=p.released==true,redirect_count=p.redirect_count,
      cleanup_pending=p.cleanup_pending,error=p.error,stop_reason=p.stop_reason,
      before=p.before,after=p.after,slot=p.inserted_slot,status=p.insertion_status,
      source_hex=hex(p.output_record),destination_hex=hex(p.destination),
      remainder_hex=hex(p.remainder),breakpoint_count=type(b)=='table' and #b or -1}
  end
  local function handle(q)
    assert(q.token==token,'Connection token differs')
    local method=q.method
    if method=='ping' then
      return {pid=getOpenedProcessID(),protocol=1,profile_id='pc-v2.01-live-add-r1',
        busy=nioh3DispatchNoop and not nioh3DispatchNoop.released or false}
    end
    if method=='stop' then
      assert(not nioh3DispatchNoop or nioh3DispatchNoop.released,'Remote ownership remains')
      state.stopped=true;return {stopped=true}
    end
    assert(q.operation_id and #q.operation_id==36 and q.operation_id:match('^[%x%-]+$'),'Invalid operation ID')
    if method=='status' or method=='release' then
      if state.receipts[q.operation_id] then return state.receipts[q.operation_id] end
      assert(state.operation==q.operation_id,'Unknown operation; do not replay')
      local p=assert(nioh3DispatchNoop)
      if method=='release' then p.retry_cleanup();p.release() end
      local result=snapshot(p)
      if p.released then state.receipts[q.operation_id]=result end
      return result
    end
    assert(method=='preview' or method=='insert','Unknown executor method')
    assert(q.profile_id=='pc-v2.01-live-add-r1','Unsupported executor profile')
    assert(not state.receipts[q.operation_id] and state.operation~=q.operation_id,'Operation already submitted')
    assert(not nioh3DispatchNoop or nioh3DispatchNoop.released,'Previous execution retains ownership')
    local function number(name)
      assert(q[name] and q[name]:match('^%d+$'),'Missing integer field '..name)
      return assert(math.tointeger(tonumber(q[name])),'Invalid integer '..name)
    end
    assert(number('pid')==getOpenedProcessID(),'Process changed')
    local options={assembly_preview={descriptor_hex=q.descriptor_hex,
      expected_record_hex=q.expected_record_hex,builder_code_hex=q.builder_code_hex}}
    if method=='insert' then
      options.single_insertion={operation_id=q.operation_id,pid=number('pid'),manager=number('manager'),
        data=number('data'),serial=number('serial'),slot=number('slot'),scheduler_owner=number('scheduler_owner'),
        function_address=number('function_address'),container_hex=q.container_hex,insertion_code_hex=q.insertion_code_hex}
    end
    state.operation=q.operation_id
    nioh3DispatchProbeOptions=options
    local previous=nioh3DispatchNoop
    local ok,err=pcall(dofile,root..'probe_pickup_dispatch_noop_ce.lua')
    if not ok and nioh3DispatchNoop==previous then
      local result={operation_id=q.operation_id,phase='rejected',redirect_count=0,
        released=true,active=false,error=tostring(err),breakpoint_count=0,pid=getOpenedProcessID()}
      state.receipts[q.operation_id]=result
      return result
    end
    if nioh3DispatchNoop and not nioh3DispatchNoop.released then
      nioh3DispatchNoop.operation_id=q.operation_id
    end
    if not ok then error(err) end
    return snapshot(nioh3DispatchNoop)
  end
  state.thread=createThread(function()
    while not state.stopped do
      local connected=pcall(function()
        server.acceptConnection()
        if not server.Connected then return end
        local size=assert(server.readDword(),'Request disconnected')
        assert(size>0 and size<=512*1024,'Request too large')
        local raw=assert(server.readString(size),'Partial request')
        local ok,result=pcall(function()return handle(parse(raw))end)
        local response=json({protocol=1,ok=ok,result=ok and result or nil,error=not ok and tostring(result) or nil})
        assert(server.writeDword(#response));assert(server.writeString(response))
        state.requests=state.requests+1
      end)
      -- A fresh single-instance pipe disconnects the previous client. Remote
      -- operation ownership remains in the probe independently of this pipe.
      server.destroy()
      if not state.stopped then
        server=createPipe(pipeName,512*1024,512*1024,1)
        if not server or not server.valid then state.error='Pipe recreation failed';break end
        server.Timeout=5;state.pipe=server
      end
    end
    state.stopped=true
  end)
  return {pipe_name=pipeName,protocol=1}
end
return M
