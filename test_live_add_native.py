"""Native executor ABI and accepted shim byte regression, without game access."""
import hashlib
import unittest

from nioh3_scroll_editor.live_add_dispatch_code import build_dispatch_code
from nioh3_scroll_editor.windows_debug_session import Context, DebugEvent


class NativeExecutorTests(unittest.TestCase):
    def test_emitter_matches_previously_accepted_ce_insertion(self):
        actual = build_dispatch_code(0x700000, 0x7FF7CE446847, bytes.fromhex('4053574883EC38'),
            0x7FF7CF3DC4CC, 0x700600, 0x700400,
            {'serial': 2416080, 'data': 0x21FFC441280, 'manager': 0x7FF7D18AD490,
             'function_address': 0x7FF7CD6AD294})
        # Digest of the historically executed 357-byte CE fixture. Only our
        # generated shim is covered; no original game instructions are vendored.
        self.assertEqual(len(actual), 357)
        self.assertEqual(hashlib.sha256(actual).hexdigest(),
                         'b8dde7b008aa695f3c70cef19a32d4acd8cf42f680dafcb849e46107dd0a801f')

    def test_sdk_layout_offsets_and_sizes(self):
        import ctypes
        self.assertEqual(ctypes.sizeof(Context), 1232)
        self.assertEqual(Context.ContextFlags.offset, 48)
        self.assertEqual(Context.Rip.offset, 248)
        self.assertEqual(ctypes.sizeof(DebugEvent), 176)
        self.assertEqual(DebugEvent.data.offset, 16)


if __name__ == '__main__':
    unittest.main()
