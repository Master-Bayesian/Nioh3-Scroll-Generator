"""Compare the count port with raw v2.01 E3A900 in an isolated Linux process.

Only the 78C8F4 effect-presence call is replaced with a controlled boolean stub.
No game is opened. This is fragment differential testing, NOT live parity.
"""
from __future__ import annotations
import argparse, hashlib, json, platform, shutil, struct, subprocess, sys, tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]))
from nioh3_scroll_editor.enemy_state_rng import MT19937, f32
from nioh3_scroll_editor.curse_generation import selection_count

FRAGMENT_SHA256='d0505f5c6537147e7b66d2e5bfb239c241b8fcc04272d286e34b251d7be36481'


def verify() -> dict:
    if sys.platform!='linux' or platform.machine().lower() not in ('x86_64','amd64'):
        raise RuntimeError('raw fragment verification needs Linux x86-64; no game dependency')
    compiler=shutil.which('g++')
    if not compiler:raise RuntimeError('g++ not found')
    here=Path(__file__).resolve().parent;fragment=here/'e3a900.bin'
    if hashlib.sha256(fragment.read_bytes()).hexdigest()!=FRAGMENT_SHA256:
        raise RuntimeError('raw fragment hash mismatch; refusing to execute')
    scalars=[0,.0001,.01,.03,.05,.1,.2,.33333334,.49999997,.5,.75,.99999994,1,1.1,2,100]
    cases=[]
    for p in scalars:
        for n in [1,2,3,4,6,10,91,255]:
            for already in [0,6,8,9]:
                for bonus in [0,1]:
                    seed=(0x078E0001+len(cases)*2654435761)&0xFFFFFFFF
                    cases.append((seed,n,p,already,bonus,1))
    for seed,p in [(0,.03),(0x078E0001,.5),(0xFFFFFFFF,1)]:
        cases.append((seed,91,p,0,1,1500))
    payload=b''.join(struct.pack('<IIfIII',*c) for c in cases)
    with tempfile.TemporaryDirectory(prefix='nioh3-count-fragment-') as td:
        exe=Path(td)/'count_fragment'
        built=subprocess.run([compiler,'-std=c++17','-O2',str(here/'count_fragment.cpp'),'-o',str(exe)],capture_output=True,text=True,timeout=30)
        if built.returncode:raise RuntimeError('compile failed: '+built.stderr)
        native=subprocess.run([str(exe),str(fragment)],input=payload,capture_output=True,timeout=30)
        if native.returncode:raise RuntimeError(f'native fragment exited {native.returncode}: {native.stderr[:1000]!r}')
    if len(native.stdout)!=len(cases)*5004:raise RuntimeError('native output length mismatch')
    differences=[];calls=0
    for i,(seed,n,p,already,bonus,repeat) in enumerate(cases):
        mt=MT19937(seed)
        for _ in range(repeat):expected=selection_count(n,f32(p),already,mt,has_bonus_3b37=bool(bonus))
        calls+=repeat;buf=native.stdout[i*5004:(i+1)*5004]
        actual,index=struct.unpack_from('<iI',buf);words=struct.unpack_from('<1248I',buf,8)
        bank=(index-1)//624
        checks={'count':actual==expected,'cursor':index%624==mt.index%624,
                'active_state':bank in (0,1) and tuple(mt.words)==words[bank*624:(bank+1)*624]}
        if not all(checks.values()):differences.append({'case':i,'inputs':cases[i],'checks':checks,'actual':actual,'expected':expected,'native_index':index,'python_index':mt.index})
    return {'scope':'isolated raw mathematical instructions; no game, no full E667A0/native mission acceptance',
            'fragment_rva':'0xE3A900..0xE3AD89','fragment_bytes':1161,'sha256':FRAGMENT_SHA256,
            'modified_call_only':'0xE3AD62 -> 78C8F4 effect 3B37 presence stub',
            'cases':len(cases),'sequential_count_calls':calls,'mismatches':len(differences),
            'checks':['return value','MT cursor modulo624','all624 active state words'],
            'scalar_domain_tested':scalars,'differences':differences[:20]}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    d=verify();text=json.dumps(d,indent=2)+'\n';a.output.write_text(text);print(text)
    raise SystemExit(1 if d['mismatches'] else 0)
