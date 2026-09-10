"""Bounded eight-cell live installation acceptance through product services.

Run only after normal saving with the character idle. Each insertion receives
its own verified backup. A failed or uncertain operation stops the matrix.
No retries occur after dispatch; reruns require a new directory and new seeds.
"""
import argparse
import json
from pathlib import Path
import sys
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.save_application import SaveApplication
from nioh3_scroll_editor.runtime_application import RuntimeApplication
from nioh3_scroll_editor.live_add_application import LiveAddApplication
from nioh3_scroll_editor.live_add_native_adapter import NativeLiveAddAdapter
from nioh3_scroll_editor.live_inventory import capture_inventory

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--resume-verified', action='store_true')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=args.resume_verified)
save = SaveApplication(args.output / 'save-workspace')
references = save.discover()['saves']
if len(references) != 1:
    raise ValueError('An explicit save selection is required')
reference = references[0]
inventory = save.inventory(reference['save_id'])
runtime = RuntimeApplication(service=save.service)
application = LiveAddApplication(args.output, save.service.context.context_digest,
    adapter=NativeLiveAddAdapter(args.output / 'executor'))
cells = []
baseline = capture_inventory()
(args.output / 'inventory-before.json').write_text(json.dumps(baseline, indent=2), encoding='utf-8')
existing = {entry['seed'] for entry in baseline['entries']}
results = json.loads((args.output / 'verification.json').read_text()) if args.resume_verified else []
for prior in results:
    if application.status(prior['operation']['operation_id'])['state'] != 'verified':
        raise ValueError('Only verified operations can be skipped on resume')
completed_seeds = {item['seed'] for item in results}
for playthrough, rarity in [(p, r) for p in (1, 2, 3) for r in (3, 4, 5) if (p, r) != (3, 3)]:
    seed = 10031000 + playthrough * 10 + rarity
    if seed in completed_seeds:
        continue
    if seed in existing:
        raise ValueError('Matrix seed already exists; do not replay')
    template = save.template(reference['save_id'], inventory['snapshot_id'], playthrough)
    result = runtime.generate(template, seed, playthrough, rarity, 170, 585,
                              threading.Event(), lambda update: None)
    if result['candidate'] is None or len(runtime.candidates) != 1:
        raise ValueError('Native generation did not produce one ready candidate')
    payload = next(iter(runtime.candidates.values()))
    materialized = save.materialize_live_many(reference['save_id'], inventory['snapshot_id'],
        [payload], 585, 0xFFFFFFFF)['candidates'][0]
    cells.append({'playthrough': playthrough, 'rarity': rarity, 'seed': seed,
                  'candidate': materialized})
    print(json.dumps({'generated': seed, 'playthrough': playthrough, 'rarity': rarity}), flush=True)
    observed = capture_inventory()
    (args.output / f'inventory-after-generation-{playthrough}-{rarity}.json').write_text(json.dumps(observed, indent=2), encoding='utf-8')
    differences = [field for field in ('pid', 'entries', 'serial_counter', 'acquisition_order_counter', 'container_sha256') if baseline[field] != observed[field]]
    if differences:
        raise ValueError('Isolated generation changed: ' + ', '.join(differences))
after_generation = capture_inventory()
for field in ('pid', 'entries', 'serial_counter', 'acquisition_order_counter', 'container_sha256'):
    if baseline[field] != after_generation[field]:
        raise ValueError('Isolated candidate generation changed inventory')
(args.output / 'candidates.json').write_text(json.dumps(cells, indent=2), encoding='utf-8')
print('READY: enter execute to install the eight distinct cells once', flush=True)
if sys.stdin.readline().strip() != 'execute':
    raise SystemExit('No insertion requested')
previous = results[-1]['operation']['operation_id'] if results else None
for cell in cells:
    for attempt in range(20):
        try:
            prepared = application.prepare(cell['candidate'], reference['path'], previous_operation_id=previous)
            break
        except RuntimeError as error:
            if str(error) != 'Mission scheduler is not in the accepted idle phase' or attempt == 19:
                raise
            time.sleep(0.1)
    for attempt in range(20):
        try:
            receipt = application.execute(prepared['operation_id'], prepared['plan_digest'])
            break
        except RuntimeError as error:
            # This exact error is raised by inspection before the durable claim.
            if (str(error) != 'Mission scheduler is not in the accepted idle phase'
                    or application.status(prepared['operation_id'])['state'] != 'prepared' or attempt == 19):
                raise
            time.sleep(0.1)
    results.append({key: cell[key] for key in ('playthrough', 'rarity', 'seed')} | {'operation': receipt})
    (args.output / 'verification.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
    print(json.dumps(results[-1]), flush=True)
    if receipt['state'] != 'verified' or not application.safe_to_shutdown():
        raise RuntimeError('Matrix stopped: insertion was not verified and released')
    previous = prepared['operation_id']
print('MATRIX_VERIFIED', flush=True)
