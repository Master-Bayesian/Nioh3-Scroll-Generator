import unittest

from nioh3_scroll_editor.recommended_level import (
    RecommendedLevelCurve,
    native_recommended_level_curve,
    predict_recommended_level,
    resolve_recommended_level,
)


class RecommendedLevelTests(unittest.TestCase):
    def test_known_native_examples(self) -> None:
        self.assertEqual(predict_recommended_level(181).displayed_level, 159)
        self.assertEqual(predict_recommended_level(183).displayed_level, 160)
        self.assertEqual(predict_recommended_level(1400).displayed_level, 700)

    def test_constructor_clamps_out_of_range_inputs(self) -> None:
        below = predict_recommended_level(0)
        above = predict_recommended_level(1500)

        self.assertEqual(below.canonical_internal_level, 156)
        self.assertEqual(below.displayed_level, 142)
        self.assertTrue(below.was_clamped)
        self.assertEqual(above.canonical_internal_level, 1400)
        self.assertEqual(above.displayed_level, 700)
        self.assertTrue(above.was_clamped)

    def test_curve_is_monotonic_over_the_supported_constructor_range(self) -> None:
        curve = native_recommended_level_curve()
        values = [curve.displayed_level(value) for value in range(156, 1401)]
        self.assertEqual(values, sorted(values))
        self.assertEqual(values[0], 142)
        self.assertEqual(values[-1], 700)

    def test_displayed_target_350_returns_both_exact_inputs_and_selects_lowest(self) -> None:
        result = resolve_recommended_level(350)
        self.assertEqual(result.status, "exact")
        self.assertEqual(result.canonical_internal_levels, (585, 586))
        self.assertEqual(result.selected_internal_level, 585)
        self.assertEqual(predict_recommended_level(350).displayed_level, 238)

    def test_inverse_is_complete_over_every_canonical_native_input(self) -> None:
        curve = native_recommended_level_curve()
        expected = {}
        for raw in range(curve.minimum_internal_level, curve.maximum_internal_level + 1):
            expected.setdefault(curve.displayed_level(raw), []).append(raw)
        for displayed, inputs in expected.items():
            with self.subTest(displayed=displayed):
                result = curve.resolve_displayed_level(displayed)
                self.assertEqual(result.status, "exact")
                self.assertEqual(result.canonical_internal_levels, tuple(inputs))
                self.assertEqual(result.selected_internal_level, min(inputs))
                self.assertEqual(curve.predict(result.selected_internal_level).displayed_level, displayed)
        # The upper plateau must include canonical inputs beyond the last curve point.
        self.assertEqual(expected[700], list(range(1301, 1401)))

    def test_unreachable_gap_and_out_of_range_are_distinct_without_clamping(self) -> None:
        curve = RecommendedLevelCurve(1, 4, ((0, 0), (4, 8)))
        self.assertEqual(curve.displayed_level_bounds(), (2, 8))
        for requested, expected in ((3, "unreachable"), (1, "out_of_range"), (9, "out_of_range")):
            with self.subTest(requested=requested):
                result = curve.resolve_displayed_level(requested)
                self.assertEqual(result.requested_displayed_level, requested)
                self.assertEqual(result.status, expected)
                self.assertEqual(result.canonical_internal_levels, ())
                self.assertIsNone(result.selected_internal_level)
        for requested in (-1, 0, 141, 701, 2**80):
            self.assertEqual(resolve_recommended_level(requested).status, "out_of_range")

    def test_inverse_rejects_noninteger_domain_inputs(self) -> None:
        for requested in (True, False, 350.0, 350.5, "350", None, float("nan"), float("inf")):
            with self.subTest(requested=requested):
                with self.assertRaisesRegex(TypeError, "must be an integer"):
                    resolve_recommended_level(requested)


if __name__ == "__main__":
    unittest.main()
