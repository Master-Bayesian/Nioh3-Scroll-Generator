"""Prepare one automatically backed-up live addition; execute once on stdin.

Reuses the product candidate/materialization/plan/evidence implementation.
Never edits a save. Keep this process alive while a dispatch is unresolved.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.save_application import SaveApplication
from nioh3_scroll_editor.live_add_application import LiveAddApplication
from nioh3_scroll_editor.live_add_native_adapter import NativeLiveAddAdapter
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.models import ScrollCandidate
from nioh3_scroll_editor.candidate_transfer import export_candidate
from nioh3_scroll_editor.live_inventory import capture_inventory

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--seed', type=int, required=True)
parser.add_argument('--rarity', type=int, default=3)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
save = SaveApplication(args.output / 'save-workspace')
references = save.discover()['saves']
if len(references) != 1:
    raise ValueError('Choose an explicit save when multiple accounts or slots exist')
reference = references[0]
inventory = save.inventory(reference['save_id'])
before = capture_inventory()
if any(entry['seed'] == args.seed for entry in before['entries']):
    raise ValueError('Acceptance seed already exists; choose a distinguishable seed')
digest = save.service.context.context_digest
candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(args.seed, rarity=args.rarity, level=170))
payload = export_candidate(candidate, digest, 170)
materialized = save.materialize_live_many(reference['save_id'], inventory['snapshot_id'], [payload], 585, 0xFFFFFFFF)['candidates'][0]
adapter = NativeLiveAddAdapter(args.output / 'executor')
application = LiveAddApplication(args.output, digest, adapter=adapter)
for attempt in range(20):
    try:
        prepared = application.prepare(materialized, reference['path'])
        break
    except RuntimeError as error:
        if str(error) != 'Mission scheduler is not in the accepted idle phase' or attempt == 19:
            raise
        time.sleep(0.1)
(args.output / 'prepared.json').write_text(json.dumps(prepared, indent=2), encoding='utf-8')
print(json.dumps({'event': 'prepared', **prepared}), flush=True)
for line in sys.stdin:
    command = line.strip()
    if command == 'execute':
        receipt = application.execute(prepared['operation_id'], prepared['plan_digest'])
        source_hash = hashlib.sha256(Path(reference['path']).read_bytes()).hexdigest()
        plan = application.operations.plan(prepared['operation_id'])['plan']
        result = {'receipt': receipt, 'source_save_unchanged': source_hash == plan['source_save_sha256'],
                  'safe_to_shutdown': application.safe_to_shutdown()}
        (args.output / 'verification.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
        print(json.dumps(result), flush=True)
    elif command == 'status':
        print(json.dumps(application.status(prepared['operation_id'])), flush=True)
    elif command == 'exit':
        if not application.safe_to_shutdown():
            raise RuntimeError('Runtime ownership remains unresolved')
        break
