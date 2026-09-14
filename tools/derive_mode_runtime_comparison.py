"""Read the two supplied captures without pretending they are one live entry.

No game process is opened. Does not fabricate outputs for the censored new run.
"""
from __future__ import annotations
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from validate_mode_upstream_capture import unwrap, validate


def derive(handoff: Path) -> dict:
    rel_new='evidence/live-mode-upstream/mode-upstream.json'
    rel_old='evidence/prior-run-d/assignment-origin.json'
    raw_new=json.loads((handoff/rel_new).read_text(encoding='utf-8'))
    cleanup=json.loads((handoff/'evidence/live-mode-upstream/mode-upstream.cleanup.json').read_text(encoding='utf-8'))
    raw_old=json.loads((handoff/rel_old).read_text(encoding='utf-8'))
    accepted=validate(raw_new,cleanup)
    new,old=unwrap(raw_new),unwrap(raw_old)
    ne=new['events'];oe=old['events'];origin=next(e for e in oe if e['site']=='origin_entry')
    q0=bytes.fromhex(ne[0]['request_hex']);q1=bytes.fromhex(origin['global_request_hex'])
    ds=origin['descriptors'];counts=Counter(d['wave_index'] for d in ds)
    # Cross-check parsed descriptor claims from native-record byte evidence.
    for d in ds:
        b=bytes.fromhex(d['raw_hex'])
        assert len(b)==0x14
        assert int.from_bytes(b[:4],'little')==d['spawn']
        assert int.from_bytes(b[4:8],'little')==d['lookup']
        assert b[0x10]==d['selector_class']
    return {
      'source_sha256':{rel:hashlib.sha256((handoff/rel).read_bytes()).hexdigest() for rel in (rel_new,rel_old)},
      'legacy_new_chain_validation':accepted,
      'new':{'run_id':new['run_id'],'pid':new['pid'],'creation_filetime':new['identity']['creation_filetime'],
             'request_hex':q0.hex().upper(),'sites':[e['site'] for e in ne],
             'stop_reason':new['stop_reason'],'context':ne[-1]['context'],
             'actual_extra_frame':ne[-1]['extra_generation'],'completed_descriptor_count':None,
             'native_task_count_for_this_entry':None,
             'thread_provenance':[e['thread_id_source'] for e in ne]},
      'prior_d':{'run_id':old['run_id'],'pid':old['pid'],'creation_filetime':old['identity']['creation_filetime'],
                 'request_hex':q1.hex().upper(),'event_site_counts':dict(Counter(e['site'] for e in oe)),
                 'descriptors_in_origin':len(ds),'descriptors_per_wave':[counts[i] for i in sorted(counts)],
                 'class0_count':sum(d['selector_class']==0 for d in ds),'class1_count':sum(d['selector_class']==1 for d in ds),
                 'producer_of_this_request':None},
      'request_differences':[{'offset':hex(i),'new':a,'prior_d':b} for i,(a,b) in enumerate(zip(q0,q1)) if a!=b],
      'identical_context_row_bytes':ne[-1]['context']['raw_hex']==origin['generator_context_row']['raw_hex'],
      'different_process_instances':(new['pid'],new['identity']['creation_filetime'])!=(old['pid'],old['identity']['creation_filetime']),
      'cross_run_sequential_invocation_join':False,
      'boundary':'Prior D is an independent run, not the uncaptured suffix of the new entry.'}


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--handoff',type=Path,required=True)
    args=ap.parse_args();print(json.dumps(derive(args.handoff),indent=2))

if __name__=='__main__':main()
