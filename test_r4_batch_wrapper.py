from __future__ import annotations

import struct
import unittest
from dataclasses import replace

from nioh3_scroll_editor.native import build_effect_finalizer_batch_wrapper
from nioh3_scroll_editor.native import (
    DEFAULT_NATIVE_RUNTIME_PROFILE,
    NativeBatchOracle,
    NativeRuntimeProfile,
    build_explicit_playthrough_seed_range_wrapper,
)


class EffectFinalizerBatchWrapperTests(unittest.TestCase):
    def test_v201_product_mode_preserves_requested_raw_rarity_five(self) -> None:
        oracle = object.__new__(NativeBatchOracle)
        oracle.preserve_requested_rarity = True
        oracle.runtime_profile = replace(
            DEFAULT_NATIVE_RUNTIME_PROFILE,
            display_version="PC v2.01",
        )
        record = bytearray(0xE8)
        record[0x30:0x32] = b"\x04\x04"

        preserved = oracle._preserve_rarity_headers([bytes(record)], [5])[0]

        self.assertEqual(preserved[0x30:0x32], b"\x05\x05")

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

    def test_explicit_playthrough_wrapper_uses_profiled_addresses(self) -> None:
        profile = NativeRuntimeProfile(
            display_version="test",
            canonicalize_rva=0x100,
            canonicalize_signature=b"a",
            finalize_effect_rva=0x200,
            finalize_effect_signature=b"b",
            descriptor_complete_rva=0x300,
            descriptor_complete_signature=b"c",
            init_compact_rva=0x310,
            reset_compact_rva=0x320,
            effective_level_rva=0x330,
            init_generation_context_rva=0x340,
            incomplete_record_rva=0x350,
            generate_effects_rva=0x360,
            assemble_scroll_rva=0x370,
            playthrough_vector_rva=0x380,
            playthrough_manager_pointer_rva=0x390,
            native_signatures=(),
        )
        module_base = 0x100000000
        wrapper = build_explicit_playthrough_seed_range_wrapper(
            source=0x200000000,
            destination=0x300000000,
            module_base=module_base,
            start_seed=1,
            seed_step=1,
            count=2,
            playthrough=3,
            runtime_profile=profile,
        )

        for rva in (
            profile.init_compact_rva,
            profile.reset_compact_rva,
            profile.effective_level_rva,
            profile.init_generation_context_rva,
            profile.incomplete_record_rva,
            profile.generate_effects_rva,
            profile.assemble_scroll_rva,
            profile.playthrough_vector_rva,
            profile.playthrough_manager_pointer_rva,
        ):
            self.assertIn(struct.pack("<Q", module_base + rva), wrapper)


if __name__ == "__main__":
    unittest.main()
