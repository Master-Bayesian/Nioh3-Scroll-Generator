"""Factor the R3 template flag contract without changing production algorithms."""
from pathlib import Path
import argparse
import hashlib
import inspect
import json
import struct
import sys
import time
from dataclasses import replace

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from emaki_exchange import CATEGORY_TO_TYPE
from nioh3_scroll_editor import effect_sequence as effects
from nioh3_scroll_editor.runtime_application import running_game_identity
from nioh3_scroll_editor.native import NativeBatchOracle, build_source_record
from nioh3_scroll_editor.savegame import SaveInstaller, SaveCrypto, default_crypto_tool, discover_save_paths
from research.validate_ng3_rarity3_native_parity_live import deterministic_natural_seeds


def experimental_replay(playthrough):
    # Controlled research specialization of the frozen algorithm, never imported by product code.
    namespace = dict(vars(effects))
    namespace.update(NG3_RECORD_TYPE=CATEGORY_TO_TYPE[playthrough], research_playthrough=playthrough)
    source = inspect.getsource(effects.generate_ng3_rarity3_effect_sequence).replace('playthrough=3', 'playthrough=research_playthrough')
    exec(compile(source, '<research-r3-specialization>', 'exec'), namespace)
    return namespace['generate_ng3_rarity3_effect_sequence']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--count', type=int, default=4096)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--title-screen-confirmed', action='store_true', required=True)
    args = parser.parse_args()
    if args.output.exists() or not 1 <= args.count <= 100000:
        raise ValueError('Use a new output and a bounded count')
    pid, profile, _ = running_game_identity()
    paths = discover_save_paths()
    if len(paths) != 1: raise ValueError('Ambiguous save context')
    before = hashlib.sha256(paths[0].read_bytes()).hexdigest()
    inventory = SaveInstaller(save_path=paths[0], crypto=SaveCrypto(default_crypto_tool(ROOT)), state_root=args.output.parent / 'state').capture_inventory()
    seeds = deterministic_natural_seeds(args.count, 20260908)
    report = {'schema': 'nioh3-r3-template-factor/v1', 'game_version': profile.display_version,
              'samples_per_cell': len(seeds), 'cells': [], 'production_model_changed': False,
              'scope': 'Isolated native generation; controlled source metadata flags; not natural drop or game inventory acceptance'}
    oracle = NativeBatchOracle(pid=pid, runtime_profile=profile, max_batch_size=128)
    try:
        with oracle:
            for playthrough in (1, 2, 3):
                replay = experimental_replay(playthrough)
                original = inventory.template_record_for_playthrough(playthrough)
                for level in (1, 180):
                    for mode in ('original', 'clear_flags', 'fixed_growth_flag'):
                        template = bytearray(original)
                        if mode != 'original':
                            for slot in range(7): template[0x42 + slot * 24] = 0
                        if mode == 'fixed_growth_flag': template[0xA2] = 0x84
                        growth, ordinary_mismatch, full_mismatch, samples = 0, 0, 0, []
                        for start in range(0, len(seeds), 128):
                            selected = seeds[start:start + 128]
                            records = oracle.generate([build_source_record(bytes(template), seed=seed, rarity=3, level=level, recommended_level=183) for seed in selected])
                            for seed, record in zip(selected, records, strict=True):
                                predicted = effects.serialize_ng3_rarity3_effect_slots(replace(replay(seed, level=level), record_type=0xE604, playthrough=3))
                                actual = record[0x34:0xDC]
                                growth += struct.unpack_from('<I', record, 0x98)[0] == 1
                                ordinary_mismatch += actual[:96] != predicted[:96]
                                full_mismatch += actual != predicted
                                if actual != predicted and len(samples) < 4:
                                    samples.append({'seed': seed, 'differing_effect_offsets': [i for i,(a,b) in enumerate(zip(actual,predicted)) if a!=b]})
                        row = {'playthrough': playthrough, 'level': level, 'template_mode': mode, 'growth_count': growth,
                               'ordinary_effect_bytes_mismatches': ordinary_mismatch, 'all_effect_bytes_mismatches': full_mismatch,
                               'mismatch_samples': samples}
                        report['cells'].append(row)
                        print({key:value for key,value in row.items() if key!='mismatch_samples'}, flush=True)
                        args.output.parent.mkdir(parents=True, exist_ok=True)
                        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    finally:
        while oracle.remote_call_pending: time.sleep(1)
        report['save_unchanged'] = hashlib.sha256(paths[0].read_bytes()).hexdigest() == before
        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__': main()
