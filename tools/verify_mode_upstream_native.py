"""Validate new mode-upstream locators and bounded disassembly against the handoff.

Requires the ORIGINAL raw section files, not a live game. Does not scan text for
semantic names, change target memory, or infer a function role from uniqueness.
"""
from pathlib import Path
import argparse,hashlib,json,re,struct
ROOT=Path(__file__).resolve().parents[1]

def verify(handoff:Path,report:Path,source:Path=ROOT):
    r=handoff/'evidence/runtime-sections'
    expected={
        'text':'F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023',
        'rdata':'DF245B4EF1478643845680DB183EA170F448B1EF8FF9710CBD040CB48F4E363D',
        'pdata':'928D0407FFCCFBD081559A2DF00D4995598B60B5C3F1D325D086CB531B09B904'}
    sections={}
    for name,digest in expected.items():
        b=(r/f'Nioh3_v2.0.1.0.{name}.bin').read_bytes()
        if hashlib.sha256(b).hexdigest().upper()!=digest:raise ValueError(f'{name} build hash mismatch')
        sections[name]=b
    text=sections['text'];p=sections['pdata'];ranges=set(struct.iter_unpack('<III',p[:len(p)//12*12]))
    catalog=json.loads((report/'NATIVE_SIGNATURES.json').read_text());instructions={};nbytes=0
    for f in catalog['functions']:
        a=int(f['rva'],16);z=int(f['end_exclusive'],16);b=text[a-0x1000:z-0x1000]
        if hashlib.sha256(b).hexdigest().upper()!=f['range_sha256']:raise ValueError('function bytes mismatch')
        sig=bytes.fromhex(f['signature'])
        if b[:len(sig)]!=sig or text.count(sig)!=1:raise ValueError('function locator mismatch')
        for u in f['unwind_ranges']:
            if tuple(int(u[k],16) for k in ['begin','end','unwind']) not in ranges:raise ValueError('unwind row mismatch')
        last=a
        for line in (report/f['evidence']).read_text().splitlines():
            m=re.match(r'([0-9A-Fa-f]+):\s*((?:[0-9a-f]{2} )*[0-9a-f]{2})\s+(.+)',line)
            if not m:continue
            pc=int(m[1],16);code=bytes.fromhex(m[2])
            if pc!=last or text[pc-0x1000:pc-0x1000+len(code)]!=code:raise ValueError('disassembly bytes/gap')
            last=pc+len(code);instructions[pc]=code;nbytes+=len(code)
        if last!=z:raise ValueError('truncated function listing')
    for c in catalog['checked_calls']:
        pc=int(c['site'],16);b=instructions[pc]
        if len(b)!=5 or b[0]!=0xE8 or pc+5+struct.unpack_from('<i',b,1)[0]!=int(c['target'],16):raise ValueError('not the claimed direct call')
    loc=json.loads((source/'research/possessed_enemy_capture/mode_upstream_v201_locators.json').read_text())
    lua=(source/'research/possessed_enemy_capture/mode_upstream_ce.lua').read_text()
    parsed={name:(int(rva,16),bytes.fromhex(hexbytes)) for name,rva,hexbytes in
        re.findall(r"(\w+)=\{rva=0x([0-9a-fA-F]+),hex='([0-9A-Fa-f]+)'",lua)}
    if len(loc['sites'])!=4 or len(parsed)!=4:raise ValueError('requires exactly four observation points')
    for name,s in loc['sites'].items():
        pc=int(s['rva'],16);b=bytes.fromhex(s['bytes'])
        if parsed[name]!=(pc,b) or text[pc-0x1000:pc-0x1000+len(b)]!=b or text.count(b)!=1:raise ValueError('Lua/JSON/build signature mismatch')
        if pc not in instructions:raise ValueError('site not on a disassembled instruction boundary')
        if tuple(int(x,16) for x in s['runtime_function']) not in ranges:raise ValueError('missing probe unwind owner')
    return dict(functions=len(catalog['functions']),disassembled_bytes=nbytes,
                instruction_starts=len(instructions),direct_calls=len(catalog['checked_calls']),
                live_sites=len(parsed),meaning='Pinned byte/locator checks only; not live or semantic acceptance')

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument('--handoff',type=Path,required=True)
    a.add_argument('--report',type=Path,required=True);a.add_argument('--source',type=Path,default=ROOT)
    args=a.parse_args();print(json.dumps(verify(args.handoff,args.report,args.source),indent=2))

if __name__=='__main__':main()
