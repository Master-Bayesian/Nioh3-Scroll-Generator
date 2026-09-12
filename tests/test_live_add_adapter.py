"""Safe retry policy for the read-only native live-add preview."""
import struct
from unittest.mock import patch
import unittest

from nioh3_scroll_editor.live_add_adapter import LiveAddAdapter


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


if __name__ == '__main__':
    unittest.main()
