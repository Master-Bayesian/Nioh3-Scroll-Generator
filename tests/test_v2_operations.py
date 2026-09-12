"""V2 orchestration tests use isolated synthetic saves and injected runtime owners."""
from copy import deepcopy
from dataclasses import replace
from pathlib import Path
import tempfile
import json
import threading
import unittest
from unittest.mock import patch

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE
from tests.test_beta_editor import make_record, TEST_ACCOUNT_ID, SCROLL_GROUP_OFFSET
from nioh3_scroll_editor.save_application import SaveApplication
from nioh3_scroll_editor.protected_jobs import ProtectedJobs
from nioh3_scroll_editor.runtime_application import RuntimeApplication
from nioh3_scroll_editor.candidate_transfer import export_candidate, import_candidate
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.models import ScrollCandidate
from nioh3_scroll_editor.protected_worker import RESPONSE_VALIDATOR
from nioh3_scroll_editor.search_jobs import SearchJobs
from nioh3_scroll_editor.grace_map import load_grace_output_map, grace_map_to_cache_payload
from tests.test_frontend_v2 import parameters


class FixtureCrypto:
    @staticmethod
    def decrypt(source, output):
        data = source.read_bytes()
        assert data.startswith(b'ENC')
        output.write_bytes(data[3:])

    @staticmethod
    def encrypt(source, output):
        output.write_bytes(b'ENC' + source.read_bytes())


class SaveOperationsTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = Path(self.directory.name)
        folder = root / str(TEST_ACCOUNT_ID) / 'SAVEDATA00'
        folder.mkdir(parents=True)
        self.path = folder / 'SAVEDATA.BIN'
        data = bytearray(USER_SAVE_SIZE)
        data[:6] = b'RNNUSR'
        data[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE] = make_record(seed=101, account_id=TEST_ACCOUNT_ID)
        data[SCROLL_GROUP_OFFSET:SCROLL_GROUP_OFFSET + 2] = b'\x04\xe6'
        self.path.write_bytes(b'ENC' + data)
        self.state = root / 'state'
        self.application = SaveApplication(self.state, crypto=FixtureCrypto())
        self.save_id = self.application.register(str(self.path))['save_id']
        self.snapshot = self.application.inventory(self.save_id)

    def plan(self):
        edit = deepcopy(self.snapshot['entries'][0])
        edit['header']['seed'] = 202
        return self.application.prepare_edit(self.save_id, self.snapshot['snapshot_id'], [edit])

    def test_preview_commit_and_durable_idempotency(self):
        original = self.path.read_bytes()
        plan = self.plan()
        self.assertEqual(original, self.path.read_bytes())
        result = self.application.commit(plan['plan_id'])
        self.assertEqual('committed', result['commit_status'])
        self.assertNotEqual(original, self.path.read_bytes())
        self.assertEqual(202, self.application.inventory(self.save_id)['entries'][0]['header']['seed'])
        fresh = SaveApplication(self.state, crypto=FixtureCrypto())
        self.assertEqual(result, fresh.commit(plan['plan_id']))
        self.assertEqual(1, len(self.application.backups(self.save_id)['backups']))
        for value in [self.snapshot, plan, result, self.application.backups(self.save_id)]:
            response = {'protocol': 1, 'id': 'test', 'ok': True, 'result': {
                'job_id': '0' * 32, 'kind': 'save.inventory', 'state': 'completed', 'sequence': 1,
                'cancellable': False, 'progress': None, 'result': value, 'error': None}}
            RESPONSE_VALIDATOR.validate(response)

    def test_r4_transfer_materializes_installation_payload(self):
        candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(36526331, rarity=4, level=180))
        digest = self.application.service.context.context_digest
        wire = export_candidate(candidate, digest, 180)
        self.assertEqual(candidate.effects, import_candidate(wire, digest).effects)
        plan = self.application.prepare_install(self.save_id, self.snapshot['snapshot_id'], wire, 183, 4294967295)
        record, transfer = self.application.plans[plan['plan_id']]['data']
        self.assertEqual(4294967295, transfer)
        self.assertEqual(SCROLL_RECORD_SIZE, len(record))
        self.assertEqual('committed', self.application.commit(plan['plan_id'])['commit_status'])
        wire['seed'] += 1
        with self.assertRaisesRegex(ValueError, 'identity'):
            import_candidate(wire, digest)

    def test_external_change_rejects_commit_without_overwrite(self):
        self.application.game_process_ids = lambda: (1234,)
        plan = self.plan()
        changed = self.path.read_bytes() + b'changed'
        self.path.write_bytes(changed)
        with self.assertRaisesRegex(RuntimeError, 'changed'):
            self.application.commit(plan['plan_id'])
        self.assertEqual(changed, self.path.read_bytes())

    def test_running_game_allows_title_edit_with_backup_and_no_replay(self):
        original = self.path.read_bytes()
        plan = self.plan()
        self.application.game_process_ids = lambda: (1234,)
        result = self.application.commit(plan['plan_id'])
        self.assertEqual('committed', result['commit_status'])
        self.assertEqual(202, self.application.inventory(self.save_id)['entries'][0]['header']['seed'])
        backups = self.application.backups(self.save_id)['backups']
        self.assertEqual(1, len(backups))
        self.assertEqual(original, (self.state / 'backups' / backups[0]['backup_id'] / 'SAVEDATA.BIN').read_bytes())
        fresh = SaveApplication(self.state, crypto=FixtureCrypto(), game_process_ids=lambda: (1234,))
        self.assertEqual(result, fresh.commit(plan['plan_id']))
        self.assertEqual(1, len(self.application.backups(self.save_id)['backups']))

    def test_title_edit_preview_is_read_only_while_game_is_running(self):
        original = self.path.read_bytes()
        self.application.game_process_ids = lambda: (1234,)
        plan = self.plan()
        self.assertEqual('edit', plan['kind'])
        self.assertEqual(original, self.path.read_bytes())
        self.assertFalse(self.application._ledger_path(plan['plan_id']).exists())
        self.assertEqual([], self.application.backups(self.save_id)['backups'])

    def test_title_delete_preserves_backup_while_game_is_running(self):
        original = self.path.read_bytes()
        self.application.game_process_ids = lambda: (1234,)
        plan = self.application.prepare_delete(self.save_id, self.snapshot['snapshot_id'], [0])
        self.assertEqual(original, self.path.read_bytes())
        self.assertEqual('committed', self.application.commit(plan['plan_id'])['commit_status'])
        self.assertEqual([], self.application.inventory(self.save_id)['entries'])
        backups = self.application.backups(self.save_id)['backups']
        self.assertEqual(1, len(backups))
        self.assertEqual(original, (self.state / 'backups' / backups[0]['backup_id'] / 'SAVEDATA.BIN').read_bytes())

    def test_running_game_allows_title_screen_install_plan_and_commit(self):
        self.application.game_process_ids = lambda: (1234,)
        candidate = ScrollCandidate.from_effect_sequence(
            generate_ng3_certified_effect_sequence(
                36526331,
                rarity=4,
                level=180,
            )
        )
        wire = export_candidate(
            candidate,
            self.application.service.context.context_digest,
            180,
        )
        plan = self.application.prepare_install(
            self.save_id,
            self.snapshot['snapshot_id'],
            wire,
            183,
            0xFFFFFFFF,
        )
        self.assertEqual('install', plan['kind'])
        self.assertEqual(
            'committed',
            self.application.commit(plan['plan_id'])['commit_status'],
        )

    def test_delete_then_restore(self):
        original = self.path.read_bytes()
        plan = self.application.prepare_delete(self.save_id, self.snapshot['snapshot_id'], [0])
        self.assertEqual('committed', self.application.commit(plan['plan_id'])['commit_status'])
        snapshot = self.application.inventory(self.save_id)
        self.assertEqual([], snapshot['entries'])
        backup = self.application.backups(self.save_id)['backups'][0]
        restore = self.application.prepare_restore(self.save_id, snapshot['snapshot_id'], backup['backup_id'])
        self.assertEqual('committed', self.application.commit(restore['plan_id'])['commit_status'])
        self.assertEqual(original, self.path.read_bytes())

    def test_running_game_allows_restore_with_checkpoint_and_no_replay(self):
        original = self.path.read_bytes()
        self.assertEqual('committed', self.application.commit(self.plan()['plan_id'])['commit_status'])
        edited = self.path.read_bytes()
        snapshot = self.application.inventory(self.save_id)
        backup = self.application.backups(self.save_id)['backups'][0]
        self.application.game_process_ids = lambda: (1234,)

        plan = self.application.prepare_restore(self.save_id, snapshot['snapshot_id'], backup['backup_id'])
        self.assertEqual(edited, self.path.read_bytes())
        result = self.application.commit(plan['plan_id'])
        self.assertEqual('committed', result['commit_status'])
        self.assertEqual(original, self.path.read_bytes())
        checkpoint = Path(result['details']['checkpoint_directory'])
        self.assertEqual(edited, (checkpoint / 'SAVEDATA.BIN').read_bytes())

        backup_count = len(self.application.backups(self.save_id)['backups'])
        fresh = SaveApplication(self.state, crypto=FixtureCrypto(), game_process_ids=lambda: (1234,))
        self.assertEqual(result, fresh.commit(plan['plan_id']))
        self.assertEqual(backup_count, len(self.application.backups(self.save_id)['backups']))

    def test_interrupted_receipt_never_replays(self):
        plan = self.plan()
        self.application._write_receipt(plan['plan_id'], {'operation_id': plan['plan_id'], 'commit_status': 'executing'})
        original = self.path.read_bytes()
        self.assertEqual('unknown', self.application.commit(plan['plan_id'])['commit_status'])
        self.assertEqual(original, self.path.read_bytes())

    def test_restore_exception_is_conservative_for_sibling_changes(self):
        plan = self.application._plan(self.save_id, self.snapshot['source_sha256'], 'restore', self.state, {})
        with patch('nioh3_scroll_editor.savegame.SaveInstaller.restore_backup', side_effect=OSError('partial restore')):
            self.assertEqual('unknown', self.application.commit(plan['plan_id'])['commit_status'])


class ProtectedOwnershipTests(unittest.TestCase):
    def test_terminal_job_waits_for_owner_exit_before_next_request(self):
        original_thread = threading.Thread
        for failed in (False, True):
            with self.subTest(failed=failed):
                jobs = ProtectedJobs()
                terminal, release, returned = threading.Event(), threading.Event(), threading.Event()
                outcomes, errors = [], []

                class DelayedExitThread(original_thread):
                    def run(self):
                        super().run()
                        if self.name == 'protected-save.commit':
                            terminal.set()
                            release.wait(5)

                def first_action(*_):
                    if failed:
                        raise ValueError('Expected operation failure')
                    return {'committed': True}

                def next_request():
                    try:
                        outcomes.append(jobs.start('save.backups', lambda *_: {'backups': []}))
                    except Exception as error:
                        errors.append(str(error))
                    finally:
                        returned.set()

                with patch('nioh3_scroll_editor.protected_jobs.threading.Thread', DelayedExitThread):
                    first = jobs.start('save.commit', first_action)
                    caller = original_thread(target=next_request)
                    try:
                        self.assertTrue(terminal.wait(5))
                        snapshot = jobs.snapshot(first['job_id'])
                        self.assertEqual('failed' if failed else 'completed', snapshot['state'])
                        self.assertEqual(1, snapshot['sequence'])
                        caller.start()
                        finished = returned.wait(0.1)
                        self.assertEqual([], errors)
                        self.assertFalse(finished, 'A terminal owner must be joined before replying to the next request')
                    finally:
                        release.set()
                        if caller.ident is not None:
                            caller.join(5)
                        jobs.join()
                self.assertFalse(caller.is_alive())
                self.assertEqual([], errors)
                self.assertEqual(1, len(outcomes))
                self.assertNotEqual(first['job_id'], outcomes[0]['job_id'])
                self.assertEqual({'backups': []}, jobs.current()['job']['result'])

    def test_current_protected_job_is_detached_and_tracks_cancellation(self):
        jobs = ProtectedJobs(); entered, release = threading.Event(), threading.Event()
        def action(cancel, progress):
            entered.set(); release.wait(5)
            return {'stopped': cancel.is_set()}
        self.assertEqual(jobs.current(), {'job': None})
        job = jobs.start('runtime.search', action, cancellable=True)
        self.assertTrue(entered.wait(5))
        try:
            current = jobs.current()['job']; current['kind'] = 'modified'
            self.assertEqual(jobs.snapshot(job['job_id'])['kind'], 'runtime.search')
            jobs.cancel(job['job_id'])
            self.assertEqual(jobs.current()['job']['state'], 'cancel_requested')
        finally:
            release.set(); jobs.join()
        self.assertEqual(jobs.current()['job']['result'], {'stopped': True})

    def test_cached_ng4_search_requires_matching_map_and_context(self):
        jobs = SearchJobs()
        params = parameters()
        params['query'].update(playthrough=4, rarity=5)
        params.update(page_trials=1, job_trials=1, allow_cpu_fallback=True)
        with self.assertRaisesRegex(ValueError, 'save-bound'):
            jobs.start(params)
        # Synthetic partition checks routing only; it is not a measured NG4 map.
        mapping = replace(load_grace_output_map(rarity=5), record_type=0xDD82, playthrough='category-4-test')
        payload = grace_map_to_cache_payload(mapping, context_fingerprint='a' * 64,
                                             generation_context_digest=jobs.service.context.context_digest)
        params['cache_id'] = jobs.register_cache(json.dumps(payload))['cache_id']
        job = jobs.start(params)
        jobs.thread.join(15)
        self.assertEqual('completed', jobs.snapshot(job['job_id'])['state'])
        params['query']['playthrough'] = 5
        with self.assertRaisesRegex(ValueError, 'save-bound'):
            jobs.start(params)
        payload['generation_context_digest'] = 'b' * 64
        with self.assertRaisesRegex(ValueError, 'generation context'):
            jobs.register_cache(json.dumps(payload))

    def test_native_search_reuses_scanner_and_retains_timed_out_owner(self):
        class Oracle:
            remote_call_pending = False
            def __enter__(self): return self
            def __exit__(self, *_): self.remote_call_pending = True
        oracle = Oracle()
        runtime = RuntimeApplication(identity=lambda: (123, object(), 'verified'), oracle_factory=lambda **_: oracle)
        template = {'context_digest': runtime.service.context.context_digest, 'template_hex': make_record().hex()}
        with patch('nioh3_scroll_editor.native_search_maps.prepare_maps', return_value={}), patch('nioh3_scroll_editor.runtime_application.scan_next_candidate', side_effect=TimeoutError('in flight')) as scanner:
            with self.assertRaises(TimeoutError):
                runtime.search(template, 100, 1, 5, 180, 183,
                               {'primary_effect_ids': [123]}, 1000, threading.Event(), lambda _: None)
            self.assertEqual(1000, scanner.call_args.kwargs['max_seeds'])
            self.assertEqual(frozenset([123]), scanner.call_args.kwargs['primary_effect_ids'])
        self.assertFalse(runtime.shutdown()['safe_to_shutdown'])

    def test_busy_write_cannot_be_cancelled_or_replaced(self):
        jobs = ProtectedJobs()
        release = threading.Event()
        first = jobs.start('save.commit', lambda *_: (release.wait(2), {'done': True})[1])
        try:
            self.assertFalse(jobs.idle())
            with self.assertRaisesRegex(ValueError, 'cannot be cancelled'):
                jobs.cancel(first['job_id'])
            with self.assertRaisesRegex(RuntimeError, 'BUSY'):
                jobs.start('save.commit', lambda *_: {})
        finally:
            release.set()
            jobs.join()
        self.assertEqual('completed', jobs.snapshot(first['job_id'])['state'])

    def test_failed_hook_restoration_retains_owner(self):
        class Session:
            def stop(self):
                raise OSError('restoration uncertain')
            def hit_count(self):
                return 0
        runtime = RuntimeApplication()
        runtime.session = Session()
        self.assertFalse(runtime.shutdown()['safe_to_shutdown'])
        self.assertIsNotNone(runtime.session)

    def test_retired_native_call_blocks_shutdown_until_completed(self):
        class Oracle:
            remote_call_pending = True
        runtime = RuntimeApplication()
        oracle = Oracle()
        runtime.retired_oracles.append(oracle)
        self.assertFalse(runtime.shutdown()['safe_to_shutdown'])
        oracle.remote_call_pending = False
        self.assertTrue(runtime.shutdown()['safe_to_shutdown'])


if __name__ == '__main__':
    unittest.main()
