"""Reject false acceptance of the bounded game-thread probe evidence."""
from copy import deepcopy
import unittest

from research.verify_dispatch_probe import REGISTERS, verify


class DispatchProbeEvidenceTests(unittest.TestCase):
    def setUp(self):
        before = {key: index + 1 for index, key in enumerate(REGISTERS)}
        before.update(RSP=0x900008, RIP=0x1000, EFLAGS=0x206)
        after = dict(before, RSP=0x8FFFC0, RIP=0x1007, EFLAGS=0x206)
        self.execution = {'phase': 'completed', 'redirect_count': 1, 'released': True,
                          'breakpoints': [], 'before': before, 'after': after,
                          'mode': 'read_only_slot_lookup', 'actual': 123, 'expected': 123}
        self.inventory = {'pid': 1, 'capacity': 400, 'entries': [], 'serial_counter': '42',
                          'acquisition_order_counter': 10, 'container_sha256': 'same',
                          'duplicate_scroll_serials': []}

    def test_valid_prologue_and_query(self):
        self.assertTrue(verify(self.execution, self.inventory, self.inventory)['native_read_only_query'])

    def test_register_stack_flags_and_result_corruption_are_rejected(self):
        for field in ('RAX', 'RSP', 'RIP', 'EFLAGS'):
            value = deepcopy(self.execution)
            value['after'][field] ^= 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify(value, self.inventory, self.inventory)
        value = deepcopy(self.execution)
        value['actual'] = 124
        with self.assertRaises(ValueError):
            verify(value, self.inventory, self.inventory)

    def test_missing_cleanup_extra_execution_and_inventory_changes_are_rejected(self):
        for field, replacement in (('released', False), ('redirect_count', 2), ('breakpoints', [123])):
            value = dict(self.execution, **{field: replacement})
            with self.subTest(field=field), self.assertRaises(ValueError):
                verify(value, self.inventory, self.inventory)
        changed = dict(self.inventory, serial_counter='43')
        with self.assertRaises(ValueError):
            verify(self.execution, self.inventory, changed)


if __name__ == '__main__':
    unittest.main()
