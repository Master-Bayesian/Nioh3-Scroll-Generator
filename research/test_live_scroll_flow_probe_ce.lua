-- Pure closed mocks: no fallback to real CE functions, no real game access.
local root=NIOH3_RESEARCH_TEST_ROOT or 'F:/Nioh3_ScrollEditor/research/'
local BASE,BUILD,INSERT,GLOBAL=0x10000000,0x1227C4CC,0x1054D294,0x1474D4E0
local MANAGER,DATA,DESC,BOUT,ISOURCE,IOUT,SLOT=0x24000000,0x25000000,0x26000000,0x27000000,0x28000000,0x29000000,0x2A000000
local BSP,ISP,BRET,IRET,BRET2=0x30001000,0x30000800,0x13000000,0x13001000,0x13002000
local SERIAL='78563412ABCDEF01'
local signatures={
  [BUILD]={0x48,0x89,0x5C,0x24,0x08,0x48,0x89,0x6C,0x24,0x10,0x48,0x89,0x74,0x24,0x18,0x57,
    0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x83,0xEC,0x20,0x45,0x33,0xED,0xC7},
  [BASE+0x227C707]={0x45,0x38,0x6E,0x21,0x75,0x1A,0x48,0x8B,0x05,0xCC,0x0D,0x4D,0x02,0x48,0x8B,0x10,
    0x48,0x8B,0x4A,0x08,0x48,0x8D,0x41,0x01,0x48,0x89,0x42,0x08,0x48,0x89,0x4F,0x28},
  [INSERT]={0x40,0x55,0x53,0x56,0x57,0x41,0x54,0x41,0x55,0x41,0x56,0x41,0x57,0x48,0x8D,0xAC},
  [BASE+0x54DC33]={0x41,0x80,0xF9,0x12,0x0F,0x85,0xB8,0x00,0x00,0x00,0x48,0x81,0xC1,0x60,0x4A,0x22,
    0x00,0x41,0x83,0xCF,0xFF,0xE8,0x6F,0x53}}
local function zeros(size)
  local result={}
  for index=1,size do result[index]=0 end
  return result
end
local function scenario(options)
  options=options or {}
  local state={sites={},memory={},pointers={[GLOBAL]=MANAGER,[MANAGER]=DATA},timers={},
    removals={},arm_calls=0,nextId=1,peak_sites=0,mode='immediate',continues=0,failRead={},pid=123}
  state.memory[DATA]=zeros(16)
  state.memory[DATA][9]=50
  state.memory[DESC]=zeros(0xCC)
  state.memory[DESC][0x22+1]=3
  for _,address in ipairs({BOUT,ISOURCE,IOUT}) do
    state.memory[address]=zeros(0xE8)
    state.memory[address][1],state.memory[address][2]=0x04,0xE6
    for index=0,7 do state.memory[address][0x28+index+1]=tonumber(SERIAL:sub(index*2+1,index*2+2),16) end
  end
  state.memory[SLOT]={0xFF,0xFF,0xFF,0xFF}
  local env={assert=assert,error=error,next=next,ipairs=ipairs,pairs=pairs,pcall=pcall,type=type,
    tostring=tostring,tonumber=tonumber,math=math,string=string,table=table,loadfile=loadfile,
    bptExecute=0,bpmDebugRegister=1,co_run=0,NIOH3_SCROLL_FLOW_ACTION='mock mixed loot',
    NIOH3_BREAKPOINT_LIFECYCLE_PATH=root..'owned_breakpoint_lifecycle_ce.lua'}
  state.env=env
  env.getAddressSafe=function() return BASE end
  env.getOpenedProcessID=function() return state.pid end
  env.readQword=function(address) return state.pointers[address] end
  env.readBytes=function(address,count,asTable)
    assert(asTable==true,'Unexpected scalar mock read')
    local bytes=signatures[address]
    if bytes then return bytes end
    local source,start
    for candidate,data in pairs(state.memory) do
      if address>=candidate and address+count<=candidate+#data then source,start=data,address-candidate;break end
    end
    if not source then return nil end
    local result={}
    for index=1,(state.failRead[address] and count-1 or count) do result[index]=source[start+index] end
    return result
  end
  env.debug_isDebugging=function() return true end
  env.debug_getContext=function() end
  env.debug_continueFromBreakpoint=function() state.continues=state.continues+1 end
  env.debug_getBreakpointList=function()
    local result={}
    for address in pairs(state.sites) do result[#result+1]=address end
    return result
  end
  env.debug_setBreakpoint=function(address,size,trigger,method,callback)
    assert(size==1 and trigger==0 and method==1 and not state.sites[address])
    if address==INSERT and options.failSecondEntry=='before' then return false end
    local count=0
    for _ in pairs(state.sites) do count=count+1 end
    assert(count<4,'Mock caught a fifth hardware breakpoint')
    state.arm_calls=state.arm_calls+1
    state.sites[address]={id=state.nextId,callback=callback}
    state.nextId=state.nextId+1
    state.peak_sites=math.max(state.peak_sites,count+1)
    if address==INSERT and options.failSecondEntry=='after' then return false,state.nextId-1 end
    return true,state.nextId-1
  end
  local function remove(address)
    state.removals[#state.removals+1]=address
    if state.mode=='failure' then error('injected removal failure') end
    if state.mode=='deferred' then return end
    state.sites[address]=nil
  end
  env.debug_removeBreakpoint=remove
  env.debug_removeBreakpointByID=function(id)
    for address,site in pairs(state.sites) do if site.id==id then remove(address);return end end
  end
  env.createTimer=function(delay,callback)
    if options.failTimer and delay==180000 then error('injected budget timer failure') end
    state.timers[#state.timers+1]={delay=delay,callback=callback}
    return {}
  end
  function state.fire(delay)
    for index,timer in ipairs(state.timers) do
      if timer.delay==delay then table.remove(state.timers,index);timer.callback();return true end
    end
    return false
  end
  function state.enter(name,stack,caller)
    stack=stack or (name=='builder' and BSP or ISP)
    caller=caller or (name=='builder' and BRET or IRET)
    state.pointers[stack]=caller
    state.memory[stack+0x28]={0}
    env.RSP=stack
    if name=='builder' then env.RCX,env.RDX,env.R8,env.R9=BOUT,DESC,0,0
    else env.RCX,env.RDX,env.R8,env.R9=MANAGER,IOUT,ISOURCE,SLOT end
    state.sites[name=='builder' and BUILD or INSERT].callback()
  end
  function state.finish(name,stack,caller)
    stack=stack or (name=='builder' and BSP or ISP)
    caller=caller or (name=='builder' and BRET or IRET)
    env.RSP,env.RAX=stack+8,name=='builder' and BOUT or IOUT
    state.sites[caller].callback()
  end
  if options.foreignBefore then state.sites[0x14000000]={id=9999} end
  local chunk,failure=loadfile(root..'capture_live_scroll_flow_ce.lua','t',env)
  assert(chunk,failure)
  state.load_ok,state.result=pcall(chunk)
  state.probe=env.nioh3ScrollFlowProbe
  if not options.foreignBefore and not options.failSecondEntry then assert(state.load_ok,state.result) end
  return state
end
local passed={}
local function test(name,body)
  local ok,failure=pcall(body)
  if not ok then error(name..': '..tostring(failure)) end
  passed[#passed+1]=name
end
local function events(state,kind)
  local result={}
  for _,event in ipairs(state.probe.events) do if event.kind==kind then result[#result+1]=event end end
  return result
end

test('Different channels nest at four sites and retain rapid same-caller returns',function()
  local state=scenario()
  state.enter('builder')
  state.enter('insertion')
  assert(state.peak_sites==4 and state.arm_calls==4)
  state.finish('insertion')
  state.finish('builder')
  state.enter('insertion')
  state.finish('insertion')
  assert(state.arm_calls==4 and #state.removals==0,'Return sites were needlessly replaced')
  local returns=events(state,'return')
  assert(#returns==3 and returns[1].channel=='insertion' and returns[2].channel=='builder')
  assert(returns[1].output_slot_index==-1 and returns[1].output_serial_le_hex==SERIAL)
  assert(events(state,'entry')[2].source_serial_le_hex==SERIAL,'High serial bytes were lost')
  assert(state.probe.scroll_insertion_hits==2 and not state.probe.natural_acquisition_confirmed)
  local builderEntry=events(state,'entry')[1]
  assert(builderEntry.descriptor_byte_22==3 and builderEntry.playthrough_input==nil)
  state.probe.stop('complete')
  assert(not state.probe.cleanup_pending and next(state.sites)==nil)
end)

test('Same-channel nested sample is explicitly skipped and cannot steal outer return',function()
  local state=scenario()
  state.enter('builder')
  state.enter('builder',BSP-0x100)
  local entries=events(state,'entry')
  assert(entries[2].return_capture_skip_reason=='channel_call_pending')
  state.finish('builder',BSP-0x100)
  assert(#events(state,'return')==0)
  state.finish('builder')
  assert(events(state,'return')[1].entry_sequence==1)
  assert(entries[1].return_capture_status=='paired' and entries[2].return_capture_status=='skipped')
  state.probe.stop('complete')
end)

test('Shared return address collision is skipped without a fifth or duplicate site',function()
  local state=scenario()
  state.enter('builder')
  state.enter('insertion',ISP,BRET)
  assert(events(state,'entry')[2].return_capture_skip_reason=='return_address_owned_by_other_site')
  assert(state.peak_sites==3)
  state.finish('insertion',ISP,BRET)
  state.finish('builder')
  assert(#events(state,'return')==1)
  state.probe.stop('complete')
end)

test('Changed caller waits for deferred cleanup and then replaces one return site',function()
  local state=scenario()
  state.enter('builder');state.finish('builder')
  state.enter('insertion');state.finish('insertion')
  state.mode='deferred'
  state.enter('builder',BSP,BRET2)
  assert(events(state,'entry')[3].return_capture_skip_reason=='return_site_replacement_pending')
  state.enter('builder',BSP,BRET2)
  assert(events(state,'entry')[4].return_capture_skip_reason=='return_site_cleanup_pending')
  assert(state.peak_sites==4 and state.arm_calls==4 and state.probe.cleanup_pending)
  state.sites[BRET]=nil -- CE's later confirmed removal.
  assert(state.fire(100))
  state.mode='immediate'
  state.enter('builder',BSP,BRET2);state.finish('builder',BSP,BRET2)
  assert(state.arm_calls==5 and state.peak_sites==4 and not state.probe.cleanup_pending)
  state.probe.stop('complete')
end)

test('Next entry reconciles completed CE removal without timer or manual retry',function()
  local state=scenario()
  state.enter('builder');state.finish('builder')
  state.enter('insertion');state.finish('insertion')
  state.mode='deferred'
  state.enter('builder',BSP,BRET2)
  assert(events(state,'entry')[3].return_capture_skip_reason=='return_site_replacement_pending')
  assert(state.probe.cleanup_pending and #state.timers>0 and state.arm_calls==4)
  state.sites[BRET]=nil -- CE removal completes between calls; do not fire a timer.
  state.enter('builder',BSP,BRET2)
  state.finish('builder',BSP,BRET2)
  local entry=events(state,'entry')[4]
  assert(entry.return_capture_status=='paired' and not entry.return_capture_skipped)
  assert(not state.probe.cleanup_pending and state.arm_calls==5 and state.peak_sites==4)
  state.mode='immediate'
  state.probe.stop('complete')
end)

test('Shared stop records both unpaired calls and bounds failed cleanup',function()
  local state=scenario()
  state.enter('builder');state.enter('insertion')
  state.mode='failure'
  state.probe.stop('manual')
  assert(#events(state,'unpaired')==2 and state.probe.cleanup_pending and not state.probe.active)
  state.finish('insertion')
  assert(#events(state,'return')==0,'Late stopped return was published')
  for _=1,3 do assert(state.fire(100)) end
  assert(not state.fire(100) and state.probe.cleanup_pending)
  state.mode='immediate'
  state.probe.retry_cleanup()
  assert(not state.probe.cleanup_pending and next(state.sites)==nil)
end)

test('Required output failure stops both channels without publishing a partial pair',function()
  local state=scenario()
  state.enter('builder');state.enter('insertion')
  state.failRead[IOUT]=true
  state.finish('insertion')
  assert(#events(state,'return')==0 and #events(state,'capture_error')==1)
  assert(#events(state,'unpaired')==2 and not state.probe.active and next(state.sites)==nil)
end)

test('Inventory owner and process switches fail closed',function()
  local owner=scenario()
  owner.enter('builder')
  owner.pointers[MANAGER]=DATA+0x1000
  owner.finish('builder')
  assert(owner.probe.stop_reason=='return_capture_error' and #events(owner,'return')==0)
  local process=scenario()
  process.pid=124
  process.enter('insertion')
  assert(process.probe.stop_reason=='entry_capture_error' and #events(process,'entry')==0)
end)

test('Foreign breakpoints are rejected and never adopted for cleanup',function()
  local initial=scenario({foreignBefore=true})
  assert(not initial.load_ok and initial.arm_calls==0 and #initial.removals==0)
  local later=scenario()
  later.sites[BRET]={id=9999}
  later.enter('builder')
  assert(not later.probe.active and later.sites[BRET].id==9999)
  for _,address in ipairs(later.removals) do assert(address~=BRET) end
end)

test('Shared hit, time and timer-creation budgets clean owned sites',function()
  local hits=scenario()
  hits.probe.max_hits=2
  hits.enter('builder');hits.enter('insertion')
  assert(hits.probe.stop_reason=='hit_budget' and #events(hits,'unpaired')==2 and next(hits.sites)==nil)
  local timed=scenario()
  timed.enter('builder')
  assert(timed.fire(180000))
  assert(timed.probe.stop_reason=='time_budget' and #events(timed,'unpaired')==1 and next(timed.sites)==nil)
  local failed=scenario({failTimer=true})
  assert(failed.probe.stop_reason=='budget_timer_failed' and not failed.probe.active and next(failed.sites)==nil)
end)

test('Second entry arm failure cleans first entry and any partially armed site',function()
  for _,mode in ipairs({'before','after'}) do
    local state=scenario({failSecondEntry=mode})
    assert(not state.load_ok and state.probe.stop_reason=='entry_arm_failed')
    assert(not state.probe.active and not state.probe.cleanup_pending and next(state.sites)==nil)
  end
end)

test('Retained return without a pending sample stays unpaired and obeys activity cap',function()
  local state=scenario()
  state.enter('insertion');state.finish('insertion')
  local count=#events(state,'return')
  state.probe.max_unmatched_returns=1
  state.finish('insertion')
  assert(#events(state,'return')==count and state.probe.stop_reason=='unmatched_return_budget')
  assert(not state.probe.active and next(state.sites)==nil)
end)

return {schema='nioh3-scroll-flow-probe-mocked-tests/v1',ok=true,test_count=#passed,passed=passed,
  real_debugger_calls=0,real_game_memory_access=false,max_hardware_sites=4}
