"""Pinned PC v2.01 read-only function exporter; no debugger, breakpoints, or calls.

Only explicit targets of verified E8 call instructions may be exported. Function
bounds come from the live image's PE exception directory, not nearby addresses.
Pure parsing functions below are tested with synthetic PE images. This tool has
not been run against the game in the analysis environment.
"""
from __future__ import annotations
import argparse
import ctypes as C
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import struct
import sys
from typing import Callable

EXPECTED_SHA256 = '4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'
EXPECTED_FILE_SIZE = 77814240
EXPECTED_TEXT_SHA256 = 'F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'
EXPECTED_TEXT_SIZE = 59624448
TEXT_RVA = 0x1000
PDATA_RVA = 0x4E24000
MAX_READ = 8 * 1024 * 1024
MAX_FUNCTION = 64 * 1024
# (call-site RVA, exact E8 bytes, target RVA, limited purpose)
REQUESTS = (
    (0x2235E04, 'E8F3060000', 0x22364FC, 'alternate queue-node continuation'),
    (0x1C245C7, 'E8745721FF', 0xE39D40, 'class 0/1 pre-iteration helper'),
    (0x1C2460F, 'E8382251FE', 0x13684C, 'task key lookup and container layout'),
    (0x4BC93F, 'E8B40C0000', 0x4BD5F8, 'tagged/base task insertion helper'),
    (0x4BC919, 'E806130000', 0x4BDC24, 'tagged task constructor'),
    (0x1C24540, 'E87B29FDFF', 0x1BF6EC0, 'pre-materialization reset helper'),
    (0x223799B, 'E860E99EFF', 0x1C26300, 'first post-materializer continuation'),
    (0x22379CC, 'E85FCA9EFF', 0x1C24430, 'following request/state continuation'),
)
# This direct-call target is a verified x64 leaf function and therefore has no
# entry in this build's exception directory. Its end is pinned by complete
# control-flow disassembly of the identified build, not inferred from the next
# pdata row or an adjacent address.
PINNED_LEAF_INTERVALS = {
    0x13684C: (0x1368A2, '627C74A4D65ED70D8F1D6A6D699C6786B23E4A49D5C3EDCF0B81FD134851BF59'),
}


class CaptureError(RuntimeError): pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


class BoundedReader:
    def __init__(self, read: Callable[[int, int], bytes], limit: int = MAX_READ):
        self.read_raw = read
        self.limit = limit
        self.used = 0

    def __call__(self, address: int, size: int) -> bytes:
        if type(address) is not int or type(size) is not int or not 0 < address < (1 << 47):
            raise CaptureError('invalid address')
        if size < 0 or size > 4*1024*1024 or self.used + size > self.limit:
            raise CaptureError('read budget exceeded')
        self.used += size
        raw = self.read_raw(address, size)
        if not isinstance(raw, bytes) or len(raw) != size:
            raise CaptureError('short/unreadable process data')
        return raw


def rel32_call_target(rva: int, data: bytes) -> int:
    if len(data) != 5 or data[0] != 0xE8:
        raise CaptureError('not an exact direct E8 call')
    return rva + 5 + struct.unpack_from('<i', data, 1)[0]


@dataclass(frozen=True)
class Section:
    name: str
    rva: int
    size: int
    flags: int


@dataclass(frozen=True)
class RuntimeFunction:
    begin: int
    end: int
    unwind: int


@dataclass(frozen=True)
class Image:
    size: int
    sections: tuple[Section, ...]
    exception_rva: int
    exception_data: bytes
    functions: tuple[RuntimeFunction, ...]

    def exact_function(self, target: int) -> RuntimeFunction:
        matches = [f for f in self.functions if f.begin == target]
        if len(matches) != 1:
            raise CaptureError(f'no unique exception-directory function BEGIN at {target:#x}')
        f = matches[0]
        if f.end <= f.begin or f.end - f.begin > MAX_FUNCTION:
            raise CaptureError('function length exceeds bounded exporter; never truncate')
        if not any(s.name == '.text' and s.flags & 0x20000000 and
                   s.rva <= f.begin < f.end <= s.rva+s.size for s in self.sections):
            raise CaptureError('function is not entirely within executable .text')
        return f


def resolve_function_interval(image: Image, target: int, reader: BoundedReader,
                              base: int) -> tuple[RuntimeFunction, str]:
    try:
        return image.exact_function(target), 'exception_directory_exact_begin'
    except CaptureError:
        pinned = PINNED_LEAF_INTERVALS.get(target)
        if pinned is None:
            raise

    end, expected_sha256 = pinned
    if any(f.begin <= target < f.end for f in image.functions):
        raise CaptureError(f'pinned leaf unexpectedly overlaps pdata at {target:#x}')
    if not 0 < target < end or end-target > MAX_FUNCTION:
        raise CaptureError('pinned leaf interval bound')
    if not any(s.name == '.text' and s.flags & 0x20000000 and
               s.rva <= target < end <= s.rva+s.size for s in image.sections):
        raise CaptureError('pinned leaf is not entirely within executable .text')
    raw = reader(base+target, end-target)
    if sha256(raw) != expected_sha256:
        raise CaptureError(f'pinned leaf bytes changed at {target:#x}')
    return RuntimeFunction(target, end, 0), 'pinned_leaf_complete_control_flow'


def read_image(reader: BoundedReader, base: int) -> Image:
    dos = reader(base, 64)
    if dos[:2] != b'MZ': raise CaptureError('DOS signature')
    off = struct.unpack_from('<I', dos, 0x3c)[0]
    if not 64 <= off <= 0x10000: raise CaptureError('e_lfanew bound')
    head = reader(base+off, 24)
    if head[:4] != b'PE\0\0': raise CaptureError('PE signature')
    machine, nsections = struct.unpack_from('<HH', head, 4)
    optsize = struct.unpack_from('<H', head, 20)[0]
    if machine != 0x8664 or not 1 <= nsections <= 96 or not 144 <= optsize <= 4096:
        raise CaptureError('AMD64 section/optional-header bound')
    opt = reader(base+off+24, optsize)
    if struct.unpack_from('<H', opt)[0] != 0x20B: raise CaptureError('not PE32+')
    size = struct.unpack_from('<I', opt, 56)[0]
    ndir = struct.unpack_from('<I', opt, 108)[0]
    if not 0x1000 <= size <= 0x20000000 or ndir < 4: raise CaptureError('image/data-directory bound')
    erva, esize = struct.unpack_from('<II', opt, 112+3*8)
    if esize == 0 or esize % 12 or esize > 4*1024*1024 or not 0 < erva < erva+esize <= size:
        raise CaptureError('exception directory bound')
    raw = reader(base+off+24+optsize, nsections*40)
    sections = []
    for i in range(nsections):
        row=raw[40*i:40*(i+1)]
        name=row[:8].rstrip(b'\0').decode('ascii','strict')
        vsize, rva, rawsize = struct.unpack_from('<III',row,8)
        extent=max(vsize,rawsize)
        if rva+extent > size: raise CaptureError('section exceeds image')
        sections.append(Section(name,rva,extent,struct.unpack_from('<I',row,36)[0]))
    if not any(s.rva <= erva and erva+esize <= s.rva+s.size for s in sections):
        raise CaptureError('exception directory outside sections')
    pdata=reader(base+erva,esize)
    functions=[]
    prev=-1
    for begin,end,unwind in struct.iter_unpack('<III',pdata):
        if not 0 < begin < end <= size or not 0 < unwind < size or begin <= prev:
            raise CaptureError('invalid/unsorted runtime function table')
        functions.append(RuntimeFunction(begin,end,unwind));prev=begin
    return Image(size,tuple(sections),erva,pdata,tuple(functions))


def collect(reader: BoundedReader, base: int, image: Image) -> dict:
    functions=[]
    for call, sig, target, purpose in REQUESTS:
        expected=bytes.fromhex(sig)
        actual=reader(base+call,5)
        if actual != expected or rel32_call_target(call,actual) != target:
            raise CaptureError(f'call-site signature/target mismatch at {call:#x}')
        f,boundary_source=resolve_function_interval(image,target,reader,base)
        raw=reader(base+f.begin,f.end-f.begin)
        if raw != reader(base+f.begin,len(raw)):
            raise CaptureError(f'code changed during read at {f.begin:#x}')
        # Short, bounded context around a KNOWN call instruction; not discovery by proximity.
        surrounding=reader(base+call-8,21)
        functions.append({'begin_rva':f.begin,'end_rva':f.end,
                          'unwind_rva':f.unwind or None,'boundary_source':boundary_source,
                          'purpose':purpose,'call_site_rva':call,'call_bytes':actual.hex().upper(),
                          'call_window_rva':call-8,'call_window_hex':surrounding.hex().upper(),
                          'body_hex':raw.hex().upper(),'body_sha256':sha256(raw)})
    return {'schema':'nioh3.augmentation-code-bodies.v1','module_base':hex(base),
            'image_size':image.size,'read_bytes':reader.used,
            'sections':[vars(s) for s in image.sections],
            'exception_directory_rva':image.exception_rva,
            'exception_directory_sha256':sha256(image.exception_data),
            'functions':functions,'read_only':True,'breakpoints_used':0,
            'calls_game_functions':False,'writes_game_memory':False,
            'runtime_body_provenance':'pinned file identity plus caller signatures; runtime bytes may include external instrumentation',
            'mechanism_resolved':False}


def parse_pdata_section(data: bytes, image_size: int = 0x6000000) -> tuple[bytes, tuple[RuntimeFunction, ...]]:
    """Section dump may have zero alignment padding beyond the exception table.

    Never infer function bounds from proximity: trim only an all-zero suffix,
    require increasing begins, and require an exact BEGIN for every target.
    The handoff's historical .pdata digest is truncated; compute a new digest,
    do not pretend it is a matching full SHA-256.
    """
    if not data or len(data)>4*1024*1024:raise CaptureError('pdata section bound')
    functions=[];end_used=0;prev=-1
    for off in range(0,len(data)-11,12):
        begin,end,unwind=struct.unpack_from('<III',data,off)
        if begin==end==unwind==0:
            if any(data[off:]):raise CaptureError('nonzero data after empty pdata entry')
            break
        if not 0<begin<end<=image_size or not 0<unwind<image_size or begin<=prev:
            raise CaptureError('invalid/unsorted pdata section')
        functions.append(RuntimeFunction(begin,end,unwind));end_used=off+12;prev=begin
    if not functions or any(data[end_used:]):raise CaptureError('nonzero pdata suffix; export PE exception directory instead')
    return data[:end_used],tuple(functions)


def from_section_bytes(text: bytes, pdata: bytes, verify_text: bool = True):
    if verify_text and (len(text)!=EXPECTED_TEXT_SIZE or sha256(text)!=EXPECTED_TEXT_SHA256):
        raise CaptureError('offline .text size/SHA-256 does not match pinned v2.01')
    raw,functions=parse_pdata_section(pdata)
    base=0x140000000  # synthetic addressing only; not a claimed runtime module base
    image=Image(0x6000000,(Section('.text',TEXT_RVA,len(text),0x60000020),),PDATA_RVA,raw,functions)
    def read(a,n):
        offset=a-base-TEXT_RVA
        if offset<0 or offset+n>len(text):raise CaptureError('read beyond supplied .text')
        return text[offset:offset+n]
    result=collect(BoundedReader(read),base,image)
    result['module_base']=None
    result['capture_kind']='offline section slicing; no process access'
    result['text_sha256']=sha256(text)
    result['pdata_section_sha256']=sha256(pdata)
    result['pdata_hash_matched_prior_manifest']=False
    result['pdata_provenance_note']='historical digest incomplete; validated sorted bounds and exact target entries'
    return result,raw


class WindowsReader:
    """QUERY_LIMITED_INFORMATION | VM_READ only. Handle binds one process lifetime."""
    def __init__(self, pid: int):
        if sys.platform != 'win32': raise CaptureError('live export requires Windows')
        from ctypes import wintypes as W
        self.k=C.WinDLL('kernel32',use_last_error=True)
        k=self.k
        for name,args,ret in [
            ('OpenProcess',[W.DWORD,W.BOOL,W.DWORD],W.HANDLE),
            ('CloseHandle',[W.HANDLE],W.BOOL),
            ('ReadProcessMemory',[W.HANDLE,C.c_void_p,C.c_void_p,C.c_size_t,C.POINTER(C.c_size_t)],W.BOOL),
            ('GetProcessTimes',[W.HANDLE]+[C.POINTER(W.FILETIME)]*4,W.BOOL),
            ('GetExitCodeProcess',[W.HANDLE,C.POINTER(W.DWORD)],W.BOOL),
            ('QueryFullProcessImageNameW',[W.HANDLE,W.DWORD,W.LPWSTR,C.POINTER(W.DWORD)],W.BOOL),
            ('CreateToolhelp32Snapshot',[W.DWORD,W.DWORD],W.HANDLE)]:
            fn=getattr(k,name);fn.argtypes=args;fn.restype=ret
        self.h=k.OpenProcess(0x1010,False,pid)
        if not self.h: raise C.WinError(C.get_last_error())
        self.pid=pid
        try:
            buf=C.create_unicode_buffer(32768);n=W.DWORD(len(buf))
            if not k.QueryFullProcessImageNameW(self.h,0,buf,C.byref(n)): raise C.WinError(C.get_last_error())
            self.path=Path(buf.value)
            if self.path.name.lower() != 'nioh3.exe': raise CaptureError('not Nioh3.exe')
            if self.path.stat().st_size != EXPECTED_FILE_SIZE: raise CaptureError('wrong executable size')
            with self.path.open('rb') as f:
                h=hashlib.sha256()
                for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
            if h.hexdigest().upper() != EXPECTED_SHA256: raise CaptureError('wrong executable SHA-256')
            self.birth=self.identity()
            class MODULEENTRY32W(C.Structure):
                _fields_=[('dwSize',W.DWORD),('th32ModuleID',W.DWORD),('th32ProcessID',W.DWORD),
                          ('GlblcntUsage',W.DWORD),('ProccntUsage',W.DWORD),('modBaseAddr',C.c_void_p),
                          ('modBaseSize',W.DWORD),('hModule',W.HMODULE),('szModule',W.WCHAR*256),('szExePath',W.WCHAR*260)]
            k.Module32FirstW.argtypes=[W.HANDLE,C.POINTER(MODULEENTRY32W)];k.Module32FirstW.restype=W.BOOL
            k.Module32NextW.argtypes=k.Module32FirstW.argtypes;k.Module32NextW.restype=W.BOOL
            snapshot=k.CreateToolhelp32Snapshot(0x18,pid)
            if snapshot in (None, C.c_void_p(-1).value): raise C.WinError(C.get_last_error())
            try:
                row=MODULEENTRY32W();row.dwSize=C.sizeof(row);matches=[]
                ok=k.Module32FirstW(snapshot,C.byref(row));count=0
                while ok:
                    count+=1
                    if count>4096:raise CaptureError('module count bound')
                    if row.szModule.lower()=='nioh3.exe':
                        if Path(row.szExePath).resolve()!=self.path.resolve():raise CaptureError('module/exe path mismatch')
                        matches.append(int(row.modBaseAddr))
                    ok=k.Module32NextW(snapshot,C.byref(row))
                if len(matches)!=1:raise CaptureError('no unique main module')
                self.base=matches[0]
            finally:k.CloseHandle(snapshot)
            if self.identity()!=self.birth: raise CaptureError('process identity changed')
        except BaseException:
            self.close();raise

    def identity(self) -> str:
        from ctypes import wintypes as W
        code=W.DWORD();times=[W.FILETIME() for _ in range(4)]
        if not self.k.GetExitCodeProcess(self.h,C.byref(code)) or code.value!=259:
            raise CaptureError('process has exited or exit status unreadable')
        if not self.k.GetProcessTimes(self.h,*(C.byref(x) for x in times)):raise C.WinError(C.get_last_error())
        return str((times[0].dwHighDateTime<<32)|times[0].dwLowDateTime)

    def read(self,address:int,size:int) -> bytes:
        if self.identity()!=self.birth:raise CaptureError('process instance changed')
        buf=C.create_string_buffer(size);n=C.c_size_t()
        if not self.k.ReadProcessMemory(self.h,C.c_void_p(address),buf,size,C.byref(n)) or n.value!=size:
            raise CaptureError(f'unreadable memory {address:#x}+{size:#x}')
        return buf.raw

    def close(self):
        if self.h:self.k.CloseHandle(self.h);self.h=None


def main(argv=None):
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--pid',type=int,help='Live read-only alternative; not required with section files')
    p.add_argument('--text',type=Path,help='Preferred: existing pinned v2.01 raw .text section')
    p.add_argument('--pdata',type=Path,help='Matching raw .pdata section (including zero padding)')
    p.add_argument('--output',type=Path,required=True)
    args=p.parse_args(argv)
    offline=args.text is not None or args.pdata is not None
    if offline and (args.text is None or args.pdata is None or args.pid is not None):
        p.error('use both --text and --pdata OR --pid, not a mixture')
    if not offline and (args.pid is None or args.pid<=0):p.error('provide section files or a positive --pid')
    args.output.mkdir(parents=True,exist_ok=False)
    process=None
    try:
        if offline:
            if args.text.stat().st_size!=EXPECTED_TEXT_SIZE or args.pdata.stat().st_size>4*1024*1024:
                raise CaptureError('section input size bound')
            result,pdata=from_section_bytes(args.text.read_bytes(),args.pdata.read_bytes())
        else:
            process=WindowsReader(args.pid);reader=BoundedReader(process.read)
            image=read_image(reader,process.base);result=collect(reader,process.base,image);pdata=image.exception_data
            if process.identity()!=process.birth:raise CaptureError('process exited/replaced during export')
            result['identity']={'pid':args.pid,'creation_filetime':process.birth,
                               'executable_sha256':EXPECTED_SHA256,'executable_file_size':EXPECTED_FILE_SIZE}
        (args.output/'exception_directory.bin').write_bytes(pdata)
        for f in result['functions']:
            name=f"{f['begin_rva']:08X}.bin";raw=bytes.fromhex(f['body_hex'])
            (args.output/name).write_bytes(raw)
            if sha256((args.output/name).read_bytes())!=f['body_sha256']:raise CaptureError('disk verification')
            f['body_file']=name
        (args.output/'FUNCTION_BODIES.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
        print(f"Exported {len(result['functions'])} exact runtime functions; no game actions or breakpoints.")
    except BaseException as exc:
        (args.output/'CAPTURE_FAILED.json').write_text(json.dumps({'error':f'{type(exc).__name__}: {exc}',
            'mechanism_resolved':False,'complete':False},indent=2),encoding='utf-8')
        raise
    finally:
        if process is not None:process.close()


if __name__=='__main__':main()
