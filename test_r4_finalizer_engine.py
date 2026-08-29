from __future__ import annotations

import json
from pathlib import Path
import unittest

from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity4_stage_one_effect_sequence,
    materialize_ng3_rarity4_final_record,
    materialize_ng3_rarity4_stage_one_record,
    serialize_ng3_rarity4_stage_one_effect_slots,
)
from nioh3_scroll_editor.r4_finalizer_engine import R4FinalizerEngine


ROOT = Path(__file__).resolve().parent
CORPUS_ROOTS = (
    ROOT / "test_fixtures" / "r4_native_corpus" / "base",
    ROOT / "test_fixtures" / "r4_native_corpus" / "distributed",
)
CONTROLLED = json.loads(
    (
        ROOT
        / "research"
        / "test_vectors"
        / "r4_seed_183696634_records_sanitized.json"
    ).read_text(encoding="utf-8")
)


class R4FinalizerEngineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.engine = R4FinalizerEngine()

    def test_all_native_corpus_records_match_exactly(self) -> None:
        checked = 0
        for corpus_root in CORPUS_ROOTS:
            for stage_path in sorted(corpus_root.glob("*_stage.bin")):
                final_path = Path(str(stage_path).replace("_stage.bin", "_final.bin"))
                result = self.engine.finalize_completion(stage_path.read_bytes())
                self.assertEqual(
                    result.record,
                    final_path.read_bytes(),
                    stage_path.name,
                )
                checked += 1
        self.assertEqual(checked, 10)

    def test_offline_stage_one_matches_all_native_corpus_effect_areas(self) -> None:
        checked = 0
        for corpus_root in CORPUS_ROOTS:
            for stage_path in sorted(corpus_root.glob("*_stage.bin")):
                stage = stage_path.read_bytes()
                seed = int.from_bytes(stage[0x20:0x24], "little")
                level = int.from_bytes(stage[0x06:0x08], "little")
                result = generate_ng3_rarity4_stage_one_effect_sequence(
                    seed,
                    level=level,
                )
                self.assertEqual(
                    serialize_ng3_rarity4_stage_one_effect_slots(result),
                    stage[0x34:0xDC],
                    stage_path.name,
                )
                checked += 1
        self.assertEqual(checked, 10)

    def test_materialized_stage_and_final_match_all_native_corpus_records(self) -> None:
        checked = 0
        for corpus_root in CORPUS_ROOTS:
            for stage_path in sorted(corpus_root.glob("*_stage.bin")):
                final_path = Path(str(stage_path).replace("_stage.bin", "_final.bin"))
                stage = stage_path.read_bytes()
                fields = {
                    "seed": int.from_bytes(stage[0x20:0x24], "little"),
                    "level": int.from_bytes(stage[0x06:0x08], "little"),
                    "recommended_level": int.from_bytes(stage[0x10:0x12], "little"),
                    "generation_serial": int.from_bytes(stage[0x28:0x2C], "little"),
                    "transfer_count": int.from_bytes(stage[0xDC:0xE0], "little"),
                }
                rebuilt_stage, _ = materialize_ng3_rarity4_stage_one_record(
                    stage,
                    **fields,
                )
                rebuilt_final, _ = materialize_ng3_rarity4_final_record(
                    stage,
                    **fields,
                )
                self.assertEqual(rebuilt_stage, stage, stage_path.name)
                self.assertEqual(rebuilt_final, final_path.read_bytes(), stage_path.name)
                checked += 1
        self.assertEqual(checked, 10)

    def test_distributed_corpus_acceptance_indexes_match(self) -> None:
        expected = (4, 3, 3, 2, None, 1, 2, 1)
        root = CORPUS_ROOTS[1]
        actual = tuple(
            self.engine.finalize_completion(path.read_bytes()).accepted_index
            for path in sorted(root.glob("*_stage.bin"))
        )
        self.assertEqual(actual, expected)

    def test_controlled_vector_matches_stable_effect_area(self) -> None:
        stage = bytes.fromhex(CONTROLLED["stage_one_record_hex"])
        expected = bytes.fromhex(CONTROLLED["final_record_hex"])
        result = self.engine.finalize_completion(stage)
        self.assertEqual(result.accepted_index, 4)
        self.assertEqual(result.record[0x34:0xDC], expected[0x34:0xDC])
        self.assertEqual(
            [index for index in range(0xE8) if result.record[index] != expected[index]],
            [0x18, 0x1B, 0x33],
        )

    def test_wrapper_generated_prior_rows_are_required_for_seed_one(self) -> None:
        stage = (CORPUS_ROOTS[1] / "sample_01_seed_1_stage.bin").read_bytes()
        direct, direct_trace = self.engine.finalize_effect(stage, 4)
        wrapped, wrapped_trace = self.engine.build_completion_candidate(stage, 4)
        self.assertNotEqual(direct_trace.selected_effect_id, wrapped_trace.selected_effect_id)
        self.assertEqual(direct_trace.selected_effect_id, 0x2B06)
        self.assertEqual(wrapped_trace.selected_effect_id, 0x6AAF)
        self.assertNotEqual(direct, wrapped)

    def test_unsupported_contexts_fail_closed(self) -> None:
        stage = bytearray(
            (CORPUS_ROOTS[1] / "sample_01_seed_1_stage.bin").read_bytes()
        )
        stage[0x30] = 3
        with self.assertRaisesRegex(ValueError, "rarity 4"):
            self.engine.finalize_completion(bytes(stage))


if __name__ == "__main__":
    unittest.main()
