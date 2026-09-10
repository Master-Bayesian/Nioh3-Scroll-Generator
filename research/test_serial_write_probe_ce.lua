-- Closed mocks: never attach, write, or place real breakpoints.
local source='F:/Nioh3_ScrollEditor/research/capture_serial_writes_ce.lua'
local helper='F:/Nioh3_ScrollEditor/research/owned_breakpoint_lifecycle_ce.lua'
local passed={}
local function scenario(name,run,settings)
  settings=settings or {}
  local sites,callbacks,timers={},{},{}
  local resumed,ownerChanged=0,false
  local env={}
  for _,key in ipairs({'assert','error','ipairs','pairs','pcall','next','type','tostring','tonumber'}) do env[key]=_G[key] end
  env.string=string;env.table=table;env.math=math;env.os={time=function() return 1 end}
  env.bptWrite=2;env.bpmDebugRegister=1;env.co_run=0
  env.RIP=0x200;env.RSP=0x1000
  env.getAddressSafe=function() return 0x100000 end
  env.getOpenedProcessID=function() return 42 end
  env.readQword=function(address)
    if address==0x100000+0x474D4E0 then return ownerChanged and 0x999 or 0x200000 end
    if address==0x200000 then return 0x300000 end
    if address==0x300000+0x224A60+0x16A80 then return 400 end
    error('Unexpected read')
  end
  env.readBytes=function() return {1,0,0,0,0,0,0,0} end
  env.debug_isDebugging=function() return true end
  env.debug_getBreakpointList=function() local out={};for address in pairs(sites) do out[#out+1]=address end;return out end
  env.debug_setBreakpoint=function(address,size,kind,method,callback)
    assert(address==0x300008 and size==8 and kind==2 and method==1)
    sites[address]=true;callbacks[#callbacks+1]=callback
    return true,settings.callbackAsId and callback or nil
  end
  env.debug_removeBreakpoint=function(address)
    if settings.defer then return false end
    sites[address]=nil;return true
  end
  env.debug_removeBreakpointByID=function() error('Unexpected numeric ID removal') end
  env.debug_getContext=function() if settings.contextError then error('Context unavailable') end end
  env.debug_continueFromBreakpoint=function() resumed=resumed+1 end
  env.createTimer=function(delay,callback)
    if settings.timerError and delay==30000 then error('Timer unavailable') end
    timers[#timers+1]=callback;return {}
  end
  env.dofile=function(path)
    if path=='F:/Nioh3_ScrollEditor/research/ce_main_thread_timer.lua' then return env.createTimer end
    assert(path==helper);return assert(loadfile(path,'t',env))()
  end
  local ok,err=pcall(assert(loadfile(source,'t',env)))
  run(env,callbacks,timers,sites,ok,err,function() return resumed end,function() ownerChanged=true end,settings)
  passed[#passed+1]=name
end
scenario('hit budget and late callback',function(e,c,t,s,ok,err,resumed)
  assert(ok,err);for i=1,16 do c[1]() end
  assert(not e.nioh3SerialWriteProbe.active and #e.nioh3SerialWriteProbe.events==16)
  c[1]();assert(#e.nioh3SerialWriteProbe.events==16 and resumed()==17 and next(s)==nil)
end)
scenario('owner change resumes and stops',function(e,c,t,s,ok,err,resumed,change)
  assert(ok,err);change();c[1]()
  assert(e.nioh3SerialWriteProbe.stop_reason=='capture_error' and resumed()==1 and next(s)==nil)
end)
scenario('context failure resumes and stops',function(e,c,t,s,ok,err,resumed)
  assert(ok,err);c[1]();assert(resumed()==1 and next(s)==nil and e.nioh3SerialWriteProbe.error)
end,{contextError=true})
scenario('timeout cleanup',function(e,c,t,s,ok,err)
  assert(ok,err);t[1]();assert(next(s)==nil and e.nioh3SerialWriteProbe.stop_reason=='time_budget')
end)
scenario('callback return is not a breakpoint ID',function(e,c,t,s,ok,err)
  assert(ok,err);t[1]();assert(next(s)==nil and not e.nioh3SerialWriteProbe.cleanup_pending)
end,{callbackAsId=true})
scenario('timer failure cleanup',function(e,c,t,s,ok)
  assert(not ok and next(s)==nil and not e.nioh3SerialWriteProbe.active)
end,{timerError=true})
scenario('deferred cleanup retains ownership',function(e,c,t,s,ok,err,resumed,change,settings)
  assert(ok,err);t[1]();assert(e.nioh3SerialWriteProbe.cleanup_pending and next(s))
  settings.defer=false;e.nioh3SerialWriteProbe.retry_cleanup()
  assert(not e.nioh3SerialWriteProbe.cleanup_pending and next(s)==nil)
end,{defer=true})
return {passed=#passed,cases=passed,closed_mock=true}
