"""Verify the new observer against supplied v2.01 instruction-byte excerpts.

This checks excerpt consistency and rel32 edges, NOT live execution, whole-image
AOB uniqueness, or completeness of unsupplied call targets.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CAP = ROOT / 'research' / 'possessed_enemy_capture'
LINE = re.compile(r'^([0-9A-Fa-f]{8,16}):\s+((?:[0-9A-Fa-f]{2}\s+)+)([A-Za-z].*)$')
# Exact call instructions relevant to the two independent routes and copies.
EDGES = {
    '00F1E300.asm': [(0xF1E4EC, 0x10D9180)],
    '021D9D24.asm': [(0x21D9D45,0x1C96F28),(0x21D9E1F,0x10D9180)],
    '021DBCE0.asm': [(0x21DC433,0x10D9180)],
    '02233AAC.asm': [(0x2233DE3,0x10D9180)],
    '022377B8.asm': [(0x2237973,0x20E198C),(0x223798F,0x1C244E4)],
    '020E198C.asm': [(0x20E19BD,0x20E1864)],
    '020E1864.asm': [(0x20E193E,0x1029E80),(0x20E1957,0x10292C0)],
    '02237C60.asm': [(0x2237D34,0x217B9C0)],
    '010D9180.asm': [(0x10D91BE,0x1029C30),(0x10D91C9,0x1029910)],
    '01029E80.asm': [(0x102A017,0x9D8978),(0x102A8F5,0x1027A10)],
}
SITE_FILES = {'request_enqueue':'010D9180.asm','request_queued':'010D9180.asm',
              'mission_consume':'020E198C.asm','mission_generated':'022377B8.asm'}


def parse_asm(text: str) -> dict[int, tuple[bytes,str]]:
    result = {}
    memory = {}
    for line in text.splitlines():
        m = LINE.match(line)
        if not m:
            continue
        address=int(m[1],16); code=bytes.fromhex(m[2]); asm=m[3].strip()
        if address in result:
            raise ValueError('duplicate instruction start')
        for offset,value in enumerate(code):
            if address+offset in memory:
                raise ValueError('overlapping instruction bytes')
            memory[address+offset]=value
        result[address]=(code,asm)
    if not result:
        raise ValueError('no instruction bytes')
    return result


def read_instruction_span(instructions, address: int, size: int) -> bytes:
    if address not in instructions:
        raise ValueError('site is not an instruction boundary')
    result=bytearray(); current=address
    while len(result)<size:
        if current not in instructions:
            raise ValueError('non-contiguous instruction span')
        code,_=instructions[current];result.extend(code);current+=len(code)
    if len(result)!=size:
        raise ValueError('signature ends inside an instruction')
    return bytes(result)


def direct_call(code: bytes, address: int) -> int:
    if len(code)!=5 or code[0]!=0xE8:
        raise ValueError('not an E8 rel32 call')
    return address+5+int.from_bytes(code[1:],'little',signed=True)


def verify(directory: Path) -> dict:
    locators=json.loads((CAP/'mode_upstream_sequence_v201_locators.json').read_text())
    parsed={};files={}
    for filename in set(SITE_FILES.values()) | set(EDGES):
        p=directory/filename; data=p.read_bytes()
        parsed[filename]=parse_asm(data.decode('utf-8'))
        files[filename]=hashlib.sha256(data).hexdigest()
    checked_sites=[]
    lua=(CAP/'mode_upstream_sequence_ce.lua').read_text()
    for name,filename in SITE_FILES.items():
        site=locators['sites'][name];address=int(site['rva'],16);expected=bytes.fromhex(site['bytes'])
        if site['length']!=len(expected) or site['mask']!='FF'*len(expected):
            raise ValueError('signature length/mask mismatch')
        actual=read_instruction_span(parsed[filename],address,len(expected))
        if expected!=actual:
            raise ValueError(f'signature mismatch: {name}')
        if site['bytes'] not in lua:
            raise ValueError('Lua and locator signature disagree')
        checked_sites.append({'site':name,'rva':site['rva'],'byte_count':len(expected),'source':filename})
    checked_edges=[]
    for filename,edges in EDGES.items():
        for address,target in edges:
            actual=direct_call(parsed[filename][address][0],address)
            if actual!=target:
                raise ValueError(f'call edge mismatch at {address:X}')
            checked_edges.append({'call_rva':hex(address),'target':hex(target),'source':filename})
    mission_calls=[(pc,direct_call(code,pc)) for pc,(code,_) in parsed['022377B8.asm'].items() if len(code)==5 and code[0]==0xE8]
    # Direct-local facts only. Do not make claims about unsupplied callee bodies.
    if [pc for pc,target in mission_calls if target==0x20E198C] != [0x2237973]:
        raise ValueError('mission wrapper invocation inventory changed')
    if any(target==0x10D9180 for _,target in mission_calls):
        raise ValueError('new direct enqueue in mission handler')
    return {'checked_sites':checked_sites,'checked_edges':checked_edges,
            'source_sha256':files,'mission_direct_wrapper_calls':[hex(0x2237973)],
            'scope':'supplied text disassembly with byte annotations; no fresh native/full-image proof'}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--disassembly-dir',type=Path,required=True)
    args=p.parse_args();print(json.dumps(verify(args.disassembly_dir),indent=2))

if __name__=='__main__':main()
