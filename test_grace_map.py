import itertools
import json
import struct
import tempfile
import unittest
from collections.abc import Iterator
from pathlib import Path

from nioh3_seed_math import (
    is_natural_scroll_id,
    iter_natural_seeds_for_first_u16,
    is_natural_scroll_id,
    lcg_step,
)
from nioh3_scroll_editor.grace_map import (
    GraceOutputMap,
    GraceRange,
    build_live_grace_output_map,
    find_natural_seed_for_grace,
    first_u16_ranges_for_grace,
    grace_id_for_first_u16,
    grace_id_for_seed,
    iter_natural_seeds_for_grace,
    load_grace_map_cache,
    load_grace_output_map,
    save_grace_map_cache,
)
from nioh3_scroll_editor.catalog import effect_name
from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE


VECTORS = (
    (0x6553, 0, 0x040D1755),
    (0xCE68, 5964, 0x0C891755),
    (0xBABD, 11921, 0x09079123),
    (0xEEEA, 17878, 0x09932246),
    (0x16E2, 23836, 0x0733F50F),
    (0x4192, 29793, 0x07BF8632),
    (0x47EC, 35750, 0x084B1755),
    (0x4FE4, 41707, 0x04C99123),
    (0xEB61, 47665, 0x06777B41),
    (0x7ECE, 53622, 0x02F5F50F),
    (0x71F6, 59579, 0x03818632),
)


class GraceMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.mapping = load_grace_output_map()

    def test_ranges_are_a_complete_contiguous_partition(self) -> None:
        ranges = self.mapping.ranges
        self.assertEqual(ranges[0].start, 0)
        self.assertEqual(ranges[-1].end, 0xFFFF)
        self.assertEqual(sum(entry.end - entry.start + 1 for entry in ranges), 0x10000)
        self.assertEqual(len({entry.grace_id for entry in ranges}), len(ranges))
        for previous, current in itertools.pairwise(ranges):
            self.assertEqual(previous.end + 1, current.start)

    def test_all_vectors_forward_to_their_verified_grace(self) -> None:
        for grace_id, first_u16, seed in VECTORS:
            with self.subTest(grace_id=hex(grace_id)):
                self.assertTrue(is_natural_scroll_id(seed))
                self.assertEqual(lcg_step(seed) >> 16, first_u16)
                self.assertEqual(grace_id_for_seed(seed, self.mapping), grace_id)

    def test_every_range_boundary_maps_without_off_by_one(self) -> None:
        for index, current in enumerate(self.mapping.ranges):
            with self.subTest(boundary="start", grace_id=hex(current.grace_id)):
                self.assertEqual(grace_id_for_first_u16(current.start, self.mapping), current.grace_id)
            with self.subTest(boundary="end", grace_id=hex(current.grace_id)):
                self.assertEqual(grace_id_for_first_u16(current.end, self.mapping), current.grace_id)
            if index:
                self.assertEqual(
                    grace_id_for_first_u16(current.start - 1, self.mapping),
                    self.mapping.ranges[index - 1].grace_id,
                )
            if index + 1 < len(self.mapping.ranges):
                self.assertEqual(
                    grace_id_for_first_u16(current.end + 1, self.mapping),
                    self.mapping.ranges[index + 1].grace_id,
                )

    def test_inverse_finds_a_natural_seed_for_every_grace(self) -> None:
        for grace_range in self.mapping.ranges:
            with self.subTest(grace_id=hex(grace_range.grace_id)):
                result = find_natural_seed_for_grace(grace_range.grace_id, self.mapping)
                self.assertTrue(is_natural_scroll_id(result.seed))
                self.assertEqual(result.state1, lcg_step(result.seed))
                self.assertEqual(result.random_u16, result.state1 >> 16)
                self.assertEqual(grace_id_for_seed(result.seed, self.mapping), grace_range.grace_id)

    def test_first_u16_iterator_is_lazy_and_preserves_the_bucket(self) -> None:
        iterator = iter_natural_seeds_for_first_u16(5964)
        self.assertIsInstance(iterator, Iterator)
        results = list(itertools.islice(iterator, 8))
        self.assertEqual(len(results), 8)
        self.assertEqual(len({item.seed for item in results}), len(results))
        for result in results:
            self.assertTrue(is_natural_scroll_id(result.seed))
            self.assertEqual(lcg_step(result.seed) >> 16, 5964)

    def test_grace_iterator_has_limit_and_never_materializes_the_space(self) -> None:
        iterator = iter_natural_seeds_for_grace(0xCE68, self.mapping, max_results=3)
        self.assertIsInstance(iterator, Iterator)
        results = list(iterator)
        self.assertEqual(len(results), 3)
        self.assertTrue(all(grace_id_for_seed(item.seed, self.mapping) == 0xCE68 for item in results))
        self.assertEqual(list(iter_natural_seeds_for_grace(0xCE68, self.mapping, max_results=0)), [])

    def test_grace_iterator_resumes_directly_after_its_seed_cursor(self) -> None:
        expected = list(
            itertools.islice(iter_natural_seeds_for_grace(0xCE68, self.mapping), 8)
        )
        first_page = list(
            iter_natural_seeds_for_grace(0xCE68, self.mapping, max_results=3)
        )
        resumed_page = list(
            iter_natural_seeds_for_grace(
                0xCE68,
                self.mapping,
                max_results=5,
                start_after_seed=first_page[-1].seed,
            )
        )
        self.assertEqual(first_page + resumed_page, expected)

    def test_grace_iterator_rejects_a_cursor_for_another_grace(self) -> None:
        other_grace_seed = VECTORS[0][2]
        with self.assertRaisesRegex(ValueError, "does not belong to the requested grace"):
            list(
                iter_natural_seeds_for_grace(
                    0xCE68, self.mapping, start_after_seed=other_grace_seed
                )
            )

    def test_loader_rejects_a_context_that_is_not_verified(self) -> None:
        data_path = Path(__file__).parent / "nioh3_scroll_editor" / "data" / "grace_output_map_e604_r5_current.json"
        data = json.loads(data_path.read_text(encoding="utf-8"))
        data["context"]["rarity"] = 4
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "invalid.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "verified E604/r5/slot6"):
                load_grace_output_map(path)

    def test_grace_ranges_are_selected_by_effect_id(self) -> None:
        target = first_u16_ranges_for_grace(0xCE68, self.mapping)
        self.assertEqual(target, (self.mapping.ranges[1],))
        with self.assertRaises(ValueError):
            first_u16_ranges_for_grace(0xDEAD, self.mapping)

    def test_6553_uses_the_confirmed_amaterasu_display_name(self) -> None:
        self.assertEqual(effect_name(0x6553), "天照大神的恩宠")

    def test_live_category_four_map_covers_all_first_draw_buckets(self) -> None:
        class FakeOracle:
            max_batch_size = 0x10000

            @staticmethod
            def generate(source_records: list[bytes], *, timeout_ms: int = 60_000) -> list[bytes]:
                generated = []
                for source in source_records:
                    record = bytearray(source)
                    seed = struct.unpack_from("<I", record, 0x20)[0]
                    effect_id = 0x6553 if lcg_step(seed) >> 16 < 0x8000 else 0xCE68
                    struct.pack_into(
                        "<I", record, EFFECT_START + 5 * EFFECT_STRIDE + 4, effect_id
                    )
                    generated.append(bytes(record))
                return generated

        template = bytearray(SCROLL_RECORD_SIZE)
        struct.pack_into("<H", template, 0, 0xDD82)
        mapping = build_live_grace_output_map(
            FakeOracle(), template=bytes(template), category=4
        )
        self.assertEqual(mapping.record_type, 0xDD82)
        self.assertEqual(mapping.playthrough, "category-4-live-native")
        self.assertEqual(
            mapping.ranges,
            (
                GraceRange(0, 0x7FFF, 0x6553),
                GraceRange(0x8000, 0xFFFF, 0xCE68),
            ),
        )

    def test_live_map_cache_roundtrip_uses_context_fingerprint(self) -> None:
        mapping = GraceOutputMap(
            record_type=0xDD82,
            rarity=5,
            playthrough="category-4-live-native",
            effect_slot=6,
            ranges=(
                GraceRange(0, 0x7FFF, 0x6553),
                GraceRange(0x8000, 0xFFFF, 0xCE68),
            ),
        )
        fingerprint = "ab" * 32
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "grace-map.json"
            save_grace_map_cache(
                path,
                mapping,
                context_fingerprint=fingerprint,
            )
            self.assertEqual(
                load_grace_map_cache(
                    path,
                    expected_context_fingerprint=fingerprint.upper(),
                ),
                mapping,
            )
            with self.assertRaisesRegex(ValueError, "different save context"):
                load_grace_map_cache(
                    path,
                    expected_context_fingerprint="cd" * 32,
                )


if __name__ == "__main__":
    unittest.main()

class Rarity4GraceMapTests(unittest.TestCase):
    def setUp(self) -> None:
        self.mapping = load_grace_output_map(rarity=4)

    def test_r4_map_is_complete_21_range_partition(self) -> None:
        self.assertEqual(self.mapping.rarity, 4)
        self.assertEqual(self.mapping.effect_slot, 5)
        self.assertEqual(len(self.mapping.ranges), 21)
        expected_start = 0
        total = 0
        for entry in self.mapping.ranges:
            self.assertEqual(entry.start, expected_start)
            self.assertLessEqual(entry.start, entry.end)
            total += entry.end - entry.start + 1
            expected_start = entry.end + 1
        self.assertEqual(expected_start, 0x10000)
        self.assertEqual(total, 0x10000)

    def test_r4_known_boundaries_from_full_probe(self) -> None:
        expected = (
            (0, 3123, 0x6553),
            (3124, 6244, 0xCE68),
            (28089, 31209, 0x23E5),
            (62416, 65535, 0x71F6),
        )
        by_span = {(entry.start, entry.end): entry.grace_id for entry in self.mapping.ranges}
        for start, end, grace_id in expected:
            self.assertEqual(by_span[(start, end)], grace_id)
            self.assertEqual(grace_id_for_first_u16(start, self.mapping), grace_id)
            self.assertEqual(grace_id_for_first_u16(end, self.mapping), grace_id)

    def test_r4_inverse_returns_natural_seed_for_every_grace(self) -> None:
        for entry in self.mapping.ranges:
            result = find_natural_seed_for_grace(entry.grace_id, self.mapping)
            self.assertTrue(is_natural_scroll_id(result.seed))
            self.assertEqual(grace_id_for_seed(result.seed, self.mapping), entry.grace_id)
