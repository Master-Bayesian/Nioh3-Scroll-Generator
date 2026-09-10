"""Evidence integrity checks independent of a running game or Cheat Engine."""
import copy
import json
from pathlib import Path
import struct
import tempfile
import unittest

from research.analyze_live_item_insertion import (
    EvidenceError, analyze_capture, compare_inventories, load_json,
)


def raw_record(record_type=0xE604, serial=17, seed=123, quantity=1, flags=0):
    raw = bytearray(0xE8)
    struct.pack_into('<H', raw, 0, record_type)
    struct.pack_into('<I', raw, 4, quantity)
    struct.pack_into('<I', raw, 0x18, flags)
    struct.pack_into('<I', raw, 0x20, seed)
    struct.pack_into('<Q', raw, 0x28, serial)
    raw[0x30] = 3
    return raw.hex()


def capture():
    source = raw_record()
    return {'schema': 'nioh3-live-insertion-observation/v1', 'pid': 42,
            'entry_hits': 1, 'scroll_hits': 1, 'events': [
                {'kind': 'entry', 'sequence': 1, 'record_type': '0xE604',
                 'is_scroll': True, 'seed': 123, 'source_hex': source,
                 'rdx': '0x2000', 'rsp': '0x3000'},
                {'kind': 'return', 'entry_sequence': 1, 'output_address': '0x2000',
                 'rax': '0x2000', 'rsp': '0x3008', 'source_after_hex': source,
                 'output_record_hex': raw_record(record_type=0, serial=0xFFFFFFFFFFFFFFFF, quantity=0)},
            ]}


def snapshot(*records):
    return {'entries': [{'slot_index': index, 'record_hex': raw}
                        for index, raw in enumerate(records)]}


class LiveInsertionEvidenceTests(unittest.TestCase):
    def test_empty_return_and_preserved_source_do_not_claim_success(self):
        result = analyze_capture(capture())
        self.assertEqual(result['paired_count'], 1)
        self.assertEqual(result['source_unchanged_pair_count'], 1)
        pair = result['pairs'][0]
        self.assertTrue(pair['output_is_empty'])
        self.assertEqual(pair['source_minus_output_quantity'], 1)
        self.assertEqual(pair['insertion_success'], 'not_established_by_return_buffer')
        self.assertEqual(pair['provenance']['kind'], 'unknown')

    def test_changed_source_and_nonempty_output_preserve_exact_offsets(self):
        value = capture()
        changed = bytearray.fromhex(value['events'][1]['source_after_hex'])
        changed[0xA2] = 0x84
        value['events'][1]['source_after_hex'] = changed.hex()
        value['events'][1]['output_record_hex'] = raw_record(serial=18)
        result = analyze_capture(value)
        pair = result['pairs'][0]
        self.assertEqual(pair['source_changed_offsets'], ['0xA2'])
        self.assertEqual(pair['source_to_output_changed_offsets'], ['0x28'])
        self.assertEqual(pair['output']['serial_u64'], 18)
        self.assertEqual(pair['source_minus_output_quantity'], 0)

    def test_missing_return_remains_explicitly_unpaired(self):
        value = capture()
        value['events'].pop()
        value['entry_hits'] = 9
        result = analyze_capture(value)
        self.assertEqual(result['unpaired_entry_sequences'], [1])
        self.assertEqual(result['paired_count'], 0)
        self.assertEqual(result['unrecorded_entry_hits'], 8)

    def test_duplicate_or_orphan_pairing_is_rejected(self):
        for kind in ('entry', 'return', 'orphan', 'return_before_entry'):
            with self.subTest(kind=kind):
                value = capture()
                if kind == 'entry':
                    value['events'].append(copy.deepcopy(value['events'][0]))
                elif kind == 'return':
                    value['events'].append(copy.deepcopy(value['events'][1]))
                elif kind == 'orphan':
                    value['events'][1]['entry_sequence'] = 2
                else:
                    value['events'].reverse()
                with self.assertRaises(EvidenceError):
                    analyze_capture(value)

    def test_return_readback_mismatch_is_rejected(self):
        for field in ('output_address', 'rax', 'rsp'):
            with self.subTest(field=field):
                value = capture()
                value['events'][1][field] = '0x9999'
                with self.assertRaises(EvidenceError):
                    analyze_capture(value)

    def test_malformed_or_unreadable_record_cannot_be_counted(self):
        for bad in (None, '', '00' * 231, 'GG' * 232, '00' * 233):
            with self.subTest(bad=bad):
                value = capture()
                value['events'][1]['source_after_hex'] = bad
                with self.assertRaises(EvidenceError):
                    analyze_capture(value)
        value = capture()
        value['events'][0]['record_type'] = '0x1E82'
        with self.assertRaises(EvidenceError):
            analyze_capture(value)

    def test_inventory_identity_does_not_match_seed(self):
        result = compare_inventories(snapshot(raw_record(serial=17)), snapshot(raw_record(serial=18)))
        self.assertEqual(result['matched_count'], 0)
        self.assertEqual(len(result['added']), 1)
        self.assertEqual(len(result['removed']), 1)
        self.assertEqual(result['added'][0]['provenance']['kind'], 'unknown')

    def test_inventory_identity_uses_full_uint64_serial(self):
        low, high = 17, (1 << 32) + 17
        result = compare_inventories(snapshot(raw_record(serial=low)), snapshot(raw_record(serial=high)))
        self.assertEqual(result['matched_count'], 0)
        self.assertEqual(result['added'][0]['serial_u64'], high)
        self.assertEqual(result['added'][0]['serial_hex'], '0x0000000100000011')
        combined = snapshot(raw_record(serial=low), raw_record(serial=high))
        self.assertEqual(compare_inventories(combined, combined)['matched_count'], 2)
        # All-one low words are valid identities when the full serial is not
        # the uint64 sentinel; uint32 truncation would reject these records.
        serial = (1 << 32) + 0xFFFFFFFF
        combined = snapshot(raw_record(serial=serial))
        combined['entries'][0]['serial_u64'] = serial
        self.assertEqual(compare_inventories(combined, combined)['matched_count'], 1)
        combined['entries'][0]['serial_u64'] = 0xFFFFFFFF
        with self.assertRaises(EvidenceError):
            compare_inventories(combined, combined)

    def test_quantity_getter_honors_width_and_constant_one_precedence(self):
        for flags, stored, expected, encoding in (
            (0, 0x12340002, 2, 'uint16_at_04'),
            (0x00200000, 0x12340002, 0x12340002, 'uint32_at_04'),
            (0x00800000, 0, 1, 'constant_one_flag_00800000'),
            (0x00A00000, 0x12340002, 1, 'constant_one_flag_00800000'),
        ):
            with self.subTest(flags=hex(flags)):
                value = capture()
                value['events'][0]['source_hex'] = raw_record(quantity=stored, flags=flags)
                value['events'][1]['source_after_hex'] = value['events'][0]['source_hex']
                pair = analyze_capture(value)['pairs'][0]
                self.assertEqual(pair['source']['quantity'], expected)
                self.assertEqual(pair['source']['quantity_encoding'], encoding)
                self.assertEqual(pair['source_minus_output_quantity'], expected)

    def test_uint32_stack_remainder_difference_does_not_truncate(self):
        value = capture()
        value['events'][0]['source_hex'] = raw_record(quantity=0x10005, flags=0x00200000)
        value['events'][1]['source_after_hex'] = value['events'][0]['source_hex']
        value['events'][1]['output_record_hex'] = raw_record(quantity=3, flags=0x00200000)
        pair = analyze_capture(value)['pairs'][0]
        self.assertEqual(pair['source_minus_output_quantity'], 0x10002)

    def test_snapshot_provenance_object_shape_is_required(self):
        value = snapshot(raw_record())
        value['provenance'] = {'kind': 'unknown', 'basis': 'Prior experiments possible'}
        value['entries'][0]['provenance'] = dict(value['provenance'])
        self.assertEqual(compare_inventories(value, value)['after_provenance_counts'], {'unknown': 1})
        value['entries'][0]['provenance'] = 'unknown'
        with self.assertRaises(EvidenceError):
            compare_inventories(value, value)

    def test_inventory_full_bytes_and_slot_moves_are_independent(self):
        first, second = raw_record(serial=17), raw_record(serial=18)
        changed = bytearray.fromhex(first)
        changed[0xA2] = 0x84
        result = compare_inventories(snapshot(first, second), snapshot(second, changed.hex()))
        self.assertEqual(result['matched_count'], 2)
        self.assertEqual(result['changed_record_count'], 1)
        self.assertEqual(result['moved_slot_count'], 2)
        self.assertEqual(result['matched'][0]['changed_offsets'], ['0xA2'])
        self.assertEqual(result['after_provenance_counts'], {'unknown': 2})

    def test_inventory_duplicate_identity_and_metadata_mismatch_are_rejected(self):
        value = snapshot(raw_record(), raw_record())
        with self.assertRaises(EvidenceError):
            compare_inventories(value, value)
        value = snapshot(raw_record())
        value['entries'][0]['seed'] = 456
        with self.assertRaises(EvidenceError):
            compare_inventories(value, value)

    def test_natural_origin_claims_are_rejected_at_each_input_level(self):
        for target in ('capture', 'event', 'snapshot', 'record'):
            with self.subTest(target=target):
                value, inventory = capture(), snapshot(raw_record())
                claim = {'kind': 'natural', 'evidence_ref': 'same seed or inventory presence'}
                selected = {'capture': value, 'event': value['events'][0],
                            'snapshot': inventory, 'record': inventory['entries'][0]}[target]
                selected['provenance'] = claim
                with self.assertRaises(EvidenceError):
                    if target in ('capture', 'event'):
                        analyze_capture(value)
                    else:
                        compare_inventories(inventory, inventory)

    def test_explicit_experimental_origin_is_preserved(self):
        value = snapshot(raw_record())
        claim = {'kind': 'experimental', 'evidence_ref': 'controlled-test-17'}
        value['entries'][0]['provenance'] = claim
        result = compare_inventories(snapshot(), value)
        self.assertEqual(result['added'][0]['provenance'], claim)
        value['entries'][0]['provenance'] = {'kind': 'experimental'}
        with self.assertRaises(EvidenceError):
            compare_inventories(snapshot(), value)

    def test_owner_mapping_requires_same_process_and_matching_bounds(self):
        owners = {'schema': 'nioh3-observed-stack-owners/v1', 'pid': 42,
                  'matches': [{'thread_id': 99, 'stack_limit': '0x2000',
                               'stack_base': '0x4000', 'matched_pointers': ['0x3000']}]}
        result = analyze_capture(capture(), owners)
        self.assertEqual(result['pairs'][0]['observed_stack_owner_thread_id'], 99)
        owners['pid'] = 43
        with self.assertRaises(EvidenceError):
            analyze_capture(capture(), owners)
        owners['pid'] = 42
        owners['matches'][0]['stack_limit'] = '0x3001'
        with self.assertRaises(EvidenceError):
            analyze_capture(capture(), owners)

    def test_json_rejects_duplicate_keys_and_nonfinite_numbers(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'input.json'
            for text in ('{"events": [], "events": [1]}', '{"value": NaN}', '{"value": Infinity}'):
                path.write_text(text, encoding='utf-8')
                with self.assertRaises(EvidenceError):
                    load_json(path)
            path.write_text(json.dumps(snapshot(raw_record())), encoding='utf-8')
            parsed, identity = load_json(path)
            self.assertEqual(len(parsed['entries']), 1)
            self.assertEqual(len(identity['sha256']), 64)


if __name__ == '__main__':
    unittest.main()
