"""Safe retry policy and pre-dispatch serial boundary for the native live-add adapter."""
import hashlib
import json
import struct
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch
import unittest

from nioh3_scroll_editor import savegame
from nioh3_scroll_editor.live_add_adapter import (
    IDENTITY_RESOURCES,
    SAVE_GENERATION_SERIAL_MAX,
    LiveAddAdapter,
    serial_in_save_domain,
    verify_executable_identity,
)
from nioh3_scroll_editor.live_add_profile import PC_V201


class ScriptedTransport:
    def __init__(self, results):
        self.results = list(results)
        self.current = None
        self.preview_calls = 0

    def call(self, method, **params):
        if method == 'preview':
            self.preview_calls += 1
            self.current = dict(self.results.pop(0), operation_id=params['operation_id'])
            return self.current
        if method in ('status', 'release'):
            return self.current
        raise AssertionError(method)


class LiveAddPreviewRetryTests(unittest.TestCase):
    def setUp(self):
        self.plan = {'pid': 7, 'profile_id': 'profile', 'builder_code_hex': '00'}
        record = bytearray(232)
        struct.pack_into('<HHHH', record, 0, 0xE604, 0x1122, 0x3344, 170)
        struct.pack_into('<H', record, 0x10, 561)
        struct.pack_into('<I', record, 0x14, 0x55667788)
        struct.pack_into('<I', record, 0x18, 0x02800002)
        struct.pack_into('<I', record, 0x20, 10030565)
        record[0x0F] = 1
        record[0x30] = 3
        record[0x34:0xDC] = bytes(range(168))
        record[0xDC:0xE0] = b'abcd'
        self.record = bytes(record)

    @staticmethod
    def idle_miss():
        return {'phase': 'rejected', 'redirect_count': 0, 'released': True,
                'active': False, 'breakpoint_count': 0, 'error': 'No accepted idle dispatch before timeout'}

    @staticmethod
    def accepted():
        return {'phase': 'completed', 'redirect_count': 1, 'released': True,
                'active': False, 'breakpoint_count': 0, 'source_hex': bytes(232).hex()}

    @patch('nioh3_scroll_editor.live_add_adapter.verify_assembly_preview')
    @patch('nioh3_scroll_editor.live_add_adapter.verify_dispatch')
    def test_retries_only_released_zero_redirect_idle_misses(self, verify_dispatch, verify_record):
        transport = ScriptedTransport([self.idle_miss(), self.accepted()])
        result = LiveAddAdapter(transport).preview(self.plan, self.record)
        self.assertEqual(result['phase'], 'completed')
        self.assertEqual(transport.preview_calls, 2)
        verify_dispatch.assert_called_once()
        verify_record.assert_called_once()

    @patch('nioh3_scroll_editor.live_add_adapter.verify_dispatch', side_effect=ValueError('unsafe'))
    def test_does_not_retry_an_other_released_failure(self, _verify_dispatch):
        result = {'phase': 'rejected', 'redirect_count': 0, 'released': True,
                  'active': False, 'error': 'native code identity changed'}
        transport = ScriptedTransport([result])
        with self.assertRaisesRegex(ValueError, 'unsafe'):
            LiveAddAdapter(transport).preview(self.plan, self.record)
        self.assertEqual(transport.preview_calls, 1)


class _FixedDigest:
    def __init__(self, value):
        self._value = value

    def hexdigest(self):
        return self._value


class _StubHashlib:
    """The adapter hashes the code ranges and the container; both agree here."""

    digest = 'deadbeef'

    def sha256(self, _payload):
        return _FixedDigest(self.digest)


class _StubIdentityJson:
    """Substitutes the code-identity resource so the guard is reachable offline."""

    def __init__(self, **extra):
        self.extra = extra

    def loads(self, _payload):
        payload = {'profile_id': PC_V201.profile_id,
                   'ranges': [{'rva': 0, 'size': 4, 'sha256': _StubHashlib.digest}]}
        payload.update(self.extra)
        return payload


class _PingTransport:
    """Answers only the pre-flight ping; any submission is a test failure."""

    def __init__(self):
        self.calls = []

    def call(self, method, **params):
        self.calls.append(method)
        if method == 'ping':
            return {'pid': 42, 'profile_id': PC_V201.profile_id, 'busy': False}
        raise AssertionError(f'unexpected submission: {method}')


class _SerialReader:
    """Minimal canonical reader for the pre-dispatch serial guard."""

    def __init__(self, values, signature, pid, creation_time, ones=()):
        self.values = dict(values)
        self.signature = signature
        self.pid = pid
        self._creation_time = creation_time
        self.ones = set(ones)
        self.module_base = 0x10000000

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        return False

    def creation_time(self):
        return self._creation_time

    def u64(self, address):
        try:
            return self.values[address]
        except KeyError as error:
            raise AssertionError(f'unexpected read at {address:#x}') from error

    def read(self, address, size):
        if address == self.module_base + PC_V201.dispatch_rva:
            return self.signature
        if address in self.ones:
            assert size == 1
            return b'\1'
        return bytes(size)


class LiveAddSerialBoundaryTests(unittest.TestCase):
    """The live counter must leave its successor inside the saved u32 field."""

    PID = 42
    CREATION_TIME = 12345
    SCHEDULER = 0x40000000

    def _stack(self, serial, reader, identity_extra=None):
        """Patch every collaborator so only the serial guard can decide."""

        stack = ExitStack()
        self.addCleanup(stack.close)
        stack.enter_context(patch.object(LiveAddAdapter, 'identity',
                                         return_value=(self.PID, PC_V201)))
        stack.enter_context(patch(
            'nioh3_scroll_editor.live_add_adapter.capture_inventory',
            return_value={'pid': self.PID,
                          'process_creation_time': self.CREATION_TIME,
                          'serial_counter': str(serial),
                          'acquisition_order_counter': 10,
                          'container_sha256': _StubHashlib.digest}))
        stack.enter_context(patch(
            'nioh3_scroll_editor.live_add_adapter.capture_index',
            return_value={'pid': self.PID,
                          'process_creation_time': self.CREATION_TIME}))
        stack.enter_context(patch(
            'nioh3_scroll_editor.live_add_adapter.ProcessReader',
            return_value=reader))
        stack.enter_context(patch(
            'nioh3_scroll_editor.live_add_adapter.hashlib', _StubHashlib()))
        stack.enter_context(patch(
            'nioh3_scroll_editor.live_add_adapter.json',
            _StubIdentityJson(**(identity_extra or {}))))
        return stack

    def _reader(self, serial, with_scheduler=False):
        values = {0x10000000 + PC_V201.manager_pointer_rva: 0x20000000,
                  0x20000000: 0x30000000,
                  0x30000000 + PC_V201.serial_counter_offset: serial}
        ones = ()
        if with_scheduler:
            values[0x10000000 + PC_V201.scheduler_pointer_rva] = self.SCHEDULER
            ones = (self.SCHEDULER + PC_V201.scheduler_ready_offset,)
        return _SerialReader(values, bytes.fromhex(PC_V201.dispatch_signature_hex),
                             pid=self.PID, creation_time=self.CREATION_TIME, ones=ones)

    def test_save_domain_boundary_cases(self):
        cases = {
            0: False,
            1: True,
            0x3345: True,
            0xFFFF_FFFB: True,
            0xFFFF_FFFC: False,
            0xFFFF_FFFD: False,
            0x1_0000_0000: False,
        }
        for serial, expected in cases.items():
            with self.subTest(serial=hex(serial)):
                self.assertIs(serial_in_save_domain(serial), expected)

    def test_cap_matches_the_save_codec_constant(self):
        self.assertEqual(SAVE_GENERATION_SERIAL_MAX, savegame.SCROLL_GENERATION_SERIAL_MAX)
        self.assertEqual(SAVE_GENERATION_SERIAL_MAX, 0xFFFFFFFC)

    def _run_inspect(self, serial):
        """Drive the adapter to the serial guard without a process or a dispatch."""

        transport = _PingTransport()
        self._stack(serial, self._reader(serial))
        return LiveAddAdapter(transport).inspect()

    def test_serial_at_the_save_cap_is_refused_before_any_submission(self):
        for serial in (0xFFFF_FFFC, 0xFFFF_FFFD, 0x1_0000_0000):
            with self.subTest(serial=hex(serial)):
                with self.assertRaisesRegex(ValueError,
                                            'cannot be advanced within the save format'):
                    self._run_inspect(serial)

    def test_zero_serial_keeps_the_shipped_identity_message(self):
        with self.assertRaisesRegex(ValueError, 'Serial changed or exceeds'):
            self._run_inspect(0)

    def test_unverified_ranges_require_a_declared_executable_identity(self):
        transport = _PingTransport()
        self._stack(0x3345, self._reader(0x3345),
                    identity_extra={'unverified_ranges': [
                        {'v201_rva': 0x3fcb18, 'size': 49, 'reason': 'ambiguous-shape-match'}]})
        with self.assertRaisesRegex(ValueError, 'without an executable identity'):
            LiveAddAdapter(transport).inspect()

    def test_a_mismatched_executable_identity_is_refused_before_any_submission(self):
        transport = _PingTransport()
        self._stack(0x3345, self._reader(0x3345),
                    identity_extra={'executable_sha256': '00' * 32,
                                    'unverified_ranges': [
                                        {'v201_rva': 0x3fcb18, 'size': 49,
                                         'reason': 'ambiguous-shape-match'}]})
        with self.assertRaisesRegex(RuntimeError, 'executable identity differs'):
            LiveAddAdapter(transport).inspect()
        self.assertEqual(transport.calls, ['ping'])

    def test_top_allowed_serial_passes_the_guard(self):
        """0xFFFF_FFFB advances to exactly the cap, so it must not be refused."""

        transport = _PingTransport()
        self._stack(0xFFFF_FFFB, self._reader(0xFFFF_FFFB, with_scheduler=True))
        plan, _inventory, _index = LiveAddAdapter(transport).inspect()
        self.assertEqual(plan['serial'], 0xFFFF_FFFB)
        self.assertEqual(plan['scheduler_owner'], self.SCHEDULER)
        self.assertEqual(transport.calls, ['ping'])


class LiveAddExecutableIdentityTests(unittest.TestCase):
    """The v2.02 resource's primary identity is the installed executable."""

    def test_helper_accepts_a_matching_executable_and_rejects_everything_else(self):
        import tempfile

        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'Nioh3.exe'
            path.write_bytes(b'nioh3-build-bytes')
            digest = hashlib.sha256(b'nioh3-build-bytes').hexdigest().upper()
            self.assertTrue(verify_executable_identity(path, digest))
            self.assertTrue(verify_executable_identity(path, digest.lower()))
            self.assertFalse(verify_executable_identity(path, '00' * 32))
        self.assertTrue(verify_executable_identity(None, None))
        self.assertFalse(verify_executable_identity(None, 'AB' * 32))
        self.assertFalse(verify_executable_identity('/nonexistent/Nioh3.exe', 'AB' * 32))

    def test_both_profiles_have_a_code_identity_resource(self):
        root = Path(__file__).resolve().parents[1] / 'nioh3_scroll_editor'
        self.assertEqual(
            sorted(IDENTITY_RESOURCES),
            ['pc-v2.01-live-add-r1', 'pc-v2.02-live-add-candidate'])
        for profile_id, relative in IDENTITY_RESOURCES.items():
            with self.subTest(profile_id=profile_id):
                self.assertTrue((root / relative).is_file())

    def test_v202_resource_declares_the_verified_executable(self):
        root = Path(__file__).resolve().parents[1] / 'nioh3_scroll_editor'
        payload = json.loads((root / IDENTITY_RESOURCES['pc-v2.02-live-add-candidate'])
                             .read_text(encoding='utf-8'))
        self.assertEqual(payload['profile_id'], 'pc-v2.02-live-add-candidate')
        self.assertEqual(
            payload['executable_sha256'],
            'E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130')
        self.assertEqual(len(payload['ranges']), 50)
        self.assertEqual(len(payload['unverified_ranges']), 10)
        for entry in payload['unverified_ranges']:
            self.assertEqual(set(entry), {'v201_rva', 'size', 'reason'})


if __name__ == '__main__':
    unittest.main()
