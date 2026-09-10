"""PC v2.01 inspection and optional CE execution behind an application boundary."""
import hashlib
import json
from pathlib import Path
import time
from uuid import uuid4

from .dispatch_evidence import verify_dispatch
from .live_add_ce_transport import CELiveAddTransport
from .live_add_descriptor import assembly_descriptor, verify_assembly_preview
from .live_add_profile import live_add_profile
from .live_inventory import capture_inventory, capture_index
from .process_memory_readonly import ProcessReader


class LiveAddAdapter:
    def __init__(self, transport):
        self.transport = transport
        self.pending = None
        self.pending_pid = None

    def safe_to_shutdown(self):
        if self.pending is None:
            return True
        if self.pending_pid is None:
            return False
        import ctypes
        from ctypes import wintypes
        dll = ctypes.WinDLL('kernel32', use_last_error=True)
        dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        dll.OpenProcess.restype = wintypes.HANDLE
        dll.CloseHandle.argtypes = [wintypes.HANDLE]
        dll.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
        handle = dll.OpenProcess(0x1000, False, self.pending_pid)
        if not handle:
            return ctypes.get_last_error() == 87  # PID no longer exists; access denial is not exit.
        try:
            code = wintypes.DWORD()
            return bool(dll.GetExitCodeProcess(handle, ctypes.byref(code))) and code.value != 259
        finally:
            dll.CloseHandle(handle)

    def identity(self):
        from .runtime_application import running_game_identity
        pid, profile, path = running_game_identity()
        if profile.display_version != 'PC v2.01':
            raise ValueError('Live addition requires accepted PC v2.01')
        return pid, live_add_profile((2, 0, 1, 0))

    def inspect(self):
        pid, profile = self.identity()
        endpoint = self.transport.call('ping')
        if endpoint['pid'] != pid or endpoint['profile_id'] != profile.profile_id or endpoint['busy']:
            raise RuntimeError('Live-add executor is busy or attached to a different process/profile')
        inventory, index = capture_inventory(), capture_index()
        if inventory['pid'] != pid or index['pid'] != pid:
            raise RuntimeError('Process changed during inspection')
        if not 0 <= inventory['acquisition_order_counter'] < 0xFFFFFFFF:
            raise ValueError('Acquisition-order counter cannot be advanced without overflow')
        with ProcessReader() as reader:
            if reader.pid != pid:
                raise RuntimeError('Process changed')
            base = reader.module_base
            identities = json.loads((Path(__file__).parent / 'data/live_add_pc_v201_identity.json').read_bytes())
            if identities['profile_id'] != profile.profile_id:
                raise ValueError('Code identity profile differs')
            for site in identities['ranges']:
                if hashlib.sha256(reader.read(base + site['rva'], site['size'])).hexdigest() != site['sha256']:
                    raise RuntimeError(f"Loaded native code differs at RVA {site['rva']:#x}; remove the conflicting modification")
            if reader.read(base + profile.dispatch_rva, 7).hex().upper() != profile.dispatch_signature_hex.upper():
                raise RuntimeError('Dispatch instructions differ')
            manager = reader.u64(base + profile.manager_pointer_rva)
            data = reader.u64(manager)
            container = reader.read(data + profile.container_offset, profile.capacity * profile.record_size)
            if hashlib.sha256(container).hexdigest() != inventory['container_sha256']:
                raise RuntimeError('Inventory changed during planning')
            serial = reader.u64(data + profile.serial_counter_offset)
            if str(serial) != inventory['serial_counter'] or not 0 < serial < 0x7FFFFFFFFFFFFFFE:
                raise ValueError('Serial changed or exceeds this executor ABI range')
            slots = [i for i in range(profile.capacity) if container[i*profile.record_size:i*profile.record_size+2] == b'\0\0']
            if not slots:
                raise ValueError('Scroll inventory is full')
            scheduler = reader.u64(base + profile.scheduler_pointer_rva)
            # Normal frames briefly set these flags even with an idle character.
            # Wait read-only for a bounded window; the stopped dispatch event
            # still rechecks the full inventory, serial and exact idle flags.
            deadline = time.monotonic() + 0.5
            while reader.read(scheduler + profile.scheduler_pending_offset, 4) != bytes(4) or reader.read(scheduler + profile.scheduler_ready_offset, 1) != b'\1':
                if time.monotonic() >= deadline:
                    raise RuntimeError('Mission scheduler is not in the accepted idle phase')
                if reader.u64(base + profile.scheduler_pointer_rva) != scheduler:
                    raise RuntimeError('Mission scheduler ownership changed')
                time.sleep(0.02)
            plan = {'pid': pid, 'profile_id': profile.profile_id, 'manager': manager, 'data': data,
                    'process_creation_time': reader.creation_time(),
                    'serial': serial, 'slot': slots[0], 'scheduler_owner': scheduler,
                    'function_address': base + profile.insertion_rva, 'container_hex': container.hex(),
                    'insertion_code_hex': reader.read(base + profile.insertion_rva, profile.insertion_size).hex(),
                    'builder_code_hex': reader.read(base + profile.builder_rva, profile.builder_size).hex()}
        return plan, inventory, index

    def wait(self, operation_id):
        self.pending = operation_id
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = self.transport.call('status', operation_id=operation_id)
            if value.get('released') is True and not value.get('active', False):
                self.pending = None
                return self.normalize(value)
            if value.get('phase') == 'completed' or not value.get('active', False):
                if value.get('redirect_count') == 0 or value.get('phase') == 'completed':
                    value = self.transport.call('release', operation_id=operation_id)
                    if value.get('released'):
                        self.pending = None
                    return self.normalize(value)
                raise RuntimeError('Native dispatch result is uncertain; allocation retained, do not retry')
            time.sleep(0.05)
        raise TimeoutError('Native dispatch did not settle; query its receipt before any further operation')

    @staticmethod
    def normalize(value):
        value = dict(value)
        count = value.pop('breakpoint_count', -1)
        value['breakpoints'] = [] if count == 0 else ['unconfirmed']
        return value

    def preview(self, plan, installation_record):
        # A preview cannot mutate inventory or advance the serial counter. The
        # game's accepted idle dispatch is periodic, though, so a quiet window
        # can end one attempt before the breakpoint is reached. Retry only that
        # explicit, fully released, zero-redirect outcome. Writes are never
        # replayed and every attempt retains its own native receipt.
        for attempt in range(3):
            operation_id = str(uuid4())
            self.pending = operation_id
            self.pending_pid = plan['pid']
            self.transport.call('preview', operation_id=operation_id, profile_id=plan['profile_id'], pid=plan['pid'],
                                descriptor_hex=assembly_descriptor(installation_record).hex(),
                                expected_record_hex=installation_record.hex(), builder_code_hex=plan['builder_code_hex'])
            result = self.wait(operation_id)
            idle_miss = (result.get('redirect_count') == 0
                         and result.get('released') is True
                         and result.get('active') is False
                         and result.get('error') == 'No accepted idle dispatch before timeout')
            if idle_miss and attempt < 2:
                continue
            verify_dispatch(result)
            verify_assembly_preview(installation_record, bytes.fromhex(result['source_hex']))
            return result
        raise AssertionError('Preview retry loop did not return')

    def insert(self, plan):
        fields = {key: plan[key] for key in ('operation_id', 'profile_id', 'pid', 'manager', 'data',
                  'serial', 'slot', 'scheduler_owner', 'function_address', 'container_hex', 'insertion_code_hex',
                  'builder_code_hex', 'descriptor_hex', 'expected_record_hex')}
        self.pending = plan['operation_id']
        self.pending_pid = plan['pid']
        self.transport.call('insert', **fields)
        return self.wait(plan['operation_id'])

    def readback(self):
        return capture_inventory(), capture_index()

    def recover(self, operation_id, pid):
        self.pending_pid = pid
        return self.wait(operation_id)


class CELiveAddAdapter(LiveAddAdapter):
    """Optional compatibility transport for the previously accepted CE path."""

    def __init__(self, transport=None):
        super().__init__(transport or CELiveAddTransport())
