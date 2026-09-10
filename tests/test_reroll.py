from __future__ import annotations

import struct
import tempfile
import unittest
from pathlib import Path

from nioh3_scroll_editor.effect_generation_tables import (
    EMPTY_EFFECT_ID,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.reroll import (
    advance_reroll_counter,
    derive_reroll_rng_seed,
    predict_reroll_candidates,
    simulate_accept_candidate,
)
from research.analyze_scroll_reroll_capture import analyze_capture


def _control_record() -> bytes:
    tables = load_default_effect_generation_tables()
    record = bytearray(0xE8)
    struct.pack_into("<H", record, 0x00, 0xE604)
    struct.pack_into("<H", record, 0x06, 180)
    struct.pack_into("<H", record, 0x08, 180)
    struct.pack_into("<H", record, 0x10, 183)
    struct.pack_into("<H", record, 0x12, 183)
    struct.pack_into("<I", record, 0x20, 0x9E3779B2)
    record[0x30] = 4
    effect_ids = (0x23E8, 0x6BEB, 0xB82B, 0xF9BE, 0x71F6)
    rolls = (96, 83, 90, 95, 0)
    for index in range(7):
        offset = 0x34 + index * 0x18
        if index >= len(effect_ids):
            struct.pack_into("<I", record, offset + 0x04, EMPTY_EFFECT_ID)
            continue
        effect_id = effect_ids[index]
        group = tables.group_for_effect(effect_id)
        struct.pack_into("<H", record, offset + 0x00, group.group_key)
        struct.pack_into("<I", record, offset + 0x04, effect_id)
        record[offset + 0x0C] = rolls[index]
        record[offset + 0x0D] = group.category_key
    return bytes(record)


class RerollPredictionTests(unittest.TestCase):
    def test_rng_seed_is_displayed_seed_plus_uint16_counter(self) -> None:
        record = bytearray(_control_record())
        struct.pack_into("<H", record, 0x0C, 0x1234)
        self.assertEqual(derive_reroll_rng_seed(record), 0x9E378BE6)

    def test_counter_advance_wraps_like_native_uint16_increment(self) -> None:
        record = bytearray(_control_record())
        struct.pack_into("<H", record, 0x0C, 0xFFFF)
        advanced = advance_reroll_counter(record)
        self.assertEqual(struct.unpack_from("<H", advanced, 0x0C)[0], 0)

    def test_control_vector_matches_recovered_candidate_order(self) -> None:
        prediction = predict_reroll_candidates(
            _control_record(),
            1,
            dynamic_gate_group_keys=(),
        )
        self.assertTrue(prediction.context_complete)
        self.assertEqual(prediction.rng_seed, 0x9E3779B2)
        self.assertEqual(prediction.initial_pool_size, 28)
        self.assertEqual(prediction.initial_total_weight, 476000)
        self.assertEqual(
            [
                (candidate.effect_id, candidate.roll_percent, candidate.resolved_value)
                for candidate in prediction.candidates
            ],
            [
                (0x4CAE, 86, 54),
                (0xA0A7, 92, 345),
                (0xDFF0, 84, 150),
                (0x1355, 95, 58),
                (0x3A8E, 84, 55),
            ],
        )
        self.assertEqual(prediction.final_rng_state, 0x1D28F5E2)

    def test_candidates_are_unique_by_group_and_compatible(self) -> None:
        tables = load_default_effect_generation_tables()
        record = _control_record()
        prediction = predict_reroll_candidates(
            record,
            2,
            dynamic_gate_group_keys=(),
        )
        groups = [candidate.group_key for candidate in prediction.candidates]
        self.assertEqual(len(groups), len(set(groups)))
        existing_ids = (0x23E8, 0x6BEB, 0xF9BE, 0x71F6)
        for candidate in prediction.candidates:
            self.assertTrue(
                tables.is_compatible(
                    candidate.effect_id,
                    existing_effect_ids=existing_ids,
                )
            )

    def test_counter_changes_the_pool_sequence_without_changing_seed(self) -> None:
        record = _control_record()
        first = predict_reroll_candidates(
            record,
            1,
            dynamic_gate_group_keys=(),
        )
        second_record = advance_reroll_counter(record)
        second = predict_reroll_candidates(
            second_record,
            1,
            dynamic_gate_group_keys=(),
        )
        self.assertEqual(first.displayed_seed, second.displayed_seed)
        self.assertEqual(second.reroll_counter, first.reroll_counter + 1)
        self.assertNotEqual(
            [candidate.effect_id for candidate in first.candidates],
            [candidate.effect_id for candidate in second.candidates],
        )

    def test_acceptance_replaces_slot_and_advances_counter(self) -> None:
        record = _control_record()
        prediction = predict_reroll_candidates(
            record,
            1,
            dynamic_gate_group_keys=(),
        )
        selected = prediction.candidates[0]
        updated = simulate_accept_candidate(record, 1, selected)
        offset = 0x34 + 0x18
        self.assertEqual(struct.unpack_from("<I", updated, offset + 4)[0], 0x4CAE)
        self.assertEqual(updated[offset + 0x0C], 86)
        self.assertEqual(struct.unpack_from("<H", updated, 0x0C)[0], 1)

    def test_unspecified_save_gate_is_explicitly_incomplete(self) -> None:
        prediction = predict_reroll_candidates(_control_record(), 1)
        self.assertFalse(prediction.context_complete)
        self.assertEqual(
            prediction.dynamic_gate_group_keys,
            (0x17B4, 0x62CB),
        )

    def test_intermediate_unknown_effect_fails_closed(self) -> None:
        record = bytearray(_control_record())
        struct.pack_into("<I", record, 0x34 + 4, 0xDEADBEEF)
        with self.assertRaisesRegex(ValueError, "known final effect"):
            predict_reroll_candidates(record, 0, dynamic_gate_group_keys=())

    def test_capture_analyzer_recovers_a_matching_dynamic_context(self) -> None:
        record = _control_record()
        prediction = predict_reroll_candidates(
            record,
            1,
            dynamic_gate_group_keys=(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            record_name = "001_candidate_input_record_e8.bin"
            output_name = "002_candidate_output_5_x18.bin"
            (root / record_name).write_bytes(record)
            raw_candidates = bytearray(len(prediction.candidates) * 0x18)
            for index, candidate in enumerate(prediction.candidates):
                offset = index * 0x18
                struct.pack_into("<I", raw_candidates, offset + 4, candidate.effect_id)
                raw_candidates[offset + 0x0C] = candidate.roll_percent
            (root / output_name).write_bytes(raw_candidates)
            (root / "manifest.tsv").write_text(
                "candidate_entry\t0x020C4BD0\t0x1\t2654435762\t0\t1\t0x2\t0\t\t"
                + record_name
                + "\n"
                + "candidate_return\t0x020C4F4F\t0x1\t2654435762\t0\t1\t0x2\t5\t\t"
                + output_name
                + "\n",
                encoding="utf-8",
            )
            analysis = analyze_capture(root)
        self.assertTrue(analysis["all_candidate_vectors_match"])
        comparison = analysis["comparisons"][0]
        self.assertIn([], comparison["matching_dynamic_contexts"])


if __name__ == "__main__":
    unittest.main()
