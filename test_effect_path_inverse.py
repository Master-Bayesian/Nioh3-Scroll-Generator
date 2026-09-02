from __future__ import annotations

import unittest

from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    OneWildcardCompositionRequest,
    compile_full_composition_plans,
    compile_one_wildcard_composition_plans,
    seed_satisfies_compiled_plan,
    verify_complete_matches,
    verify_one_wildcard_matches,
)
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
)


class EffectPathInverseTests(unittest.TestCase):
    def _assert_known_seed_round_trip(self, rarity: int, generator) -> None:
        result = generator(1)
        request = FullCompositionRequest(
            rarity,
            result.primary.effect_id,
            tuple(effect.effect_id for effect in result.secondaries),
            None if rarity == 3 else result.grace.effect_id,
        )
        plans = compile_full_composition_plans(request)
        self.assertTrue(any(seed_satisfies_compiled_plan(plan, 1) for plan in plans))
        self.assertEqual(verify_complete_matches(request, (1,)), (1,))

    def test_rarity3_known_seed_round_trip(self) -> None:
        self._assert_known_seed_round_trip(3, generate_ng3_rarity3_effect_sequence)

    def test_rarity4_stage_one_known_seed_round_trip(self) -> None:
        self._assert_known_seed_round_trip(
            4,
            generate_ng3_rarity4_stage_one_effect_sequence,
        )

    def test_rarity5_known_seed_matches_proof_pivot(self) -> None:
        result = generate_ng3_rarity5_effect_sequence(1)
        request = FullCompositionRequest(
            5,
            result.primary.effect_id,
            tuple(effect.effect_id for effect in result.secondaries),
            result.grace.effect_id,
        )
        plan = compile_full_composition_plans(request)[0]
        self.assertEqual(plan.pivot_draw_index, 10)
        self.assertEqual(plan.pivot_state_count, 107_413_504)
        self.assertEqual(len(plan.paths), 24)
        self.assertTrue(seed_satisfies_compiled_plan(plan, 1))

    def test_rarity5_requires_four_secondaries(self) -> None:
        with self.assertRaisesRegex(ValueError, "requires 5 distinct IDs"):
            FullCompositionRequest(5, 0xA051, (0xD40A, 0x34F3, 0x3E7A), 0x6553)

    def test_rarity4_one_wildcard_known_seed_round_trip(self) -> None:
        request = OneWildcardCompositionRequest(
            4,
            (0xA73D, 0x23E8, 0xD40A),
            0x6553,
        )
        plans = compile_one_wildcard_composition_plans(request)
        self.assertEqual(len(plans), 1)
        self.assertGreater(len(plans[0].paths), 0)
        self.assertTrue(any(seed_satisfies_compiled_plan(plan, 2) for plan in plans))
        self.assertEqual(verify_one_wildcard_matches(request, (2,)), (2,))


if __name__ == "__main__":
    unittest.main()
