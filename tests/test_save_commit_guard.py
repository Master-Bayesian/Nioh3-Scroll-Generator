"""Fault injection for title-screen save insertion transactions."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from emaki_exchange import SCROLL_RECORD_SIZE, USER_SAVE_SIZE
from nioh3_scroll_editor.savegame import SaveInstaller
from tests.test_beta_editor import SCROLL_GROUP_OFFSET, TEST_ACCOUNT_ID, make_record


class PassthroughCrypto:
    @staticmethod
    def decrypt(source: Path, output: Path) -> None:
        encrypted = source.read_bytes()
        if not encrypted.startswith(b"ENC"):
            raise AssertionError("expected fake encrypted input")
        output.write_bytes(encrypted[3:])

    @staticmethod
    def encrypt(source: Path, output: Path) -> None:
        output.write_bytes(b"ENC" + source.read_bytes())


class SaveCommitGuardTests(unittest.TestCase):
    def make_fixture(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        save_directory = root / str(TEST_ACCOUNT_ID) / "SAVEDATA00"
        system_directory = root / str(TEST_ACCOUNT_ID) / "SYSTEMSAVEDATA00"
        save_directory.mkdir(parents=True)
        system_directory.mkdir()
        save_path = save_directory / "SAVEDATA.BIN"
        backup_path = save_directory / "BACKUP.BIN"
        system_path = system_directory / "SAVEDATA.BIN"
        decrypted = bytearray(USER_SAVE_SIZE)
        decrypted[:6] = b"RNNUSR"
        decrypted[
            SCROLL_GROUP_OFFSET : SCROLL_GROUP_OFFSET + SCROLL_RECORD_SIZE
        ] = make_record(account_id=TEST_ACCOUNT_ID)
        save_path.write_bytes(b"ENC" + decrypted)
        backup_path.write_bytes(b"game-backup-generation")
        system_path.write_bytes(b"system-generation")
        return root, save_path, backup_path, system_path

    def test_sibling_change_during_preparation_aborts_without_main_write(self):
        for role in ("game_backup", "system_save"):
            with self.subTest(role=role):
                root, save_path, backup_path, system_path = self.make_fixture()
                original_main = save_path.read_bytes()
                target = backup_path if role == "game_backup" else system_path

                class MutatingCrypto(PassthroughCrypto):
                    @staticmethod
                    def encrypt(source: Path, output: Path) -> None:
                        PassthroughCrypto.encrypt(source, output)
                        target.write_bytes(target.read_bytes() + b"-changed")

                installer = SaveInstaller(
                    save_path=save_path,
                    crypto=MutatingCrypto(),
                    state_root=root / "state",
                )
                with patch(
                    "nioh3_scroll_editor.savegame.time.sleep",
                    return_value=None,
                ):
                    with self.assertRaisesRegex(RuntimeError, "SAVE_SYNC_ACTIVE"):
                        installer.install(
                            make_record(seed=86872488, account_id=TEST_ACCOUNT_ID),
                            transfer_count=0xFFFFFFFF,
                        )
                self.assertEqual(original_main, save_path.read_bytes())
                backup_directories = list((root / "state" / "backups").iterdir())
                self.assertEqual(1, len(backup_directories))
                self.assertEqual(
                    b"game-backup-generation",
                    (backup_directories[0] / "BACKUP.BIN").read_bytes(),
                )
                self.assertEqual(
                    b"system-generation",
                    (backup_directories[0] / "SYSTEMSAVEDATA.BIN").read_bytes(),
                )
                journal = json.loads(
                    (backup_directories[0] / "save-write-journal.json").read_text(
                        encoding="utf-8"
                    )
                )
                self.assertEqual("aborted", journal["state"])

    def test_failed_post_commit_decrypt_restores_only_original_main(self):
        root, save_path, backup_path, system_path = self.make_fixture()
        original_main = save_path.read_bytes()
        original_backup = backup_path.read_bytes()
        original_system = system_path.read_bytes()

        class CorruptInstalledReadbackCrypto(PassthroughCrypto):
            main_decryptions = 0

            @classmethod
            def decrypt(cls, source: Path, output: Path) -> None:
                if source.resolve() == save_path.resolve():
                    cls.main_decryptions += 1
                    if cls.main_decryptions == 2:
                        output.write_bytes(b"corrupt-installed-readback")
                        return
                PassthroughCrypto.decrypt(source, output)

        installer = SaveInstaller(
            save_path=save_path,
            crypto=CorruptInstalledReadbackCrypto(),
            state_root=root / "state",
        )
        with patch("nioh3_scroll_editor.savegame.time.sleep", return_value=None):
            with self.assertRaisesRegex(RuntimeError, "SAVE_COMMIT_ROLLED_BACK"):
                installer.install(
                    make_record(seed=86872488, account_id=TEST_ACCOUNT_ID),
                    transfer_count=0xFFFFFFFF,
                )

        self.assertEqual(original_main, save_path.read_bytes())
        self.assertEqual(original_backup, backup_path.read_bytes())
        self.assertEqual(original_system, system_path.read_bytes())
        backup_directory = next((root / "state" / "backups").iterdir())
        journal = json.loads(
            (backup_directory / "save-write-journal.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("rolled_back", journal["state"])

    def test_success_report_contains_reproduction_evidence(self):
        root, save_path, _backup_path, _system_path = self.make_fixture()
        candidate = make_record(seed=86872488, account_id=TEST_ACCOUNT_ID)
        installer = SaveInstaller(
            save_path=save_path,
            crypto=PassthroughCrypto(),
            state_root=root / "state",
        )
        with patch("nioh3_scroll_editor.savegame.time.sleep", return_value=None):
            result = installer.install(candidate, transfer_count=0xFFFFFFFF)
        report = json.loads(result.report_path.read_text(encoding="utf-8"))
        self.assertEqual(str(save_path.resolve()), report["save_path"])
        self.assertEqual(candidate.hex(), report["candidate_record_hex"])
        self.assertEqual(3, len(report["baseline_files"]))
        journal = json.loads(
            (result.backup_directory / report["write_journal"]).read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual("committed", journal["state"])


if __name__ == "__main__":
    unittest.main()
