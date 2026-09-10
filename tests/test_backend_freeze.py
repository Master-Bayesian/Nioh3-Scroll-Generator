"""Regression tests for the bounded backend freeze before Frontend V2."""

from __future__ import annotations

import json
import os
from dataclasses import replace
from pathlib import Path
import struct
import tempfile
import threading
import unittest
from unittest.mock import patch
import ctypes
import sys

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE
from nioh3_scroll_editor import native
from nioh3_scroll_editor import seed_accelerator
from nioh3_scroll_editor.app import ScrollEditorApp
from nioh3_scroll_editor.core_services import (
    CandidateApplicationService,
    CoreErrorCode,
    CoreServiceError,
    GenerationContext,
    OperationCommand,
    OperationPolicy,
)
from nioh3_scroll_editor.headless import handle_command
from nioh3_scroll_editor.models import CandidateRecordStage, ScrollCandidate
from nioh3_scroll_editor.runtime_auxiliary_override import (
    RuntimeAuxiliaryOverrideProfile,
    RuntimeAuxiliaryOverrideSession,
)
from nioh3_scroll_editor.savegame import (
    BACKUP_MANIFEST_SCHEMA,
    SAVE_SCHEMA_PROFILE,
    SaveInstaller,
    _save_operation_lock,
    sha256_file,
)


class _ValidRestoreCrypto:
    @staticmethod
    def decrypt(_source: Path, destination: Path) -> None:
        decrypted = bytearray(USER_SAVE_SIZE)
        decrypted[:6] = b"RNNUSR"
        destination.write_bytes(decrypted)


def _record(*, seed: int, rarity: int) -> bytes:
    record = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", record, 0, 0xE604)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30:0x32] = bytes((rarity, rarity))
    for index in range(7):
        struct.pack_into("<I", record, 0x38 + index * 0x18, 0xFFFFFFFF)
    return bytes(record)


def _prepare_restore_fixture(base: Path, *, account: int = 222, slot: int = 1):
    save_directory = base / str(account) / f"SAVEDATA{slot:02d}"
    system_directory = base / str(account) / "SYSTEMSAVEDATA00"
    save_directory.mkdir(parents=True)
    system_directory.mkdir()
    save = save_directory / "SAVEDATA.BIN"
    backup = save_directory / "BACKUP.BIN"
    system = system_directory / "SAVEDATA.BIN"
    save.write_bytes(b"CURRENT-MAIN")
    backup.write_bytes(b"CURRENT-BACKUP")
    system.write_bytes(b"CURRENT-SYSTEM")

    state_root = base / "state"
    source = state_root / "backups" / "source"
    source.mkdir(parents=True)
    source_files = (
        ("SAVEDATA.BIN", "main_save", b"SOURCE-MAIN"),
        ("BACKUP.BIN", "game_backup", b"SOURCE-BACKUP"),
        ("SYSTEMSAVEDATA.BIN", "system_save", b"SOURCE-SYSTEM"),
    )
    declared = []
    for name, role, content in source_files:
        path = source / name
        path.write_bytes(content)
        declared.append(
            {
                "source_role": role,
                "backup_file": name,
                "size": len(content),
                "sha256": sha256_file(path),
            }
        )
    report_path = source / "backup-manifest.json"
    report_path.write_text(
        json.dumps(
            {
                "backup_manifest_schema": BACKUP_MANIFEST_SCHEMA,
                "save_schema_profile": SAVE_SCHEMA_PROFILE,
                "steam_account_id": account,
                "save_slot_index": slot,
                "backup_files": declared,
            }
        ),
        encoding="utf-8",
    )
    installer = SaveInstaller(
        save_path=save,
        crypto=_ValidRestoreCrypto(),
        state_root=state_root,
    )
    return installer, source, save, backup, system, report_path


class SaveTransactionFreezeTests(unittest.TestCase):
    def test_restore_rejects_legacy_backup_without_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory))
            installer, source, save, backup, system, report_path = fixture
            report_path.unlink()

            with self.assertRaisesRegex(RuntimeError, "旧备份"):
                installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
            self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
            self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")

    def test_restore_rejects_wrong_account_before_writing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory))
            installer, source, save, backup, system, report_path = fixture
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["steam_account_id"] = 111
            report_path.write_text(json.dumps(report), encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "账号"):
                installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
            self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
            self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")

    def test_restore_rejects_wrong_save_slot_before_writing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory), slot=2)
            installer, source, save, backup, system, report_path = fixture
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["save_slot_index"] = 1
            report_path.write_text(json.dumps(report), encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "角色栏位"):
                installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
            self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
            self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")

    def test_restore_rejects_hash_mismatch_before_writing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory))
            installer, source, save, backup, system, report_path = fixture
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["backup_files"][0]["sha256"] = "0" * 64
            report_path.write_text(json.dumps(report), encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "哈希"):
                installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
            self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
            self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")

    def test_restore_rejects_undecryptable_main_save_before_writing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory))
            installer, source, save, backup, system, _report_path = fixture

            class FailingCrypto:
                @staticmethod
                def decrypt(_source: Path, _destination: Path) -> None:
                    raise RuntimeError("synthetic decrypt failure")

            installer.crypto = FailingCrypto()
            with self.assertRaisesRegex(RuntimeError, "解密与结构验证"):
                installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
            self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
            self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")

    def test_failure_at_each_restore_replace_rolls_back_committed_targets(self) -> None:
        for failed_replace in (1, 2, 3):
            with self.subTest(failed_replace=failed_replace), tempfile.TemporaryDirectory() as directory:
                fixture = _prepare_restore_fixture(Path(directory))
                installer, source, save, backup, system, _report_path = fixture
                real_replace = os.replace
                call_count = 0
                restore_targets = {
                    save.resolve(),
                    backup.resolve(),
                    system.resolve(),
                }

                def fail_selected_replace(source_path, target_path):
                    nonlocal call_count
                    if Path(target_path).resolve() in restore_targets:
                        call_count += 1
                        if call_count == failed_replace:
                            raise PermissionError("synthetic replace failure")
                    return real_replace(source_path, target_path)

                with patch(
                    "nioh3_scroll_editor.savegame.os.replace",
                    side_effect=fail_selected_replace,
                ):
                    with self.assertRaises(PermissionError):
                        installer.restore_backup(source)

                self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")
                self.assertEqual(backup.read_bytes(), b"CURRENT-BACKUP")
                self.assertEqual(system.read_bytes(), b"CURRENT-SYSTEM")
                journals = tuple(
                    (installer.state_root / "backups").glob(
                        "*/restore-journal.json"
                    )
                )
                journal = json.loads(journals[-1].read_text(encoding="utf-8"))
                self.assertEqual(journal["state"], "rolled_back")

    def test_report_failure_after_restore_is_committed_with_warning(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = _prepare_restore_fixture(Path(directory))
            installer, source, save, _backup, _system, _report_path = fixture
            original_write_text = Path.write_text

            def fail_restore_report(path: Path, *args, **kwargs):
                if path.name == "restore-report.json":
                    raise OSError("synthetic report disk full")
                return original_write_text(path, *args, **kwargs)

            with patch.object(Path, "write_text", fail_restore_report):
                result = installer.restore_backup(source)

            self.assertEqual(save.read_bytes(), b"SOURCE-MAIN")
            self.assertEqual(result.commit_status, "committed_with_warning")
            self.assertIn("报告写入失败", result.warning or "")
            manifest_path = result.checkpoint_directory / "backup-manifest.json"
            self.assertTrue(manifest_path.is_file())

            rollback = installer.restore_backup(result.checkpoint_directory)
            self.assertEqual(rollback.commit_status, "committed")
            self.assertEqual(save.read_bytes(), b"CURRENT-MAIN")

    def test_account_lock_rejects_a_second_writer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save = root / "222" / "SAVEDATA00" / "SAVEDATA.BIN"
            save.parent.mkdir(parents=True)
            save.write_bytes(b"sentinel")
            state_root = root / "state"

            with _save_operation_lock(state_root, save):
                with self.assertRaisesRegex(RuntimeError, "另一个本程序实例"):
                    with _save_operation_lock(state_root, save):
                        self.fail("a second save writer acquired the same lock")


class CandidatePolicyFreezeTests(unittest.TestCase):
    def test_generated_ng4_and_ng5_candidates_are_never_installable(self) -> None:
        for playthrough in (4, 5):
            with self.subTest(playthrough=playthrough):
                candidate = ScrollCandidate.from_record(
                    _record(seed=100 + playthrough, rarity=5),
                    playthrough=playthrough,
                    record_stage=CandidateRecordStage.FINAL_RECORD,
                )
                self.assertIn("禁止", candidate.install_blocker or "")

    def test_early_ng_r4_final_preview_keeps_paired_stage_one_install(self) -> None:
        seed = 43723117
        stage_one = _record(seed=seed, rarity=4)
        candidate = ScrollCandidate.from_record(
            _record(seed=seed, rarity=4),
            playthrough=1,
            record_stage=CandidateRecordStage.FINAL_RECORD,
        )
        candidate = replace(candidate, installation_record=stage_one)
        self.assertIsNone(candidate.install_blocker)

    def test_shared_service_blocks_ng4_for_tk_and_headless_callers(self) -> None:
        context = GenerationContext.capture()
        service = CandidateApplicationService(context)
        candidate = ScrollCandidate.from_record(
            _record(seed=12345, rarity=5),
            playthrough=4,
            record_stage=CandidateRecordStage.FINAL_RECORD,
        )

        preview = service.preview(candidate)
        self.assertFalse(preview.installable)
        with self.assertRaises(CoreServiceError) as raised:
            service.prepare_generated_install(candidate)
        self.assertEqual(raised.exception.code, CoreErrorCode.CANDIDATE_NOT_INSTALLABLE)
        headless = handle_command(
            {
                "command": "candidate_preflight",
                "record_hex": candidate.record.hex(),
                "playthrough": 4,
                "record_stage": "final_record",
            }
        )
        self.assertFalse(headless["preview"]["installable"])

    def test_custom_edit_is_separate_from_generated_install_policy(self) -> None:
        policy = OperationPolicy()
        decision = policy.evaluate(OperationCommand.CUSTOM_EDIT)
        self.assertTrue(decision.allowed)

    def test_headless_handshake_exposes_pinned_generation_context(self) -> None:
        response = handle_command({"command": "handshake"})
        context = response["context"]
        self.assertTrue(response["ok"])
        self.assertEqual(len(context["resources_digest"]), 64)
        self.assertEqual(len(context["context_digest"]), 64)
        self.assertEqual(context["policy_version"], "operation-policy-v1")

    def test_ng3_final_candidate_remains_installable(self) -> None:
        candidate = ScrollCandidate.from_record(
            _record(seed=36526331, rarity=4),
            playthrough=3,
            record_stage=CandidateRecordStage.FINAL_RECORD,
        )
        self.assertIsNone(candidate.install_blocker)


class NativeLifecycleFreezeTests(unittest.TestCase):
    def test_window_close_waits_for_background_worker(self) -> None:
        trace: list[object] = []

        class FakeWorker:
            @staticmethod
            def is_alive() -> bool:
                return True

        class FakeRoot:
            @staticmethod
            def after(delay_ms, callback) -> None:
                trace.append(("after", delay_ms, callback))

            @staticmethod
            def destroy() -> None:
                trace.append("destroy")

        class FakeStatus:
            @staticmethod
            def set(message) -> None:
                trace.append(("status", message))

        class FakeSearchEvents:
            @staticmethod
            def cancel(run_id) -> None:
                trace.append(("cancel", run_id))

        app = ScrollEditorApp.__new__(ScrollEditorApp)
        app.worker = FakeWorker()
        app.cancel_event = threading.Event()
        app.search_events = FakeSearchEvents()
        app.active_search_run_id = 73
        app.status = FakeStatus()
        app.root = FakeRoot()

        with patch.object(
            app,
            "_stop_local_runtime_override",
            side_effect=AssertionError("hook stop must wait for the worker"),
        ):
            app._close_application()

        self.assertTrue(app.cancel_event.is_set())
        self.assertIn(("cancel", 73), trace)
        self.assertTrue(any(item[:2] == ("after", 100) for item in trace if isinstance(item, tuple)))
        self.assertNotIn("destroy", trace)

    def test_window_close_stays_open_when_hook_stop_is_uncertain(self) -> None:
        trace: list[str] = []

        class FakeRoot:
            @staticmethod
            def destroy() -> None:
                trace.append("destroy")

        app = ScrollEditorApp.__new__(ScrollEditorApp)
        app.worker = None
        app.root = FakeRoot()
        app._closing_requested = True

        with patch.object(app, "_stop_local_runtime_override", return_value=False):
            app._close_application()

        self.assertFalse(app._closing_requested)
        self.assertNotIn("destroy", trace)

    def test_remote_timeout_retires_memory_instead_of_freeing_it(self) -> None:
        trace: list[str] = []
        remote_finished = threading.Event()

        class FakeDll:
            def CreateRemoteThread(self, *_args):
                trace.append("create_thread")
                return 999

            def WaitForSingleObject(self, _handle, timeout_ms):
                if timeout_ms == 1:
                    trace.append("timeout")
                    return 258
                trace.append("retired_wait")
                remote_finished.wait(2)
                return 0

            def CloseHandle(self, handle):
                trace.append(f"close_{handle}")
                return True

            def VirtualFreeEx(self, *_args):
                trace.append("free_remote")
                return True

        oracle = native.NativeBatchOracle.__new__(native.NativeBatchOracle)
        oracle.process = 111
        oracle.allocation = 4096
        oracle.source_address = 8192
        oracle.destination_address = 12288
        oracle.module_base = 0x140000000
        oracle.max_batch_size = 1
        oracle.runtime_profile = native.DEFAULT_NATIVE_RUNTIME_PROFILE
        oracle.preserve_requested_rarity = True
        oracle.dll = FakeDll()
        oracle.write = lambda *_args: None

        with self.assertRaisesRegex(RuntimeError, "等待游戏原生生成器失败"):
            with oracle:
                oracle.generate([_record(seed=1, rarity=5)], timeout_ms=1)

        self.assertNotIn("free_remote", trace)
        self.assertIsNone(oracle.allocation)
        self.assertIsNone(oracle.process)
        self.assertEqual(oracle.source_address, 0)
        self.assertEqual(oracle.destination_address, 0)
        remote_finished.set()
        assert oracle._retired_call_watcher is not None
        oracle._retired_call_watcher.join(2)
        self.assertIn("free_remote", trace)

    def test_hook_access_error_does_not_claim_stop_for_live_process(self) -> None:
        session = RuntimeAuxiliaryOverrideSession(
            RuntimeAuxiliaryOverrideProfile(seed=1, terrain_value=0)
        )
        session.process = 222
        session.allocation = 4096
        session.patch = b"PATCH"
        session._installed_once = True
        session.counter_address = 5000
        session.hook_address = 6000
        session._read = lambda *_args: (_ for _ in ()).throw(
            OSError("synthetic access denied")
        )

        with patch.object(session, "_process_has_exited", return_value=False):
            with self.assertRaisesRegex(RuntimeError, "仍标记为启用"):
                session.stop()

        self.assertEqual(session.process, 222)
        self.assertEqual(session.patch, b"PATCH")

    def test_hook_access_error_can_close_only_after_confirmed_process_exit(self) -> None:
        session = RuntimeAuxiliaryOverrideSession(
            RuntimeAuxiliaryOverrideProfile(seed=1, terrain_value=0)
        )
        session.process = 222
        session.allocation = 4096
        session.patch = b"PATCH"
        session._installed_once = True
        session.counter_address = 5000
        session.hook_address = 6000
        session._read = lambda *_args: (_ for _ in ()).throw(
            OSError("process ended")
        )

        with patch.object(session, "_process_has_exited", return_value=True), patch.object(
            session,
            "_release_session",
        ) as release:
            session.stop()

        release.assert_called_once_with(release_allocation=False)


@unittest.skipUnless(sys.platform == "win32", "native DLL ABI test requires Windows")
class NativeExecutionPolicyFreezeTests(unittest.TestCase):
    def test_forced_cuda_failure_never_enters_cpu_without_explicit_policy(self) -> None:
        library = seed_accelerator._load_accelerator()
        if library is None:
            self.skipTest("ABI-v2 Seed accelerator DLL is unavailable")
        force_failure = library.seed_accelerator_test_force_cuda_failure
        force_failure.argtypes = (ctypes.c_int,)
        force_failure.restype = None
        reset_counter = library.seed_accelerator_reset_bulk_cpu_call_count
        reset_counter.argtypes = ()
        reset_counter.restype = None
        read_counter = library.seed_accelerator_bulk_cpu_call_count
        read_counter.argtypes = ()
        read_counter.restype = ctypes.c_uint64

        reset_counter()
        force_failure(1)
        try:
            with seed_accelerator.seed_acceleration_execution_policy(
                allow_bulk_cpu=False
            ):
                with self.assertRaises(RuntimeError):
                    seed_accelerator.collect_natural_pivot_seeds(
                        (0,),
                        start_index=0,
                        stop_index=1,
                        low16_stride=1,
                    )
            self.assertEqual(read_counter(), 0)

            with seed_accelerator.seed_acceleration_execution_policy(
                allow_bulk_cpu=True
            ):
                result = seed_accelerator.collect_natural_pivot_seeds(
                    (0,),
                    start_index=0,
                    stop_index=1,
                    low16_stride=1,
                )
            self.assertIsInstance(result, tuple)
            self.assertEqual(read_counter(), 1)
        finally:
            force_failure(0)
            library.seed_accelerator_set_execution_policy(
                seed_accelerator.EXECUTION_POLICY_STRICT_GPU
            )

    def test_cuda_and_explicit_cpu_preserve_exact_pivot_cursor_results(self) -> None:
        library = seed_accelerator._load_accelerator()
        if library is None or not seed_accelerator.cuda_seed_acceleration_available():
            self.skipTest("a healthy CUDA device is unavailable")
        force_failure = library.seed_accelerator_test_force_cuda_failure
        force_failure.argtypes = (ctypes.c_int,)
        force_failure.restype = None
        values = tuple(range(64))
        try:
            force_failure(0)
            with seed_accelerator.seed_acceleration_execution_policy(
                allow_bulk_cpu=False
            ):
                cuda_result = seed_accelerator.collect_natural_pivot_seeds(
                    values,
                    start_index=0,
                    stop_index=4096,
                    low16_stride=0x9E37,
                )
            force_failure(1)
            with seed_accelerator.seed_acceleration_execution_policy(
                allow_bulk_cpu=True
            ):
                cpu_result = seed_accelerator.collect_natural_pivot_seeds(
                    values,
                    start_index=0,
                    stop_index=4096,
                    low16_stride=0x9E37,
                )
            self.assertEqual(cuda_result, cpu_result)
        finally:
            force_failure(0)
            library.seed_accelerator_set_execution_policy(
                seed_accelerator.EXECUTION_POLICY_STRICT_GPU
            )


if __name__ == "__main__":
    unittest.main()
