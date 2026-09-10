"""Title-screen native matrix in isolated buffers; never installs a record."""
from __future__ import annotations
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
from nioh3_scroll_editor.effect_sequence import generate_ng3_rarity3_effect_sequence, serialize_ng3_rarity3_effect_slots
from research.validate_ng3_rarity3_native_parity_live import deterministic_natural_seeds


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--count', type=int, default=4096)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--title-screen-confirmed', action='store_true', required=True)
    args = parser.parse_args()
    if not 1 <= args.count <= 100000 or args.output.exists():
        raise ValueError('Use a bounded count and a new output file')
    pid, profile, executable = running_game_identity()
    paths = discover_save_paths()
    if len(paths) != 1:
        raise ValueError('Select an unambiguous save context before running the matrix')
    save = paths[0]
    siblings = list(save.parent.glob('*.BIN')) + list((save.parent.parent / 'SYSTEMSAVEDATA00').glob('*.BIN'))
    before = {str(path): digest(path) for path in siblings}
    inventory = SaveInstaller(save_path=save, crypto=SaveCrypto(default_crypto_tool(ROOT)), state_root=args.output.parent / 'state').capture_inventory()
    seeds = deterministic_natural_seeds(args.count, 20260907)
    result = {'schema': 'nioh3-early-playthrough-matrix/v1', 'pid': pid, 'game_version': profile.display_version,
              'executable_sha256': digest(Path(executable)), 'seed_count_per_cell': len(seeds),
              'seed_sequence_sha256': hashlib.sha256(b''.join(struct.pack('<I', seed) for seed in seeds)).hexdigest(),
              'save_context_sha256': hashlib.sha256(inventory.decrypted).hexdigest(),
              'title_screen_confirmed': True, 'preserve_requested_rarity': False,
              'evidence_scope': 'Isolated native construction in the current loaded title-screen context; not natural drops, installation or reveal acceptance',
              'cells': [], 'save_files_unchanged': None}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    oracle = NativeBatchOracle(pid=pid, runtime_profile=profile, max_batch_size=128, preserve_requested_rarity=False)
    try:
        with oracle:
            for level in (1, 180):
                reference = {}
                for rarity in (3, 5):
                    for playthrough in (3, 1, 2):
                        template = inventory.template_record_for_playthrough(playthrough)
                        headers, terminal, ordinary_count, special_flags = Counter(), Counter(), Counter(), Counter()
                        ng3_mismatches, replay_mismatches, samples = 0, 0, []
                        for start in range(0, len(seeds), 128):
                            selected = seeds[start:start + 128]
                            sources = [build_source_record(template, seed=seed, rarity=rarity, level=level, recommended_level=183) for seed in selected]
                            records = oracle.generate(sources)
                            for seed, record in zip(selected, records, strict=True):
                                effects = record[0x34:0xDC]
                                ids = [struct.unpack_from('<I', record, 0x38 + i * 24)[0] for i in range(7)]
                                headers[f'{record[0x30]}/{record[0x31]}'] += 1
                                terminal[','.join(f'{value:08X}' for value in ids[4:])] += 1
                                ordinary_count[str(sum(value not in (0, 1, 0xFFFFFFFF) for value in ids))] += 1
                                special_flags[','.join(str(struct.unpack_from('<I', record, 0x34 + i * 24)[0]) for i in (4, 5, 6))] += 1
                                key = rarity, seed
                                if playthrough == 3:
                                    reference[key] = effects
                                elif effects != reference[key]:
                                    ng3_mismatches += 1
                                if rarity == 3:
                                    predicted = serialize_ng3_rarity3_effect_slots(generate_ng3_rarity3_effect_sequence(seed, level=level))
                                    replay_mismatches += effects != predicted
                                if len(samples) < 8:
                                    samples.append({'seed': seed, 'header_rarity': record[0x30], 'effect_ids': ids,
                                                    'effect_slots_hex': effects.hex(), 'level': struct.unpack_from('<H', record, 6)[0]})
                        result['cells'].append({'playthrough': playthrough, 'requested_rarity': rarity, 'level': level,
                            'header_histogram': dict(headers), 'terminal_slot_histogram': dict(terminal),
                            'populated_effect_count_histogram': dict(ordinary_count), 'terminal_prefix_histogram': dict(special_flags),
                            'effect_bytes_different_from_ng3': ng3_mismatches if playthrough != 3 else None,
                            'ng3_r3_replay_mismatches': replay_mismatches if rarity == 3 else None, 'samples': samples})
                        args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
                        print(f'P{playthrough} R{rarity} L{level}: headers={dict(headers)} vsNG3={ng3_mismatches} replay={replay_mismatches}', flush=True)
    finally:
        # A timed-out call must retain its cleanup owner until retirement completes.
        while oracle.remote_call_pending:
            time.sleep(1)
        result['save_files_unchanged'] = all(path.is_file() and digest(path) == before[str(path)] for path in siblings)
        result['elapsed_seconds'] = round(time.monotonic() - started, 3)
        args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    if not result['save_files_unchanged']:
        raise RuntimeError('Save-file identity changed during the experiment; investigate before continuing')


if __name__ == '__main__':
    main()
