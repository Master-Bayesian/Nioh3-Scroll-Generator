"""Focused synthetic tests for the versioned live-add inventory ABI."""
from dataclasses import replace
import unittest

from nioh3_scroll_editor.live_add_profile import (
    InventoryGlobalMode,
    PC_V201,
    PC_V202,
    PC_V202_EVIDENCE,
    resolve_inventory_pointers,
    live_add_profile,
)


class _Memory:
    def __init__(self, values):
        self.values = dict(values)

    def u64(self, address):
        try:
            return self.values[address]
        except KeyError as error:
            raise AssertionError(f"unexpected synthetic read at {address:#x}") from error


class LiveAddProfileTests(unittest.TestCase):
    def test_manager_object_mode_resolves_global_manager_and_data(self):
        base = 0x10000000
        global_address = base + PC_V201.manager_pointer_rva
        manager = 0x20000000
        data = 0x30000000
        memory = _Memory({global_address: manager, manager: data})

        pointers = resolve_inventory_pointers(memory.u64, base, PC_V201)

        self.assertEqual(pointers.global_address, global_address)
        self.assertEqual(pointers.manager_address, manager)
        self.assertEqual(pointers.data_address, data)
        self.assertEqual(PC_V201.inventory_global_mode, InventoryGlobalMode.MANAGER_OBJECT.value)

    def test_direct_data_mode_resolves_global_as_data_without_manager_read(self):
        base = 0x10000000
        global_address = base + PC_V201.manager_pointer_rva
        data = 0x30000000
        memory = _Memory({global_address: data})
        direct = replace(PC_V201, inventory_global_mode=InventoryGlobalMode.DIRECT_DATA.value)

        pointers = direct.resolve_inventory(memory.u64, base)

        self.assertEqual(pointers.global_address, global_address)
        self.assertIsNone(pointers.manager_address)
        self.assertEqual(pointers.data_address, data)

    def test_absent_or_invalid_mode_fails_closed_before_pointer_dereference(self):
        base = 0x10000000
        global_address = base + PC_V201.manager_pointer_rva
        memory = _Memory({global_address: 0x20000000})
        for mode, message in ((None, 'required'), ('future_shape', 'Unsupported')):
            with self.subTest(mode=mode):
                profile = replace(PC_V201, inventory_global_mode=mode)
                with self.assertRaisesRegex(ValueError, message):
                    resolve_inventory_pointers(memory.u64, base, profile)

    def test_v202_candidate_is_content_complete_but_not_selectable(self):
        self.assertEqual(PC_V202.file_version, (2, 0, 2, 0))
        self.assertEqual(PC_V202.inventory_global_mode, InventoryGlobalMode.MANAGER_OBJECT.value)
        self.assertEqual(
            (PC_V202.dispatch_rva, PC_V202.dispatch_return_rva,
             PC_V202.dispatch_signature_hex, PC_V202.builder_rva, PC_V202.builder_size,
             PC_V202.insertion_rva, PC_V202.insertion_size, PC_V202.slot_lookup_rva),
            (0x12E9E50, 0x20BB1C, '4053574883EC38', 0x227FC5C, 0x27B,
             0x54D324, 0xE17, 0x55308C))
        self.assertEqual(
            (PC_V202.manager_pointer_rva, PC_V202.scheduler_pointer_rva,
             PC_V202.container_offset, PC_V202.capacity_offset,
             PC_V202.serial_counter_offset, PC_V202.serial_index_offset),
            (0x4751530, 0x4745348, 0x224A60, 0x16A80, 8, 0x23B5E8))
        self.assertEqual(
            (PC_V202.scheduler_pending_offset, PC_V202.scheduler_ready_offset,
             PC_V202.queue_begin_offset, PC_V202.queue_end_offset,
             PC_V202.record_size, PC_V202.descriptor_size, PC_V202.capacity),
            (0x1408, 0x1629, 0x60, 0x68, 0xE8, 0xCC, 400))
        # Content-complete does not mean selectable.
        self.assertFalse(PC_V202.accepted)
        self.assertTrue(PC_V202.candidate_only)
        self.assertFalse(PC_V202.is_dispatchable())
        with self.assertRaisesRegex(ValueError, 'not dispatchable'):
            PC_V202.require_dispatchable()
        with self.assertRaisesRegex(ValueError, 'not dispatchable'):
            PC_V202.resolve_inventory(lambda _address: 0x30000000, 0x10000000)
        with self.assertRaisesRegex(ValueError, 'not been accepted'):
            live_add_profile(PC_V202.file_version)

    def test_v202_candidate_numbers_resolve_the_manager_object_chain(self):
        """The candidate's own numbers must be coherent for the accepted ABI.

        This deliberately forces dispatchability on a copy, so the check does
        not depend on the candidate being selectable in the product.
        """

        base = 0x10000000
        global_address = base + PC_V202.manager_pointer_rva
        manager = 0x20000000
        data = 0x30000000
        memory = _Memory({global_address: manager, manager: data})
        forced = replace(PC_V202, accepted=True, candidate_only=False)

        pointers = forced.resolve_inventory(memory.u64, base)

        self.assertEqual(pointers.global_address, global_address)
        self.assertEqual(pointers.manager_address, manager)
        self.assertEqual(pointers.data_address, data)
        self.assertEqual(forced.container_offset, 0x224A60)

    def test_v202_candidate_evidence_names_every_version_field(self):
        layout_keys = set(PC_V202.lua_layout())
        # Only the three record/descriptor constants inherited unchanged from
        # PC_V201 carry no separate lane in the candidate evidence.
        shared_defaults = {'record_size', 'descriptor_size', 'capacity'}
        self.assertEqual({key for key in PC_V202_EVIDENCE if key in layout_keys},
                         layout_keys - shared_defaults)
        for key, lane in PC_V202_EVIDENCE.items():
            with self.subTest(key=key):
                self.assertIsInstance(lane, str)
                self.assertTrue(lane)
        self.assertEqual(
            PC_V202_EVIDENCE['executable_sha256'],
            'E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130')

    def test_manager_object_missing_owner_keeps_the_shipped_diagnostics(self):
        base = 0x10000000
        global_address = base + PC_V201.manager_pointer_rva
        empty = _Memory({global_address: 0})
        with self.assertRaisesRegex(RuntimeError, 'Item manager is not loaded'):
            resolve_inventory_pointers(empty.u64, base, PC_V201)
        manager = 0x20000000
        missing_data = _Memory({global_address: manager, manager: 0})
        with self.assertRaisesRegex(RuntimeError, 'Inventory data is not loaded'):
            resolve_inventory_pointers(missing_data.u64, base, PC_V201)

    def test_direct_data_missing_global_reports_data_without_manager_read(self):
        base = 0x10000000
        global_address = base + PC_V201.manager_pointer_rva
        empty = _Memory({global_address: 0})
        direct = replace(PC_V201, inventory_global_mode=InventoryGlobalMode.DIRECT_DATA.value)
        with self.assertRaisesRegex(RuntimeError, 'Inventory data is not loaded'):
            resolve_inventory_pointers(empty.u64, base, direct)

    def test_layout_export_is_explicit_and_hides_candidate_metadata(self):
        shipped = PC_V201.lua_layout()
        self.assertEqual(shipped['inventory_global_mode'], 'manager_object')
        self.assertFalse(any(value is None for value in shipped.values()))
        for hidden in ('profile_id', 'file_version', 'accepted', 'candidate_only'):
            self.assertNotIn(hidden, shipped)
        self.assertEqual(shipped['manager_pointer_rva'], PC_V201.manager_pointer_rva)
        candidate = PC_V202.lua_layout()
        self.assertEqual(candidate['inventory_global_mode'], 'manager_object')
        self.assertEqual(candidate['manager_pointer_rva'], PC_V202.manager_pointer_rva)
        self.assertNotIn('accepted', candidate)
        self.assertNotIn('candidate_only', candidate)
        self.assertFalse(any(value is None for value in candidate.values()))

    def test_missing_or_placeholder_fields_are_not_dispatchable(self):
        for change in ({'manager_pointer_rva': None}, {'builder_size': -1},
                       {'dispatch_signature_hex': ''}, {'dispatch_rva': None},
                       {'accepted': False}, {'candidate_only': True}):
            with self.subTest(change=change):
                profile = replace(PC_V201, **change)
                self.assertFalse(profile.is_dispatchable())
                with self.assertRaisesRegex(ValueError, 'not dispatchable'):
                    profile.resolve_inventory(lambda _address: 0x30000000, 0x10000000)


if __name__ == '__main__':
    unittest.main()
