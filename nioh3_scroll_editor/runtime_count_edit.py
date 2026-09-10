"""Reviewed instance-scoped remaining-count writes; no seed or capacity edits."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import threading
from uuid import UUID, uuid4

from .live_add_operations import canonical, exclusive_json
from .live_add_profile import PC_V201 as LAYOUT
from .live_inventory import capture_inventory
from .process_memory_readonly import ProcessReader
from .runtime_auxiliary_override import _kernel32
from .savegame import create_backup_directory, write_backup_manifest


def checked_count(value):
    if type(value) is not int or not 0 <= value <= 7:
        raise ValueError('Remaining count must be an integer from 0 to 7')
    return value


def stable_identity(raw):
    value = bytearray(raw)
    value[0x18] &= ~2  # Viewing a scroll can clear its new-item marker.
    value[0x33] = 0  # Live count can differ from the last saved checkpoint.
    return bytes(value[:0x24] + value[0x28:0xE4])


class WindowsCountMemory:
    def capture(self, serial):
        inventory = capture_inventory()  # PC v2.01 and function signature gate.
        matches = [entry for entry in inventory['entries'] if entry['serial'] == serial]
        if len(matches) != 1:
            raise ValueError('Scroll instance is no longer in the current inventory')
        entry = matches[0]
        with ProcessReader() as reader:
            if reader.pid != inventory['pid']:
                raise ValueError('Game process changed')
            manager = reader.u64(reader.module_base + LAYOUT.manager_pointer_rva)
            data = reader.u64(manager)
            address = data + LAYOUT.container_offset + entry['slot_index'] * 0xE8
            if reader.read(address, 0xE8).hex() != entry['record_hex']:
                raise ValueError('Scroll changed during inspection')
            return {'pid': reader.pid, 'creation_time': reader.creation_time(),
                    'manager': manager, 'data': data, 'address': address,
                    'record_hex': entry['record_hex'], 'serial': serial}

    def write(self, expected, desired):
        # Re-open with minimal write rights only after all planning gates pass.
        with ProcessReader() as reader:
            if reader.pid != expected['pid'] or reader.creation_time() != expected['creation_time']:
                raise ValueError('Game process changed before count write')
            if reader.u64(reader.module_base + LAYOUT.manager_pointer_rva) != expected['manager'] or reader.u64(expected['manager']) != expected['data']:
                raise ValueError('Inventory owner changed before count write')
            if reader.read(expected['address'], 0xE8).hex() != expected['record_hex']:
                raise ValueError('Scroll changed before count write')
            dll = _kernel32()
            handle = dll.OpenProcess(0x1000 | 0x20 | 0x8, False, reader.pid)
            if not handle:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                value, written = ctypes.c_ubyte(desired), ctypes.c_size_t()
                if not dll.WriteProcessMemory(handle, expected['address'] + 0x33, ctypes.byref(value), 1, ctypes.byref(written)) or written.value != 1:
                    raise ctypes.WinError(ctypes.get_last_error())
                return reader.read(expected['address'], 0xE8)
            finally:
                dll.CloseHandle(handle)


class RuntimeCountEditor:
    def __init__(self, root, *, memory=None):
        self.root = Path(root)
        self.operations = self.root / 'count-edits'
        self.operations.mkdir(parents=True, exist_ok=True)
        self.memory = memory or WindowsCountMemory()
        self.lock = threading.RLock()

    def directory(self, operation_id):
        if str(UUID(operation_id)) != operation_id:
            raise ValueError('Expected canonical operation UUID')
        return self.operations / operation_id

    def plan(self, operation_id):
        envelope = json.loads((self.directory(operation_id) / 'plan.json').read_bytes())
        if hashlib.sha256(canonical(envelope['plan'])).hexdigest() != envelope['digest']:
            raise ValueError('Count plan changed')
        return envelope

    def prepare(self, source, new_count):
        checked_count(new_count)
        with self.lock:
            saved_record = bytes.fromhex(source['record_hex'])
            if len(saved_record) != 0xE8:
                raise ValueError('Expected full saved record')
            serial = str(int.from_bytes(saved_record[0x28:0x30], 'little'))
            state = self.memory.capture(serial)
            current = bytes.fromhex(state['record_hex'])
            if stable_identity(current) != stable_identity(saved_record):
                raise ValueError('Saved defined record fields differ')
            if current[0x0E] != 0:
                raise ValueError('Current scroll state is not supported for count editing')
            path = Path(source['save_path']).resolve(strict=True)
            raw = path.read_bytes()
            if hashlib.sha256(raw).hexdigest().lower() != source['source_sha256'].lower():
                raise ValueError('Save changed; refresh inventory')
            backup = create_backup_directory(self.root)
            with (backup / 'SAVEDATA.BIN').open('xb') as stream:
                stream.write(raw); stream.flush(); os.fsync(stream.fileno())
            if (backup / 'SAVEDATA.BIN').read_bytes() != raw or path.read_bytes() != raw:
                raise ValueError('Automatic count backup verification failed')
            operation_id = str(uuid4())
            write_backup_manifest(backup, path, [{'source_role': 'main_save', 'backup_file': 'SAVEDATA.BIN', 'size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest().upper()}], action='v2-count-edit', operation_id=operation_id)
            plan = {'operation_id': operation_id, 'target': state, 'new_count': new_count,
                    'old_count': current[0x33], 'seed': int.from_bytes(current[0x20:0x24], 'little'),
                    'rarity': current[0x30], 'save_path': str(path), 'source_sha256': hashlib.sha256(raw).hexdigest(),
                    'backup_path': str(backup / 'SAVEDATA.BIN')}
            directory = self.directory(operation_id); directory.mkdir()
            exclusive_json(directory / 'plan.json', {'digest': hashlib.sha256(canonical(plan)).hexdigest(), 'plan': plan})
            return self.status(operation_id)

    def status(self, operation_id):
        envelope = self.plan(operation_id); plan = envelope['plan']; directory = self.directory(operation_id)
        state, error = 'prepared', None
        if (directory / 'claim.json').exists():
            claim = json.loads((directory / 'claim.json').read_bytes())
            if claim['digest'] != envelope['digest']:
                raise ValueError('Count claim differs')
            state = 'uncertain'
        if (directory / 'receipt.json').exists():
            receipt = json.loads((directory / 'receipt.json').read_bytes())
            if receipt['digest'] != envelope['digest'] or state != 'uncertain':
                raise ValueError('Count receipt differs')
            state, error = receipt['state'], receipt.get('error')
        if (directory / 'recovery.json').exists():
            recovery = json.loads((directory / 'recovery.json').read_bytes())
            if recovery['digest'] != envelope['digest'] or state != 'uncertain' or recovery['state'] != 'verified':
                raise ValueError('Count recovery differs')
            state, error = 'verified', None
        return {'count_edit': {'operation_id': operation_id, 'plan_digest': envelope['digest'], 'state': state,
                              'seed': plan['seed'], 'rarity': plan['rarity'], 'old_count': plan['old_count'],
                              'new_count': plan['new_count'], 'error': error}}

    def recover(self, operation_id):
        """Verify the requested state after a lost reply; never execute a write."""
        with self.lock:
            if self.status(operation_id)['count_edit']['state'] != 'uncertain':
                return self.status(operation_id)
            envelope = self.plan(operation_id); plan = envelope['plan']
            if hashlib.sha256(Path(plan['backup_path']).read_bytes()).hexdigest() != plan['source_sha256']:
                raise ValueError('Automatic backup changed; retain uncertain receipt')
            current = self.memory.capture(plan['target']['serial'])
            for field in ('pid', 'creation_time', 'manager', 'data', 'address', 'serial'):
                if current[field] != plan['target'][field]:
                    raise ValueError('Count recovery process or instance differs')
            raw = bytes.fromhex(current['record_hex'])
            if raw[0x33] != plan['new_count'] or stable_identity(raw) != stable_identity(bytes.fromhex(plan['target']['record_hex'])):
                return self.status(operation_id)
            exclusive_json(self.directory(operation_id) / 'recovery.json',
                           {'digest': envelope['digest'], 'state': 'verified', 'record_hex': raw.hex()})
            return self.status(operation_id)

    def execute(self, operation_id, plan_digest):
        with self.lock:
            envelope = self.plan(operation_id); plan = envelope['plan']; directory = self.directory(operation_id)
            if plan_digest != envelope['digest']:
                raise ValueError('Reviewed count plan digest differs')
            previous = self.status(operation_id)['count_edit']
            if previous['state'] != 'prepared':
                return {'count_edit': previous}  # No duplicate or uncertain replay.
            exclusive_json(directory / 'claim.json', {'digest': plan_digest})
            attempted, state, error = False, 'rejected', None
            try:
                for path in (plan['save_path'], plan['backup_path']):
                    if hashlib.sha256(Path(path).read_bytes()).hexdigest() != plan['source_sha256']:
                        raise ValueError('Save or automatic backup changed; prepare again')
                current = self.memory.capture(plan['target']['serial'])
                if current != plan['target']:
                    raise ValueError('Scroll or game state changed; prepare again')
                attempted = True
                after = self.memory.write(current, plan['new_count'])
                expected = bytearray.fromhex(current['record_hex']); expected[0x33] = plan['new_count']
                if after != bytes(expected):
                    raise ValueError('Count write readback differs; inspect before retrying')
                exclusive_json(directory / 'verified-record.json', {'record_hex': after.hex()})
                state = 'verified'
            except Exception as exception:
                state, error = ('uncertain' if attempted else 'rejected'), str(exception)
            exclusive_json(directory / 'receipt.json', {'digest': plan_digest, 'state': state, 'error': error})
            return self.status(operation_id)
