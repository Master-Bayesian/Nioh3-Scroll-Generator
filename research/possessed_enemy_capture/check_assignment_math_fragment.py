"""Opt-in LOCAL x64 math-fragment differential check; does not attach to a game.

The 33 instruction bytes under test are copied from the exact supplied .text
window 0x102854F..0x1028570 after checking the entire section hash. A SysV ABI
adapter supplies XMM7/XMM8 and R13d constants. It is not a native mission run.
Run in a disposable Linux x86-64 process. No game process or save is accessed.
"""
from __future__ import annotations
import argparse, ctypes, hashlib, json, mmap, platform, struct, sys
from pathlib import Path
from assignment_origin_reference import A, MASK32, draw_10000
TEXT_HASH='F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'
EXPECTED=bytes.fromhex('C1E9100F57C08BC1F3480F2AC0F30F59C7F3410F59C0F30F2CC0413BC5410F4FC5')

def check(text: Path):
    if sys.platform!='linux' or platform.machine().lower() not in ('x86_64','amd64'):
        raise RuntimeError('Local fragment adapter is Linux x86-64 SysV only')
    blob=text.read_bytes()
    if hashlib.sha256(blob).hexdigest().upper()!=TEXT_HASH:raise ValueError('Wrong .text')
    raw=blob[0x102854F-0x1000:0x1028570-0x1000]
    if raw!=EXPECTED:raise ValueError('Wrong fragment')
    # Save nonvolatile R13, ecx=uint32 argument, xmm7=2^-16, xmm8=10000.0.
    prefix=bytes.fromhex('415589F941BD0F270000B800008037660F6EF8B800401C4666440F6EC0')
    code=prefix+raw+bytes.fromhex('415DC3')
    mem=mmap.mmap(-1,mmap.PAGESIZE,prot=mmap.PROT_READ|mmap.PROT_WRITE)
    mem.write(code);address=ctypes.addressof(ctypes.c_char.from_buffer(mem))
    libc=ctypes.CDLL(None,use_errno=True);libc.mprotect.argtypes=(ctypes.c_void_p,ctypes.c_size_t,ctypes.c_int)
    if libc.mprotect(address,mmap.PAGESIZE,mmap.PROT_READ|mmap.PROT_EXEC)!=0:raise OSError(ctypes.get_errno(),'mprotect')
    fn=ctypes.CFUNCTYPE(ctypes.c_int32,ctypes.c_uint32)(address)
    inverse=pow(A,-1,2**32);diff=[]
    for h in range(65536):
        state=(h<<16)|456
        got=fn(state);_,expected=draw_10000(((state-1)*inverse)&MASK32)
        if got!=expected:raise AssertionError((h,got,expected))
        integer=h*10000//65536
        if got!=integer:diff.append({'high16':h,'native_ticket':got,'integer_shortcut':integer})
    del fn;mem.close()
    return {'scope':'isolated original range-reduction instructions, NOT mission/live parity',
            'source_rva':'0x102854F','end_exclusive':'0x1028570','original_bytes':len(raw),
            'original_fragment_sha256':hashlib.sha256(raw).hexdigest(),
            'high16_values_checked':65536,'mismatches':0,'integer_shortcut_disagreements':diff}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--text',type=Path,required=True)
    p.add_argument('--execute-local-x64-fragment',action='store_true',required=True)
    a=p.parse_args();print(json.dumps(check(a.text),indent=2))
