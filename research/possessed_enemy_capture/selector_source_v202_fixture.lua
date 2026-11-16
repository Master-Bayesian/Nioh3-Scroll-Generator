-- Owned in-process E2E adapter only. This file never opens a game process.
-- Its artificial memory and simulated game steps cannot certify native parity.
local model,profile=assert(TEST_MODEL),assert(TEST_PROFILE)
local fault=TEST_FAULT or ''
local memory,callbacks,timers={},{},{}
local base,frame,waves,wrapper=0x140000000,0x700000,0x710000,0x720000
local thread,debugging,continued=77,false,0
local function zero(a,n) for i=0,n-1 do memory[a+i]=0 end end
local function put(a,v,n) for i=0,n-1 do memory[a+i]=(v>>(8*i))&255 end end
local function fromHex(a,h) for i=1,#h,2 do memory[a+(i-1)//2]=tonumber(h:sub(i,i+1),16) end end
local function copy(a,b,n) for i=0,n-1 do memory[b+i]=assert(memory[a+i]) end end
local function hex(v) return string.format('0x%X',v) end
function readBytes(a,n) local b={};for i=0,n-1 do if memory[a+i]==nil then return nil end;b[#b+1]=memory[a+i] end;return b end
function getAddressSafe() return base end
function getOpenedProcessID() return 123 end
function getOpenedProcessHandle() return 456 end
function getCurrentThreadId() return thread end
function debug_isBroken() return false end
function debug_isDebugging() return debugging end
function debugProcess() debugging=true;return true end
function debug_getBreakpointList() local a={};for x in pairs(callbacks) do a[#a+1]=x end;table.sort(a);return a end
function debug_setBreakpoint(a,_,_,_,fn)
  assert(#debug_getBreakpointList()<4);callbacks[a]=fn
  return not (fault=='partial-arm' and a==base+tonumber(profile.sites.task_copy.rva))
end
function debug_removeBreakpoint(a) if fault=='cleanup-failure' then return false end;callbacks[a]=nil;return true end
function debug_removeBreakpointByID(a) return debug_removeBreakpoint(a) end
function debug_continueFromBreakpoint() continued=continued+1 end
function createTimer(delay,fn)
  if fault=='budget-timer' and delay==120000 then error('Owned timer startup failure') end
  timers[#timers+1]={delay=delay,fn=fn};return {}
end
bptExecute,bpmDebugRegister,co_run=0,1,0
local function drain(maxDelay)
  for _=1,32 do
    local selected=nil
    for i,t in ipairs(timers) do if t.delay<=maxDelay then selected=i;break end end
    if not selected then return end
    local t=table.remove(timers,selected);t.fn()
  end
  error('Owned timer budget exceeded')
end
for _,group in ipairs({profile.sites,profile.admission_anchors}) do
  for _,site in pairs(group) do fromHex(base+tonumber(site.rva),site.hex) end
end
if fault=='signature' then memory[base+tonumber(profile.sites.origin_entry.rva)]=0 end
if fault=='ancestry-signature' then memory[base+tonumber(profile.admission_anchors.descriptor_selector.rva)]=0 end
if fault=='foreign-breakpoint' then callbacks[0xDEAD]=function() end end
NIOH3_POSSESSED_TARGET_IDENTITY={executable_sha256=profile.exe_sha256,image_size=profile.exe_size,
  process_id=123,module_base=hex(base),creation_filetime='123456789'}
if fault=='executable' then NIOH3_POSSESSED_TARGET_IDENTITY.executable_sha256=string.rep('0',64) end
NIOH3_ASSIGNMENT_SEED=model.seed
NIOH3_SELECTOR_SOURCE_PROFILE=profile
NIOH3_POSSESSED_RUN_ID='owned-'..model.seed..'-'..fault
zero(frame-0x1000,0x4000)
put(frame+0x1608,model.seed,4);put(frame+0x1640,model.auxiliary_mode,1)
put(frame+0x1650,fault=='descriptor-selector' and 0 or 1,1)
for i,v in ipairs(model.descriptor_flags) do put(frame+0x1658+(i-1)*8,v and 1 or 0,1) end
for i,offset in ipairs({0x1618,0x16C8,0x1778}) do put(frame+offset,base+tonumber(profile.parent_returns[i]),8) end
local entryState=model.source_entry_state
put(frame+0xC0,entryState,4)
local request=base+profile.globals.normal_request
zero(request,12);put(request,model.seed,4);put(request+7,3,1)
if fault=='request-playthrough' then put(request+7,2,1) end
zero(waves,0x28);zero(wrapper,8);put(wrapper,waves,8)
local waveBase,descriptorBase=0x730000,0x740000
local countWaves=0;for _,d in ipairs(model.occurrences) do countWaves=math.max(countWaves,d.wave_index+1) end
zero(waveBase,countWaves*0x28);put(waves,waveBase,8);put(waves+8,waveBase+countWaves*0x28,8)
put(waves+0x1F,model.terrain,1)
local descriptors,counts={},{}
for _,d in ipairs(model.occurrences) do
  local a=descriptorBase+d.wave_index*0x1000+d.position*0x14
  zero(a,20);put(a,d.spawn_key,4);put(a+4,d.lookup_key,4);put(a+8,d.role,1)
  put(a+0x10,d.occurrence_selector_class,1);descriptors[d.spawn_key]=a
  counts[d.wave_index]=(counts[d.wave_index] or 0)+1
end
for w=0,countWaves-1 do
  local a=descriptorBase+w*0x1000;put(waveBase+w*0x28,a,8)
  put(waveBase+w*0x28+8,a+(counts[w] or 0)*0x14,8)
end
if fault=='preassigned-flag' then put(descriptors[model.occurrences[1].spawn_key]+15,1,1) end
local parameterManager,rngManager=0x800000,0x810000
zero(parameterManager,0xC00);zero(rngManager,0x600)
put(base+profile.globals.parameter_manager,parameterManager,8)
put(parameterManager+0xB0A,1,1)
put(base+profile.globals.rng_manager,rngManager,8);put(rngManager+0x528,thread,4)
put(base+profile.globals.rng_secondary_owner,0,4)
put(base+profile.globals.rng_primary,0x820000,8);put(0x820000,123,4)
put(base+profile.globals.rng_secondary,0,8);put(base+profile.globals.rng_fallback,123,4)
local nextAllocation=0x900000
local function tableRows(keys,stride,fill)
  local ctx=nextAllocation;nextAllocation=nextAllocation+0x20000
  local store,hash,array=ctx+0x100,ctx+0x10000,ctx+0x10100
  zero(ctx,0x40);zero(store,8+#keys*stride);zero(hash,0x20);zero(array,#keys*8)
  put(ctx,store,8);put(store+4,#keys,4);put(ctx+0x20,hash,8)
  put(hash+4,0xFFFFFFFF,4);put(hash+8,array,8);put(hash+0x10,array+#keys*8,8)
  for i,key in ipairs(keys) do put(array+(i-1)*8,key,4);put(array+(i-1)*8+4,i-1,4);fill(store+8+(i-1)*stride,key,i) end
  return ctx
end
local keys,byLookup={},{}
for _,d in ipairs(model.occurrences) do if not byLookup[d.lookup_key] then keys[#keys+1]=d.lookup_key;byLookup[d.lookup_key]=d end end
table.sort(keys)
local enemy=tableRows(keys,0x398,function(a,_,i) put(a+0xA8,i,2) end)
local subtypeKeys={};for i=1,#keys do subtypeKeys[i]=i end
local subtype=tableRows(subtypeKeys,0x54,function(a,key) put(a+0x14,byLookup[keys[key]].source_eligible and 0 or 1,1) end)
local config=tableRows({0x4543},0x20,function(a) fromHex(a,TEST_CONFIG_HEX) end)
local special=tableRows({model.auxiliary_mode},0x30,function(a) put(a+0x29,1,1) end)
put(parameterManager+0x40,enemy,8);put(parameterManager+0x118,subtype,8)
put(parameterManager+0x230,config,8);put(parameterManager+0xA98,special,8)
local root,manager,owner,vector=0xB00000,0xB10000,0xB20000,0xB30000
zero(root,0x60);zero(manager,0x60);zero(owner,0x60);zero(vector,#model.occurrences*16)
put(base+profile.globals.task_manager,root,8);put(root,owner,8);put(owner+0x48,manager,8)
put(manager,vector,8);put(manager+8,#model.occurrences,4)
local function hit(site)
  RIP=base+tonumber(profile.sites[site].rva)
  local fn=assert(callbacks[RIP],'Owned breakpoint unavailable');fn()
end
local ok,err=pcall(function() assert(loadfile(TEST_COLLECTOR_PATH))() end)
if ok and fault=='timeout' then
  drain(120000)
elseif ok then
  RBP=frame;RSP=frame-0x1600;RCX=wrapper;RDX=0
  put(RSP,base+tonumber(profile.source_returns[1]),8)
  hit('origin_entry')
  if nioh3PossessedCapture.active then
    for _,trial in ipairs(model.source_trials) do
      RSI=descriptors[trial.spawn_key];R15=trial.occurrence_selector_class
      RSP=frame-0x1600-0x78;RAX=trial.ticket;RDI=TEST_THRESHOLD
      put(frame+0xC0,trial.state,4)
      if fault=='trial-thread' then thread=78 end
      hit('trial_decision');thread=77
      if trial.accepted then put(RSI+15,1,1) end
    end
    for i,d in ipairs(model.occurrences) do
      if not nioh3PossessedCapture.active then break end
      local source=descriptors[d.spawn_key];local temporary=0xC00000+i*0x1000;local persistent=0xD00000+i*0x1000
      zero(temporary,0x100);zero(persistent,0x100)
      put(temporary+0x20,d.spawn_key,4);put(temporary+0x24,0xCC96,4)
      RSP=0x680000;put(RSP+0x28,base+tonumber(profile.constructor_return),8)
      RDI=temporary;RBX=source;hit('task_copy')
      if not nioh3PossessedCapture.active then break end
      copy(source,temporary+0x80,20);copy(temporary,persistent,0x100)
      put(persistent+0x28,d.lookup_key,4);put(vector+(i-1)*16+8,persistent,8)
      RDI=source;R14=persistent;hit('task_linked')
    end
  end
end
drain(1000)
local function encode(v)
  local kind=type(v)
  if kind=='nil' then return 'null' elseif kind=='boolean' or kind=='number' then return tostring(v)
  elseif kind=='string' then return '"'..v:gsub('\\','\\\\'):gsub('"','\\"'):gsub('\n','\\n'):gsub('\r','\\r')..'"'
  elseif kind=='table' then
    local array=true;for k in pairs(v) do if type(k)~='number' then array=false;break end end
    local values={}
    if array then for _,x in ipairs(v) do values[#values+1]=encode(x) end;return '['..table.concat(values,',')..']' end
    for k,x in pairs(v) do if type(x)~='function' then values[#values+1]=encode(tostring(k))..':'..encode(x) end end
    table.sort(values);return '{'..table.concat(values,',')..'}'
  end
  error('Unsupported owned output type: '..kind)
end
return encode({bootstrap_ok=ok,bootstrap_error=not ok and tostring(err) or nil,
  probe=nioh3PossessedCapture,breakpoints=debug_getBreakpointList(),continued=continued})
