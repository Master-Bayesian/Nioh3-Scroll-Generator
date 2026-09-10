"""Exercise prepare/execute/recovery with a synthetic native adapter, no game."""
from copy import deepcopy
from pathlib import Path
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from nioh3_scroll_editor.live_add_application import LiveAddApplication
from nioh3_scroll_editor.models import CandidateRecordStage
from nioh3_scroll_editor.savegame import SCROLL_GROUP_OFFSET
import test_single_native_insertion as evidence_tests


class FakeCrypto:
    def decrypt(self, source, target):
        target.write_bytes(source.read_bytes())


class FakeAdapter:
    def __init__(self, evidence):
        self.e = evidence
        self.pending = None
        self.calls = 0
        self.inserted = False
        self.disconnect = False
        self.changed = False
        self.restarted = False

    def inspect(self):
        p = dict(self.e.plan, profile_id='pc-v2.01-live-add-r1', manager=11, data=12,
                 process_creation_time='synthetic-process-instance',
                 scheduler_owner=13, function_address=14, builder_code_hex='00', insertion_code_hex='00')
        if self.changed:
            p['serial'] += 1
        if self.restarted:
            p['process_creation_time'] = 'another-process-with-the-same-pid'
        return p, deepcopy(self.e.before), deepcopy(self.e.index_before)

    def preview(self, plan, record):
        return {'native_invoked': False, 'kind': 'synthetic_adapter'}

    def readback(self):
        return deepcopy((self.e.after, self.e.index_after) if self.inserted else (self.e.before, self.e.index_before))

    def insert(self, plan):
        self.calls += 1
        self.inserted = True
        self.receipt = dict(self.e.execution, operation_id=plan['operation_id'])
        if self.disconnect:
            self.pending = plan['operation_id']
            raise ConnectionError('Synthetic disconnect after native insertion')
        return self.receipt

    def recover(self, operation_id, pid):
        self.pending = None
        return self.receipt

    def safe_to_shutdown(self):
        return self.pending is None


class LiveAddApplicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.e = evidence_tests.InsertionEvidenceTests()
        self.e.setUp()
        self.adapter = FakeAdapter(self.e)
        self.app = LiveAddApplication(self.temp.name, 'context', adapter=self.adapter, crypto=FakeCrypto())
        self.save = Path(self.temp.name) / '76561198000000000' / 'SAVEDATA00' / 'SAVEDATA.BIN'
        self.save.parent.mkdir(parents=True)
        self.save.write_bytes(bytes(SCROLL_GROUP_OFFSET + 400*232))
        raw = bytearray(self.e.raw)
        struct.pack_into('<I', raw, 24, 0x800002)
        raw[48] = 3
        candidate = SimpleNamespace(record=bytes(raw), installation_record=None, seed=123, rarity=3,
                                    record_stage=CandidateRecordStage.FINAL_RECORD)
        for name, value in [('import_candidate', candidate), ('require_search_candidate_ready', candidate)]:
            patcher = patch('nioh3_scroll_editor.live_add_application.' + name, return_value=value)
            patcher.start();self.addCleanup(patcher.stop)
        patcher = patch('nioh3_scroll_editor.live_add_application.OperationPolicy')
        patcher.start();self.addCleanup(patcher.stop)

    def prepare(self):
        return self.app.prepare({'candidate_id': 'candidate'}, self.save)

    def test_prepare_execute_then_duplicate_returns_receipt(self):
        prepared = self.prepare()
        result = self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertEqual(result['state'], 'verified')
        self.assertEqual(self.app.execute(prepared['operation_id'], prepared['plan_digest']), result)
        self.assertEqual(self.adapter.calls, 1)

    def test_missing_or_changed_automatic_backup_never_dispatches(self):
        for corrupt in (False, True):
            prepared = self.prepare()
            backup = Path(prepared['backup_path'])
            self.assertEqual(backup.read_bytes(), self.save.read_bytes())
            if corrupt:
                backup.write_bytes(b'corrupted')
            else:
                backup.unlink()
            with self.assertRaisesRegex(ValueError, 'backup'):
                self.app.execute(prepared['operation_id'], prepared['plan_digest'])
            self.assertEqual(self.adapter.calls, 0)

    def test_live_backup_is_visible_to_existing_account_scoped_manager(self):
        from nioh3_scroll_editor.savegame import list_backup_entries
        import json
        prepared = self.prepare()
        entries = list_backup_entries(Path(self.temp.name))
        self.assertEqual(len(entries), 1)
        entry = entries[0]
        self.assertEqual(entry.action, 'v2-live-add')
        self.assertEqual(entry.account_id, 76561198000000000)
        self.assertEqual(entry.save_slot_index, 0)
        backup = Path(prepared['backup_path'])
        self.assertEqual(entry.directory, backup.parent)
        manifest = json.loads((backup.parent / 'backup-manifest.json').read_text())
        self.assertEqual(manifest['backup_files'][0]['source_role'], 'main_save')
        self.assertEqual(manifest['backup_files'][0]['size'], len(self.save.read_bytes()))

    def test_disconnect_is_recovered_without_second_insertion(self):
        prepared = self.prepare()
        self.adapter.disconnect = True
        with self.assertRaises(ConnectionError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertFalse(self.app.safe_to_shutdown())
        self.assertEqual(self.app.status(prepared['operation_id'])['state'], 'uncertain')
        with self.assertRaises(RuntimeError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertEqual(self.app.recover(prepared['operation_id'])['state'], 'verified')
        self.assertEqual(self.adapter.calls, 1)

    def test_cancel_and_changed_inventory_never_dispatch(self):
        prepared = self.prepare()
        self.app.cancel(prepared['operation_id'])
        with self.assertRaises(RuntimeError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        prepared = self.prepare()
        self.adapter.changed = True
        with self.assertRaises(ValueError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertEqual(self.adapter.calls, 0)

    def test_changed_save_and_corrupt_review_digest_never_dispatch(self):
        prepared = self.prepare()
        with self.assertRaises(ValueError):
            self.app.execute(prepared['operation_id'], 'wrong')
        self.save.write_bytes(self.save.read_bytes() + b'changed')
        with self.assertRaises(ValueError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertEqual(self.adapter.calls, 0)

    def test_reused_process_id_cannot_execute_an_old_plan(self):
        prepared = self.prepare()
        self.adapter.restarted = True
        with self.assertRaises(ValueError):
            self.app.execute(prepared['operation_id'], prepared['plan_digest'])
        self.assertEqual(self.adapter.calls, 0)


if __name__ == '__main__':
    unittest.main()
