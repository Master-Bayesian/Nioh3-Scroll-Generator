"""Extract reviewed, sanitized record vectors from the completed live observation."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'deliverables/frontend-v2/live-acceptance/20260907T225708Z'
RANGES = [(2, 4), (0x14, 4), (0x1C, 4), (0x28, 8)]


def read(name):
    return json.loads((SOURCE / name).read_text(encoding='utf-8'))


def scrub(raw):
    record = bytearray.fromhex(raw)
    assert len(record) == 232
    for offset, size in RANGES:
        record[offset:offset + size] = bytes(size)
    return record.hex()


def record_for(name, serial):
    value = read(name)
    inventory = value.get('result', value.get('inventory'))
    return next(e['record_hex'] for e in inventory['entries']
                if struct.unpack_from('<Q', bytes.fromhex(e['record_hex']), 0x28)[0] == serial)


def main():
    sources = ['r3-10030565-first-reveal-analysis.json', 'r4-43723117-first-reveal-analysis.json',
               'r4-36526331-first-reveal-analysis.json']
    records = []
    for name, seed, before_key, saved_key in [(sources[0], 10030565, 'pre_clear', 'decrypted_save_copy'),
                                             (sources[1], 43723117, 'before_clear', 'decrypted_save_copy')]:
        phases = read(name)['target_phase_records']
        records.append({'seed': seed, 'rarity': 3 if seed == 10030565 else 4,
                        'initial_capacity': 7, 'remaining_after': 6,
                        'recommended_displayed_level': 343 if seed == 10030565 else 160,
                        'before_hex': scrub(phases[before_key]['record_hex']),
                        'revealed_hex': scrub(phases['after_first_reveal']['record_hex']),
                        'saved_hex': scrub(phases[saved_key]['record_hex'])})
    records.append({'seed': 36526331, 'rarity': 4, 'initial_capacity': 4, 'remaining_after': 3,
                    'recommended_displayed_level': 160,
                    'before_hex': scrub(record_for('r4-36526331-before-first-clear.json', 2375803)),
                    'revealed_hex': scrub(record_for('r4-36526331-after-first-reveal.json', 2375803)),
                    'saved_hex': scrub(record_for('final-saved-runtime.json', 2375803)),
                    'completion_candidates': [{'effect_index': p['effect_slot_one_based'] - 1,
                        'source_hex': scrub(p['source_record_hex']), 'candidate_hex': scrub(p['actual_candidate_hex'])}
                        for p in read(sources[2])['paired_attempts']]})
    result = {'schema': 'nioh3-sanitized-first-reveal/v1', 'game_version': 'PC v2.01',
              'source_directory': SOURCE.relative_to(ROOT).as_posix(),
              'sanitized_ranges': [{'offset': a, 'length': b} for a, b in RANGES],
              'provenance': 'R3 acquired during observed gameplay with maximum drop rate enabled. The two R4 records pre-existed and may originate from experiments. All three reveal transitions were observed in game.',
              'sources': [{'path': name, 'sha256': hashlib.sha256((SOURCE / name).read_bytes()).hexdigest()} for name in sources],
              'records': records}
    target = ROOT / 'test_fixtures/live_first_reveal_pc_v201.json'
    target.parent.mkdir(exist_ok=True)
    target.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(target)


if __name__ == '__main__':
    main()
