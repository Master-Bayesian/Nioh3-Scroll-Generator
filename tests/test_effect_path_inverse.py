from __future__ import annotations

import unittest

from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    OneWildcardCompositionRequest,
    U16Run,
    compile_full_composition_plans,
    compile_ng3_rarity3_primary_pivot_families,
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
from nioh3_seed_math import state_after_draw_from_seed


# The parity gate's partial-effect fixture
# (``tests/migration/test_search_worker_parity.py``): the shipped full-family
# route served this rarity-3 primary search from the whole 2**32 Seed family.
PARITY_PRIMARY_EFFECT = 60020


def _seed_state_in_family(family, seed: int) -> bool:
    value = state_after_draw_from_seed(seed, family.pivot_draw_index) >> 16
    return any(run.start <= value <= run.end for run in family.pivot_allowed_u16)


def _family_promotion_runs(family, seed: int) -> bool:
    value = state_after_draw_from_seed(seed, family.promotion_draw_index) >> 16
    return any(
        run.start <= value <= run.end for run in family.promotion_u16_runs
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

    def test_rarity3_primary_pivot_family_matches_the_parity_fixture(self) -> None:
        """The named rarity-3 primary owns a finite draw-9 preimage.

        Effect 60020 is only drawable through the promoted source slot 0, so
        its compiled family keeps the recorded 6,553 high-16 states instead of
        the 65,536 states of a full draw pivot.
        """

        families = compile_ng3_rarity3_primary_pivot_families(
            frozenset((PARITY_PRIMARY_EFFECT,))
        )
        self.assertEqual(len(families), 1)
        family = families[0]
        self.assertEqual(family.promoted_states, (0,))
        self.assertTrue(family.requires_promotion)
        self.assertEqual(family.pivot_draw_index, 9)
        self.assertEqual(family.pivot_allowed_u16, (U16Run(52430, 58982),))
        self.assertEqual(family.promotion_draw_index, 1)
        self.assertEqual(family.promotion_u16_runs, (U16Run(0, 6553),))
        self.assertEqual(family.pivot_state_count, 429_457_408)
        self.assertLess(family.pivot_state_count, 2**32)

    def test_rarity3_primary_pivot_covers_every_promotion_state(self) -> None:
        """The promotion intervals partition the draw-1 states and cover both outcomes.

        A Seed whose exact primary is one of the requested IDs must land in the
        family that owns its promotion outcome, so the compiled space can be
        enumerated once per Seed without a per-hit Python promotion filter.
        """

        generated = {
            seed: generate_ng3_rarity3_effect_sequence(seed)
            for seed in range(1, 513)
        }
        requested = frozenset(
            result.primary.effect_id for result in generated.values()
        )
        families = compile_ng3_rarity3_primary_pivot_families(requested)
        self.assertEqual(len(families), 2)
        un_promoted, promoted = families
        self.assertFalse(un_promoted.requires_promotion)
        self.assertTrue(promoted.requires_promotion)
        self.assertEqual(un_promoted.pivot_draw_index, 2)
        self.assertEqual(promoted.pivot_draw_index, 9)
        self.assertEqual(promoted.promotion_u16_runs[0].start, 0)
        self.assertEqual(un_promoted.promotion_u16_runs[0].end, 0xFFFF)
        self.assertEqual(
            promoted.promotion_u16_runs[0].end + 1,
            un_promoted.promotion_u16_runs[0].start,
        )
        for seed, result in generated.items():
            with self.subTest(seed=seed):
                self.assertIn(result.primary.effect_id, requested)
                owners = tuple(
                    family
                    for family in families
                    if _family_promotion_runs(family, seed)
                )
                self.assertEqual(len(owners), 1)
                self.assertTrue(_seed_state_in_family(owners[0], seed))


if __name__ == "__main__":
    unittest.main()
