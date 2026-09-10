"""Check assembly serialization and protocol bounds without invoking the game."""
from io import BytesIO
from pathlib import Path
import struct
import unittest

from nioh3_scroll_editor.live_add_descriptor import assembly_descriptor, verify_assembly_preview, new_assembly_record
from nioh3_scroll_editor.live_add_ce_transport import encode_request, read_exact
from nioh3_scroll_editor.live_add_profile import PC_V201


class LiveAddDescriptorTests(unittest.TestCase):
    def source(self):
        r = bytearray(232)
        struct.pack_into('<HHHH', r, 0, 0xE604, 0x1122, 0x3344, 170)
        struct.pack_into('<H', r, 0x10, 561)
        struct.pack_into('<I', r, 0x14, 0x55667788)
        struct.pack_into('<I', r, 0x18, 0x2800002)
        struct.pack_into('<I', r, 0x20, 10030565)
        r[0x0F] = 1;r[0x30] = 3
        r[0x34:0xDC] = bytes(range(168))
        r[0xDC:0xE0] = b'abcd'
        return bytes(r)

    def test_descriptor_preserves_identity_effect_inputs_and_seed(self):
        r = self.source();d = assembly_descriptor(r)
        self.assertEqual(len(d), 204)
        self.assertEqual(struct.unpack_from('<Q', d, 24)[0], 0x1122334455667788)
        self.assertEqual(d[0x24:], r[0x34:0xDC])
        self.assertEqual(d[0x14:0x18], b'abcd')
        self.assertEqual(struct.unpack_from('<I', d, 8)[0], 561)
        self.assertEqual(d[0x21], 1)
        other = bytearray(d);other[0x21] = 0
        self.assertEqual(assembly_descriptor(r, allocate_serial=True), bytes(other))

    def test_new_record_drops_only_template_inventory_metadata(self):
        old = bytearray(self.source())
        struct.pack_into('<II', old, 0x18, 0x0F800080, 0xC6BE)
        clean = new_assembly_record(bytes(old))
        self.assertEqual(clean, self.source())
        self.assertEqual(assembly_descriptor(clean), assembly_descriptor(bytes(old)))
        actual = bytearray(clean)
        actual[40:48] = b'\xff' * 8
        self.assertTrue(verify_assembly_preview(clean, bytes(actual)))
        for offset in (0x18, 0x1C, 0x20, 0x34):
            changed = bytearray(actual)
            changed[offset] ^= 1
            with self.assertRaises(ValueError):
                verify_assembly_preview(clean, bytes(changed))

    def test_preview_rejects_changed_content_and_allocated_serial(self):
        r = self.source();a = bytearray(r);a[40:48] = b'\xff'*8
        self.assertTrue(verify_assembly_preview(r, bytes(a)))
        a[60] ^= 1
        with self.assertRaises(ValueError): verify_assembly_preview(r, bytes(a))
        with self.assertRaises(ValueError): verify_assembly_preview(r, r)
        with self.assertRaises(ValueError): assembly_descriptor(bytes(232))

    def test_only_supported_playthrough_types_are_accepted(self):
        for kind in (0x1E82, 0x516D, 0xE604):
            source = bytearray(self.source())
            struct.pack_into('<H', source, 0, kind)
            self.assertEqual(struct.unpack_from('<H', assembly_descriptor(source))[0], kind)
        source = bytearray(self.source())
        struct.pack_into('<H', source, 0, 0xFFFF)
        with self.assertRaises(ValueError):
            assembly_descriptor(source)

    def test_generated_ce_layout_matches_the_python_profile(self):
        p = Path(__file__).parent / 'research/live_add_layout_ce.lua'
        text = p.read_text()
        for key, value in PC_V201.lua_layout().items():
            expected = '"' + value + '"' if isinstance(value, str) else hex(value)
            self.assertIn(key + '=' + expected + ',', text)

    def test_protocol_rejects_injection_unknown_methods_and_truncation(self):
        token = 'a'*64
        self.assertEqual(struct.unpack('<I', encode_request('ping', token)[:4])[0], len(encode_request('ping', token))-4)
        for method, fields in [('eval', {}), ('ping', {'token': 'b'*64}), ('preview', {'operation_id': 'a\nmethod=insert'})]:
            with self.assertRaises(ValueError): encode_request(method, token, fields)
        with self.assertRaises(ConnectionError): read_exact(BytesIO(b'abc'), 4)


if __name__ == '__main__':
    unittest.main()
