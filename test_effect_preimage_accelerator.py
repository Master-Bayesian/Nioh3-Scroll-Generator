from __future__ import annotations

from itertools import islice
from pathlib import Path
import unittest

from nioh3_scroll_editor.app import (
    _complete_preimage_request,
    _complete_preimage_requests,
    _legal_complete_preimage_layouts,
    _one_wildcard_preimage_request,
    collect_offline_ng3_search_batch,
    collect_offline_rarity5_search_batch,
)
from nioh3_scroll_editor.effect_seed_solver import (
    EffectSeedRequest,
    fixed_draw_constraints,
    iter_effect_seed_candidates,
)
from nioh3_scroll_editor.effect_batch_filter import (
    match_partial_effect_constraints_batch,
)
from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    OneWildcardCompositionRequest,
    compile_full_composition_plans,
    compile_one_wildcard_composition_plans,
    seed_satisfies_compiled_plan,
    verify_complete_matches,
)
from nioh3_scroll_editor.effect_preimage_accelerator import (
    AMD_VENDOR_ID,
    collect_effect_preimage_matches_d3d11,
    collect_fixed_draw_pivot_seeds_d3d11,
    d3d11_effect_acceleration_available,
    last_effect_preimage_backend,
    plan_trial_for_seed,
    reset_effect_preimage_backend,
)
import nioh3_scroll_editor.effect_preimage_accelerator as preimage_accelerator
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity4_final_effect_sequence,
    generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
)
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.joint_solver import _permuted_values, choose_pivot
from nioh3_seed_math import state_after_draw_from_seed


def _native_pivot_trial_for_seed(seed: int, pivot) -> int:
    values = _permuted_values(pivot.allowed_u16)
    state = state_after_draw_from_seed(seed, pivot.draw_index)
    low_index = ((state & 0xFFFF) * pow(0x9E37, -1, 0x10000)) & 0xFFFF
    rotation = low_index % len(values)
    bucket_index = (values.index(state >> 16) - rotation) % len(values)
    return low_index * len(values) + bucket_index + 1


class EffectPreimageAcceleratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        result = generate_ng3_rarity5_effect_sequence(1)
        cls.request = FullCompositionRequest(
            5,
            result.primary.effect_id,
            tuple(effect.effect_id for effect in result.secondaries),
            result.grace.effect_id,
        )
        cls.plan = compile_full_composition_plans(cls.request)[0]

    def _assert_vendor_round_trip(self, vendor_id: int) -> None:
        trial = plan_trial_for_seed(self.plan, 1)
        matches = collect_effect_preimage_matches_d3d11(
            self.plan,
            start_trial=trial - 64,
            stop_trial=trial + 65,
            vendor_id=vendor_id,
        )
        self.assertIsNotNone(matches)
        assert matches is not None
        self.assertIn((1, trial), matches)
        self.assertIn(1, verify_complete_matches(
            self.request,
            (seed for seed, _trial in matches),
        ))

    def test_partial_effect_forward_filter_matches_exact_sequences(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seeds = tuple(range(1, 513))
        for rarity, generator, mapping in (
            (3, generate_ng3_rarity3_effect_sequence, None),
            (5, generate_ng3_rarity5_effect_sequence, load_grace_output_map(rarity=5)),
        ):
            with self.subTest(rarity=rarity):
                sequences = tuple(generator(seed) for seed in seeds)
                required = sequences[0].secondaries[0].effect_id
                alternative = sequences[1].secondaries[0].effect_id
                result = match_partial_effect_constraints_batch(
                    seeds,
                    playthrough=3,
                    rarity=rarity,
                    primary_effect_ids=frozenset(),
                    required_secondary_ids=frozenset((required,)),
                    required_secondary_id_groups=(frozenset((alternative,)),),
                    special_mapping=mapping,
                )
                self.assertIsNotNone(result)
                assert result is not None
                expected = []
                for sequence in sequences:
                    actual = {
                        effect.effect_id
                        for effect in (sequence.primary, *sequence.secondaries)
                    }
                    expected.append(
                        (1 if required in actual else 0)
                        | (2 if alternative in actual else 0)
                    )
                self.assertEqual(result.masks, tuple(expected))

    def test_rarity4_stage_filter_allows_one_finalizer_rewrite(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seeds = tuple(range(1, 513))
        sequences = tuple(
            generate_ng3_rarity4_stage_one_effect_sequence(seed)
            for seed in seeds
        )
        required = tuple(effect.effect_id for effect in sequences[0].effects[:3])
        result = match_partial_effect_constraints_batch(
            seeds,
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset(required),
            required_secondary_id_groups=(),
            special_mapping=load_grace_output_map(rarity=4),
        )
        self.assertIsNotNone(result)
        assert result is not None
        target = (1 << len(required)) - 1
        expected = []
        for sequence in sequences:
            actual = {effect.effect_id for effect in sequence.effects[:4]}
            matched = sum(effect_id in actual for effect_id in required)
            expected.append(target if matched >= len(required) - 1 else 0)
        self.assertEqual(result.masks, tuple(expected))

    def test_rarity4_generic_prefilter_keeps_finalizer_generated_effect(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seed = 3
        stage = generate_ng3_rarity4_stage_one_effect_sequence(seed)
        final = generate_ng3_rarity4_final_effect_sequence(seed)
        self.assertNotIn(0xDFF0, {effect.effect_id for effect in stage.effects})
        self.assertIn(0xDFF0, {effect.effect_id for effect in final.effects})
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0x6CE3, 0x47D2)),
            required_secondary_id_groups=(frozenset((0xDFF0, 0xB613)),),
            grace_effect_id=0x6553,
        )
        mapping = load_grace_output_map(rarity=4)
        constraints = fixed_draw_constraints(
            request,
            grace_mapping=mapping,
            replay_primary=True,
        )
        pivot = choose_pivot(constraints)
        target_trial = _native_pivot_trial_for_seed(seed, pivot)
        result = collect_offline_ng3_search_batch(
            request,
            grace_mapping=mapping,
            level=180,
            result_count=1,
            max_trials_per_batch=129,
            start_after_trial=max(0, target_trial - 64),
        )
        self.assertEqual(tuple(candidate.seed for candidate in result.candidates), (seed,))

    def test_rarity4_single_group_filter_matches_finalizer_output(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seeds = tuple(range(1, 1025))
        target_effect_id = 0xDFF0
        result = match_partial_effect_constraints_batch(
            seeds,
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset((target_effect_id,)),
            required_secondary_id_groups=(),
            special_mapping=load_grace_output_map(rarity=4),
        )
        self.assertIsNotNone(result)
        assert result is not None
        expected = tuple(
            int(
                target_effect_id
                in {
                    effect.effect_id
                    for effect in generate_ng3_rarity4_final_effect_sequence(seed).effects
                }
            )
            for seed in seeds
        )
        self.assertEqual(result.target_mask, 1)
        self.assertEqual(result.masks, expected)

    def test_rarity4_gpu_finalizer_matches_distributed_seed_masks(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seeds = tuple(
            ((index * 0x9E3779B1) & 0x0FFFFFFF) or 1
            for index in range(1, 2049)
        )
        sequences = tuple(
            generate_ng3_rarity4_final_effect_sequence(seed) for seed in seeds
        )
        effect_ids = tuple(
            sorted(
                {
                    effect.effect_id
                    for sequence in sequences[:128]
                    for effect in (sequence.primary, *sequence.secondaries)
                }
            )[:32]
        )
        requirement_groups = (
            frozenset(effect_ids[:16]),
            frozenset(effect_ids[16:]),
        )
        result = match_partial_effect_constraints_batch(
            seeds,
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset(),
            required_secondary_id_groups=requirement_groups,
            special_mapping=load_grace_output_map(rarity=4),
        )
        self.assertIsNotNone(result)
        assert result is not None
        expected = tuple(
            sum(
                1 << index
                for index, group in enumerate(requirement_groups)
                if group.intersection(
                    effect.effect_id
                    for effect in (sequence.primary, *sequence.secondaries)
                )
            )
            for sequence in sequences
        )
        self.assertEqual(result.masks, expected)

    def test_default_d3d11_device_round_trip(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        self._assert_vendor_round_trip(0)

    def test_amd_d3d11_device_round_trip_when_present(self) -> None:
        if not d3d11_effect_acceleration_available(vendor_id=AMD_VENDOR_ID):
            self.skipTest("no AMD Direct3D 11 compute adapter")
        self._assert_vendor_round_trip(AMD_VENDOR_ID)

    def test_generic_fixed_draw_directcompute_round_trip(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        seed = 1
        pivot_state = state_after_draw_from_seed(seed, 1)
        draw2 = state_after_draw_from_seed(seed, 2) >> 16
        pivot_low16 = pivot_state & 0xFFFF
        matches = collect_fixed_draw_pivot_seeds_d3d11(
            (pivot_state >> 16,),
            start_index=max(0, pivot_low16 - 64),
            stop_index=min(0x10000, pivot_low16 + 65),
            low16_stride=0x9E37,
            pivot_draw_index=1,
            other_constraints=((2, ((draw2, draw2),)),),
        )
        self.assertIsNotNone(matches)
        assert matches is not None
        self.assertIn((seed, pivot_low16 + 1), matches)

    def test_unknown_hardware_vendor_still_reports_directcompute(self) -> None:
        original = preimage_accelerator._last_d3d11_vendor_id
        try:
            preimage_accelerator._last_d3d11_vendor_id = 0x1414
            self.assertEqual(last_effect_preimage_backend(), "d3d11_other")
        finally:
            preimage_accelerator._last_d3d11_vendor_id = original

    def test_generic_effect_solver_automatically_uses_directcompute(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        mapping = load_grace_output_map(rarity=5)
        generated = generate_ng3_rarity5_effect_sequence(1)
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=generated.grace.effect_id,
        )
        constraints = fixed_draw_constraints(
            request,
            grace_mapping=mapping,
            replay_primary=True,
        )
        pivot = choose_pivot(constraints)
        pivot_state = state_after_draw_from_seed(1, pivot.draw_index)
        values = _permuted_values(pivot.allowed_u16)
        target_trial = values.index(pivot_state >> 16) * 0x10000 + (
            pivot_state & 0xFFFF
        )
        reset_effect_preimage_backend()
        matches = tuple(
            islice(
                iter_effect_seed_candidates(
                    request,
                    grace_mapping=mapping,
                    effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
                    start_after_trial=max(0, target_trial - 64),
                    max_trials=129,
                    prefer_d3d11_fixed_draw=True,
                ),
                20,
            )
        )
        self.assertIn(1, (match.seed for match in matches))
        self.assertTrue(last_effect_preimage_backend().startswith("d3d11_"))

    def test_product_rarity5_complete_composition_uses_verified_preimage(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        result = collect_offline_rarity5_search_batch(
            EffectSeedRequest(
                playthrough=3,
                rarity=5,
                primary_effect_ids=frozenset((0xA051,)),
                required_secondary_ids=frozenset(
                    (0xD40A, 0x34F3, 0x3E7A, 0xAE5A)
                ),
                grace_effect_id=0x6553,
            ),
            grace_mapping=load_grace_output_map(rarity=5),
            level=180,
            result_count=20,
            max_trials_per_batch=200_000_000,
        )
        self.assertEqual(
            tuple(sorted(candidate.seed for candidate in result.candidates)),
            (1, 7_898_609, 25_934_837, 29_849_823, 33_957_113,
             48_135_696, 76_175_780, 94_647_132, 250_107_693),
        )

    def test_rarity4_final_request_does_not_use_incomplete_stage_inverse(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0xB613,)),
            required_secondary_ids=frozenset((0x4647, 0xD411, 0x3F41)),
            grace_effect_id=0x6553,
        )
        self.assertIsNone(_complete_preimage_request(request))

    def test_rarity4_complete_request_uses_finalizer_aware_prefilter(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0xA73D, 0x23E8, 0xD40A, 0x28C4)),
            grace_effect_id=0x6553,
        )
        self.assertEqual(_complete_preimage_requests(request), ())

    def test_generic_search_respects_one_batch_trial_budget(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            grace_effect_id=0xCE68,
        )
        result = collect_offline_ng3_search_batch(
            request,
            grace_mapping=load_grace_output_map(rarity=4),
            level=180,
            result_count=20,
            max_trials_per_batch=1,
        )
        self.assertLessEqual(result.next_start_after_trial, 1)

    def test_rarity4_unrestricted_primary_directcompute_round_trip(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        mapping = load_grace_output_map(rarity=4)
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0xA73D, 0x23E8, 0xD40A, 0x28C4)),
            grace_effect_id=0x6553,
        )
        self.assertIsNone(_legal_complete_preimage_layouts(
            request,
            grace_mapping=mapping,
        ))
        constraints = fixed_draw_constraints(
            request,
            grace_mapping=mapping,
            replay_primary=True,
        )
        pivot = choose_pivot(constraints)
        target_trial = _native_pivot_trial_for_seed(2, pivot)
        result = collect_offline_ng3_search_batch(
            request,
            grace_mapping=mapping,
            level=180,
            result_count=1,
            max_trials_per_batch=129,
            start_after_trial=target_trial - 64,
        )
        self.assertEqual(tuple(candidate.seed for candidate in result.candidates), (2,))

    def test_rarity4_one_wildcard_directcompute_round_trip(self) -> None:
        if not d3d11_effect_acceleration_available():
            self.skipTest("no Direct3D 11 compute adapter")
        mapping = load_grace_output_map(rarity=4)
        product_request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0xA73D, 0x23E8, 0xD40A)),
            grace_effect_id=0x6553,
        )
        self.assertIsNone(_one_wildcard_preimage_request(product_request))
        constraints = fixed_draw_constraints(
            product_request,
            grace_mapping=mapping,
            replay_primary=True,
        )
        pivot = choose_pivot(constraints)
        target_trial = _native_pivot_trial_for_seed(2, pivot)
        result = collect_offline_ng3_search_batch(
            product_request,
            grace_mapping=mapping,
            level=180,
            result_count=1,
            max_trials_per_batch=129,
            start_after_trial=target_trial - 64,
        )
        self.assertEqual(tuple(candidate.seed for candidate in result.candidates), (2,))

    def test_packaged_app_includes_directcompute_preimage_backend(self) -> None:
        spec = (
            Path(__file__).parent / "packaging" / "Nioh3ScrollGenerator.spec"
        ).read_text(encoding="utf-8")
        self.assertIn("nioh3_effect_preimage_accelerator.dll", spec)


if __name__ == "__main__":
    unittest.main()
