import json
import struct
import tempfile
import unittest
from pathlib import Path

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE, write_account_id
from nioh3_scroll_editor.catalog import (
    BETA_EFFECTS,
    GRACE_EFFECTS,
    R4_FINAL_GRACE_EFFECTS,
    R4_SLOT5_EFFECTS,
    contextual_effect_name,
    effect_name,
    native_effect_definitions,
    searchable_scroll_effect_definitions,
    target_effects_for_rarity,
)
from nioh3_scroll_editor.models import (
    CandidateRecordStage,
    ScrollCandidate,
    candidate_has_expected_effect_count,
    candidate_matches,
)
from nioh3_scroll_editor.effect_seed_solver import EffectSeedRequest
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_certified_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
)
from nioh3_scroll_editor.app import (
    FAQ_TEXT,
    FEATURE_GUIDE_TEXT,
    PRODUCT_RARITIES,
    QUICK_START_TEXT,
    TITLE_SCREEN_ACK_TEXT,
    TITLE_SCREEN_PROMPT_TEXT,
    application_title,
    collect_offline_ng3_search_batch,
    collect_offline_rarity5_search_batch,
    is_cached_game_closed_effect_context,
    is_game_closed_effect_context,
    special_rule_variant_label,
    toggle_rule_filter_option,
    user_facing_error_message,
)
from nioh3_scroll_editor.grace_map import GraceOutputMap, GraceRange, load_grace_output_map
from nioh3_scroll_editor.native import (
    ASSEMBLE_SCROLL_RVA,
    GENERATE_EFFECTS_RVA,
    INIT_GENERATION_CONTEXT_RVA,
    PLAYTHROUGH_MANAGER_POINTER_RVA,
    PLAYTHROUGH_VECTOR_RVA,
    REMOTE_CODE_SIZE,
    build_batch_wrapper,
    build_effect_finalizer_batch_wrapper,
    build_effect_finalizer_wrapper,
    build_explicit_playthrough_seed_range_wrapper,
    build_seed_range_wrapper,
    build_source_record,
    scan_next_candidate,
)
from nioh3_scroll_editor.savegame import (
    BackupEntry,
    LocalEffectEdit,
    SCROLL_GROUP_OFFSET,
    SaveInstaller,
    SaveInventory,
    list_backup_entries,
    materialize_effect_sequence_candidate,
    next_generation_serial,
    patch_local_scroll_record,
    prepare_candidate_for_install,
)
from nioh3_scroll_editor.experiments import (
    CONTEXTUAL_TEST_EFFECT_ID,
    build_contextual_babd_experiment,
    build_existing_contextual_babd_experiment,
)


TEST_ACCOUNT_ID = 0x1111222233334444
TEST_FOREIGN_ACCOUNT_ID = 0x5555666677778888


def make_record(
    *,
    seed: int = 1,
    account_id: int = TEST_ACCOUNT_ID,
    effects: tuple[int, ...] = (0x47BC, 0x4647, 0xA051, 0x190A, 0x2B06, 0xB613),
) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, 0x1E82)
    struct.pack_into("<H", record, 6, 180)
    struct.pack_into("<H", record, 8, 180)
    struct.pack_into("<H", record, 0x10, 183)
    struct.pack_into("<H", record, 0x12, 183)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30] = 5
    record[0x31] = 5
    write_account_id(record, account_id)
    for index, effect_id in enumerate(effects):
        offset = 0x34 + index * 0x18
        struct.pack_into("<6I", record, offset, index + 1, effect_id, 100 + index, 200 + index, 0, 0)
    struct.pack_into("<I", record, 0x34 + 6 * 0x18 + 4, 0xFFFFFFFF)
    return bytes(record)


class BetaEditorTests(unittest.TestCase):
    def test_normal_title_does_not_expose_internal_safety_mode(self) -> None:
        self.assertEqual(application_title(research_mode=False), "仁王3绘卷生成器 Beta")
        self.assertEqual(
            application_title(research_mode=True),
            "仁王3绘卷生成器 Beta（研究模式）",
        )
        self.assertNotIn("安全", application_title(research_mode=False))

    def test_quick_start_leads_with_product_actions(self) -> None:
        for phrase in (
            "搜索所有词条",
            "计算候选 Seed",
            "回到标题界面",
            "自动备份",
        ):
            self.assertIn(phrase, QUICK_START_TEXT)
        for technical_term in ("LCG", "draw-1", "前像"):
            self.assertNotIn(technical_term, QUICK_START_TEXT)
        self.assertIn("搜索并添加可以传播的合法绘卷", FEATURE_GUIDE_TEXT)
        self.assertIn("不需要断开网络", FAQ_TEXT)
        self.assertIn("3609 项原生名称目录", FAQ_TEXT)

    def test_product_ui_exposes_only_supported_rarities(self) -> None:
        self.assertEqual(PRODUCT_RARITIES, (3, 4))
        self.assertIn("暂不再提供稀有度 5", FAQ_TEXT)

    def test_special_rule_selection_preserves_other_rule_families(self) -> None:
        head_family = frozenset(("any:head", "exact:head:65", "exact:head:80"))
        grace_family = frozenset(("any:grace", "exact:grace:30"))
        families = {
            token: family
            for family in (head_family, grace_family)
            for token in family
        }

        selected = toggle_rule_filter_option(set(), "exact:head:80", families)
        selected = toggle_rule_filter_option(selected, "exact:grace:30", families)

        self.assertEqual(selected, {"exact:head:80", "exact:grace:30"})
        selected = toggle_rule_filter_option(selected, "any:head", families)
        self.assertEqual(selected, {"any:head", "exact:grace:30"})
        selected = toggle_rule_filter_option(selected, "any:head", families)
        self.assertEqual(selected, {"exact:grace:30"})

    def test_exact_special_rule_variant_keeps_rule_name(self) -> None:
        label = special_rule_variant_label("一难横行（头部防具）", 0x598F, "+80%")
        self.assertIn("一难横行（头部防具）", label)
        self.assertIn("+80%", label)

    def test_title_screen_confirmation_does_not_require_disconnection(self) -> None:
        self.assertEqual(TITLE_SCREEN_ACK_TEXT, "我确认游戏当前位于标题界面")
        self.assertIn("不需要断开网络", TITLE_SCREEN_PROMPT_TEXT)
        self.assertNotIn("离线状态", TITLE_SCREEN_PROMPT_TEXT)

    def test_error_dialog_hides_internal_python_exception_prefix(self) -> None:
        self.assertEqual(
            user_facing_error_message(
                "Traceback (most recent call last):\n"
                "RuntimeError: 安装时物化记录与求解器预览不一致，已拒绝写入"
            ),
            "安装时物化记录与求解器预览不一致，已拒绝写入",
        )

    def test_seed_91104224_uses_full_native_name_catalog(self) -> None:
        candidate = ScrollCandidate.from_effect_sequence(
            generate_ng3_certified_effect_sequence(91_104_224, rarity=4)
        )
        self.assertEqual(
            [effect.effect_id for effect in candidate.effects],
            [0xD495, 0x34F3, 0x28C4, 0x2B06, 0xCE68],
        )
        self.assertEqual(
            [candidate.display_name(effect) for effect in candidate.effects],
            [
                "属性攻击伤害",
                "防御时的属性攻击伤害降低",
                "绝妙追杀",
                "咒之深奥",
                "素盏呜尊的恩宠",
            ],
        )

    def test_searchable_pool_uses_native_final_effect_context(self) -> None:
        effects = searchable_scroll_effect_definitions(3, 5)
        ids = {effect.effect_id for effect in effects}

        self.assertEqual(len(effects), 50)
        self.assertEqual(len(ids), len(effects))
        self.assertIn(0xFBEE, ids)
        self.assertIn(0xAE5A, ids)
        self.assertIn(0xDFF0, ids)
        self.assertIn(0xD495, ids)
        self.assertIn(0x34F3, ids)
        self.assertIn(0x28C4, ids)
        self.assertNotIn(0xBABD, ids)
        self.assertNotIn(0x0001, ids)
        self.assertTrue(all(effect.name != "未知词条" for effect in effects))

    def test_full_native_catalog_always_resolves_known_preview_names(self) -> None:
        catalog = native_effect_definitions()
        self.assertEqual(len(catalog), 3609)
        unresolved = [
            effect.effect_id
            for effect in catalog
            if contextual_effect_name(effect.effect_id, rarity=5, slot=2).startswith(
                "编号 "
            )
        ]
        self.assertEqual(unresolved, [])

    def test_any_grace_search_streams_each_exact_candidate(self) -> None:
        streamed = []
        result = collect_offline_ng3_search_batch(
            EffectSeedRequest(playthrough=3, rarity=5),
            grace_mapping=load_grace_output_map(rarity=5),
            level=180,
            result_count=2,
            max_trials_per_batch=4,
            candidate_found=streamed.append,
        )

        self.assertTrue(result.streamed)
        self.assertEqual(tuple(streamed), result.candidates)
        self.assertEqual(len(result.candidates), 2)
        self.assertTrue(all(candidate.rarity == 5 for candidate in streamed))

    def test_rarity4_exact_grace_filter_replays_finalizer(self) -> None:
        result = collect_offline_ng3_search_batch(
            EffectSeedRequest(
                playthrough=3,
                rarity=4,
                grace_effect_id=0xCE68,
            ),
            grace_mapping=load_grace_output_map(rarity=4),
            level=180,
            result_count=3,
            max_trials_per_batch=64,
        )

        self.assertEqual(len(result.candidates), 3)
        self.assertTrue(all(candidate.grace is not None for candidate in result.candidates))
        self.assertTrue(
            all(candidate.grace.effect_id == 0xCE68 for candidate in result.candidates)
        )
        self.assertTrue(
            all(
                0xCE68 not in {effect.effect_id for effect in candidate.secondaries}
                for candidate in result.candidates
            )
        )
        self.assertIsNotNone(result.intersection_report)
        self.assertEqual(result.intersection_report.stages[0].kind, "grace")
        self.assertEqual(result.intersection_report.stages[0].count, 3)

    def test_rarity4_joint_search_reports_the_optimized_filter_order(self) -> None:
        result = collect_offline_ng3_search_batch(
            EffectSeedRequest(
                playthrough=3,
                rarity=4,
                grace_effect_id=0x71F6,
                primary_effect_ids=frozenset((0xB613,)),
                required_secondary_ids=frozenset((0x23E8,)),
            ),
            grace_mapping=load_grace_output_map(rarity=4),
            level=180,
            result_count=1,
            max_trials_per_batch=250_000,
        )

        self.assertEqual(len(result.candidates), 1)
        assert result.intersection_report is not None
        self.assertEqual(
            tuple(stage.kind for stage in result.intersection_report.stages),
            ("primary", "grace", "secondary"),
        )

    def test_ng3_rarity345_use_game_closed_ui_path(self) -> None:
        base = (
            frozenset(),
            frozenset(),
            0xBABD,
            5,
            180,
            183,
            None,
            3,
        )
        self.assertTrue(is_game_closed_effect_context(base))
        self.assertFalse(is_game_closed_effect_context((*base[:-1], 4)))
        self.assertTrue(is_game_closed_effect_context((*base[:3], 4, *base[4:])))
        self.assertTrue(is_game_closed_effect_context((*base[:3], 3, *base[4:])))
        self.assertFalse(is_game_closed_effect_context((*base[:3], 2, *base[4:])))
        self.assertFalse(is_game_closed_effect_context((*base[:3], 1, *base[4:])))
        self.assertTrue(is_cached_game_closed_effect_context((*base[:-1], 4)))
        self.assertTrue(is_cached_game_closed_effect_context((*base[:-1], 5)))
        self.assertFalse(is_cached_game_closed_effect_context(base))

    def test_ng4_cached_map_runs_exact_game_closed_search(self) -> None:
        mapping = GraceOutputMap(
            record_type=0xDD82,
            rarity=5,
            playthrough="category-4-live-native",
            effect_slot=6,
            ranges=(GraceRange(0, 0xFFFF, 0x6553),),
        )
        result = collect_offline_rarity5_search_batch(
            EffectSeedRequest(
                playthrough=4,
                rarity=5,
                grace_effect_id=0x6553,
            ),
            grace_mapping=mapping,
            level=180,
            result_count=1,
            max_trials_per_batch=1,
        )

        self.assertEqual(len(result.candidates), 1)
        self.assertEqual(result.candidates[0].playthrough, 4)
        self.assertEqual(result.candidates[0].effects[-1].effect_id, 0x6553)
        self.assertFalse(result.candidates[0].can_materialize_for_install)

    def test_certified_ng3_rarity5_preview_is_materializable(self) -> None:
        sequence = generate_ng3_rarity5_effect_sequence(1)
        candidate = ScrollCandidate.from_effect_sequence(sequence)

        self.assertEqual(
            candidate.record_stage,
            CandidateRecordStage.EFFECT_SEQUENCE_ONLY,
        )
        self.assertEqual(candidate.record, b"")
        self.assertEqual(candidate.effects[0].effect_id, 0xA051)
        self.assertEqual(candidate.effects[0].roll_percent, 94)
        self.assertEqual(candidate.effects[0].value, 9)
        self.assertTrue(candidate.can_materialize_for_install)
        self.assertIsNone(candidate.install_blocker)

    def test_r4_final_graces_are_selectable_without_exposing_stage_tokens(self) -> None:
        self.assertEqual(target_effects_for_rarity(4), R4_FINAL_GRACE_EFFECTS)
        self.assertEqual(
            target_effects_for_rarity(4, include_transient_stage_one=True),
            R4_FINAL_GRACE_EFFECTS,
        )
        self.assertTrue(all("非最终词条" not in effect.name for effect in R4_FINAL_GRACE_EFFECTS))

    def test_every_r4_stage_one_candidate_is_install_blocked(self) -> None:
        record = bytearray(
            make_record(
                effects=(0xA051, 0xAE5A, 0xDB20, 0xD40A, 0xDEADBEEF),
            )
        )
        record[0x30] = 4
        record[0x31] = 4
        candidate = ScrollCandidate.from_record(
            bytes(record),
            playthrough=3,
            record_stage=CandidateRecordStage.NATIVE_STAGE_ONE,
        )
        self.assertEqual(candidate.unresolved_effect_slots, (5,))
        self.assertIsNotNone(candidate.install_blocker)

    def test_special_slots_do_not_count_as_ordinary_secondaries(self) -> None:
        r5 = ScrollCandidate.from_record(
            make_record(
                effects=(0xA051, 0xAE5A, 0xDB20, 0xD40A, 0x6BEB, 0xBABD),
            ),
            playthrough=3,
            record_stage=CandidateRecordStage.FINAL_RECORD,
        )
        self.assertEqual(
            {effect.effect_id for effect in r5.secondaries},
            {0xAE5A, 0xDB20, 0xD40A, 0x6BEB},
        )
        self.assertFalse(
            candidate_matches(
                r5,
                primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset({0xBABD}),
            )
        )

        r4_record = bytearray(
            make_record(effects=(0xA051, 0xAE5A, 0xDB20, 0xD40A, 0xBABD))
        )
        r4_record[0x30] = 4
        r4_record[0x31] = 4
        r4 = ScrollCandidate.from_record(
            bytes(r4_record),
            playthrough=3,
            record_stage=CandidateRecordStage.NATIVE_STAGE_ONE,
        )
        self.assertEqual(
            {effect.effect_id for effect in r4.secondaries},
            {0xAE5A, 0xDB20, 0xD40A},
        )

    def test_partial_native_result_is_rejected_for_rarity_four(self) -> None:
        complete = ScrollCandidate.from_record(
            make_record(effects=(0x1355, 0x92E0, 0x4647, 0x2B06, 0x4192))
        )
        partial_record = bytearray(make_record(effects=(0x23E5,)))
        for index in range(1, 6):
            struct.pack_into("<I", partial_record, 0x38 + index * 0x18, 0xFFFFFFFF)
        partial = ScrollCandidate.from_record(bytes(partial_record))
        self.assertTrue(candidate_has_expected_effect_count(complete, 4))
        self.assertFalse(candidate_has_expected_effect_count(partial, 4))

    def test_beta_pool_has_unique_little_endian_ids(self) -> None:
        ids = [effect.effect_id for effect in BETA_EFFECTS]
        self.assertEqual(len(ids), 40)
        self.assertEqual(len(ids), len(set(ids)))
        self.assertIn(0x4647, ids)
        self.assertIn(0x47BC, ids)
        self.assertIn(0xFBEE, ids)
        self.assertIn(0xDFF0, ids)
        self.assertEqual(effect_name(0xA73D), "体力")
        self.assertEqual(effect_name(0x583B), "负面效果持续时间缩短")
        self.assertIn(0x2B06, ids)
        self.assertNotIn(0x0001, ids)
        self.assertIn("画龙点睛", effect_name(0x0001))
        grace_ids = {effect.effect_id for effect in GRACE_EFFECTS}
        self.assertEqual(len(grace_ids), 11)
        self.assertTrue(grace_ids.isdisjoint(ids))
        self.assertIn(0x6553, grace_ids)
        self.assertIn(0xCE68, grace_ids)
        self.assertEqual(effect_name(0x6553), "天照大神的恩宠")
        self.assertEqual(effect_name(0xCE68), "素盏呜尊的恩宠")
        self.assertEqual(effect_name(0xEB61), "祸津日的恩宠")
        r4_ids = {effect.effect_id for effect in R4_SLOT5_EFFECTS}
        self.assertEqual(len(r4_ids), 21)
        self.assertIn(0xBABD, r4_ids)


    def test_contextual_slot_names_separate_final_babd_from_r4_stage_one(self) -> None:
        # Direct in-game captures placed 0xBABD in physical slots 4, 5, and 6;
        # all three displayed 月读的恩宠. The former 技之深奥 report came from
        # an external build with the wrong natural-language label.
        self.assertEqual(
            contextual_effect_name(0xBABD, rarity=4, slot=4),
            "月读的恩宠",
        )
        self.assertEqual(
            contextual_effect_name(0xBABD, rarity=4, slot=5),
            "月读的恩宠",
        )
        self.assertEqual(
            contextual_effect_name(
                0xBABD,
                rarity=4,
                slot=5,
                native_stage_one=True,
            ),
            "R4生成阶段结果码 0xBABD（非最终词条）",
        )
        self.assertEqual(
            contextual_effect_name(0xBABD, rarity=5, slot=6),
            "月读的恩宠",
        )
        self.assertEqual(
            contextual_effect_name(0xAE5A, rarity=3, slot=1),
            "技之深奥",
        )

    def test_seed_183696634_stage_one_token_is_not_installable_as_final_effect(self) -> None:
        stage_one = bytes.fromhex(
            "04e600000000b400b400000000000001b700b700000000000200800200000000"
            "fafcf20a000000002ce531000000000004040006"
            "1bb50000062b000096000000584d04000000000000000000"
            "b6b100003da7000050010000581400000000000000000000"
            "a5b600004746000011000000600300000000000000000000"
            "d17a0000413f00000b0000005e0600000000000000000000"
            "b1a10000bdba000000000000000c02000000000000000000"
            "00000000ffffffff00000000000000000000000000000000"
            "00000000ffffffff00000000000000000000000000000000"
            "000000000000000000000000"
        )
        final_record = bytes.fromhex(
            "04e600000000b400b400000000000001b700b700000000000600800b00000000"
            "fafcf20a000000002ce531000000000004040005"
            "1bb50000062b000096000000584d04000000000000000000"
            "b6b100003da7000050010000581400000000000000000000"
            "a5b600004746000011000000600300000000000000000000"
            "d17a0000413f00000b0000005e0600000000000000000000"
            "de8900005aae0000960000005a0d04000000000000000000"
            "00000000ffffffff00000000000000000000000000000000"
            "00000000ffffffff00000000000000000000000000000000"
            "000000000000000000000000"
        )
        unresolved = ScrollCandidate.from_record(
            stage_one,
            record_stage=CandidateRecordStage.NATIVE_STAGE_ONE,
        )
        resolved = ScrollCandidate.from_record(final_record)

        self.assertEqual(unresolved.seed, 183696634)
        self.assertEqual(unresolved.effects[4].effect_id, 0xBABD)
        self.assertEqual(unresolved.unresolved_effect_slots, (5,))
        self.assertIn("非最终词条", unresolved.display_name(unresolved.effects[4]))
        self.assertIsNotNone(unresolved.install_blocker)
        self.assertEqual(resolved.effects[4].effect_id, 0xAE5A)
        self.assertEqual(resolved.display_name(resolved.effects[4]), "技之深奥")
        self.assertEqual(resolved.unresolved_effect_slots, ())
        self.assertIsNone(resolved.install_blocker)
        self.assertEqual(
            tuple(effect.effect_id for effect in unresolved.effects[:4]),
            tuple(effect.effect_id for effect in resolved.effects[:4]),
        )
        # Do not silently reuse the R5 name for an R4 stage-one token.
        self.assertIn(
            "非最终词条",
            contextual_effect_name(
                0xCE68,
                rarity=4,
                slot=5,
                native_stage_one=True,
            ),
        )

    def test_user_supplied_little_endian_effect_mappings(self) -> None:
        expected = {
            0x2EFC: "远距离伤害", 0x9A3D: "强攻击精力消耗降低",
            0xA73D: "体力", 0x6CE3: "不消耗使役符",
            0x6BEB: "近距离攻击精力伤害", 0xEA53: "坚忍度",
            0x512D: "精髓并存（武士）", 0x7499: "对人战术",
            0x3E7A: "精力恢复速度", 0xB82B: "敌人精力耗尽时赋予^09~BUFF~{}^09~~",
            0xD40A: "近距离攻击的精力消耗降低", 0x6E2B: "不消耗仙药",
            0xBC51: "精华槽增加量", 0x3A8E: "武技精力伤害",
            0xCE1A: "武技伤害", 0x3F41: "属性攻击伤害降低",
            0x583B: "负面效果持续时间缩短", 0xAE5A: "技之深奥",
            0xA0A7: "近距离攻击打倒敌人时恢复体力", 0xEF97: "速攻击伤害",
            0x28D1: "九十九化身持续时间延长", 0x5CAC: "闪避动作精力消耗降低",
            0x1355: "灵力增加量", 0xD411: "冲刺精力消耗降低",
            0x6AAF: "智之深奥",
            0xDAC2: "体之深奥", 0x23E8: "刚之深奥",
            0x600F: "伤害反映（阴阳术术力）", 0x8184: "装备品掉落率",
            0xDB20: "近距离攻击伤害",
        }
        for effect_id, name in expected.items():
            with self.subTest(effect_id=f"{effect_id:#06x}"):
                self.assertEqual(effect_name(effect_id), name)

    def test_fbee_conflict_is_resolved_from_full_ng3_records(self) -> None:
        audit_path = Path(__file__).parent / "test_fixtures" / "effect_mapping_31.json"
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        conflict = audit["existing_id_audit"]["different_name_conflicts"]
        self.assertEqual(conflict, [])
        self.assertEqual(effect_name(0xFBEE), "防御精力消耗降低")
        self.assertEqual(effect_name(0xDFF0), "武之深奥")
        self.assertEqual(effect_name(0x23E8), "刚之深奥")
        self.assertEqual(effect_name(0x16E2), "迦具土的恩宠")

    def test_primary_and_secondary_candidate_pools(self) -> None:
        candidate = ScrollCandidate.from_record(make_record())
        # Rarity-5 slot 6 is the special/Grace slot, not an ordinary secondary.
        actual_secondaries = frozenset((0x4647, 0xA051, 0x190A, 0x2B06))
        self.assertFalse(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((0x47BC, 0x8613)),
                required_secondary_ids=actual_secondaries | frozenset((0xDFF0,)),
            )
        )
        self.assertFalse(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((0x4647, 0x8613)),
                required_secondary_ids=frozenset(),
            )
        )
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset(),
                required_secondary_ids=actual_secondaries,
            )
        )
        self.assertFalse(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((0x47BC,)),
                required_secondary_ids=frozenset((0xDFF0,)),
            )
        )

    def test_cross_list_overlap_is_satisfied_by_actual_primary(self) -> None:
        # A/B/C selected on both sides means the actual primary may satisfy
        # its own duplicated right-side requirement; only the other two must
        # appear as ordinary secondaries.
        a, b, c = 0x47BC, 0x4647, 0xA051
        candidate = ScrollCandidate.from_record(
            make_record(effects=(a, b, c, 0x190A, 0x2B06, 0xB613))
        )
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((a, b, c)),
                required_secondary_ids=frozenset((a, b, c)),
            )
        )

    def test_single_overlapping_primary_satisfies_duplicate_right_requirement(self) -> None:
        # Screenshot regression: left selects A; right selects A+B+C+D.
        # A is allowed to live in the primary slot, while B/C/D remain required
        # ordinary secondaries.  The generator must not require a duplicate A.
        a, b, c, d = 0x6E2B, 0x7499, 0xB613, 0xFBEE
        candidate = ScrollCandidate.from_record(
            make_record(effects=(a, b, c, d, 0x2B06, 0x190A))
        )
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((a,)),
                required_secondary_ids=frozenset((a, b, c, d)),
            )
        )

    def test_secondary_only_selection_does_not_gain_primary_exemption(self) -> None:
        # If A/B/C were selected only on the right, A being the actual primary
        # must NOT satisfy the right-side A requirement.
        a, b, c = 0x47BC, 0x4647, 0xA051
        candidate = ScrollCandidate.from_record(
            make_record(effects=(a, b, c, 0x190A, 0x2B06, 0xB613))
        )
        self.assertFalse(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset((a, b, c)),
            )
        )

    def test_scanner_supports_cross_list_pairing(self) -> None:
        a, b, c = 0x47BC, 0x4647, 0xA051

        class FakeOracle:
            max_batch_size = 1

            def generate_seed_range(
                self, template: bytes, *, start_seed: int, seed_step: int,
                count: int, playthrough=None,
            ) -> list[bytes]:
                return [
                    make_record(
                        seed=start_seed,
                        effects=(a, b, c, 0x190A, 0x2B06, 0xB613),
                    )
                ]

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(seed=1),
            start_seed=123,
            primary_effect_ids=frozenset((a, b, c)),
            required_secondary_ids=frozenset((a, b, c)),
            rarity=5,
            playthrough=None,
            max_seeds=1,
        )
        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertEqual(candidate.primary.effect_id, a)

    def test_source_record_updates_all_canonical_mirrors(self) -> None:
        record = build_source_record(
            make_record(),
            seed=114514,
            rarity=5,
            level=180,
            recommended_level=183,
            transfer_count=99,
        )
        self.assertEqual(struct.unpack_from("<I", record, 0x20)[0], 114514)
        self.assertEqual(record[0x30:0x32], b"\x05\x05")
        self.assertEqual(struct.unpack_from("<2H", record, 6), (180, 180))
        self.assertEqual(struct.unpack_from("<2H", record, 0x10), (183, 183))
        self.assertEqual(struct.unpack_from("<I", record, 0xDC)[0], 99)

    def test_batch_wrapper_contains_count_and_record_stride(self) -> None:
        wrapper = build_batch_wrapper(0x1111222233334444, 0x5555666677778888, 0x9999AAAABBBBCCCC, 512)
        self.assertIn(struct.pack("<I", 512), wrapper)
        self.assertEqual(wrapper.count(bytes.fromhex("E8 00 00 00")), 2)
        self.assertTrue(wrapper.endswith(bytes.fromhex("41 5C 5F 5E 5B C3")))

    def test_seed_range_wrapper_reuses_source_and_increments_seed(self) -> None:
        wrapper = build_seed_range_wrapper(
            0x1111222233334444,
            0x5555666677778888,
            0x9999AAAABBBBCCCC,
            114514,
            0x9E3779B1,
            2048,
        )
        self.assertIn(bytes.fromhex("44 89 6B 20"), wrapper)
        self.assertIn(bytes.fromhex("41 81 C5 B1 79 37 9E"), wrapper)
        self.assertIn(struct.pack("<I", 114514), wrapper)
        self.assertIn(struct.pack("<I", 2048), wrapper)
        self.assertTrue(wrapper.endswith(bytes.fromhex("41 5D 41 5C 5F 5E 5B C3")))

    def test_effect_finalizer_wrapper_passes_index_and_reveal_flag(self) -> None:
        source = 0x1111222233334444
        destination = 0x5555666677778888
        function = 0x9999AAAABBBBCCCC
        wrapper = build_effect_finalizer_wrapper(
            source,
            destination,
            function,
            4,
            True,
        )
        self.assertIn(b"\x48\xB9" + struct.pack("<Q", destination), wrapper)
        self.assertIn(b"\x48\xBA" + struct.pack("<Q", source), wrapper)
        self.assertIn(bytes.fromhex("41 B8 04 00 00 00"), wrapper)
        self.assertIn(bytes.fromhex("41 B9 01 00 00 00"), wrapper)
        self.assertIn(b"\x48\xB8" + struct.pack("<Q", function), wrapper)
        self.assertTrue(wrapper.endswith(bytes.fromhex("31 C0 48 83 C4 28 C3")))

        with self.assertRaises(ValueError):
            build_effect_finalizer_wrapper(source, destination, function, 7, True)

    def test_effect_finalizer_batch_wrapper_contains_stride_and_count(self) -> None:
        wrapper = build_effect_finalizer_batch_wrapper(
            0x1111222233334444,
            0x5555666677778888,
            0x9999AAAABBBBCCCC,
            2048,
            4,
            True,
        )
        self.assertIn(struct.pack("<I", 2048), wrapper)
        self.assertIn(bytes.fromhex("41 B8 04 00 00 00"), wrapper)
        self.assertIn(bytes.fromhex("41 B9 01 00 00 00"), wrapper)
        self.assertEqual(wrapper.count(bytes.fromhex("48 81 C3 E8 00 00 00")), 1)
        self.assertEqual(wrapper.count(bytes.fromhex("48 81 C6 E8 00 00 00")), 1)
        self.assertTrue(wrapper.endswith(bytes.fromhex("41 5C 5F 5E 5B C3")))

    def test_explicit_playthrough_wrapper_uses_temporary_context(self) -> None:
        module_base = 0x00007FF700000000
        wrapper = build_explicit_playthrough_seed_range_wrapper(
            0x1111222233334444,
            0x5555666677778888,
            module_base,
            114514,
            0x9E3779B1,
            2048,
            5,
        )
        self.assertLessEqual(len(wrapper), REMOTE_CODE_SIZE)
        self.assertIn(bytes.fromhex("41 BF 05 00 00 00"), wrapper)
        self.assertIn(
            struct.pack("<Q", module_base + PLAYTHROUGH_MANAGER_POINTER_RVA),
            wrapper,
        )
        for rva in (
            INIT_GENERATION_CONTEXT_RVA,
            PLAYTHROUGH_VECTOR_RVA,
            GENERATE_EFFECTS_RVA,
            ASSEMBLE_SCROLL_RVA,
        ):
            self.assertIn(struct.pack("<Q", module_base + rva), wrapper)
        self.assertIn(bytes.fromhex("44 88 BC 24 40 01 00 00"), wrapper)
        self.assertIn(bytes.fromhex("44 88 7C 24 40"), wrapper)
        self.assertIn(bytes.fromhex("0F 11 44 24 44"), wrapper)
        self.assertNotIn(bytes.fromhex("48 81 C3 E8 00 00 00"), wrapper)
        self.assertIn(bytes.fromhex("48 81 C6 E8 00 00 00"), wrapper)

    def test_explicit_playthrough_wrapper_rejects_invalid_values(self) -> None:
        with self.assertRaises(ValueError):
            build_explicit_playthrough_seed_range_wrapper(1, 2, 3, 4, 1, 1, 0)
        with self.assertRaises(ValueError):
            build_explicit_playthrough_seed_range_wrapper(1, 2, 3, 4, 1, 1, 6)
        with self.assertRaises(ValueError):
            build_explicit_playthrough_seed_range_wrapper(1, 2, 3, 4, 1, 1, 3, 2)

    def test_explicit_playthrough_wrapper_can_select_completion_mode(self) -> None:
        wrapper = build_explicit_playthrough_seed_range_wrapper(
            1,
            2,
            0x00007FF700000000,
            183696634,
            1,
            1,
            3,
            1,
        )
        self.assertIn(bytes.fromhex("41 B1 01"), wrapper)

    def test_scanner_uses_a_full_space_seed_step(self) -> None:
        class FakeOracle:
            max_batch_size = 4

            def generate_seed_range(
                self,
                template: bytes,
                *,
                start_seed: int,
                seed_step: int,
                count: int,
                playthrough: int | None = None,
            ) -> list[bytes]:
                self.playthrough = playthrough
                records = []
                for index in range(count):
                    seed = (start_seed + index * seed_step) & 0xFFFFFFFF
                    primary = 0x47BC if seed == 24 else 0x4647
                    records.append(
                        make_record(
                            seed=seed,
                            effects=(primary, 0xA051, 0x190A, 0x2B06, 0xB613, 0xDFF0),
                        )
                    )
                return records

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(),
            start_seed=10,
            seed_step=7,
            primary_effect_ids=frozenset((0x47BC,)),
            required_secondary_ids=frozenset((0xA051, 0x190A, 0x2B06, 0xB613)),
            # This regression covers the legacy unrestricted seed-range path;
            # grace-targeted searches now require the verified E604/current
            # context and are covered by test_grace_accelerated_scanner.
            grace_effect_id=None,
            playthrough=4,
            max_seeds=8,
        )
        self.assertIsNotNone(candidate)
        self.assertEqual(candidate.seed, 24)
        self.assertEqual(candidate.playthrough, 4)

    def test_inventory_uses_own_origin_template_and_only_zero_slots(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        own = make_record(account_id=account)
        friend = make_record(seed=2, account_id=TEST_FOREIGN_ACCOUNT_ID)
        save[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = friend
        second = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
        save[second:second + SCROLL_RECORD_SIZE] = own
        third = second + SCROLL_RECORD_SIZE
        save[third] = 1
        inventory = SaveInventory.load(save_path, bytes(save))
        self.assertEqual(inventory.template_record, own)
        self.assertNotIn(0, inventory.empty_slots)
        self.assertNotIn(1, inventory.empty_slots)
        self.assertNotIn(2, inventory.empty_slots)
        self.assertIn(3, inventory.empty_slots)
        self.assertEqual(inventory.next_slot_index, 3)

    def test_transfer_count_edit_does_not_change_effects(self) -> None:
        record = make_record()
        edited = prepare_candidate_for_install(record, transfer_count=9_999_999)
        self.assertEqual(struct.unpack_from("<I", edited, 0xDC)[0], 9_999_999)
        self.assertEqual(record[0x34:0xDC], edited[0x34:0xDC])

    def test_game_closed_preview_materializes_against_current_ng3_template(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = bytearray(make_record(seed=241719428, account_id=account))
        struct.pack_into("<H", template, 0, 0xE604)
        struct.pack_into("<I", template, 0x28, 40)
        save[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = template
        inventory = SaveInventory.load(save_path, bytes(save))
        preview = ScrollCandidate.from_effect_sequence(
            generate_ng3_rarity5_effect_sequence(1, level=180)
        )

        materialized = materialize_effect_sequence_candidate(
            inventory,
            preview,
            level=180,
            recommended_level=183,
            transfer_count=12,
        )

        self.assertEqual(materialized.record_stage, CandidateRecordStage.FINAL_RECORD)
        self.assertEqual(materialized.seed, 1)
        self.assertEqual(struct.unpack_from("<H", materialized.record, 0)[0], 0xE604)
        self.assertEqual(struct.unpack_from("<I", materialized.record, 0x28)[0], 41)
        self.assertEqual(struct.unpack_from("<I", materialized.record, 0xDC)[0], 12)
        self.assertEqual(
            [effect.effect_id for effect in materialized.effects[:6]],
            [effect.effect_id for effect in preview.effects],
        )
        self.assertEqual(next_generation_serial(inventory), 41)

    def test_rarity4_materialization_clears_donor_completion_salt(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = bytearray(make_record(seed=241719428, account_id=account))
        struct.pack_into("<H", template, 0, 0xE604)
        struct.pack_into("<H", template, 0x0C, 0xFFFF)
        struct.pack_into("<I", template, 0x28, 40)
        save[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = template
        inventory = SaveInventory.load(save_path, bytes(save))
        preview = ScrollCandidate.from_effect_sequence(
            generate_ng3_certified_effect_sequence(47_878_870, rarity=4, level=180)
        )

        materialized = materialize_effect_sequence_candidate(
            inventory,
            preview,
            level=180,
            recommended_level=183,
            transfer_count=0,
        )

        self.assertEqual(struct.unpack_from("<H", materialized.record, 0x0C)[0], 0)
        self.assertEqual(
            [effect.effect_id for effect in materialized.effects[:5]],
            [effect.effect_id for effect in preview.effects],
        )

    def test_contextual_babd_experiment_changes_only_controlled_fields(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = bytearray(make_record(account_id=account))
        struct.pack_into("<H", template, 0, 0xE604)
        struct.pack_into("<I", template, 0x28, 10)
        save[
            SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
        ] = template
        tombstone_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
        struct.pack_into("<I", save, tombstone_offset + 0x28, 0xFFFFFFFF)
        inventory = SaveInventory.load(save_path, bytes(save))

        experiment = build_contextual_babd_experiment(inventory)

        self.assertEqual(experiment.template_slot, 0)
        self.assertEqual(tuple(item.target_slot for item in experiment.records), (4, 5, 6))
        for index, item in enumerate(experiment.records):
            effect_offset = 0x34 + (item.target_slot - 1) * 0x18 + 4
            self.assertEqual(
                struct.unpack_from("<I", item.record, effect_offset)[0],
                CONTEXTUAL_TEST_EFFECT_ID,
            )
            self.assertEqual(item.generation_serial, 11 + index)
            self.assertEqual(item.transfer_count, item.target_slot)
            allowed = set(range(effect_offset, effect_offset + 4))
            allowed.update(range(0x28, 0x2C))
            allowed.update(range(0xDC, 0xE0))
            changed = {
                offset
                for offset, (before, after) in enumerate(
                    zip(experiment.template_record, item.record, strict=True)
                )
                if before != after
            }
            self.assertTrue(changed)
            self.assertTrue(changed <= allowed)

    def test_existing_babd_experiment_uses_three_revealed_records(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        originals: list[bytes] = []
        for slot, seed in enumerate((101, 102, 103)):
            record = bytearray(make_record(seed=seed, account_id=account))
            struct.pack_into("<H", record, 0, 0xE604)
            originals.append(bytes(record))
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            save[offset:offset + SCROLL_RECORD_SIZE] = record
        inventory = SaveInventory.load(save_path, bytes(save))

        experiment = build_existing_contextual_babd_experiment(inventory)

        self.assertEqual(
            tuple(edit.inventory_slot for edit in experiment.edits), (0, 1, 2)
        )
        self.assertEqual(tuple(edit.target_slot for edit in experiment.edits), (4, 5, 6))
        for original, edit in zip(originals, experiment.edits, strict=True):
            self.assertEqual(edit.original_record, original)
            effect_offset = 0x34 + (edit.target_slot - 1) * 0x18 + 4
            self.assertEqual(
                struct.unpack_from("<I", edit.replacement_record, effect_offset)[0],
                CONTEXTUAL_TEST_EFFECT_ID,
            )
            changed = {
                offset
                for offset, (before, after) in enumerate(
                    zip(original, edit.replacement_record, strict=True)
                )
                if before != after
            }
            self.assertTrue(changed)
            self.assertTrue(changed <= set(range(effect_offset, effect_offset + 4)))

    def test_existing_babd_experiment_rejects_mismatched_rarity_mirrors(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        for slot, seed in enumerate((201, 202, 203)):
            record = bytearray(make_record(seed=seed, account_id=account))
            struct.pack_into("<H", record, 0, 0xE604)
            record[0x30] = 4
            record[0x31] = 5
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            save[offset:offset + SCROLL_RECORD_SIZE] = record
        inventory = SaveInventory.load(save_path, bytes(save))

        with self.assertRaisesRegex(RuntimeError, "没有三张"):
            build_existing_contextual_babd_experiment(inventory)

    def test_edit_many_requires_exact_original_and_preserves_slot_positions(self) -> None:
        class FakeCrypto:
            @staticmethod
            def decrypt(source: Path, output: Path) -> None:
                encrypted = source.read_bytes()
                if not encrypted.startswith(b"ENC"):
                    raise AssertionError("expected fake encrypted input")
                output.write_bytes(encrypted[3:])

            @staticmethod
            def encrypt(source: Path, output: Path) -> None:
                output.write_bytes(b"ENC" + source.read_bytes())

        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            system_directory = root / str(account) / "SYSTEMSAVEDATA00"
            save_directory.mkdir(parents=True)
            system_directory.mkdir()
            save_path = save_directory / "SAVEDATA.BIN"
            (save_directory / "BACKUP.BIN").write_bytes(b"game backup")
            (system_directory / "SAVEDATA.BIN").write_bytes(b"system save")
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            first = make_record(seed=101, account_id=account)
            second = make_record(seed=102, account_id=account)
            for slot, record in ((0, first), (2, second)):
                offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
                decrypted[offset:offset + SCROLL_RECORD_SIZE] = record
            save_path.write_bytes(b"ENC" + bytes(decrypted))
            replacement_first = bytearray(first)
            replacement_second = bytearray(second)
            struct.pack_into("<I", replacement_first, 0x38, 0xBABD)
            struct.pack_into("<I", replacement_second, 0x50, 0xBABD)
            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )

            result = installer.edit_many(
                (
                    (0, first, bytes(replacement_first)),
                    (2, second, bytes(replacement_second)),
                ),
                action="test-edit-many",
                metadata={"controlled": True},
            )

            self.assertEqual(result.slot_indices, (0, 2))
            installed = save_path.read_bytes()[3:]
            self.assertEqual(
                struct.unpack_from("<I", installed, SCROLL_GROUP_OFFSET + 0x38)[0],
                0xBABD,
            )
            second_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
            self.assertEqual(
                struct.unpack_from("<I", installed, second_offset + 0x50)[0],
                0xBABD,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["slot_indices"], [0, 2])
            self.assertEqual(report["metadata"], {"controlled": True})
            self.assertEqual(len(report["backup_files"]), 3)

    def test_inventory_entries_preserve_physical_slots_and_parse_fields(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        decrypted = bytearray(USER_SAVE_SIZE)
        decrypted[:6] = b"RNNUSR"
        first = bytearray(make_record(seed=101, account_id=account))
        struct.pack_into("<H", first, 0, 0xE604)
        second = bytearray(make_record(seed=202, account_id=account))
        struct.pack_into("<H", second, 0, 0x516D)
        for slot, record in ((2, first), (9, second)):
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            decrypted[offset:offset + SCROLL_RECORD_SIZE] = record

        entries = SaveInventory.load(save_path, bytes(decrypted)).scroll_entries()

        self.assertEqual(tuple(entry.slot_index for entry in entries), (2, 9))
        self.assertEqual(tuple(entry.seed for entry in entries), (101, 202))
        self.assertEqual(tuple(entry.playthrough for entry in entries), (3, 2))
        self.assertEqual(entries[0].record_offset, SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE)
        self.assertEqual(entries[0].candidate.primary.effect_id, 0x47BC)

    def test_nonzero_type_zero_record_is_visible_but_never_a_scroll_template(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        decrypted = bytearray(USER_SAVE_SIZE)
        decrypted[:6] = b"RNNUSR"
        valid = bytearray(make_record(seed=101, account_id=account))
        struct.pack_into("<H", valid, 0, 0xE604)
        invalid = bytearray(SCROLL_RECORD_SIZE)
        invalid[0x20:0x24] = (114514).to_bytes(4, "little")
        for slot, record in ((0, valid), (1, invalid)):
            offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
            decrypted[offset:offset + SCROLL_RECORD_SIZE] = record

        inventory = SaveInventory.load(save_path, bytes(decrypted))
        entries = inventory.scroll_entries(include_unmapped=True)

        self.assertEqual(tuple(entry.slot_index for entry in entries), (0, 1))
        self.assertEqual(entries[1].record_type, 0)
        self.assertIsNone(entries[1].playthrough)
        self.assertFalse(entries[1].is_mapped_scroll)
        self.assertEqual(
            tuple(entry.slot_index for entry in inventory.scroll_entries()),
            (0,),
        )
        self.assertEqual(
            tuple(record_type for record_type, _record in inventory.template_records),
            (0xE604,),
        )

    def test_local_effect_patch_changes_only_requested_fields(self) -> None:
        original = make_record(seed=101)
        replacement = patch_local_scroll_record(
            original,
            (
                LocalEffectEdit(slot_index=0, effect_id=0xBABD, value=114514),
                LocalEffectEdit(slot_index=4, value=0xFFFFFFFE),
            ),
        )
        expected_changed = set(range(0x38, 0x40)) | set(range(0x9C, 0xA0))
        actual_changed = {
            index
            for index, (before, after) in enumerate(
                zip(original, replacement, strict=True)
            )
            if before != after
        }
        self.assertTrue(actual_changed <= expected_changed)
        self.assertEqual(struct.unpack_from("<I", replacement, 0x38)[0], 0xBABD)
        self.assertEqual(struct.unpack_from("<I", replacement, 0x3C)[0], 114514)
        self.assertEqual(struct.unpack_from("<I", replacement, 0x9C)[0], 0xFFFFFFFE)
        self.assertEqual(replacement[:0x34], original[:0x34])

    def test_delete_many_clears_records_without_compacting_neighbors(self) -> None:
        class FakeCrypto:
            @staticmethod
            def decrypt(source: Path, output: Path) -> None:
                output.write_bytes(source.read_bytes()[3:])

            @staticmethod
            def encrypt(source: Path, output: Path) -> None:
                output.write_bytes(b"ENC" + source.read_bytes())

        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            for slot, seed in ((0, 101), (1, 102), (3, 103)):
                offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
                decrypted[offset:offset + SCROLL_RECORD_SIZE] = make_record(
                    seed=seed,
                    account_id=account,
                )
            save_path.write_bytes(b"ENC" + bytes(decrypted))
            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            inventory = installer.capture_inventory()
            entry = next(item for item in inventory.scroll_entries() if item.slot_index == 1)

            result = installer.delete_many((entry,))

            installed = save_path.read_bytes()[3:]
            deleted_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            self.assertEqual(
                installed[deleted_offset:deleted_offset + SCROLL_RECORD_SIZE],
                bytes(SCROLL_RECORD_SIZE),
            )
            self.assertEqual(
                struct.unpack_from(
                    "<I",
                    installed,
                    SCROLL_GROUP_OFFSET + 3 * SCROLL_RECORD_SIZE + 0x20,
                )[0],
                103,
            )
            self.assertEqual(result.slot_indices, (1,))
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["action"], "local-scroll-delete")
            self.assertTrue(report["metadata"]["local_only"])

    def test_backup_listing_and_restore_checkpoint_all_save_roles(self) -> None:
        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            system_directory = root / str(account) / "SYSTEMSAVEDATA00"
            save_directory.mkdir(parents=True)
            system_directory.mkdir()
            save_path = save_directory / "SAVEDATA.BIN"
            backup_path = save_directory / "BACKUP.BIN"
            system_path = system_directory / "SAVEDATA.BIN"
            save_path.write_bytes(b"current main")
            backup_path.write_bytes(b"current game backup")
            system_path.write_bytes(b"current system")

            state_root = root / "state"
            source_directory = state_root / "backups" / "source-bundle"
            source_directory.mkdir(parents=True)
            (source_directory / "SAVEDATA.BIN").write_bytes(b"old main")
            (source_directory / "BACKUP.BIN").write_bytes(b"old game backup")
            (source_directory / "SYSTEMSAVEDATA.BIN").write_bytes(b"old system")
            (source_directory / "edit-report.json").write_text(
                json.dumps(
                    {
                        "action": "local-effect-edit",
                        "steam_account_id": account,
                    }
                ),
                encoding="utf-8",
            )

            entries = list_backup_entries(state_root)
            self.assertEqual(len(entries), 1)
            self.assertIsInstance(entries[0], BackupEntry)
            self.assertEqual(entries[0].action, "local-effect-edit")
            self.assertEqual(entries[0].account_id, account)
            self.assertEqual(entries[0].file_count, 4)
            self.assertIsNotNone(entries[0].main_save_sha256)

            installer = SaveInstaller(
                save_path=save_path,
                crypto=object(),  # restore is an encrypted-file transaction
                state_root=state_root,
            )
            result = installer.restore_backup(source_directory)

            self.assertEqual(save_path.read_bytes(), b"old main")
            self.assertEqual(backup_path.read_bytes(), b"old game backup")
            self.assertEqual(system_path.read_bytes(), b"old system")
            self.assertEqual(
                (result.checkpoint_directory / "SAVEDATA.BIN").read_bytes(),
                b"current main",
            )
            self.assertEqual(
                (result.checkpoint_directory / "BACKUP.BIN").read_bytes(),
                b"current game backup",
            )
            self.assertEqual(
                (result.checkpoint_directory / "SYSTEMSAVEDATA.BIN").read_bytes(),
                b"current system",
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["action"], "restore-backup")
            self.assertEqual(
                report["restored_targets"],
                ["system_save", "game_backup", "main_save"],
            )
            self.assertNotIn(str(root), result.report_path.read_text(encoding="utf-8"))
            self.assertTrue(source_directory.is_dir())

    def test_restore_rejects_directory_outside_managed_backup_root(self) -> None:
        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            save_path.write_bytes(b"current")
            outside = root / "outside"
            outside.mkdir()
            (outside / "SAVEDATA.BIN").write_bytes(b"old")
            installer = SaveInstaller(
                save_path=save_path,
                crypto=object(),
                state_root=root / "state",
            )

            with self.assertRaisesRegex(ValueError, "backups"):
                installer.restore_backup(outside)

            self.assertEqual(save_path.read_bytes(), b"current")

    def test_install_many_uses_one_backup_and_consecutive_slots(self) -> None:
        class FakeCrypto:
            @staticmethod
            def decrypt(source: Path, output: Path) -> None:
                encrypted = source.read_bytes()
                if not encrypted.startswith(b"ENC"):
                    raise AssertionError("expected fake encrypted input")
                output.write_bytes(encrypted[3:])

            @staticmethod
            def encrypt(source: Path, output: Path) -> None:
                output.write_bytes(b"ENC" + source.read_bytes())

        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            system_directory = root / str(account) / "SYSTEMSAVEDATA00"
            save_directory.mkdir(parents=True)
            system_directory.mkdir()
            save_path = save_directory / "SAVEDATA.BIN"
            (save_directory / "BACKUP.BIN").write_bytes(b"game backup")
            (system_directory / "SAVEDATA.BIN").write_bytes(b"system save")
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            original = make_record(account_id=account)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = original
            save_path.write_bytes(b"ENC" + bytes(decrypted))

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            records = tuple(
                make_record(seed=seed, account_id=account)
                for seed in (101, 102, 103)
            )
            result = installer.install_many(
                records,
                action="test-batch",
                metadata={"test": True},
            )

            self.assertEqual(result.slot_indices, (1, 2, 3))
            installed = save_path.read_bytes()[3:]
            for slot, seed in zip(result.slot_indices, (101, 102, 103), strict=True):
                offset = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_SIZE
                self.assertEqual(struct.unpack_from("<I", installed, offset + 0x20)[0], seed)
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["action"], "test-batch")
            self.assertEqual(report["slot_indices"], [1, 2, 3])
            self.assertEqual(report["metadata"], {"test": True})
            self.assertEqual(len(report["backup_files"]), 3)

    def test_install_backs_up_then_uses_the_next_slot_and_redacts_paths(self) -> None:
        class FakeCrypto:
            @staticmethod
            def decrypt(source: Path, output: Path) -> None:
                encrypted = source.read_bytes()
                if not encrypted.startswith(b"ENC"):
                    raise AssertionError("expected fake encrypted input")
                output.write_bytes(encrypted[3:])

            @staticmethod
            def encrypt(source: Path, output: Path) -> None:
                output.write_bytes(b"ENC" + source.read_bytes())

        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            system_directory = root / str(account) / "SYSTEMSAVEDATA00"
            save_directory.mkdir(parents=True)
            system_directory.mkdir()
            save_path = save_directory / "SAVEDATA.BIN"
            backup_path = save_directory / "BACKUP.BIN"
            system_path = system_directory / "SAVEDATA.BIN"

            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            original = make_record(account_id=account)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = original
            encrypted = b"ENC" + bytes(decrypted)
            save_path.write_bytes(encrypted)
            backup_path.write_bytes(b"game backup")
            system_path.write_bytes(b"system save")

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            candidate = make_record(seed=114514, account_id=account)
            result = installer.install(candidate, transfer_count=7)

            installed = save_path.read_bytes()[3:]
            second = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            self.assertEqual(installed[SCROLL_GROUP_OFFSET:second], original)
            self.assertEqual(
                struct.unpack_from("<I", installed, second + 0x20)[0], 114514
            )
            self.assertEqual(struct.unpack_from("<I", installed, second + 0xDC)[0], 7)
            self.assertEqual(result.slot_index, 1)
            self.assertTrue((result.backup_directory / "SAVEDATA.BIN").is_file())
            self.assertTrue((result.backup_directory / "BACKUP.BIN").is_file())
            self.assertTrue((result.backup_directory / "SYSTEMSAVEDATA.BIN").is_file())
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["steam_account_id"], account)
            self.assertNotIn("save_path", report)
            rendered = json.dumps(report)
            self.assertNotIn(str(root), rendered)

    def test_install_effect_sequence_candidate_materializes_inside_hash_gate(self) -> None:
        class FakeCrypto:
            @staticmethod
            def decrypt(source: Path, output: Path) -> None:
                encrypted = source.read_bytes()
                if not encrypted.startswith(b"ENC"):
                    raise AssertionError("expected fake encrypted input")
                output.write_bytes(encrypted[3:])

            @staticmethod
            def encrypt(source: Path, output: Path) -> None:
                output.write_bytes(b"ENC" + source.read_bytes())

        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            system_directory = root / str(account) / "SYSTEMSAVEDATA00"
            save_directory.mkdir(parents=True)
            system_directory.mkdir()
            save_path = save_directory / "SAVEDATA.BIN"
            (save_directory / "BACKUP.BIN").write_bytes(b"game backup")
            (system_directory / "SAVEDATA.BIN").write_bytes(b"system save")
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            template = bytearray(make_record(seed=241719428, account_id=account))
            struct.pack_into("<H", template, 0, 0xE604)
            struct.pack_into("<I", template, 0x28, 100)
            decrypted[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = template
            save_path.write_bytes(b"ENC" + bytes(decrypted))
            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            preview = ScrollCandidate.from_effect_sequence(
                generate_ng3_rarity5_effect_sequence(1, level=180)
            )

            result = installer.install_effect_sequence_candidate(
                preview,
                level=180,
                recommended_level=183,
                transfer_count=7,
            )

            installed = save_path.read_bytes()[3:]
            offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            record = installed[offset:offset + SCROLL_RECORD_SIZE]
            self.assertEqual(result.slot_index, 1)
            self.assertEqual(struct.unpack_from("<H", record, 0)[0], 0xE604)
            self.assertEqual(struct.unpack_from("<I", record, 0x20)[0], 1)
            self.assertEqual(struct.unpack_from("<I", record, 0x28)[0], 101)
            self.assertEqual(struct.unpack_from("<I", record, 0xDC)[0], 7)
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(
                report["metadata"]["materializer"],
                "game-closed-ng3-rarity5-v2.00.02",
            )
            self.assertEqual(report["metadata"]["native_full_record_parity_vectors"], 10_000)

    def test_install_rejects_stale_materialization_hash_before_backup(self) -> None:
        account = TEST_ACCOUNT_ID
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_directory = root / str(account) / "SAVEDATA00"
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            save_path.write_bytes(b"not-the-expected-save")
            installer = SaveInstaller(
                save_path=save_path,
                crypto=object(),
                state_root=root / "state",
            )

            with self.assertRaisesRegex(RuntimeError, "候选绑定后"):
                installer.install(
                    make_record(account_id=account),
                    transfer_count=0,
                    expected_source_sha256="00" * 32,
                )

            self.assertFalse((root / "state" / "backups").exists())


if __name__ == "__main__":
    unittest.main()
