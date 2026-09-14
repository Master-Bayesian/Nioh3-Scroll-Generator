"""Execute actual Lua 5.4 code with deterministic CE API mocks; never native-game proof."""
import os
import ctypes as C
import ctypes.util

class Lua54:
    def __init__(self):
        name=os.environ.get('LUA54_LIBRARY') or ctypes.util.find_library('lua5.4') or ctypes.util.find_library('lua54')
        if not name:raise RuntimeError('Lua 5.4 shared library required (liblua5.4.so / lua54.dll)')
        self.l=C.CDLL(name);l=self.l
        def api(n,args,ret):
            f=getattr(l,n);f.argtypes=args;f.restype=ret
        api('luaL_newstate',[],C.c_void_p);api('luaL_openlibs',[C.c_void_p],None)
        api('lua_close',[C.c_void_p],None)
        api('luaL_loadbufferx',[C.c_void_p,C.c_char_p,C.c_size_t,C.c_char_p,C.c_char_p],C.c_int)
        api('lua_pcallk',[C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ssize_t,C.c_void_p],C.c_int)
        api('lua_gettop',[C.c_void_p],C.c_int);api('lua_settop',[C.c_void_p,C.c_int],None)
        api('lua_type',[C.c_void_p,C.c_int],C.c_int)
        api('lua_tolstring',[C.c_void_p,C.c_int,C.POINTER(C.c_size_t)],C.c_void_p)
        api('lua_tointegerx',[C.c_void_p,C.c_int,C.c_void_p],C.c_longlong)
        api('lua_tonumberx',[C.c_void_p,C.c_int,C.c_void_p],C.c_double)
        api('lua_isinteger',[C.c_void_p,C.c_int],C.c_int)
        api('lua_toboolean',[C.c_void_p,C.c_int],C.c_int)
        api('lua_pushnil',[C.c_void_p],None);api('lua_next',[C.c_void_p,C.c_int],C.c_int)
        self.s=l.luaL_newstate();l.luaL_openlibs(self.s)
    def close(self):
        if self.s:self.l.lua_close(self.s);self.s=None
    def __del__(self):self.close()
    def value(self,idx):
        l,s=self.l,self.s;t=l.lua_type(s,idx)
        if t==0:return None
        if t==1:return bool(l.lua_toboolean(s,idx))
        if t==3:return l.lua_tointegerx(s,idx,None) if l.lua_isinteger(s,idx) else l.lua_tonumberx(s,idx,None)
        if t==4:
            size=C.c_size_t();p=l.lua_tolstring(s,idx,C.byref(size));return C.string_at(p,size.value).decode('utf-8',errors='replace')
        if t==5:
            absidx=idx if idx>0 else l.lua_gettop(s)+idx+1
            out={};l.lua_pushnil(s)
            while l.lua_next(s,absidx):
                if l.lua_type(s,-1)!=6:out[self.value(-2)]=self.value(-1)
                l.lua_settop(s,-2)
            if out and all(isinstance(k,int) and not isinstance(k,bool) for k in out) and set(out)==set(range(1,len(out)+1)):
                return [out[k] for k in range(1,len(out)+1)]
            return out
        return '<lua-type-%s>'%t
    def run(self,source):
        b=source.encode();l,s=self.l,self.s
        if l.luaL_loadbufferx(s,b,len(b),b'test',None) or l.lua_pcallk(s,0,1,0,0,None):
            e=self.value(-1);l.lua_settop(s,0);raise RuntimeError(e)
        v=self.value(-1);l.lua_settop(s,0);return v
