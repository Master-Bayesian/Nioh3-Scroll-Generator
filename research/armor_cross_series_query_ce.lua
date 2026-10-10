-- Bounded native getter queries on temporary test records, not inventory items.
-- Loading returns an API and performs no reads, allocations, or native calls.
-- The verified runner supplies complete runtime code/row bytes and identity.
local function run(config)
  assert(type(config)=='table' and config.authorized_native_queries==true,
    'Explicit authorization for temporary-record native getter queries required')
  assert(type(config.pid)=='number' and type(config.module_base)=='number')
  assert(type(config.run_id)=='string' and #config.run_id<=128)
  assert(type(config.creation_filetime)=='string' and config.creation_filetime:match('^%d+$'))
  assert(type(config.expected_age_ms)=='number' and config.expected_age_ms>=0)
  assert(config.executable_sha256=='E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130')
  assert(config.stage>=1 and config.stage<=5 and config.stage%1==0)
  assert(type(config.signatures)=='table' and #config.signatures==11)
  assert(type(config.vectors)=='table' and #config.vectors==18)
  local allowed={[0x818018]='weight',[0x817FD4]='toughness',[0x2F9E30]='requirements'}
  local required={[0x818018]=true,[0x817FD4]=true,[0x2F9E30]=true,[0x8180AC]=true,
    [0x2F9E88]=true,[0x2F9EEC]=true,[0x2FB078]=true,[0x2F9F34]=true,[0x1111850]=true,[0xF4DBC]=true,[0xC624A8]=true}
  local ids={[0xC18E]=true,[0x35BC]=true,[0xC288]=true}
  local modes={['0/0']=true,['1/0']=true,['2/0']=true,['1/1']=true,['2/2']=true,['1/2']=true}
  assert(not nioh3ArmorCrossSeriesQuery or nioh3ArmorCrossSeriesQuery.released,
    'Previous native query allocation requires review')
  local pid,base,handle=config.pid,config.module_base,getOpenedProcessHandle()
  local age_start,tick_start=getProcessAge(),getTickCount()
  assert(type(age_start)=='number' and math.abs(age_start-config.expected_age_ms)<10000,
    'Process age differs from freshly verified lifetime')
  local function guard()
    assert(getOpenedProcessID()==pid and getAddressSafe('Nioh3.exe')==base and getOpenedProcessHandle()==handle,
      'Target attachment changed')
    assert(math.abs(getProcessAge()-age_start-(getTickCount()-tick_start))<2000,
      'Target process lifetime changed')
  end
  local reads=0
  local function bytes(address,n)
    assert(n>0 and n<=416);reads=reads+n;assert(reads<=262144,'Read budget exceeded')
    local value=assert(readBytes(address,n,true),'Unreadable approved range')
    assert(#value==n,'Partial read');return value
  end
  local function decode(hex,n)
    assert(type(hex)=='string' and #hex==n*2 and not hex:find('[^%x]'))
    local out={};for pair in hex:gmatch('..') do out[#out+1]=tonumber(pair,16) end;return out
  end
  local function same(a,b)
    assert(#a==#b);for i=1,#a do assert(a[i]==b[i],'Approved bytes changed') end
  end
  local function integer(a,n)
    local v=0;local b=bytes(a,n);for i=n,1,-1 do v=(v<<8)|b[i] end;return v
  end
  local function stable_context()
    guard()
    assert(integer(base+0x45B9E30,8)==config.manager)
    assert(integer(config.manager+0x68,8)==config.item_container)
    assert(integer(config.item_container,8)==config.item_raw)
    assert(integer(config.item_raw+4,4)==3362)
    assert(integer(config.item_container+0x20,8)==config.item_map)
    assert(integer(config.item_map+8,8)==config.item_map_begin)
    assert(integer(config.item_map+0x10,8)==config.item_map_end)
    assert(config.item_map_end>config.item_map_begin and (config.item_map_end-config.item_map_begin)%8==0)
    assert(integer(config.manager+0x70,8)==config.curve_container)
    assert(integer(config.curve_container,8)==config.curve_raw)
    assert(integer(config.curve_raw+4,4)==502)
    assert(integer(base+0x47514D0,8)==config.stage_container)
    assert(integer(config.stage_container+8,8)==config.stage_object)
    assert(integer(config.stage_object+0x45,1)==config.stage,'Stage changed')
  end
  stable_context()
  local signatures={}
  for _,site in ipairs(config.signatures) do
    assert(required[site.rva] and not signatures[site.rva],'Unexpected or repeated function range')
    assert(site.size>0 and site.size<=416)
    local expected=decode(site.hex,site.size)
    same(bytes(base+site.rva,site.size),expected);signatures[site.rva]=expected
  end
  local seen={}
  for _,vector in ipairs(config.vectors) do
    assert(ids[vector.item_id] and modes[vector.mode] and #vector.expected_requirements==7)
    local key=vector.item_id..':'..vector.mode;assert(not seen[key]);seen[key]=true
    assert(vector.row_index>=0 and vector.row_index<3362 and vector.selected_row_index>=0 and vector.selected_row_index<3362)
    same(bytes(config.item_raw+8+vector.row_index*416,416),decode(vector.row_hex,416))
    same(bytes(config.item_raw+8+vector.selected_row_index*416,416),decode(vector.selected_row_hex,416))
  end
  local debugging=debug_isDebugging()
  if debugging then
    local bps=debug_getBreakpointList();assert(type(bps)=='table' and next(bps)==nil,'Debugger has foreign breakpoints')
    assert(debug_getCurrentContextTable()==nil,'Debugger is stopped')
  end
  local state={run_id=config.run_id,pid=pid,stage=config.stage,active=false,released=false,
    in_flight=false,allocation=nil,events={},native_calls=0,inventory_written=false,
    executable_patched=false,save_written=false,read_bytes=reads}
  nioh3ArmorCrossSeriesQuery=state
  local scratch=assert(allocateMemory(272,nil,0x04),'Temporary record allocation failed')
  assert(scratch~=0);state.allocation=scratch;state.active=true
  local record=scratch+16
  local start=getTickCount()
  local ok,err=pcall(function()
    for _,vector in ipairs(config.vectors) do
      assert(getTickCount()-start<60000,'Query time budget exceeded')
      stable_context()
      local image={};for i=1,272 do image[i]=0xA5 end
      for i=17,256 do image[i]=0 end
      local function put(offset,value,n)
        for i=0,n-1 do image[17+offset+i]=(value>>(8*i))&255 end
      end
      put(0,vector.item_id,2);put(6,180,2);put(10,20,2);put(24,0,4);put(48,4,1)
      local a,b=vector.mode:match('^(%d)/(%d)$');put(49,tonumber(a),1);put(50,tonumber(b),1)
      assert(writeBytes(scratch,image),'Temporary record initialization failed')
      same(bytes(scratch,272),image)
      local row=config.item_raw+8+vector.row_index*416
      local function query(rva,index,expected)
        assert(allowed[rva] and state.native_calls<162)
        assert(getTickCount()-start<60000,'Query time budget exceeded')
        stable_context();same(bytes(base+rva,#signatures[rva]),signatures[rva])
        state.in_flight=true;state.native_calls=state.native_calls+1
        local actual
        if index~=nil then
          actual=executeCodeEx(0,1000,base+rva,{type=0,value=row},{type=0,value=record},{type=0,value=index})
        else
          actual=executeCodeEx(0,1000,base+rva,{type=0,value=row},{type=0,value=record})
        end
        assert(type(actual)=='number','Native completion unconfirmed; retain temporary allocation')
        state.in_flight=false;actual=actual&0xFFFFFFFF
        same(bytes(scratch,272),image);stable_context()
        state.events[#state.events+1]={item_id=vector.item_id,mode=vector.mode,kind=allowed[rva],
          stat_index=index,expected=expected,actual=actual,matched=actual==expected}
        assert(actual==expected,'Native getter/model mismatch')
      end
      query(0x818018,nil,vector.expected_weight)
      query(0x817FD4,nil,vector.expected_toughness)
      for index=0,6 do query(0x2F9E30,index,vector.expected_requirements[index+1]) end
      same(bytes(row,416),decode(vector.row_hex,416))
      same(bytes(config.item_raw+8+vector.selected_row_index*416,416),decode(vector.selected_row_hex,416))
    end
    assert(state.native_calls==162 and #state.events==162)
  end)
  state.active=false;state.read_bytes=reads;state.elapsed_ms=getTickCount()-start
  if not ok then state.error=tostring(err) end
  if not state.in_flight then
    local release_ok,release_error=pcall(function()
      guard();assert(deAlloc(scratch)==true,'Temporary allocation release failed')
    end)
    state.released=release_ok
    if not release_ok then state.cleanup_error=tostring(release_error) end
  else
    state.cleanup_error='Native thread completion unknown; allocation retained, do not retry'
  end
  state.success=ok and state.released
  return state
end
return {run=run}
