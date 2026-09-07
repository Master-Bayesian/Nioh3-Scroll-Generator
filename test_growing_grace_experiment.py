import struct
import unittest

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE
from nioh3_scroll_editor.native import scan_next_candidate


TARGET_GRACE = 0x6553
OTHER_GRACE = 0xCE68
PRIMARY = 0x47BC
ORDINARY = (0x4647, 0xA051, 0x190A)


def make_record(
    *, seed: int, rarity: int, slot5: int, record_type: int = 0xE604
) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, record_type)
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
    def test_seed_43723117_filters_the_post_reveal_effects(self) -> None:
        stage_effect_id = 0xD411
        final_effect_id = 0xF9BE

        def record_with_third_effect(effect_id: int) -> bytes:
            record = bytearray(
                make_record(
                    seed=43_723_117,
                    rarity=4,
                    slot5=TARGET_GRACE,
                    record_type=0x516D,
                )
            )
            struct.pack_into("<I", record, EFFECT_START + 2 * EFFECT_STRIDE + 4, effect_id)
            return bytes(record)

        stage_record = record_with_third_effect(stage_effect_id)
        final_record = record_with_third_effect(final_effect_id)
        self_test = self

        class FakeOracle:
            max_batch_size = 1

            @staticmethod
            def generate_seed_range(
                template: bytes,
                *,
                start_seed: int,
                seed_step: int,
                count: int,
                playthrough=None,
            ) -> list[bytes]:
                return [stage_record]

            @staticmethod
            def finalize_stage_records_batch(source_records: list[bytes]) -> list[bytes]:
                self_test.assertEqual(source_records, [stage_record])
                return [final_record]

        rejected = scan_next_candidate(
            FakeOracle(),
            template=stage_record,
            start_seed=43_723_117,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset((stage_effect_id,)),
            rarity=4,
            playthrough=2,
            max_seeds=1,
        )
        accepted = scan_next_candidate(
            FakeOracle(),
            template=stage_record,
            start_seed=43_723_117,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset((final_effect_id,)),
            rarity=4,
            playthrough=2,
            max_seeds=1,
        )

        self.assertIsNone(rejected)
        self.assertIsNotNone(accepted)
        assert accepted is not None
        self.assertEqual(accepted.record, final_record)
        self.assertEqual(accepted.installation_record, stage_record)
        self.assertEqual(accepted.record_stage.value, "final_record")
        self.assertIsNone(accepted.install_blocker)

    def test_seed_36526331_never_leaves_the_native_stage_as_candidate(self) -> None:
        stage_record = bytearray(
            make_record(
                seed=36_526_331,
                rarity=4,
                slot5=0xEB61,
                record_type=0x516D,
            )
        )
        final_record = bytearray(stage_record)
        final_record[EFFECT_START + 2 * EFFECT_STRIDE + 0x0E] |= 0x04

        class FakeOracle:
            max_batch_size = 1

            @staticmethod
            def generate_seed_range(
                template: bytes,
                *,
                start_seed: int,
                seed_step: int,
                count: int,
                playthrough=None,
            ) -> list[bytes]:
                return [bytes(stage_record)]

            @staticmethod
            def finalize_stage_records_batch(source_records: list[bytes]) -> list[bytes]:
                return [bytes(final_record)]

        candidate = scan_next_candidate(
            FakeOracle(),
            template=bytes(stage_record),
            start_seed=36_526_331,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(),
            rarity=4,
            playthrough=2,
            max_seeds=1,
        )

        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertEqual(candidate.record, bytes(final_record))
        self.assertEqual(candidate.installation_record, bytes(stage_record))
        self.assertEqual(candidate.record_stage.value, "final_record")
        self.assertEqual(candidate.unresolved_effect_slots, ())
        self.assertIsNone(candidate.install_blocker)

    def test_playthrough_one_rarity_four_uses_final_preview_and_stage_install_record(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            @staticmethod
            def generate_seed_range(
                template: bytes,
                *,
                start_seed: int,
                seed_step: int,
                count: int,
                playthrough=None,
            ) -> list[bytes]:
                return [
                    make_record(
                        seed=start_seed,
                        rarity=4,
                        slot5=TARGET_GRACE,
                        record_type=0x1E82,
                    )
                ]

            @staticmethod
            def finalize_stage_records_batch(source_records: list[bytes]) -> list[bytes]:
                return source_records

        candidate = scan_next_candidate(
            FakeOracle(),
            template=make_record(
                seed=1,
                rarity=4,
                slot5=TARGET_GRACE,
                record_type=0x1E82,
            ),
            start_seed=456,
            primary_effect_ids=frozenset((PRIMARY,)),
            required_secondary_ids=frozenset(),
            rarity=4,
            playthrough=1,
            max_seeds=1,
        )
        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertEqual(candidate.record_stage.value, "final_record")
        self.assertEqual(candidate.installation_record, candidate.record)
        self.assertIsNone(candidate.install_blocker)

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

    def test_rarity4_control_filters_finalized_slot5(self) -> None:
        class FakeOracle:
            max_batch_size = 1

            def generate_seed_range(self, template: bytes, *, start_seed: int, seed_step: int, count: int, playthrough=None):
                return [make_record(seed=start_seed, rarity=4, slot5=TARGET_GRACE)]

            @staticmethod
            def finalize_stage_records_batch(source_records: list[bytes]) -> list[bytes]:
                return source_records

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
        self.assertEqual(candidate.installation_record, candidate.record)
        self.assertEqual(candidate.record_stage.value, "final_record")


if __name__ == "__main__":
    unittest.main()
