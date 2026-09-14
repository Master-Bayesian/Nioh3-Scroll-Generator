"""Independent GNU objdump check of supplied byte columns; no AOB uniqueness claim."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from analyze_mode_augmentation import parse_asm
from dump_augmentation_function_bodies import REQUESTS, rel32_call_target
FILES=('01C244E4.asm','004BC874.asm','022377B8.asm','02235DA4.asm',
       '02230590.asm','020E1A04.asm','01BF2CF8.asm','004BBBB8.asm')
PATTERN=re.compile(r'^\s*([0-9a-f]+):\s+((?:[0-9a-f]{2}\s+)+)(\S+)(.*)$')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--handoff',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    tool=shutil.which('objdump')
    if not tool:raise SystemExit('GNU objdump is required; no fabricated fallback')
    a.output.mkdir(parents=True,exist_ok=False)
    allrows={};allbytes={};reports=[]
    for name in FILES:
        path=a.handoff/'evidence/prior-mode-upstream-static/disassembly'/name
        rows,data=parse_asm(path);start=min(data);end=max(data)+1
        if len(data)!=end-start:raise ValueError('noncontiguous excerpt; not filling gaps with zeros')
        raw=bytes(data[i] for i in range(start,end))
        binary=a.output/(name+'.bin');binary.write_bytes(raw)
        result=subprocess.run([tool,'-D','-b','binary','-m','i386:x86-64','-M','intel',
            '--insn-width=16',f'--adjust-vma={start}',str(binary)],capture_output=True,text=True,check=True)
        (a.output/(name+'.objdump.txt')).write_text(result.stdout,encoding='utf-8')
        decoded={}
        for line in result.stdout.splitlines():
            m=PATTERN.match(line)
            if m:decoded[int(m[1],16)]={'raw':bytes.fromhex(m[2]),'mnemonic':m[3],'operands':m[4].strip()}
        # Compare instruction START and byte length, not cosmetic mnemonic aliases.
        if set(decoded)!=set(rows) or any(decoded[r]['raw']!=v['raw'] for r,v in rows.items()):
            raise ValueError(f'independent instruction boundary mismatch: {name}')
        allrows.update(decoded);allbytes.update(data)
        reports.append({'source':str(path.relative_to(a.handoff)), 'start_rva':hex(start),'end_rva':hex(end),
            'instructions':len(rows),'byte_count':len(raw),'raw_sha256':hashlib.sha256(raw).hexdigest().upper()})
    loc=json.loads((ROOT/'research/possessed_enemy_capture/materialization_frontier_v201_locators.json').read_text())
    verified=[]
    for name,s in loc['sites'].items():
        r=int(s['rva'],16);raw=bytes.fromhex(s['bytes'])
        assert len(raw)==s['length'] and r in allrows
        assert bytes(allbytes[r+i] for i in range(len(raw)))==raw
        verified.append({'site':name,'rva':hex(r),'signature_bytes':len(raw),'independent_boundary':True})
    for call,sig,target,purpose in REQUESTS:
        row=allrows[call]
        assert row['mnemonic']=='call' and row['raw']==bytes.fromhex(sig)
        assert rel32_call_target(call,row['raw'])==target
    result={'tool_version':subprocess.run([tool,'--version'],capture_output=True,text=True,check=True).stdout.splitlines()[0],
            'supplied_excerpts':reports,'verified_observer_sites':verified,'verified_missing_call_edges':len(REQUESTS),
            'whole_module_uniqueness_tested':False,'live_game_executed':False,
            'scope':'independent re-disassembly of original handoff bytes, not missing function bodies'}
    (a.output/'CHECKS.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
