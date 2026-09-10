"""Locale changes must never change matching identities or hide catalog gaps."""
import unittest
from tools.audit_v2_localization import audit
from nioh3_scroll_editor.presentation_strings import LABELS, label


class LocalizationTests(unittest.TestCase):
    def test_numeric_catalog_identity_and_known_exact_coverage_gaps(self):
        result = audit()['locales']
        for locale, row in result.items():
            with self.subTest(locale=locale):
                self.assertTrue(row['all_playthrough_option_identities_equal'])
                self.assertEqual(row['effect_count'], 3609)
        self.assertEqual(result['zh-CN']['missing_exact_effect_names'], [])
        self.assertEqual(result['ja-JP']['missing_exact_effect_names'], [])
        self.assertEqual(set(result['en-US']['missing_exact_effect_names']), {'0x0000', '0x788B', '0x966D', '0xC542', '0xFD4C'})
        groups = result['en-US']['missing_effect_text_groups']
        self.assertEqual(len(groups), 1)
        self.assertEqual(groups[0]['text_id'], '0x03441371')
        self.assertTrue(groups[0]['known_placeholder'])
        self.assertEqual(set(groups[0]['effect_ids']), set(result['en-US']['missing_exact_effect_names']))
        self.assertEqual(result['en-US']['missing_exact_item_qualifiers'], [])
        self.assertEqual(result['zh-CN']['missing_exact_item_qualifiers'], [])
        self.assertEqual(result['ja-JP']['missing_exact_item_qualifiers'], [])

    def test_catalog_fallbacks_have_complete_localized_labels(self):
        for locale, values in LABELS.items():
            self.assertEqual(set(values), set(LABELS['en-US']))
            self.assertIn('0x1234', label(locale, 'unknown_rule', value='0x1234'))


if __name__ == '__main__': unittest.main()
