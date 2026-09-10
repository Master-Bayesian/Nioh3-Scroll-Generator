"""Synthetic batch safety checks. Never attach to a game."""
import tempfile
import threading
import unittest
from pathlib import Path
from types import SimpleNamespace
from uuid import uuid4
from unittest.mock import patch
from unittest.mock import Mock
import struct

from nioh3_scroll_editor.live_add_batch import LiveAddBatch
from tests import test_live_add_application as live_add_tests
from nioh3_scroll_editor.save_application import SaveApplication
from nioh3_scroll_editor.savegame import SaveInstaller


class FakeApplication:
    def __init__(self, root):
        self.operations = SimpleNamespace(root=Path(root))
        self.lock = threading.RLock()
        self.calls, self.parents, self.states = [], [], {}
        self.fail_at = None

    def validate_candidate(self, candidate):
        if candidate.get('invalid'):
            raise ValueError('Invalid candidate')

    def prepare(self, candidate, path, *, previous_operation_id=None):
        self.parents.append(previous_operation_id)
        operation_id = str(uuid4())
        self.states[operation_id] = 'prepared'
        return dict(operation_id=operation_id, plan_digest='digest', count_before=len(self.calls))

    def execute(self, operation_id, digest):
        self.calls.append(operation_id)
        self.states[operation_id] = 'uncertain'
        if len(self.calls) == self.fail_at:
            raise ConnectionError('Outcome unknown')
        self.states[operation_id] = 'verified'
        return self.status(operation_id)

    def cancel(self, operation_id):
        self.states[operation_id] = 'cancelled'

    def status(self, operation_id):
        return dict(operation_id=operation_id, state=self.states[operation_id])


class BatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.app = FakeApplication(self.temp.name)
        self.batch = LiveAddBatch(self.app)
        self.candidates = [{'candidate_id': str(i)} for i in range(3)]

    def prepare(self):
        return self.batch.prepare(self.candidates, Path(self.temp.name) / 'save.bin')

    def test_sequence_binds_each_verified_predecessor(self):
        p = self.prepare()
        result = self.batch.execute(p['batch_id'], p['plan_digest'])
        self.assertEqual(result['verified_count'], 3)
        self.assertEqual(self.app.parents, [None, *self.app.calls[:-1]])
        with self.assertRaises(FileExistsError):
            self.batch.execute(p['batch_id'], p['plan_digest'])
        self.assertEqual(len(self.app.calls), 3)

    def test_unknown_stops_and_cannot_replay(self):
        self.app.fail_at = 2
        p = self.prepare()
        with self.assertRaises(ConnectionError):
            self.batch.execute(p['batch_id'], p['plan_digest'])
        states = self.batch.status(p['batch_id'])['children']
        self.assertEqual([s['state'] for s in states], ['verified', 'uncertain'])
        with self.assertRaises(FileExistsError):
            self.batch.execute(p['batch_id'], p['plan_digest'])
        self.assertEqual(len(self.app.calls), 2)

    def test_invalid_later_item_rejected_before_preparing_any(self):
        self.candidates[-1]['invalid'] = True
        with self.assertRaises(ValueError):
            self.prepare()
        self.assertFalse(self.app.parents)

    def test_reconciled_child_settles_partial_batch_without_replay(self):
        self.app.fail_at = 2
        plan = self.prepare()
        with self.assertRaises(ConnectionError):
            self.batch.execute(plan['batch_id'], plan['plan_digest'])
        self.assertEqual(self.batch.status(plan['batch_id'])['state'], 'uncertain')
        self.app.states[self.app.calls[-1]] = 'verified'
        self.assertEqual(self.batch.status(plan['batch_id'])['state'], 'partial')
        self.assertEqual(len(self.app.calls), 2)
        with self.assertRaises(FileExistsError):
            self.batch.execute(plan['batch_id'], plan['plan_digest'])

    def test_reconciled_last_child_settles_complete_batch_without_replay(self):
        self.app.fail_at = 3
        plan = self.prepare()
        with self.assertRaises(ConnectionError):
            self.batch.execute(plan['batch_id'], plan['plan_digest'])
        self.app.states[self.app.calls[-1]] = 'verified'
        self.assertEqual(self.batch.status(plan['batch_id'])['state'], 'complete')
        self.assertEqual(len(self.app.calls), 3)

    def test_cancel_between_items_keeps_verified_receipts(self):
        p = self.prepare()
        progress = []
        result = self.batch.execute(p['batch_id'], p['plan_digest'], cancelled=lambda: len(self.app.calls) == 1, progress=progress.append)
        self.assertEqual(progress, [{'completed': 0, 'total': 3}, {'completed': 1, 'total': 3}])
        self.assertEqual(result['state'], 'partial')
        self.assertEqual(result['verified_count'], 1)

    def test_bad_digest_never_claims(self):
        p = self.prepare()
        with self.assertRaises(ValueError):
            self.batch.execute(p['batch_id'], 'wrong')
        self.assertFalse(self.batch.status(p['batch_id'])['claimed'])


class ChainedPreparationTests(unittest.TestCase):
    setUp = live_add_tests.LiveAddApplicationTests.setUp

    def advance(self):
        first = self.app.prepare({'candidate_id': 'first'}, self.save)
        self.app.execute(first['operation_id'], first['plan_digest'])
        self.e.before, self.e.index_before = self.e.after, self.e.index_after
        self.e.plan['serial'] += 1
        self.e.plan['slot'] += 1
        self.adapter.inserted = False
        return first

    def test_unsaved_verified_predecessor_can_prepare_next_item(self):
        first = self.advance()
        second = self.app.prepare({'candidate_id': 'second'}, self.save, previous_operation_id=first['operation_id'])
        self.assertEqual(second['state'], 'prepared')

    def test_unrelated_inventory_change_rejected(self):
        first = self.advance()
        self.e.before['acquisition_order_counter'] += 1
        with self.assertRaises(ValueError):
            self.app.prepare({'candidate_id': 'second'}, self.save, previous_operation_id=first['operation_id'])


class SaveBatchTests(unittest.TestCase):
    def test_reviewed_source_hash_rejected_before_backup_or_decryption(self):
        with tempfile.TemporaryDirectory() as root:
            source = Path(root) / '76561198000000000' / 'SAVEDATA00' / 'SAVEDATA.BIN'
            source.parent.mkdir(parents=True)
            source.write_bytes(b'changed')
            fake = SimpleNamespace(save_path=source, state_root=Path(root))
            record = b'\x01' + bytes(231)
            with self.assertRaisesRegex(RuntimeError, 'Save changed'):
                SaveInstaller.install_many(fake, [record], action='test', expected_source_sha256='old')
            self.assertEqual(source.read_bytes(), b'changed')
            self.assertFalse(list(Path(root).rglob('backup-*')))

    def test_batch_preparation_retains_records_and_unsigned_transfer_count(self):
        app = SaveApplication.__new__(SaveApplication)
        app.lock, app.plans = threading.RLock(), {}
        app._snapshot = Mock(return_value=('source-hash', None))
        app._plan = Mock(return_value={'plan_id': 'batch'})
        def child(save_id, snapshot_id, candidate, recommended, transfer):
            key = candidate['candidate_id']
            app.plans[key] = {'data': (bytes(232), transfer)}
            return {'plan_id': key, 'preview': {'candidate_id': key}}
        app.prepare_install = child
        candidates = [{'candidate_id': 'one'}, {'candidate_id': 'two'}]
        app.prepare_install_many('save', 'snapshot', candidates, 123, 0xFFFFFFFF)
        args = app._plan.call_args.args
        self.assertEqual(args[2], 'install_many')
        self.assertEqual(len(args[3]), 2)
        self.assertTrue(all(struct.unpack_from('<I', record, 0xDC)[0] == 0xFFFFFFFF for record in args[3]))
        self.assertFalse(app.plans)


if __name__ == '__main__':
    unittest.main()
