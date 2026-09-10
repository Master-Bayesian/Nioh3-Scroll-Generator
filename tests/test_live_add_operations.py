"""At-most-once claims remain conservative across cancellation and restart."""
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import tempfile
import unittest
from uuid import uuid4

from nioh3_scroll_editor.live_add_operations import LiveAddOperations
from nioh3_scroll_editor.live_add_profile import live_add_profile


class LiveAddOperationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.ops = LiveAddOperations(self.temp.name)
        self.id = str(uuid4())
        self.plan = {'operation_id': self.id, 'serial': '18446744073709550000'}
        self.digest = self.ops.prepare(self.id, self.plan)['plan_digest']

    def test_restart_does_not_replay_uncertain_dispatch(self):
        self.ops.claim(self.id, self.digest)
        reopened = LiveAddOperations(self.temp.name)
        self.assertEqual(reopened.snapshot(self.id)['state'], 'uncertain')
        with self.assertRaises(FileExistsError):
            reopened.claim(self.id, self.digest)
        with self.assertRaises(FileExistsError):
            reopened.cancel(self.id)

    def test_cancel_and_dispatch_compete_for_one_claim(self):
        def claim(which):
            try:
                self.ops.cancel(self.id) if which else self.ops.claim(self.id, self.digest)
                return True
            except FileExistsError:
                return False
        with ThreadPoolExecutor(max_workers=2) as pool:
            self.assertEqual(sum(pool.map(claim, (True, False))), 1)

    def test_pre_dispatch_cancel_survives_restart(self):
        self.ops.cancel(self.id)
        self.assertEqual(LiveAddOperations(self.temp.name).snapshot(self.id)['state'], 'cancelled')

    def test_tampered_plan_and_wrong_digest_are_rejected(self):
        with self.assertRaises(ValueError):
            self.ops.claim(self.id, 'wrong')
        path = Path(self.temp.name) / self.id / 'plan.json'
        value = json.loads(path.read_bytes())
        value['plan']['serial'] = '5'
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):
            self.ops.claim(self.id, self.digest)

    def test_receipt_requires_matching_dispatch_and_verification(self):
        self.ops.claim(self.id, self.digest)
        receipt = {'operation_id': self.id, 'state': 'verified'}
        with self.assertRaises(ValueError):
            self.ops.complete(self.id, receipt)
        receipt.update(full_container_and_native_index_verified=True, dispatch_and_cleanup_verified=True)
        self.assertEqual(self.ops.complete(self.id, receipt)['state'], 'verified')
        with self.assertRaises(FileExistsError):
            self.ops.complete(self.id, receipt)

    def test_path_traversal_and_unknown_versions_are_rejected(self):
        with self.assertRaises(ValueError):
            self.ops.snapshot('../escape')
        for version in ((2, 0, 0, 2), (2, 0, 2, 0)):
            with self.assertRaises(ValueError):
                live_add_profile(version)
        self.assertEqual(live_add_profile((2, 0, 1, 0)).profile_id, 'pc-v2.01-live-add-r1')

    def test_modified_durable_receipt_is_not_reported_as_verified(self):
        self.ops.claim(self.id, self.digest)
        self.ops.complete(self.id, {'operation_id': self.id, 'state': 'verified',
            'full_container_and_native_index_verified': True, 'dispatch_and_cleanup_verified': True})
        path = Path(self.temp.name) / self.id / 'receipt.json'
        value = json.loads(path.read_bytes())
        value['receipt']['state'] = 'rejected_before_dispatch'
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):
            self.ops.snapshot(self.id)


if __name__ == '__main__':
    unittest.main()
