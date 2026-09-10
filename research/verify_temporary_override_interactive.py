"""Bounded, operator-driven temporary-rule acceptance with owned cleanup.

This script never adds inventory or edits a save. Commands are status/stop/exit.
Use only after the operator has selected the explicitly supplied seed.
"""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.runtime_application import RuntimeApplication
from nioh3_scroll_editor.live_inventory import capture_inventory

parser = argparse.ArgumentParser()
parser.add_argument('--seed', type=int, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
application = RuntimeApplication()
events = []

def report(event):
    events.append(event)
    (args.output / 'events.json').write_text(json.dumps(events, indent=2), encoding='utf-8')
    print(json.dumps(event), flush=True)

before = capture_inventory()
(args.output / 'before.json').write_text(json.dumps(before, indent=2), encoding='utf-8')
matches = [entry for entry in before['entries'] if entry['seed'] == args.seed]
if not matches:
    raise ValueError('Target seed is not in the current inventory')
try:
    report({'event': 'armed', 'seed': args.seed, 'matching_slots': [e['slot_index'] for e in matches],
            'profile': {'special_rule_keys': [0, 0, 0]},
            'runtime': application.start_override({'seed': args.seed, 'enemy_keys': [],
                       'special_rule_keys': [0, 0, 0], 'terrain_value': None})})
    for line in sys.stdin:
        command = line.strip()
        if command == 'status':
            report({'event': command, 'runtime': application.status()})
        elif command == 'stop':
            report({'event': command, 'runtime': application.stop_override()})
        elif command == 'exit':
            break
        else:
            report({'event': 'invalid_command'})
finally:
    report({'event': 'cleanup', 'runtime': application.stop_override()})
    after = capture_inventory()
    (args.output / 'after.json').write_text(json.dumps(after, indent=2), encoding='utf-8')
    report({'event': 'inventory_comparison',
            'unchanged': before['container_sha256'] == after['container_sha256'],
            'before_count': len(before['entries']), 'after_count': len(after['entries'])})
