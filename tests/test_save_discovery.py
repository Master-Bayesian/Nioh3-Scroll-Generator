import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from nioh3_scroll_editor.savegame import (
    account_id_from_save_path,
    discover_save_paths,
    save_slot_index_from_path,
)


class SaveDiscoveryTests(unittest.TestCase):
    def test_discovers_every_character_slot_for_each_account(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            save_root = root / "KoeiTecmo" / "NIOH3" / "Savedata"
            expected: list[Path] = []
            for account_id in (76561198000000001, 76561198000000002):
                for slot_index in (0, 1, 2):
                    path = (
                        save_root
                        / str(account_id)
                        / f"SAVEDATA{slot_index:02d}"
                        / "SAVEDATA.BIN"
                    )
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(b"test")
                    expected.append(path)
            unrelated = (
                save_root
                / "76561198000000001"
                / "SYSTEMSAVEDATA00"
                / "SAVEDATA.BIN"
            )
            unrelated.parent.mkdir(parents=True)
            unrelated.write_bytes(b"system")

            with patch.dict(os.environ, {"LOCALAPPDATA": str(root)}, clear=False):
                discovered = discover_save_paths()

            self.assertEqual(discovered, expected)
            self.assertEqual(
                [save_slot_index_from_path(path) for path in discovered],
                [0, 1, 2, 0, 1, 2],
            )
            self.assertEqual(account_id_from_save_path(discovered[3]), 76561198000000002)

    def test_rejects_non_character_save_directory(self) -> None:
        with self.assertRaises(ValueError):
            save_slot_index_from_path(Path("SYSTEMSAVEDATA00") / "SAVEDATA.BIN")


if __name__ == "__main__":
    unittest.main()
