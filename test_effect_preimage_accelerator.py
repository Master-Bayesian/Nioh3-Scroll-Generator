from __future__ import annotations

from pathlib import Path
import unittest

from nioh3_scroll_editor.app import (
    _complete_preimage_request,
    collect_offline_rarity5_search_batch,
)
from nioh3_scroll_editor.effect_seed_solver import EffectSeedRequest
from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    compile_full_composition_plans,
    verify_complete_matches,
)
from nioh3_scroll_editor.effect_preimage_accelerator import (
    AMD_VENDOR_ID,
    collect_effect_preimage_matches_d3d11,
    d3d11_effect_acceleration_available,
    plan_trial_for_seed,
)
from nioh3_scroll_editor.effect_sequence import generate_ng3_rarity5_effect_sequence
from nioh3_scroll_editor.grace_map import load_grace_output_map


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

    def test_rarity4_final_request_does_not_use_stage_one_inverse(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0xB613,)),
            required_secondary_ids=frozenset((0x4647, 0xD411, 0x3F41)),
            grace_effect_id=0x6553,
        )
        self.assertIsNone(_complete_preimage_request(request))

    def test_packaged_app_includes_directcompute_preimage_backend(self) -> None:
        spec = (
            Path(__file__).parent / "packaging" / "Nioh3ScrollGenerator.spec"
        ).read_text(encoding="utf-8")
        self.assertIn("nioh3_effect_preimage_accelerator.dll", spec)


if __name__ == "__main__":
    unittest.main()
