import unittest

from nioh3_scroll_editor.recommended_level import (
    native_recommended_level_curve,
    predict_recommended_level,
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


if __name__ == "__main__":
    unittest.main()
