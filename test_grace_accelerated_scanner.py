import struct
import unittest
from itertools import islice

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE
from nioh3_scroll_editor.grace_map import (
    grace_id_for_seed,
    iter_natural_seeds_for_grace,
    load_grace_output_map,
)
from nioh3_scroll_editor.native import scan_next_candidate
from nioh3_seed_math import is_natural_scroll_id


TARGET_GRACE = 0xCE68
PRIMARY = 0x47BC
ORDINARY = (0xA051, 0x190A, 0x2B06, 0xB613)


def record_for(seed: int, *, primary: int, grace: int) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, 0xE604)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30] = record[0x31] = 5
    for index, effect_id in enumerate((primary, *ORDINARY, grace)):
        struct.pack_into("<I", record, EFFECT_START + index * EFFECT_STRIDE + 4, effect_id)
    return bytes(record)


class GraceAcceleratedScannerTests(unittest.TestCase):
    def test_grace_seeds_are_batched_and_then_filtered_by_primary_and_secondaries(self) -> None:
        mapping = load_grace_output_map()

        class FakeOracle:
            max_batch_size = 2

            def __init__(self) -> None:
                self.batches: list[list[int]] = []

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                seeds = [struct.unpack_from("<I", source, 0x20)[0] for source in source_records]
                self.batches.append(seeds)
                # The invariant that makes this an acceleration: every input to
                # the native generator is both natural and pre-mapped to grace.
                for seed in seeds:
                    self_test.assertTrue(is_natural_scroll_id(seed))
                    self_test.assertEqual(grace_id_for_seed(seed, mapping), TARGET_GRACE)
                generated_count = sum(len(batch) for batch in self.batches)
                return [
                    record_for(
                        seed,
                        primary=PRIMARY if generated_count - len(seeds) + index == 2 else 0x4647,
                        grace=TARGET_GRACE,
                    )
                    for index, seed in enumerate(seeds)
                ]

        self_test = self
        oracle = FakeOracle()
        progress = []
        candidate = scan_next_candidate(
            oracle,
            template=record_for(1, primary=0, grace=TARGET_GRACE),
            start_seed=0xDEADBEEF,  # ignored by the grace-constrained iterator
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(ORDINARY),
            grace_effect_id=TARGET_GRACE,
            rarity=5,
            playthrough=None,
            max_seeds=4,
            progress=progress.append,
        )
        self.assertIsNotNone(candidate)
        self.assertEqual(candidate.primary.effect_id, PRIMARY)
        self.assertEqual(len(oracle.batches), 2)
        self.assertEqual([len(batch) for batch in oracle.batches], [2, 2])
        self.assertEqual(progress[-1].scanned, 2)

    def test_native_grace_mismatch_fails_closed(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                seed = struct.unpack_from("<I", source_records[0], 0x20)[0]
                return [record_for(seed, primary=PRIMARY, grace=0xDFF0)]

        with self.assertRaisesRegex(RuntimeError, "contradicts"):
            scan_next_candidate(
                FakeOracle(),
                template=record_for(1, primary=0, grace=TARGET_GRACE),
                start_seed=0,
                primary_effect_ids=frozenset((PRIMARY,)),
                required_secondary_ids=frozenset(ORDINARY),
                grace_effect_id=TARGET_GRACE,
                rarity=5,
                playthrough=None,
                max_seeds=1,
            )

    def test_one_required_secondary_allows_other_ordinary_secondaries(self) -> None:
        """Reproduces selecting only one effect from a populated four-slot roll."""
        class FakeOracle:
            max_batch_size = 1

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                seed = struct.unpack_from("<I", source_records[0], 0x20)[0]
                return [record_for(seed, primary=PRIMARY, grace=TARGET_GRACE)]

        candidate = scan_next_candidate(
            FakeOracle(),
            template=record_for(1, primary=0, grace=TARGET_GRACE),
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY,)),
            # B613 is present, while A051/190A/2B06 are deliberately unselected.
            required_secondary_ids=frozenset((0xB613,)),
            grace_effect_id=TARGET_GRACE,
            rarity=5,
            playthrough=None,
            max_seeds=1,
        )
        self.assertIsNotNone(candidate)

    def test_grace_cursor_resumes_without_revisiting_the_previous_candidate(self) -> None:
        mapping = load_grace_output_map()
        expected = list(islice(iter_natural_seeds_for_grace(TARGET_GRACE, mapping), 2))

        class FakeOracle:
            max_batch_size = 1

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                seed = struct.unpack_from("<I", source_records[0], 0x20)[0]
                return [record_for(seed, primary=PRIMARY, grace=TARGET_GRACE)]

        common = dict(
            template=record_for(1, primary=0, grace=TARGET_GRACE),
            start_seed=0,  # Legacy range controls do not affect grace paging.
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(ORDINARY),
            grace_effect_id=TARGET_GRACE,
            rarity=5,
            playthrough=None,
            max_seeds=1,
        )
        first = scan_next_candidate(FakeOracle(), **common)
        self.assertIsNotNone(first)
        second = scan_next_candidate(
            FakeOracle(), grace_start_after_seed=first.seed, **common
        )
        self.assertIsNotNone(second)
        self.assertEqual((first.seed, second.seed), (expected[0].seed, expected[1].seed))
        self.assertNotEqual(first.seed, second.seed)

    def test_grace_cursor_is_rejected_without_a_grace_target(self) -> None:
        class UnusedOracle:
            max_batch_size = 1

        with self.assertRaisesRegex(ValueError, "only valid"):
            scan_next_candidate(
                UnusedOracle(),
                template=record_for(1, primary=0, grace=TARGET_GRACE),
                start_seed=0,
                primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset(),
                grace_start_after_seed=1,
                max_seeds=1,
            )

    def test_grace_acceleration_rejects_explicit_playthrough_or_wrong_context(self) -> None:
        class UnusedOracle:
            max_batch_size = 1

        kwargs = dict(
            template=record_for(1, primary=0, grace=TARGET_GRACE),
            start_seed=0,
            primary_effect_ids=frozenset(),
                required_secondary_ids=frozenset(),
            grace_effect_id=TARGET_GRACE,
            rarity=5,
            max_seeds=1,
        )
        with self.assertRaisesRegex(ValueError, "category-3/E604"):
            scan_next_candidate(UnusedOracle(), playthrough=1, **kwargs)
        wrong_type = bytearray(kwargs["template"])
        struct.pack_into("<H", wrong_type, 0, 0x1E82)
        with self.assertRaisesRegex(ValueError, "record type"):
            scan_next_candidate(UnusedOracle(), playthrough=None, template=bytes(wrong_type), **{key: value for key, value in kwargs.items() if key != "template"})

    def test_rarity4_uses_its_own_first_u16_map_and_slot5(self) -> None:
        mapping = load_grace_output_map(rarity=4)
        ordinary_r4 = ORDINARY[:3]

        def r4_record(seed: int, primary: int) -> bytes:
            record = bytearray(SCROLL_RECORD_SIZE)
            struct.pack_into("<H", record, 0, 0xE604)
            struct.pack_into("<I", record, 0x20, seed)
            record[0x30] = record[0x31] = 4
            for index, effect_id in enumerate((primary, *ordinary_r4, TARGET_GRACE)):
                struct.pack_into("<I", record, EFFECT_START + index * EFFECT_STRIDE + 4, effect_id)
            return bytes(record)

        class FakeOracle:
            max_batch_size = 2

            def __init__(self) -> None:
                self.seeds: list[int] = []

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                out = []
                for source in source_records:
                    self_test.assertEqual(source[0x31], 4)
                    seed = struct.unpack_from("<I", source, 0x20)[0]
                    self_test.assertEqual(grace_id_for_seed(seed, mapping), TARGET_GRACE)
                    self.seeds.append(seed)
                    out.append(r4_record(seed, PRIMARY))
                return out

        self_test = self
        oracle = FakeOracle()
        candidate = scan_next_candidate(
            oracle,
            template=r4_record(1, 0),
            start_seed=0xDEADBEEF,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(ordinary_r4),
            grace_effect_id=TARGET_GRACE,
            rarity=4,
            playthrough=None,
            max_seeds=2,
        )
        self.assertIsNotNone(candidate)
        self.assertEqual(candidate.seed, oracle.seeds[0])
        self.assertEqual(candidate.effects[4].effect_id, TARGET_GRACE)

    def test_rarity3_acceleration_uses_r4_map_then_shadow_validates_only_match(self) -> None:
        mapping = load_grace_output_map(rarity=4)
        ordinary_r3 = ORDINARY[:3]

        def make_record(seed: int, *, rarity: int, slot5: int, primary: int) -> bytes:
            record = bytearray(SCROLL_RECORD_SIZE)
            struct.pack_into("<H", record, 0, 0xE604)
            struct.pack_into("<I", record, 0x20, seed)
            record[0x30] = record[0x31] = rarity
            for index, effect_id in enumerate((primary, *ordinary_r3, slot5)):
                struct.pack_into("<I", record, EFFECT_START + index * EFFECT_STRIDE + 4, effect_id)
            return bytes(record)

        class FakeOracle:
            max_batch_size = 2

            def __init__(self) -> None:
                self.r3_calls = 0
                self.r4_shadow_calls = 0

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                rarity = source_records[0][0x31]
                if rarity == 3:
                    self.r3_calls += 1
                    result = []
                    for source in source_records:
                        seed = struct.unpack_from("<I", source, 0x20)[0]
                        self_test.assertEqual(grace_id_for_seed(seed, mapping), TARGET_GRACE)
                        result.append(make_record(seed, rarity=3, slot5=0x0001, primary=PRIMARY))
                    return result
                if rarity == 4:
                    self.r4_shadow_calls += 1
                    self_test.assertEqual(len(source_records), 1)
                    seed = struct.unpack_from("<I", source_records[0], 0x20)[0]
                    self_test.assertEqual(grace_id_for_seed(seed, mapping), TARGET_GRACE)
                    return [make_record(seed, rarity=4, slot5=TARGET_GRACE, primary=PRIMARY)]
                raise AssertionError(f"unexpected rarity {rarity}")

        self_test = self
        oracle = FakeOracle()
        template = make_record(1, rarity=3, slot5=0x0001, primary=0)
        candidate = scan_next_candidate(
            oracle,
            template=template,
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(ordinary_r3),
            grace_effect_id=TARGET_GRACE,
            rarity=3,
            playthrough=None,
            max_seeds=2,
        )
        self.assertIsNotNone(candidate)
        self.assertEqual(candidate.effects[4].effect_id, 0x0001)
        self.assertEqual(candidate.predicted_growth_grace_id, TARGET_GRACE)
        self.assertEqual(oracle.r3_calls, 1)
        self.assertEqual(oracle.r4_shadow_calls, 1)


if __name__ == "__main__":
    unittest.main()
