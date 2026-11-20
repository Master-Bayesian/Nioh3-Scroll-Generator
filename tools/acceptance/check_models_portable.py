"""Run the repository's real TLA+ models and named fault controls on any host.

The caller must supply the exact SHA256 of its independently obtained jar. This
does not change or claim to satisfy tools/check_models.ps1's separate pin.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--jar-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--model', action='append', default=[])
    parser.add_argument('--timeout', type=int, default=45)
    parser.add_argument('--workers', type=int, default=2)
    args = parser.parse_args()
    jar = args.jar.resolve()
    actual = hashlib.sha256(jar.read_bytes()).hexdigest()
    if actual != args.jar_sha256.lower():
        parser.error(f'jar hash mismatch: actual {actual}')
    root = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    specs = sorted((root / 'spec').rglob('*.tla'))
    if set(args.model) - {p.stem for p in specs}:
        parser.error('unknown model requested')
    tasks = []
    for spec in specs:
        if args.model and spec.stem not in args.model:
            continue
        config = spec.with_suffix('.cfg').read_text()
        tasks.append((spec, 'correct', config, None))
        for control in json.loads(spec.with_suffix('.negative.json').read_text()):
            faulty = config
            for key, value in control['overrides'].items():
                if isinstance(value, bool):
                    pattern = rf'(\b{re.escape(key)}\s*=\s*)(TRUE|FALSE)\b'
                    replacement = 'TRUE' if value else 'FALSE'
                elif isinstance(value, str) and re.fullmatch('[A-Za-z][A-Za-z0-9]*', value):
                    pattern = rf'(\b{re.escape(key)}\s*=\s*)"None"'
                    replacement = json.dumps(value)
                else:
                    raise ValueError(f'unsupported fault control {key}')
                faulty, count = re.subn(pattern, lambda m: m[1] + replacement, faulty)
                if count != 1:
                    raise ValueError(f'expected one fault switch for {spec.name}/{key}')
            tasks.append((spec, control['name'], faulty, control['expected']))

    def run(task):
        spec, name, config, expected = task
        case = output / f'{spec.stem}-{name}'
        case.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(spec, case / spec.name)
        (case / f'{spec.stem}.cfg').write_text(config)
        command = ['java', '-XX:+UseParallelGC', '-cp', str(jar), 'tlc2.TLC',
                   '-metadir', str(case / 'states'), '-cleanup', '-config',
                   f'{spec.stem}.cfg', spec.name]
        try:
            result = subprocess.run(command, cwd=case, text=True, capture_output=True, timeout=args.timeout)
            text = result.stdout + result.stderr
            code = result.returncode
        except subprocess.TimeoutExpired as error:
            text = str(error)
            code = -1
        (case / 'tlc.log').write_text(text)
        passed = (code == 0 and 'No error has been found.' in text) if expected is None else (
            code == 12 and re.search(r'^Error: ' + re.escape(expected) + r'\.?\s*$', text, re.M) is not None)
        states = re.findall(r'([\d,]+) states generated, ([\d,]+) distinct states found', text)
        return {'model': spec.stem, 'case': name, 'expected': expected, 'exit_code': code,
                'pass': passed, 'states': states[-1] if states else None,
                'log': f'{case.name}/tlc.log', 'command': command,
                'model_sha256': hashlib.sha256(spec.read_bytes()).hexdigest()}

    with ThreadPoolExecutor(max_workers=max(1, min(args.workers, 4))) as pool:
        results = list(pool.map(run, tasks))
    report = {'schema': 'portable-model-controls/v1', 'jar_sha256': actual,
              'scope': 'Original models and fault switches; does not prove implementation refinement',
              'pass': all(r['pass'] for r in results), 'cases': results}
    (output / 'model-controls.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'pass': report['pass'], 'cases': len(results),
                      'models': len({r['model'] for r in results}),
                      'failed': [r for r in results if not r['pass']]}))
    return 0 if report['pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
