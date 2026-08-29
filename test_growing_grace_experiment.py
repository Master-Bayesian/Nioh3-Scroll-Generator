import struct
import unittest

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE
from nioh3_scroll_editor.native import scan_next_candidate


TARGET_GRACE = 0x6553
OTHER_GRACE = 0xCE68
PRIMARY = 0x47BC
ORDINARY = (0x4647, 0xA051, 0x190A)


def make_record(*, seed: int, rarity: int, slot5: int) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, 0xE604)
    struct.pack_into("<H", record, 6, 180)
    struct.pack_into("<H", record, 8, 180)
    struct.pack_into("<H", record, 0x10, 183)
    struct.pack_into("<H", record, 0x12, 183)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30] = rarity
    record[0x31] = rarity
    effects = (PRIMARY, *ORDINARY, slot5)
    for index, effect_id in enumerate(effects):
        offset = EFFECT_START + index * EFFECT_STRIDE
        struct.pack_into(
            "<6I", record, offset,
            index + 1, effect_id, 100 + index, 200 + index, 0, 0,
        )
    # Remaining slots are explicitly empty.
    for index in range(len(effects), 7):
        struct.pack_into("<I", record, EFFECT_START + index * EFFECT_STRIDE + 4, 0xFFFFFFFF)
    return bytes(record)


class GrowingGraceExperimentTests(unittest.TestCase):
    def test_rarity3_returns_growing_record_when_same_seed_r4_shadow_matches(self) -> None:
        seed = 67_966_805

        class FakeOracle:
            max_batch_size = 8

            def generate_seed_range(self, template: bytes, *, start_seed: int, seed_step: int, count: int, playthrough=None):
                self.assert_rarity(template, 3)
                return [
                    make_record(seed=(start_seed + i * seed_step) & 0xFFFFFFFF, rarity=3, slot5=0x0001)
                    for i in range(count)
                ]

            @staticmethod
            def assert_rarity(record: bytes, rarity: int) -> None:
                assert record[0x30] == rarity and record[0x31] == rarity

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                result = []
                for source in source_records:
                    self.assert_rarity(source, 4)
                    shadow_seed = struct.unpack_from("<I", source, 0x20)[0]
                    result.append(make_record(seed=shadow_seed, rarity=4, slot5=TARGET_GRACE))
                return result

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(seed=1, rarity=3, slot5=0x0001),
            start_seed=seed,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset((0xA051,)),
            grace_effect_id=TARGET_GRACE,
            rarity=3,
            playthrough=None,
            max_seeds=1,
            accelerate_grace=False,
        )
        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertEqual(candidate.seed, seed)
        self.assertEqual(candidate.record[0x30:0x32], b"\x03\x03")
        self.assertEqual(
            struct.unpack_from("<I", candidate.record, EFFECT_START + 4 * EFFECT_STRIDE + 4)[0],
            0x0001,
        )
        self.assertEqual(candidate.predicted_growth_grace_id, TARGET_GRACE)

    def test_rarity3_rejects_when_shadow_grace_does_not_match_target(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            def generate_seed_range(self, template: bytes, *, start_seed: int, seed_step: int, count: int, playthrough=None):
                return [make_record(seed=start_seed, rarity=3, slot5=0x0001)]

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                seed = struct.unpack_from("<I", source_records[0], 0x20)[0]
                return [make_record(seed=seed, rarity=4, slot5=OTHER_GRACE)]

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(seed=1, rarity=3, slot5=0x0001),
            start_seed=123,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset(),
            grace_effect_id=TARGET_GRACE,
            rarity=3,
            playthrough=None,
            max_seeds=1,
            accelerate_grace=False,
        )
        self.assertIsNone(candidate)

    def test_rarity3_requires_actual_growing_effect_in_slot5(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            def generate_seed_range(self, template: bytes, *, start_seed: int, seed_step: int, count: int, playthrough=None):
                return [make_record(seed=start_seed, rarity=3, slot5=OTHER_GRACE)]

            def generate(self, source_records: list[bytes]) -> list[bytes]:
                raise AssertionError("shadow generation must not run without slot5 0x0001")

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(seed=1, rarity=3, slot5=0x0001),
            start_seed=123,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset(),
            grace_effect_id=TARGET_GRACE,
            rarity=3,
            playthrough=None,
            max_seeds=1,
            accelerate_grace=False,
        )
        self.assertIsNone(candidate)

    def test_rarity4_control_filters_slot5_directly(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            def generate_seed_range(self, template: bytes, *, start_seed: int, seed_step: int, count: int, playthrough=None):
                return [make_record(seed=start_seed, rarity=4, slot5=TARGET_GRACE)]

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(seed=1, rarity=4, slot5=TARGET_GRACE),
            start_seed=456,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset(),
            grace_effect_id=TARGET_GRACE,
            rarity=4,
            playthrough=None,
            max_seeds=1,
            accelerate_grace=False,
        )
        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertIsNone(candidate.predicted_growth_grace_id)
        self.assertEqual(candidate.seed, 456)


if __name__ == "__main__":
    unittest.main()
