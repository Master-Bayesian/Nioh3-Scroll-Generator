import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from nioh3_scroll_editor.app_settings import (
    ENV_DATA_ROOT,
    SETTINGS_SCHEMA,
    default_state_root,
    load_app_settings,
    save_data_root,
    settings_path,
)


class AppSettingsTests(unittest.TestCase):
    def test_default_root_uses_local_app_data(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.dict(os.environ, {"LOCALAPPDATA": str(root)}, clear=False):
                self.assertEqual(
                    default_state_root(fallback_root=root / "fallback"),
                    (root / "Nioh3ScrollGenerator").resolve(),
                )

    def test_selected_data_root_is_persisted_atomically(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            selected = root / "portable-data"
            with patch.dict(os.environ, {"LOCALAPPDATA": str(root)}, clear=False):
                saved = save_data_root(selected, fallback_root=root / "fallback")
                loaded = load_app_settings(fallback_root=root / "fallback")

            self.assertEqual(saved.data_root, selected.resolve())
            self.assertEqual(loaded.data_root, selected.resolve())
            payload = json.loads(saved.settings_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["schema"], SETTINGS_SCHEMA)
            self.assertEqual(Path(payload["data_root"]), selected.resolve())
            self.assertFalse(any(saved.settings_path.parent.glob("*.tmp")))

    def test_invalid_pointer_fails_closed_to_default(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.dict(os.environ, {"LOCALAPPDATA": str(root)}, clear=False):
                pointer = settings_path(fallback_root=root / "fallback")
                pointer.parent.mkdir(parents=True)
                pointer.write_text(
                    '{"schema":"wrong","data_root":"relative"}',
                    encoding="utf-8",
                )
                loaded = load_app_settings(fallback_root=root / "fallback")

            self.assertEqual(
                loaded.data_root,
                (root / "Nioh3ScrollGenerator").resolve(),
            )

    def test_environment_override_does_not_rewrite_pointer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            override = root / "override"
            with patch.dict(
                os.environ,
                {"LOCALAPPDATA": str(root), ENV_DATA_ROOT: str(override)},
                clear=False,
            ):
                loaded = load_app_settings(fallback_root=root / "fallback")

            self.assertEqual(loaded.data_root, override.resolve())
            self.assertFalse(loaded.settings_path.exists())


if __name__ == "__main__":
    unittest.main()
