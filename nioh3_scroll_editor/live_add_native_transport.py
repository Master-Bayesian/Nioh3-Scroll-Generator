"""Owned Windows debugger transport replacing CE for the fixed live-add ABI.

Only the protected host instantiates this class. An unresolved dispatch keeps
its event thread and allocation alive; neither cancellation nor timeout replays it.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import threading
import time

from .live_add_dispatch_code import build_dispatch_code
from .live_add_profile import PC_V201 as LAYOUT
from .native import find_module_base
from .runtime_application import running_game_identity
from .windows_debug_session import WindowsDebug


def registers(context):
    names = ('Rax', 'Rbx', 'Rcx', 'Rdx', 'Rsi', 'Rdi', 'Rbp', 'R8', 'R9',
             'R10', 'R11', 'R12', 'R13', 'R14', 'R15', 'Rsp', 'Rip', 'EFlags')
    return {name.upper(): getattr(context, name) for name in names}


class NativeLiveAddTransport:
    def __init__(self, directory):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.lock = threading.RLock()
        self.thread = None
        self.receipt = None
        self.owner = None

    def _save(self):
        path = self.directory / (self.receipt['operation_id'] + '.json')
        temporary = path.with_suffix('.tmp')
        with temporary.open('w', encoding='utf-8') as stream:
            json.dump(self.receipt, stream)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)

    def call(self, method, **params):
        with self.lock:
            if method == 'ping':
                pid, profile, _ = running_game_identity()
                if profile.display_version != 'PC v2.01':
                    raise ValueError('Native live addition requires accepted PC v2.01')
                return {'pid': pid, 'profile_id': LAYOUT.profile_id,
                        'busy': self.thread is not None and self.thread.is_alive()}
            from uuid import UUID
            operation_id = str(UUID(params['operation_id']))
            path = self.directory / (operation_id + '.json')
            if method in ('status', 'release'):
                if self.receipt and self.receipt['operation_id'] == operation_id:
                    return copy.deepcopy(self.receipt)
                if path.exists():
                    value = json.loads(path.read_text(encoding='utf-8'))
                    if value.get('released'):
                        return value
                    raise RuntimeError('Previous executor ownership is unresolved; never replay this operation')
                raise ValueError('Unknown native operation')
            pid, profile, _ = running_game_identity()
            if profile.display_version != 'PC v2.01':
                raise ValueError('Native live addition requires accepted PC v2.01')
            if method not in ('preview', 'insert', 'noop'):
                raise ValueError('Unsupported native executor method')
            if params.get('pid') != pid or params.get('profile_id') != LAYOUT.profile_id:
                raise ValueError('Process/profile changed')
            if path.exists() or (self.thread and self.thread.is_alive()):
                raise RuntimeError('Operation already submitted or executor is occupied')
            self.receipt = {'operation_id': operation_id, 'pid': pid, 'phase': 'preparing',
                            'active': True, 'released': False, 'redirect_count': 0,
                            'breakpoint_count': -1, 'executor': 'windows-native',
                            'mode': 'single_native_insertion' if method == 'insert' else method}
            self._save()
            self.thread = threading.Thread(target=self._run, args=(method, dict(params)),
                                           name='nioh3-native-dispatch', daemon=False)
            self.thread.start()
            return copy.deepcopy(self.receipt)

    def _run(self, mode, params):
        api = None
        allocation = None
        event = None
        redirected = False
        acknowledged = False
        attached_ready = False
        before = None
        chosen_tid = None
        error = None
        canary = bytes([0xA5]) * 16
        try:
            pid = params['pid']
            base = find_module_base(pid)
            entry = base + LAYOUT.dispatch_rva
            target = entry + 7
            original = bytes.fromhex(LAYOUT.dispatch_signature_hex)
            api = WindowsDebug(pid)
            self.owner = api
            def same(address, value):
                if api.read(address, len(value)) != value:
                    raise RuntimeError(f'Native precondition changed at {address:#x}')
            def u64(address):
                return struct.unpack('<Q', api.read(address, 8))[0]
            same(entry, original)
            manager = u64(base + LAYOUT.manager_pointer_rva)
            data = u64(manager)
            if not manager or not data:
                raise RuntimeError('Inventory owner is not loaded')
            container = data + LAYOUT.container_offset
            descriptor = expected = None
            if mode != 'noop':
                descriptor = bytearray.fromhex(params['descriptor_hex'])
                expected = bytes.fromhex(params['expected_record_hex'])
                if len(descriptor) != LAYOUT.descriptor_size or len(expected) != LAYOUT.record_size:
                    raise ValueError('Incomplete assembly input')
                descriptor[0x21] = 0 if mode == 'insert' else 1
                same(base + LAYOUT.builder_rva, bytes.fromhex(params['builder_code_hex']))
            if mode == 'insert':
                if (params['manager'], params['data'], params['function_address']) != (manager, data, base + LAYOUT.insertion_rva):
                    raise ValueError('Insertion targets changed')
                if not 0 <= params['slot'] < LAYOUT.capacity:
                    raise ValueError('Invalid destination slot')
                same(base + LAYOUT.insertion_rva, bytes.fromhex(params['insertion_code_hex']))
                same(container, bytes.fromhex(params['container_hex']))
            allocation = api.allocate()
            code = build_dispatch_code(allocation, target, original,
                     leaf=base + LAYOUT.builder_rva if mode != 'noop' else None,
                     argument=allocation + 0x600, second_argument=allocation + 0x400,
                     insertion=params if mode == 'insert' else None,
                     preserve_rarity5=expected is not None and expected[0x30:0x32] == b'\x05\x05')
            api.write(allocation, bytes(4096))
            api.write(allocation, code)
            if descriptor is not None:
                api.write(allocation + 0x400, descriptor)
                for offset in (0x5F0, 0x6E8, 0x7F0, 0x8E8):
                    api.write(allocation + offset, canary)
                api.write(allocation + 0x320, b'\xff' * 4)
            same(allocation, code)
            api.require(api.dll.FlushInstructionCache(api.process, allocation, len(code)), 'FlushInstructionCache')
            api.attach()
            deadline = time.monotonic() + 10
            timed_out = False
            while True:
                event = api.wait(100)
                if event is None:
                    if time.monotonic() > deadline and not redirected and not timed_out:
                        timed_out = True
                        api.require(api.dll.DebugBreakProcess(api.process), 'DebugBreakProcess')
                    continue
                handled = True
                finish = False
                try:
                    if event.code == 3:
                        info = event.data.process
                        if info.file:
                            api.dll.CloseHandle(info.file)
                        if info.process:
                            api.dll.CloseHandle(info.process)
                        try:
                            api.arm_thread(event.tid, info.thread, entry, target)
                        except Exception as setup_error:
                            error = str(setup_error)
                    elif event.code == 2:
                        try:
                            api.arm_thread(event.tid, event.data.thread.thread, entry, target)
                        except Exception as setup_error:
                            error = str(setup_error)
                    elif event.code == 4:
                        api.threads.pop(event.tid, None)
                        api.original_debug.pop(event.tid, None)
                    elif event.code == 5:
                        api.original_debug.clear()
                        for handle in api.threads.values():
                            api.dll.CloseHandle(handle)
                        api.threads.clear()
                        allocation = None
                        api.attached = False
                        error = 'Game exited during native dispatch'
                        finish = True
                    elif event.code == 6 and event.data.file:
                        api.dll.CloseHandle(event.data.file)
                    elif event.code == 1:
                        exception = event.data.exception.record
                        if exception.code == 0x80000003 and not attached_ready:
                            attached_ready = True
                            with self.lock:
                                self.receipt['phase'] = 'armed'
                                self._save()
                            if error:
                                finish = True
                        elif exception.code == 0x80000003 and timed_out and not redirected:
                            error, finish = 'No accepted idle dispatch before timeout', True
                        elif exception.code == 0x80000004:
                            context = api.context(event.tid)
                            if context.Rip == entry and context.Dr6 & 1:
                                context.Dr6 &= ~1
                                context.EFlags |= 0x10000
                                if attached_ready and not redirected and not error:
                                    same(entry, original)
                                    if mode != 'noop':
                                        same(base + LAYOUT.builder_rva, bytes.fromhex(params['builder_code_hex']))
                                    if mode == 'insert':
                                        same(base + LAYOUT.insertion_rva, bytes.fromhex(params['insertion_code_hex']))
                                    if context.Rsp % 16 != 8 or u64(context.Rsp) != base + LAYOUT.dispatch_return_rva:
                                        raise RuntimeError('Unexpected dispatch caller or stack alignment')
                                    if u64(context.Rcx + LAYOUT.queue_begin_offset) != u64(context.Rcx + LAYOUT.queue_end_offset):
                                        raise RuntimeError('Pickup queue is not empty')
                                    if u64(base + LAYOUT.manager_pointer_rva) != manager or u64(manager) != data:
                                        raise RuntimeError('Inventory ownership changed')
                                    serial_before = api.read(data + LAYOUT.serial_counter_offset, 8)
                                    if mode == 'insert':
                                        same(container, bytes.fromhex(params['container_hex']))
                                        if u64(data + LAYOUT.serial_counter_offset) != params['serial']:
                                            raise RuntimeError('Serial counter changed')
                                        if u64(base + LAYOUT.scheduler_pointer_rva) != params['scheduler_owner']:
                                            raise RuntimeError('Scheduler ownership changed')
                                        same(params['scheduler_owner'] + LAYOUT.scheduler_pending_offset, bytes(4))
                                        same(params['scheduler_owner'] + LAYOUT.scheduler_ready_offset, b'\1')
                                        same(container + params['slot'] * LAYOUT.record_size, bytes(2))
                                    before, chosen_tid = registers(context), event.tid
                                    with self.lock:
                                        self.receipt.update(phase='redirected', redirect_count=1,
                                                            before=before, thread_id=chosen_tid)
                                        self._save()
                                    # Claim is durable before any fallible redirect operation.
                                    redirected = True
                                    context.Rip = allocation
                                api.set_context(event.tid, context)
                            elif context.Rip == target and context.Dr6 & 2:
                                context.Dr6 &= ~2
                                context.EFlags |= 0x10000
                                if redirected and event.tid == chosen_tid and context.Rsp == before['RSP'] - 0x48:
                                    same(allocation + 0x300, b'\1\0\0\0')
                                    acknowledged = True
                                    with self.lock:
                                        self.receipt['after'] = registers(context)
                                        if mode != 'noop':
                                            self.receipt['source_hex'] = api.read(allocation + 0x600, 0xE8).hex()
                                        if mode == 'insert':
                                            self.receipt.update(slot=struct.unpack('<I', api.read(allocation+0x320,4))[0],
                                                status=struct.unpack('<I', api.read(allocation+0x318,4))[0],
                                                remainder_hex=api.read(allocation+0x800,0xE8).hex(),
                                                destination_hex=api.read(container+params['slot']*0xE8,0xE8).hex())
                                    if mode != 'noop':
                                        actual = bytes.fromhex(self.receipt['source_hex'])
                                        wanted_serial = params['serial'] if mode == 'insert' else 0xFFFFFFFFFFFFFFFF
                                        if (actual[:0x24] != expected[:0x24]
                                                or actual[0x30:0xE4] != expected[0x30:0xE4]
                                                or struct.unpack_from('<Q', actual, 0x28)[0] != wanted_serial):
                                            raise RuntimeError('Native builder output differs from reviewed record')
                                        same(allocation + 0x5F0, canary)
                                        same(allocation + 0x6E8, canary)
                                        same(allocation + 0x400, descriptor)
                                        if u64(allocation + 0x308) != allocation + 0x600:
                                            raise RuntimeError('Builder returned another pointer')
                                    if mode != 'insert':
                                        same(data + LAYOUT.serial_counter_offset, serial_before)
                                    else:
                                        same(allocation+0x7F0, canary)
                                        same(allocation+0x8E8, canary)
                                        if u64(allocation+0x328) != allocation+0x800:
                                            raise RuntimeError('Insertion returned another pointer')
                                    finish = True
                                api.set_context(event.tid, context)
                            else:
                                handled = False
                        else:
                            handled = False
                    if timed_out and not redirected:
                        error, finish = 'No accepted idle dispatch before timeout', True
                    if error and attached_ready and not redirected:
                        finish = True
                except Exception as exception:
                    error = str(exception)
                    finish = not redirected or acknowledged
                    if not finish:
                        with self.lock:
                            self.receipt['error'] = error
                            self._save()
                if finish:
                    api.restore_threads()
                    if allocation and (not redirected or acknowledged):
                        api.free(allocation)
                        allocation = None
                    api.resume(event, handled)
                    event = None
                    break
                api.resume(event, handled)
                event = None
            api.close()
            self.owner = None
            with self.lock:
                self.receipt.update(phase='completed' if acknowledged and not error else 'rejected',
                                    active=False, released=allocation is None, breakpoint_count=0, error=error)
                self._save()
        except Exception as exception:
            error = str(exception)
            # On an attached failure do not abandon suspended threads, their DRs,
            # or an allocation possibly executing. Retain explicit ownership.
            clean = api is None or not api.attached
            if clean and api:
                if allocation and not redirected:
                    api.free(allocation)
                    allocation = None
                api.close()
            with self.lock:
                self.receipt.update(phase='rejected' if not redirected else 'uncertain', error=error,
                                    active=not clean, released=clean and allocation is None,
                                    breakpoint_count=0 if clean else -1)
                self._save()
            # Keep the debugger thread alive while ownership is unresolved.
            # A stopped event is retained for retry; never let thread teardown
            # implicitly detach with modified debug registers.
            while not clean:
                try:
                    if event is None:
                        event = api.wait(100)
                        if event is None:
                            if not redirected or acknowledged:
                                api.require(api.dll.DebugBreakProcess(api.process), 'DebugBreakProcess')
                            continue
                    if event.code == 5:
                        # The process owns no executable pages after exit. Close
                        # our handles and settle ownership even after a failed
                        # context restoration or an unacknowledged redirect.
                        api.original_debug.clear()
                        for handle in api.threads.values():
                            api.dll.CloseHandle(handle)
                        api.threads.clear()
                        allocation = None
                        api.resume(event, True)
                        event = None
                        api.attached = False
                        api.close()
                        clean = True
                        continue
                    if not redirected or acknowledged:
                        if api.all_threads_exited():
                            # TerminateProcess can be waiting for our stopped
                            # event. No thread can execute the owned allocation;
                            # resume to receive EXIT_PROCESS_DEBUG_EVENT.
                            api.resume(event, True)
                            event = None
                            continue
                        api.restore_threads()
                        if allocation:
                            api.free(allocation)
                            allocation = None
                        api.resume(event, event.code != 1 or event.data.exception.record.code in (0x80000003, 0x80000004))
                        event = None
                        api.close()
                        clean = True
                    else:
                        handled = event.code != 1
                        if event.code == 1 and event.data.exception.record.code == 0x80000004:
                            context = api.context(event.tid)
                            if context.Rip in (entry, target) and context.Dr6 & 3:
                                if (context.Rip == target and event.tid == chosen_tid and before
                                        and context.Rsp == before['RSP'] - 0x48
                                        and api.read(allocation + 0x300, 4) == b'\1\0\0\0'):
                                    acknowledged = True
                                    continue
                                context.Dr6 &= ~3
                                context.EFlags |= 0x10000
                                api.set_context(event.tid, context)
                                handled = True
                        # An uncertain redirect retains its executable allocation
                        # until an acknowledgement or process exit is observed.
                        api.resume(event, handled)
                        event = None
                except Exception as cleanup_error:
                    with self.lock:
                        message = str(cleanup_error)
                        if self.receipt.get('cleanup_error') != message:
                            self.receipt['cleanup_error'] = message
                            self._save()
                    time.sleep(0.1)
            if clean:
                self.owner = None
                with self.lock:
                    self.receipt.update(active=False, released=True, breakpoint_count=0)
                    self._save()
