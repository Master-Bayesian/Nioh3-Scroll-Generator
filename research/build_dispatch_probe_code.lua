-- x64 research shim: bounded native calls, restore state, replay the prologue.
return function(memory,resume,original,leaf,argument,secondArgument,insertion)
  local code={}
  local function emit(bytes) for _,v in ipairs(bytes) do code[#code+1]=v end end
  local function u64(value) for i=0,7 do code[#code+1]=(value>>(8*i))&0xFF end end
  local function marker(offset,value)
    local d=offset-(#code+10)
    emit({0xC7,0x05,d&0xFF,(d>>8)&0xFF,(d>>16)&0xFF,(d>>24)&0xFF,value,0,0,0})
  end
  local jumps={}
  local function rejectUnlessEqual() emit({0x0F,0x85,0,0,0,0});jumps[#jumps+1]=#code-3 end
  if leaf then
    -- Entry RSP is 8 mod 16. Eight pushes plus 0x88 bytes align the call.
    emit({0x9C,0x50,0x51,0x52,0x41,0x50,0x41,0x51,0x41,0x52,0x41,0x53})
    local stackSize=insertion and 0x98 or 0x88
    local xmmOffset=insertion and 0x30 or 0x20
    emit({0x48,0x81,0xEC,stackSize,0,0,0})
    for i=0,5 do
      local offset=xmmOffset+i*16
      if offset<0x80 then emit({0xF3,0x0F,0x7F,0x44+i*8,0x24,offset})
      else emit({0xF3,0x0F,0x7F,0x84+i*8,0x24,offset,0,0,0}) end
    end
    emit({0x48,0xB9});u64(argument)
    emit({0x48,0xB8});u64(leaf)
    if secondArgument then emit({0x48,0xBA});u64(secondArgument) else emit({0x33,0xD2}) end
    emit({0xFF,0xD0,0x48,0xA3});u64(memory+0x308)
    if insertion then
      marker(0x318,1)
      emit({0x49,0xBA});u64(memory+0x600)
      emit({0x4C,0x39,0xD0});rejectUnlessEqual() -- RAX must be builder output.
      emit({0x49,0xBB});u64(insertion.serial)
      emit({0x4D,0x39,0x5A,0x28});rejectUnlessEqual()
      emit({0x49,0xBA});u64(insertion.data+(insertion.serial_counter_offset or 8))
      emit({0x49,0xBB});u64(insertion.serial+1)
      emit({0x4D,0x39,0x1A});rejectUnlessEqual()
      marker(0x318,2)
      emit({0x48,0xB9});u64(insertion.manager)
      emit({0x48,0xBA});u64(memory+0x800)
      emit({0x49,0xB8});u64(memory+0x600)
      emit({0x49,0xB9});u64(memory+0x320)
      emit({0xC7,0x44,0x24,0x20,0,0,0,0}) -- Fifth argument, separate from saved XMM registers.
      emit({0x48,0xB8});u64(insertion.function_address)
      emit({0xFF,0xD0,0x48,0xA3});u64(memory+0x328)
      marker(0x318,3)
    end
    for _,position in ipairs(jumps) do
      local delta=#code-(position+3)
      for i=0,3 do code[position+i]=(delta>>(8*i))&0xFF end
    end
    for i=0,5 do
      local offset=xmmOffset+i*16
      if offset<0x80 then emit({0xF3,0x0F,0x6F,0x44+i*8,0x24,offset})
      else emit({0xF3,0x0F,0x6F,0x84+i*8,0x24,offset,0,0,0}) end
    end
    emit({0x48,0x81,0xC4,stackSize,0,0,0,0x41,0x5B,0x41,0x5A,0x41,0x59,0x41,0x58,0x5A,0x59,0x58,0x9D})
  end
  local displacement=0x300-(#code+10)
  emit({0xC7,0x05,displacement&0xFF,(displacement>>8)&0xFF,0,0,1,0,0,0})
  emit(original);emit({0xFF,0x25,0,0,0,0});u64(resume)
  assert(#code<0x300,'Probe code overlaps data')
  return code
end
