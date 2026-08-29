from __future__ import annotations

import unittest

from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.auxiliary_generation import (
    load_default_auxiliary_generation_tables,
)


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


if __name__ == "__main__":
    unittest.main()
