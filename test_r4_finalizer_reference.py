from __future__ import annotations

import json
from pathlib import Path
import unittest

from nioh3_scroll_editor.r4_finalizer_reference import (
    Candidate,
    EffectRow,
    EffectSlot,
    GroupRow,
    Lcg32,
    completion_loop,
    derive_finalizer_rng_seed,
    finalizer_discard_count,
    make_finalizer_rng,
    progress_bucket,
    roll_percentile,
    weighted_select_inclusive,
)

ROOT = Path(__file__).resolve().parent
EVIDENCE = json.loads(
    (ROOT / "research" / "test_vectors" / "r4_seed_183696634_records_sanitized.json").read_text(
        encoding="utf-8"
    )
)
STAGE = bytes.fromhex(EVIDENCE["stage_one_record_hex"])
FINAL = bytes.fromhex(EVIDENCE["final_record_hex"])


class ControlledVectorTests(unittest.TestCase):
    def test_layout_and_transition(self) -> None:
        before = EffectSlot.parse(STAGE, 4)
        after = EffectSlot.parse(FINAL, 4)
        self.assertEqual(before.prefix_id, 0xA1B1)
        self.assertEqual(before.raw_id, 0xBABD)
        self.assertEqual(before.value, 0)
        self.assertEqual(before.roll_percent, 0)
        self.assertEqual(before.category, 0x0C)
        self.assertEqual(before.effect_flags, 0x02)
        self.assertEqual(after.prefix_id, 0x89DE)
        self.assertEqual(after.raw_id, 0xAE5A)
        self.assertEqual(after.value, 150)
        self.assertEqual(after.roll_percent, 0x5A)
        self.assertEqual(after.category, 0x0D)
        self.assertEqual(after.effect_flags, 0x04)

    def test_exact_derived_seed_and_discard(self) -> None:
        self.assertEqual(derive_finalizer_rng_seed(STAGE, 4), 0x0B88D880)
        self.assertEqual(finalizer_discard_count(STAGE, 4), 4)
        rng = make_finalizer_rng(STAGE, 4)
        self.assertEqual(rng.state, 0x01F4D84C)
        self.assertEqual(rng.next_u16(), 0xD859)  # category lottery
        self.assertEqual(rng.next_u16(), 0xD60A)  # effect lottery on the normal path
        self.assertEqual(rng.next_u16(), 0xAC02)  # first roll lottery
        self.assertEqual(rng.next_u16(), 0x7F62)  # second roll lottery

    def test_completion_loop_accepts_first_generated_bit04(self) -> None:
        calls: list[int] = []

        def oracle(source: bytes, index: int, reveal: bool) -> bytes:
            calls.append(index)
            candidate = bytearray(source)
            off = 0x34 + index * 0x18 + 0x0E
            if index == 4:
                candidate[off] |= 0x04
            return bytes(candidate)

        result, index = completion_loop(STAGE, oracle)
        self.assertEqual(index, 4)
        self.assertEqual(calls, [1, 2, 3, 4])
        self.assertEqual(EffectSlot.parse(result, 4).effect_flags & 0x04, 0x04)


class MathTests(unittest.TestCase):
    def test_progress_buckets(self) -> None:
        self.assertEqual(progress_bucket(6999), 0)
        self.assertEqual(progress_bucket(7000), 1)
        self.assertEqual(progress_bucket(7999), 1)
        self.assertEqual(progress_bucket(8000), 2)
        self.assertEqual(progress_bucket(8999), 2)
        self.assertEqual(progress_bucket(9000), 3)

    def test_inclusive_weight_bias(self) -> None:
        empty_effect = EffectRow(bytes(0xD8))
        empty_group = GroupRow(bytes(0x70))
        candidates = [
            Candidate(empty_effect, empty_group, 0),
            Candidate(empty_effect, empty_group, 0),
        ]
        # Zero-weight rows are omitted; no candidates is the correct fail-closed result.
        self.assertIsNone(weighted_select_inclusive(candidates, Lcg32(1)))

    def test_roll_percentile_is_deterministic(self) -> None:
        first = roll_percentile(10, 110, Lcg32(0x12345678))
        second = roll_percentile(10, 110, Lcg32(0x12345678))
        self.assertEqual(first, second)
        self.assertGreaterEqual(first, 10)
        self.assertLessEqual(first, 110)


if __name__ == "__main__":
    unittest.main()
