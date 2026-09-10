"""Integration boundaries for the review UI; no live process or user save access."""
import json
import subprocess
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path
from nioh3_scroll_editor.save_application import SaveApplication
from nioh3_scroll_editor.core_services import CandidateApplicationService
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.models import ScrollCandidate
from nioh3_scroll_editor.candidate_transfer import export_candidate, import_candidate
from nioh3_scroll_editor.effect_sequence import materialize_ng3_certified_install_record
from nioh3_scroll_editor.savegame import next_generation_serial
from nioh3_scroll_editor.search_jobs import SearchJobs
from nioh3_scroll_editor.search_application import SearchBatchResult
from nioh3_scroll_editor.worker_contracts import RequestError
from tests.test_frontend_v2 import parameters
from tests.test_cart_batch import FakeApplication
from nioh3_scroll_editor.live_add_batch import LiveAddBatch

class ReviewIntegrationTests(unittest.TestCase):
    def test_auxiliary_preview_tracks_draft_seed_in_all_contexts(self):
        from nioh3_scroll_editor.scroll_input_metadata import initial_challenge_capacity
        with tempfile.TemporaryDirectory() as directory:
            app = SaveApplication(Path(directory))
            for ng in range(1, 6):
                for seed in (36526331, 10030565):
                    preview = json.loads(app.auxiliary_preview(seed, ng)['auxiliary_json'])
                    self.assertEqual(preview['initial_challenge_capacity'], initial_challenge_capacity(seed))
                    self.assertTrue(preview['enemy_groups'])

    def test_data_directory_pointer_does_not_move_active_operations(self):
        import os
        from unittest.mock import patch
        from nioh3_scroll_editor.app_settings import load_app_settings, default_state_root
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {'LOCALAPPDATA': directory, 'NIOH3_SCROLL_DATA_ROOT': ''}):
            # Hosted Windows TEMP may use an 8.3 alias; compare canonical paths.
            root = Path(directory).resolve()
            original = root / 'active'
            app = SaveApplication(original)
            target = root / 'chosen'
            result = app.data_directory('set', str(target))
            self.assertTrue(result['restart_required'])
            self.assertEqual(app.state_root, original)
            self.assertEqual(load_app_settings(fallback_root=root).data_root, target)
            app.data_directory('reset', None)
            self.assertEqual(load_app_settings(fallback_root=root).data_root, default_state_root(fallback_root=root))
            self.assertEqual(app.state_root, original)

    def test_grace_or_filter_advances_rejections_and_binds_resume(self):
        candidates=[ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(seed,rarity=4,level=180)) for seed in (10030565,36526331,43723117)]
        desired=candidates[1].grace.effect_id
        def collector(_request,**kwargs):
            cursor=kwargs['start_after_trial']
            return SearchBatchResult((candidates[cursor],),1,cursor+1)
        jobs=SearchJobs(collector=collector);p=parameters();p.update(result_count=1,page_trials=1,job_trials=3)
        p['query']['grace_effect_ids']=[desired]
        first=jobs.start(p);jobs.thread.join(10);result=jobs.snapshot(first['job_id'])
        self.assertEqual(result['state'],'completed',result['error'])
        self.assertEqual([v['seed'] for v in result['candidates']],[36526331]);self.assertEqual(result['cursor'],2)
        p['resume_token']=result['resume_token'];p['query']['grace_effect_ids']=[]
        with self.assertRaises(RequestError):jobs.start(p)

    def test_group_threshold_accepts_any_qualifying_member(self):
        candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(10030565, rarity=4, level=180))
        members = [e for e in candidate.effects[1:4] if e.roll_percent < 100][:2]
        self.assertEqual(len(members), 2)
        for second_threshold, expected in ((0, 1), (100, 0)):
            p = parameters()
            p.update(result_count=1, page_trials=1, job_trials=1)
            p['query'].update(primary_effect_ids=[], required_secondary_ids=[], required_secondary_id_groups=[[e.effect_id for e in members]], minimum_roll_percent_by_effect_id=[[members[0].effect_id, 100], [members[1].effect_id, second_threshold]], grace_effect_id=None)
            jobs = SearchJobs(collector=lambda *args, **kwargs: SearchBatchResult((candidate,), 1, 1))
            started = jobs.start(p)
            jobs.thread.join(10)
            result = jobs.snapshot(started['job_id'])
            self.assertEqual(result['state'], 'completed', result['error'])
            self.assertEqual(len(result['candidates']), expected)

    def test_early_playthrough_native_route_keeps_category_and_auxiliary(self):
        from unittest.mock import patch
        from nioh3_scroll_editor.runtime_application import RuntimeApplication
        from tests.test_beta_editor import make_record
        import threading
        class Oracle:
            remote_call_pending = False
            def __enter__(self): return self
            def __exit__(self, *_): return False
        for playthrough in (1, 2):
            runtime = RuntimeApplication(identity=lambda: (123, object(), 'fixture'), oracle_factory=lambda **_: Oracle())
            candidate = ScrollCandidate.from_record(make_record(), playthrough=playthrough)
            template = {'context_digest': runtime.service.context.context_digest, 'template_hex': make_record().hex()}
            with patch('nioh3_scroll_editor.runtime_application.scan_next_candidate', return_value=candidate) as scanner:
                result = runtime.search(template, 100, playthrough, 4, 180, 585, {}, 1024, threading.Event(), lambda _: None)
                self.assertEqual(scanner.call_args.kwargs['playthrough'], playthrough)
                self.assertIsNotNone(result['candidate']['auxiliary'])
                self.assertEqual(result['candidate']['playthrough'], playthrough)
                self.assertEqual(runtime.export(result['candidate']['candidate_id'])['playthrough'], playthrough)
            self.assertTrue(runtime.shutdown()['safe_to_shutdown'])

    def test_cancelled_batch_is_never_dispatched(self):
        with tempfile.TemporaryDirectory() as directory:
            app=FakeApplication(directory);batch=LiveAddBatch(app)
            plan=batch.prepare([{'candidate_id':'one'}],Path(directory)/'save.bin')
            self.assertEqual(batch.cancel(plan['batch_id'])['state'],'cancelled')
            with self.assertRaises(FileExistsError):batch.execute(plan['batch_id'],plan['plan_digest'])
            self.assertFalse(app.calls)

    def test_live_materialization_preserves_r4_pair_and_does_not_write(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            fixture=json.loads(subprocess.check_output([sys.executable,'apps/desktop/tests/fixtures/create-save.py',str(root/'fixture')],text=True))
            path=Path(fixture['path']);original=path.read_bytes();service=CandidateApplicationService();app=SaveApplication(root/'state',service=service)
            reference=app.register(str(path));inventory=app.inventory(reference['save_id'])
            candidates=[export_candidate(ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331 if rarity==4 else 10030565,rarity=rarity,level=180)),service.context.context_digest,180) for rarity in (3,4,5)]
            output=app.materialize_live_many(reference['save_id'],inventory['snapshot_id'],candidates,585,0xFFFFFFFF)
            self.assertEqual(path.read_bytes(),original)
            for source,payload in zip(candidates,output['candidates']):
                materialized=import_candidate(payload,service.context.context_digest)
                self.assertEqual(materialized.record_stage.value,'final_record')
                self.assertEqual([(e.effect_id,e.value) for e in materialized.effects if not e.is_empty],[(e['effect_id'],e['value']) for e in source['effects'] if e['effect_id']!=0xFFFFFFFF])
                snapshot_inventory = app._snapshot(reference['save_id'], inventory['snapshot_id'])[1]
                expected, _ = materialize_ng3_certified_install_record(snapshot_inventory.template_record_for_playthrough(3), seed=payload['seed'], rarity=payload['rarity'], level=180, recommended_level=585, transfer_count=0xFFFFFFFF, generation_serial=next_generation_serial(snapshot_inventory))
                self.assertEqual(materialized.installation_record, expected)
                self.assertEqual(int.from_bytes(materialized.installation_record[0xDC:0xE0],'little'),0xFFFFFFFF)

if __name__=='__main__':unittest.main()
