"""Capture exact localized qualifier names by native item/text IDs, read-only."""
import argparse
from dataclasses import asdict
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT), str(ROOT / 'research')]
from dump_effect_catalog_current_locale import (
    ProcessReader, _candidate_regions, _best_anchor_cluster,
    scan_range_for_anchor_entries, parse_text_entry, TEXT_ID_ANCHORS,
)
from nioh3_scroll_editor.runtime_application import running_game_identity


def capture(locale, center_hint=None, scan_budget_mb=8192):
    pid, profile, _ = running_game_identity()
    if profile.display_version != 'PC v2.01':
        raise ValueError('This item layout requires verified PC v2.01')
    catalog = json.loads((ROOT / 'nioh3_scroll_editor/data/effect_names_multilingual.json').read_text(encoding='utf-8'))
    expected = {text_id: catalog['effects'][f'0x{effect_id:08X}']['names'][locale]
                for effect_id, text_id in TEXT_ID_ANCHORS.items()}
    item_keys = set(json.loads((ROOT / 'nioh3_scroll_editor/data/special_rule_item_names.json').read_text(encoding='utf-8'))['items'])
    with ProcessReader() as reader:
        if reader.pid != pid:
            raise RuntimeError('Game changed during identity verification')
        for rva, signature in profile.native_signatures:
            if reader.read(reader.module_base + rva, len(signature)) != signature:
                raise RuntimeError('Native runtime signature mismatch')
        manager = reader.u64(reader.module_base + 0x45B5DF0)
        if not manager:
            raise RuntimeError('Parameter manager is not loaded')
        context = reader.u64(manager + 0x68)
        store = reader.u64(context)
        count = reader.u32(store + 4)
        if not 1 <= count <= 10000:
            raise RuntimeError('Invalid item row count')
        rows = reader.read(store + 8, count * 0x1A0)
        text_ids = {}
        for index in range(count):
            row = rows[index * 0x1A0:(index + 1) * 0x1A0]
            key = f'0x{struct.unpack_from("<H", row, 0x152)[0]:04X}'
            if key in item_keys:
                if key in text_ids:
                    raise RuntimeError(f'Ambiguous native item key: {key}')
                text_ids[key] = struct.unpack_from('<I', row, 0x68)[0]
        if set(text_ids) != item_keys:
            raise RuntimeError('Native item table does not cover every qualifier')
        regions = _candidate_regions(reader)
        windows = []
        if center_hint is not None:
            for region in regions:
                if region.base <= center_hint < region.base + region.size:
                    windows.append((region, max(region.base, center_hint - 2097152), min(region.base + region.size, center_hint + 2097152)))
        windows += [(r, r.base, r.base + r.size) for r in regions if r.memory_type == 0x20000 and r.size >= 1048576]
        remaining = scan_budget_mb * 1048576
        found = None
        for region, cursor, end in windows:
            anchors = []
            while cursor < end and remaining > 0:
                size = min(64 * 1048576, end - cursor, remaining)
                entries = scan_range_for_anchor_entries(reader, start=cursor, end=cursor + size, anchor_ids=set(expected))
                anchors.extend(e for e in entries if e.text == expected[e.text_id])
                cluster = _best_anchor_cluster(anchors, radius=2097152)
                if cluster and len(cluster[1]) >= 2:
                    found = region, cluster[0], anchors
                    break
                cursor += size
                remaining -= size
            if found:
                break
        if not found:
            raise RuntimeError(f'No validated {locale} text pool in the bounded read budget; load the requested language')
        region, center, anchors = found
        start, end = max(region.base, center - 16 * 1048576), min(region.base + region.size, center + 16 * 1048576)
        block = reader.read(start, end - start)
        result = {}
        for key, text_id in text_ids.items():
            cursor, values = 0, []
            while True:
                offset = block.find(struct.pack('<I', text_id), cursor)
                if offset < 0:
                    break
                cursor = offset + 1
                entry = parse_text_entry(reader, start + offset, expected_id=text_id)
                if entry:
                    values.append(entry)
            names = {entry.text for entry in values}
            if len(names) != 1:
                raise RuntimeError(f'Missing or mixed-language item text for {key}: {sorted(names)!r}')
            result[key] = {'text_id': text_id, 'name': names.pop(), 'addresses': [entry.address for entry in values]}
        if reader.u64(reader.module_base + 0x45B5DF0) != manager or reader.u64(manager + 0x68) != context or reader.u64(context) != store:
            raise RuntimeError('Parameter table owner changed during capture')
        return {'schema': 'nioh3-native-item-localization/v1', 'captured_at_utc': datetime.now(timezone.utc).isoformat(),
                'pid': pid, 'game_version': profile.display_version, 'locale': locale, 'read_only': True,
                'item_row_size': 0x1A0, 'item_key_offset': 0x152, 'text_id_offset': 0x68,
                'pool_center': center, 'anchors': [asdict(entry) for entry in anchors], 'items': result}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--locale', choices=['zh-CN', 'ja-JP', 'en-US'], required=True)
    parser.add_argument('--center-hint', type=lambda value: int(value, 0))
    parser.add_argument('--scan-budget-mb', type=int, default=8192, choices=range(1, 16385))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    value = capture(args.locale, args.center_hint, args.scan_budget_mb)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write('\n')
    print(json.dumps({'locale': value['locale'], 'resolved_items': len(value['items']), 'pid': value['pid']}))
