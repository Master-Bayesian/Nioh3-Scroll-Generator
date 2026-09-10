"""Separate early-playthrough effects from the title-screen rarity cap."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.runtime_application import running_game_identity
from nioh3_scroll_editor.native import NativeBatchOracle, build_source_record
from nioh3_scroll_editor.savegame import SaveInstaller, SaveCrypto, default_crypto_tool, discover_save_paths
from research.validate_ng3_rarity3_native_parity_live import deterministic_natural_seeds


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--count', type=int, default=4096)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--title-screen-confirmed', action='store_true', required=True)
    args = parser.parse_args()
    if args.output.exists() or not 1 <= args.count <= 100000:
        raise ValueError('Use a new output and a bounded count')
    pid, profile, executable = running_game_identity()
    paths = discover_save_paths()
    if len(paths) != 1: raise ValueError('Ambiguous save context')
    siblings = list(paths[0].parent.glob('*.BIN')) + list((paths[0].parent.parent / 'SYSTEMSAVEDATA00').glob('*.BIN'))
    before = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in siblings}
    inventory = SaveInstaller(save_path=paths[0], crypto=SaveCrypto(default_crypto_tool(ROOT)), state_root=args.output.parent / 'state').capture_inventory()
    seeds = deterministic_natural_seeds(args.count, 20260909)
    report = {'schema': 'nioh3-r5-early-context/v1', 'samples_per_cell': len(seeds),
              'game_version': profile.display_version, 'executable_sha256': hashlib.sha256(Path(executable).read_bytes()).hexdigest(),
              'scope': 'Isolated construction with cleared effect metadata; preserve mode is a temporary existing native capability override, not natural-drop evidence',
              'cells': [], 'save_files_unchanged': None}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    reference = {}
    for preserve in (False, True):
        oracle = NativeBatchOracle(pid=pid, runtime_profile=profile, max_batch_size=128, preserve_requested_rarity=preserve)
        try:
            with oracle:
                for playthrough in (1, 2, 3):
                    template = bytearray(inventory.template_record_for_playthrough(playthrough))
                    for slot in range(7): template[0x42 + slot * 24] = 0
                    for rarity in (4, 5):
                        headers, counts, last_ids = Counter(), Counter(), Counter()
                        different = 0
                        for start in range(0, len(seeds), 128):
                            selected = seeds[start:start + 128]
                            records = oracle.generate([build_source_record(bytes(template), seed=seed, rarity=rarity, level=180, recommended_level=183) for seed in selected])
                            for seed, record in zip(selected, records, strict=True):
                                slots = record[0x34:0xDC]
                                ids = [struct.unpack_from('<I', record, 0x38 + i * 24)[0] for i in range(7)]
                                headers[f'{record[0x30]}/{record[0x31]}'] += 1
                                counts[str(sum(value not in (0, 1, 0xFFFFFFFF) for value in ids))] += 1
                                last_ids[f'{ids[5]:08X}'] += 1
                                key = preserve, playthrough, seed
                                if rarity == 4: reference[key] = slots
                                else: different += slots != reference[key]
                        row = {'preserve_requested_rarity': preserve, 'playthrough': playthrough, 'requested_rarity': rarity,
                               'headers': dict(headers), 'populated_effect_counts': dict(counts), 'slot6_ids': dict(last_ids),
                               'effect_bytes_different_from_r4': different if rarity == 5 else None}
                        report['cells'].append(row)
                        print({k:v for k,v in row.items() if k != 'slot6_ids'}, flush=True)
                        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
        finally:
            while oracle.remote_call_pending: time.sleep(1)
    report['save_files_unchanged'] = all(path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == before[str(path)] for path in siblings)
    args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    if not report['save_files_unchanged']: raise RuntimeError('Save identity changed during experiment')


if __name__ == '__main__': main()
