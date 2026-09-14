"""PC v2.01 read-only capture: A90 positions, enemy/subtype and config rows.

No CE, breakpoints, remote threads, game-function calls, WriteProcessMemory,
process suspension, or save-file access. Run only after resources are loaded.
The resulting profile extends table coverage, never caches answers by Seed.
"""
from __future__ import annotations
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import struct
import sys
import time

EXE_SHA='4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159'
TEXT_SHA='F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'
PARAM_RVA=0x45B5DF0
SIGNATURES={'0x10283c0': '48895c240848896c2410488974241857', '0x102bfd0': '488b05199e58034c8d4c2438488d4db8', '0x1027a10': '48895c240848896c2410565741564883', '0xe3a900': '48894c240853415441554883ec40f30f'}


class CaptureError(RuntimeError):pass


class BudgetReader:
    def __init__(self,reader,max_bytes=32*1024*1024,max_seconds=20):
        self.reader=reader;self.remaining=max_bytes;self.deadline=time.monotonic()+max_seconds
    def read(self,address,size):
        if not 0<address<0x800000000000 or size<0 or size>self.remaining:
            raise CaptureError('invalid pointer/read budget')
        if time.monotonic()>self.deadline:raise CaptureError('capture time budget expired')
        self.remaining-=size
        data=self.reader.read(address,size)
        if len(data)!=size:raise CaptureError('short read')
        return data
    def qword(self,address):return struct.unpack('<Q',self.read(address,8))[0]


def indexed_table(reader,context,stride,key_width,cap=4096):
    ctx=reader.read(context,0x28);store=struct.unpack_from('<Q',ctx)[0]
    header=reader.read(store,8);count=struct.unpack_from('<I',header,4)[0]
    if not 0<count<=cap:raise CaptureError('table count outside bounds')
    hp=struct.unpack_from('<Q',ctx,0x20)[0];h=reader.read(hp,0x20)
    begin,end=struct.unpack_from('<QQ',h,8)
    if end<begin or (end-begin)%8 or (end-begin)//8>cap*4:raise CaptureError('invalid hash vector')
    raw=reader.read(store,8+count*stride);entries=reader.read(begin,end-begin)
    if reader.read(context,0x28)!=ctx or reader.read(hp,0x20)!=h or reader.read(store,len(raw))!=raw or reader.read(begin,len(entries))!=entries:
        raise CaptureError('table changed during capture; keep failure, do not publish profile')
    sentinel=int.from_bytes(h[4:4+key_width],'little');mapping={}
    for offset in range(0,len(entries),8):
        key=int.from_bytes(entries[offset:offset+key_width],'little');i=struct.unpack_from('<I',entries,offset+4)[0]
        if key==sentinel or i>=count:continue  # Native lookup returns null for invalid row.
        if key in mapping:raise CaptureError('duplicate live hash key')
        mapping[key]=i
    if not mapping:raise CaptureError('no active indexed rows')
    return {'stride':stride,'count':count,'key_width':key_width,'mapping':mapping,
            'rows':raw,'context':ctx,'hash_context':h,'hash_entries':entries}


def row(table,key):
    i=table['mapping'].get(key)
    if i is None:return None
    s=table['stride'];return table['rows'][8+i*s:8+(i+1)*s]


def build_profile(positions,tables,selector):
    active=tables['enemy_40' if selector else 'enemy_38'];sub=tables['subtype'];cfg=tables['config']
    byterrain={}
    for i in range(0,len(positions),24):
        r=positions[i:i+24]
        if len(r)!=24:raise CaptureError('position row partial')
        byterrain.setdefault(f'0x{r[0x12]:X}',{'complete_terrain_scan':True,'rows_hex':[]})['rows_hex'].append(r.hex())
    gates={}
    for key,i in active['mapping'].items():
        r=row(active,key);sk=struct.unpack_from('<H',r,0xA8)[0];sr=row(sub,sk)
        e={'enemy_row_present':True,'enemy_row_index':i,'enemy_weight244':struct.unpack_from('<I',r,0x244)[0],
           'subtype_key':sk,'subtype_row_present':sr is not None}
        if sr is not None:e.update(flags14=sr[0x14],subtype_raw_hex=sr.hex())
        gates[f'0x{key:X}']=e
    c=row(cfg,0x4543)
    return dict(schema_version=1,text_sha256=TEXT_SHA,coverage='complete captured table indexes',
                enemy_index_complete=True,enemy_table_selector=selector,positions_by_terrain=byterrain,
                eligibility_by_lookup=gates,config_4543_lookup_observed=True,
                config_4543_hex=c.hex() if c else None,
                source_note='Read-only matching-PC-v2.01 parameter capture. Contains no per-Seed result lookup.')


def capture(reader,base):
    for rva,expected in SIGNATURES.items():
        if reader.read(base+int(rva,16),len(bytes.fromhex(expected)))!=bytes.fromhex(expected):
            raise CaptureError('PC v2.01 local signature mismatch at '+rva)
    manager=reader.qword(base+PARAM_RVA)
    selector=reader.read(manager+0xB0A,1)[0]
    contexts={n:reader.qword(manager+off) for n,off in [('enemy_38',0x38),('enemy_40',0x40),('subtype',0x118),('config',0x230),('positions',0xA90)]}
    tables={}
    for n,stride,width in [('enemy_38',0x398,4),('enemy_40',0x398,4),('subtype',0x54,2),('config',0x20,4)]:
        if contexts[n]:tables[n]=indexed_table(reader,contexts[n],stride,width)
    if ('enemy_40' if selector else 'enemy_38') not in tables:raise CaptureError('active enemy table absent')
    if 'subtype' not in tables or 'config' not in tables:raise CaptureError('required table absent')
    pc=reader.read(contexts['positions'],16);begin,end=struct.unpack('<QQ',pc)
    if end<=begin or (end-begin)%24 or (end-begin)//24>4096:raise CaptureError('invalid A90 vector')
    positions=reader.read(begin,end-begin)
    if reader.read(contexts['positions'],16)!=pc or reader.read(begin,end-begin)!=positions:raise CaptureError('positions changed')
    if reader.qword(base+PARAM_RVA)!=manager or reader.read(manager+0xB0A,1)[0]!=selector:raise CaptureError('manager/selector changed')
    for n,off in [('enemy_38',0x38),('enemy_40',0x40),('subtype',0x118),('config',0x230),('positions',0xA90)]:
        if reader.qword(manager+off)!=contexts[n]:raise CaptureError('table context changed')
    return build_profile(positions,tables,selector),tables,positions


def verified_executable(reader):
    fn=reader.dll.QueryFullProcessImageNameW
    fn.argtypes=[wintypes.HANDLE,wintypes.DWORD,wintypes.LPWSTR,ctypes.POINTER(wintypes.DWORD)]
    fn.restype=wintypes.BOOL
    buf=ctypes.create_unicode_buffer(32768);size=wintypes.DWORD(len(buf))
    if not fn(reader.handle,0,buf,ctypes.byref(size)):raise ctypes.WinError(ctypes.get_last_error())
    p=Path(buf.value);h=hashlib.sha256()
    with p.open('rb') as f:
        while chunk:=f.read(1<<20):h.update(chunk)
    if h.hexdigest().upper()!=EXE_SHA:raise CaptureError('executable identity mismatch')
    return {'file_name':p.name,'file_size':p.stat().st_size,'sha256':h.hexdigest()}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(argv)
    if sys.platform!='win32':parser.error('live capture requires Windows; profile loading/tests are cross-platform')
    sys.path.insert(0,str(Path(__file__).resolve().parents[2]))
    from nioh3_scroll_editor.process_memory_readonly import ProcessReader
    args.output.mkdir(parents=True,exist_ok=False)
    try:
        with ProcessReader() as r:
            exe=verified_executable(r);creation=r.creation_time()
            profile,tables,positions=capture(BudgetReader(r),r.module_base)
            if r.creation_time()!=creation:raise CaptureError('process identity changed')
        # No callbacks and no memory handles remain open at this point.
        for n,t in tables.items():
            d=args.output/n;d.mkdir()
            for key in ['rows','context','hash_context','hash_entries']:(d/(key+'.bin')).write_bytes(t[key])
        (args.output/'positions.bin').write_bytes(positions)
        (args.output/'enemy_state_tables.json').write_text(json.dumps(profile,indent=2)+'\n',encoding='utf8')
        files={str(p.relative_to(args.output)):hashlib.sha256(p.read_bytes()).hexdigest() for p in args.output.rglob('*') if p.is_file()}
        (args.output/'manifest.json').write_text(json.dumps({'executable':exe,'read_only':True,'files':files},indent=2)+'\n')
        print('CAPTURED',args.output,'; not a new native generation parity result')
        return 0
    except Exception as e:
        (args.output/'CAPTURE_FAILED.txt').write_text(f'{type(e).__name__}: {e}\n')
        raise

if __name__=='__main__':raise SystemExit(main())
