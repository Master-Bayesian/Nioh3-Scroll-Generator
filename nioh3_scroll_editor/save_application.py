"""Snapshot-bound save plans over the frozen transactional save adapter.

No GUI objects or renderer-provided record bytes enter this service. Generated
candidate transfer is a private broker command, separate from local free edits.
"""
from __future__ import annotations

from dataclasses import asdict
from pathlib import Path
import hashlib
import json
import os
import threading
import time
import uuid

from .candidate_transfer import import_candidate
from .core_services import CandidateApplicationService, OperationCommand
from .models import ScrollCandidate
from .scroll_input_metadata import record_input_metadata
from .savegame import (
    SaveCrypto, SaveInstaller, account_id_from_save_path, save_slot_index_from_path,
    default_crypto_tool, discover_save_paths, sha256_file, list_backup_entries,
    read_local_scroll_header, read_local_effect_slots, patch_local_scroll_header,
    patch_local_scroll_record, LocalEffectEdit, materialize_effect_sequence_candidate,
    prepare_candidate_for_install,
)


def json_value(value):
    """Convert path-bearing adapter result dataclasses to plain JSON values."""
    return json.loads(json.dumps(value, default=lambda item: str(item) if isinstance(item, Path) else asdict(item)))


class SaveApplication:
    def __init__(self, state_root: Path, *, crypto=None, service=None, game_process_ids=None):
        self.state_root = state_root.resolve()
        self.crypto = crypto or SaveCrypto(default_crypto_tool(Path(__file__).resolve().parents[1]))
        self.service = service or CandidateApplicationService()
        self.game_process_ids = game_process_ids
        self.saves = {}
        self.snapshots = {}
        self.plans = {}
        self.receipts = {}
        self.lock = threading.RLock()

    def _require_game_closed(self):
        if self.game_process_ids is None:
            return
        try:
            running = tuple(self.game_process_ids())
        except Exception as error:
            raise RuntimeError(
                'GAME_STATE_UNKNOWN: Could not verify that Nioh 3 is closed; no save write attempted'
            ) from error
        if running:
            raise RuntimeError(
                'GAME_RUNNING: Close Nioh 3 completely before writing the save; no save write attempted'
            )

    def register(self, path: str) -> dict:
        selected = Path(path).resolve(strict=True)
        if selected.name.upper() != 'SAVEDATA.BIN':
            raise ValueError('Select a character SAVEDATA.BIN')
        account = account_id_from_save_path(selected)
        slot = save_slot_index_from_path(selected)
        save_id = hashlib.sha256(str(selected).casefold().encode()).hexdigest()
        if save_id not in self.saves and len(self.saves) >= 16:
            raise ValueError('Save registry is full; restart the idle save worker')
        self.saves[save_id] = selected
        return {'save_id': save_id, 'path': str(selected), 'account_id': str(account), 'save_slot': slot}

    def data_directory(self, action, path):
        from .app_settings import load_app_settings, save_data_root, default_state_root
        root = Path(__file__).resolve().parents[1]
        if action == 'inspect':
            return {'data_directory': str(self.state_root), 'restart_required': False}
        if action == 'reset':
            selected = default_state_root(fallback_root=root)
        elif action == 'set' and path:
            selected = Path(path)
        else:
            raise ValueError('Invalid data directory action')
        settings = save_data_root(selected, fallback_root=root)
        # Active plans and receipts remain attached to their original root until restart.
        return {'data_directory': str(settings.data_root), 'restart_required': True}

    def auxiliary_preview(self, seed, playthrough):
        from .auxiliary_generation import generate_complete_auxiliary
        from .scroll_input_metadata import initial_challenge_capacity
        auxiliary = generate_complete_auxiliary(seed, playthrough)
        value = {'terrain': {'value': auxiliary.terrain.value, 'display_effect_keys': list(auxiliary.terrain.display_effect_keys)},
                 'enemy_groups': [[{'lookup_key': entry.lookup_key, 'role': entry.role} for entry in group.entries] for group in auxiliary.enemies.groups],
                 'special_rules': [{key: getattr(entry, key) for key in ('key', 'raw_value', 'display_value', 'display_unit', 'display_grade', 'qualifier_kind', 'qualifier_key')} for entry in auxiliary.special_rules.entries]}
        value['initial_challenge_capacity'] = initial_challenge_capacity(seed)
        return {'auxiliary_json': json.dumps(value)}

    def discover(self) -> dict:
        return {'saves': [self.register(str(path)) for path in discover_save_paths()[:16]]}

    def _installer(self, save_id):
        if save_id not in self.saves:
            raise ValueError('Unknown save ID; select or discover the save first')
        return SaveInstaller(save_path=self.saves[save_id], crypto=self.crypto, state_root=self.state_root)

    def live_add_source(self, save_id, snapshot_id):
        """Broker-only handoff of a registered, current save for backup checks."""
        self._snapshot(save_id, snapshot_id)
        return {'save_path': str(self.saves[save_id])}

    def count_edit_source(self, save_id, snapshot_id, slot_index):
        source_hash, inventory = self._snapshot(save_id, snapshot_id)
        entries = {entry.slot_index: entry for entry in inventory.scroll_entries()}
        if slot_index not in entries:
            raise ValueError('Select an occupied scroll slot')
        return {'count_source': {'save_path': str(self.saves[save_id]),
                                'source_sha256': source_hash, 'record_hex': entries[slot_index].record.hex()}}

    def inventory(self, save_id):
        installer = self._installer(save_id)
        before = sha256_file(installer.save_path)
        inventory = installer.capture_inventory(allow_empty=True)
        if before != sha256_file(installer.save_path):
            raise RuntimeError('Save changed while reading inventory')
        snapshot_id = uuid.uuid4().hex
        # Only the latest snapshot for each selected save is retained.
        self.snapshots[save_id] = (snapshot_id, before, inventory)
        entries = []
        for entry in inventory.scroll_entries():
            header = read_local_scroll_header(entry.record)
            entries.append({'slot_index': entry.slot_index,
                            'header': asdict(header),
                            'derived': record_input_metadata(entry.record, header),
                            'effects': [asdict(effect) for effect in read_local_effect_slots(entry.record)]})
        return {'save_id': save_id, 'snapshot_id': snapshot_id, 'source_sha256': before,
                'account_id': str(inventory.account_id), 'empty_slots': len(inventory.empty_slots), 'entries': entries}

    def _snapshot(self, save_id, snapshot_id):
        current = self.snapshots.get(save_id)
        if current is None or current[0] != snapshot_id:
            raise ValueError('Snapshot expired; refresh inventory')
        if sha256_file(self.saves[save_id]) != current[1]:
            raise RuntimeError('Save changed after preview; refresh inventory')
        return current[1], current[2]

    def _plan(self, save_id, source_hash, kind, data, preview):
        self._require_game_closed()
        self.plans = {key: value for key, value in self.plans.items() if value['expires'] > time.monotonic()}
        if len(self.plans) >= 32:
            raise ValueError('Too many uncommitted plans; discard or wait for expiry')
        plan_id = uuid.uuid4().hex
        self.plans[plan_id] = {'save_id': save_id, 'source_hash': source_hash, 'kind': kind,
                               'data': data, 'expires': time.monotonic() + 600}
        return {'plan_id': plan_id, 'save_id': save_id, 'kind': kind, 'source_sha256': source_hash,
                'expires_in_seconds': 600, 'preview': preview}

    def prepare_edit(self, save_id, snapshot_id, edits):
        source_hash, inventory = self._snapshot(save_id, snapshot_id)
        entries = {entry.slot_index: entry for entry in inventory.scroll_entries()}
        if len({edit['slot_index'] for edit in edits}) != len(edits):
            raise ValueError('A slot cannot be edited twice in one plan')
        replacements, preview = [], []
        for edit in edits:
            entry = entries.get(edit['slot_index'])
            if entry is None:
                raise ValueError('Only occupied scroll slots may be edited')
            header = edit['header']
            replacement = patch_local_scroll_header(entry.record, **header)
            replacement = patch_local_scroll_record(replacement, [LocalEffectEdit(**effect) for effect in edit['effects']])
            replacements.append((entry.slot_index, entry.record, replacement))
            preview.append({'slot_index': entry.slot_index,
                            'before_header': asdict(read_local_scroll_header(entry.record)), 'after_header': header,
                            'changed_offsets': [index for index, (a, b) in enumerate(zip(entry.record, replacement)) if a != b],
                            'before_effects': [asdict(effect) for effect in read_local_effect_slots(entry.record)],
                            'after_effects': [asdict(effect) for effect in read_local_effect_slots(replacement)]})
        return self._plan(save_id, source_hash, 'edit', replacements, {'changes': preview, 'local_only': True})

    def prepare_delete(self, save_id, snapshot_id, slots):
        source_hash, inventory = self._snapshot(save_id, snapshot_id)
        entries = {entry.slot_index: entry for entry in inventory.scroll_entries()}
        if len(set(slots)) != len(slots) or any(slot not in entries for slot in slots):
            raise ValueError('Delete requires distinct occupied scroll slots')
        edits = [(slot, entries[slot].record, bytes(len(entries[slot].record))) for slot in slots]
        return self._plan(save_id, source_hash, 'delete', edits, {'slots': slots, 'local_only': True})

    def prepare_install(self, save_id, snapshot_id, candidate, recommended_level, transfer_count):
        source_hash, inventory = self._snapshot(save_id, snapshot_id)
        source = import_candidate(candidate, self.service.context.context_digest)
        self.service.policy.require(OperationCommand.INSTALL_GENERATED, candidate=source)
        self.service.prepare_generated_install(source)
        if source.can_materialize_for_install:
            materialized = materialize_effect_sequence_candidate(inventory, source, level=candidate['level'],
                                                                 recommended_level=recommended_level, transfer_count=transfer_count)
            record = materialized.record
        else:
            record = source.installation_record or source.record
            if read_local_scroll_header(record).recommended_level != recommended_level:
                raise ValueError('Recommended level differs from the native candidate; regenerate before installation')
        return self._plan(save_id, source_hash, 'install', (record, transfer_count), {
            'candidate_id': candidate['candidate_id'], 'seed': source.seed, 'rarity': source.rarity,
            'playthrough': source.playthrough, 'level': candidate['level'],
            'recommended_level': recommended_level, 'transfer_count': transfer_count,
            'installation_sha256': hashlib.sha256(record).hexdigest(),
            'custom_only': source.playthrough in (1, 2) and source.rarity == 4,
        })

    def materialize_live_many(self, save_id, snapshot_id, candidates, recommended_level, transfer_count):
        """Broker-only live input, preserving the finalized/stage-one pair."""
        from dataclasses import replace
        from .candidate_transfer import export_candidate
        from .effect_sequence import materialize_ng3_certified_record
        from .models import ScrollCandidate
        from .savegame import next_generation_serial
        if not 1 <= len(candidates) <= 200 or len({c['candidate_id'] for c in candidates}) != len(candidates):
            raise ValueError('Expected 1-200 distinct candidates')
        with self.lock:
            _source_hash, inventory = self._snapshot(save_id, snapshot_id)
            result = []
            for payload in candidates:
                source = import_candidate(payload, self.service.context.context_digest)
                self.service.policy.require(OperationCommand.INSTALL_GENERATED, candidate=source)
                self.service.prepare_generated_install(source)
                if source.can_materialize_for_install:
                    installed = materialize_effect_sequence_candidate(inventory, source, level=payload['level'],
                        recommended_level=recommended_level, transfer_count=transfer_count)
                    finalized, _ = materialize_ng3_certified_record(inventory.template_record_for_playthrough(3),
                        seed=source.seed, rarity=source.rarity, level=payload['level'], recommended_level=recommended_level,
                        transfer_count=transfer_count, generation_serial=next_generation_serial(inventory))
                    value = replace(ScrollCandidate.from_record(finalized, playthrough=3), installation_record=installed.record)
                else:
                    if read_local_scroll_header(source.record).recommended_level != recommended_level:
                        raise ValueError('Regenerate candidate with the selected recommended level')
                    value = replace(source, record=prepare_candidate_for_install(source.record, transfer_count=transfer_count),
                        installation_record=prepare_candidate_for_install(source.installation_record, transfer_count=transfer_count) if source.installation_record else None)
                result.append(export_candidate(value, self.service.context.context_digest, payload['level']))
            return {'candidates': result, 'save_path': str(self._installer(save_id).save_path)}

    def prepare_install_many(self, save_id, snapshot_id, candidates, recommended_level, transfer_count):
        if not 1 <= len(candidates) <= 200:
            raise ValueError('Batch size must be 1-200')
        if len({candidate['candidate_id'] for candidate in candidates}) != len(candidates):
            raise ValueError('Duplicate candidate identity')
        with self.lock:
            source_hash, _ = self._snapshot(save_id, snapshot_id)
            records, previews = [], []
            for candidate in candidates:
                child = self.prepare_install(save_id, snapshot_id, candidate, recommended_level, transfer_count)
                plan = self.plans.pop(child['plan_id'])
                record, count = plan['data']
                records.append(prepare_candidate_for_install(record, transfer_count=count))
                previews.append(child['preview'])
            return self._plan(save_id, source_hash, 'install_many', tuple(records), {'count': len(records), 'items': previews})

    def backups(self, save_id):
        path = self._installer(save_id).save_path
        account, slot = account_id_from_save_path(path), save_slot_index_from_path(path)
        return {'backups': [{'backup_id': entry.directory.name, 'timestamp': entry.timestamp,
                              'action': entry.action, 'manifest_schema': entry.manifest_schema,
                              'file_count': entry.file_count}
                             for entry in list_backup_entries(self.state_root)
                             if entry.account_id == account and entry.save_slot_index == slot][:256]}

    def recycle_backups(self, save_id, backup_ids):
        from .savegame import move_backup_to_recycle_bin
        with self.lock:
            allowed = {item['backup_id'] for item in self.backups(save_id)['backups']}
            if not backup_ids or not set(backup_ids).issubset(allowed):
                raise ValueError('Select backups belonging to this save')
            for backup_id in dict.fromkeys(backup_ids):
                move_backup_to_recycle_bin(self.state_root, self.state_root / 'backups' / backup_id)
            return self.backups(save_id)

    def backup_location(self):
        directory = self.state_root / 'backups'
        directory.mkdir(parents=True, exist_ok=True)
        return {'backup_directory': str(directory)}

    def prepare_restore(self, save_id, snapshot_id, backup_id):
        source_hash, _inventory = self._snapshot(save_id, snapshot_id)
        allowed = {entry['backup_id'] for entry in self.backups(save_id)['backups']}
        if backup_id not in allowed:
            raise ValueError('Backup does not belong to the selected save')
        directory = self.state_root / 'backups' / backup_id
        return self._plan(save_id, source_hash, 'restore', directory, {'backup_id': backup_id})

    def template(self, save_id, snapshot_id, playthrough):
        source_hash, inventory = self._snapshot(save_id, snapshot_id)
        return {'template_hex': inventory.template_record_for_playthrough(playthrough).hex(),
                'save_fingerprint': hashlib.sha256(inventory.decrypted).hexdigest(),
                'source_sha256': source_hash.lower(), 'context_digest': self.service.context.context_digest}

    def cached_grace(self, save_id, snapshot_id, playthrough, rarity):
        from .cache_application import grace_map_cache_path
        from .grace_map import load_grace_map_cache, grace_map_to_cache_payload
        _source_hash, inventory = self._snapshot(save_id, snapshot_id)
        fingerprint = hashlib.sha256(inventory.decrypted).hexdigest()
        digest = self.service.context.context_digest
        path = grace_map_cache_path(self.state_root, save_fingerprint=fingerprint,
                                    playthrough=playthrough, rarity=rarity, generation_context_digest=digest)
        mapping = load_grace_map_cache(path, expected_context_fingerprint=fingerprint,
                                       expected_generation_context_digest=digest)
        return {'cache_json': json.dumps(grace_map_to_cache_payload(mapping,
            context_fingerprint=fingerprint, generation_context_digest=digest))}

    def discard(self, plan_id):
        self.plans.pop(plan_id, None)
        return {'discarded': True}

    def _ledger_path(self, plan_id):
        if len(plan_id) != 32 or any(c not in '0123456789abcdef' for c in plan_id):
            raise ValueError('Invalid operation ID')
        return self.state_root / 'v2-operations' / f'{plan_id}.json'

    def _write_receipt(self, plan_id, result):
        path = self._ledger_path(plan_id)
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_suffix('.tmp')
        with temporary.open('w', encoding='utf-8') as handle:
            json.dump(result, handle); handle.flush(); os.fsync(handle.fileno())
        os.replace(temporary, path)

    def operation(self, plan_id):
        if plan_id in self.receipts:
            return self.receipts[plan_id]
        path = self._ledger_path(plan_id)
        if not path.is_file():
            raise ValueError('Unknown operation ID')
        result = json.loads(path.read_text(encoding='utf-8'))
        if result['commit_status'] == 'executing':
            result.update(commit_status='unknown', warning='Previous process ended before recording the outcome; inspect backups and current save before further writes')
        return result

    def operations(self, save_id):
        self._installer(save_id)
        folder = self.state_root / 'v2-operations'
        results = []
        for path in sorted(folder.glob('*.json'), key=lambda item: item.stat().st_mtime, reverse=True):
            try:
                result = self.operation(path.stem)
            except (ValueError, OSError):
                continue
            if result.get('save_id') == save_id:
                results.append(result)
            if len(results) == 128:
                break
        return {'operations': results}

    def commit(self, plan_id):
        with self.lock:
            if plan_id in self.receipts or self._ledger_path(plan_id).exists():
                return self.operation(plan_id)
            plan = self.plans.get(plan_id)
            if not plan or plan['expires'] <= time.monotonic():
                raise ValueError('Plan expired; prepare a new plan')
            self._require_game_closed()
            installer = self._installer(plan['save_id'])
            if sha256_file(installer.save_path) != plan['source_hash']:
                raise RuntimeError('Save changed after preparation; no write attempted')
            result = {'operation_id': plan_id, 'save_id': plan['save_id'],
                      'commit_status': 'executing', 'warning': None, 'details': {}}
            self._write_receipt(plan_id, result)  # Durable intent precedes every write.
            try:
                if plan['kind'] in ('edit', 'delete'):
                    outcome = installer.edit_many(plan['data'], action=f'v2-local-{plan["kind"]}', expected_source_sha256=plan['source_hash'])
                elif plan['kind'] == 'restore':
                    outcome = installer.restore_backup(plan['data'], expected_source_sha256=plan['source_hash'])
                elif plan['kind'] == 'install_many':
                    outcome = installer.install_many(plan['data'], action='v2-cart-install', expected_source_sha256=plan['source_hash'])
                else:
                    record, transfer_count = plan['data']
                    outcome = installer.install(record, transfer_count=transfer_count, expected_source_sha256=plan['source_hash'])
                result.update(commit_status=outcome.commit_status, warning=outcome.warning, details=json_value(outcome))
            except Exception as error:
                try:
                    unchanged = sha256_file(installer.save_path) == plan['source_hash']
                except OSError:
                    unchanged = False
                # Restore may have touched siblings even when the main file is unchanged.
                result.update(commit_status='not_committed' if unchanged and plan['kind'] != 'restore' else 'unknown', warning=str(error))
            self.receipts[plan_id] = result
            if len(self.receipts) > 128:
                self.receipts.pop(next(iter(self.receipts)))
            self.plans.pop(plan_id, None)
            self.snapshots.pop(plan['save_id'], None)
            try:
                self._write_receipt(plan_id, result)
            except OSError as error:
                if result['commit_status'] == 'committed':
                    result['commit_status'] = 'committed_with_warning'
                result['warning'] = f'{result["warning"] or ""} Operation ledger update failed: {error}'.strip()
            return result
