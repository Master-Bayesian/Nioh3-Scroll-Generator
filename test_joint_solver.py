from __future__ import annotations

import struct
import unittest

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE, account_id_from_record
from nioh3_scroll_editor.auxiliary_generation import AuxiliarySearchCriteria
from nioh3_seed_math import state_after_draw_from_seed
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.joint_solver import (
    DrawConstraint,
    U16Runs,
    iter_constraint_intersection,
)
from nioh3_scroll_editor.seed_accelerator import (
    collect_natural_pivot_seeds,
    native_seed_acceleration_available,
)
from nioh3_scroll_editor.native import scan_next_candidate
from nioh3_scroll_editor.primary_map import (
    build_primary_first_draw_output_map,
    build_primary_output_map,
)
from nioh3_scroll_editor.savegame import SaveInventory, SCROLL_GROUP_OFFSET


GRACE = 0x6553
PRIMARY_LOW = 0x47BC
PRIMARY_HIGH = 0xA051


def make_template(*, record_type: int = 0xE604, account_id: int = 1) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, record_type)
    struct.pack_into("<H", record, 0x02, (account_id >> 48) & 0xFFFF)
    struct.pack_into("<H", record, 0x04, (account_id >> 32) & 0xFFFF)
    struct.pack_into("<I", record, 0x14, account_id & 0xFFFFFFFF)
    record[0x30] = record[0x31] = 5
    return bytes(record)


class FakeNativeOracle:
    max_batch_size = 4096

    def generate(self, source_records: list[bytes], *, timeout_ms: int = 60_000) -> list[bytes]:
        results: list[bytes] = []
        for source in source_records:
            record = bytearray(source)
            seed = struct.unpack_from("<I", record, 0x20)[0]
            draw2 = state_after_draw_from_seed(seed, 2) >> 16
            effects = (
                PRIMARY_LOW if draw2 < 0x8000 else PRIMARY_HIGH,
                0x190A,
                0x2B06,
                0xB613,
                0x4647,
                GRACE,
            )
            for index, effect_id in enumerate(effects):
                struct.pack_into(
                    "<6I",
                    record,
                    EFFECT_START + index * EFFECT_STRIDE,
                    index + 1,
                    effect_id,
                    100 + index,
                    200 + index,
                    0,
                    0,
                )
            struct.pack_into(
                "<I", record, EFFECT_START + 6 * EFFECT_STRIDE + 4, 0xFFFFFFFF
            )
            results.append(bytes(record))
        return results


class FakeFirstDrawOracle:
    max_batch_size = 4096

    def generate(self, source_records: list[bytes], *, timeout_ms: int = 60_000) -> list[bytes]:
        results: list[bytes] = []
        for source in source_records:
            record = bytearray(source)
            seed = struct.unpack_from("<I", record, 0x20)[0]
            draw1 = state_after_draw_from_seed(seed, 1) >> 16
            effects = (
                PRIMARY_LOW if draw1 < 0x8000 else PRIMARY_HIGH,
                0x190A,
                0x2B06,
                0xB613,
                0x4647,
                0x3E7A,
            )
            for index, effect_id in enumerate(effects):
                struct.pack_into(
                    "<6I",
                    record,
                    EFFECT_START + index * EFFECT_STRIDE,
                    index + 1,
                    effect_id,
                    100 + index,
                    200 + index,
                    0,
                    0,
                )
            struct.pack_into(
                "<I", record, EFFECT_START + 6 * EFFECT_STRIDE + 4, 0xFFFFFFFF
            )
            results.append(bytes(record))
        return results


class JointSolverTests(unittest.TestCase):
    def test_native_accelerator_rejects_oversized_single_allocation(self) -> None:
        if not native_seed_acceleration_available():
            self.skipTest("native Seed accelerator is not built")
        with self.assertRaisesRegex(ValueError, "1,000,000"):
            collect_natural_pivot_seeds(
                (1,),
                start_index=0,
                stop_index=1_000_001,
                low16_stride=0x9E37,
            )

    def test_native_pivot_accelerator_matches_python_fallback(self) -> None:
        if not native_seed_acceleration_available():
            self.skipTest("native Seed accelerator is not built")
        constraints = (
            DrawConstraint("grace", 1, U16Runs.from_ranges(((100, 105),))),
        )
        arguments = {
            "natural_only": True,
            "start_after_trial": 12_345,
            "max_trials": 250_000,
        }
        accelerated = list(
            iter_constraint_intersection(
                constraints,
                **arguments,
                use_native_acceleration=True,
            )
        )
        fallback = list(
            iter_constraint_intersection(
                constraints,
                **arguments,
                use_native_acceleration=False,
            )
        )
        self.assertEqual(accelerated, fallback)

    def test_native_draw2_pivot_matches_python_fallback(self) -> None:
        if not native_seed_acceleration_available():
            self.skipTest("native Seed accelerator is not built")
        constraints = (
            DrawConstraint("primary", 2, U16Runs.from_ranges(((400, 407),))),
        )
        arguments = {
            "natural_only": True,
            "start_after_trial": 23_456,
            "max_trials": 250_000,
        }
        accelerated = list(
            iter_constraint_intersection(
                constraints,
                **arguments,
                use_native_acceleration=True,
            )
        )
        fallback = list(
            iter_constraint_intersection(
                constraints,
                **arguments,
                use_native_acceleration=False,
            )
        )
        self.assertEqual(accelerated, fallback)

    @classmethod
    def setUpClass(cls) -> None:
        cls.mapping = load_grace_output_map(rarity=5)
        cls.template = make_template()
        cls.primary_map = build_primary_output_map(
            FakeNativeOracle(),
            template=cls.template,
            grace_effect_id=GRACE,
            mapping=cls.mapping,
        )

    def test_primary_map_covers_every_draw2_bucket_once(self) -> None:
        self.assertEqual(self.primary_map.bucket_count, 0x10000)
        effects = dict(self.primary_map.effects)
        self.assertEqual(effects[PRIMARY_LOW].bucket_count, 0x8000)
        self.assertEqual(effects[PRIMARY_HIGH].bucket_count, 0x8000)

    def test_constraint_intersection_resumes_without_replay(self) -> None:
        first = DrawConstraint("first", 1, U16Runs.from_ranges(((100, 200),)))
        second = DrawConstraint("second", 2, U16Runs.from_ranges(((300, 400),)))
        initial = next(iter_constraint_intersection((first, second), natural_only=False))
        resumed = next(
            iter_constraint_intersection(
                (first, second),
                natural_only=False,
                start_after_trial=initial.pivot_trial,
            )
        )
        self.assertGreater(resumed.pivot_trial, initial.pivot_trial)
        self.assertNotEqual(resumed.seed, initial.seed)
        self.assertTrue(first.matches(resumed.seed))
        self.assertTrue(second.matches(resumed.seed))

    def test_complete_grace_primary_and_multi_secondary_search(self) -> None:
        first = scan_next_candidate(
            FakeNativeOracle(),
            template=self.template,
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY_LOW,)),
            required_secondary_ids=frozenset((0x190A, 0x2B06, 0xB613)),
            grace_effect_id=GRACE,
            rarity=5,
            max_seeds=128,
            primary_output_map=self.primary_map,
        )
        self.assertIsNotNone(first)
        assert first is not None
        self.assertEqual(first.primary.effect_id, PRIMARY_LOW)
        self.assertEqual(first.effects[5].effect_id, GRACE)
        self.assertIsNotNone(first.joint_search_trial)

        second = scan_next_candidate(
            FakeNativeOracle(),
            template=self.template,
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY_LOW,)),
            required_secondary_ids=frozenset((0x190A, 0x2B06, 0xB613)),
            grace_effect_id=GRACE,
            rarity=5,
            max_seeds=128,
            primary_output_map=self.primary_map,
            joint_start_after_trial=first.joint_search_trial or 0,
        )
        self.assertIsNotNone(second)
        assert second is not None
        self.assertNotEqual(second.seed, first.seed)
        self.assertGreater(second.joint_search_trial or 0, first.joint_search_trial or 0)

    def test_playthrough_one_primary_draw_is_inverted_without_seed_scanning(self) -> None:
        template = make_template(record_type=0x1E82)
        mapping = build_primary_first_draw_output_map(
            FakeFirstDrawOracle(),
            template=template,
            category=1,
            rarity=5,
        )
        self.assertEqual(mapping.bucket_count, 0x10000)
        self.assertEqual(dict(mapping.effects)[PRIMARY_LOW].bucket_count, 0x8000)
        candidate = scan_next_candidate(
            FakeFirstDrawOracle(),
            template=template,
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY_LOW,)),
            required_secondary_ids=frozenset((0x190A, 0x2B06, 0xB613)),
            rarity=5,
            playthrough=1,
            max_seeds=128,
            primary_first_output_map=mapping,
        )
        self.assertIsNotNone(candidate)
        assert candidate is not None
        self.assertEqual(candidate.primary.effect_id, PRIMARY_LOW)
        self.assertEqual(candidate.playthrough, 1)
        self.assertIsNotNone(candidate.joint_search_trial)

    def test_offline_auxiliary_rejection_skips_native_generation(self) -> None:
        template = make_template(record_type=0x1E82)
        mapping = build_primary_first_draw_output_map(
            FakeFirstDrawOracle(),
            template=template,
            category=1,
            rarity=5,
        )

        class CountingOracle(FakeFirstDrawOracle):
            def __init__(self) -> None:
                self.calls = 0

            def generate(
                self, source_records: list[bytes], *, timeout_ms: int = 60_000
            ) -> list[bytes]:
                self.calls += 1
                return super().generate(source_records, timeout_ms=timeout_ms)

        oracle = CountingOracle()
        candidate = scan_next_candidate(
            oracle,
            template=template,
            start_seed=0,
            primary_effect_ids=frozenset((PRIMARY_LOW,)),
            required_secondary_ids=frozenset(),
            rarity=5,
            playthrough=1,
            max_seeds=16,
            primary_first_output_map=mapping,
            auxiliary_criteria=AuxiliarySearchCriteria(
                required_terrain_effect_keys=frozenset((0xFFFF,))
            ),
        )
        self.assertIsNone(candidate)
        self.assertEqual(oracle.calls, 0)

    def test_resigned_save_can_rebind_a_foreign_e604_template_in_memory(self) -> None:
        account = 0x1111222233334444
        foreign = 0x5555666677778888
        save = bytearray(0x9001B0)
        save[:6] = b"RNNUSR"
        save[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = make_template(
            account_id=foreign
        )
        path = __import__("pathlib").Path(
            f"C:/dummy/{account}/SAVEDATA00/SAVEDATA.BIN"
        )
        inventory = SaveInventory.load(path, bytes(save))
        self.assertEqual(struct.unpack_from("<H", inventory.template_record, 0)[0], 0xE604)
        self.assertEqual(account_id_from_record(inventory.template_record), account)
        self.assertEqual(
            account_id_from_record(inventory.template_record_for_playthrough(3)),
            account,
        )
        for playthrough, expected_type in ((4, 0xDD82), (5, 0xD523)):
            synthetic = inventory.template_record_for_playthrough(playthrough)
            self.assertEqual(struct.unpack_from("<H", synthetic, 0)[0], expected_type)
            self.assertEqual(account_id_from_record(synthetic), account)
            self.assertEqual(synthetic[2:], inventory.template_record[2:])
        original = save[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE]
        self.assertEqual(account_id_from_record(original), foreign)


if __name__ == "__main__":
    unittest.main()
