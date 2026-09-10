-- Closed mocks validate the actual probe's state transitions and write scope.
local root='F:/Nioh3_ScrollEditor/research/'
local passed={}
local function scenario(name,run,options)
  options=options or {}
  local base,entry,allocation=0x100000,0x100000+0x12E6840,0x700000
  local memory,words,sites,timers={},{},{},{}
  local allocated,freed,resumes,sets=false,false,0,0
  local env={};env._G=env
  for _,key in ipairs({'assert','error','ipairs','pairs','pcall','next','type','tostring','tonumber'}) do env[key]=_G[key] end
  env.debug=debug;env.string=string;env.table=table;env.math=math;env.os={time=function() return 1 end}
  env.bptExecute=0;env.bpmDebugRegister=1;env.co_run=0
  env.RSP=0x900008;env.RIP=entry;env.EFLAGS=0x202
  env.nioh3DispatchProbeOptions={read_container_slot=options.readSlot==true}
  if options.preview or options.insertion then
    env.nioh3DispatchProbeOptions.assembly_preview={
      descriptor_hex=string.rep('00',16)..'01'..string.rep('00',16)..((options.allocatingPreview or options.insertion) and '00' or '01')..string.rep('00',170),
      expected_record_hex=string.rep('00',0xE8),builder_code_hex=string.rep('90',0x27B)}
  end
  if options.insertion then
    env.nioh3DispatchProbeOptions.single_insertion={pid=42,manager=0x400000,data=0x500000,serial=1,
      function_address=base+0x54D294,slot=0,operation_id='mock-single-insertion-0001',
      scheduler_owner=0x800000,container_hex=string.rep('00',400*0xE8),insertion_code_hex=string.rep('90',0xE17)}
    for i=0,400*0xE8-1 do memory[0x500000+0x224A60+i]=0 end
    words[0x500008]=1;words[base+0x47412F8]=0x800000
    if options.repeatedInsertion then env.nioh3InsertionAttempts={['mock-single-insertion-0001']=true} end
  end
  for i,name in ipairs({'RAX','RBX','RCX','RDX','RSI','RDI','RBP','R8','R9','R10','R11','R12','R13','R14','R15'}) do env[name]=0x300000+i*0x100 end
  words[base+0x474D4E0]=0x400000;words[0x400000]=0x500000
  words[0x500000+0x224A60+0x16A80]=400
  words[env.RCX+0x60]=0x600000;words[env.RCX+0x68]=options.busy and 0x600008 or 0x600000
  words[env.RSP]=options.wrongCaller and 1 or base+0x20BB2C
  env.getAddressSafe=function() return base end;env.getOpenedProcessID=function() return 42 end
  env.debug_isDebugging=function() return true end
  env.readQword=function(address) return words[address] end
  env.readInteger=function(address) if address==0x500000+0x224A60+0x18 then return 0x800000 end;return memory[address] or 0 end
  env.readSmallInteger=function(address) return address==0x500000+0x224A60 and 4 or 1 end
  env.readBytes=function(address,count)
    if address==entry then return {0x40,0x53,0x57,0x48,0x83,0xEC,0x38} end
    if address==base+0x227C4CC then local r={};for i=1,count do r[i]=0x90 end;return r end
    if address==base+0x54D294 then local r={};for i=1,count do r[i]=0x90 end;return r end
    if address==0x800000+0x1408 then return {options.busyTask and 1 or 0,0,0,0} end
    if address==0x800000+0x1629 then return {1} end
    if address==0x500008 then return {1,0,0,0,0,0,0,0} end
    if address==base+0x552FBC then return {0x8B,0xC2,0x48,0x3B,0x81,0x80,0x6A,0x01,0x00,0x72,0x03,0x33,0xC0,0xC3,0x48,0x69,0xC0,0xE8,0,0,0,0x48,0x03,0xC1,0xC3} end
    if address==0x500000+0x224A60 and not options.insertion then local out={};for i=1,count do out[i]=0 end;out[1]=4;return out end
    local result={};for i=0,count-1 do result[i+1]=memory[address+i] end;return result
  end
  env.allocateMemory=function(size) assert(size==4096);allocated=true;return allocation end
  env.deAlloc=function(address) assert(address==allocation and not freed,'Repeated allocation release');freed=true;return true end
  env.writeBytes=function(address,bytes)
    assert(address>=allocation and address+#bytes<=allocation+4096,'Write outside probe allocation')
    for i,v in ipairs(bytes) do memory[address+i-1]=v end
  end
  env.debug_getBreakpointList=function() local r={};for address in pairs(sites) do r[#r+1]=address end;return r end
  env.debug_setBreakpoint=function(address,size,kind,method,callback)
    if options.entryArmFailure and address==entry then return false end
    sites[address]=callback;return true
  end
  env.debug_removeBreakpoint=function(address) sites[address]=nil;return true end
  env.debug_getContext=function() end
  env.debug_setContext=function(extra) assert(not extra and env.RIP==allocation);sets=sets+1 end
  env.debug_continueFromBreakpoint=function() resumes=resumes+1 end
  env.dofile=function(path)
    if options.missingDependency then error('Missing dependency') end
    if path==root..'live_add_layout_ce.lua' then return dofile(path) end
    if path==root..'ce_main_thread_timer.lua' then return function(delay,callback) timers[#timers+1]=callback end end
    if path==root..'build_dispatch_probe_code.lua' then return assert(loadfile(path,'t',env))() end
    assert(path==root..'owned_breakpoint_lifecycle_ce.lua');return assert(loadfile(path,'t',env))()
  end
  if options.foreign then sites[0x999]=true end
  local ok,err=pcall(assert(loadfile(root..'probe_pickup_dispatch_noop_ce.lua','t',env)))
  local function enter() sites[entry]() end
  local function acknowledge(corrupt)
    local before=env.nioh3DispatchNoop.before
    env.RSP=before.RSP-0x48;env.RIP=entry+7;memory[allocation+0x300]=1;words[allocation+0x308]=0x500000+0x224A60
    words[env.RSP+0x38]=before.RDI;words[env.RSP+0x40]=before.RBX
    if options.preview or options.insertion then
      words[allocation+0x308]=allocation+0x600
      for i=0x28,0x2F do memory[allocation+0x600+i]=0xFF end
      if options.corruptCanary then memory[allocation+0x6E8]=0 end
    end
    if options.insertion then
      words[0x500008]=2
      for i=0,7 do memory[allocation+0x628+i]=(1>>(8*i))&0xFF end
      memory[allocation+0x318]=3;memory[allocation+0x320]=0
      words[allocation+0x328]=allocation+0x800
      local container=0x500000+0x224A60
      for i=0,0xE8-1 do memory[container+i]=memory[allocation+0x600+i] end
      words[container+0x28]=1
    end
    if corrupt then env.RAX=0 end
    sites[entry+7]()
  end
  run(env,ok,err,enter,acknowledge,timers,function() return allocated,freed,resumes,sets,next(sites)==nil end)
  passed[#passed+1]=name
end
scenario('one redirect, prologue acknowledgement and release',function(e,ok,err,enter,ack,t,state)
  assert(ok,err);enter();assert(e.nioh3DispatchNoop.phase=='redirected');ack()
  assert(e.nioh3DispatchNoop.phase=='completed');e.nioh3DispatchNoop.release()
  local allocated,freed,resumes,sets,empty=state();assert(allocated and freed and resumes==2 and sets==1 and empty)
end)
scenario('read-only slot lookup result',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack();assert(e.nioh3DispatchNoop.actual_result==0x500000+0x224A60)
  assert(e.nioh3DispatchNoop.phase=='completed');e.nioh3DispatchNoop.release()
end,{readSlot=true})
scenario('assembly preview matches output without serial allocation',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack();assert(e.nioh3DispatchNoop.preview_matches_natural_defined_fields)
  assert(e.nioh3DispatchNoop.release())
end,{preview=true})
scenario('assembly preview rejects serial allocation descriptor',function(e,ok,err,enter,ack,t,state)
  assert(not ok);local allocated=state();assert(not allocated)
end,{preview=true,allocatingPreview=true})
scenario('assembly output bounds reject corrupt canary',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack();assert(e.nioh3DispatchNoop.error and not pcall(e.nioh3DispatchNoop.release))
end,{preview=true,corruptCanary=true})
scenario('single insertion acknowledgement and preserved other slots',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack();assert(e.nioh3DispatchNoop.phase=='completed',e.nioh3DispatchNoop.error)
  assert(e.nioh3DispatchNoop.other_slots_unchanged);assert(e.nioh3DispatchNoop.release())
end,{insertion=true})
scenario('insertion rejects observed busy task phase before redirect',function(e,ok,err,enter,ack,t,state)
  assert(ok,err);enter();assert(e.nioh3DispatchNoop.redirect_count==0);assert(e.nioh3DispatchNoop.release())
end,{insertion=true,busyTask=true})
scenario('insertion operation identity cannot be repeated',function(e,ok,err,enter,ack,t,state)
  assert(not ok);local allocated=state();assert(not allocated)
end,{insertion=true,repeatedInsertion=true})
scenario('repeated release is idempotent',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack();assert(e.nioh3DispatchNoop.release());assert(e.nioh3DispatchNoop.release())
end)
scenario('process switch rejects release',function(e,ok,err,enter,ack,t,state)
  assert(ok,err);enter();ack();e.getOpenedProcessID=function() return 99 end
  assert(not pcall(e.nioh3DispatchNoop.release));local allocated,freed=state();assert(allocated and not freed)
end)
scenario('missing dependency rejects before allocation',function(e,ok,err,enter,ack,t,state)
  assert(not ok);local allocated=state();assert(not allocated)
end,{missingDependency=true})
scenario('busy pickup queue rejects without redirect',function(e,ok,err,enter,ack,t,state)
  assert(ok,err);enter();assert(e.nioh3DispatchNoop.redirect_count==0);e.nioh3DispatchNoop.release()
end,{busy=true})
scenario('unexpected caller rejects without redirect',function(e,ok,err,enter)
  assert(ok,err);enter();assert(e.nioh3DispatchNoop.redirect_count==0);e.nioh3DispatchNoop.release()
end,{wrongCaller=true})
scenario('timeout after redirect retains allocation',function(e,ok,err,enter,ack,t,state)
  assert(ok,err);enter();t[1]();assert(not pcall(e.nioh3DispatchNoop.release))
  local allocated,freed=state();assert(allocated and not freed)
end)
scenario('register mismatch does not claim completion',function(e,ok,err,enter,ack)
  assert(ok,err);enter();ack(true);assert(e.nioh3DispatchNoop.error and not pcall(e.nioh3DispatchNoop.release))
end)
scenario('partial arm failure removes acknowledgement',function(e,ok,err,enter,ack,t,state)
  assert(not ok);e.nioh3DispatchNoop.release();local a,f,r,s,empty=state();assert(a and f and empty and s==0)
end,{entryArmFailure=true})
scenario('foreign breakpoint rejects before allocation',function(e,ok,err,enter,ack,t,state)
  assert(not ok);local allocated=state();assert(not allocated)
end,{foreign=true})
return {schema='nioh3-dispatch-noop-mock/v1',passed=#passed,cases=passed,real_game_access=false}
