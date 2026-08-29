from __future__ import annotations

import unittest

from research.dump_effect_catalog_current_locale import TextEntry, choose_name


class RuntimeCatalogDumpTests(unittest.TestCase):
    def test_current_pool_excludes_a_previous_language_pool(self) -> None:
        entries = [
            TextEntry(1, 0x100000, 5, "日本語"),
            TextEntry(1, 0x300000, 8, "English"),
        ]
        name, conflicts = choose_name(entries, pool_center=0x100000)
        self.assertEqual(name, "日本語")
        self.assertEqual(conflicts, [])

    def test_conflicts_inside_current_pool_remain_visible(self) -> None:
        entries = [
            TextEntry(1, 0x100000, 4, "first"),
            TextEntry(1, 0x100100, 4, "second"),
        ]
        name, conflicts = choose_name(entries, pool_center=0x100000)
        self.assertEqual(name, "first")
        self.assertEqual(conflicts, ["second"])


if __name__ == "__main__":
    unittest.main()
