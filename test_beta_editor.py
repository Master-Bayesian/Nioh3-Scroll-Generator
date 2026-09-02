import json
import struct
import tempfile
import unittest
from unittest.mock import patch
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
from nioh3_scroll_editor.effect_generation_tables import (
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.app import (
    ENEMY_COMBINATION_GUIDE_TEXT,
    ENEMY_TIER_HIGH,
    ENEMY_TIER_LOW,
    ENEMY_TIER_MIDDLE,
    ENEMY_TIER_MIDDLE_HIGH,
    FAQ_TEXT,
    FEATURE_GUIDE_TEXT,
    PRODUCT_RARITIES,
    QUICK_START_TEXT,
    TITLE_SCREEN_ACK_TEXT,
    TITLE_SCREEN_PROMPT_TEXT,
    SearchBatchResult,
    application_title,
    collect_search_pages_until_requested,
    collect_offline_ng3_search_batch,
    collect_offline_rarity5_search_batch,
    build_runtime_terrain_options,
    build_terrain_filter_options,
    combine_terrain_filter_rows,
    classify_enemy_roles,
    centered_child_geometry,
    compile_secondary_effect_requirements,
    copy_text_to_clipboard,
    default_window_dimensions,
    format_local_auxiliary_preview,
    initial_search_filter_width,
    is_cached_game_closed_effect_context,
    is_game_closed_effect_context,
    legal_enemy_display_groups,
    local_effect_raw_value_hint,
    partition_grouped_selections,
    partial_effect_batch_generator,
    enemy_tier_columns,
    enemy_tiers_are_compatible,
    enemy_variant_display_name,
    filter_runtime_labels,
    format_runtime_enemy_slot_summary,
    requirement_mode_group_index,
    require_accelerated_generic_search,
    special_rule_variant_label,
    split_enemy_variant_display_groups,
    toggle_rule_filter_option,
    user_facing_error_message,
)
from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.auxiliary_generation import (
    AuxiliarySearchCriteria,
    TERRAIN_DISPLAY_CRUCIBLE_KEY,
    TERRAIN_DISPLAY_SPECIAL_KEYS,
    generate_complete_auxiliary,
    load_default_auxiliary_generation_tables,
    terrain_display_effect_keys_for_row,
    terrain_rows_containing_effects,
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
    LocalEffectSlotFields,
    SCROLL_GROUP_OFFSET,
    SaveInstaller,
    SaveInventory,
    list_backup_entries,
    materialize_effect_sequence_candidate,
    next_generation_serial,
    patch_local_scroll_header,
    patch_local_scroll_record,
    patch_local_scroll_seed,
    prepare_candidate_for_install,
    read_local_effect_slots,
    read_local_scroll_header,
    retarget_local_effect_identity,
)
from nioh3_scroll_editor.experiments import (
    CONTEXTUAL_TEST_EFFECT_ID,
    build_contextual_babd_experiment,
    build_existing_contextual_babd_experiment,
)
from nioh3_scroll_editor.runtime_auxiliary_override import (
    DESCRIPTOR_COMPLETE_BYTES,
    RuntimeAuxiliaryOverrideProfile,
    build_override_trampoline,
    build_relative_jump,
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
    def test_ui_search_automatically_continues_bounded_pages(self) -> None:
        calls: list[tuple[int, int]] = []

        def collect(remaining: int, cursor: int) -> SearchBatchResult:
            calls.append((remaining, cursor))
            candidate = ScrollCandidate.from_effect_sequence(
                generate_ng3_rarity5_effect_sequence(cursor + 1)
            )
            return SearchBatchResult(
                candidates=(candidate,),
                requested_count=remaining,
                next_start_after_trial=cursor + 1_000_000,
                streamed=True,
            )

        result = collect_search_pages_until_requested(
            collect,
            result_count=20,
        )

        self.assertEqual(len(result.candidates), 20)
        self.assertEqual(len(calls), 20)
        self.assertEqual(calls[0], (20, 0))
        self.assertEqual(calls[-1], (1, 19_000_000))

    def test_intel_auxiliary_search_requires_explicit_cpu_consent(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            auxiliary_criteria=AuxiliarySearchCriteria(
                required_special_rule_key_groups=(frozenset((0x0071,)),),
            ),
        )
        with (
            patch(
                "nioh3_scroll_editor.app.cuda_seed_acceleration_available",
                return_value=False,
            ),
            patch(
                "nioh3_scroll_editor.app.d3d11_effect_acceleration_available",
                return_value=True,
            ),
        ):
            with self.assertRaisesRegex(RuntimeError, "CUDA"):
                require_accelerated_generic_search(request)
            require_accelerated_generic_search(
                request,
                allow_cpu_fallback=True,
            )

    def test_r4_primary_search_uses_directcompute_when_cuda_self_test_fails(self) -> None:
        request = EffectSeedRequest(
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset((0xB613,)),
        )
        with patch(
            "nioh3_scroll_editor.app.cuda_seed_acceleration_available",
            return_value=False,
        ):
            generator = partial_effect_batch_generator(
                request,
                grace_mapping=load_grace_output_map(rarity=4),
                level=180,
            )
        self.assertIsNotNone(generator)

        with patch(
            "nioh3_scroll_editor.app.cuda_seed_acceleration_available",
            return_value=True,
        ):
            generator = partial_effect_batch_generator(
                request,
                grace_mapping=load_grace_output_map(rarity=4),
                level=180,
            )
        self.assertIsNone(generator)

    def test_default_window_uses_available_desktop_width(self) -> None:
        self.assertEqual(default_window_dimensions(1920, 1080), (1880, 1000))
        self.assertEqual(default_window_dimensions(2560, 1440), (2040, 1160))
        self.assertEqual(default_window_dimensions(1366, 768), (1326, 688))

    def test_initial_search_split_prioritizes_filter_controls(self) -> None:
        self.assertEqual(initial_search_filter_width(1820), 860)
        self.assertEqual(initial_search_filter_width(1660), 860)
        self.assertEqual(initial_search_filter_width(1440), 820)
        self.assertEqual(initial_search_filter_width(1100), 550)
        self.assertEqual(initial_search_filter_width(0), 860)

    def test_child_dialog_is_centered_and_screen_bounded(self) -> None:
        self.assertEqual(
            centered_child_geometry(
                parent_x=163,
                parent_y=107,
                parent_width=2043,
                parent_height=1165,
                child_width=680,
                child_height=540,
                screen_width=2560,
                screen_height=1440,
            ),
            "680x540+844+419",
        )
        self.assertEqual(
            centered_child_geometry(
                parent_x=1200,
                parent_y=700,
                parent_width=900,
                parent_height=700,
                child_width=680,
                child_height=540,
                screen_width=1600,
                screen_height=900,
            ),
            "680x540+920+360",
        )

    def test_grouped_selections_preserve_and_or_semantics(self) -> None:
        mandatory, groups = partition_grouped_selections(
            {10, 20, 30, 40},
            {20: 2, 30: 1, 40: 2},
        )
        self.assertEqual(mandatory, (10,))
        self.assertEqual(groups, (frozenset((30,)), frozenset((20, 40))))
        self.assertEqual(requirement_mode_group_index("必含"), 0)
        self.assertEqual(requirement_mode_group_index("任一组 3"), 3)

    def test_primary_candidates_can_be_required_as_secondary_fallbacks(self) -> None:
        mandatory, groups = compile_secondary_effect_requirements(
            {30, 40},
            {40: 1},
            {10, 20},
        )
        self.assertEqual(mandatory, frozenset((10, 20, 30)))
        self.assertEqual(groups, (frozenset((40,)),))

        a, b = 0x47BC, 0x4647
        paired_required, _ = compile_secondary_effect_requirements(
            set(),
            {},
            {a, b},
        )
        paired = ScrollCandidate.from_record(
            make_record(effects=(a, b, 0xA051, 0x190A, 0x2B06, 0xB613))
        )
        missing_partner = ScrollCandidate.from_record(
            make_record(effects=(a, 0xDFF0, 0xA051, 0x190A, 0x2B06, 0xB613))
        )
        self.assertTrue(
            candidate_matches(
                paired,
                primary_effect_ids=frozenset((a, b)),
                required_secondary_ids=paired_required,
            )
        )
        self.assertFalse(
            candidate_matches(
                missing_partner,
                primary_effect_ids=frozenset((a, b)),
                required_secondary_ids=paired_required,
            )
        )

    def test_player_enemy_options_exclude_localization_only_names(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        tables = load_default_auxiliary_generation_tables()
        candidate_keys = {
            struct.unpack_from("<I", row, 0x04)[0]
            for row in tables.enemy_candidates.rows()
        }
        groups = legal_enemy_display_groups(
            catalog.enemy_key_groups(),
            candidate_keys,
        )
        self.assertEqual(len(groups), 142)
        self.assertTrue(all(keys.issubset(candidate_keys) for keys in groups.values()))

    def test_named_boss_variants_are_exposed_as_exact_choices(self) -> None:
        groups = split_enemy_variant_display_groups(
            {
                "武田信玄": frozenset((0x00071ED1, 0x0000A5D2)),
                "比留呼": frozenset((0x000B605B, 0x0004ACDF, 0x00093F79)),
                "金井半兵卫": frozenset((0x0000F9CA, 0x000179F7)),
            }
        )
        self.assertEqual(groups["武田信玄（人形）"], frozenset((0x00071ED1,)))
        self.assertEqual(groups["武田信玄（妖怪形态）"], frozenset((0x0000A5D2,)))
        self.assertEqual(groups["比留呼（古代妖怪形态）"], frozenset((0x0004ACDF,)))
        self.assertEqual(groups["比留呼（江户妖怪形态）"], frozenset((0x00093F79,)))
        self.assertEqual(groups["金井半兵卫（人形）"], frozenset((0x0000F9CA,)))
        self.assertNotIn("武田信玄", groups)

    def test_enemy_variant_names_distinguish_both_hattori_hanzo_identities(self) -> None:
        self.assertEqual(
            enemy_variant_display_name("服部半藏", 0x000D35E1),
            "服部半藏（现任／子）",
        )
        self.assertEqual(
            enemy_variant_display_name("服部半藏", 0x000202A7),
            "服部半藏（先代／父（鬼半藏））",
        )

    def test_player_enemy_tiers_match_native_role_families(self) -> None:
        self.assertEqual(classify_enemy_roles((0, 1, 2, 3)), ENEMY_TIER_LOW)
        self.assertEqual(classify_enemy_roles((4,)), ENEMY_TIER_MIDDLE)
        self.assertEqual(classify_enemy_roles((5,)), ENEMY_TIER_HIGH)
        self.assertEqual(
            classify_enemy_roles((4, 5)),
            ENEMY_TIER_MIDDLE_HIGH,
        )
        self.assertEqual(
            enemy_tier_columns(ENEMY_TIER_MIDDLE_HIGH),
            (ENEMY_TIER_MIDDLE, ENEMY_TIER_HIGH),
        )
        with self.assertRaises(ValueError):
            classify_enemy_roles((1, 4))

    def test_runtime_enemy_override_requires_a_compatible_native_tier(self) -> None:
        self.assertTrue(enemy_tiers_are_compatible(ENEMY_TIER_LOW, ENEMY_TIER_LOW))
        self.assertTrue(
            enemy_tiers_are_compatible(
                ENEMY_TIER_MIDDLE,
                ENEMY_TIER_MIDDLE_HIGH,
            )
        )
        self.assertFalse(
            enemy_tiers_are_compatible(ENEMY_TIER_LOW, ENEMY_TIER_HIGH)
        )

    def test_runtime_enemy_search_filters_name_and_hex_id(self) -> None:
        values = (
            "[低手] 一目连 [0x0005AAF9]",
            "[高手] 高杉晋作 [0x000BE2C7]",
        )
        self.assertEqual(filter_runtime_labels(values, "一目连"), values[:1])
        self.assertEqual(filter_runtime_labels(values, "be2c7"), values[1:])
        self.assertEqual(filter_runtime_labels(values, ""), values)

    def test_runtime_enemy_slot_summary_explains_missing_tiers(self) -> None:
        summary = format_runtime_enemy_slot_summary(
            (ENEMY_TIER_LOW, ENEMY_TIER_LOW, ENEMY_TIER_HIGH)
        )
        self.assertIn("低手×2", summary)
        self.assertIn("高手×1", summary)
        self.assertIn("未出现的档位不是漏项", summary)
        self.assertNotIn("中手×", summary)

    def test_runtime_terrain_options_use_player_visible_names(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        tables = load_default_auxiliary_generation_tables()
        options = build_runtime_terrain_options(tables, catalog)
        self.assertEqual(
            set(options),
            {"无地形影响", "地狱", "地狱＋火", "恶臭", "地狱＋瘴血"},
        )
        self.assertEqual(options["地狱＋火"], 0x2D)
        self.assertEqual(options["地狱＋瘴血"], 0x08)

    def test_search_terrain_options_are_exact_native_results(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        tables = load_default_auxiliary_generation_tables()
        options = build_terrain_filter_options(tables, catalog)

        self.assertEqual(
            options["含有地狱（任意组合）"],
            frozenset((0, 6, 7, 11, 14, 17)),
        )
        self.assertEqual(
            options["无地形影响"],
            frozenset((1, 2, 3, 4, 5, 8, 9, 12, 13, 15, 16, 18, 19)),
        )
        self.assertEqual(options["地狱（仅地狱）"], frozenset((0, 7, 11, 17)))
        self.assertEqual(options["地狱＋火"], frozenset((6,)))
        self.assertEqual(options["恶臭"], frozenset((10,)))
        self.assertEqual(options["地狱＋瘴血"], frozenset((14,)))

    def test_search_terrain_options_combine_with_or_semantics(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        tables = load_default_auxiliary_generation_tables()
        options = build_terrain_filter_options(tables, catalog)

        self.assertEqual(
            combine_terrain_filter_rows(
                ("地狱＋火", "恶臭"),
                options,
            ),
            frozenset((6, 10)),
        )
        self.assertEqual(
            combine_terrain_filter_rows(
                ("含有地狱（任意组合）", "恶臭"),
                options,
            ),
            frozenset((0, 6, 7, 10, 11, 14, 17)),
        )

    def test_terrain_effect_filter_supports_native_multi_effect_rows(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        foulblood = TERRAIN_DISPLAY_SPECIAL_KEYS[0x08]
        fire = TERRAIN_DISPLAY_SPECIAL_KEYS[0x2D]

        self.assertEqual(
            terrain_display_effect_keys_for_row(14, tables=tables),
            frozenset((TERRAIN_DISPLAY_CRUCIBLE_KEY, foulblood)),
        )
        self.assertEqual(
            terrain_rows_containing_effects(
                frozenset((TERRAIN_DISPLAY_CRUCIBLE_KEY, foulblood)),
                tables=tables,
            ),
            (14,),
        )
        self.assertEqual(
            terrain_rows_containing_effects(
                frozenset((fire, foulblood)),
                tables=tables,
            ),
            (),
        )

    def test_enemy_combination_guide_describes_complete_group_structures(self) -> None:
        for structure in (
            "中＋高",
            "高＋高",
            "中＋中＋高",
            "中＋高＋高",
            "高＋高＋高",
            "低＋低＋高",
            "低＋低＋低",
            "低＋低＋低＋高",
            "低＋低＋低＋低",
            "低＋低＋低＋低＋低",
        ):
            self.assertIn(structure, ENEMY_COMBINATION_GUIDE_TEXT)
        self.assertIn("必须至少包含", ENEMY_COMBINATION_GUIDE_TEXT)
        self.assertIn("不代表实际战斗强弱", ENEMY_COMBINATION_GUIDE_TEXT)

    def test_qq_group_copy_uses_non_modal_system_clipboard_path(self) -> None:
        class FakeRoot:
            def __init__(self) -> None:
                self.value = "old"
                self.updated = False

            def clipboard_clear(self) -> None:
                self.value = ""

            def clipboard_append(self, value: str) -> None:
                self.value += value

            def update_idletasks(self) -> None:
                self.updated = True

        root = FakeRoot()
        copy_text_to_clipboard(root, "  1106302479  ")

        self.assertEqual(root.value, "1106302479")
        self.assertTrue(root.updated)

    def test_normal_title_does_not_expose_internal_safety_mode(self) -> None:
        self.assertEqual(application_title(research_mode=False), "仁王3绘卷生成器")
        self.assertEqual(
            application_title(research_mode=True),
            "仁王3绘卷生成器（研究模式）",
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
        self.assertIn("它不负责设置挑战敌人等级", FAQ_TEXT)
        self.assertIn("敌人/Boss 等级按该最终值执行", FAQ_TEXT)

    def test_product_ui_exposes_only_supported_rarities(self) -> None:
        self.assertEqual(PRODUCT_RARITIES, (3, 4, 5))
        self.assertIn("三种都保留搜索", FAQ_TEXT)

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
            max_trials_per_batch=100,
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

    def test_rarity4_two_effect_groups_use_exact_gpu_finalizer(self) -> None:
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
        candidate = result.candidates[0]
        self.assertEqual(candidate.primary.effect_id, 0xB613)
        self.assertIn(0x23E8, {effect.effect_id for effect in candidate.secondaries})

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
            max_trials_per_batch=100,
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

    def test_shinatsuhiko_is_named_and_selectable_for_rarity_four_and_five(self) -> None:
        for rarity in (4, 5):
            choices = {
                effect.effect_id: effect.name
                for effect in target_effects_for_rarity(rarity)
            }
            self.assertEqual(choices[0x4192], "志那都彦的恩宠")
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
            0x3E7A: "精力恢复速度", 0xB82B: "敌人精力耗尽时赋予受到伤害增加",
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
        searchable = {
            effect.effect_id: effect.name
            for effect in searchable_scroll_effect_definitions(3, 4)
        }
        self.assertEqual(
            searchable[0xB82B],
            "敌人精力耗尽时赋予受到伤害增加",
        )
        self.assertNotIn("BUFF", searchable[0xB82B])

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
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((0x47BC,)),
                required_secondary_ids=frozenset(),
                required_secondary_id_groups=(
                    frozenset((0xDFF0, 0xA051)),
                    frozenset((0x190A, 0xBABD)),
                ),
            )
        )
        self.assertFalse(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset((0x47BC,)),
                required_secondary_ids=frozenset(),
                required_secondary_id_groups=(frozenset((0xDFF0, 0xBABD)),),
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

    def test_unconstrained_primary_allows_any_ordinary_slot(self) -> None:
        # With no explicit primary target, A/B/C mean "appear in any ordinary
        # slot".  The actual primary may therefore satisfy A.
        a, b, c = 0x47BC, 0x4647, 0xA051
        candidate = ScrollCandidate.from_record(
            make_record(effects=(a, b, c, 0x190A, 0x2B06, 0xB613))
        )
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset((a, b, c)),
            )
        )

    def test_unconstrained_primary_can_satisfy_an_any_group(self) -> None:
        a, b = 0x47BC, 0xDFF0
        candidate = ScrollCandidate.from_record(
            make_record(effects=(a, 0x4647, 0xA051, 0x190A, 0x2B06, 0xB613))
        )
        self.assertTrue(
            candidate_matches(
                candidate,
                primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset(),
                required_secondary_id_groups=(frozenset((a, b)),),
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

    def test_inventory_reuses_zero_hole_before_an_occupied_tail_slot(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = make_record(account_id=account)
        for slot_index in range(164):
            offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            save[offset : offset + SCROLL_RECORD_SIZE] = template
        # Reproduce the false-full condition: an unrelated or malformed record
        # at the physical tail must not hide the 235 all-zero slots before it.
        tail = SCROLL_GROUP_OFFSET + 399 * SCROLL_RECORD_SIZE
        save[tail] = 1

        inventory = SaveInventory.load(save_path, bytes(save))

        self.assertEqual(len(inventory.scroll_entries()), 164)
        self.assertEqual(len(inventory.empty_slots), 235)
        self.assertEqual(inventory.next_slot_index, 164)

    def test_inventory_treats_type_zero_stale_records_as_free_slots(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = make_record(account_id=account)
        for slot_index in range(164):
            offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            save[offset : offset + SCROLL_RECORD_SIZE] = template
        for slot_index in range(164, 400):
            offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            # Native deletion/free state: type is zero while stale payload
            # bytes remain. The old all-zero test falsely reported full here.
            save[offset + 2] = 0xA5
            save[offset + 0x20] = slot_index & 0xFF

        inventory = SaveInventory.load(save_path, bytes(save))

        self.assertEqual(len(inventory.scroll_entries()), 164)
        self.assertEqual(len(inventory.empty_slots), 236)
        self.assertEqual(inventory.next_slot_index, 164)

    def test_inventory_reports_full_only_when_all_400_slots_are_nonzero(self) -> None:
        account = TEST_ACCOUNT_ID
        save_path = Path(f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN")
        save = bytearray(USER_SAVE_SIZE)
        save[:6] = b"RNNUSR"
        template = make_record(account_id=account)
        for slot_index in range(400):
            offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
            save[offset : offset + SCROLL_RECORD_SIZE] = template

        inventory = SaveInventory.load(save_path, bytes(save))

        self.assertEqual(len(inventory.scroll_entries()), 400)
        self.assertEqual(inventory.empty_slots, ())
        self.assertIsNone(inventory.next_slot_index)

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

        self.assertEqual(
            materialized.record_stage,
            CandidateRecordStage.NATIVE_STAGE_ONE,
        )
        self.assertEqual(struct.unpack_from("<H", materialized.record, 0x0C)[0], 0)
        from nioh3_scroll_editor.r4_finalizer_engine import (
            load_default_r4_finalizer_engine,
        )

        completed = load_default_r4_finalizer_engine().finalize_completion(
            materialized.record
        )
        completed_candidate = ScrollCandidate.from_record(
            completed.record,
            playthrough=3,
            record_stage=CandidateRecordStage.FINAL_RECORD,
        )
        self.assertEqual(
            [effect.effect_id for effect in completed_candidate.effects[:5]],
            [effect.effect_id for effect in preview.effects],
        )

    def test_rarity4_install_avoids_seed_125804734_double_completion(self) -> None:
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
            generate_ng3_certified_effect_sequence(125_804_734, rarity=4, level=180)
        )

        materialized = materialize_effect_sequence_candidate(
            inventory,
            preview,
            level=180,
            recommended_level=183,
            transfer_count=0,
        )

        from nioh3_scroll_editor.r4_finalizer_engine import (
            load_default_r4_finalizer_engine,
        )

        finalizer = load_default_r4_finalizer_engine()
        revealed_once = finalizer.finalize_completion(materialized.record).record
        revealed_twice = finalizer.finalize_completion(revealed_once).record

        def effect_ids(record: bytes) -> tuple[int, ...]:
            return tuple(
                struct.unpack_from("<I", record, 0x38 + index * 0x18)[0]
                for index in range(5)
            )

        expected = (0x23E8, 0x190A, 0x2B06, 0xD40A, 0xBABD)
        self.assertEqual(effect_ids(revealed_once), expected)
        self.assertEqual(
            effect_ids(revealed_twice),
            (0x23E8, 0x190A, 0x2B06, 0x6AAF, 0xBABD),
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

    def test_nonzero_type_zero_record_is_a_native_free_slot(self) -> None:
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

        self.assertEqual(tuple(entry.slot_index for entry in entries), (0,))
        self.assertEqual(
            tuple(entry.slot_index for entry in inventory.scroll_entries()),
            (0,),
        )
        self.assertIn(1, inventory.empty_slots)
        self.assertEqual(inventory.next_slot_index, 1)
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

    def test_local_scroll_seed_patch_changes_only_seed_field(self) -> None:
        original = make_record(seed=101)
        replacement = patch_local_scroll_seed(original, 0x89ABCDEF)
        actual_changed = {
            index
            for index, (before, after) in enumerate(
                zip(original, replacement, strict=True)
            )
            if before != after
        }

        self.assertTrue(actual_changed <= set(range(0x20, 0x24)))
        self.assertEqual(struct.unpack_from("<I", replacement, 0x20)[0], 0x89ABCDEF)

    def test_local_scroll_header_patch_changes_only_exposed_fields(self) -> None:
        original = make_record(seed=101)
        replacement = patch_local_scroll_header(
            original,
            playthrough=5,
            level=0x1234,
            recommended_level=0x5678,
            seed=0x89ABCDEF,
            rarity=3,
            transfer_count=0x10203040,
        )
        editable_offsets = (
            set(range(0x00, 0x02))
            | set(range(0x06, 0x0A))
            | set(range(0x10, 0x14))
            | set(range(0x20, 0x24))
            | set(range(0x30, 0x32))
            | set(range(0xDC, 0xE0))
        )
        actual_changed = {
            index
            for index, (before, after) in enumerate(
                zip(original, replacement, strict=True)
            )
            if before != after
        }
        header = read_local_scroll_header(replacement)

        self.assertTrue(actual_changed <= editable_offsets)
        self.assertEqual(header.playthrough, 5)
        self.assertEqual(header.level, 0x1234)
        self.assertEqual(header.recommended_level, 0x5678)
        self.assertEqual(header.seed, 0x89ABCDEF)
        self.assertEqual(header.rarity, 3)
        self.assertEqual(header.transfer_count, 0x10203040)
        self.assertEqual(struct.unpack_from("<H", replacement, 0x08)[0], 0x1234)
        self.assertEqual(struct.unpack_from("<H", replacement, 0x12)[0], 0x5678)
        self.assertEqual(replacement[0x31], 3)
        self.assertEqual(replacement[0x34:0xDC], original[0x34:0xDC])

    def test_local_scroll_header_patch_rejects_unmapped_rarity_and_type(self) -> None:
        original = make_record(seed=101)
        valid = {
            "playthrough": 3,
            "level": 180,
            "recommended_level": 183,
            "seed": 101,
            "rarity": 4,
            "transfer_count": 0,
        }
        with self.assertRaisesRegex(ValueError, "playthrough"):
            patch_local_scroll_header(original, **(valid | {"playthrough": 0}))
        with self.assertRaisesRegex(ValueError, "rarity"):
            patch_local_scroll_header(original, **(valid | {"rarity": 2}))

    def test_local_auxiliary_preview_and_raw_value_hint_are_user_readable(self) -> None:
        auxiliary = generate_complete_auxiliary(203900415, 3)
        terrain, enemies, rules = format_local_auxiliary_preview(
            auxiliary,
            load_auxiliary_name_catalog("zh-CN"),
        )
        tables = load_default_effect_generation_tables()
        native_hint = local_effect_raw_value_hint(
            tables,
            effect_id=0x47BC,
            rarity=4,
            level=180,
            current_value=100,
        )
        unknown_hint = local_effect_raw_value_hint(
            tables,
            effect_id=0xDEADBEEF,
            rarity=4,
            level=180,
        )

        self.assertTrue(terrain.startswith("地形："))
        self.assertIn("原生行", terrain)
        self.assertIn("敌人（class ", enemies)
        self.assertTrue(rules.startswith("特殊规则："))
        self.assertIn("可输入 raw", native_hint)
        self.assertIn("原生 raw", native_hint)
        self.assertIn("没有可验证的原生数值范围", unknown_hint)

    def test_runtime_auxiliary_override_profile_is_explicitly_bounded(self) -> None:
        profile = RuntimeAuxiliaryOverrideProfile(
            seed=203900415,
            enemy_keys=(0x0006DE91, 0x0006DE91, 0x0006DE91),
            special_rule_keys=(0x6171, 0, 0),
            terrain_value=0x74,
        )

        self.assertEqual(len(profile.enemy_keys), 3)
        with self.assertRaisesRegex(ValueError, "change at least one"):
            RuntimeAuxiliaryOverrideProfile(seed=203900415)
        with self.assertRaisesRegex(ValueError, "eight enemy groups"):
            RuntimeAuxiliaryOverrideProfile(
                seed=203900415,
                enemy_keys=(0x0006DE91,) * 9,
            )
        with self.assertRaisesRegex(ValueError, "uint16"):
            RuntimeAuxiliaryOverrideProfile(
                seed=203900415,
                special_rule_keys=(0x10000, 0, 0),
            )

    def test_runtime_auxiliary_trampoline_replays_instruction_and_returns(self) -> None:
        return_address = 0x00007FF61234567D
        profile = RuntimeAuxiliaryOverrideProfile(
            seed=0x0C2745FF,
            enemy_keys=(0x0006DE91,) * 3,
            special_rule_keys=(0x6171, 0, 0),
            terrain_value=0x74,
        )

        code = build_override_trampoline(profile, return_address=return_address)

        self.assertLess(len(code), 0x1000)
        self.assertTrue(code.startswith(bytes.fromhex("9C 50 51 52")))
        self.assertGreaterEqual(code.count(struct.pack("<I", 0x0006DE91)), 3)
        self.assertGreaterEqual(
            code.count(bytes.fromhex("41 C7 40 08 01 00 00 00")),
            3,
        )
        self.assertIn(struct.pack("<H", 0x6171), code)
        self.assertTrue(
            code.endswith(
                DESCRIPTOR_COMPLETE_BYTES
                + bytes.fromhex("48 B8")
                + struct.pack("<Q", return_address)
                + bytes.fromhex("FF E0")
            )
        )

    def test_runtime_auxiliary_profile_rejects_non_native_enemy_key(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown native enemy"):
            RuntimeAuxiliaryOverrideProfile(
                seed=203900415,
                enemy_keys=(0xDEADBEEF,),
            )

    def test_runtime_enemy_variants_write_their_exact_native_roles(self) -> None:
        code = build_override_trampoline(
            RuntimeAuxiliaryOverrideProfile(
                seed=203900415,
                enemy_keys=(0x0000F9CA, 0x000179F7),
            ),
            return_address=0x00007FF61234567D,
        )
        self.assertIn(bytes.fromhex("41 C7 40 08 04 00 00 00"), code)
        self.assertIn(bytes.fromhex("41 C7 40 08 05 00 00 00"), code)

    def test_runtime_auxiliary_hook_uses_one_exact_rel32_instruction(self) -> None:
        patch = build_relative_jump(0x00007FF600001000, 0x00007FF600101000)

        self.assertEqual(len(patch), len(DESCRIPTOR_COMPLETE_BYTES))
        self.assertEqual(patch, bytes.fromhex("E9 FB FF 0F 00"))
        with self.assertRaisesRegex(ValueError, "rel32"):
            build_relative_jump(0x1000, 0x1_0000_1000)

    def test_local_effect_slots_roundtrip_all_raw_fields(self) -> None:
        original = bytearray(make_record(seed=101))
        for slot_index in range(7):
            struct.pack_into(
                "<6I",
                original,
                0x34 + slot_index * 0x18,
                0x10000000 + slot_index,
                0x20000000 + slot_index,
                0x30000000 + slot_index,
                0x40000000 + slot_index,
                0x50000000 + slot_index,
                0x60000000 + slot_index,
            )

        fields = read_local_effect_slots(bytes(original))
        replacement = patch_local_scroll_record(
            bytes(original),
            tuple(slot.as_edit() for slot in fields),
        )

        self.assertEqual(len(fields), 7)
        self.assertEqual(fields[0].prefix, 0x10000000)
        self.assertEqual(fields[6].effect_id, 0x20000006)
        self.assertEqual(fields[4].tail_1, 0x60000004)
        self.assertEqual(replacement, bytes(original))

    def test_unrestricted_local_effect_patch_accepts_duplicates_and_unknown_ids(self) -> None:
        original = make_record(seed=101)
        replacement = patch_local_scroll_record(
            original,
            (
                LocalEffectEdit(
                    slot_index=0,
                    effect_id=0xDEADBEEF,
                    value=0xFFFFFFFF,
                    prefix=0xCAFEBABE,
                    metadata=0xA5A5A5A5,
                    tail_0=0x12345678,
                    tail_1=0x87654321,
                ),
                LocalEffectEdit(slot_index=1, effect_id=0xDEADBEEF),
                LocalEffectEdit(slot_index=2, effect_id=0xDEADBEEF),
            ),
        )
        fields = read_local_effect_slots(replacement)

        self.assertEqual(
            tuple(fields[index].effect_id for index in range(3)),
            (0xDEADBEEF, 0xDEADBEEF, 0xDEADBEEF),
        )
        self.assertEqual(fields[0].value, 0xFFFFFFFF)
        self.assertEqual(fields[0].prefix, 0xCAFEBABE)
        self.assertEqual(fields[0].metadata, 0xA5A5A5A5)
        self.assertEqual(fields[0].tail_0, 0x12345678)
        self.assertEqual(fields[0].tail_1, 0x87654321)
        self.assertEqual(replacement[:0x34], original[:0x34])

    def test_catalog_retarget_preserves_value_role_roll_and_tails(self) -> None:
        tables = load_default_effect_generation_tables()
        definition = tables.effect(0xDAC2)
        group = tables.group_for_effect(0xDAC2)
        original = LocalEffectSlotFields(
            slot_index=3,
            effect_id=0x190A,
            value=0x12345678,
            prefix=0x11111111,
            metadata=0xBEEF_D37A,
            tail_0=0x22222222,
            tail_1=0x33333333,
        )

        retargeted = retarget_local_effect_identity(
            original,
            effect_id=definition.effect_id,
            group_key=definition.group_key,
            category_key=group.category_key,
        )

        expected_category_and_role = (0xD3 & 0xC0) | group.category_key
        self.assertEqual(retargeted.effect_id, 0xDAC2)
        self.assertEqual(retargeted.prefix, definition.group_key)
        self.assertEqual((retargeted.metadata >> 8) & 0xFF, expected_category_and_role)
        self.assertEqual(retargeted.metadata & 0xFFFF00FF, original.metadata & 0xFFFF00FF)
        self.assertEqual(retargeted.value, original.value)
        self.assertEqual(retargeted.tail_0, original.tail_0)
        self.assertEqual(retargeted.tail_1, original.tail_1)

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
            # Match the game's deleted-slot state: type zero with stale bytes.
            stale_free_slot = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            decrypted[stale_free_slot + 2] = 0xA5
            struct.pack_into("<I", decrypted, stale_free_slot + 0x20, 0xDEADBEEF)
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
            # Allocation skips every save-wide four-byte collision, including
            # conservative false positives from unrelated small scalars.
            self.assertEqual(struct.unpack_from("<I", installed, second + 0x1C)[0], 7)
            self.assertEqual(result.slot_index, 1)
            self.assertTrue((result.backup_directory / "SAVEDATA.BIN").is_file())
            self.assertTrue((result.backup_directory / "BACKUP.BIN").is_file())
            self.assertTrue((result.backup_directory / "SYSTEMSAVEDATA.BIN").is_file())
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["steam_account_id"], account)
            self.assertNotIn("save_path", report)
            rendered = json.dumps(report)
            self.assertNotIn(str(root), rendered)

    def test_install_replaces_colliding_template_inventory_key(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            template = bytearray(make_record(seed=1, account_id=account))
            struct.pack_into("<I", template, 0x1C, 0x8AC9)
            second = bytearray(make_record(seed=2, account_id=account))
            struct.pack_into("<I", second, 0x1C, 0xA0E0)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = template
            second_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            decrypted[second_offset:second_offset + SCROLL_RECORD_SIZE] = second
            save_path.write_bytes(b"ENC" + bytes(decrypted))
            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            candidate = bytearray(make_record(seed=3, account_id=account))
            struct.pack_into("<I", candidate, 0x1C, 0x8AC9)

            result = installer.install(bytes(candidate), transfer_count=0)

            installed = save_path.read_bytes()[3:]
            target_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
            self.assertEqual(result.slot_index, 2)
            self.assertEqual(
                struct.unpack_from("<I", installed, target_offset + 0x1C)[0],
                0xA0E1,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["inventory_key"], 0xA0E1)
            self.assertEqual(report["inventory_key_hex"], "0xa0e1")

    def test_install_avoids_equipment_key_collision_outside_scroll_array(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            first = bytearray(make_record(seed=1, account_id=account))
            struct.pack_into("<I", first, 0x1C, 0x8AC9)
            second = bytearray(make_record(seed=2, account_id=account))
            struct.pack_into("<I", second, 0x1C, 0xA0E0)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = first
            second_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            decrypted[second_offset:second_offset + SCROLL_RECORD_SIZE] = second
            # FB-016: the same four-byte instance key can belong to an
            # equipment record in a different, unaligned save array.
            external_key_offset = SCROLL_GROUP_OFFSET - 0x101
            external_record_offset = external_key_offset - 0x1C
            struct.pack_into("<H", decrypted, external_record_offset, 0xD782)
            struct.pack_into("<H", decrypted, external_record_offset + 0x02, 0xD782)
            struct.pack_into("<H", decrypted, external_record_offset + 0x04, 1)
            struct.pack_into("<H", decrypted, external_record_offset + 0x06, 150)
            struct.pack_into("<H", decrypted, external_record_offset + 0x08, 150)
            struct.pack_into("<I", decrypted, external_key_offset, 0xA0E1)
            save_path.write_bytes(b"ENC" + bytes(decrypted))

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            candidate = make_record(seed=3, account_id=account)
            result = installer.install(candidate, transfer_count=0)

            installed = save_path.read_bytes()[3:]
            target_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
            self.assertEqual(result.slot_index, 2)
            self.assertEqual(
                struct.unpack_from("<I", installed, target_offset + 0x1C)[0],
                0xA0E2,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["inventory_key"], 0xA0E2)

    def test_install_preserves_existing_scroll_key_colliding_with_equipment(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            first = bytearray(make_record(seed=1, account_id=account))
            struct.pack_into("<I", first, 0x1C, 0x8AC9)
            affected = bytearray(make_record(seed=2, account_id=account))
            struct.pack_into("<I", affected, 0x1C, 0xA0E1)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = first
            affected_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            decrypted[affected_offset:affected_offset + SCROLL_RECORD_SIZE] = affected
            external_key_offset = SCROLL_GROUP_OFFSET - 0x101
            external_record_offset = external_key_offset - 0x1C
            struct.pack_into("<H", decrypted, external_record_offset, 0xD782)
            struct.pack_into("<H", decrypted, external_record_offset + 0x02, 0xD782)
            struct.pack_into("<H", decrypted, external_record_offset + 0x04, 1)
            struct.pack_into("<H", decrypted, external_record_offset + 0x06, 150)
            struct.pack_into("<H", decrypted, external_record_offset + 0x08, 150)
            struct.pack_into("<I", decrypted, external_key_offset, 0xA0E1)
            save_path.write_bytes(b"ENC" + bytes(decrypted))

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            result = installer.install(
                make_record(seed=3, account_id=account),
                transfer_count=0,
            )

            installed = save_path.read_bytes()[3:]
            inserted_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
            self.assertEqual(
                struct.unpack_from("<I", installed, affected_offset + 0x1C)[0],
                0xA0E1,
            )
            self.assertEqual(
                struct.unpack_from("<I", installed, inserted_offset + 0x1C)[0],
                0xA0E2,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["inventory_key_repairs"], [])

    def test_install_preserves_existing_duplicate_inventory_keys(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            for slot_index, seed in enumerate((1, 2)):
                record = bytearray(make_record(seed=seed, account_id=account))
                struct.pack_into("<I", record, 0x1C, 0x8AC9)
                offset = SCROLL_GROUP_OFFSET + slot_index * SCROLL_RECORD_SIZE
                decrypted[offset:offset + SCROLL_RECORD_SIZE] = record
            save_path.write_bytes(b"ENC" + bytes(decrypted))
            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            candidate = bytearray(make_record(seed=3, account_id=account))
            struct.pack_into("<I", candidate, 0x1C, 0x8AC9)

            result = installer.install(bytes(candidate), transfer_count=0)

            installed = save_path.read_bytes()[3:]
            keys = [
                struct.unpack_from(
                    "<I",
                    installed,
                    SCROLL_GROUP_OFFSET
                    + slot_index * SCROLL_RECORD_SIZE
                    + 0x1C,
                )[0]
                for slot_index in range(3)
            ]
            self.assertEqual(keys, [0x8AC9, 0x8AC9, 0x8ACA])
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["inventory_key_repairs"], [])

    def test_install_skips_equipment_generation_serial_collision(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            template = bytearray(make_record(seed=1, account_id=account))
            struct.pack_into("<I", template, 0x28, 0x00244071)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = template
            external_serial_offset = SCROLL_GROUP_OFFSET - 0x101
            external_record_offset = external_serial_offset - 0x28
            struct.pack_into("<H", decrypted, external_record_offset, 0xBA2C)
            struct.pack_into("<H", decrypted, external_record_offset + 0x02, 0xBA2C)
            struct.pack_into("<H", decrypted, external_record_offset + 0x04, 1)
            struct.pack_into("<H", decrypted, external_record_offset + 0x06, 145)
            struct.pack_into("<H", decrypted, external_record_offset + 0x08, 145)
            struct.pack_into("<I", decrypted, external_serial_offset, 0x00244072)
            save_path.write_bytes(b"ENC" + bytes(decrypted))

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            result = installer.install(
                make_record(seed=2, account_id=account),
                transfer_count=0,
            )

            installed = save_path.read_bytes()[3:]
            inserted_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            self.assertEqual(
                struct.unpack_from("<I", installed, inserted_offset + 0x28)[0],
                0x00244073,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(report["generation_serial"], 0x00244073)
            self.assertEqual(report["generation_serial_repairs"], [])

    def test_install_repairs_existing_equipment_generation_serial_collision(self) -> None:
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
            save_directory.mkdir(parents=True)
            save_path = save_directory / "SAVEDATA.BIN"
            decrypted = bytearray(USER_SAVE_SIZE)
            decrypted[:6] = b"RNNUSR"
            first = bytearray(make_record(seed=1, account_id=account))
            struct.pack_into("<I", first, 0x28, 0x00244071)
            affected = bytearray(make_record(seed=180443387, account_id=account))
            struct.pack_into("<I", affected, 0x28, 0x00244072)
            decrypted[
                SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            ] = first
            affected_offset = SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
            decrypted[affected_offset:affected_offset + SCROLL_RECORD_SIZE] = affected
            external_serial_offset = SCROLL_GROUP_OFFSET - 0x101
            external_record_offset = external_serial_offset - 0x28
            struct.pack_into("<H", decrypted, external_record_offset, 0xBA2C)
            struct.pack_into("<H", decrypted, external_record_offset + 0x02, 0xBA2C)
            struct.pack_into("<H", decrypted, external_record_offset + 0x04, 1)
            struct.pack_into("<H", decrypted, external_record_offset + 0x06, 145)
            struct.pack_into("<H", decrypted, external_record_offset + 0x08, 145)
            struct.pack_into("<I", decrypted, external_serial_offset, 0x00244072)
            save_path.write_bytes(b"ENC" + bytes(decrypted))

            installer = SaveInstaller(
                save_path=save_path,
                crypto=FakeCrypto(),
                state_root=root / "state",
            )
            result = installer.install(
                make_record(seed=3, account_id=account),
                transfer_count=0,
            )

            installed = save_path.read_bytes()[3:]
            inserted_offset = SCROLL_GROUP_OFFSET + 2 * SCROLL_RECORD_SIZE
            self.assertEqual(
                struct.unpack_from("<I", installed, affected_offset + 0x28)[0],
                0x00244073,
            )
            self.assertEqual(
                struct.unpack_from("<I", installed, inserted_offset + 0x28)[0],
                0x00244074,
            )
            report = json.loads(result.report_path.read_text(encoding="utf-8"))
            self.assertEqual(
                report["generation_serial_repairs"],
                [
                    {
                        "slot_index": 1,
                        "old_generation_serial": 0x00244072,
                        "new_generation_serial": 0x00244073,
                        "reason": "non_scroll_item_generation_serial_collision",
                    }
                ],
            )

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
