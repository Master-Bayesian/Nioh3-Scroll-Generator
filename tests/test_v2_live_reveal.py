"""Replay sanitized live evidence without CE, a game process or a user save."""
import json
from pathlib import Path
import unittest

from nioh3_scroll_editor.r4_finalizer_engine import load_default_r4_finalizer_engine
from nioh3_scroll_editor.savegame import read_local_scroll_header
from nioh3_scroll_editor.scroll_input_metadata import record_input_metadata


class LiveRevealRegressionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = json.loads((Path(__file__).resolve().parents[1] / 'test_fixtures/live_first_reveal_pc_v201.json').read_text(encoding='utf-8'))

    def test_revealed_and_saved_effects_match_frozen_prediction(self):
        engine = load_default_r4_finalizer_engine()
        for vector in self.fixture['records']:
            with self.subTest(seed=vector['seed']):
                before, revealed, saved = [bytes.fromhex(vector[key]) for key in ('before_hex', 'revealed_hex', 'saved_hex')]
                expected = engine.finalize_completion(before, reveal=True).record if vector['rarity'] == 4 else before
                self.assertEqual(expected[0x34:0xDC], revealed[0x34:0xDC])
                self.assertEqual(revealed[0x34:0xDC], saved[0x34:0xDC])
                self.assertEqual(record_input_metadata(saved, read_local_scroll_header(saved)), {
                    'initial_challenge_capacity': vector['initial_capacity'],
                    'remaining_challenge_attempts': vector['remaining_after'],
                    'recommended_displayed_level': vector['recommended_displayed_level'],
                    'recommended_raw_was_clamped': False})
                for record in (before, revealed, saved):
                    for region in self.fixture['sanitized_ranges']:
                        self.assertEqual(record[region['offset']:region['offset'] + region['length']], bytes(region['length']))

    def test_all_three_unaccepted_completion_candidates_match_232_captured_bytes(self):
        vector = self.fixture['records'][2]
        engine = load_default_r4_finalizer_engine()
        before = bytes.fromhex(vector['before_hex'])
        completion = engine.finalize_completion(before, reveal=True)
        self.assertIsNone(completion.accepted_index)
        self.assertEqual(completion.record, before)
        self.assertEqual([v['effect_index'] for v in vector['completion_candidates']], [1, 3, 4])
        for candidate in vector['completion_candidates']:
            actual, trace = engine.build_completion_candidate(bytes.fromhex(candidate['source_hex']), candidate['effect_index'], reveal=True)
            self.assertEqual(actual, bytes.fromhex(candidate['candidate_hex']))


if __name__ == '__main__':
    unittest.main()
