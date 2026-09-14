"""Reproduce the six-vs-ten derivation and check explicit E8 evidence.

Use only this handoff's original .asm byte columns and raw captures. This is a
focused analysis, not heuristic address discovery or an algorithm completeness gate.
"""
from __future__ import annotations
import argparse
from pathlib import Path
import hashlib
import json
import re
import sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from dump_augmentation_function_bodies import REQUESTS, rel32_call_target
from mode_augmentation_reference import compare_native_controls

INSTRUCTION=re.compile(r'^([0-9A-Fa-f]+):\s+((?:[0-9A-Fa-f]{2}\s+)+)(\S+)(.*)$')


def parse_asm(path: Path):
    instructions={};image={}
    for line_number,line in enumerate(path.read_text(encoding='utf-8').splitlines(),1):
        m=INSTRUCTION.match(line)
        if not m:continue
        a=int(m.group(1),16);raw=bytes.fromhex(m.group(2))
        if a in instructions:raise ValueError('duplicate instruction address')
        instructions[a]={'raw':raw,'mnemonic':m.group(3),'operands':m.group(4).strip(),
                         'line':line_number,'file':path.name}
        for i,b in enumerate(raw):
            if a+i in image:raise ValueError('overlapping instruction bytes')
            image[a+i]=b
    if not instructions:raise ValueError('no instructions')
    return instructions,image


def analyze(handoff: Path):
    dis=handoff/'evidence/prior-mode-upstream-static/disassembly'
    instructions={};images={}
    for p in sorted(dis.glob('*.asm')):
        rows,data=parse_asm(p)
        for a,row in rows.items():
            if a in instructions and instructions[a]['raw']!=row['raw']:raise ValueError('conflicting asm')
            instructions[a]=row
        images[p.name]=data
    calls=[]
    for call,sig,target,purpose in REQUESTS:
        row=instructions[call]
        assert row['raw']==bytes.fromhex(sig) and row['mnemonic']=='call'
        assert rel32_call_target(call,row['raw'])==target
        calls.append({'site_rva':hex(call),'target_rva':hex(target),'bytes':sig,'purpose':purpose,
                      'evidence':f"evidence/prior-mode-upstream-static/disassembly/{row['file']}:{row['line']}",
                      'target_instruction_present_in_package':target in instructions})
    cpath=handoff/'evidence/live-sequence-c/mode-upstream-sequence.json'
    dpath=handoff/'evidence/prior-run-d/assignment-origin.json'
    result=compare_native_controls(json.loads(cpath.read_text()),json.loads(dpath.read_text()))
    result['source_sha256']={str(p.relative_to(handoff)):hashlib.sha256(p.read_bytes()).hexdigest().upper() for p in (cpath,dpath)}
    result['exact_missing_call_targets']=calls
    # Supplemental targets whose consumers are already observed. A queue-only
    # bypass cannot explain invisibility at those consumer sites.
    checked=[]
    for call,target in [(0x2235DF4,0x2230608),(0x20E1A3C,0x20E198C),
                        (0x2237973,0x20E198C),(0x20E19BD,0x20E1864),(0x20E193E,0x1029E80),
                        (0x223798F,0x1C244E4),(0x1C245D3,0xE39D40),(0x1C2464C,0x4BC874),
                        (0x4BC9DB,0x4BD5F8),(0x4BCA4F,0x4BD5F8),(0x4BCA34,0x4BBBB8)]:
        row=instructions[call];assert rel32_call_target(call,row['raw'])==target
        checked.append({'call_site':hex(call),'target':hex(target),'bytes':row['raw'].hex().upper()})
    result['other_verified_direct_calls']=checked
    return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--handoff',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();result=analyze(a.handoff)
    with a.output.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
