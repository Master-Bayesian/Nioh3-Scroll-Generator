"""Recheck new locators and de-identified control views against the INPUT ZIP tree.
No disassembly guessing or semantic inference from an address match is performed.
"""
from __future__ import annotations
import argparse,hashlib,json,struct
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
CAP=Path(__file__).resolve().parent
EXPECTED_TEXT='F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'

def verify(root: Path):
    text=(root/'evidence/runtime-sections/Nioh3_v2.0.1.0.text.bin').read_bytes()
    assert hashlib.sha256(text).hexdigest().upper()==EXPECTED_TEXT,'different binary'
    loc=json.loads((CAP/'assignment_origin_v201_locators.json').read_text())
    checked=[]
    for group in ('sites','static_anchors','function_locators'):
        for name,s in loc[group].items():
            r=int(s['rva'],16);raw=bytes.fromhex(s['bytes'])
            assert len(bytes.fromhex(s['mask']))==len(raw) and set(bytes.fromhex(s['mask']))=={255}
            assert text[r-0x1000:r-0x1000+len(raw)]==raw,(name,'mismatch')
            n=text.count(raw)
            # Instruction proof windows (including a common epilogue) are
            # addressed relative to their verified parent, not independent AOB keys.
            if group in ('sites','function_locators'):assert n==1,(name,n)
            elif 'unique_matches' in s:assert n==s['unique_matches'],(name,n)
            checked.append({'group':group,'name':name,'rva':s['rva'],'matches':n})
    f=ROOT/'tests/fixtures/assignment_origin_controls.json'
    cs=json.loads(f.read_text());n=0
    for c in cs:
        path=root/c['source'];rawfile=path.read_bytes()
        assert hashlib.sha256(rawfile).hexdigest()==c['source_sha256']
        d=json.loads(rawfile);final=[x for x in d['bridge_result']['events'] if x['site']=='late_mask_final'][-1]
        actual={}
        for r in final['records']:
            b=bytes.fromhex(r['record_raw_hex']);spawn=int.from_bytes(b[0x20:0x24],'little')
            if 0xF3C<=spawn<=0xF96:actual[spawn]=b
        assert len(actual)==len(c['records'])
        for v in c['records']:
            b=actual[v['spawn']]
            assert b[0x80:0x94].hex()==v['descriptor_hex']
            assert b[0x8F]==v['flag8f'] and b[0xE9]==v['e9'] and b[0xEA]==v['ea']
            assert int.from_bytes(b[0x28:0x2C],'little')==v['lookup']
            assert int.from_bytes(b[0x24:0x28],'little')==v['mission']
            assert (not any(b[:8]))==v['object0_is_null']
            assert (not any(b[0x18:0x20]))==v['object18_is_null']
            n+=1
    return {'input_controls_checked':len(cs),'generated_task_records_checked':n,'locators_checked':checked,
            'warning':'Byte/fixture integrity only; causal semantics are the documented typed data-flow. Not a new live run.'}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--handoff',required=True,type=Path)
    print(json.dumps(verify(p.parse_args().handoff),indent=2))
