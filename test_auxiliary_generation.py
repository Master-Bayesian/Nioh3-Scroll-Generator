from __future__ import annotations

import unittest
import struct
import json
import hashlib
from pathlib import Path
import shutil
import tempfile

from nioh3_scroll_editor.auxiliary_generation import (
    AuxiliaryGenerationError,
    AuxiliarySearchCriteria,
    AuxiliaryGenerationTables,
    DEFAULT_AUXILIARY_RESOURCE_ROOT,
    derive_auxiliary_descriptor_seed,
    derive_auxiliary_mode_seed,
    derive_special_rule_seed,
    derive_terrain_seed,
    describe_special_rule,
    generate_auxiliary_mode,
    generate_auxiliary_descriptor_flags,
    generate_class0_enemies,
    generate_class1_enemies,
    generate_class2_enemies,
    generate_complete_auxiliary,
    generate_enemy_match_masks_batch,
    generate_matching_auxiliary,
    generate_special_rules,
    generate_terrain,
    generate_terrain_row_indices_batch,
    load_default_auxiliary_generation_tables,
    load_enemy_parameter_gate_capture,
)
from nioh3_scroll_editor.auxiliary_feasibility import (
    SpecialRuleKeyRequirement,
    analyze_special_rule_feasibility,
)
from nioh3_scroll_editor.r4_table_bundle import FixedStrideTable


def make_table(name: str, row_size: int, rows: list[bytes]) -> FixedStrideTable:
    store = bytearray(8)
    struct.pack_into("<I", store, 4, len(rows))
    store.extend(b"".join(rows))
    return FixedStrideTable(name, row_size, len(rows), bytes(store))


def make_terrain_row(
    value: int,
    *,
    flags_2e: int = 0,
    crucible_marker: int = 0,
) -> bytes:
    row = bytearray(0x34)
    struct.pack_into("<H", row, 0x2C, crucible_marker)
    row[0x2E] = flags_2e
    row[0x30] = value
    return bytes(row)


def make_enemy_row(
    lookup_key: int,
    cost: float,
    role: int,
    *,
    playthrough_mask: int = 0x1F,
    scratch_rule_key: int = 0,
) -> bytes:
    row = bytearray(0x1C)
    struct.pack_into("<I", row, 0x04, lookup_key)
    struct.pack_into("<f", row, 0x0C, cost)
    struct.pack_into("<H", row, 0x12, scratch_rule_key)
    row[0x16] = playthrough_mask
    row[0x1A] = role
    return bytes(row)


def make_special_context_row(mode: int, budgets: list[float]) -> bytes:
    row = bytearray(0x30)
    for index, budget in enumerate((budgets + [0.0] * 5)[:5]):
        struct.pack_into("<f", row, 4 + index * 4, budget)
    row[0x28] = mode
    row[0x29] = 1
    return bytes(row)


class AuxiliaryModeTests(unittest.TestCase):
    def test_native_vector_seed_203900415(self) -> None:
        result = generate_auxiliary_mode(203900415)
        self.assertEqual(result.value, 125)
        self.assertEqual(result.scoped_seed, 0x01DA6C7F)
        self.assertEqual(result.branch_class, 1)
        self.assertEqual(result.selected_row_index, 3)
        self.assertEqual(result.random_draws, 3)

    def test_native_vector_seed_6096970(self) -> None:
        result = generate_auxiliary_mode(6096970)
        self.assertEqual(result.value, 76)
        self.assertEqual(result.scoped_seed, 0x0209C0D4)
        self.assertEqual(result.branch_class, 1)
        self.assertEqual(result.selected_row_index, 2)
        self.assertEqual(result.random_draws, 3)

    def test_threshold_branch_uses_class_two_rows(self) -> None:
        result = generate_auxiliary_mode(0)
        self.assertEqual(result.branch_class, 2)
        self.assertEqual(result.value, 72)
        self.assertEqual(result.selected_row_index, 4)
        self.assertEqual(result.random_draws, 2)

    def test_seed_derivation_masks_to_uint32(self) -> None:
        self.assertEqual(
            derive_auxiliary_mode_seed(0x1_0BEA0C3F),
            derive_auxiliary_mode_seed(0x0BEA0C3F),
        )


class TerrainTests(unittest.TestCase):
    def test_seed_derivation_swaps_low_fourteen_bit_halves(self) -> None:
        seed = 0x0ABC1234
        expected = ((seed >> 14) & 0x3FFF) | ((seed & 0x3FFF) << 14)
        self.assertEqual(derive_terrain_seed(seed), expected)

    def test_filtered_path_uses_row_value_and_skips_flagged_rows(self) -> None:
        terrain = make_table(
            "auxiliary_terrain",
            0x34,
            [
                make_terrain_row(0xAA),
                make_terrain_row(0xBB, flags_2e=0x02),
            ],
        )
        result = generate_terrain(
            0,
            0x57,
            tables=AuxiliaryGenerationTables(terrain, (0x11, 0x22)),
        )
        self.assertTrue(result.used_filtered_pool)
        self.assertEqual(result.eligible_row_indices, (0,))
        self.assertEqual(result.selected_row_index, 0)
        self.assertEqual(result.value, 0xAA)

    def test_unmatched_context_uses_native_hash_key(self) -> None:
        terrain = make_table(
            "auxiliary_terrain",
            0x34,
            [make_terrain_row(0xAA), make_terrain_row(0xBB)],
        )
        result = generate_terrain(
            0,
            0xFE,
            tables=AuxiliaryGenerationTables(terrain, (0x11, 0x22)),
        )
        self.assertFalse(result.used_filtered_pool)
        self.assertEqual(result.selected_row_index, 0)
        self.assertEqual(result.value, 0x11)

    def test_key_count_mismatch_fails_closed(self) -> None:
        terrain = make_table(
            "auxiliary_terrain", 0x34, [make_terrain_row(0xAA)]
        )
        with self.assertRaises(AuxiliaryGenerationError):
            generate_terrain(
                0,
                0xFE,
                tables=AuxiliaryGenerationTables(terrain, ()),
            )

    def test_ui_consumer_effect_keys_are_derived_from_enum_and_row(self) -> None:
        terrain = make_table(
            "auxiliary_terrain",
            0x34,
            [make_terrain_row(0x2D, crucible_marker=1)],
        )
        result = generate_terrain(
            0,
            0x57,
            tables=AuxiliaryGenerationTables(terrain, (0x2D,)),
        )
        self.assertEqual(result.value, 0x2D)
        self.assertEqual(result.display_effect_keys, (0x0024, 0x0039))


class AuxiliaryDescriptorFlagsTests(unittest.TestCase):
    def test_seed_203900415_native_descriptor_vector(self) -> None:
        result = generate_auxiliary_descriptor_flags(203900415, 0x7D)
        self.assertEqual(result.selector, 0)
        self.assertEqual(result.flags, (False, False, False))
        self.assertEqual(result.scoped_seed, 0x0394D8FF)
        self.assertEqual(result.random_draws, 4)

    def test_seed_6096970_native_descriptor_vector(self) -> None:
        result = generate_auxiliary_descriptor_flags(6096970, 0x4C)
        self.assertEqual(result.selector, 0)
        self.assertEqual(result.flags, (False, False, False))
        self.assertEqual(result.scoped_seed, 0x04138129)
        self.assertEqual(result.random_draws, 4)

    def test_seed_derivation_masks_to_uint32(self) -> None:
        self.assertEqual(
            derive_auxiliary_descriptor_seed(0x1_0BEA0C3F),
            derive_auxiliary_descriptor_seed(0x0BEA0C3F),
        )

    def test_uncommon_selector_path_uses_unique_native_table_value(self) -> None:
        selected = None
        for seed in range(10_000):
            mode = generate_auxiliary_mode(seed)
            descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
            if descriptor.selector:
                selected = descriptor
                break
        self.assertIsNotNone(selected)
        assert selected is not None
        self.assertEqual(selected.selector, 1)


class SpecialRuleTests(unittest.TestCase):
    def test_structural_preflight_accepts_reported_rare_rule_pair(self) -> None:
        report = analyze_special_rule_feasibility(
            (
                SpecialRuleKeyRequirement("Cursed Cavalcade", frozenset((0x2FEA,))),
                SpecialRuleKeyRequirement("Tsukuyomi drop", frozenset((0x7EF1,))),
            ),
            playthrough=3,
        )
        self.assertTrue(report.possible)
        self.assertIsNotNone(report.witness_budget)
        self.assertTrue({0x2FEA, 0x7EF1}.issubset(report.witness_keys))

    def test_structural_preflight_rejects_native_conflict_group(self) -> None:
        report = analyze_special_rule_feasibility(
            (
                SpecialRuleKeyRequirement("left", frozenset((0x6171,))),
                SpecialRuleKeyRequirement("right", frozenset((0xED18,))),
            ),
            playthrough=3,
        )
        self.assertFalse(report.possible)
        self.assertEqual(report.failure_code, "conflict")
        self.assertEqual(
            report.universally_conflicting_pairs,
            (("left", "right"),),
        )

    def test_structural_preflight_rejects_zero_weight_for_playthrough(self) -> None:
        report = analyze_special_rule_feasibility(
            (SpecialRuleKeyRequirement("R3-only rule", frozenset((0x2FEA,))),),
            playthrough=5,
        )
        self.assertFalse(report.possible)
        self.assertEqual(report.failure_code, "unavailable")

    def test_structural_preflight_accepts_native_generated_rule_sets(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        observed: set[tuple[int, ...]] = set()
        for seed in range(256):
            keys = tuple(
                key
                for key in generate_special_rules(
                    seed,
                    3,
                    tables=tables,
                ).keys
                if key
            )
            if not keys or keys in observed:
                continue
            observed.add(keys)
            report = analyze_special_rule_feasibility(
                tuple(
                    SpecialRuleKeyRequirement(
                        f"0x{key:04X}",
                        frozenset((key,)),
                    )
                    for key in keys
                ),
                playthrough=3,
                tables=tables,
            )
            self.assertTrue(report.possible, keys)

    def test_seed_183696634_native_rule_vector(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        result = generate_special_rules(
            183696634,
            3,
            (0xFFFF, 0xFFFF, 0xFFFF),
            tables=tables,
        )
        self.assertEqual(result.scoped_seed, 183696634 & 0x0FFFFFFF)
        self.assertEqual(result.target_budget, 1)
        self.assertEqual(result.keys, (0x359A, 0x81DE, 0x0000))
        self.assertEqual(
            [
                (entry.key, entry.display_value, entry.display_unit)
                for entry in result.entries
            ],
            [
                (0x359A, 9.0, "percent"),
                (0x81DE, 65.0, "percent"),
            ],
        )

    def test_seed_6096970_native_rule_vector(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        result = generate_special_rules(
            6096970,
            3,
            (0xFFFF, 0xFFFF, 0xFFFF),
            tables=tables,
        )
        self.assertEqual(result.target_budget, 1)
        self.assertEqual(result.keys, (0xD324, 0x0000, 0x0000))

    def test_special_rule_seed_uses_low_twenty_eight_bits(self) -> None:
        self.assertEqual(
            derive_special_rule_seed(0xF1234567),
            derive_special_rule_seed(0x01234567),
        )

    def test_ui_value_families_match_native_consumer(self) -> None:
        duration = describe_special_rule(0x0E2E)
        self.assertEqual(duration.raw_value, 7200.0)
        self.assertEqual(duration.display_value, 120.0)
        self.assertEqual(duration.display_unit, "seconds")
        self.assertEqual(duration.qualifier_kind, "item")
        self.assertEqual(duration.qualifier_key, 0x729D)

        effect_qualified = describe_special_rule(0xCF88)
        self.assertEqual(effect_qualified.display_value, 10.0)
        self.assertEqual(effect_qualified.display_unit, "percent")
        self.assertEqual(effect_qualified.qualifier_kind, "effect")
        self.assertEqual(effect_qualified.qualifier_key, 0x23E5)

        enemy_qualified = describe_special_rule(0x2589)
        self.assertIsNone(enemy_qualified.display_value)
        self.assertIsNone(enemy_qualified.display_unit)
        self.assertEqual(enemy_qualified.qualifier_kind, "enemy")

    def test_grade_family_matches_native_sorted_index(self) -> None:
        self.assertEqual(describe_special_rule(0x2BFD).display_grade, "C")
        self.assertEqual(describe_special_rule(0x8318).display_grade, "B")
        self.assertEqual(describe_special_rule(0x62C0).display_grade, "A")

    def test_every_native_rule_row_has_fail_closed_value_semantics(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        entries = [
            describe_special_rule(key, tables=tables)
            for key in tables.special_rule_keys_by_row
        ]
        self.assertEqual(len(entries), 301)
        self.assertEqual(
            sum(entry.display_unit == "percent" for entry in entries),
            181,
        )
        self.assertEqual(
            sum(entry.display_unit == "seconds" for entry in entries),
            102,
        )
        self.assertEqual(
            sum(entry.display_unit == "grade" for entry in entries),
            3,
        )
        self.assertEqual(
            sum(entry.display_unit is None for entry in entries),
            15,
        )


class Class1EnemyTests(unittest.TestCase):
    def assert_native_vector(
        self,
        seed: int,
        expected_groups: list[list[int]],
    ) -> None:
        tables = load_default_auxiliary_generation_tables()
        mode = generate_auxiliary_mode(seed)
        terrain = generate_terrain(seed, mode.value, tables=tables)
        descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
        result = generate_class1_enemies(
            seed,
            3,
            mode.value,
            terrain.selected_row_index,
            descriptor_selector=descriptor.selector,
            descriptor_flags=descriptor.flags,
            caller_option=0,
            tables=tables,
        )
        self.assertEqual(
            [
                [entry.lookup_key for entry in group.entries]
                for group in result.groups
            ],
            expected_groups,
        )

    def test_seed_203900415_native_enemy_vector(self) -> None:
        self.assert_native_vector(
            203900415,
            [[0x1AA93], [0x01AB5], [0x594DC, 0x00F02], [0x71ED1]],
        )

    def test_seed_6096970_native_enemy_vector(self) -> None:
        self.assert_native_vector(
            6096970,
            [[0x6CAA5], [0xBC496], [0xC5F3E]],
        )

    def test_budget_groups_are_generated_high_to_low_then_reversed(self) -> None:
        terrain = make_table(
            "auxiliary_terrain", 0x34, [make_terrain_row(0xAA)]
        )
        candidates = make_table(
            "auxiliary_enemy_candidate",
            0x1C,
            [
                make_enemy_row(0x100, 3.0, 1, scratch_rule_key=0x1111),
                make_enemy_row(0x200, 1.0, 0, scratch_rule_key=0x2222),
                make_enemy_row(0x500, 4.0, 5, scratch_rule_key=0x5555),
            ],
        )
        contexts = make_table(
            "special_context",
            0x30,
            [make_special_context_row(0xAA, [3.0, 4.0])],
        )
        tables = AuxiliaryGenerationTables(
            terrain,
            (0xAA,),
            enemy_candidates=candidates,
            special_context=contexts,
        )
        result = generate_class1_enemies(
            123,
            3,
            0xAA,
            0,
            enemy_param_type_by_key={0x100: 1, 0x200: 1, 0x500: 1},
            tables=tables,
        )
        self.assertEqual(
            [[entry.lookup_key for entry in group.entries] for group in result.groups],
            [[0x100], [0x500]],
        )
        self.assertEqual(result.random_draws, 4)

    def test_missing_complete_enemy_parameter_gate_fails_closed(self) -> None:
        terrain = make_table(
            "auxiliary_terrain", 0x34, [make_terrain_row(0xAA)]
        )
        candidates = make_table(
            "auxiliary_enemy_candidate", 0x1C, [make_enemy_row(0x100, 3.0, 1)]
        )
        contexts = make_table(
            "special_context", 0x30, [make_special_context_row(0xAA, [3.0])]
        )
        with self.assertRaises(AuxiliaryGenerationError):
            generate_class1_enemies(
                123,
                3,
                0xAA,
                0,
                enemy_param_type_by_key={},
                tables=AuxiliaryGenerationTables(
                    terrain,
                    (0xAA,),
                    enemy_candidates=candidates,
                    special_context=contexts,
                ),
            )


class Class0EnemyTests(unittest.TestCase):
    NATIVE_VECTORS = (
        (1664, [[0xA3F52], [0x4B03B]]),
        (1665, [[0xBD9D5], [0x03921]]),
        (1666, [[0x41DB6], [0x71ED1]]),
        (1673, [[0x17AD0], [0xD35E1]]),
        (1667, [[0x75E4F], [0x782F6], [0x4B03B]]),
        (1668, [[0x30EDD], [0x41A50], [0xB0DAA]]),
        (1676, [[0xA3F52], [0x64B4D], [0x202A7]]),
        (1681, [[0x782F6], [0x431DA], [0x41DB6]]),
    )

    def test_native_vectors_cover_both_class0_modes(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        seen_modes: set[int] = set()
        for seed, expected_groups in self.NATIVE_VECTORS:
            with self.subTest(seed=seed):
                mode = generate_auxiliary_mode(seed)
                terrain = generate_terrain(seed, mode.value, tables=tables)
                descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
                seen_modes.add(mode.value)
                result = generate_class0_enemies(
                    seed,
                    3,
                    mode.value,
                    terrain.selected_row_index,
                    descriptor_selector=descriptor.selector,
                    descriptor_flags=descriptor.flags,
                    caller_option=0,
                    tables=tables,
                )
                self.assertEqual(
                    [
                        [entry.lookup_key for entry in group.entries]
                        for group in result.groups
                    ],
                    expected_groups,
                )
                self.assertEqual(result.random_draws, len(expected_groups) * 2 - 1)
        self.assertEqual(seen_modes, {0x57, 0x6F})

    def test_rejects_class2_mode(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        seed = 1
        mode = generate_auxiliary_mode(seed)
        terrain = generate_terrain(seed, mode.value, tables=tables)
        descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
        with self.assertRaises(AuxiliaryGenerationError):
            generate_class0_enemies(
                seed,
                3,
                mode.value,
                terrain.selected_row_index,
                descriptor_selector=descriptor.selector,
                descriptor_flags=descriptor.flags,
                tables=tables,
            )


class Class2EnemyTests(unittest.TestCase):
    NATIVE_VECTORS = (
        (1, [[0xF3566], [0x3EC8A], [0xD5AE8, 0x8BC99], [0x6F167, 0xE55D2], [0x1AA93, 0x46F7D]]),
        (2, [[0xD0779], [0xBC496], [0x3D18B, 0x56F1F], [0x6314C, 0xF0DC3], [0x2F535, 0xDEE24, 0xDEE24]]),
        (9, [[0xF3EB0], [0xD5AE8], [0xF3EB0, 0xDF255], [0xC3FF5, 0x0CB88], [0xBCC5B, 0x5040C, 0x5040C, 0x5040C]]),
        (10, [[0x2D571], [0xC18A9, 0xECDB4], [0x69D5B, 0x2CD16], [0xBCC5B, 0x99943], [0x5DB6E, 0x77FDE, 0x77FDE, 0x77FDE, 0x77FDE]]),
        (3, [[0x26C12], [0x061EB], [0xBC496, 0x232D5], [0x26C12, 0x232D5]]),
        (4, [[0xC0618], [0xBC496], [0xBADDA, 0x04388], [0xBCC5B, 0xC18A9]]),
        (5, [[0x643F5], [0x82783, 0x892BB], [0x5A056, 0xE2E6D], [0xD2B55, 0x99943]]),
        (12, [[0x816E0], [0x82783, 0xE55D2], [0x4CF03, 0xED6B1], [0x545A1, 0xE1ABA, 0xE1ABA]]),
        (6, [[0x5A056], [0x061EB], [0x3B86E, 0xE3769]]),
        (7, [[0x8BE37], [0xC18A9, 0x2CD16], [0x4A66A, 0x0CB88]]),
        (8, [[0x4A66A], [0xC18A9, 0xE6F42], [0x8BE37, 0xCD93B]]),
        (15, [[0x91574], [0x06CD3], [0xC18A9, 0x810E4]]),
    )

    def test_native_vectors_cover_all_three_class2_modes(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        seen_modes: set[int] = set()
        for seed, expected_groups in self.NATIVE_VECTORS:
            with self.subTest(seed=seed):
                mode = generate_auxiliary_mode(seed)
                terrain = generate_terrain(seed, mode.value, tables=tables)
                descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
                seen_modes.add(mode.value)
                result = generate_class2_enemies(
                    seed,
                    3,
                    mode.value,
                    terrain.selected_row_index,
                    descriptor_selector=descriptor.selector,
                    descriptor_flags=descriptor.flags,
                    caller_option=0,
                    tables=tables,
                )
                self.assertEqual(
                    [
                        [entry.lookup_key for entry in group.entries]
                        for group in result.groups
                    ],
                    expected_groups,
                )
                self.assertEqual(result.random_draws, len(expected_groups) * 3)
        self.assertEqual(seen_modes, {0x48, 0x62, 0x8E})

    def test_rejects_class1_mode(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        seed = 203900415
        mode = generate_auxiliary_mode(seed)
        terrain = generate_terrain(seed, mode.value, tables=tables)
        descriptor = generate_auxiliary_descriptor_flags(seed, mode.value)
        with self.assertRaises(AuxiliaryGenerationError):
            generate_class2_enemies(
                seed,
                3,
                mode.value,
                terrain.selected_row_index,
                descriptor_selector=descriptor.selector,
                descriptor_flags=descriptor.flags,
                tables=tables,
            )


class CompleteAuxiliaryTests(unittest.TestCase):
    def test_native_enemy_constraint_masks_match_exact_python_replay(self) -> None:
        seeds = tuple(range(1, 513))
        terrain_rows = generate_terrain_row_indices_batch(seeds)
        criteria = AuxiliarySearchCriteria(
            required_enemy_lookup_keys=frozenset((0xD35E1, 0x40A3B)),
            required_enemy_lookup_key_groups=(
                frozenset((0xF3566, 0xD0779, 0xC0618)),
                frozenset((0x3EC8A, 0xBC496, 0x82783, 0x782F6)),
            ),
        )
        condition_groups = (
            frozenset((0x40A3B,)),
            frozenset((0xD35E1,)),
            *criteria.required_enemy_lookup_key_groups,
        )

        for playthrough in (1, 2, 3, 4):
            with self.subTest(playthrough=playthrough):
                actual_masks = generate_enemy_match_masks_batch(
                    seeds,
                    terrain_rows,
                    playthrough,
                    criteria=criteria,
                )
                expected_masks = []
                for seed in seeds:
                    generated = generate_complete_auxiliary(seed, playthrough)
                    actual_keys = frozenset(
                        entry.lookup_key
                        for group in generated.enemies.groups
                        for entry in group.entries
                    )
                    mask = 0
                    for index, group in enumerate(condition_groups):
                        if group.intersection(actual_keys):
                            mask |= 1 << index
                    expected_masks.append(mask)
                self.assertEqual(actual_masks, tuple(expected_masks))

    def test_class0_enemy_scratch_key_changes_native_rule_result(self) -> None:
        result = generate_complete_auxiliary(1665, 3)
        self.assertEqual(result.mode.branch_class, 0)
        self.assertEqual(result.mode.value, 0x57)
        self.assertEqual(result.terrain.value, 0xEE)
        self.assertEqual(
            [
                [entry.lookup_key for entry in group.entries]
                for group in result.enemies.groups
            ],
            [[0xBD9D5], [0x03921]],
        )
        self.assertEqual(result.special_rules.keys, (0xA4FA, 0x48C6, 0x0000))

    def test_class2_complete_native_vector(self) -> None:
        result = generate_complete_auxiliary(1, 3)
        self.assertEqual(result.mode.branch_class, 2)
        self.assertEqual(result.mode.value, 0x62)
        self.assertEqual(result.terrain.value, 0x79)
        self.assertEqual(result.descriptor.flags, (True, False, False))
        self.assertEqual(
            [
                [entry.lookup_key for entry in group.entries]
                for group in result.enemies.groups
            ],
            [
                [0xF3566],
                [0x3EC8A],
                [0xD5AE8, 0x8BC99],
                [0x6F167, 0xE55D2],
                [0x1AA93, 0x46F7D],
            ],
        )
        self.assertEqual(result.special_rules.keys, (0x37DF, 0xF801, 0x2DCF))

    def test_auxiliary_constraints_match_unordered_native_outputs(self) -> None:
        result = generate_complete_auxiliary(183696634, 3)
        self.assertEqual(result.terrain.display_effect_keys, (0x0024,))
        rule_keys = frozenset(key for key in result.special_rules.keys if key)
        enemy_keys = tuple(
            entry.lookup_key
            for group in result.enemies.groups
            for entry in group.entries
        )
        criteria = AuxiliarySearchCriteria(
            required_terrain_effect_keys=frozenset((0x0024,)),
            required_special_rule_keys=rule_keys,
            required_enemy_lookup_keys=frozenset((enemy_keys[0], enemy_keys[-1])),
        )
        self.assertTrue(criteria.matches(result))
        self.assertFalse(
            AuxiliarySearchCriteria(
                required_terrain_effect_keys=frozenset((0x0039,))
            ).matches(result)
        )
        self.assertFalse(
            AuxiliarySearchCriteria(
                required_special_rule_keys=frozenset((0xFFFF,))
            ).matches(result)
        )
        self.assertFalse(
            AuxiliarySearchCriteria(
                required_enemy_lookup_keys=frozenset((0x12345678,))
            ).matches(result)
        )

    def test_matching_generator_returns_identical_complete_result_or_none(self) -> None:
        expected = generate_complete_auxiliary(183696634, 3)
        matching = generate_matching_auxiliary(
            183696634,
            3,
            criteria=AuxiliarySearchCriteria(
                required_terrain_effect_keys=frozenset((0x0024,)),
            ),
        )
        rejected = generate_matching_auxiliary(
            183696634,
            3,
            criteria=AuxiliarySearchCriteria(
                required_terrain_effect_keys=frozenset((0x0039,)),
            ),
        )

        self.assertEqual(matching, expected)
        self.assertIsNone(rejected)


class EnemyParameterGateCaptureTests(unittest.TestCase):
    def test_loads_u32_hash_keys_and_row_gate_type(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            rows = bytearray(8 + 0x398)
            struct.pack_into("<I", rows, 4, 1)
            struct.pack_into("<I", rows, 8 + 0x80, 3)
            context = bytearray(0x20)
            struct.pack_into("<I", context, 4, 0xFFFFFFFF)
            entries = struct.pack("<II", 0x12345678, 0)

            def write(name: str, data: bytes | bytearray) -> dict[str, object]:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                payload = bytes(data)
                path.write_bytes(payload)
                return {
                    "filename": name,
                    "size": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest().upper(),
                }

            manifest = {
                "schema": "nioh3-enemy-parameter-gate-capture/v1",
                "tables": [
                    {
                        "name": "enemy_parameter",
                        "row_size": 0x398,
                        "row_count": 1,
                        "rows_blob": write("rows.bin", rows),
                        "hash": {
                            "available": True,
                            "context_blob": write("context.bin", context),
                            "entries_blob": write("entries.bin", entries),
                        },
                    }
                ],
            }
            (root / "manifest.json").write_text(
                json.dumps(manifest), encoding="utf-8"
            )
            self.assertEqual(
                load_enemy_parameter_gate_capture(root),
                {0x12345678: 3},
            )


class AuxiliaryResourceTests(unittest.TestCase):
    def test_bundled_resource_contains_complete_enemy_inputs(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        self.assertIsNotNone(tables.enemy_candidates)
        self.assertIsNotNone(tables.special_context)
        self.assertEqual(len(tables.enemy_param_type_by_key), 1022)

    def test_bundled_resource_reproduces_native_terrain_vectors(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        for seed, expected in ((203900415, 0x74), (6096970, 0x8E)):
            mode = generate_auxiliary_mode(seed)
            terrain = generate_terrain(seed, mode.value, tables=tables)
            self.assertEqual(terrain.value, expected)

    def test_corrupt_resource_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            copied = Path(temporary) / "resource"
            shutil.copytree(DEFAULT_AUXILIARY_RESOURCE_ROOT, copied)
            manifest = json.loads(
                (copied / "manifest.json").read_text(encoding="utf-8")
            )
            terrain_path = (
                copied
                / manifest["tables"]["auxiliary_terrain"]["file"]["filename"]
            )
            data = bytearray(terrain_path.read_bytes())
            data[-1] ^= 0xFF
            terrain_path.write_bytes(data)
            with self.assertRaises(AuxiliaryGenerationError):
                AuxiliaryGenerationTables.from_resource(copied, verify=True)


if __name__ == "__main__":
    unittest.main()
