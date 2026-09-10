from __future__ import annotations

import unittest

from nioh3_scroll_editor.effect_generation_tables import (
    SCROLL_ITEM_MODE,
    SCROLL_RECORD_TYPES,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.r4_finalizer_reference import Lcg32


class EffectGenerationTableIndexTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.tables = load_default_effect_generation_tables()

    def test_indexes_all_five_scroll_item_rows(self) -> None:
        self.assertEqual(
            tuple(self.tables.items_by_record_type),
            SCROLL_RECORD_TYPES,
        )
        self.assertEqual(
            [self.tables.item(record_type).row_index for record_type in SCROLL_RECORD_TYPES],
            [3308, 3309, 3310, 3311, 3312],
        )
        self.assertTrue(
            all(
                self.tables.item(record_type).mode == SCROLL_ITEM_MODE
                for record_type in SCROLL_RECORD_TYPES
            )
        )

    def test_known_native_effect_group_and_category_vectors(self) -> None:
        expected = {
            0x2B06: (1170, 0xB51B, 137, 13),
            0x92E0: (1117, 0x84D4, 159, 9),
            0x3F41: (1112, 0x7AD1, 797, 6),
            0x4647: (1087, 0xB6A5, 1085, 3),
            0x3E7A: (1115, 0xD405, 234, 23),
            0xBABD: (269, 0xA1B1, 386, 12),
        }
        for effect_id, vector in expected.items():
            with self.subTest(effect_id=f"0x{effect_id:04X}"):
                effect = self.tables.effect(effect_id)
                group = self.tables.group_for_effect(effect_id)
                self.assertEqual(
                    (effect.row_index, effect.group_key, group.row_index, group.category_key),
                    vector,
                )

    def test_mode12_category_capacity_vector_matches_native_builder(self) -> None:
        capacities = self.tables.category_capacities(
            record_type=0xE604,
            rarity=5,
        )
        self.assertEqual(len(capacities), 32)
        self.assertEqual(capacities[0], 0)
        self.assertEqual(capacities[3], 1)
        self.assertEqual(capacities[6], 1)
        self.assertEqual(capacities[12], 1)
        self.assertEqual(capacities[13], 6)
        self.assertEqual(capacities[26], 1)
        self.assertEqual(capacities[31], 1)
        self.assertEqual(sum(capacities), 21)

    def test_mode12_category_lottery_triplets_match_native_rows(self) -> None:
        category_13 = self.tables.category(13)
        self.assertEqual(
            (
                category_13.mode12_lottery_weight,
                category_13.mode12_capacity,
                category_13.mode12_count_multiplier_key,
            ),
            (45, 6, 4),
        )
        self.assertAlmostEqual(
            self.tables.category_count_multiplier(4, 0),
            100.0,
        )
        self.assertAlmostEqual(
            self.tables.category_count_multiplier(4, 4),
            6.25,
        )

    def test_effect_category_resolves_into_capacity_vector(self) -> None:
        capacities = self.tables.category_capacities(
            record_type=0xE604,
            rarity=5,
        )
        for effect_id in (0x2B06, 0x92E0, 0x3F41, 0x4647, 0x3E7A, 0xBABD):
            category = self.tables.category_for_effect(effect_id)
            self.assertGreater(capacities[category.category_key], 0)

    def test_group_equality_and_mask_intersection_reject_candidates(self) -> None:
        self.assertTrue(self.tables.effects_conflict(0x2B06, 0x2B06))
        self.assertTrue(self.tables.effects_conflict(0xFEBC, 0xB52E))
        self.assertFalse(self.tables.effects_conflict(0x2B06, 0x4647))

    def test_compatibility_checks_existing_and_special_effects(self) -> None:
        self.assertTrue(
            self.tables.is_compatible(
                0x2B06,
                existing_effect_ids=(0x4647, 0x3F41),
                special_effect_id=0xBABD,
            )
        )
        self.assertFalse(
            self.tables.is_compatible(0xFEBC, existing_effect_ids=(0xB52E,))
        )

    def test_native_ng3_rarity5_weight_vectors(self) -> None:
        expected = {
            0x2B06: 6000,
            0x92E0: 20000,
            0x3F41: 20000,
            0x4647: 20000,
            0x3E7A: 20000,
            0xBABD: 0,
        }
        for effect_id, weight in expected.items():
            with self.subTest(effect_id=f"0x{effect_id:04X}"):
                self.assertEqual(
                    self.tables.native_effect_weight(
                        effect_id,
                        record_type=0xE604,
                        rarity=5,
                        playthrough=3,
                    ),
                    weight,
                )

    def test_restricted_slot_uses_native_0x29_selector(self) -> None:
        self.assertEqual(
            self.tables.native_effect_weight(
                0x2B06,
                record_type=0xE604,
                rarity=5,
                playthrough=3,
                restricted_destination_slot=True,
            ),
            0,
        )

    def test_candidate_context_gate_matches_ordinary_scroll_generation(self) -> None:
        self.assertTrue(
            self.tables.candidate_context_allowed(
                0x2B06,
                record_type=0xE604,
            )
        )
        self.assertFalse(
            self.tables.candidate_context_allowed(
                0xBABD,
                record_type=0xE604,
            )
        )

    def test_promoted_candidate_pool_uses_normalization_flag(self) -> None:
        candidates = self.tables.weighted_candidate_pool(
            record_type=0xE604,
            rarity=5,
            playthrough=3,
            destination_category_and_flags=13,
            destination_effect_flags=0x04,
            remaining_category_capacities=self.tables.category_capacities(
                record_type=0xE604,
                rarity=5,
            ),
            special_effect_id=0xBABD,
        )
        weights = {
            candidate.effect.effect_id: candidate.weight for candidate in candidates
        }
        self.assertEqual(weights[0x2B06], 6000)
        self.assertNotIn(0x92E0, weights)

    def test_ordinary_candidate_pool_respects_requested_category(self) -> None:
        candidates = self.tables.weighted_candidate_pool(
            record_type=0xE604,
            rarity=5,
            playthrough=3,
            destination_category_and_flags=9,
            destination_effect_flags=0,
            remaining_category_capacities=self.tables.category_capacities(
                record_type=0xE604,
                rarity=5,
            ),
            special_effect_id=0xBABD,
        )
        weights = {
            candidate.effect.effect_id: candidate.weight for candidate in candidates
        }
        self.assertEqual(weights[0x92E0], 20000)
        self.assertNotIn(0x2B06, weights)

    def test_empty_category_capacity_removes_all_candidates(self) -> None:
        candidates = self.tables.weighted_candidate_pool(
            record_type=0xE604,
            rarity=5,
            playthrough=3,
            destination_category_and_flags=9,
            destination_effect_flags=0,
            remaining_category_capacities=(0,) * 32,
            special_effect_id=0xBABD,
        )
        self.assertEqual(candidates, ())

    def test_candidate_lottery_and_rarity_roll_have_exact_rng_order(self) -> None:
        capacities = self.tables.category_capacities(
            record_type=0xE604,
            rarity=5,
        )
        candidates = self.tables.weighted_candidate_pool(
            record_type=0xE604,
            rarity=5,
            playthrough=3,
            destination_category_and_flags=9,
            destination_effect_flags=0,
            remaining_category_capacities=capacities,
            special_effect_id=0xBABD,
        )
        rng = Lcg32(1)
        selected = self.tables.select_weighted_candidate(candidates, rng=rng)
        self.assertIsNotNone(selected)
        self.assertEqual(selected.effect.effect_id, 0xB393)
        self.assertEqual(self.tables.roll_effect_percentile(rarity=5, rng=rng), 94)
        self.assertEqual(rng.state, 0xC35937CC)

    def test_fixed_roll_mode_returns_maximum_without_consuming_rng(self) -> None:
        rng = Lcg32(0x12345678)
        self.assertEqual(
            self.tables.roll_effect_percentile(
                rarity=5,
                rng=rng,
                use_fixed_maximum=True,
            ),
            100,
        )
        self.assertEqual(rng.state, 0x12345678)

    def test_destination_flag_0x40_fails_closed(self) -> None:
        with self.assertRaisesRegex(NotImplementedError, "0x572C20"):
            self.tables.weighted_candidate_pool(
                record_type=0xE604,
                rarity=5,
                playthrough=3,
                destination_category_and_flags=0,
                destination_effect_flags=0x40,
                remaining_category_capacities=self.tables.category_capacities(
                    record_type=0xE604,
                    rarity=5,
                ),
            )

    def test_optional_multiplier_lookup_uses_native_keys(self) -> None:
        self.assertEqual(self.tables.optional_multiplier(0x0415), 1.0)
        self.assertAlmostEqual(self.tables.optional_multiplier(0xA6D1), 0.1, places=6)

    def test_standard_slot_promotion_reproduces_rng_order(self) -> None:
        rng = Lcg32(1)
        selected = self.tables.select_promoted_slot_indexes(
            record_type=0xE604,
            rarity=5,
            rng=rng,
        )
        self.assertEqual(selected, (0,))
        self.assertEqual(rng.state, 0x79D76079)

    def test_slot_promotion_skips_native_effect_flag_0x01_0x02(self) -> None:
        rng = Lcg32(1)
        selected = self.tables.select_promoted_slot_indexes(
            record_type=0xE604,
            rarity=5,
            rng=rng,
            effect_flags=(1, 0, 0, 0, 0, 0, 0),
        )
        self.assertEqual(selected, (2,))

    def test_low_type_class_does_not_consume_shuffle_draws(self) -> None:
        rng = Lcg32(1)
        selected = self.tables.select_promoted_slot_indexes(
            record_type=0x516D,
            rarity=5,
            rng=rng,
        )
        self.assertEqual(selected, ())
        self.assertEqual(rng.state, 0x00010DCE)

    def test_failed_promotion_trial_does_not_consume_shuffle_draws(self) -> None:
        rng = Lcg32(0xFFFFFFFF)
        selected = self.tables.select_promoted_slot_indexes(
            record_type=0xE604,
            rarity=5,
            rng=rng,
        )
        self.assertEqual(selected, ())
        self.assertEqual(rng.state, 0xFFFEF234)

    def test_unknown_effect_fails_closed(self) -> None:
        with self.assertRaisesRegex(KeyError, "unknown effect ID"):
            self.tables.effect(0xDEADBEEF)


if __name__ == "__main__":
    unittest.main()
