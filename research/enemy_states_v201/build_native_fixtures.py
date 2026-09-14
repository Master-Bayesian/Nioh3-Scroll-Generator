"""Extract expectations from supplied raw captures, never from the new oracle."""
from pathlib import Path
import argparse, hashlib, json, struct


def build(root):
    provenance=[]
    def load(path):
        p=root/path;b=p.read_bytes();provenance.append(dict(path=path,sha256=hashlib.sha256(b).hexdigest()))
        return json.loads(b)
    controls=[]
    for name,variant in [('evidence/current-live/mode-transaction-join.json','expedition'),
                          ('evidence/prior-sequence-c/mode-upstream-sequence.json','solo')]:
        d=load(name);event=next(e for e in d['bridge_result']['events'] if e['site']=='mission_generated')
        waves=[]
        for wave in event['output']['waves']:
            items=[]
            for desc in wave['descriptors']:
                raw=bytes.fromhex(desc['raw_hex']);spawn,lookup,role=struct.unpack_from('<III',raw)
                items.append(dict(spawn=spawn,lookup=lookup,role=role,selector=raw[16],flag=raw[15],raw_hex=raw.hex()))
            waves.append(items)
        controls.append(dict(seed=86872488,playthrough=3,variant=variant,waves=waves,
                             source=name,event_sequence=event['sequence']))
    # These pre-existing sanitized controls are not used as generation tables.
    old=load('evidence/assignment-origin-pro-return/source/tests/fixtures/assignment_origin_controls.json')
    groups=[]
    for c in old:
        groups.append(dict(seed=c['seed'],run=c['run'],source=c['source'],source_sha256=c['source_sha256'],
                           records=[dict(spawn=x['spawn'],lookup=x['lookup'],flag8f=x['flag8f'],e9=x['e9'],
                                         assigned_index=x['assigned_index'],raw_hex=x['descriptor_hex']) for x in c['records']]))
    rd=load('evidence/prior-run-d/assignment-origin.json')['bridge_result']['events']
    trials=[dict(spawn=e['descriptor']['spawn'],ticket=e['ticket'],threshold=e['threshold'],
                 state=e['parent_rng']['state'],selector=e['selector'],accepted=bool(e['branch_will_set']))
            for e in rd if e['site']=='trial_decision']
    return dict(rosters=controls,late_controls=groups,run_d_source_entry_state=rd[0]['parent_rng']['state'],
                run_d_trials=trials,provenance=provenance)

if __name__=='__main__':
    a=argparse.ArgumentParser();a.add_argument('handoff',type=Path);a.add_argument('output',type=Path);v=a.parse_args()
    v.output.write_text(json.dumps(build(v.handoff),indent=2)+'\n')
