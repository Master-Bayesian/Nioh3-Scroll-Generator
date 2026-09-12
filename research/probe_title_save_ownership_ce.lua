-- Research only: hardware execute breakpoints, no target writes or native calls.
-- Load this file, then call TitleSaveOwnership.start(options). Nothing auto-arms.
local M = {}
local PROFILES = {
  ownership = {'request_entry','request_exit','task_bind_entry','completion_consume'},
  snapshots = {'queue_capture_entry','snapshot_ready','queue_pop_entry','serializer_entry'},
  serialization = {'serializer_entry','serializer_copyback','serializer_exit','coordinator_exit'},
  files = {'writer_entry','writer_write_result','writer_rename_result','writer_exit'},
  file_handles = {'writer_open_result','writer_flush_result','writer_close_result','writer_exit'},
  loading = {'read_entry','read_exit','apply_entry','apply_exit'},
  registry = {'snapshot_entry','registry_dispatch','snapshot_ready','apply_entry'},
  worker = {'task_bind_entry','worker_start','worker_dispatch','coordinator_exit'},
}
M.profiles = PROFILES
local function hex(n) if type(n)=='number' then return string.format('0x%X',n) end end
local function u32(n) if type(n)=='number' then return n & 0xffffffff end end
local function byte(n) if type(n)=='number' then return n & 0xff end end
local function label(s)
  return type(s)=='string' and #s>=1 and #s<=64 and s:match('^[A-Za-z0-9][A-Za-z0-9_.-]*$')
end
local function js(s)
  return '"'..s:gsub('[%z\1-\31\\"]',function(c)
    if c=='"' or c=='\\' then return '\\'..c end
    return string.format('\\u%04x',c:byte())
  end)..'"'
end
local function json(v,depth)
  depth=depth or 0; assert(depth<16,'JSON depth limit')
  local t=type(v)
  if t=='nil' then return 'null' elseif t=='boolean' then return v and 'true' or 'false'
  elseif t=='number' then assert(v==v and math.abs(v)<math.huge,'nonfinite'); return tostring(v)
  elseif t=='string' then assert(#v<=4096,'JSON string limit'); return js(v) end
  assert(t=='table','Unsupported JSON type')
  local keys={};for k in pairs(v) do keys[#keys+1]=k end
  local arr=#keys>0
  for _,k in ipairs(keys) do if type(k)~='number' or k<1 or k>#v or k%1~=0 then arr=false end end
  local out={}
  if arr then for _,x in ipairs(v) do out[#out+1]=json(x,depth+1) end;return '['..table.concat(out,',')..']' end
  table.sort(keys);for _,k in ipairs(keys) do out[#out+1]=js(k)..':'..json(v[k],depth+1) end
  return '{'..table.concat(out,',')..'}'
end
M.encode_json=json

-- Dependency injection is used by executable Lua tests, not as an oracle stub.
-- The production adapter below implements every operation used here.
function M.new(api,cfg,ownerFactory,loc)
  assert(label(cfg.run_id),'Use a unique safe run_id')
  assert(PROFILES[cfg.profile],'Unknown observation profile')
  assert(type(cfg.account_key)=='string' and #cfg.account_key>=16 and #cfg.account_key<=64 and cfg.account_key:match('^%x+$'),'Disk account_key required')
  assert(type(cfg.slot)=='number' and cfg.slot>=0 and cfg.slot<=99 and cfg.slot%1==0,'slot required')
  local limit=cfg.max_events or 1024
  assert(limit>=4 and limit<=4096 and limit%1==0,'max_events must be 4..4096')
  local state={schema='nioh3.title-save-observer/v1',run_id=cfg.run_id,profile=cfg.profile,
    account_key=cfg.account_key,slot=cfg.slot,account_binding='unverified_disk_to_native',
    release='BLOCK',live_acceptance=false,active=false,events={},errors={},dropped=0,
    epoch=0,sequence=0,bytes_logged=0,hash_bytes=0,cleanup_pending=false,owned_breakpoints={}}
  local owner,identity,base,attachment
  local requests,accounts,paths,writers={},{},{},{}
  local nrequests,naccounts,npaths,nwriters=0,0,0,0
  local MAX_READ=512
  local function read(a,n)
    if type(a)~='number' or a<0x10000 or n<0 or n>MAX_READ then return nil end
    local ok,b=pcall(api.read,a,n)
    if not ok or type(b)~='table' or #b~=n then return nil end
    return b
  end
  local function num(a,n)
    local b=read(a,n);if not b then return nil end
    local v=0;for i=n,1,-1 do v=(v<<8)|b[i] end;return v
  end
  local function ptr(a) return num(a,8) end
  local function field(a,o,n) if type(a)=='number' and a>=0x10000 then return num(a+o,n) end end
  local function pfield(a,o) return hex(field(a,o,8)) end
  local function rva(a)
    if type(a)=='number' and a>=base+loc.text_rva and a<base+loc.text_rva+loc.text_size then return hex(a-base) end
  end
  local function token(map,key,prefix)
    if not key then return nil end
    if map[key] then return map[key] end
    if prefix=='account' then
      if naccounts>=16 then return 'account_overflow' end;naccounts=naccounts+1;map[key]='account_'..naccounts
    else
      if npaths>=32 then return 'path_overflow' end;npaths=npaths+1;map[key]='path_'..npaths
    end
    return map[key]
  end
  local function wide(a)
    -- Bound by 256 UTF-16 code units; no readString defaults or unbounded pointer traversal.
    local b=read(a,512);if not b then return nil end
    local out={}
    for i=1,#b-1,2 do local c=b[i]+256*b[i+1];if c==0 then break end
      if c>=32 and c<=126 then out[#out+1]=string.char(c) else out[#out+1]='?' end
    end
    return table.concat(out)
  end
  local function pathinfo(a)
    local s=wide(a);if not s then return {readable=false} end
    local up=s:upper():gsub('/','\\')
    local tail=up:match('(SYSTEMSAVEDATA%d%d.*)') or up:match('(SAVEDATA%d%d.*)')
    local leaf=up:match('([^\\]+)\\?$')
    if not tail then
      if leaf=='SAVEDATA.BIN' or leaf=='BACKUP.BIN' or leaf=='SAVEDATA.BIN.TMP' or leaf=='BACKUP.BIN.TMP' then tail=leaf end
    end
    return {readable=true,path_ref=token(paths,s,'path'),save_tail=tail and tail:sub(1,128) or nil,
      truncated=(#s>=256),redacted=true}
  end
  local function context(h)
    if not h or h<0x10000 then return {readable=false} end
    local f={};for _,o in ipairs({0x38,0x39,0x3a,0x3b,0x3c,0x3d,0x3e,0x3f,0x40,0x41,0x42}) do f[hex(o)]=field(h,o,1) end
    return {address=hex(h),character_stage=pfield(h,0),system_stage=pfield(h,8),
      result=field(h,0x18,4),slot=field(h,0x28,4),option34=field(h,0x34,4),fields=f}
  end
  local function task(s)
    if not s or s<0x10000 then return {readable=false} end
    local thread=field(s,8,8)
    return {address=hex(s),vtable=pfield(s,0),worker_object=hex(thread),worker_tid=field(thread,0x10,4),
      worker_handle=pfield(thread,0x18),account_ref=token(accounts,pfield(s,0xb0),'account'),
      error=field(s,0xb8,4),operation=field(s,0xd0,4),format=field(s,0xd8,8),
      payload=pfield(s,0xe0),payload_bytes=field(s,0xe8,8),slot=field(s,0xe1c,4),
      system=field(s,0xe27,1),option_e24=field(s,0xe24,1),option_e25=field(s,0xe25,1),
      controller_phase=field(s,0xe8f8,4),worker_phase=field(s,0xe8fc,4)}
  end
  local function globals()
    local r=ptr(base+0x45c4448);local h=field(r,0,8);local s=ptr(base+0x45c2e80)
    local begin=field(r,0x18,8);local finish=field(r,0x20,8)
    local out={root=hex(r),root_phase=field(r,0x10,4),context=context(h),task=task(s),
      snapshot_mutex=hex(base+0x4b56780),queue={}}
    if begin and finish and finish>=begin and (finish-begin)%8==0 and (finish-begin)<=16 then
      out.queue_count=(finish-begin)//8
      for i=0,out.queue_count-1 do local q=ptr(begin+8*i)
        out.queue[#out.queue+1]={address=hex(q),operation=field(q,0,4),slot=field(q,4,4),system=field(q,8,1),snapshot=pfield(q,0x38)}
      end
    else out.queue_readable=false end
    return out
  end
  local function signature(site)
    local b=read(base+site.signature_rva,#site.bytes//2)
    if not b then return false end
    for i=1,#b do
      local m=tonumber(site.mask:sub(2*i-1,2*i),16)
      local e=tonumber(site.bytes:sub(2*i-1,2*i),16)
      if (b[i]&m)~=(e&m) then return false end
    end
    return true
  end
  local function err(message)
    if #state.errors<32 then state.errors[#state.errors+1]=tostring(message):sub(1,512) end
  end
  local function halt(reason)
    state.active=false;state.stop_reason=reason
    if owner then owner.stop() end
    if not state.cleanup_pending and #state.owned_breakpoints==0 and api.unwatch then api.unwatch() end
  end
  local function emit(e)
    if #state.events>=limit then state.dropped=state.dropped+1;halt('event_limit');return end
    local line=json(e)
    if #line>65536 or state.bytes_logged+#line>8*1024*1024 then state.dropped=state.dropped+1;halt('byte_limit');return end
    state.events[#state.events+1]=e;state.bytes_logged=state.bytes_logged+#line
    local ok,res=pcall(api.emit,line)
    if not ok or res==false then err('Local log sink failed');halt('log_sink_failure') end
  end
  local function digest(a,n)
    if not cfg.hash_payloads then return {status='disabled'} end
    if not a or not n or n<=0 or n>0x9001b0 or state.hash_bytes+n>64*1024*1024 then return {status='budget_or_range_rejected'} end
    state.hash_bytes=state.hash_bytes+n
    local ok,h=pcall(api.md5,a,n)
    if ok and type(h)=='string' and #h==32 and h:match('^%x+$') then
      return {status='ok',md5=h:upper(),bytes=n,purpose='investigative_content_fingerprint_not_generation_ack'}
    end
    return {status='unreadable'}
  end
  local function capture(name)
    local regs=api.context();assert(type(regs.THREADID)=='number' and regs.THREADID>0,'Missing debug THREADID')
    assert(api.pid()==identity.pid and api.base()==base and api.attachment()==attachment,'CE process attachment changed')
    local e={schema=state.schema,run_id=state.run_id,profile=state.profile,epoch=state.epoch,
      process={pid=identity.pid,creation_filetime=identity.creation_filetime},site=name,
      rva=hex(loc.sites[name].rva),address=hex(base+loc.sites[name].rva),thread_id=regs.THREADID,
      tick_ms=api.tick(),globals=globals()}
    state.sequence=state.sequence+1;e.sequence=state.sequence
    if name:sub(-6)=='_entry' then e.entry_return_rva=rva(ptr(regs.RSP)) end
    -- This is a bounded scan for code-looking stack words, not an unwind/backtrace.
    e.stack_code_candidates={}
    for i=0,31 do local v=ptr(regs.RSP+8*i);local rv=rva(v)
      if rv then e.stack_code_candidates[#e.stack_code_candidates+1]={stack_offset=8*i,rva=rv} end
    end
    if name=='request_entry' then
      e.context=context(regs.RCX);e.operation=u32(regs.RDX);e.slot=u32(regs.R8);e.system=byte(regs.R9)
      e.busy_at_entry=field(regs.RCX,0x38,1)
      local key=tostring(regs.THREADID)..':'..hex(regs.RSP)
      assert(not requests[key],'duplicate outstanding request frame')
      assert(nrequests<32,'unpaired request limit')
      requests[key]={context=regs.RCX,sequence=e.sequence,busy=e.busy_at_entry};nrequests=nrequests+1
    elseif name=='request_exit' then
      local key=tostring(regs.THREADID)..':'..hex(regs.RSP+0x28);local req=requests[key]
      if req then e.entry_sequence=req.sequence;e.busy_at_entry=req.busy;e.context=context(req.context)
        if req.busy==nil then e.disposition='unknown_busy_field_unreadable'
        elseif req.busy~=0 then e.disposition='busy_rejected'
        else e.disposition='submitted_attempt_not_native_acceptance_proof' end
        requests[key]=nil;nrequests=nrequests-1
      else e.unpaired=true end
    elseif name=='task_bind_entry' then
      local a=regs.R8;e.task_address=hex(regs.RCX)
      e.request={operation=field(a,0,4),format=field(a,4,4),payload=pfield(a,8),bytes=field(a,0x10,8),
        metadata=pfield(a,0x18),slot=field(a,0x20,4),system=field(a,0x24,1)}
      e.account_ref=token(accounts,hex(regs.RDX),'account')
    elseif name=='completion_consume' then
      e.context=context(regs.RBX);e.meaning='poll_result_before_busy_clear_not_product_commit_ack'
    elseif name=='snapshot_entry' then e.destination=hex(regs.RCX)
    elseif name=='snapshot_ready' then
      e.snapshot=hex(regs.RSI);e.bytes=0x900028;e.fingerprint=digest(regs.RSI+8,0x900000)
      e.checksum=field(regs.RSI,0x90000c,4);e.checksum_not_generation=true
    elseif name=='queue_capture_entry' then
      e.root_argument=hex(regs.RCX);e.operation=u32(regs.RDX);e.slot=u32(regs.R8);e.system=byte(regs.R9)
    elseif name=='queue_pop_entry' then e.root_argument=hex(regs.RCX)
    elseif name=='snapshot_copy_entry' then e.destination=hex(regs.RCX);e.source=hex(regs.RDX)
    elseif name=='coordinator_entry' or name=='worker_dispatch' then e.task=task(regs.RCX)
    elseif name=='coordinator_exit' then e.task=task(regs.RDI);e.return_al=byte(regs.RAX)
    elseif name=='serializer_entry' then
      e.task=task(regs.RCX);e.output=hex(regs.RDX)
      local a=field(regs.RCX,0xe0,8);local n=field(regs.RCX,0xe8,8)
      if a and n==0x900058 then e.fingerprint=digest(a+0x38,0x900000) else e.fingerprint={status='unsupported_payload_shape'} end
    elseif name=='serializer_copyback' then
      e.destination=hex(regs.RCX);e.source=hex(regs.RDX);e.bytes=regs.R8
      e.meaning='transform_copied_back_to_staging_not_inventory_owner'
    elseif name=='serializer_exit' then e.task=task(regs.RSI);e.output=hex(regs.RDI);e.error_code=u32(regs.RBX)
    elseif name=='writer_entry' then
      e.directory=pathinfo(regs.RCX);e.filename=pathinfo(regs.RDX);e.buffer=hex(regs.R8);e.bytes=regs.R9
      e.error_pointer=hex(ptr(regs.RSP+0x28))
      local key=tostring(regs.THREADID)..':'..hex(regs.RSP)
      assert(not writers[key] and nwriters<32,'outstanding writer frame limit/collision')
      writers[key]={sequence=e.sequence,bytes=regs.R9,origin='entry'};nwriters=nwriters+1
    elseif name:sub(1,7)=='writer_' then
      e.directory=pathinfo(regs.R14);e.filename=pathinfo(regs.R13)
      e.buffer=hex(regs.R15);e.error=field(regs.RBX,0,4)
      -- Eight pushes (0x40) + sub rsp,0x898. After close, success code reuses
      -- RDI=-1 and ESI=0x104 for path formatting; neither remains size/handle.
      local key=tostring(regs.THREADID)..':'..hex(regs.RSP+0x8d8)
      local w=writers[key]
      if not w and name=='writer_open_result' then
        assert(nwriters<32,'outstanding writer frame limit')
        w={sequence=e.sequence,bytes=regs.RDI,origin='open_result'}
        writers[key]=w;nwriters=nwriters+1
      end
      if w then e.writer_start_sequence=w.sequence;e.writer_start_origin=w.origin;e.bytes=w.bytes
      else e.unpaired=true end
      if name=='writer_open_result' then
        e.handle=hex(regs.RAX);w.handle=e.handle
      elseif name=='writer_exit' then
        e.return_al=byte(regs.RAX)
        if w then writers[key]=nil;nwriters=nwriters-1 end
      elseif name=='writer_rename_result' then
        e.return_u32=u32(regs.RAX)
        e.previously_observed_handle=w and w.handle or nil
        e.handle_state='close_call_already_executed'
      else
        e.return_u32=u32(regs.RAX);e.handle=hex(regs.RSI)
        if w then w.handle=e.handle else e.bytes=regs.RDI end
      end
      if name=='writer_write_result' then e.bytes_written=field(regs.RSP,0x30,4) end
    elseif name=='read_entry' then e.task=task(regs.RCX)
    elseif name=='read_exit' then e.task=task(regs.RSI);e.return_al=byte(regs.RAX)
    elseif name=='apply_entry' then e.payload=hex(regs.RCX);e.bytes=0x900028;e.fingerprint=digest(regs.RCX+8,0x900000)
    elseif name=='apply_exit' then e.return_al=byte(regs.RAX)
    elseif name=='registry_dispatch' then
      local vt=field(regs.RBX,0,8);local cb=field(vt,8,8)
      e.registration_node=hex(regs.RBX);e.vtable=hex(vt);e.callback={address=hex(cb),rva=rva(cb)}
      e.stream=hex(regs.RDI);e.stream_mode28=field(regs.RDI,0x28,1)
      e.stream_begin=pfield(regs.RDI,0);e.stream_end=pfield(regs.RDI,8)
    end
    emit(e)
  end
  local probe={}
  function probe.status()
    for i,v in ipairs(state.cleanup_errors or {}) do state.cleanup_errors[i]=tostring(v):sub(1,512) end
    return state
  end
  function probe.stop() halt('operator_stop');return state end
  function probe.retry_cleanup()
    if owner then owner.retry_cleanup() end
    if not state.cleanup_pending and #state.owned_breakpoints==0 and api.unwatch then api.unwatch() end
    return state
  end
  function probe.clear_captures()
    assert(not state.active,'Stop before clear_captures')
    assert(not state.cleanup_pending and #(state.owned_breakpoints or {})==0,'Cleanup not confirmed')
    state.events={};state.epoch=state.epoch+1;return state
  end
  function probe.start()
    assert(not owner,'Create a new observer instance/run ID to re-arm')
    local before_pid=api.pid();base=api.base();attachment=api.attachment()
    identity=api.identity()
    assert(api.pid()==before_pid and api.base()==base and api.attachment()==attachment,'Attachment changed during executable attestation')
    assert(identity and identity.pid==api.pid() and identity.pid>0,'Process identity mismatch')
    assert(type(identity.creation_filetime)=='string' and identity.creation_filetime:match('^%d+$'),'Creation FILETIME missing')
    assert(identity.sha256==loc.disk_sha256 and identity.size==loc.disk_size and identity.version=='2.0.1.0','Unsupported executable identity')
    assert(base and base>0x10000,'Module not attached')
    state.process=identity;state.module_base=hex(base)
    local inventory=api.list();assert(type(inventory)=='table','Breakpoint inventory unavailable')
    local existing={};local count=0
    for _,a in pairs(inventory) do assert(type(a)=='number','Unsupported breakpoint inventory');existing[a]=true;count=count+1 end
    -- A profile uses all four hardware registers. Never evict unrelated breakpoints.
    assert(count==0,'This four-site profile needs an empty breakpoint inventory; no breakpoint was removed')
    for name,site in pairs(loc.sites) do assert(signature(site),'Signature mismatch: '..name) end
    assert(api.windows_debugger(),'Select the Windows debugger (no VEH injection) before arming')
    local function bound(fn)
      return function(...)
        assert(api.pid()==identity.pid and api.attachment()==attachment,'Attachment replaced; do not touch new-process breakpoints')
        return fn(...)
      end
    end
    owner=ownerFactory({list=bound(api.list),arm=bound(api.arm),remove=bound(api.remove),
      timer=function(ms,fn)
        api.timer(ms,function()
          fn()
          if not state.cleanup_pending and #state.owned_breakpoints==0 and api.unwatch then api.unwatch() end
        end)
      end},state)
    if api.watch then api.watch(function() halt('attachment_replaced') end) end
    state.active=true
    local ok,message=pcall(function()
      for _,name in ipairs(PROFILES[cfg.profile]) do
        local a=base+loc.sites[name].rva
        local armed,why=owner.arm(a,function()
          local good,problem=pcall(function() if state.active then capture(name) end end)
          if not good then err(problem);halt('callback_failure') end
          local continued,ceerr=pcall(api.continue)
          if not continued then err('Continue failed: '..tostring(ceerr));state.continue_failed=true;halt('continue_failed') end
          return 1
        end)
        assert(armed,why or 'Arm failed')
      end
    end)
    if not ok then err(message);halt('arming_failed');return false,state end
    return true,state
  end
  return probe
end

-- A local PowerShell identity query never executes inside the game. The target
-- disk path is not exported. Hashes are checked BEFORE any breakpoint is armed.
local function b64utf16(s)
  local alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
  local b={};for i=1,#s do b[#b+1]=s:byte(i);b[#b+1]=0 end
  local out={}
  for i=1,#b,3 do local a,c,d=b[i],b[i+1],b[i+2];local x=a*65536+(c or 0)*256+(d or 0)
    out[#out+1]=alphabet:sub((x>>18&63)+1,(x>>18&63)+1)
    out[#out+1]=alphabet:sub((x>>12&63)+1,(x>>12&63)+1)
    out[#out+1]=c and alphabet:sub((x>>6&63)+1,(x>>6&63)+1) or '='
    out[#out+1]=d and alphabet:sub((x&63)+1,(x&63)+1) or '='
  end
  return table.concat(out)
end
local function realApi()
  for _,n in ipairs({'getOpenedProcessID','getOpenedProcessHandle','getAddressSafe','readBytes','debug_getBreakpointList',
    'debug_setBreakpoint','debug_removeBreakpoint','debug_continueFromBreakpoint','debug_getCurrentDebuggerInterface','getTickCount',
    'synchronize','inMainThread'}) do
    assert(type(_G[n])=='function','Missing CE API: '..n)
  end
  assert(type(bpmDebugRegister)=='number' and type(bptExecute)=='number','Hardware constants missing')
  local opened_epoch=0
  local saved_handler,installed_handler
  local function onMainThread(fn)
    if inMainThread() then return fn() end
    local outcome,completed
    synchronize(function() outcome=table.pack(pcall(fn));completed=true end)
    assert(completed and outcome,'CE main-thread synchronization did not complete')
    assert(outcome[1],outcome[2])
    return table.unpack(outcome,2,outcome.n)
  end
  return {
    attachment=function() return tostring(getOpenedProcessHandle())..':'..opened_epoch end,
    watch=function(onlost)
      onMainThread(function()
        assert(MainForm,'CE MainForm required for attachment-change tracking')
        saved_handler=MainForm.OnProcessOpened
        installed_handler=function(...)
          opened_epoch=opened_epoch+1
          onlost()
          if saved_handler then saved_handler(...) end
        end
        MainForm.OnProcessOpened=installed_handler
      end)
    end,
    unwatch=function()
      onMainThread(function()
        if installed_handler and MainForm.OnProcessOpened==installed_handler then MainForm.OnProcessOpened=saved_handler end
      end)
    end,
    pid=getOpenedProcessID,base=function() return getAddressSafe('Nioh3.exe') end,
    read=function(a,n) return readBytes(a,n,true) end,
    windows_debugger=function() return debug_getCurrentDebuggerInterface()==1 end,
    list=debug_getBreakpointList,
    arm=function(a,cb)
      debug_setBreakpoint(a,1,bptExecute,bpmDebugRegister,cb) -- standard CE can return nil on success
      for _,x in pairs(debug_getBreakpointList()) do if x==a then return true end end
      return false
    end,
    remove=function(a) debug_removeBreakpoint(a) end,
    timer=function(ms,fn)
      return onMainThread(function()
        local t=createTimer(nil,false);t.Interval=ms
        t.OnTimer=function(sender) sender.Enabled=false;sender.destroy();fn() end
        t.Enabled=true
        return t
      end)
    end,
    continue=function() debug_continueFromBreakpoint(co_run) end,
    tick=getTickCount,md5=function(a,n) return md5memory(a,n) end,
    context=function() return {THREADID=THREADID,RSP=RSP,RAX=RAX,RBX=RBX,RCX=RCX,RDX=RDX,
      RSI=RSI,RDI=RDI,R8=R8,R9=R9,R13=R13,R14=R14,R15=R15} end,
    emit=function(_) return true end,
    identity=function()
      local pid=getOpenedProcessID();assert(type(pid)=='number' and pid>0 and pid%1==0,'Attach Nioh3.exe first')
      local s="$ErrorActionPreference='Stop';$p=[Diagnostics.Process]::GetProcessById("..pid..");"..
        "$t=$p.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();$f=Get-Item -LiteralPath $p.MainModule.FileName;"..
        "$sha=[Security.Cryptography.SHA256]::Create();$stream=$null;try{$stream=[IO.File]::OpenRead($f.FullName);"..
        "$bytes=$sha.ComputeHash($stream)}finally{if($stream){$stream.Dispose()};$sha.Dispose()};"..
        "$h=([BitConverter]::ToString($bytes)).Replace('-','');"..
        "$v=$f.VersionInfo;[Console]::Write(('{0}|{1}|{2}|{3}|{4}.{5}.{6}.{7}' -f "..
        "$p.Id,$t,$f.Length,$h,$v.FileMajorPart,$v.FileMinorPart,$v.FileBuildPart,$v.FilePrivatePart))"
      local pipe=assert(io.popen('powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand '..b64utf16(s)..' 2>NUL','r'))
      local text=pipe:read(2048) or '';pipe:close();assert(#text<2048,'Identity output limit')
      local id,time,size,hash,ver=text:match('^(%d+)|(%d+)|(%d+)|(%x+)|([%d%.]+)%s*$')
      assert(id and hash,'Could not attest running executable; no breakpoints armed')
      return {pid=tonumber(id),creation_filetime=time,size=tonumber(size),sha256=hash:upper(),version=ver}
    end,
  }
end
function M.start(cfg)
  assert(type(cfg)=='table' and type(cfg.project_root)=='string','project_root required')
  local root=cfg.project_root:gsub('[/\\]$','')
  local factory=assert(loadfile(root..'/research/owned_breakpoint_lifecycle_ce.lua'))()
  local loc=assert(loadfile(root..'/research/title_save_v201/locators.lua'))()
  local api=realApi()
  -- The observer uses an existing CE attachment. It never calls openProcess,
  -- detachIfPossible, injects VEH, or clears another script's debugger callback.
  if M.current then local s=M.current.status();assert(not s.active and not s.cleanup_pending and #s.owned_breakpoints==0,'Previous observer still owns breakpoints') end
  local p=M.new(api,cfg,factory,loc);M.current=p
  local ok,status=p.start();if not ok then return p,false,status end
  print('TitleSaveOwnership armed '..cfg.profile..' (research only; release BLOCK)')
  return p,true,status
end
function M.export(path)
  assert(M.current,'No observer');local s=M.current.status()
  assert(not s.active,'Stop before exporting a final capture')
  assert(type(path)=='string' and #path<1024 and path:lower():match('%.jsonl$'),'Use a new local .jsonl file')
  local old=io.open(path,'rb');if old then old:close();error('Refusing to overwrite evidence') end
  local f=assert(io.open(path,'wb'));local ok,why=pcall(function()
    assert(f:write(json({kind='header',schema=s.schema,run_id=s.run_id,profile=s.profile,process=s.process,
      account_key=s.account_key,slot=s.slot,account_binding=s.account_binding,module_base=s.module_base})..'\n'))
    for _,e in ipairs(s.events) do assert(f:write(json(e)..'\n')) end
    assert(f:write(json({kind='status',sequence=s.sequence,epoch=s.epoch,dropped=s.dropped,errors=s.errors,
      cleanup_pending=s.cleanup_pending,cleanup_errors=s.cleanup_errors or {},owned_breakpoints=s.owned_breakpoints,active=s.active,
      stop_reason=s.stop_reason,continue_failed=s.continue_failed or false,live_acceptance=false,release='BLOCK'})..'\n'))
    assert(f:flush())
  end);f:close();assert(ok,why);return #s.events
end
TitleSaveOwnership=M
return M
