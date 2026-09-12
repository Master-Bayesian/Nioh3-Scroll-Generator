-- Executed by Lua 5.4 through ctypes in pytest. This is not live CE evidence.
local M=assert(loadfile(ROOT..'/research/probe_title_save_ownership_ce.lua'))()
local owner=assert(loadfile(ROOT..'/research/owned_breakpoint_lifecycle_ce.lua'))()
local loc=assert(loadfile(ROOT..'/research/title_save_v201/locators.lua'))()
local base=0x140000000
function mock(profile, options)
  options=options or {}
  local x={memory={},bp={},removed={},ticks=0,continued=0,reads=0,maxread=0,
    pid=10,handle='handle1',logs={},timers={},arms=0}
  function x.write(a,value,n) for i=0,n-1 do x.memory[a+i]=(value>>(8*i))&255 end end
  function x.fill(a,n) for i=0,n-1 do x.memory[a+i]=0 end end
  for _,s in pairs(loc.sites) do
    for i=1,#s.bytes//2 do x.memory[base+s.signature_rva+i-1]=tonumber(s.bytes:sub(2*i-1,2*i),16) end
  end
  x.regs={THREADID=17,RSP=0x200000,RAX=0,RCX=0x300000,RDX=0,R8=0,R9=0,RBX=0x300000,
    RSI=0x300000,RDI=0x300000,R13=0,R14=0,R15=0}
  local api={
    pid=function() return x.pid end,base=function() return base end,attachment=function() return x.handle end,
    identity=function() return {pid=10,creation_filetime='134336000000000000',sha256=loc.disk_sha256,size=loc.disk_size,version='2.0.1.0'} end,
    read=function(a,n) x.reads=x.reads+1;x.maxread=math.max(n,x.maxread)
      if x.throwread then error('read failure') end
      local b={};for i=0,n-1 do if x.memory[a+i]==nil then return nil end;b[#b+1]=x.memory[a+i] end;return b end,
    list=function() if x.listfail then error('inventory failure') end;local a={};for b in pairs(x.bp) do a[#a+1]=b end;return a end,
    arm=function(a,cb) x.arms=x.arms+1;x.bp[a]=cb;if x.failarm==x.arms then error('arm failed after creating breakpoint') end;return true end,
    remove=function(a) x.removed[#x.removed+1]=a;if not x.refuseremoval then x.bp[a]=nil end end,
    timer=function(_,fn) x.timers[#x.timers+1]=fn end,
    windows_debugger=function() return not x.veh end,
    continue=function() x.continued=x.continued+1;if x.failcontinue then error('continue failure') end end,
    context=function() return x.regs end,tick=function() x.ticks=x.ticks+1;return x.ticks end,
    emit=function(s) x.logs[#x.logs+1]=s;if x.failsink then return false end;return true end,
    md5=function(a,n) x.md5_called=(x.md5_called or 0)+1;return string.rep('A',32) end,
    watch=function(cb) x.onlost=cb end,unwatch=function() x.unwatched=true end,
  }
  for k,v in pairs(options.api or {}) do api[k]=v end
  local cfg={run_id='C0.test-01',profile=profile or 'ownership',account_key='0123456789abcdef',slot=0,max_events=options.max_events or 64,hash_payloads=options.hash_payloads}
  x.api=api;x.cfg=cfg;x.probe=M.new(api,cfg,owner,loc)
  function x.hit(name) local callback=assert(x.bp[base+loc.sites[name].rva],name..' not armed');return callback() end
  function x.start() return x.probe.start() end
  function x.status() return x.probe.status() end
  function x.drain() local t=x.timers;x.timers={};for _,fn in ipairs(t) do fn() end end
  return x
end
function count(t) local n=0;for _ in pairs(t) do n=n+1 end;return n end
_G.M=M;_G.loc=loc;_G.base=base

-- Exercise the real CE adapter too: only the external API implementations are mocked.
function use_production_adapter(x)
  getOpenedProcessID=x.api.pid
  getOpenedProcessHandle=function() return x.handle end
  getAddressSafe=function(name) assert(name=='Nioh3.exe');return base end
  readBytes=function(a,n,as_table) assert(as_table);return x.api.read(a,n) end
  debug_getBreakpointList=x.api.list
  bptExecute=0;bpmDebugRegister=1;co_run=0
  debug_setBreakpoint=function(a,n,t,m,cb)
    assert(n==1 and t==bptExecute and m==bpmDebugRegister)
    x.api.arm(a,cb)
    -- Official CE binding returns no result on success. Inventory is the proof.
    return nil
  end
  debug_removeBreakpoint=x.api.remove
  debug_continueFromBreakpoint=function(m) assert(m==co_run);return x.api.continue() end
  debug_getCurrentDebuggerInterface=function() return x.veh and 2 or 1 end
  getTickCount=x.api.tick;md5memory=x.api.md5
  inMainThread=function() return false end
  synchronize=function(fn) x.synchronize_calls=(x.synchronize_calls or 0)+1;return fn() end
  MainForm={OnProcessOpened=function() x.previous_handler_calls=(x.previous_handler_calls or 0)+1 end}
  x.previous_handler=MainForm.OnProcessOpened
  createTimer=function(_,enabled)
    assert(not enabled)
    local t={destroy=function()end};x.production_timer=t;return t
  end
  io.popen=function(command,mode)
    assert(command:find('powershell.exe',1,true) and mode=='r')
    assert(not command:find('Secret',1,true))
    x.identity_command=command
    return {read=function(_,n)
      assert(n==2048)
      return tostring(x.pid)..'|134336000000000000|'..loc.disk_size..'|'..loc.disk_sha256..'|2.0.1.0'
    end,close=function() return true end}
  end
  for k,v in pairs(x.regs) do _G[k]=v end
  x.cfg.project_root=ROOT
  return M.start(x.cfg)
end
