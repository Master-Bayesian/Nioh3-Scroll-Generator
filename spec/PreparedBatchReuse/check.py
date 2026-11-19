#!/usr/bin/env python3
"""Run bounded protocol controls. This is not a code refinement proof."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--jar', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    here = pathlib.Path(__file__).resolve().parent
    jar = pathlib.Path(args.jar).resolve()
    output = pathlib.Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    expected = {
        'None': None,
        'ReuseInFlight': 'NoReuseBeforeJoin',
        'DestroyInFlight': 'BufferLivesUntilJoin',
        'PublishCancelled': 'NoCancelledCommit',
        'PublishFailed': 'NoFailedCommit',
        'CacheIncomplete': 'CachedAnswersExact',
        'HashOnly': 'CompletedAnswersExact',
    }
    receipt = {
        'scope': 'bounded abstract protocol model; not a proof of Rust/C++ refinement or CUDA behavior',
        'bounds': {'batches':2, 'packets':3, 'maximum_request_rows':3},
        'jar_source': 'https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar',
        'jar_sha256': hashlib.sha256(jar.read_bytes()).hexdigest(),
        'model_sha256': hashlib.sha256((here/'PreparedBatchReuse.tla').read_bytes()).hexdigest(),
        'passed':True, 'cases':[],
    }
    for fault, invariant in expected.items():
        config = 'PreparedBatchReuse.cfg' if fault == 'None' else f'Fault{fault}.cfg'
        case_dir = output / fault
        case_dir.mkdir(exist_ok=True)
        shutil.copy2(here / 'PreparedBatchReuse.tla', case_dir)
        shutil.copy2(here / config, case_dir)
        command = ['java','-XX:+UseParallelGC','-cp',str(jar),'tlc2.TLC','-workers','1',
                   '-seed','1','-fp','0','-config',config,'-metadir',str(case_dir / 'states'),
                   'PreparedBatchReuse.tla']
        run = subprocess.run(command,cwd=case_dir,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
        (output/f'{fault}.log').write_text(run.stdout)
        error = re.search(r'Error: Invariant ([A-Za-z0-9_]+) is violated',run.stdout)
        observed = error.group(1) if error else None
        passed = ((run.returncode == 0 and 'No error has been found' in run.stdout) if invariant is None
                  else (run.returncode != 0 and observed == invariant))
        states = re.search(r'(\d+) states generated, (\d+) distinct states found',run.stdout)
        receipt['passed'] &= passed
        receipt['cases'].append({'fault':fault,'expected_invariant':invariant,'observed_invariant':observed,
            'exit_code':run.returncode,'passed':passed,'states_generated':int(states[1]) if states else None,
            'states_distinct':int(states[2]) if states else None,'log':f'{fault}.log','command':command})
        print(f'{fault}: {"PASS" if passed else "FAIL"}; invariant={observed}',flush=True)
    (output/'TLA_PREPARED_BATCH.json').write_text(json.dumps(receipt,indent=2)+'\n')
    if not receipt['passed']:
        raise SystemExit('A protocol control did not behave as expected; inspect logs')


if __name__ == '__main__':
    main()
