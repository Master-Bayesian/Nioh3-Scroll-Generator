"""Exercise production lifecycle code using controlled transport/debugger faults."""
from copy import deepcopy
import json
from pathlib import Path
import struct
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from uuid import uuid4

from nioh3_scroll_editor.live_add_adapter import LiveAddAdapter
from nioh3_scroll_editor import live_add_native_transport as nt
from nioh3_scroll_editor.live_add_profile import PC_V201 as P
from nioh3_scroll_editor import process_instance as pi
from tests import test_live_add_application as app_tests


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        t = tempfile.TemporaryDirectory(); self.addCleanup(t.cleanup)
        self.root = Path(t.name)
        self.op = str(uuid4())
        self.identity = patch.object(nt, 'running_game_identity', return_value=(71, SimpleNamespace(display_version='PC v2.01'), 'game'))
        self.identity.start(); self.addCleanup(self.identity.stop)
        self.process = patch.object(nt, 'process_creation_time', return_value='born-1')
        self.process.start(); self.addCleanup(self.process.stop)
        self.exited = patch.object(nt, 'original_process_exited', return_value=False)
        self.exited.start(); self.addCleanup(self.exited.stop)

    def receipt(self, **extra):
        return dict(operation_id=self.op, pid=71, process_creation_time='born-1', phase='preparing',
                    active=True, released=False, breakpoint_count=-1, redirect_count=0, **extra)

    def test_worker_restart_blocks_new_id_when_old_receipt_is_unreleased(self):
        (self.root / (self.op + '.json')).write_text(json.dumps(self.receipt()))
        transport = nt.NativeLiveAddTransport(self.root)
        self.assertTrue(transport.call('ping')['busy'])
        with self.assertRaisesRegex(RuntimeError, self.op):
            transport.call('preview', operation_id=str(uuid4()), pid=71, profile_id=P.profile_id, process_creation_time='born-1')
        self.assertIsNone(transport.thread)

    def test_corrupt_receipt_is_not_treated_as_an_empty_directory(self):
        (self.root / (self.op + '.json')).write_text('{partial')
        with self.assertRaisesRegex(RuntimeError, 'Unresolved native receipt'):
            nt.NativeLiveAddTransport(self.root).call('ping')

    def test_original_process_exit_allows_new_session_but_never_old_id_replay(self):
        (self.root / (self.op + '.json')).write_text(json.dumps(self.receipt()))
        transport = nt.NativeLiveAddTransport(self.root)
        with patch.object(nt, 'original_process_exited', return_value=True):
            self.assertFalse(transport.call('ping')['busy'])
            with self.assertRaisesRegex(RuntimeError, 'already submitted'):
                transport.call('preview', operation_id=self.op, pid=71, profile_id=P.profile_id, process_creation_time='born-1')

    def test_wrong_process_lifetime_rejected_before_receipt_or_thread(self):
        transport = nt.NativeLiveAddTransport(self.root)
        with self.assertRaisesRegex(RuntimeError, 'PROCESS_INSTANCE_CHANGED'):
            transport.call('preview', operation_id=self.op, pid=71, profile_id=P.profile_id, process_creation_time='previous-process')
        self.assertFalse(transport.operation_known(self.op))
        self.assertIsNone(transport.thread)

    def test_receipt_creation_failure_remains_known_without_starting_native_thread(self):
        transport = nt.NativeLiveAddTransport(self.root)
        with patch.object(transport, '_save', side_effect=OSError('disk-full')):
            with self.assertRaisesRegex(OSError, 'disk-full'):
                transport.call('preview', operation_id=self.op, pid=71, profile_id=P.profile_id, process_creation_time='born-1')
        self.assertTrue(transport.operation_known(self.op))
        self.assertIsNone(transport.thread)
        self.assertTrue(transport.call('ping')['busy'])

    def test_proof_failure_cannot_mask_original_submit_error_or_clear_owner(self):
        original = ConnectionError('lost-acknowledgement')
        class Transport:
            def call(self, *a, **k): raise original
            def operation_known(self, *a): raise OSError('receipt-unreadable')
        adapter = LiveAddAdapter(Transport())
        with self.assertRaises(ConnectionError) as caught:
            adapter._submit('insert', self.op, 71, process_creation_time='born-1')
        self.assertIs(caught.exception, original)
        self.assertEqual(adapter.pending, self.op)

    def test_late_preview_cleanup_releases_memory_owner_without_resubmission(self):
        value = self.receipt()
        class Transport:
            def call(self, *a, **k): return dict(value)
        adapter = LiveAddAdapter(Transport())
        adapter.pending, adapter.pending_pid, adapter.pending_creation_time = self.op, 71, 'born-1'
        with patch('nioh3_scroll_editor.live_add_adapter.original_process_exited', return_value=False):
            self.assertFalse(adapter.safe_to_shutdown())
            value.update(active=False, released=True, breakpoint_count=0)
            self.assertTrue(adapter.safe_to_shutdown())
        self.assertIsNone(adapter.pending_pid)

    def test_active_or_wrong_receipt_does_not_release_pending(self):
        class Transport:
            def call(self, *a, **k):
                return dict(operation_id='wrong', active=False, released=True, breakpoint_count=0)
        adapter = LiveAddAdapter(Transport())
        adapter.pending, adapter.pending_pid = self.op, 71
        with patch('nioh3_scroll_editor.live_add_adapter.original_process_exited', return_value=False):
            self.assertFalse(adapter.safe_to_shutdown())
            with self.assertRaisesRegex(RuntimeError, 'Another native operation'):
                adapter._submit('preview', str(uuid4()), 71)
        self.assertEqual(adapter.pending, self.op)

    def test_pid_reuse_and_access_denial_have_different_cleanup_semantics(self):
        with patch.object(pi, 'process_creation_time', return_value='born-2'):
            self.assertTrue(pi.original_process_exited(71, 'born-1'))
            self.assertFalse(pi.original_process_exited(71, None))
        with patch.object(pi, 'process_creation_time', side_effect=PermissionError()):
            self.assertFalse(pi.original_process_exited(71, 'born-1'))

    def test_busy_dispatch_is_skipped_but_changed_scheduler_is_not(self):
        values = {0x60: struct.pack('<Q', 1), 0x68: struct.pack('<Q', 2),
                  P.scheduler_pointer_rva: struct.pack('<Q', 100),
                  100 + P.scheduler_pending_offset: bytes(4), 100 + P.scheduler_ready_offset: b'\1'}
        api = SimpleNamespace(read=lambda address, size: values[address])
        self.assertFalse(nt.accepted_idle_dispatch(api, 0, 0, {'scheduler_owner':100}, 'insert'))
        values[0x68] = values[0x60]
        self.assertTrue(nt.accepted_idle_dispatch(api, 0, 0, {'scheduler_owner':100}, 'insert'))
        values[100 + P.scheduler_pending_offset] = b'\1\0\0\0'
        self.assertFalse(nt.accepted_idle_dispatch(api, 0, 0, {'scheduler_owner':100}, 'insert'))
        with self.assertRaisesRegex(RuntimeError, 'ownership changed'):
            nt.accepted_idle_dispatch(api, 0, 0, {'scheduler_owner':200}, 'insert')

    def test_continuous_events_still_expire_before_any_redirect(self):
        class API:
            def __init__(self, pid):
                self.process = 1; self.attached = False; self.threads = {}; self.original_debug = {}; self.context_buffers = {}
                self.memory = {P.dispatch_rva: bytes.fromhex(P.dispatch_signature_hex), P.manager_pointer_rva: struct.pack('<Q',100),100:struct.pack('<Q',200)}
                self.dll = SimpleNamespace(FlushInstructionCache=lambda *a: 1)
            def read(self,a,n): return self.memory[a][:n]
            def write(self,a,v): self.memory[a] = bytes(v)
            def allocate(self): return 0x900000
            def free(self,a): del self.memory[a]
            def require(self,v,n): assert v
            def attach(self): self.attached=True
            def wait(self,*a): return SimpleNamespace(code=6, data=SimpleNamespace(file=None))
            def restore_threads(self): pass
            def resume(self,*a): pass
            def close(self): self.attached=False
        transport = nt.NativeLiveAddTransport(self.root)
        transport.receipt = self.receipt()
        with patch.object(nt,'WindowsDebug', API), patch.object(nt,'find_module_base',return_value=0), \
             patch.object(nt,'creation_time_from_handle',return_value='born-1'), \
             patch.object(nt.time,'monotonic',side_effect=[0,1,11]):
            transport._run('noop', {'pid':71,'process_creation_time':'born-1'})
        result=transport.call('status',operation_id=self.op)
        self.assertEqual(result['redirect_count'],0)
        self.assertTrue(result['released'])
        self.assertEqual(result['error'],'No accepted idle dispatch before timeout')

    def test_readback_does_not_merge_different_process_lifetimes(self):
        adapter = LiveAddAdapter(None)
        with patch('nioh3_scroll_editor.live_add_adapter.capture_inventory', return_value={'pid':71,'process_creation_time':'first'}), \
             patch('nioh3_scroll_editor.live_add_adapter.capture_index', return_value={'pid':71,'process_creation_time':'second'}):
            with self.assertRaisesRegex(RuntimeError, 'PROCESS_INSTANCE_CHANGED'):
                adapter.readback()

    def test_recovery_never_uses_a_reused_pid(self):
        class Transport:
            def call(self, *a, **k): raise AssertionError('No receipt/readback against replacement process')
        adapter = LiveAddAdapter(Transport())
        with patch('nioh3_scroll_editor.live_add_adapter.process_creation_time', return_value='born-2'):
            with self.assertRaisesRegex(RuntimeError, 'PROCESS_INSTANCE_CHANGED'):
                adapter.recover(self.op, 71, 'born-1')

    def test_shared_directory_serializes_admission_between_workers(self):
        from nioh3_scroll_editor.native_submission_guard import submission_lock
        with submission_lock(self.root):
            with self.assertRaisesRegex(RuntimeError, 'admitting'):
                with submission_lock(self.root):
                    pass
        with submission_lock(self.root):
            pass

    def test_optional_ce_protocol_keeps_new_metadata_local(self):
        from nioh3_scroll_editor.live_add_ce_transport import CELiveAddTransport, encode_request
        captured = []
        class Transport(CELiveAddTransport):
            def __init__(inner): pass
            def call(inner, method, **params):
                captured.append(encode_request(method, '0'*64, params))
                return {}
        adapter = LiveAddAdapter(Transport())
        adapter._submit('preview', self.op, 71, pid=71, process_creation_time='born-1',
                        source_save_path='C:\\Users\\tester\\SAVEDATA.BIN',
                        candidate_id=None, parent_operation_id=None)
        self.assertNotIn(b'source_save_path', captured[0])
        self.assertNotIn(b'process_creation_time', captured[0])
        self.assertEqual(adapter.pending_creation_time, 'born-1')

    def test_receipt_persistence_failure_cannot_terminate_cleanup_owner(self):
        transport = nt.NativeLiveAddTransport(self.root)
        transport.receipt = self.receipt()
        transport._save()  # Admission was durable before any native action.
        with patch.object(transport, '_save', side_effect=OSError('receipt-disk-full')):
            self.assertFalse(transport._record_receipt())
        self.assertTrue(transport.operation_known(self.op))
        self.assertTrue(transport.call('ping')['busy'])
        transport.receipt.update(active=False, released=True, breakpoint_count=0)
        value = transport.call('status', operation_id=self.op)
        self.assertTrue(value['released'])
        persisted = json.loads((self.root / (self.op+'.json')).read_text())
        self.assertTrue(persisted['released'])
        self.assertIn('receipt-disk-full', persisted['receipt_write_error'])
        self.assertFalse(nt.NativeLiveAddTransport(self.root).call('ping')['busy'])

    def test_permission_error_is_not_an_absent_receipt_proof(self):
        transport = nt.NativeLiveAddTransport(self.root)
        with patch.object(Path, 'stat', side_effect=PermissionError('denied')):
            adapter = LiveAddAdapter(transport)
            self.assertFalse(adapter.submission_absent(self.op))

    def test_preview_retries_only_released_zero_redirect_idle_misses(self):
        from tests.test_single_native_insertion import InsertionEvidenceTests
        evidence = InsertionEvidenceTests(); evidence.setUp()
        raw = bytearray(evidence.raw); raw[0x30] = 3
        struct.pack_into('<I', raw, 0x18, 0x02800002)
        evidence.raw = bytes(raw)
        sent = []
        class Transport:
            def call(inner, method, **params):
                if method == 'preview':
                    sent.append(params['operation_id']); return {}
                if method == 'status':
                    return {'operation_id': sent[-1], 'phase':'rejected','active':False,
                            'released':True,'breakpoint_count':0,'redirect_count':0,
                            'error':'No accepted idle dispatch before timeout'}
                raise AssertionError(method)
        adapter = LiveAddAdapter(Transport())
        with self.assertRaises(ValueError):
            adapter.preview({'pid':71,'profile_id':P.profile_id,'builder_code_hex':'00'}, evidence.raw)
        self.assertEqual(len(set(sent)),3)
        self.assertIsNone(adapter.pending)

    def test_preview_does_not_retry_other_rejections(self):
        from tests.test_single_native_insertion import InsertionEvidenceTests
        evidence=InsertionEvidenceTests(); evidence.setUp()
        raw=bytearray(evidence.raw); raw[0x30]=3
        struct.pack_into('<I', raw, 0x18, 0x02800002)
        evidence.raw=bytes(raw)
        sent=[]
        class Transport:
            def call(inner, method, **params):
                if method=='preview': sent.append(params['operation_id']); return {}
                return {'operation_id':sent[-1], 'phase':'rejected','active':False,
                        'released':True,'breakpoint_count':0,'redirect_count':0,'error':'Inventory ownership changed'}
        with self.assertRaises(ValueError):
            LiveAddAdapter(Transport()).preview({'pid':71,'profile_id':P.profile_id,'builder_code_hex':'00'},evidence.raw)
        self.assertEqual(len(sent),1)


class ApplicationClaimTests(unittest.TestCase):
    setUp = app_tests.LiveAddApplicationTests.setUp
    prepare = app_tests.LiveAddApplicationTests.prepare

    def test_proved_unaccepted_submission_settles_business_claim(self):
        prepared=self.prepare()
        with patch.object(self.adapter,'insert',side_effect=RuntimeError('busy-before-acceptance')), \
             patch.object(self.adapter,'submission_absent',return_value=True,create=True):
            outcome=self.app.execute(prepared['operation_id'],prepared['plan_digest'])
        self.assertEqual(outcome['state'],'rejected_before_dispatch')
        self.assertEqual(self.app.execute(prepared['operation_id'],prepared['plan_digest']),outcome)

    def test_unreconciled_claim_blocks_fresh_plan_after_worker_restart(self):
        prepared=self.prepare()
        self.app.operations.claim(prepared['operation_id'],prepared['plan_digest'])
        with self.assertRaisesRegex(RuntimeError,'Uncertain insertion'):
            self.prepare()
        self.assertEqual(self.adapter.calls,0)

    def test_failed_verification_preserves_execution_and_inventory_evidence(self):
        prepared=self.prepare()
        self.adapter.e.execution['status']=2
        with self.assertRaises(ValueError):
            self.app.execute(prepared['operation_id'],prepared['plan_digest'])
        directory=self.app.operations.directory(prepared['operation_id'])
        attempt = next((directory/'verification').iterdir())
        self.assertTrue((attempt/'execution.json').is_file())
        self.assertTrue((attempt/'inventory-after.json').is_file())
        self.assertTrue((attempt/'index-after.json').is_file())
        self.assertFalse((directory/'inventory-after.json').exists())
        self.assertEqual(self.app.status(prepared['operation_id'])['state'],'uncertain')
        self.adapter.receipt['status'] = 3
        result = self.app.recover(prepared['operation_id'])
        self.assertEqual(result['state'], 'verified')
        self.assertEqual(self.adapter.calls, 1)
        self.assertEqual(len(list((directory/'verification').iterdir())), 2)
        self.assertEqual(json.loads((directory/'execution.json').read_text())['status'], 3)
        self.assertEqual(json.loads((attempt/'execution.json').read_text())['status'], 2)


if __name__=='__main__': unittest.main()
