"""Failure injection against our owned synthetic EXE; never attaches to a game."""
import json
from pathlib import Path
import subprocess
import threading
import time
import sys
from dataclasses import replace
from types import SimpleNamespace
from unittest.mock import patch
from uuid import uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nioh3_scroll_editor.live_add_native_transport import NativeLiveAddTransport
from nioh3_scroll_editor.windows_debug_session import WindowsDebug
from nioh3_scroll_editor.live_add_profile import PC_V201
from nioh3_scroll_editor.process_instance import process_creation_time


def run(root, scenario):
    process = subprocess.Popen([str(root / 'native_dispatch_fixture.exe')], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, text=True)
    fixture = json.loads(process.stdout.readline())
    creation = process_creation_time(fixture['pid'])
    assert creation is not None, 'Synthetic target must have a live process identity'
    profile = replace(PC_V201, dispatch_rva=fixture['entry'], dispatch_return_rva=fixture['caller_return'],
                      manager_pointer_rva=fixture['manager_pointer'], container_offset=0)
    calls = {'restores': 0}
    allow_restore = threading.Event()

    class FaultDebug(WindowsDebug):
        def arm_thread(self, *args):
            super().arm_thread(*args)
            if scenario == 'exit_during_ownership':
                process.terminate()

        def restore_threads(self):
            calls['restores'] += 1
            if scenario == 'restore_once' and calls['restores'] == 1:
                raise OSError('Injected context restoration failure')
            if scenario == 'restore_until_released' and not allow_restore.is_set():
                raise OSError('Injected restoration failure until explicitly released')
            return super().restore_threads()

        def context(self, tid):
            context = super().context(tid)
            if scenario == 'active_breakpoint' and not self.original_debug:
                context.Dr7 |= 1
            return context

    owner = None
    try:
        if scenario == 'attached_debugger':
            owner = WindowsDebug(fixture['pid'])
            owner.attach()
        with patch('nioh3_scroll_editor.live_add_native_transport.LAYOUT', profile), \
             patch('nioh3_scroll_editor.live_add_native_transport.running_game_identity',
                   return_value=(fixture['pid'], SimpleNamespace(display_version='PC v2.01'), 'synthetic')), \
             patch('nioh3_scroll_editor.live_add_native_transport.find_module_base', return_value=0), \
             patch('nioh3_scroll_editor.live_add_native_transport.WindowsDebug', FaultDebug):
            transport = NativeLiveAddTransport(root / 'fault-receipts')
            operation = str(uuid4())
            transport.call('noop', operation_id=operation, pid=fixture['pid'], profile_id=profile.profile_id,
                           process_creation_time=creation)
            if scenario == 'restore_until_released':
                deadline = time.monotonic() + 10
                while calls['restores'] < 3 and time.monotonic() < deadline:
                    time.sleep(0.05)
                assert calls['restores'] >= 3 and transport.receipt['active'] and not transport.receipt['released']
                allow_restore.set()
            transport.thread.join(20)
            assert not transport.thread.is_alive(), (scenario, transport.receipt)
            result = transport.call('status', operation_id=operation)
            assert result['released'] and not result['active'] and result['breakpoint_count'] == 0, result
            assert result['phase'] != 'completed', result
            try:
                transport.call('noop', operation_id=operation, pid=fixture['pid'], profile_id=profile.profile_id,
                               process_creation_time=creation)
            except RuntimeError:
                pass
            else:
                raise AssertionError('An existing operation was replayed')
            if scenario in ('restore_once','active_breakpoint','attached_debugger'):
                assert process.poll() is None
            return {'scenario':scenario,'released':True,'phase':result['phase'],'redirect_count':result['redirect_count'],**calls}
    finally:
        allow_restore.set()
        if owner:
            owner.close()
        if process.poll() is None:
            process.communicate('\n', timeout=10)


if __name__ == '__main__':
    root = Path(sys.argv[1]).resolve()
    if len(sys.argv) > 2:
        print(json.dumps(run(root, sys.argv[2])), flush=True)
        sys.exit(0)
    results = []
    for name in ('attached_debugger','active_breakpoint','restore_once','exit_during_ownership','restore_until_released'):
        result = subprocess.run([sys.executable, __file__, str(root), name], capture_output=True, text=True, timeout=35)
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
        results.append(json.loads(result.stdout))
        print(json.dumps(results[-1]), flush=True)
    print(json.dumps(results, indent=2))
