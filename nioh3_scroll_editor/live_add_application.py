"""Reviewed live additions from broker-owned, ready search candidates."""
import hashlib
import json
import os
from pathlib import Path
import struct
import threading
from uuid import uuid4

from .candidate_transfer import import_candidate
from .core_services import OperationPolicy, OperationCommand
from .models import CandidateRecordStage
from .live_add_adapter import CELiveAddAdapter
from .live_add_descriptor import assembly_descriptor, new_assembly_record
from .live_add_evidence import verify, verify_persistence, inventory_entries, index_entries
from .live_add_operations import LiveAddOperations, exclusive_json
from .search_application import require_search_candidate_ready
from .savegame import (SaveCrypto, default_crypto_tool, SCROLL_GROUP_OFFSET,
                       create_backup_directory, write_backup_manifest,
                       account_id_from_save_path, save_slot_index_from_path)


class LiveAddApplication:
    def __init__(self, state_root, context_digest, *, adapter=None, crypto=None):
        self.state_root = Path(state_root)
        self.operations = LiveAddOperations(Path(state_root) / 'live-add')
        self.context_digest = context_digest
        self.adapter = adapter
        self.crypto = crypto or SaveCrypto(default_crypto_tool(Path(__file__).resolve().parents[1]))
        self.lock = threading.RLock()

    def executor(self):
        if self.adapter is None:
            executor = os.environ.get('NIOH3_LIVE_ADD_EXECUTOR', 'native')
            if executor == 'native':
                from .live_add_native_adapter import NativeLiveAddAdapter
                self.adapter = NativeLiveAddAdapter(self.operations.root / 'native-executor')
            elif executor == 'ce':
                self.adapter = CELiveAddAdapter()
            else:
                raise ValueError('Unknown live-add executor')
        return self.adapter

    def validate_candidate(self, candidate):
        value = import_candidate(candidate, self.context_digest)
        require_search_candidate_ready(value)
        OperationPolicy().require(OperationCommand.INSTALL_GENERATED, candidate=value)
        if value.record_stage is not CandidateRecordStage.FINAL_RECORD:
            raise ValueError('Materialize and finalize the candidate before live addition')
        record = value.installation_record or value.record
        return value, new_assembly_record(record)

    def prepare(self, candidate, save_path, *, previous_operation_id=None):
        with self.lock:
            value, record = self.validate_candidate(candidate)
            adapter = self.executor()
            plan, before, index_before = adapter.inspect()
            for operation_id in self.operations.unresolved_ids():
                old = self.operations.plan(operation_id)['plan']
                if (old.get('pid'), old.get('process_creation_time')) == (plan['pid'], plan['process_creation_time']):
                    raise RuntimeError(f'Uncertain insertion {operation_id}; recover its receipt before preparing another')
            mapping = index_entries(index_before)
            if str(plan['serial']) in mapping or any(mapping.get(key) != item['slot_index'] for key, item in inventory_entries(before).items()):
                raise ValueError('Native serial index differs from inventory')
            operation_id = str(uuid4())
            # Backup lives outside the operation directory until the complete
            # plan is prepared. A failed preview never creates an executable plan.
            source = Path(save_path).resolve(strict=True)
            account_id_from_save_path(source)
            save_slot_index_from_path(source)
            directory = create_backup_directory(self.state_root)
            raw = source.read_bytes()
            with (directory / 'SAVEDATA.BIN').open('xb') as stream:
                stream.write(raw)
                stream.flush()
                os.fsync(stream.fileno())
            if (directory / 'SAVEDATA.BIN').read_bytes() != raw:
                raise OSError('Automatic save backup verification failed')
            if source.read_bytes() != raw:
                raise ValueError('Source save changed during backup')
            self.crypto.decrypt(directory / 'SAVEDATA.BIN', directory / 'decrypted.bin')
            saved = (directory / 'decrypted.bin').read_bytes()
            # Publish the same account/slot/hash manifest as offline operations,
            # so the existing backup manager can restore this checkpoint.
            write_backup_manifest(directory, source, [{
                'source_role': 'main_save', 'backup_file': 'SAVEDATA.BIN',
                'size': len(raw), 'sha256': hashlib.sha256(raw).hexdigest().upper(),
            }], action='v2-live-add', operation_id=operation_id)
            persistence_baseline = before
            if previous_operation_id is not None:
                previous = self.operations.snapshot(previous_operation_id)
                if previous['state'] != 'verified':
                    raise ValueError('Previous batch item has not been verified')
                parent = self.operations.plan(previous_operation_id)['plan']
                if parent['source_save_path'] != str(source) or parent['source_save_sha256'] != hashlib.sha256(raw).hexdigest():
                    raise ValueError('Batch source save changed')
                for field in ('pid', 'process_creation_time', 'profile_id', 'manager', 'data', 'scheduler_owner'):
                    if parent[field] != plan[field]:
                        raise ValueError('Batch process context changed')
                directory_before = self.operations.directory(previous_operation_id)
                verified_after = json.loads((directory_before / 'inventory-after.json').read_bytes())
                verified_index = json.loads((directory_before / 'index-after.json').read_bytes())
                for field in ('pid', 'entries', 'serial_counter', 'acquisition_order_counter', 'container_sha256'):
                    if before[field] != verified_after[field]:
                        raise ValueError('Inventory changed between batch items')
                if mapping != index_entries(verified_index):
                    raise ValueError('Native index changed between batch items')
                persistence_baseline = parent.get('persistence_baseline', parent['before'])
            verify_persistence(persistence_baseline, [saved[SCROLL_GROUP_OFFSET+i*232:SCROLL_GROUP_OFFSET+(i+1)*232] for i in range(400)])
            plan.update(parent_operation_id=operation_id, source_save_path=str(source), candidate_id=candidate['candidate_id'])
            preview = adapter.preview(plan, record)
            after_preview, after_index = adapter.readback()
            for field in ('pid', 'entries', 'serial_counter', 'acquisition_order_counter', 'container_sha256'):
                if before[field] != after_preview[field]:
                    raise ValueError('Preview changed inventory or its planning context expired')
            if index_entries(after_index) != mapping:
                raise ValueError('Native serial index changed during preview')
            plan.update(operation_id=operation_id, descriptor_hex=assembly_descriptor(record, allocate_serial=True).hex(),
                        expected_record_hex=record.hex(), before=before, index_before=index_before,
                        source_save_path=str(source), source_save_sha256=hashlib.sha256(raw).hexdigest(),
                        backup_path=str(directory / 'SAVEDATA.BIN'), candidate_id=candidate['candidate_id'],
                        persistence_baseline=persistence_baseline, previous_operation_id=previous_operation_id)
            exclusive_json(directory / 'preview.json', preview)
            snapshot = self.operations.prepare(operation_id, plan)
            return {**snapshot, 'seed': value.seed, 'rarity': value.rarity,
                    'count_before': len(before['entries']), 'backup_path': plan['backup_path'],
                    'instance_serial': str(plan['serial']), 'persistence': 'requires_normal_game_save'}

    def execute(self, operation_id, plan_digest):
        with self.lock:
            snapshot = self.operations.snapshot(operation_id)
            if snapshot['plan_digest'] != plan_digest:
                raise ValueError('Reviewed live-add plan digest differs')
            if snapshot['state'] in ('verified', 'rejected_before_dispatch'):
                return snapshot
            if snapshot['state'] != 'prepared':
                raise RuntimeError('Operation is cancelled or uncertain; do not replay it')
            plan = self.operations.plan(operation_id)['plan']
            adapter = self.executor()
            current, inventory, index = adapter.inspect()
            for field in ('pid', 'process_creation_time', 'profile_id', 'manager', 'data', 'serial', 'slot', 'scheduler_owner',
                          'function_address', 'container_hex', 'builder_code_hex', 'insertion_code_hex'):
                if current[field] != plan[field]:
                    raise ValueError('Prepared live-add plan expired; prepare a new plan')
            if hashlib.sha256(Path(plan['source_save_path']).read_bytes()).hexdigest() != plan['source_save_sha256']:
                raise ValueError('Save changed after preparation; prepare again')
            backup = Path(plan['backup_path'])
            if not backup.is_file() or hashlib.sha256(backup.read_bytes()).hexdigest() != plan['source_save_sha256']:
                raise ValueError('Automatic save backup is missing or changed; insertion was not dispatched')
            self.operations.claim(operation_id, plan_digest)
            try:
                result = adapter.insert(plan)
            except Exception as error:
                absent = getattr(adapter, 'submission_absent', None)
                if callable(absent) and absent(operation_id) is True:
                    return self.operations.complete(operation_id, {
                        'operation_id': operation_id, 'state': 'rejected_before_dispatch',
                        'redirect_count': 0, 'error': str(error),
                    })
                raise
            return self._finish(plan, result)

    def _finish(self, plan, execution):
        operation_id = plan['operation_id']
        if (execution.get('redirect_count') == 0 and execution.get('released') is True
                and execution.get('active') is False and execution.get('breakpoints') == []
                and execution.get('operation_id') == operation_id and execution.get('pid') == plan['pid']):
            return self.operations.complete(operation_id, {'operation_id': operation_id,
                'state': 'rejected_before_dispatch', 'redirect_count': 0, 'error': execution.get('error')})
        require_instance = getattr(self.executor(), 'require_process_instance', None)
        if callable(require_instance):
            require_instance(plan['pid'], plan['process_creation_time'])
        if execution.get('process_creation_time', plan['process_creation_time']) != plan['process_creation_time']:
            raise RuntimeError('Native receipt belongs to another process lifetime')
        after, index_after = self.executor().readback()
        if callable(require_instance) and any(value.get('process_creation_time') != plan['process_creation_time']
                                              for value in (after, index_after)):
            raise RuntimeError('PROCESS_INSTANCE_CHANGED: receipt and readback lifetimes differ')
        directory = self.operations.directory(operation_id)
        # Failed verification is evidence too. Keep each recovery attempt rather
        # than losing it, or later publishing success beside an older snapshot.
        attempt = directory / 'verification' / str(uuid4())
        attempt.mkdir(parents=True)
        payloads = (('execution', execution), ('inventory-after', after), ('index-after', index_after))
        for name, payload in payloads:
            exclusive_json(attempt / (name + '.json'), payload)
        try:
            result = verify(plan, execution, plan['before'], after, plan['index_before'], index_after)
        except Exception as error:
            error.add_note(f'Live verification evidence: {attempt}')
            raise
        # Batch chaining reads these fixed names; publish only verified snapshots.
        for name, payload in payloads:
            temporary = directory / (name + '-' + attempt.name + '.tmp')
            exclusive_json(temporary, payload)
            os.replace(temporary, directory / (name + '.json'))
        return self.operations.complete(operation_id, {**result, 'state': 'verified',
                                                      'verification_evidence': str(attempt)})

    def recover(self, operation_id):
        with self.lock:
            snapshot = self.operations.snapshot(operation_id)
            if snapshot['state'] != 'uncertain':
                return snapshot
            plan = self.operations.plan(operation_id)['plan']
            adapter = self.executor()
            if callable(getattr(adapter, 'require_process_instance', None)):
                execution = adapter.recover(operation_id, plan['pid'], plan['process_creation_time'])
            else:
                execution = adapter.recover(operation_id, plan['pid'])
            return self._finish(plan, execution)

    def cancel(self, operation_id):
        return self.operations.cancel(operation_id)

    def status(self, operation_id):
        return self.operations.snapshot(operation_id)

    def safe_to_shutdown(self):
        return self.adapter is None or self.adapter.safe_to_shutdown()
