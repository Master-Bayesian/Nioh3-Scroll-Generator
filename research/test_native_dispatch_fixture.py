"""Exercise real Windows debugger ownership against our synthetic process only."""
import json
from pathlib import Path
import subprocess
import sys
import time
from dataclasses import replace
from types import SimpleNamespace
from unittest.mock import patch
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.live_add_native_transport import NativeLiveAddTransport
from nioh3_scroll_editor.live_add_profile import PC_V201
from nioh3_scroll_editor.dispatch_evidence import verify_dispatch

root = Path(sys.argv[1])
process = subprocess.Popen([str(root / 'native_dispatch_fixture.exe')], stdin=subprocess.PIPE,
                           stdout=subprocess.PIPE, text=True)
try:
    fixture = json.loads(process.stdout.readline())
    profile = replace(PC_V201, dispatch_rva=fixture['entry'], dispatch_return_rva=fixture['caller_return'],
                      manager_pointer_rva=fixture['manager_pointer'], container_offset=0)
    with patch('nioh3_scroll_editor.live_add_native_transport.LAYOUT', profile), \
         patch('nioh3_scroll_editor.live_add_native_transport.running_game_identity',
               return_value=(fixture['pid'], SimpleNamespace(display_version='PC v2.01'), 'synthetic')), \
         patch('nioh3_scroll_editor.live_add_native_transport.find_module_base', return_value=0):
        transport = NativeLiveAddTransport(root / ('receipts-' + str(uuid4())))
        operation = str(uuid4())
        transport.call('noop', operation_id=operation, pid=fixture['pid'], profile_id=profile.profile_id)
        deadline = time.monotonic() + 20
        while True:
            result = transport.call('status', operation_id=operation)
            if result['released'] or time.monotonic() > deadline:
                break
            time.sleep(0.05)
        print(json.dumps(result), flush=True)
        assert result['phase'] == 'completed', result
        result['breakpoints'] = [] if result['breakpoint_count'] == 0 else ['unknown']
        verify_dispatch(result)
        assert process.poll() is None
        transport.thread.join(5)
        with patch('nioh3_scroll_editor.live_add_native_transport.LAYOUT',
                   replace(profile, dispatch_return_rva=profile.dispatch_return_rva + 1)):
            rejected_id = str(uuid4())
            transport.call('noop', operation_id=rejected_id, pid=fixture['pid'], profile_id=profile.profile_id)
            transport.thread.join(15)
            rejected = transport.call('status', operation_id=rejected_id)
            assert rejected['released'] and rejected['redirect_count'] == 0, rejected
            assert rejected['phase'] == 'rejected' and rejected['breakpoint_count'] == 0, rejected
        repeated_id = str(uuid4())
        transport.call('noop', operation_id=repeated_id, pid=fixture['pid'], profile_id=profile.profile_id)
        transport.thread.join(15)
        repeated = transport.call('status', operation_id=repeated_id)
        assert repeated['phase'] == 'completed' and repeated['released'], repeated
        assert process.poll() is None
        print('NATIVE_SYNTHETIC_DISPATCH_OK', flush=True)
finally:
    if process.poll() is None:
        process.communicate('\n', timeout=10)
