-- CE MCP Lua chunks may run off the GUI thread. Create timers on the GUI thread.
return function(delay,callback)
  assert(type(delay)=='number' and delay>0 and type(callback)=='function')
  local timer
  synchronize(function()
    timer=createTimer(nil,false)
    timer.Interval=delay
    timer.OnTimer=function(current)
      current.Enabled=false
      current.destroy()
      callback()
    end
    timer.Enabled=true
  end)
  return timer
end
