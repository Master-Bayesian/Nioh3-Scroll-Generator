from __future__ import annotations

import hashlib
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE
from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.auxiliary_feasibility import (
    EnemyKeyRequirement,
    analyze_enemy_feasibility,
)
from nioh3_scroll_editor.effect_seed_solver import (
    EffectSeedRequest,
    OfflineEffectReplayUnavailable,
    collect_effect_seed_page,
    iter_effect_seed_candidates,
    merge_intersection_reports,
    validate_effect_request_feasibility,
    _verify_effect_sequence,
)
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.joint_solver import U16Runs
from nioh3_scroll_editor.primary_map import (
    PrimaryFirstDrawOutputMap,
    PrimaryOutputMap,
    load_primary_map,
    save_primary_map,
)


GRACE = 0x6553
PRIMARY = 0x47BC
SECONDARY = 0x190A
FINGERPRINT = hashlib.sha256(b"test-save-context").hexdigest()


def complete_runs() -> U16Runs:
    return U16Runs.from_ranges(((0, 0xFFFF),))


def make_final_record(seed: int, *, rarity: int = 5) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, 0xE604)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30] = record[0x31] = rarity
    effects = (PRIMARY, SECONDARY, 0x2B06, 0xB613, 0x4647, GRACE)
    for index, effect_id in enumerate(effects):
        struct.pack_into(
            "<6I",
            record,
            EFFECT_START + index * EFFECT_STRIDE,
            index + 1,
            effect_id,
            100 + index,
            200 + index,
            0,
            0,
        )
    struct.pack_into("<I", record, EFFECT_START + 6 * EFFECT_STRIDE + 4, 0xFFFFFFFF)
    return bytes(record)


class PrimaryMapPersistenceTests(unittest.TestCase):
    def test_roundtrips_conditioned_map_with_context_gate(self) -> None:
        mapping = PrimaryOutputMap(
            game_version="2.00.02",
            record_type=0xE604,
            rarity=5,
            playthrough="current-loaded-state",
            grace_effect_id=GRACE,
            grace_effect_slot=6,
            draw_index=2,
            effects=((PRIMARY, complete_runs()),),
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "primary.json"
            save_primary_map(path, mapping, context_fingerprint=FINGERPRINT)
            loaded = load_primary_map(
                path,
                expected_context_fingerprint=FINGERPRINT.upper(),
            )
            self.assertEqual(loaded, mapping)
            with self.assertRaisesRegex(ValueError, "different save context"):
                load_primary_map(path, expected_context_fingerprint="0" * 64)

    def test_roundtrips_first_draw_map(self) -> None:
        mapping = PrimaryFirstDrawOutputMap(
            game_version="2.00.02",
            record_type=0x1E82,
            rarity=5,
            category=1,
            draw_index=1,
            effects=((PRIMARY, complete_runs()),),
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "primary-first.json"
            save_primary_map(path, mapping, context_fingerprint=FINGERPRINT)
            self.assertEqual(load_primary_map(path), mapping)


class GameClosedEffectSeedSolverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.grace_mapping = load_grace_output_map(rarity=5)
        cls.primary_mapping = PrimaryOutputMap(
            game_version="2.00.02",
            record_type=cls.grace_mapping.record_type,
            rarity=5,
            playthrough=cls.grace_mapping.playthrough,
            grace_effect_id=GRACE,
            grace_effect_slot=cls.grace_mapping.effect_slot,
            draw_index=2,
            effects=((PRIMARY, complete_runs()),),
        )

    def test_yields_exact_grace_primary_seed_without_game_process(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                primary_mapping=self.primary_mapping,
                max_trials=0x20000,
            )
        )
        self.assertEqual(candidate.fixed_draws, (("grace", 1), ("primary", 2)))
        self.assertGreater(candidate.seed, 0)
        self.assertIsNone(candidate.record)

    def test_collects_non_overlapping_candidate_pages(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
        )
        first = collect_effect_seed_page(
            request,
            page_size=3,
            grace_mapping=self.grace_mapping,
            primary_mapping=self.primary_mapping,
            max_trials=0x20000,
        )
        second = collect_effect_seed_page(
            request,
            page_size=3,
            grace_mapping=self.grace_mapping,
            primary_mapping=self.primary_mapping,
            start_after_trial=first.next_start_after_trial,
            max_trials=0x20000,
        )
        self.assertEqual(len(first.candidates), 3)
        self.assertEqual(len(second.candidates), 3)
        self.assertGreater(
            second.candidates[0].pivot_trial,
            first.candidates[-1].pivot_trial,
        )
        self.assertTrue(
            {candidate.seed for candidate in first.candidates}.isdisjoint(
                candidate.seed for candidate in second.candidates
            )
        )

    def test_rejects_invalid_page_size(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
        )
        with self.assertRaisesRegex(ValueError, "page_size"):
            collect_effect_seed_page(
                request,
                page_size=0,
                grace_mapping=self.grace_mapping,
            )

    def test_rarity5_rejects_second_promoted_only_effect_immediately(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=0x71F6,
            primary_effect_ids=frozenset((0xB613,)),
            required_secondary_ids=frozenset((0x23E8,)),
        )
        with self.assertRaisesRegex(ValueError, "只有一个升格/深奥槽"):
            validate_effect_request_feasibility(request)

    def test_rarity4_allows_two_promoted_effects_after_finalization(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            grace_effect_id=0x71F6,
            primary_effect_ids=frozenset((0xB613,)),
            required_secondary_ids=frozenset((0x23E8,)),
        )
        validate_effect_request_feasibility(request)
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_certified_effect_sequence,
        )

        result = generate_ng3_certified_effect_sequence(76_732_971, rarity=4)
        self.assertEqual(result.primary.effect_id, 0xB613)
        self.assertIn(0x23E8, {effect.effect_id for effect in result.secondaries})
        self.assertEqual(result.grace.effect_id, 0x71F6)

    def test_rarity3_unconstrained_primary_accepts_four_ordinary_effects(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=3,
            required_secondary_ids=frozenset((0xEA74, 0x4035, 0x9A3D, 0x2EFC)),
        )
        validate_effect_request_feasibility(request)

    def test_rarity4_grace_allows_one_selected_effect_to_be_primary(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            grace_effect_id=0x71F6,
            required_secondary_ids=frozenset((0xB613, 0x23E8, 0x34F3, 0x600F)),
        )
        validate_effect_request_feasibility(request)

    def test_rarity4_without_grace_accepts_five_ordinary_effects(self) -> None:
        validate_effect_request_feasibility(
            EffectSeedRequest(
                playthrough=3,
                rarity=4,
                required_secondary_ids=frozenset(
                    (0xB613, 0x4647, 0xD411, 0x3F41, 0x6AAF)
                ),
            )
        )

    def test_exact_replay_unconstrained_primary_matches_any_ordinary_slot(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_certified_effect_sequence,
        )

        result = _verify_effect_sequence(
            1,
            EffectSeedRequest(
                playthrough=3,
                rarity=3,
                required_secondary_ids=frozenset((0xEA74, 0x4035, 0x9A3D, 0x2EFC)),
            ),
            lambda seed: generate_ng3_certified_effect_sequence(seed, rarity=3),
        )
        self.assertIsNotNone(result)

    def test_rejects_native_conflict_groups_before_search(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0x4647,)),
            required_secondary_ids=frozenset((0x600F,)),
        )
        with self.assertRaisesRegex(ValueError, "原生冲突组"):
            validate_effect_request_feasibility(request)

    def test_category_capacity_error_names_every_conflicting_selection(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0xCE1A,)),
            required_secondary_ids=frozenset((0xB82B,)),
        )
        with self.assertRaises(ValueError) as captured:
            validate_effect_request_feasibility(request)
        message = str(captured.exception)
        self.assertIn("原生类别 0x03", message)
        self.assertIn("武技伤害 [0xCE1A]", message)
        self.assertIn("敌人精力耗尽时赋予受到伤害增加 [0xB82B]", message)
        self.assertIn("最多容纳 1 个", message)
        self.assertIn("请至少移除 1 个", message)

    def test_rejects_unknown_effect_before_search(self) -> None:
        with self.assertRaisesRegex(ValueError, "不在当前原生参数表"):
            validate_effect_request_feasibility(
                EffectSeedRequest(
                    playthrough=3,
                    rarity=4,
                    primary_effect_ids=frozenset((0xDEADBEEF,)),
                )
            )

    def test_exact_replay_enforces_selected_effect_roll_threshold(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
        )

        sequence = generate_ng3_rarity5_effect_sequence(1)
        effect_id = sequence.primary.effect_id
        minimum = sequence.primary.roll_percent
        accepted = _verify_effect_sequence(
            1,
            EffectSeedRequest(
                playthrough=3,
                rarity=5,
                primary_effect_ids=frozenset((effect_id,)),
                minimum_roll_percent_by_effect_id=((effect_id, minimum),),
            ),
            generate_ng3_rarity5_effect_sequence,
        )
        rejected = _verify_effect_sequence(
            1,
            EffectSeedRequest(
                playthrough=3,
                rarity=5,
                primary_effect_ids=frozenset((effect_id,)),
                minimum_roll_percent_by_effect_id=(
                    (effect_id, min(100, minimum + 1)),
                ),
            ),
            generate_ng3_rarity5_effect_sequence,
        )

        self.assertIsNotNone(accepted)
        if minimum < 100:
            self.assertIsNone(rejected)

    def test_roll_threshold_for_an_unused_primary_alternative_is_not_required(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
        )

        sequence = generate_ng3_rarity5_effect_sequence(1)
        actual_primary = sequence.primary.effect_id
        ordinary_ids = {
            effect.effect_id for effect in (sequence.primary, *sequence.secondaries)
        }
        unused_alternative = next(
            effect_id
            for effect_id in (0x774F, 0x28D1, 0x4647, 0xDAC2)
            if effect_id not in ordinary_ids
        )
        result = _verify_effect_sequence(
            1,
            EffectSeedRequest(
                playthrough=3,
                rarity=5,
                primary_effect_ids=frozenset(
                    (actual_primary, unused_alternative)
                ),
                minimum_roll_percent_by_effect_id=(
                    (actual_primary, sequence.primary.roll_percent),
                    (unused_alternative, 100),
                ),
            ),
            generate_ng3_rarity5_effect_sequence,
        )

        self.assertIsNotNone(result)

    def test_duplicate_promoted_effect_can_be_satisfied_by_actual_primary(self) -> None:
        validate_effect_request_feasibility(
            EffectSeedRequest(
                playthrough=3,
                rarity=5,
                primary_effect_ids=frozenset((0x23E8,)),
                required_secondary_ids=frozenset((0x23E8,)),
            )
        )

    def test_budget_exhaustion_advances_cursor_even_without_a_match(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
            generate_ng3_rarity5_primary_effect,
        )

        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((0x774F,)),
        )
        page = collect_effect_seed_page(
            request,
            page_size=2,
            grace_mapping=self.grace_mapping,
            effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
            primary_effect_generator=generate_ng3_rarity5_primary_effect,
            start_after_trial=25,
            max_trials=10,
        )

        self.assertTrue(page.is_empty)
        self.assertEqual(page.next_start_after_trial, 35)
        self.assertIsNotNone(page.intersection_report)
        assert page.intersection_report is not None
        report = page.intersection_report
        self.assertEqual(report.start_after_trial, 25)
        self.assertEqual(report.inspected_through_trial, 35)
        self.assertFalse(report.exhausted_family)
        self.assertFalse(report.is_global_total)
        self.assertEqual(
            tuple(stage.kind for stage in report.stages),
            ("grace", "primary"),
        )
        self.assertEqual(report.stages[0].count, report.fixed_seed_count)
        self.assertEqual(report.stages[1].count, 0)
        self.assertEqual(report.complete_match_count, 0)

    def test_reports_cumulative_intersection_progress_for_exact_replay(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
            generate_ng3_rarity5_primary_effect,
        )

        updates = []
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=0x6553,
            primary_effect_ids=frozenset((0xA051,)),
        )
        page = collect_effect_seed_page(
            request,
            page_size=1,
            grace_mapping=self.grace_mapping,
            effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
            primary_effect_generator=generate_ng3_rarity5_primary_effect,
            max_trials=0x1000,
            intersection_progress=updates.append,
            intersection_progress_interval=1,
        )

        self.assertEqual(len(page.candidates), 1)
        self.assertIsNotNone(page.intersection_report)
        assert page.intersection_report is not None
        report = page.intersection_report
        self.assertEqual(report.inspected_through_trial, page.next_start_after_trial)
        self.assertGreaterEqual(report.fixed_seed_count, 1)
        self.assertEqual(report.stages[0].kind, "grace")
        self.assertEqual(report.stages[0].count, report.fixed_seed_count)
        self.assertEqual(report.stages[1].kind, "primary")
        self.assertEqual(report.stages[1].count, 1)
        self.assertEqual(report.complete_match_count, 1)
        self.assertEqual(updates[-1], report)

    def test_batched_primary_path_preserves_candidates_and_counts(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
            generate_ng3_rarity5_primary_effect_id,
            generate_ng3_rarity5_primary_effect_ids,
        )

        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((0xA051,)),
        )
        shared = {
            "page_size": 4,
            "grace_mapping": self.grace_mapping,
            "effect_sequence_generator": generate_ng3_rarity5_effect_sequence,
            "primary_effect_id_generator": generate_ng3_rarity5_primary_effect_id,
            "max_trials": 0x4000,
        }
        baseline = collect_effect_seed_page(request, **shared)
        with patch(
            "nioh3_scroll_editor.effect_seed_solver.last_seed_acceleration_backend",
            return_value="cuda",
        ):
            accelerated = collect_effect_seed_page(
                request,
                **shared,
                primary_effect_id_batch_generator=lambda seeds: (
                    generate_ng3_rarity5_primary_effect_ids(
                        seeds,
                        grace_id=GRACE,
                        grace_mapping=self.grace_mapping,
                    )
                ),
            )

        self.assertEqual(
            tuple(candidate.seed for candidate in accelerated.candidates),
            tuple(candidate.seed for candidate in baseline.candidates),
        )
        self.assertEqual(
            accelerated.next_start_after_trial,
            baseline.next_start_after_trial,
        )
        self.assertEqual(accelerated.intersection_report, baseline.intersection_report)

    def test_merges_adjacent_intersection_ranges(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
            generate_ng3_rarity5_primary_effect,
        )

        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((0x774F,)),
        )
        first = collect_effect_seed_page(
            request,
            page_size=1,
            grace_mapping=self.grace_mapping,
            effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
            primary_effect_generator=generate_ng3_rarity5_primary_effect,
            max_trials=20,
        )
        second = collect_effect_seed_page(
            request,
            page_size=1,
            grace_mapping=self.grace_mapping,
            effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
            primary_effect_generator=generate_ng3_rarity5_primary_effect,
            start_after_trial=first.next_start_after_trial,
            max_trials=20,
        )
        assert first.intersection_report is not None
        assert second.intersection_report is not None
        merged = merge_intersection_reports(
            (first.intersection_report, second.intersection_report)
        )

        self.assertEqual(merged.start_after_trial, 0)
        self.assertEqual(merged.inspected_through_trial, 40)
        self.assertEqual(
            merged.fixed_seed_count,
            first.intersection_report.fixed_seed_count
            + second.intersection_report.fixed_seed_count,
        )
        self.assertEqual(merged.stages[1].count, 0)

    def test_secondary_constraints_fail_closed_without_offline_replay(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset((SECONDARY,)),
        )
        with self.assertRaises(OfflineEffectReplayUnavailable):
            next(
                iter_effect_seed_candidates(
                    request,
                    grace_mapping=self.grace_mapping,
                    primary_mapping=self.primary_mapping,
                )
            )

    def test_secondary_constraints_use_injected_offline_final_generator(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset((SECONDARY,)),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                primary_mapping=self.primary_mapping,
                final_record_generator=make_final_record,
                max_trials=0x20000,
            )
        )
        self.assertIsNotNone(candidate.record)
        assert candidate.record is not None
        self.assertEqual(candidate.record.seed, candidate.seed)
        self.assertIn(SECONDARY, {effect.effect_id for effect in candidate.record.secondaries})

    def test_secondary_any_group_accepts_one_exact_replay_member(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_id_groups=(
                frozenset((0xDB20, SECONDARY)),
            ),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                primary_mapping=self.primary_mapping,
                final_record_generator=make_final_record,
                max_trials=32,
            )
        )
        self.assertIsNotNone(candidate.record)

    def test_secondary_any_group_rejects_replay_with_no_member(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=GRACE,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_id_groups=(
                frozenset((0xDB20, 0xD40A)),
            ),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                primary_mapping=self.primary_mapping,
                final_record_generator=make_final_record,
                max_trials=32,
            ),
            None,
        )
        self.assertIsNone(candidate)

    def test_secondary_any_groups_reject_overlap_and_value_thresholds(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not overlap"):
            EffectSeedRequest(
                playthrough=3,
                rarity=4,
                required_secondary_id_groups=(
                    frozenset((SECONDARY, 0x2B06)),
                    frozenset((SECONDARY, 0xA051)),
                ),
            )
        with self.assertRaisesRegex(ValueError, "arbitrary values"):
            EffectSeedRequest(
                playthrough=3,
                rarity=4,
                required_secondary_id_groups=(frozenset((SECONDARY, 0x2B06)),),
                minimum_roll_percent_by_effect_id=((SECONDARY, 80),),
            )

    def test_secondary_constraints_use_exact_ng3_effect_sequence(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
        )

        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=0x6553,
            primary_effect_ids=frozenset((0xA051,)),
            required_secondary_ids=frozenset((0xD40A,)),
        )
        primary_mapping = PrimaryOutputMap(
            game_version="2.00.02",
            record_type=self.grace_mapping.record_type,
            rarity=5,
            playthrough=self.grace_mapping.playthrough,
            grace_effect_id=0x6553,
            grace_effect_slot=self.grace_mapping.effect_slot,
            draw_index=2,
            effects=((0xA051, complete_runs()),),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                primary_mapping=primary_mapping,
                effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
                max_trials=0x40000,
            )
        )
        self.assertIsNotNone(candidate.effect_sequence)
        assert candidate.effect_sequence is not None
        self.assertEqual(candidate.effect_sequence.primary.effect_id, 0xA051)
        self.assertIn(
            0xD40A,
            {effect.effect_id for effect in candidate.effect_sequence.secondaries},
        )

    def test_primary_fast_path_still_materializes_exact_effect_sequence(self) -> None:
        from nioh3_scroll_editor.effect_sequence import (
            generate_ng3_rarity5_effect_sequence,
            generate_ng3_rarity5_primary_effect,
        )

        request = EffectSeedRequest(
            playthrough=3,
            rarity=5,
            grace_effect_id=0x6553,
            primary_effect_ids=frozenset((0xA051,)),
        )
        candidate = next(
            iter_effect_seed_candidates(
                request,
                grace_mapping=self.grace_mapping,
                effect_sequence_generator=generate_ng3_rarity5_effect_sequence,
                primary_effect_generator=generate_ng3_rarity5_primary_effect,
                max_trials=0x1000,
            )
        )

        self.assertIsNotNone(candidate.effect_sequence)
        assert candidate.effect_sequence is not None
        self.assertEqual(candidate.effect_sequence.primary.effect_id, 0xA051)


class EnemyStructuralFeasibilityTests(unittest.TestCase):
    def test_requested_three_enemy_combination_is_provably_impossible(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        requirements = tuple(
            EnemyKeyRequirement(name, catalog.enemy_keys_for_name(name))
            for name in ("一目连", "德川国松", "德川庆喜")
        )
        report = analyze_enemy_feasibility(requirements, playthrough=3)
        self.assertFalse(report.possible)
        self.assertEqual(report.viable_branch_classes, ())
        roles = {entry.label: entry.roles for entry in report.requirements}
        self.assertEqual(roles["一目连"], frozenset((1,)))
        self.assertEqual(roles["德川国松"], frozenset((5,)))
        self.assertEqual(roles["德川庆喜"], frozenset((5,)))


if __name__ == "__main__":
    unittest.main()
