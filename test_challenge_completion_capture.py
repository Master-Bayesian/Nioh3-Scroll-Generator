from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parent
CAPTURE = (
    ROOT
    / "test_fixtures"
    / "challenge_completion_pc_v20002.json"
)


class ChallengeCompletionCaptureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.capture = json.loads(CAPTURE.read_text(encoding="utf-8"))

    def test_controlled_record_transition_is_exact(self) -> None:
        before = bytes.fromhex(self.capture["pre_record"]["record_hex"])
        after = bytes.fromhex(self.capture["post_record"]["record_hex"])
        self.assertEqual(len(before), 0xE8)
        self.assertEqual(len(after), 0xE8)
        actual_offsets = [
            index for index, pair in enumerate(zip(before, after)) if pair[0] != pair[1]
        ]
        expected_offsets = [
            int(item["offset"], 0) for item in self.capture["byte_differences"]
        ]
        self.assertEqual(actual_offsets, expected_offsets)

    def test_public_fixture_has_no_instance_identity(self) -> None:
        for phase in ('pre_record', 'post_record'):
            record = bytes.fromhex(self.capture[phase]['record_hex'])
            for offset, length in self.capture['sanitized_ranges']:
                self.assertEqual(record[offset:offset + length], bytes(length))
        self.assertNotIn('screenshots', self.capture)
        self.assertNotIn('inventory_slot_index', self.capture)

    def test_completion_candidates_cover_each_non_primary_slot(self) -> None:
        vector = self.capture["displayed_candidates"]
        self.assertEqual([item["target_slot"] for item in vector], [2, 3, 4, 5])
        self.assertEqual(
            [item["candidate_effect_id"] for item in vector[:3]],
            ["0xDAC2", "0xDFF0", "0x6CE3"],
        )
        self.assertIsNone(vector[3]["candidate_effect_id"])
        self.assertEqual(vector[3]["candidate_group_key"], "0x3194")
        self.assertIn("not captured", vector[3]["candidate_id_status"])

    def test_only_selected_effect_id_changed(self) -> None:
        before = self.capture["pre_record"]["effects"]
        after = self.capture["post_record"]["effects"]
        changed_ids = [
            old["slot"]
            for old, new in zip(before, after)
            if old["effect_id"] != new["effect_id"]
        ]
        self.assertEqual(changed_ids, [2])
        self.assertEqual(after[1]["effect_id"], "0xDAC2")
        self.assertEqual(after[1]["value"], 150)
        self.assertEqual(self.capture["pre_record"]["counter"], 1)
        self.assertEqual(self.capture["post_record"]["counter"], 2)


if __name__ == "__main__":
    unittest.main()
