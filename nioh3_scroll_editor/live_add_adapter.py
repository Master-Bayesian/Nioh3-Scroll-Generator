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
from .process_instance import original_process_exited, process_creation_time


class LiveAddAdapter:
    def __init__(self, transport):
        self.transport = transport
        self.pending = None
        self.pending_pid = None
        self.pending_creation_time = None

    def _clear_pending(self):
        self.pending = self.pending_pid = self.pending_creation_time = None

    def _refresh_pending(self):
        if self.pending is None:
            return
        try:
            value = self.transport.call('status', operation_id=self.pending)
            if (value.get('operation_id') == self.pending and value.get('released') is True
                    and value.get('active') is False and value.get('breakpoint_count') == 0):
                self._clear_pending()
                return
        except Exception:
            pass
        if self.pending_pid is not None and original_process_exited(
                self.pending_pid, self.pending_creation_time):
            self._clear_pending()

    def safe_to_shutdown(self):
        self._refresh_pending()
        return self.pending is None

    def submission_absent(self, operation_id):
        try:
            known = getattr(self.transport, 'operation_known', None)
            return callable(known) and known(operation_id) is False
        except Exception:
            return False

    @staticmethod
    def require_process_instance(pid, creation_time):
        if creation_time is None or process_creation_time(pid) != creation_time:
            raise RuntimeError('PROCESS_INSTANCE_CHANGED: do not verify an old receipt against a new game')

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
            creation_time = reader.creation_time()
            if any(value.get('process_creation_time') != creation_time for value in (inventory, index)):
                raise RuntimeError('PROCESS_INSTANCE_CHANGED: planning snapshots span game lifetimes')
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
                    'process_creation_time': creation_time,
                    'serial': serial, 'slot': slots[0], 'scheduler_owner': scheduler,
                    'function_address': base + profile.insertion_rva, 'container_hex': container.hex(),
                    'insertion_code_hex': reader.read(base + profile.insertion_rva, profile.insertion_size).hex(),
                    'builder_code_hex': reader.read(base + profile.builder_rva, profile.builder_size).hex()}
        return plan, inventory, index

    def wait(self, operation_id):
        if self.pending not in (None, operation_id):
            raise RuntimeError('Another native operation still owns the adapter')
        self.pending = operation_id
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = self.transport.call('status', operation_id=operation_id)
            if value.get('operation_id') != operation_id:
                raise RuntimeError('Native receipt operation identity differs')
            if (value.get('released') is True and value.get('active') is False
                    and value.get('breakpoint_count') == 0):
                self._clear_pending()
                return self.normalize(value)
            if value.get('phase') == 'completed' or not value.get('active', False):
                if value.get('redirect_count') == 0 or value.get('phase') == 'completed':
                    value = self.transport.call('release', operation_id=operation_id)
                    if (value.get('operation_id') == operation_id and value.get('released') is True
                            and value.get('active') is False and value.get('breakpoint_count') == 0):
                        self._clear_pending()
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

    def _submit(self, method, operation_id, owner_pid, **fields):
        """Own a request only after the transport accepts it.

        A rejected request has no native ownership only when the transport can
        prove that it never accepted this operation ID. Ambiguous transport
        failures keep ownership so an insertion can never be replayed.
        """

        self._refresh_pending()
        if self.pending is not None:
            raise RuntimeError('Another native operation still owns the adapter; recover it first')
        self.pending = operation_id
        self.pending_pid = owner_pid
        self.pending_creation_time = fields.get('process_creation_time')
        # The optional CE line protocol predates native receipt metadata and
        # only accepts flat identifier/hex tokens. Keep lifecycle metadata local
        # instead of serializing Windows paths/None into that accepted ABI.
        submitted_fields = fields
        if isinstance(self.transport, CELiveAddTransport):
            submitted_fields = {key: value for key, value in fields.items()
                                if key not in ('process_creation_time', 'source_save_path',
                                               'candidate_id', 'parent_operation_id')}
        try:
            return self.transport.call(method, operation_id=operation_id, **submitted_fields)
        except Exception:
            if self.submission_absent(operation_id):
                self._clear_pending()
            raise

    def preview(self, plan, installation_record):
        # A preview cannot mutate inventory or advance the serial counter. The
        # game's accepted idle dispatch is periodic, though, so a quiet window
        # can end one attempt before the breakpoint is reached. Retry only that
        # explicit, fully released, zero-redirect outcome. Writes are never
        # replayed and every attempt retains its own native receipt.
        for attempt in range(3):
            operation_id = str(uuid4())
            self._submit('preview', operation_id, plan['pid'], profile_id=plan['profile_id'], pid=plan['pid'],
                         process_creation_time=plan.get('process_creation_time'),
                         source_save_path=plan.get('source_save_path'),
                         candidate_id=plan.get('candidate_id'), parent_operation_id=plan.get('parent_operation_id'),
                         descriptor_hex=assembly_descriptor(installation_record).hex(),
                         expected_record_hex=installation_record.hex(), builder_code_hex=plan['builder_code_hex'])
            result = self.wait(operation_id)
            idle_miss = (result.get('redirect_count') == 0
                         and result.get('released') is True
                         and result.get('active') is False
                         and result.get('breakpoints') == []
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
        operation_id = plan['operation_id']
        fields.pop('operation_id')
        fields.update({key: plan.get(key) for key in ('process_creation_time', 'source_save_path', 'candidate_id', 'parent_operation_id')})
        self._submit('insert', operation_id, plan['pid'], **fields)
        return self.wait(plan['operation_id'])

    def readback(self):
        inventory, index = capture_inventory(), capture_index()
        creation_time = inventory.get('process_creation_time')
        if inventory['pid'] != index['pid'] or creation_time != index.get('process_creation_time'):
            raise RuntimeError('PROCESS_INSTANCE_CHANGED: readback snapshots span game lifetimes')
        self.require_process_instance(inventory['pid'], creation_time)
        return inventory, index

    def recover(self, operation_id, pid, process_creation_time=None):
        self._refresh_pending()
        if self.pending not in (None, operation_id):
            raise RuntimeError('Another native operation still owns the adapter')
        self.require_process_instance(pid, process_creation_time)
        self.pending_pid = pid
        self.pending_creation_time = process_creation_time
        return self.wait(operation_id)


class CELiveAddAdapter(LiveAddAdapter):
    """Optional compatibility transport for the previously accepted CE path."""

    def __init__(self, transport=None):
        super().__init__(transport or CELiveAddTransport())
