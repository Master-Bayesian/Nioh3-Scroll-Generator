"""Live-game ownership kept outside the killable offline search worker."""
from __future__ import annotations

from dataclasses import asdict, replace
from pathlib import Path
import ctypes
from ctypes import wintypes
import threading

from .candidate_transfer import export_candidate
from .core_services import CandidateApplicationService
from .game_compatibility import verify_game_executable
from .native import NativeBatchOracle, find_nioh3_pid, native_runtime_profile_for_game_version, scan_next_candidate
from .runtime_auxiliary_override import RuntimeAuxiliaryOverrideProfile, RuntimeAuxiliaryOverrideSession
from .search_application import require_search_candidate_ready


def running_game_identity():
    pid = find_nioh3_pid()
    dll = ctypes.WinDLL('kernel32', use_last_error=True)
    dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    dll.OpenProcess.restype = wintypes.HANDLE
    dll.CloseHandle.argtypes = [wintypes.HANDLE]
    dll.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
    handle = dll.OpenProcess(0x1000, False, pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        buffer = ctypes.create_unicode_buffer(32768)
        size = wintypes.DWORD(len(buffer))
        if not dll.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
            raise ctypes.WinError(ctypes.get_last_error())
        status = verify_game_executable(Path(buffer.value))
        if not status.supported or status.file_version is None:
            raise ValueError('Running game executable is not a verified supported version')
        return pid, native_runtime_profile_for_game_version(status.file_version), buffer.value
    finally:
        dll.CloseHandle(handle)


class RuntimeApplication:
    def __init__(self, *, service=None, identity=running_game_identity, oracle_factory=NativeBatchOracle,
                 override_factory=RuntimeAuxiliaryOverrideSession):
        self.service = service or CandidateApplicationService()
        self.identity = identity
        self.oracle_factory = oracle_factory
        self.override_factory = override_factory
        self.session = None
        self.retired_oracles = []
        self.candidates = {}
        self.lock = threading.RLock()
        self.live_add = None
        self.count_editor = None

    def _count_editor(self):
        if self.count_editor is None:
            import os
            from .app_settings import load_app_settings
            from .runtime_count_edit import RuntimeCountEditor
            root = Path(os.environ.get('NIOH3_STATE_ROOT') or load_app_settings(fallback_root=Path(__file__).resolve().parents[1]).data_root)
            self.count_editor = RuntimeCountEditor(root)
        return self.count_editor

    def count_prepare(self, source, new_count):
        with self.lock:
            if not self.status()['safe_to_shutdown']:
                raise ValueError('Stop temporary overrides before editing remaining count')
            return self._count_editor().prepare(source, new_count)

    def count_execute(self, operation_id, plan_digest):
        with self.lock:
            if not self.status()['safe_to_shutdown']:
                raise ValueError('Stop temporary overrides before editing remaining count')
            return self._count_editor().execute(operation_id, plan_digest)

    def count_recover(self, operation_id):
        return self._count_editor().recover(operation_id)

    def count_status(self, operation_id):
        return self._count_editor().status(operation_id)

    def _live_add(self):
        if self.live_add is None:
            import os
            from .app_settings import load_app_settings
            from .live_add_application import LiveAddApplication
            root = Path(os.environ.get('NIOH3_STATE_ROOT') or load_app_settings(fallback_root=Path(__file__).resolve().parents[1]).data_root)
            self.live_add = LiveAddApplication(root, self.service.context.context_digest)
        return self.live_add

    def live_add_prepare(self, candidate, save_path):
        if not self.status()['safe_to_shutdown']:
            raise RuntimeError('Live addition requires an idle runtime host')
        return {'live_add': self._live_add().prepare(candidate, save_path)}

    def live_add_execute(self, operation_id, plan_digest):
        if not self.status()['safe_to_shutdown']:
            raise RuntimeError('Resolve existing runtime ownership before insertion')
        return {'live_add': self._live_add().execute(operation_id, plan_digest)}

    def live_add_status(self, operation_id):
        return {'live_add': self._live_add().status(operation_id)}

    def live_add_recover(self, operation_id):
        return {'live_add': self._live_add().recover(operation_id)}

    def live_add_cancel(self, operation_id):
        return {'live_add': self._live_add().cancel(operation_id)}

    def _live_batch(self):
        from .live_add_batch import LiveAddBatch
        return LiveAddBatch(self._live_add())

    def live_batch_prepare(self, candidates, save_path):
        if not self.status()['safe_to_shutdown']:
            raise RuntimeError('Live addition requires an idle runtime host')
        plan = self._live_batch().prepare(candidates, save_path)
        return self.live_batch_status(plan['batch_id'])

    def live_batch_execute(self, batch_id, plan_digest, cancelled=None, progress=lambda value: None):
        if not self.status()['safe_to_shutdown']:
            raise RuntimeError('Resolve existing runtime ownership before insertion')
        self._live_batch().execute(batch_id, plan_digest, cancelled=cancelled.is_set if cancelled is not None else lambda: False, progress=progress)
        return self.live_batch_status(batch_id)

    def live_batch_cancel(self, batch_id):
        self._live_batch().cancel(batch_id)
        return self.live_batch_status(batch_id)

    def live_batch_status(self, batch_id):
        value = self._live_batch().status(batch_id)
        return {'live_batch': {'batch_id': batch_id, 'plan_digest': value['plan_digest'],
            'state': value['state'], 'count': value['requested_count'],
            'verified_count': sum(child['state'] == 'verified' for child in value['children']),
            'child_operation_ids': [child['operation_id'] for child in value['children']]}}

    def status(self):
        with self.lock:
            pending = sum(bool(getattr(oracle, 'remote_call_pending', False)) for oracle in self.retired_oracles)
            pending += int(self.live_add is not None and not self.live_add.safe_to_shutdown())
            self.retired_oracles = [oracle for oracle in self.retired_oracles if getattr(oracle, 'remote_call_pending', False)]
            state, hits, error = 'stopped', 0, None
            if self.session is not None:
                try:
                    hits = self.session.hit_count()
                    state = 'applied_hit' if hits else 'armed_no_hit'
                except OSError as exception:
                    state, error = 'unknown', str(exception)
            return {'override_state': state, 'hit_count': hits, 'pending_remote_calls': pending,
                    'safe_to_shutdown': self.session is None and pending == 0, 'error': error}

    def stop_override(self):
        with self.lock:
            if self.session is not None:
                self.session.stop()  # A failed restoration retains ownership.
                self.session = None
            return self.status()

    def start_override(self, profile):
        with self.lock:
            if not self.status()['safe_to_shutdown']:
                raise RuntimeError('Stop the existing override and wait for pending native calls')
            pid, runtime_profile, _path = self.identity()
            sessions = []
            if profile['enemy_keys'] or profile['special_rule_keys'] is not None or profile['terrain_value'] is not None:
                sessions.append(self.override_factory(RuntimeAuxiliaryOverrideProfile(
                seed=profile['seed'], enemy_keys=tuple(profile['enemy_keys']),
                special_rule_keys=tuple(profile['special_rule_keys']) if profile['special_rule_keys'] is not None else None,
                terrain_value=profile['terrain_value']), pid=pid, runtime_profile=runtime_profile))
            if profile.get('challenge_capacity') is not None:
                from .runtime_challenge_override import ChallengeOverrideProfile, RuntimeChallengeOverrideSession
                sessions.append(RuntimeChallengeOverrideSession(ChallengeOverrideProfile(profile['seed'], profile['challenge_capacity']),
                    pid=pid, runtime_profile=runtime_profile))
            if not sessions:
                raise ValueError('Select at least one temporary field')
            from .runtime_challenge_override import OverrideGroup
            session = sessions[0] if len(sessions) == 1 else OverrideGroup(sessions)
            self.session = session  # Retain even when start/rollback is uncertain.
            try:
                session.start()
            except Exception:
                try:
                    session.stop()
                    self.session = None
                except Exception:
                    pass
                raise
            return self.status()

    def generate(self, template, seed, playthrough, rarity, level, recommended_level, cancelled, progress):
        return self.search(template, seed, playthrough, rarity, level, recommended_level,
                           criteria=None, max_seeds=1, cancelled=cancelled, progress=progress)

    def search(self, template, seed, playthrough, rarity, level, recommended_level, criteria, max_seeds, cancelled, progress, after_trial=0):
        if template['context_digest'] != self.service.context.context_digest:
            raise ValueError('Template context differs from the running core')
        with self.lock:
            if not self.status()['safe_to_shutdown']:
                raise RuntimeError('Native generation requires an idle runtime host')
        pid, runtime_profile, _path = self.identity()
        oracle = self.oracle_factory(pid=pid, runtime_profile=runtime_profile, max_batch_size=128, preserve_requested_rarity=True)
        criteria = criteria or {}
        from .auxiliary_generation import AuxiliarySearchCriteria
        auxiliary = AuxiliarySearchCriteria(**{
            key: tuple(frozenset(group) for group in value) if key.endswith('_groups') else frozenset(value)
            for key, value in criteria.get('auxiliary', {}).items()
        })
        try:
            with oracle:
                from .native_search_maps import prepare_maps
                maps = prepare_maps(oracle, template, playthrough, rarity, level, recommended_level,
                    criteria, self.service.context, cancelled, progress) if max_seeds > 1 and criteria else {}
                cursor = {'joint_start_after_trial': after_trial} if any(key.startswith('primary_') for key in maps) else ({'grace_start_after_seed': seed - 1 if seed else None} if maps else {})
                last_progress = {}
                def report(update):
                    last_progress.update(asdict(update))
                    progress(asdict(update))
                candidate = scan_next_candidate(
                    oracle, template=bytes.fromhex(template['template_hex']), start_seed=seed,
                    primary_effect_ids=frozenset(criteria.get('primary_effect_ids', [])),
                    required_secondary_ids=frozenset(criteria.get('required_secondary_ids', [])),
                    required_secondary_id_groups=tuple(frozenset(group) for group in criteria.get('required_secondary_id_groups', [])),
                    grace_effect_id=criteria.get('grace_effect_id'), auxiliary_criteria=auxiliary,
                    rarity=rarity, level=level, recommended_level=recommended_level,
                    playthrough=playthrough, max_seeds=max_seeds, accelerate_grace=bool(maps), **maps, **cursor,
                    cancel_event=cancelled, progress=report,
                )
                if candidate is None:
                    return {'candidate': None, 'resume_trial': last_progress.get('joint_trial'), 'resume_seed': min(0xFFFFFFFF, last_progress.get('current_seed', seed) + 1)}
                require_search_candidate_ready(candidate)
                from .auxiliary_generation import generate_complete_auxiliary
                candidate = replace(candidate, auxiliary=generate_complete_auxiliary(candidate.seed, playthrough))
                wire = export_candidate(candidate, self.service.context.context_digest, level)
                self.candidates = {wire['candidate_id']: wire}
                from .worker_contracts import candidate_payload
                return {'candidate': candidate_payload(candidate, self.service, evidence='native_finalized_generation')}
        finally:
            if getattr(oracle, 'remote_call_pending', False):
                self.retired_oracles.append(oracle)

    def export(self, candidate_id):
        if candidate_id not in self.candidates:
            raise ValueError('Native candidate expired; generate again')
        return self.candidates[candidate_id]

    def capture_grace(self, template, playthrough, rarity, level, recommended_level, cancelled, progress):
        from .app_settings import load_app_settings
        from .cache_application import grace_map_cache_path
        from .grace_map import build_live_grace_output_map, save_grace_map_cache
        import os
        import sys
        if template['context_digest'] != self.service.context.context_digest:
            raise ValueError('Template context differs from the running core')
        if not self.status()['safe_to_shutdown']:
            raise RuntimeError('Map capture requires an idle runtime host')
        pid, runtime_profile, _path = self.identity()
        oracle = self.oracle_factory(pid=pid, runtime_profile=runtime_profile, max_batch_size=128, preserve_requested_rarity=True)
        try:
            with oracle:
                mapping = build_live_grace_output_map(oracle, template=bytes.fromhex(template['template_hex']),
                    category=playthrough, rarity=rarity, level=level, recommended_level=recommended_level,
                    cancel_event=cancelled, progress=lambda value: progress(asdict(value)))
                root = Path(getattr(sys, '_MEIPASS', Path(__file__).resolve().parents[1]))
                state = Path(os.environ.get('NIOH3_STATE_ROOT') or load_app_settings(fallback_root=root).data_root)
                path = grace_map_cache_path(state, save_fingerprint=template['save_fingerprint'],
                    playthrough=playthrough, rarity=rarity, generation_context_digest=self.service.context.context_digest)
                save_grace_map_cache(path, mapping, context_fingerprint=template['save_fingerprint'],
                                     generation_context_digest=self.service.context.context_digest)
                return {'captured': True, 'playthrough': playthrough, 'rarity': rarity}
        finally:
            if getattr(oracle, 'remote_call_pending', False):
                self.retired_oracles.append(oracle)

    def shutdown(self):
        try:
            self.stop_override()
        except Exception as error:
            return {'safe_to_shutdown': False, 'error': str(error)}
        return self.status()
