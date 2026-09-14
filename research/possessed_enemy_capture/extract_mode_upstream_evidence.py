"""Derive the scoped task comparison from the supplied raw captures, without a game.

This is a capture decoder, NOT an unknown-seed generator. Paths are relative to
this handoff. Preserve provenance and distinguish inferred wave joins.
"""
from __future__ import annotations
import argparse, hashlib, json, struct
from pathlib import Path

def visit(value, path=''):
    if isinstance(value,dict):
        if 'record_raw_hex' in value:yield path,value
        for k,v in value.items():yield from visit(v,path+'/'+str(k))
    elif isinstance(value,list):
        for i,v in enumerate(value):yield from visit(v,path+'/'+str(i))

def extract(root: Path) -> dict:
    paths=['evidence/controls/86872488/20260912-sp-negative-b/late-mask.json',
           'evidence/controls/86872488/20260912-one-person-expedition-a/late-mask.json',
           'evidence/controls/86872488/20260913-one-person-expedition-b/late-mask.json']
    out={'schema':'nioh3.mode-upstream.existing-evidence.v1','seed':86872488,'controls':[],
         'source':'Provided captures only; not new native execution',
         'physical_actor_join':False}
    for path in paths:
        blob=(root/path).read_bytes();data=json.loads(blob);records={}
        for pointer,item in visit(data):
            raw=bytes.fromhex(item['record_raw_hex'])
            if len(raw)<0xEB:continue
            spawn,mission,lookup=struct.unpack_from('<III',raw,0x20)
            # Reject incidental other task objects. The constructor identity is
            # independently checked by its embedded descriptor, not by a filename.
            if not (mission==0xCC96 and 0xF3C<=spawn<0xF4C and raw[0x94]==0xD4):continue
            if struct.unpack_from('<I',raw,0x80)[0]!=spawn or struct.unpack_from('<I',raw,0x84)[0]!=lookup:
                raise ValueError('task/embedded descriptor identity differs')
            rec={'spawn':spawn,'lookup':lookup,'terrain':raw[0x94],'position_key':raw[0x8E],
                 'selector_class':raw[0x90],'source_flag':raw[0x8F],
                 'descriptor_hex':raw[0x80:0x94].hex().upper(),'json_pointer':pointer}
            prior=records.get(spawn)
            if prior and any(prior[k]!=rec[k] for k in rec if k!='json_pointer'):
                raise ValueError('multiple different descriptor states; do not silently pick one')
            records[spawn]=rec
        out['controls'].append({'path':path,'sha256':hashlib.sha256(blob).hexdigest().upper(),
                                'tasks':sorted(records.values(),key=lambda r:r['spawn'])})
    path='evidence/runtime/assignment-origin-run-d/assignment-origin.json'
    blob=(root/path).read_bytes();events=json.loads(blob)['bridge_result']['events']
    entry=next(e for e in events if e['site']=='origin_entry')
    linked=[e for e in events if e['site']=='task_linked']
    trials=[e for e in events if e['site']=='trial_decision']
    state=entry['parent_rng']['state']
    for e in trials:
        state=(state*69069+1)&0xFFFFFFFF
        if state!=e['parent_rng']['state']:raise ValueError('parent-frame RNG state recurrence mismatch')
    out['run_d']={'path':path,'sha256':hashlib.sha256(blob).hexdigest().upper(),
                  'request_hex':entry['global_request_hex'],'context':entry['generator_context_row'],
                  'parent_rng_entry':entry['parent_rng']['state'],'parent_rng_final':state,
                  'diagnostic_rng_is_not_consumed_stream':True,
                  'trials':[{'sequence':e['sequence'],'selector':e['selector'],
                             'spawn':e['descriptor']['spawn'],'wave_index':e['descriptor']['wave_index'],
                             'position':e['descriptor']['position'],'state':e['parent_rng']['state'],
                             'ticket':e['ticket'],'threshold':e['threshold'],'success':e['branch_will_set']}
                             for e in trials],
                  'tasks':[{'spawn':e['source']['spawn'],'lookup':e['source']['lookup'],
                            'wave_index':e['source']['wave_index'],'position':e['source']['position'],
                            'position_key':bytes.fromhex(e['descriptor_hex'])[0xE],
                            'selector_class':e['source']['selector_class'],'source_flag':e['flag8f'],
                            'descriptor_hex':e['descriptor_hex'],'sequence':e['sequence']}
                           for e in linked]}
    return out

def main():
    a=argparse.ArgumentParser(description=__doc__);a.add_argument('handoff',type=Path);a.add_argument('output',type=Path)
    args=a.parse_args();result=extract(args.handoff);args.output.parent.mkdir(parents=True,exist_ok=True)
    with args.output.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)

if __name__=='__main__':main()
