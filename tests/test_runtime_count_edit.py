"""Remaining-count transaction boundaries, using isolated bytes and fake memory."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

from nioh3_scroll_editor.runtime_count_edit import RuntimeCountEditor


class Memory:
    def __init__(self):
        raw = bytearray(232)
        struct.pack_into('<H', raw, 0, 0xE604)
        struct.pack_into('<I', raw, 0x20, 123)
        struct.pack_into('<Q', raw, 0x28, 456)
        raw[0x30], raw[0x33] = 3, 6
        self.state = {'pid': 10, 'creation_time': 'one', 'manager': 20, 'data': 30,
                      'address': 40, 'serial': '456', 'record_hex': raw.hex()}
        self.calls, self.fail_after_write = 0, False

    def capture(self, serial):
        if serial != self.state['serial']:
            raise ValueError('Wrong instance')
        return deepcopy(self.state)

    def write(self, expected, desired):
        assert expected == self.state
        self.calls += 1
        raw = bytearray.fromhex(self.state['record_hex']); raw[0x33] = desired
        self.state['record_hex'] = raw.hex()
        if self.fail_after_write:
            raise OSError('Lost acknowledgement after write')
        return bytes(raw)


class CountEditTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.save = self.root / '76561198000000000/SAVEDATA00/SAVEDATA.BIN'
        self.save.parent.mkdir(parents=True); self.save.write_bytes(b'isolated-save-fixture')
        self.memory = Memory(); self.app = RuntimeCountEditor(self.root, memory=self.memory)
        self.source = {'save_path': str(self.save), 'source_sha256': hashlib.sha256(self.save.read_bytes()).hexdigest(),
                       'record_hex': self.memory.state['record_hex']}

    def prepare(self):
        return self.app.prepare(self.source, 2)['count_edit']

    def execute(self, plan):
        return self.app.execute(plan['operation_id'], plan['plan_digest'])['count_edit']

    def test_backup_review_single_byte_and_duplicate_submission(self):
        plan = self.prepare()
        self.assertEqual(self.memory.calls, 0)
        payload = self.app.plan(plan['operation_id'])['plan']
        self.assertEqual(Path(payload['backup_path']).read_bytes(), self.save.read_bytes())
        self.assertTrue(Path(payload['backup_path']).with_name('backup-manifest.json').exists())
        before = bytes.fromhex(self.memory.state['record_hex'])
        self.assertEqual(self.execute(plan)['state'], 'verified')
        self.assertEqual([i for i, (a,b) in enumerate(zip(before, bytes.fromhex(self.memory.state['record_hex']))) if a != b], [0x33])
        self.assertEqual(self.execute(plan)['state'], 'verified')
        self.assertEqual(self.memory.calls, 1)

    def test_actual_unsaved_count_is_reviewed_but_other_identity_cannot_drift(self):
        raw = bytearray.fromhex(self.memory.state['record_hex']); raw[0x33] = 3
        self.memory.state['record_hex'] = raw.hex()
        self.assertEqual(self.prepare()['old_count'], 3)
        raw[0x70] = 9; self.memory.state['record_hex'] = raw.hex()
        with self.assertRaisesRegex(ValueError, 'fields differ'): self.prepare()

    def test_changed_save_backup_record_or_process_prevents_write(self):
        for change in ('save', 'backup', 'record', 'process'):
            with self.subTest(change=change):
                plan = self.prepare(); payload = self.app.plan(plan['operation_id'])['plan']
                if change == 'save': self.save.write_bytes(b'changed')
                elif change == 'backup': Path(payload['backup_path']).unlink()
                elif change == 'record': self.memory.state['record_hex'] = self.memory.state['record_hex'][:-2] + 'ff'
                else: self.memory.state['creation_time'] = 'new-process'
                self.assertEqual(self.execute(plan)['state'], 'rejected'); self.assertEqual(self.memory.calls, 0)
                self.save.write_bytes(b'isolated-save-fixture'); self.memory = Memory(); self.app.memory = self.memory

    def test_lost_acknowledgement_survives_restart_without_replay(self):
        plan = self.prepare(); self.memory.fail_after_write = True
        self.assertEqual(self.execute(plan)['state'], 'uncertain')
        app = RuntimeCountEditor(self.root, memory=self.memory)
        self.assertEqual(app.status(plan['operation_id'])['count_edit']['state'], 'uncertain')
        self.assertEqual(app.execute(plan['operation_id'],plan['plan_digest'])['count_edit']['state'], 'uncertain')
        self.assertEqual(self.memory.calls, 1)
        self.assertEqual(app.recover(plan['operation_id'])['count_edit']['state'], 'verified')
        self.assertEqual(app.execute(plan['operation_id'],plan['plan_digest'])['count_edit']['state'], 'verified')
        self.assertEqual(self.memory.calls, 1)

    def test_invalid_counts_digest_and_native_side_field_are_rejected(self):
        for value in (-1, 8, 2.5, True):
            with self.assertRaises(ValueError): self.app.prepare(self.source, value)
        plan = self.prepare()
        with self.assertRaises(ValueError): self.app.execute(plan['operation_id'], 'wrong')
        self.assertEqual(self.memory.calls, 0)
        raw = bytearray.fromhex(self.memory.state['record_hex']);raw[0x0E] = 1
        self.memory.state['record_hex'] = raw.hex();self.source['record_hex'] = raw.hex()
        with self.assertRaisesRegex(ValueError, 'not supported'): self.prepare()


    def test_protected_job_outputs_match_worker_contract_without_private_records(self):
        from nioh3_scroll_editor.protected_jobs import ProtectedJobs
        from nioh3_scroll_editor.protected_worker import RESPONSE_VALIDATOR
        jobs = ProtectedJobs()
        prepared = self.prepare()
        for method, action in (
            ('runtime.count_prepare', lambda: self.app.status(prepared['operation_id'])),
            ('runtime.count_execute', lambda: self.app.execute(prepared['operation_id'], prepared['plan_digest'])),
            ('runtime.count_recover', lambda: self.app.recover(prepared['operation_id'])),
        ):
            initial = jobs.start(method, lambda _, __: action())
            jobs.join()
            result = jobs.snapshot(initial['job_id'])
            self.assertEqual(result['state'], 'completed')
            RESPONSE_VALIDATOR.validate({'protocol': 1, 'id': 'count-contract', 'ok': True, 'result': result})
            text = json.dumps(result)
            self.assertNotIn('record_hex', text)
            self.assertNotIn('save_path', text)
        self.assertEqual(self.memory.calls, 1)


if __name__ == '__main__':
    unittest.main()
