"""Mutation evidence must reject plausible but inconsistent success receipts."""
from copy import deepcopy
import hashlib
import struct
import unittest

from nioh3_scroll_editor.dispatch_evidence import REGISTERS
from nioh3_scroll_editor.live_add_evidence import verify, verify_persistence


class InsertionEvidenceTests(unittest.TestCase):
    def setUp(self):
        raw = bytearray(232)
        struct.pack_into('<H', raw, 0, 0xE604)
        struct.pack_into('<Q', raw, 40, 2**40 + 7)
        struct.pack_into('<I', raw, 32, 123)
        raw[24] = 2
        self.raw = bytes(raw)
        destination = bytearray(raw)
        struct.pack_into('<I', destination, 24, struct.unpack_from('<I', raw, 24)[0] | 0x04000080)
        struct.pack_into('<I', destination, 28, 20)
        self.destination = bytes(destination)
        serial = str(2**40 + 7)
        self.plan = {'operation_id': 'operation-one', 'pid': 1, 'slot': 0,
                     'serial': int(serial), 'container_hex': bytes(400*232).hex()}
        registers = dict.fromkeys(REGISTERS, 0)
        registers.update(RSP=0x900008, RIP=0x1000, EFLAGS=0x206)
        self.execution = {'operation_id': 'operation-one', 'mode': 'single_native_insertion',
                          'pid': 1, 'status': 3, 'slot': 0, 'phase': 'completed',
                          'redirect_count': 1, 'released': True, 'breakpoints': [],
                          'before': registers, 'after': dict(registers, RSP=0x8FFFC0, RIP=0x1007),
                          'source_hex': self.raw.hex(), 'destination_hex': self.destination.hex(),
                          'remainder_hex': bytes(232).hex()}
        self.before = {'pid': 1, 'capacity': 400, 'duplicate_scroll_serials': [], 'entries': [],
                       'serial_counter': serial, 'acquisition_order_counter': 20,
                       'container_sha256': hashlib.sha256(bytes(400*232)).hexdigest()}
        self.after = dict(self.before, serial_counter=str(int(serial)+1), acquisition_order_counter=21,
                          container_sha256=hashlib.sha256(self.destination+bytes(399*232)).hexdigest(),
                          entries=[{'serial': serial, 'slot_index': 0, 'seed': 123, 'record_hex': self.destination.hex()}])
        self.index_before = {'pid': 1, 'node_count': 0, 'entries': []}
        self.index_after = {'pid': 1, 'node_count': 1, 'entries': [{'serial': serial, 'slot': 0}]}

    def run_verify(self):
        return verify(self.plan, self.execution, self.before, self.after, self.index_before, self.index_after)

    def test_high_uint64_serial_and_complete_container(self):
        self.assertTrue(self.run_verify()['full_container_and_native_index_verified'])

    def test_partial_or_wrong_acknowledgements(self):
        original = deepcopy(self.execution)
        for key, value in (('released', False), ('status', 2), ('slot', 1), ('operation_id', 'other'), ('redirect_count', 2)):
            self.execution = dict(original, **{key: value})
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.run_verify()

    def test_wrong_index_or_duplicate_full_key(self):
        self.index_after['entries'][0]['slot'] = 1
        with self.assertRaises(ValueError):
            self.run_verify()
        self.index_after['entries'][0]['slot'] = 0
        self.index_after['entries'] *= 2
        self.index_after['node_count'] = 2
        with self.assertRaises(ValueError):
            self.run_verify()

    def test_an_unoccupied_byte_change_is_not_ignored(self):
        self.after['container_sha256'] = hashlib.sha256(self.destination+bytes(399*232-1)+b'\1').hexdigest()
        with self.assertRaises(ValueError):
            self.run_verify()

    def test_only_explicit_new_marker_clear_is_allowed_in_persistence(self):
        saved = bytearray(self.destination)
        saved[24] &= ~2
        with self.assertRaises(ValueError):
            verify_persistence(self.after, [bytes(saved)])
        self.assertEqual(verify_persistence(self.after, [bytes(saved)], allow_new_marker_clear=True)['records_verified'], 1)
        saved[50] ^= 1
        with self.assertRaises(ValueError):
            verify_persistence(self.after, [bytes(saved)], allow_new_marker_clear=True)


if __name__ == '__main__':
    unittest.main()
