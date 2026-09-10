"""Copy and decrypt a normally saved game; never write the source save."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.savegame import SaveCrypto, default_crypto_tool, SCROLL_GROUP_OFFSET
from research.capture_live_scroll_inventory_readonly import capture
from research.capture_scroll_serial_index import capture as capture_index
from research.verify_single_native_insertion import verify_persistence, inventory_entries, index_entries


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--save', type=Path, required=True)
    parser.add_argument('--operation', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False, parents=True)
    original = args.save.read_bytes()
    copy = args.output / 'saved-copy.bin'
    copy.write_bytes(original)
    if args.save.read_bytes() != original:
        raise ValueError('Save changed during copying; capture again in a new directory')
    decrypted = args.output / 'saved-decrypted.bin'
    SaveCrypto(default_crypto_tool(ROOT)).decrypt(copy, decrypted)
    data = decrypted.read_bytes()
    records = [data[SCROLL_GROUP_OFFSET + i*0xE8:SCROLL_GROUP_OFFSET + (i+1)*0xE8] for i in range(400)]
    before_reload = json.loads((args.operation / 'inventory-after.json').read_text())
    saved = verify_persistence(before_reload, records, allow_new_marker_clear=True)
    current, index = capture(), capture_index()
    reloaded = verify_persistence(current, records)
    mapping = index_entries(index)
    if current['pid'] != index['pid'] or any(mapping.get(key) != item['slot_index'] for key, item in inventory_entries(current).items()):
        raise ValueError('Current native index differs from saved records')
    if args.save.read_bytes() != original:
        raise ValueError('Source save changed during verification')
    result = {'schema': 'nioh3-live-add-persistence/v1', 'saved': saved, 'loaded_inventory': reloaded,
              'source_sha256': hashlib.sha256(original).hexdigest(), 'source_save_unchanged': True,
              'native_index_matches': True,
              'limits': ['A reload must be independently observed or confirmed by the user; this tool cannot prove its occurrence.']}
    for name, value in [('inventory.json', current), ('index.json', index), ('verification.json', result)]:
        (args.output / name).write_text(json.dumps(value, indent=2), encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
