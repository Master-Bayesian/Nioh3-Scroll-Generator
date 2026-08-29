from __future__ import annotations

import struct
import unittest

from nioh3_scroll_editor.native import build_effect_finalizer_batch_wrapper


class EffectFinalizerBatchWrapperTests(unittest.TestCase):
    def test_uses_mov_r12_imm64_not_mov_r12d_imm32(self) -> None:
        source = 0x1111111122222222
        destination = 0x3333333344444444
        function = 0x5555555566666666
        wrapper = build_effect_finalizer_batch_wrapper(
            source,
            destination,
            function,
            count=2,
            effect_index=4,
            reveal=True,
        )
        encoded = b"\x49\xBC" + struct.pack("<Q", function)
        self.assertIn(encoded, wrapper)
        self.assertNotIn(b"\x41\xBC" + struct.pack("<I", function & 0xFFFFFFFF), wrapper)

    def test_short_back_edge_targets_loop_start(self) -> None:
        wrapper = build_effect_finalizer_batch_wrapper(
            0x100000,
            0x200000,
            0x300000,
            count=17,
            effect_index=6,
            reveal=False,
        )
        # The only JNZ short is the batch-loop back edge.
        jnz = wrapper.index(b"\x75")
        displacement = struct.unpack_from("<b", wrapper, jnz + 1)[0]
        target = jnz + 2 + displacement
        loop_marker = wrapper.index(bytes.fromhex("48 89 F1 48 89 DA"))
        self.assertEqual(target, loop_marker)

    def test_call_abi_immediates(self) -> None:
        wrapper = build_effect_finalizer_batch_wrapper(
            0x1000,
            0x2000,
            0x3000,
            count=1,
            effect_index=5,
            reveal=True,
        )
        self.assertIn(b"\x41\xB8\x05\x00\x00\x00", wrapper)  # mov r8d, 5
        self.assertIn(b"\x41\xB9\x01\x00\x00\x00", wrapper)  # mov r9d, 1
        self.assertIn(b"\x41\xFF\xD4", wrapper)  # call r12


if __name__ == "__main__":
    unittest.main()
