-- Research-only bounded ownership helper. CE functions are injected explicitly.
return function(api, probe)
  local owned, timerScheduled, checksRemaining = {}, false, 0
  local pass
  probe.cleanup_pending, probe.cleanup_errors, probe.owned_breakpoints = false, {}, {}
  local function hex(value) return string.format('0x%X',value) end
  local function failure(message)
    if #probe.cleanup_errors<32 then probe.cleanup_errors[#probe.cleanup_errors+1]=tostring(message) end
  end
  local function publish()
    local addresses, pending = {}, false
    for address, site in pairs(owned) do
      addresses[#addresses+1]=hex(address)
      if site.remove_requested then pending=true end
    end
    table.sort(addresses)
    probe.owned_breakpoints, probe.cleanup_pending = addresses, pending
  end
  local function reconcile()
    local ok, addresses=pcall(api.list)
    if not ok or type(addresses)~='table' then
      failure('Breakpoint inventory unavailable: '..tostring(addresses)); return
    end
    local present={}
    for _, address in pairs(addresses) do present[address]=true end
    for address, site in pairs(owned) do
      if site.remove_requested and not present[address] then owned[address]=nil end
    end
  end
  local function schedule()
    if timerScheduled or not probe.cleanup_pending or checksRemaining<=0 then return end
    timerScheduled=true
    local ok,result=pcall(api.timer,100,function()
      timerScheduled=false
      checksRemaining=checksRemaining-1
      pass()
    end)
    if not ok then
      timerScheduled,checksRemaining=false,0
      failure('Cleanup timer unavailable: '..tostring(result))
    end
  end
  pass=function()
    reconcile()
    for address,site in pairs(owned) do
      if site.remove_requested then
        local ok,result
        if site.id~=nil and type(api.remove_id)=='function' then
          ok,result=pcall(function() return api.remove_id(site.id) end)
        else
          ok,result=pcall(function() return api.remove(address) end)
        end
        if not ok or result==false then failure('Removal unconfirmed for '..hex(address)..': '..tostring(result)) end
      end
    end
    reconcile()
    publish()
    schedule()
  end
  local owner={}
  function owner.arm(address,callback)
    assert(not owned[address],'The observer already owns this breakpoint address')
    owned[address]={remove_requested=false}
    local ok,armed,id=pcall(api.arm,address,callback)
    -- Bridge builds can return an opaque table as their second result. Keep
    -- numeric IDs for the native API; otherwise use the owned address.
    if type(id)=='number' then owned[address].id=id end
    publish()
    if not ok or not armed then
      owned[address].remove_requested=true
      checksRemaining=3
      pass()
      return false,tostring(armed)
    end
    return true
  end
  function owner.contains(address) return address~=nil and owned[address]~=nil end
  function owner.remove(address)
    if owned[address] then owned[address].remove_requested=true end
    checksRemaining=math.max(checksRemaining,3)
    pass()
  end
  function owner.retry_cleanup()
    checksRemaining=3
    pass()
    return {cleanup_pending=probe.cleanup_pending,owned_breakpoints=probe.owned_breakpoints}
  end
  function owner.stop()
    for _,site in pairs(owned) do site.remove_requested=true end
    return owner.retry_cleanup()
  end
  return owner
end
