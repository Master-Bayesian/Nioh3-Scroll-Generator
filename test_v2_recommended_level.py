"""Exact displayed-level selection and its read-only worker contract."""

from copy import deepcopy
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest

from jsonschema import Draft7Validator

from nioh3_scroll_editor.catalog_application import recommended_level_metadata, resolve_recommended_level_payload
from nioh3_scroll_editor.worker_transport import read_frame, write_frame


ROOT = Path(__file__).resolve().parent
RESPONSE = json.loads((ROOT / 'packages/contracts/response.schema.json').read_text(encoding='utf-8'))
VALIDATOR = Draft7Validator(RESPONSE)


class RecommendedLevelContractTests(unittest.TestCase):
    def test_catalog_metadata_is_compact_and_explicit_about_prediction(self):
        self.assertEqual(recommended_level_metadata(), {
            'minimum_internal_level': 156, 'maximum_internal_level': 1400,
            'minimum_displayed_level': 142, 'maximum_displayed_level': 700,
            'selection_policy': 'lowest_canonical_internal_level',
            'evidence': 'captured_native_curve_prediction',
        })
        protected = json.loads((ROOT / 'packages/contracts/protected-response.schema.json').read_text(encoding='utf-8'))
        for name in ('SearchCatalog', 'RecommendedLevelMetadata'):
            self.assertEqual(RESPONSE['definitions'][name], protected['definitions'][name])

    def test_schema_rejects_false_exact_matches_and_fallback_for_unavailable_target(self):
        exact = {'protocol': 1, 'id': 'x', 'ok': True, 'result': resolve_recommended_level_payload(350)}
        unavailable = {'protocol': 1, 'id': 'x', 'ok': True, 'result': resolve_recommended_level_payload(0)}
        self.assertTrue(VALIDATOR.is_valid(exact))
        self.assertTrue(VALIDATOR.is_valid(unavailable))
        for response, changes in ((exact, {'canonical_internal_levels': []}),
                                  (exact, {'selected_internal_level': None}),
                                  (unavailable, {'selected_internal_level': 156}),
                                  (unavailable, {'canonical_internal_levels': [156]})):
            bad = deepcopy(response)
            bad['result'].update(changes)
            self.assertFalse(VALIDATOR.is_valid(bad))

    def test_real_worker_resolves_catalog_and_levels_without_starting_a_job(self):
        requests = [
            ('handshake', {}),
            ('search.catalog', {'playthrough': 3, 'rarity': 4, 'locale': 'en-US'}),
            ('recommended_level.resolve', {'displayed_level': 350}),
            ('recommended_level.resolve', {'displayed_level': 141}),
            ('recommended_level.resolve', {'displayed_level': 701}),
            ('recommended_level.resolve', {'displayed_level': 350.0}),
            ('recommended_level.resolve', {'displayed_level': True}),
            ('recommended_level.resolve', {'displayed_level': 350.5}),
            ('recommended_level.resolve', {'displayed_level': '350'}),
            ('recommended_level.resolve', {'displayed_level': 2**40}),
            ('recommended_level.resolve', {'displayed_level': 350, 'clamp': True}),
            ('shutdown', {}),
        ]
        stream = io.BytesIO()
        for index, (method, params) in enumerate(requests):
            write_frame(stream, {'protocol': 1, 'id': str(index), 'method': method, 'params': params})
        completed = subprocess.run([sys.executable, '-m', 'nioh3_scroll_editor.search_worker'],
                                   input=stream.getvalue(), capture_output=True, cwd=ROOT, timeout=30, check=True)
        output = io.BytesIO(completed.stdout)
        responses = [read_frame(output) for _ in requests]
        self.assertIsNone(read_frame(output))
        for index, response in enumerate(responses):
            self.assertEqual(response['id'], str(index))
            self.assertTrue(VALIDATOR.is_valid(response), json.dumps(response))
        self.assertEqual(responses[1]['result']['recommended_level'], recommended_level_metadata())
        self.assertEqual(responses[2]['result']['canonical_internal_levels'], [585, 586])
        self.assertEqual(responses[2]['result']['selected_internal_level'], 585)
        for index in (3, 4):
            self.assertEqual(responses[index]['result']['status'], 'out_of_range')
            self.assertIsNone(responses[index]['result']['selected_internal_level'])
        self.assertEqual(responses[5]['result'], responses[2]['result'])
        for index in range(6, 11):
            self.assertFalse(responses[index]['ok'])
            self.assertEqual(responses[index]['error']['code'], 'INVALID_REQUEST')
        self.assertEqual(responses[-1]['result'], {'stopped': True})


if __name__ == '__main__':
    unittest.main()
