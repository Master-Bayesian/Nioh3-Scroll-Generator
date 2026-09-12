"""Faults in the actual installer; quiescence is not a game ownership proof."""
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from nioh3_scroll_editor import savegame as sg
from tests import test_save_commit_guard as support
from tests.test_beta_editor import TEST_ACCOUNT_ID, make_record


class SaveRaceTests(unittest.TestCase):
    def setUp(self):
        self.root, self.save, self.backup, self.system = support.SaveCommitGuardTests.make_fixture(self)
        self.save = self.save.resolve()
        self.backup = self.backup.resolve()
        self.system = self.system.resolve()
        self.old = self.save.read_bytes()
        self.sleep = patch.object(sg.time, 'sleep', return_value=None)
        self.sleep.start()
        self.addCleanup(self.sleep.stop)

    def install(self, crypto=None):
        return sg.SaveInstaller(save_path=self.save, crypto=crypto or support.PassthroughCrypto(),
            state_root=self.root / 'state').install(make_record(seed=86872488, account_id=TEST_ACCOUNT_ID), transfer_count=0xFFFFFFFF)

    def journal(self):
        return json.loads(next((self.root / 'state/backups').glob('*/save-write-journal.json')).read_text())

    def test_game_write_during_commit_staging_is_not_overwritten(self):
        copy = sg._copy_file_durable
        newer = self.old + b'game-flush-after-precommit-check'
        def interleave(source, target):
            copy(source, target)
            if target.parent == self.save.parent:
                self.save.write_bytes(newer)
        with patch.object(sg, '_copy_file_durable', side_effect=interleave):
            with self.assertRaisesRegex(RuntimeError, 'SAVE_SYNC_ACTIVE'):
                self.install()
        self.assertEqual(self.save.read_bytes(), newer)
        self.assertEqual(self.journal()['state'], 'aborted')

    def test_sibling_write_during_commit_staging_is_not_ignored(self):
        copy = sg._copy_file_durable
        def interleave(source, target):
            copy(source, target)
            if target.parent == self.save.parent:
                self.system.write_bytes(b'new-game-generation')
        with patch.object(sg, '_copy_file_durable', side_effect=interleave):
            with self.assertRaisesRegex(RuntimeError, 'SAVE_SYNC_ACTIVE'):
                self.install()
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_post_replace_durability_failure_is_uncertain_not_aborted(self):
        replace = sg._replace_file_durable
        def fail_after_rename(source, target):
            replace(source, target)
            if target == self.save:
                raise OSError('directory-flush-failed-after-rename')
        with patch.object(sg, '_replace_file_durable', side_effect=fail_after_rename):
            with self.assertRaises(sg.SaveCommitUncertain):
                self.install()
        self.assertNotEqual(self.save.read_bytes(), self.old)
        self.assertEqual(self.journal()['state'], 'recovery_required')

    def test_changed_peer_during_failed_readback_prohibits_rollback(self):
        save, system = self.save, self.system
        class Crypto(support.PassthroughCrypto):
            def decrypt(self, source, output):
                super().decrypt(source, output)
                if output.name == 'installed-verification.bin':
                    system.write_bytes(b'game-advanced-system-save')
                    raise ValueError('readback-failed')
        with self.assertRaisesRegex(RuntimeError, 'SAVE_COMMIT_UNCERTAIN'):
            self.install(Crypto())
        self.assertNotEqual(save.read_bytes(), self.old)
        self.assertEqual(system.read_bytes(), b'game-advanced-system-save')
        self.assertEqual(self.journal()['state'], 'recovery_required')

    def test_backup_tamper_is_not_restored_even_if_copy_matches_tampered_file(self):
        root = self.root
        class Crypto(support.PassthroughCrypto):
            def decrypt(self, source, output):
                super().decrypt(source, output)
                if output.name == 'installed-verification.bin':
                    next((root / 'state/backups').glob('*/SAVEDATA.BIN')).write_bytes(b'tampered')
                    raise ValueError('force-readback-failure')
        with self.assertRaisesRegex(RuntimeError, 'original checkpoint'):
            self.install(Crypto())
        self.assertNotEqual(self.save.read_bytes(), b'tampered')

    def test_peer_change_after_successful_decrypt_does_not_report_committed(self):
        system = self.system
        class Crypto(support.PassthroughCrypto):
            def decrypt(self, source, output):
                super().decrypt(source, output)
                if output.name == 'installed-verification.bin':
                    system.write_bytes(b'later-system-generation')
        with self.assertRaises(sg.SaveCommitUncertain):
            self.install(Crypto())
        self.assertEqual(self.journal()['state'], 'recovery_required')

    def test_write_after_readback_is_detected_even_when_decrypt_output_was_valid(self):
        save, old = self.save, self.old
        class Crypto(support.PassthroughCrypto):
            def decrypt(self, source, output):
                super().decrypt(source, output)
                if output.name == 'installed-verification.bin':
                    save.write_bytes(old)
        with self.assertRaises(sg.SaveCommitUncertain):
            self.install(Crypto())
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_final_journal_failure_reports_uncertain_without_undoing_verified_bytes(self):
        write = sg._write_json_durable
        def failure(path, payload):
            if payload.get('state') == 'committed':
                raise OSError('final-journal-full')
            return write(path, payload)
        with patch.object(sg, '_write_json_durable', side_effect=failure):
            with self.assertRaises(sg.SaveCommitUncertain):
                self.install()
        self.assertNotEqual(self.save.read_bytes(), self.old)
        self.assertEqual(self.journal()['state'], 'prepared')

    def test_rollback_journal_error_does_not_mask_the_original_verification_error(self):
        write = sg._write_json_durable
        class Crypto(support.PassthroughCrypto):
            def decrypt(self, source, output):
                super().decrypt(source, output)
                if output.name == 'installed-verification.bin':
                    raise ValueError('original-readback-fault')
        def failure(path, payload):
            if payload.get('state') == 'rolled_back':
                raise OSError('secondary-journal-fault')
            return write(path, payload)
        with patch.object(sg, '_write_json_durable', side_effect=failure):
            with self.assertRaisesRegex(RuntimeError, 'SAVE_COMMIT_ROLLED_BACK') as caught:
                self.install(Crypto())
        self.assertEqual(str(caught.exception.__cause__), 'original-readback-fault')
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_application_does_not_downgrade_uncertain_to_not_committed_when_old_bytes_return(self):
        from nioh3_scroll_editor.save_application import SaveApplication
        from types import SimpleNamespace
        from uuid import uuid4
        from unittest.mock import Mock
        import time
        app = SaveApplication(self.root/'application-state', crypto=support.PassthroughCrypto(),
                              service=object(), game_process_ids=lambda: [71])
        operation = uuid4().hex
        app.plans[operation] = {'expires':time.monotonic()+60, 'kind':'install', 'save_id':'owned',
                                'source_hash':sg.sha256_file(self.save), 'data':(bytes(232),0)}
        installer = SimpleNamespace(save_path=self.save,
            install=Mock(side_effect=sg.SaveCommitUncertain('observed a replacement then old bytes returned')))
        with patch.object(app, '_installer', return_value=installer):
            result = app.commit(operation)
        self.assertEqual(result['commit_status'], 'unknown')
        self.assertEqual(app.operation(operation)['commit_status'], 'unknown')
        installer.install.assert_called_once()
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_abort_journal_failure_keeps_the_original_precondition_error(self):
        write, check = sg._write_json_durable, sg.require_related_save_fingerprints
        preparing = False
        def write_fault(path, payload):
            nonlocal preparing
            if payload.get('state') == 'preparing':
                preparing = True
            if payload.get('state') == 'aborted':
                raise OSError('secondary-abort-journal-failure')
            return write(path, payload)
        def check_fault(*args, **kwargs):
            if preparing:
                raise RuntimeError('original-precondition-failure')
            return check(*args, **kwargs)
        with patch.object(sg, '_write_json_durable', side_effect=write_fault), \
             patch.object(sg, 'require_related_save_fingerprints', side_effect=check_fault):
            with self.assertRaisesRegex(RuntimeError, 'original-precondition-failure'):
                self.install()
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_transaction_error_retains_candidate_and_journal_identity(self):
        copy = sg._copy_file_durable
        def fail_at_stage(source, target):
            if target.parent == self.save.parent:
                raise OSError('first-staging-fault')
            return copy(source, target)
        with patch.object(sg, '_copy_file_durable', side_effect=fail_at_stage):
            with self.assertRaisesRegex(OSError, 'first-staging-fault') as caught:
                self.install()
        evidence = caught.exception.save_diagnostics
        self.assertEqual(evidence['save_path'], str(self.save))
        self.assertEqual(len(evidence['candidate_record_hex']), 0xE8 * 2)
        journal = json.loads(Path(evidence['journal_path']).read_text())
        self.assertEqual(evidence['transaction_id'], journal['operation_id'])
        self.assertTrue((Path(evidence['backup_directory']) / 'SAVEDATA.BIN').exists())
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_application_failure_preserves_transaction_evidence_with_public_id(self):
        from nioh3_scroll_editor.save_application import SaveApplication
        from types import SimpleNamespace
        from unittest.mock import Mock
        from uuid import uuid4
        import time
        app = SaveApplication(self.root/'application-state', crypto=support.PassthroughCrypto(),
                              service=object(), game_process_ids=lambda: [71])
        plan_id = uuid4().hex
        original_hash = sg.sha256_file(self.save)
        app.plans[plan_id] = {'expires': time.monotonic()+60, 'kind':'install', 'save_id':'owned',
                            'source_hash': original_hash, 'data': (bytes(232),0)}
        failure = sg.SaveCommitUncertain('durability-not-known')
        failure.save_diagnostics = {'transaction_id':'native-save-transaction',
                                    'journal_path':str(self.root/'journal.json')}
        installer = SimpleNamespace(save_path=self.save, install=Mock(side_effect=failure))
        with patch.object(app, '_installer', return_value=installer):
            result = app.commit(plan_id)
        self.assertEqual(result['operation_id'], plan_id)
        self.assertEqual(result['details']['reviewed_source_sha256'], original_hash)
        self.assertEqual(result['details']['transaction_id'], 'native-save-transaction')
        self.assertEqual(app.operation(plan_id)['details'], result['details'])
        self.assertEqual(result['commit_status'], 'unknown')

    def test_batch_append_cannot_silently_rewrite_an_existing_record(self):
        with patch.object(sg, 'repair_duplicate_scroll_generation_serials',
                          return_value=(bytes(20), ({'slot_index': 0},))):
            with self.assertRaisesRegex(RuntimeError, 'APPEND_ONLY_REPAIR_REQUIRED'):
                sg.SaveInstaller(save_path=self.save, crypto=support.PassthroughCrypto(),
                    state_root=self.root / 'state').install_many(
                        [make_record(seed=3, account_id=TEST_ACCOUNT_ID)], action='batch')
        self.assertEqual(self.save.read_bytes(), self.old)

    def test_delayed_game_flush_after_return_remains_an_explicit_release_blocker(self):
        # This is a negative capability test, NOT title-screen acceptance.
        # Even the patched filesystem transaction cannot revoke game ownership.
        result = self.install()
        self.assertTrue(result.commit_status.startswith('committed'))
        self.assertNotEqual(self.save.read_bytes(), self.old)
        self.save.write_bytes(self.old)  # Simulated cached game save on later exit.
        self.assertEqual(self.save.read_bytes(), self.old)
        self.assertEqual(self.journal()['state'], 'committed')
        self.assertEqual((result.backup_directory / 'SAVEDATA.BIN').read_bytes(), self.old)


if __name__ == '__main__':
    unittest.main()
