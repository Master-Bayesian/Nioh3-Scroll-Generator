import unittest

from nioh3_scroll_editor.effect_catalog import (
    CatalogGenerationStage,
    CatalogProvenance,
    EffectCatalogEntry,
    EffectContextKey,
    EffectRole,
    EffectSlotPayload,
    capture_all_localized_names,
    catalog_entry_to_dict,
)


class FakeTextOracle:
    def available_locales(self):
        return ("zh_CN", "en-US", "ja-JP", "de-DE")

    def resolve_text(self, text_id: int, locale: str):
        return {
            "zh-CN": "月读的恩宠",
            "en-US": "Grace of Tsukuyomi",
            "ja-JP": "ツクヨミの恩寵",
            "de-DE": None,
        }[locale]


class EffectCatalogTests(unittest.TestCase):
    def test_all_locales_are_captured_without_fallback(self) -> None:
        names = capture_all_localized_names(
            FakeTextOracle(),
            text_id=12345,
            evidence_id="resolver-capture-001",
        )
        self.assertEqual(tuple(name.locale for name in names), ("en-US", "ja-JP", "zh-CN"))
        self.assertTrue(
            all(name.provenance is CatalogProvenance.NATIVE_RESOLVER for name in names)
        )
        self.assertNotIn("de-DE", {name.locale for name in names})

    def test_context_key_keeps_same_raw_id_in_separate_slot_namespaces(self) -> None:
        raw = EffectSlotPayload(1, 0xBABD, 0, 0, 0, 0)
        ordinary = EffectContextKey(
            "2.00.02",
            0xE604,
            3,
            5,
            CatalogGenerationStage.FINAL_RECORD,
            EffectRole.SECONDARY,
            5,
            raw,
        )
        grace = EffectContextKey(
            "2.00.02",
            0xE604,
            3,
            5,
            CatalogGenerationStage.FINAL_RECORD,
            EffectRole.GRACE,
            6,
            raw,
        )
        self.assertNotEqual(ordinary, grace)

    def test_context_key_separates_stage_one_from_final_record(self) -> None:
        raw = EffectSlotPayload(0xA1B1, 0xBABD, 0, 0x20C00, 0, 0)
        stage_one = EffectContextKey(
            "2.00.02",
            0xE604,
            3,
            4,
            CatalogGenerationStage.NATIVE_STAGE_ONE,
            EffectRole.SPECIAL,
            5,
            raw,
        )
        final = EffectContextKey(
            "2.00.02",
            0xE604,
            3,
            4,
            CatalogGenerationStage.FINAL_RECORD,
            EffectRole.SPECIAL,
            5,
            raw,
        )
        self.assertNotEqual(stage_one, final)

    def test_serialized_entry_retains_provenance_and_full_context(self) -> None:
        raw = EffectSlotPayload(1, 0xBABD, 10, 20, 30, 40)
        key = EffectContextKey(
            "2.00.02",
            0xE604,
            3,
            5,
            CatalogGenerationStage.FINAL_RECORD,
            EffectRole.GRACE,
            6,
            raw,
        )
        names = capture_all_localized_names(
            FakeTextOracle(), text_id=12345, evidence_id="resolver-capture-001"
        )
        rendered = catalog_entry_to_dict(
            EffectCatalogEntry(key=key, sample_seed=1, names=names)
        )
        self.assertEqual(rendered["context"]["slot_role"], "grace")
        self.assertEqual(rendered["context"]["generation_stage"], "final_record")
        self.assertEqual(rendered["raw_slot"]["effect_id"], "0x0000BABD")
        self.assertEqual(rendered["names"][0]["provenance"], "native_resolver")


if __name__ == "__main__":
    unittest.main()
