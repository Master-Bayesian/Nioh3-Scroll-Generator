"""Research CLI over shared application verification/inspection."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.live_add_evidence import verify, verify_persistence, inventory_entries, index_entries, defined, record

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    names = ('insertion-plan.json', 'execution.json', 'inventory-before.json',
             'inventory-after.json', 'index-before.json', 'index-after.json')
    raw = [(args.directory / name).read_bytes() for name in names]
    result = verify(*(json.loads(value) for value in raw))
    result['sources'] = dict(zip(names, (hashlib.sha256(value).hexdigest() for value in raw)))
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps(result))
