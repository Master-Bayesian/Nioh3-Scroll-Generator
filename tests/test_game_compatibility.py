from __future__ import annotations

from pathlib import Path
import unittest
from unittest.mock import patch

from nioh3_scroll_editor.game_compatibility import (
    GameCompatibilityStatus,
    detect_game_compatibility,
    verify_game_executable,
)
from nioh3_scroll_editor.native import native_runtime_profile_for_game_version


class GameCompatibilityTests(unittest.TestCase):
    def test_pc_v201_approved_runtime_profile_is_loadable(self) -> None:
        profile = native_runtime_profile_for_game_version((2, 0, 1, 0))

        self.assertEqual(profile.display_version, "PC v2.01")
        self.assertEqual(profile.canonicalize_rva, 0x20E1AF0)
        self.assertEqual(profile.descriptor_complete_rva, 0x20E195C)

    def test_accepts_the_validated_file_version(self) -> None:
        with patch(
            "nioh3_scroll_editor.game_compatibility._file_version",
            return_value=(2, 0, 0, 2),
        ):
            status = verify_game_executable(Path("Nioh3.exe"))

        self.assertTrue(status.supported)
        self.assertFalse(status.known_mismatch)
        self.assertEqual(status.file_version, (2, 0, 0, 2))

    def test_accepts_pc_v2_01(self) -> None:
        with patch(
            "nioh3_scroll_editor.game_compatibility._file_version",
            return_value=(2, 0, 1, 0),
        ):
            status = verify_game_executable(Path("Nioh3.exe"))

        self.assertTrue(status.supported)
        self.assertEqual(status.file_version, (2, 0, 1, 0))
        self.assertIn("PC v2.01", status.detail)

    def test_rejects_a_newer_unvalidated_file_version(self) -> None:
        with patch(
            "nioh3_scroll_editor.game_compatibility._file_version",
            return_value=(2, 0, 1, 1),
        ):
            status = verify_game_executable(Path("Nioh3.exe"))

        self.assertTrue(status.known_mismatch)
        self.assertIn("2.0.1.1", status.detail)

    def test_reports_unreadable_version_resources_without_false_mismatch(self) -> None:
        with patch(
            "nioh3_scroll_editor.game_compatibility._file_version",
            side_effect=OSError("missing version resource"),
        ):
            status = verify_game_executable(Path("Nioh3.exe"))

        self.assertEqual(status.state, "unreadable")
        self.assertFalse(status.known_mismatch)

    def test_detected_mismatch_takes_precedence_over_a_stale_supported_copy(self) -> None:
        old_copy = Path("old/Nioh3.exe")
        updated_copy = Path("updated/Nioh3.exe")
        with (
            patch(
                "nioh3_scroll_editor.game_compatibility.discover_game_executables",
                return_value=(old_copy, updated_copy),
            ),
            patch(
                "nioh3_scroll_editor.game_compatibility.verify_game_executable",
                side_effect=(
                    GameCompatibilityStatus("supported", "supported", old_copy),
                    GameCompatibilityStatus("unsupported", "unsupported", updated_copy),
                ),
            ),
        ):
            status = detect_game_compatibility()

        self.assertTrue(status.known_mismatch)


if __name__ == "__main__":
    unittest.main()
