from __future__ import annotations

from itertools import islice
from pathlib import Path
import unittest

from nioh3_scroll_editor.app import (
    _complete_preimage_request,
    _complete_preimage_requests,
    _legal_complete_preimage_layouts,
    collect_offline_ng3_search_batch,
    collect_offline_rarity5_search_batch,
)
from nioh3_scroll_editor.effect_seed_solver import (
    EffectSeedRequest,
    fixed_draw_constraints,
    iter_effect_seed_candidates,
)
from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    compile_full_composition_plans,
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
from nioh3_scroll_editor.effect_sequence import generate_ng3_rarity5_effect_sequence
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.joint_solver import _permuted_values, choose_pivot
from nioh3_seed_math import state_after_draw_from_seed


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

    def test_rarity4_final_request_uses_stage_one_inverse_with_final_replay(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0xB613,)),
            required_secondary_ids=frozenset((0x4647, 0xD411, 0x3F41)),
            grace_effect_id=0x6553,
        )
        inverse = _complete_preimage_request(request)
        self.assertIsNotNone(inverse)
        assert inverse is not None
        self.assertEqual(inverse.stage_special_effect_id, 0x6553)

    def test_rarity4_unrestricted_primary_expands_complete_assignments(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0xA73D, 0x23E8, 0xD40A, 0x28C4)),
            grace_effect_id=0x6553,
        )
        self.assertEqual(
            {item.primary_effect_id for item in _complete_preimage_requests(request)},
            {0xA73D, 0x23E8, 0xD40A, 0x28C4},
        )

    def test_rarity4_complete_illegal_composition_stops_before_scan(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            required_secondary_ids=frozenset((0xA051, 0xB613, 0xDFF0, 0xEA53)),
            grace_effect_id=0xCE68,
            minimum_roll_percent_by_effect_id=((0xA051, 100),),
        )
        with self.assertRaisesRegex(ValueError, "原生逐槽抽取路径中无解"):
            collect_offline_ng3_search_batch(
                request,
                grace_mapping=load_grace_output_map(rarity=4),
                level=180,
                result_count=20,
                max_trials_per_batch=1,
            )

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
        layouts = _legal_complete_preimage_layouts(
            request,
            grace_mapping=mapping,
        )
        assert layouts is not None
        global_offset = 0
        target_trial = None
        for inverse, family_size in layouts:
            plan_offset = 0
            for plan in compile_full_composition_plans(
                inverse,
                special_mapping=mapping,
            ):
                if seed_satisfies_compiled_plan(plan, 2):
                    target_trial = (
                        global_offset + plan_offset + plan_trial_for_seed(plan, 2)
                    )
                    break
                plan_offset += plan.pivot_state_count
            if target_trial is not None:
                break
            global_offset += family_size
        self.assertIsNotNone(target_trial)
        assert target_trial is not None
        result = collect_offline_ng3_search_batch(
            request,
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
