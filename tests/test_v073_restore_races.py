"""Restore transaction races against isolated synthetic files, not a live game."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from nioh3_scroll_editor import savegame as sg
from tests.test_backend_freeze import _prepare_restore_fixture


class RestoreRaceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.installer, self.source, self.save, self.backup,
         self.system, _) = _prepare_restore_fixture(self.root)
        self.source = self.source.resolve()
        self.save = self.save.resolve()
        self.backup = self.backup.resolve()
        self.system = self.system.resolve()
        self.baseline = {p: p.read_bytes() for p in (self.save, self.backup, self.system)}
        sleeper = patch.object(sg.time, 'sleep', return_value=None)
        sleeper.start()
        self.addCleanup(sleeper.stop)

    def restore(self):
        return self.installer.restore_backup(self.source)

    def checkpoint(self):
        return next(p for p in (self.installer.state_root / 'backups').iterdir()
                    if p != self.source)

    def assert_unchanged(self):
        for path, content in self.baseline.items():
            self.assertEqual(path.read_bytes(), content)

    def test_source_change_after_validation_cannot_redefine_accepted_hash(self):
        decrypt = self.installer.crypto.decrypt

        def mutate(source, output):
            decrypt(source, output)
            (self.source / 'SYSTEMSAVEDATA.BIN').write_bytes(b'TAMPERED-SOURCE')

        with patch.object(self.installer.crypto, 'decrypt', side_effect=mutate):
            with self.assertRaisesRegex(RuntimeError, 'validated backup manifest'):
                self.restore()
        self.assert_unchanged()

    def test_checkpoint_copy_must_match_the_original_generation(self):
        copy = sg._copy_file_durable

        def mutate(source, target):
            if source == self.save:
                self.save.write_bytes(b'GAME-ADVANCED-DURING-CHECKPOINT')
            copy(source, target)

        with patch.object(sg, '_copy_file_durable', side_effect=mutate):
            with self.assertRaisesRegex(RuntimeError, 'automatic backup'):
                self.restore()
        self.assertEqual(self.save.read_bytes(), b'GAME-ADVANCED-DURING-CHECKPOINT')
        self.assertEqual(self.system.read_bytes(), self.baseline[self.system])

    def test_newly_created_peer_is_not_overwritten(self):
        self.backup.unlink()
        copy = sg._copy_file_durable

        def interleave(source, target):
            copy(source, target)
            if '.scroll-generator-restore-' in target.name:
                self.backup.write_bytes(b'NEW-GAME-BACKUP')

        with patch.object(sg, '_copy_file_durable', side_effect=interleave):
            with self.assertRaises(sg.SaveCommitUncertain):
                self.restore()
        self.assertEqual(self.backup.read_bytes(), b'NEW-GAME-BACKUP')
        self.assertEqual(self.save.read_bytes(), self.baseline[self.save])

    def test_game_generation_change_after_first_replace_blocks_further_writes(self):
        replace = sg.os.replace

        def interleave(source, target):
            replace(source, target)
            if target == self.system:
                self.save.write_bytes(b'NEWER-GAME-MAIN')

        with patch.object(sg.os, 'replace', side_effect=interleave):
            with self.assertRaises(sg.SaveCommitUncertain) as caught:
                self.restore()
        self.assertEqual(self.save.read_bytes(), b'NEWER-GAME-MAIN')
        self.assertEqual(self.system.read_bytes(), b'SOURCE-SYSTEM')
        self.assertEqual(self.backup.read_bytes(), self.baseline[self.backup])
        self.assertEqual(caught.exception.save_diagnostics['action'], 'restore-backup')
        journal = json.loads((self.checkpoint() / 'restore-journal.json').read_text())
        self.assertEqual(journal['state'], 'recovery_required')

    def test_tampered_checkpoint_is_never_used_for_rollback(self):
        replace = sg.os.replace

        def fail(source, target):
            if target == self.backup:
                (self.checkpoint() / 'SYSTEMSAVEDATA.BIN').write_bytes(b'TAMPERED-CHECKPOINT')
                raise PermissionError('second-replace-failed')
            replace(source, target)

        with patch.object(sg.os, 'replace', side_effect=fail):
            with self.assertRaisesRegex(sg.SaveCommitUncertain, 'original generation'):
                self.restore()
        self.assertEqual(self.system.read_bytes(), b'SOURCE-SYSTEM')
        self.assertEqual(self.save.read_bytes(), self.baseline[self.save])

    def test_game_write_during_rollback_staging_is_preserved(self):
        replace, copy = sg.os.replace, sg._copy_file_durable

        def fail(source, target):
            if target == self.backup:
                raise PermissionError('second-replace-failed')
            replace(source, target)

        def interleave(source, target):
            copy(source, target)
            if '.scroll-generator-rollback-' in target.name:
                self.system.write_bytes(b'NEWER-GAME-SYSTEM')

        with patch.object(sg.os, 'replace', side_effect=fail), \
             patch.object(sg, '_copy_file_durable', side_effect=interleave):
            with self.assertRaises(sg.SaveCommitUncertain):
                self.restore()
        self.assertEqual(self.system.read_bytes(), b'NEWER-GAME-SYSTEM')

    def test_secondary_journal_failure_preserves_the_original_restore_error(self):
        replace, write = sg.os.replace, sg._write_json_durable

        def fail(source, target):
            if target == self.backup:
                raise PermissionError('original-second-replace-failure')
            replace(source, target)

        def fail_journal(path, payload):
            if payload.get('state') == 'rolled_back':
                raise OSError('secondary-journal-failure')
            write(path, payload)

        with patch.object(sg.os, 'replace', side_effect=fail), \
             patch.object(sg, '_write_json_durable', side_effect=fail_journal):
            with self.assertRaisesRegex(PermissionError, 'original-second-replace-failure'):
                self.restore()
        self.assert_unchanged()

    def test_final_peer_change_is_not_reported_as_committed(self):
        replace = sg.os.replace

        def interleave(source, target):
            replace(source, target)
            if target == self.save:
                self.backup.write_bytes(b'NEWER-GAME-BACKUP')

        with patch.object(sg.os, 'replace', side_effect=interleave):
            with self.assertRaises(sg.SaveCommitUncertain):
                self.restore()
        self.assertEqual(self.backup.read_bytes(), b'NEWER-GAME-BACKUP')
        self.assertEqual(self.save.read_bytes(), b'SOURCE-MAIN')


if __name__ == '__main__':
    unittest.main()
