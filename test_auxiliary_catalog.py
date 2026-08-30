from __future__ import annotations

import unittest
import struct

from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.auxiliary_generation import (
    legal_special_rule_keys,
    load_default_auxiliary_generation_tables,
)
from nioh3_scroll_editor.grace_map import load_grace_output_map


class AuxiliaryNameCatalogTests(unittest.TestCase):
    def test_bundled_chinese_catalog_resolves_stable_keys(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        self.assertEqual(catalog.terrain_name(55), "\u5730\u72f1")
        self.assertEqual(catalog.terrain_effect_name(0x0024), "\u5730\u72f1")
        self.assertEqual(catalog.special_rule_name(0x359A), "\u8d4b\u4e88\u5438\u53d6\u4f53\u529b")
        self.assertEqual(
            catalog.special_rule_name(0x5132),
            "\u4e00\u96be\u6a2a\u884c\uff08\u8db3\u90e8\u9632\u5177\uff09",
        )
        self.assertEqual(catalog.enemy_name(0x000F3566), "\u72f1\u5352\u9b3c")

    def test_bundled_japanese_catalog_resolves_stable_keys(self) -> None:
        catalog = load_auxiliary_name_catalog("ja-JP")
        self.assertEqual(catalog.terrain_name(55), "\u5730\u7344")
        self.assertEqual(catalog.terrain_effect_name(0x0024), "\u5730\u7344")
        self.assertEqual(catalog.special_rule_name(0x359A), "\u4f53\u529b\u5438\u53ce\u4ed8\u4e0e")
        self.assertEqual(
            catalog.special_rule_name(0x5132),
            "\u4e00\u96e3\u6a2a\u884c\uff08\u8db3\u9632\u5177\uff09",
        )
        self.assertEqual(catalog.enemy_name(0x000F3566), "\u7344\u5352\u9b3c")

    def test_bundled_english_catalog_resolves_stable_keys(self) -> None:
        catalog = load_auxiliary_name_catalog("en-US")
        self.assertEqual(catalog.terrain_name(55), "The Crucible")
        self.assertEqual(catalog.terrain_effect_name(0x0024), "The Crucible")
        self.assertEqual(catalog.special_rule_name(0x359A), "Life Drain Enhancement")
        self.assertEqual(
            catalog.special_rule_name(0x5132),
            "Cursed Cavalcade (Foot Guards)",
        )
        self.assertEqual(catalog.enemy_name(0x000F3566), "Jailer Oni")

    def test_unknown_values_remain_visible_as_hex(self) -> None:
        catalog = load_auxiliary_name_catalog("ja-JP")
        self.assertEqual(catalog.special_rule_name(0x1234), "Unknown rule 0x1234")
        self.assertEqual(catalog.enemy_name(0x12345678), "Unknown enemy 0x12345678")

    def test_missing_locale_falls_back_to_japanese(self) -> None:
        catalog = load_auxiliary_name_catalog("missing-locale")
        self.assertEqual(catalog.locale, "ja-JP")

    def test_all_bundled_locales_cover_every_generator_enemy_key(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        self.assertIsNotNone(tables.enemy_candidates)
        candidate_keys = {
            int.from_bytes(row[4:8], "little")
            for row in tables.enemy_candidates.rows()
        }
        self.assertEqual(len(candidate_keys), 487)

        for locale in ("zh-CN", "ja-JP", "en-US"):
            with self.subTest(locale=locale):
                catalog = load_auxiliary_name_catalog(locale)
                self.assertEqual(len(catalog.terrain), 238)
                self.assertEqual(len(catalog.special_rules), 301)
                self.assertEqual(len(catalog.enemies), 960)
                missing = {
                    key
                    for key in candidate_keys
                    if catalog.enemy_name(key).startswith("Unknown enemy ")
                }
                self.assertEqual(missing, set())

    def test_enemy_key_groups_preserve_all_same_name_variants(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        groups = catalog.enemy_key_groups()
        self.assertEqual(groups["一目连"], catalog.enemy_keys_for_name("一目连"))
        self.assertGreater(len(groups["一目连"]), 1)

    def test_special_rule_key_groups_preserve_same_displayed_meaning(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        groups = catalog.special_rule_key_groups()
        self.assertIn("一难横行（足部防具）", groups)
        self.assertGreater(len(groups["一难横行（足部防具）"]), 1)

    def test_priority_drop_rule_resolves_native_grace_name(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        self.assertEqual(
            catalog.special_rule_name(0xCF88),
            "优先掉落率上升（毘沙门天的恩宠）",
        )

    def test_auto_activation_rule_resolves_known_item_name(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        self.assertEqual(catalog.special_rule_name(0xCED9), "自动发动（粹然符）")

    def test_parameterized_rule_names_never_collapse_to_empty_parentheses(self) -> None:
        catalog = load_auxiliary_name_catalog("zh-CN")
        names = [
            catalog.special_rule_name(int(key, 16))
            for key, entry in catalog.special_rules.items()
            if "{" in str(entry.get("name", ""))
        ]
        self.assertNotIn("自动发动（）", names)
        self.assertTrue(all("{" not in name for name in names))

    def test_product_rule_groups_exclude_disabled_native_rows(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        legal = legal_special_rule_keys(3, tables=tables)
        catalog = load_auxiliary_name_catalog("zh-CN")
        exposed = frozenset(
            key
            for keys in catalog.special_rule_key_groups(allowed_keys=legal).values()
            for key in keys
        )
        self.assertEqual(exposed, legal)
        self.assertEqual(len(legal), 277)
        self.assertTrue({0xE7F5, 0x1BD2, 0x39A6}.isdisjoint(exposed))

    def test_legal_auto_activation_names_use_chinese_or_mark_unknown(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        legal = legal_special_rule_keys(3, tables=tables)
        catalog = load_auxiliary_name_catalog("zh-CN")
        names = {
            catalog.special_rule_name(key)
            for key in legal
            if "自动发动" in catalog.special_rule_name(key)
        }
        self.assertFalse(any("Talisman" in name for name in names))
        self.assertFalse(any("item 0x" in name for name in names))
        self.assertIn(
            "自动发动（未识别阴阳术（原生编号 0x3011，可生成））",
            names,
        )

    def test_priority_drop_table_omits_only_shinatsuhiko_from_rarity4_map(self) -> None:
        tables = load_default_auxiliary_generation_tables()
        legal = legal_special_rule_keys(3, tables=tables)
        priority_graces = {
            struct.unpack_from("<H", row, 0x32)[0]
            for key, row in zip(
                tables.special_rule_keys_by_row,
                tables.special_rules.rows(),
                strict=True,
            )
            if key in legal and struct.unpack_from("<H", row, 0x32)[0]
        }
        rarity4_graces = {
            entry.grace_id for entry in load_grace_output_map(rarity=4).ranges
        }
        self.assertEqual(rarity4_graces - priority_graces, {0x4192})


if __name__ == "__main__":
    unittest.main()
