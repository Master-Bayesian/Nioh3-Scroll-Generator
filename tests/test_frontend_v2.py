"""Protocol, lifecycle and real-core regression gates for the V2 foundation."""
from copy import deepcopy
from dataclasses import replace
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import threading
import unittest

from nioh3_scroll_editor.core_services import CandidateApplicationService
from nioh3_scroll_editor.effect_seed_solver import EffectSeedIntersectionReport
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.models import ScrollCandidate, CandidateRecordStage
from nioh3_scroll_editor.search_application import SearchBatchResult
from nioh3_scroll_editor.search_jobs import SearchJobs
from nioh3_scroll_editor.search_worker import read_frame, write_frame
from nioh3_scroll_editor.worker_contracts import MAX_FRAME_BYTES, RequestError, validate_request
from nioh3_scroll_editor.scroll_input_metadata import initial_challenge_capacity


def parameters():
    return {
        'query': {'playthrough': 3, 'rarity': 4, 'level': 180,
                  'primary_effect_ids': [0xAE5A], 'required_secondary_ids': [],
                  'required_secondary_id_groups': [], 'grace_effect_id': None,
                  'minimum_roll_percent_by_effect_id': [],
                  'auxiliary': {key: [] for key in ('required_terrain_effect_keys', 'required_terrain_effect_key_groups',
                      'required_special_rule_keys', 'required_special_rule_key_groups',
                      'required_enemy_lookup_keys', 'required_enemy_lookup_key_groups')}},
        'context_digest': CandidateApplicationService().context.context_digest,
        'result_count': 2, 'page_trials': 100, 'job_trials': 250,
        'allow_cpu_fallback': False, 'resume_token': None,
    }


class ProtocolTests(unittest.TestCase):
    def test_strict_schema_rejects_unknown_fields_boolean_integer_and_unsupported_context(self):
        p = parameters()
        validate_request({'protocol': 1, 'id': '1', 'method': 'search.start', 'params': p})
        for mutate in (lambda p: p.update(result_count=True), lambda p: p.update(exec='bad'),
                       lambda p: p['query'].update(playthrough=1), lambda p: p.update(page_trials=100000001),
                       lambda p: p.update(continue_until_complete='yes')):
            bad = deepcopy(p); mutate(bad)
            with self.assertRaises(RequestError):
                validate_request({'protocol': 1, 'id': '1', 'method': 'search.start', 'params': bad})
            with self.assertRaises(RequestError):
                SearchJobs().start(bad)
        wide = deepcopy(p); wide.update(page_trials=100000000)
        validate_request({'protocol': 1, 'id': '1', 'method': 'search.start', 'params': wide})
        for flag in (True, False):
            flagged = deepcopy(p); flagged.update(continue_until_complete=flag)
            validate_request({'protocol': 1, 'id': '1', 'method': 'search.start', 'params': flagged})

    def test_curse_search_is_rejected_instead_of_treating_unknown_as_a_match(self):
        p = parameters()
        p['query'].update(
            enemy_variant='expedition',
            enemy_occurrence_groups=[[{
                'lookup_keys': [0xDCB98],
                'state': 'curse',
                'availability': 'any',
            }]],
        )
        with self.assertRaisesRegex(ValueError, 'Curse changes between entries'):
            SearchJobs().start(p)

    def test_enemy_state_filters_reject_incompatible_modes(self):
        solo = parameters()
        solo['query']['enemy_occurrence_groups'] = [[{
            'lookup_keys': [0xDCB98],
            'state': 'any',
            'availability': 'expedition_only',
        }]]
        with self.assertRaisesRegex(ValueError, 'require the expedition preview'):
            SearchJobs().start(solo)

        later_playthrough = parameters()
        later_playthrough['query'].update(playthrough=4, enemy_variant='expedition')
        with self.assertRaisesRegex(ValueError, 'only in playthrough 3'):
            SearchJobs().start(later_playthrough)

        for lookup_key in (12014, 451004):
            ineligible = parameters()
            ineligible['query']['enemy_occurrence_groups'] = [[{
                'lookup_keys': [lookup_key],
                'state': 'possessed',
                'availability': 'any',
            }]]
            with self.subTest(lookup_key=lookup_key):
                with self.assertRaisesRegex(ValueError, 'only for eligible low-pool'):
                    SearchJobs().start(ineligible)

    def test_frames_fragmented_utf8_and_limits(self):
        class Fragmented(io.BytesIO):
            def read(self, size=-1): return super().read(min(size, 2))
        stream = io.BytesIO(); write_frame(stream, {'text': '\u4ec1\u738b'})
        self.assertEqual(read_frame(Fragmented(stream.getvalue())), {'text': '\u4ec1\u738b'})
        for data in (b'\x01', struct.pack('<I', 8) + b'{}'):
            with self.assertRaises(EOFError): read_frame(io.BytesIO(data))
        with self.assertRaises(ValueError): read_frame(io.BytesIO(struct.pack('<I', MAX_FRAME_BYTES + 1)))

    def test_worker_import_has_no_tk_or_save_host(self):
        script = "import sys; import nioh3_scroll_editor.search_worker; assert 'tkinter' not in sys.modules; assert 'nioh3_scroll_editor.app' not in sys.modules; assert 'nioh3_scroll_editor.savegame' not in sys.modules"
        subprocess.run([sys.executable, '-c', script], check=True, timeout=15)


class JobTests(unittest.TestCase):
    def test_possessed_expedition_filter_uses_exact_occurrence_replay(self):
        seeds = [156062997, 86872488]
        candidates = [ScrollCandidate.from_effect_sequence(
            generate_ng3_certified_effect_sequence(seed, rarity=4, level=180)
        ) for seed in seeds]
        def collect(_request, **kwargs):
            cursor = kwargs['start_after_trial']
            return SearchBatchResult((candidates[cursor],), 1, cursor + 1)
        jobs = SearchJobs(collector=collect)
        p = parameters()
        p.update(result_count=1, page_trials=1, job_trials=2)
        p['query'].update(
            enemy_variant='expedition',
            enemy_occurrence_groups=[[{
                'lookup_keys': [0xDCB98],
                'state': 'possessed',
                'availability': 'expedition_only',
            }]],
        )
        started = jobs.start(p)
        jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'completed', result['error'])
        self.assertEqual([c['seed'] for c in result['candidates']], [86872488])
        occurrence = next(x for x in result['candidates'][0]['enemy_states']['expedition']['occurrences']
                          if x['native_spawn_key'] == 0xF3F)
        self.assertEqual((occurrence['possessed'], occurrence['availability']),
                         ('yes', 'expedition_only'))

    def test_capacity_filter_advances_rejected_pages_and_exports_only_matches(self):
        seeds = [10030565, 36526331, 43723117]
        self.assertEqual([initial_challenge_capacity(seed) for seed in seeds], [7, 4, 7])
        candidates = [ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(seed, rarity=4, level=180)) for seed in seeds]
        def collect(_request, **kwargs):
            cursor = kwargs['start_after_trial']
            return SearchBatchResult((candidates[cursor],), 1, cursor + 1)
        jobs = SearchJobs(collector=collect); p = parameters()
        p.update(result_count=1, page_trials=1, job_trials=3)
        p['query']['initial_challenge_counts'] = [7]
        first = jobs.start(p); jobs.thread.join(10)
        result = jobs.snapshot(first['job_id'])
        self.assertEqual(result['candidates'][0]['seed'], seeds[0])
        self.assertEqual(result['cursor'], 1)
        p['resume_token'] = result['resume_token']
        resumed = jobs.start(p); jobs.thread.join(10)
        result = jobs.snapshot(resumed['job_id'])
        self.assertEqual(result['cursor'], 3)
        self.assertEqual([c['seed'] for c in result['candidates']], [seeds[2]])
        self.assertEqual(list(jobs.candidate_records.values()), [candidates[2]])
        p['query']['initial_challenge_counts'] = [4]
        with self.assertRaisesRegex(RequestError, 'Resume token'):
            jobs.start(p)

    def test_current_job_is_a_detached_snapshot_and_count_filter_is_strict(self):
        jobs = SearchJobs(collector=lambda *_a, **_kw: SearchBatchResult((), 1, 1))
        self.assertEqual(jobs.current(), {'job': None})
        p = parameters(); p.update(page_trials=1, job_trials=1)
        job = jobs.start(p); jobs.thread.join(10)
        recovered = jobs.current()['job']; recovered['candidates'].append({'bad': True})
        self.assertEqual(jobs.snapshot(job['job_id'])['candidates'], [])
        for values in ([8], [3], [True], [7, 7]):
            p['query']['initial_challenge_counts'] = values
            with self.assertRaises(RequestError): jobs.start(p)

    def test_budget_continues_pages_and_resume_binds_policy_query_context_session(self):
        calls = []
        def collect(_request, **kwargs):
            calls.append(kwargs['max_trials_per_batch'])
            return SearchBatchResult((), 2, kwargs['start_after_trial'] + kwargs['max_trials_per_batch'])
        jobs = SearchJobs(collector=collect); p = parameters()
        started = jobs.start(p); jobs.thread.join(10); result = jobs.snapshot(started['job_id'])
        self.assertEqual(calls, [100, 100, 50]); self.assertEqual(result['stop_reason'], 'budget_reached')
        self.assertEqual(result['cursor'], 250)
        p['resume_token'] = result['resume_token']
        started = jobs.start(p); jobs.thread.join(10)
        self.assertEqual(jobs.snapshot(started['job_id'])['cursor'], 500)
        for change in ('policy', 'query', 'context', 'session', 'token', 'continuation'):
            bad = deepcopy(p); target = jobs
            if change == 'policy': bad['allow_cpu_fallback'] = True
            if change == 'query': bad['query']['level'] = 170
            if change == 'context': bad['context_digest'] = '0' * 64
            if change == 'session': target = SearchJobs(collector=collect)
            if change == 'token': bad['resume_token'] += 'bad'
            if change == 'continuation': bad['continue_until_complete'] = True
            with self.assertRaises(RequestError): target.start(bad)

    def test_continuation_reaches_a_match_past_the_bounded_budget_in_one_job(self):
        target = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(226061463, rarity=4, level=180))
        calls = []
        def collect(_request, **kwargs):
            calls.append(kwargs['max_trials_per_batch'])
            start, budget = kwargs['start_after_trial'], kwargs['max_trials_per_batch']
            if start >= 300:
                return SearchBatchResult((target,), 1, 350)
            return SearchBatchResult((), 1, start + budget)
        jobs = SearchJobs(collector=collect); p = parameters()
        p.update(result_count=1, page_trials=100, job_trials=250)
        bounded = jobs.start(p); jobs.thread.join(10)
        bounded_result = jobs.snapshot(bounded['job_id'])
        self.assertEqual(bounded_result['stop_reason'], 'budget_reached')
        self.assertEqual((bounded_result['cursor'], bounded_result['candidates']), (250, []))
        p.update(continue_until_complete=True, resume_token=bounded_result['resume_token'])
        with self.assertRaisesRegex(RequestError, 'Resume token'):
            jobs.start(p)
        p['resume_token'] = None
        started = jobs.start(p); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'completed', result['error'])
        self.assertEqual(result['stop_reason'], 'result_limit')
        self.assertEqual([c['seed'] for c in result['candidates']], [226061463])
        self.assertEqual(result['cursor'], 350)
        self.assertEqual(calls, [100, 100, 50, 100, 100, 100, 100])

    def test_continuation_accumulates_candidates_past_the_old_job_stop(self):
        seeds = (226061463, 36526331)
        targets = [ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(seed, rarity=4, level=180))
                   for seed in seeds]
        budgets = []
        def collect(_request, **kwargs):
            budgets.append(kwargs['max_trials_per_batch'])
            start = kwargs['start_after_trial']
            if start == 0:
                return SearchBatchResult((), 1, 100)
            if start == 100:
                return SearchBatchResult((targets[0],), 1, 150)
            return SearchBatchResult((targets[1],), 1, 200)
        jobs = SearchJobs(collector=collect); p = parameters()
        p.update(result_count=2, page_trials=100, job_trials=50, continue_until_complete=True)
        started = jobs.start(p); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'completed', result['error'])
        self.assertEqual(result['stop_reason'], 'result_limit')
        self.assertEqual([c['seed'] for c in result['candidates']], list(seeds))
        self.assertEqual((result['cursor'], budgets), (200, [100, 100, 100]))
        self.assertEqual(sorted(record.seed for record in jobs.candidate_records.values()), sorted(seeds))
        for payload in result['candidates']:
            exported = jobs.export(started['job_id'], payload['candidate_id'])
            self.assertEqual((exported['seed'], exported['candidate_id']),
                             (payload['seed'], payload['candidate_id']))

    def test_continuation_reports_family_exhaustion_without_candidates(self):
        report = EffectSeedIntersectionReport(0, 50, 50, 0, (), 0, True)
        def collect(_request, **kwargs):
            stop = min(50, kwargs['start_after_trial'] + kwargs['max_trials_per_batch'])
            return SearchBatchResult((), 2, stop, report)
        jobs = SearchJobs(collector=collect); p = parameters()
        p.update(page_trials=100, job_trials=1000000, continue_until_complete=True)
        started = jobs.start(p); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'completed', result['error'])
        self.assertEqual(result['stop_reason'], 'family_exhausted')
        self.assertEqual(result['cursor'], 50)
        self.assertIsNone(result['resume_token'])

    def test_continuation_cancels_between_pages_and_resumes_under_the_same_policy(self):
        target = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331, rarity=4, level=180))
        entered, release = threading.Event(), threading.Event()
        pages = []
        def collect(_request, **kwargs):
            start = kwargs['start_after_trial']
            pages.append(start)
            if start == 10:
                entered.set(); release.wait(5)
                return SearchBatchResult((), 1, 20)
            if start == 20:
                return SearchBatchResult((target,), 1, 30)
            return SearchBatchResult((), 1, start + kwargs['max_trials_per_batch'])
        jobs = SearchJobs(collector=collect); p = parameters()
        p.update(result_count=1, page_trials=10, job_trials=1000, continue_until_complete=True)
        started = jobs.start(p)
        try:
            self.assertTrue(entered.wait(5))
            self.assertEqual(jobs.cancel(started['job_id'])['state'], 'cancel_requested')
        finally:
            release.set(); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'cancelled')
        self.assertEqual(result['stop_reason'], 'cancelled')
        self.assertEqual(result['cursor'], 20)
        self.assertIsNotNone(result['resume_token'])
        p['resume_token'] = result['resume_token']
        with self.assertRaisesRegex(RequestError, 'Resume token'):
            jobs.start(dict(p, continue_until_complete=False))
        resumed = jobs.start(p); jobs.thread.join(10)
        resumed_result = jobs.snapshot(resumed['job_id'])
        self.assertEqual(resumed_result['state'], 'completed', resumed_result['error'])
        self.assertEqual(resumed_result['stop_reason'], 'result_limit')
        self.assertEqual([c['seed'] for c in resumed_result['candidates']], [36526331])
        self.assertEqual(resumed_result['cursor'], 30)
        self.assertEqual(pages, [0, 10, 20])

    def test_continuation_fails_closed_on_invalid_checkpoint_overflow_and_no_progress(self):
        candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331, rarity=4, level=180))
        for page, code in ((SearchBatchResult((), 1, 101), 'INVALID_CHECKPOINT'),
                           (SearchBatchResult((), 1, 0), 'NO_PROGRESS'),
                           (SearchBatchResult((candidate,) * 3, 1, 1), 'RESULT_OVERFLOW')):
            with self.subTest(code=code):
                jobs = SearchJobs(collector=lambda *_a, **_kw: page)
                p = parameters(); p.update(result_count=1, page_trials=100, job_trials=10, continue_until_complete=True)
                started = jobs.start(p); jobs.thread.join(10)
                result = jobs.snapshot(started['job_id'])
                self.assertEqual(result['state'], 'failed')
                self.assertEqual(result['error']['code'], code)
                self.assertEqual(result['cursor'], 0)
                self.assertEqual(result['candidates'], [])
                self.assertIsNone(result['resume_token'])

    def test_cancel_is_acknowledged_before_safe_stop_and_busy_is_enforced(self):
        entered, release = threading.Event(), threading.Event()
        def collect(_request, **kwargs):
            entered.set(); release.wait(5)
            return SearchBatchResult((), 2, kwargs['start_after_trial'])
        jobs = SearchJobs(collector=collect); p = parameters()
        started = jobs.start(p); self.assertTrue(entered.wait(5))
        try:
            with self.assertRaises(RequestError): jobs.start(p)
            self.assertEqual(jobs.cancel(started['job_id'])['state'], 'cancel_requested')
        finally: release.set(); jobs.thread.join(10)
        snapshot = jobs.snapshot(started['job_id'])
        self.assertEqual(snapshot['state'], 'cancelled'); self.assertEqual(snapshot['cursor'], 0)
        self.assertEqual(jobs.cancel(started['job_id']), snapshot)
        with self.assertRaises(RequestError): jobs.snapshot('stale')

    def test_fault_does_not_publish_a_resume_checkpoint(self):
        def collect(*_args, **_kwargs): raise RuntimeError('injected accelerator failure')
        jobs = SearchJobs(collector=collect)
        started = jobs.start(parameters()); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'failed'); self.assertIsNone(result['resume_token'])

    def test_stage_one_is_rejected_at_shared_job_boundary(self):
        candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331, rarity=4, level=180))
        candidate = replace(candidate, record_stage=CandidateRecordStage.NATIVE_STAGE_ONE)
        jobs = SearchJobs(collector=lambda *_a, **_kw: SearchBatchResult((candidate,), 2, 1))
        started = jobs.start(parameters()); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['state'], 'failed'); self.assertEqual(result['candidates'], [])

    def test_exhaustion_is_distinct_from_budget(self):
        report = EffectSeedIntersectionReport(0, 20, 20, 0, (), 0, True)
        jobs = SearchJobs(collector=lambda *_a, **_kw: SearchBatchResult((), 2, 20, report))
        started = jobs.start(parameters()); jobs.thread.join(10)
        result = jobs.snapshot(started['job_id'])
        self.assertEqual(result['stop_reason'], 'family_exhausted'); self.assertIsNone(result['resume_token'])

    def test_solver_contract_violations_fail_without_advancing_checkpoint(self):
        candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331, rarity=4, level=180))
        for page, code in ((SearchBatchResult((), 2, 0), 'NO_PROGRESS'),
                           (SearchBatchResult((candidate,) * 3, 2, 1), 'RESULT_OVERFLOW'),
                           (SearchBatchResult((), 2, 1001), 'INVALID_CHECKPOINT')):
            with self.subTest(code=code):
                jobs = SearchJobs(collector=lambda *_a, **_kw: page)
                started = jobs.start(parameters()); jobs.thread.join(10)
                result = jobs.snapshot(started['job_id'])
                self.assertEqual(result['error']['code'], code)
                self.assertEqual(result['cursor'], 0)
                self.assertEqual(result['candidates'], [])
                self.assertIsNone(result['resume_token'])

    def test_real_search_resume_matches_uninterrupted_replay(self):
        jobs = SearchJobs(); p = parameters()
        p.update(page_trials=100000, job_trials=1000000, allow_cpu_fallback=True)
        def run(params):
            started = jobs.start(params); jobs.thread.join(30)
            result = jobs.snapshot(started['job_id'])
            self.assertEqual(result['state'], 'completed', result['error'])
            return result
        first = run(p)
        second = run(dict(p, resume_token=first['resume_token']))
        together = run(dict(p, result_count=4))
        self.assertEqual([c['candidate_id'] for c in first['candidates'] + second['candidates']],
                         [c['candidate_id'] for c in together['candidates']])
        for candidate in together['candidates']:
            replay = generate_ng3_certified_effect_sequence(candidate['seed'], rarity=4, level=180)
            self.assertEqual(candidate['effects'][0]['effect_id'], replay.primary.effect_id)
            self.assertEqual(candidate['effects'][0]['effect_id'], 0xAE5A)


if __name__ == '__main__': unittest.main()
