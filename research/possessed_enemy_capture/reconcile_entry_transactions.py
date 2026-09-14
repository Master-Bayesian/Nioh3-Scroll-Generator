"""Recompute the same-process fork from raw events, not existing analysis JSON."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
from entry_transaction_evidence import load, reconcile, dump_exclusive

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('evidence_root',type=Path,help='The supplied handoff evidence/ directory')
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    names=['prior-sequence-c/mode-upstream-sequence.json',
           'prior-sequence-c/mode-upstream-sequence.cleanup.json',
           'current-frontier/materialization-frontier.json',
           'current-frontier/materialization-frontier.cleanup.json']
    result=reconcile(*(load(a.evidence_root/n) for n in names))
    result['sources']=[{'path':'evidence/'+n,'sha256':hashlib.sha256((a.evidence_root/n).read_bytes()).hexdigest()} for n in names]
    dump_exclusive(a.output,result)
    print(json.dumps({k:result[k] for k in ('same_process','same_invocation','frontier_materialization_fork_closed','next_unresolved_edge')},indent=2))
if __name__=='__main__':main()
