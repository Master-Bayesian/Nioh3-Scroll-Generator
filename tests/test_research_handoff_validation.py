"""Tests for the generic Pro research handoff validator."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from tools.validate_research_handoff import HandoffValidationError, validate_directory, validate_zip


class ResearchHandoffValidationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.package = self.base / "equipment-pro-handoff-pc-v2.01-20260912"
        (self.package / "evidence").mkdir(parents=True)
        (self.package / "project-source").mkdir()
        (self.package / "README.md").write_text("# Reading order\n", encoding="utf-8")
        (self.package / "TASK_FOR_PRO.md").write_text("# Task\nDerive the generator.\n", encoding="utf-8")
        (self.package / "ENVIRONMENT.json").write_text(
            json.dumps({"schema": "fixture/v1", "game_version": "PC v2.01"}), encoding="utf-8"
        )
        (self.package / "evidence" / "capture.json").write_text("{}\n", encoding="utf-8")
        (self.package / "project-source" / "model.py").write_text("VALUE = 1\n", encoding="utf-8")
        self.write_hashes()

    def files_without_manifest(self):
        return sorted(
            path for path in self.package.rglob("*")
            if path.is_file() and path.name != "SHA256SUMS.txt"
        )

    def write_hashes(self):
        rows = []
        for path in self.files_without_manifest():
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            rows.append(f"{digest}  {path.relative_to(self.package).as_posix()}")
        (self.package / "SHA256SUMS.txt").write_text("\n".join(rows) + "\n", encoding="utf-8")

    def write_zip(self):
        archive_path = self.package.with_suffix(".zip")
        with zipfile.ZipFile(archive_path, "w", zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(self.package.rglob("*")):
                if path.is_file():
                    archive.write(path, f"{self.package.name}/{path.relative_to(self.package).as_posix()}")
        return archive_path

    def test_valid_directory_and_zip(self):
        directory_report = validate_directory(self.package)
        archive_report = validate_zip(self.package, self.write_zip())
        self.assertEqual(directory_report["hashes_verified"], 5)
        self.assertEqual(archive_report["zip_files_verified"], 6)

    def test_hash_coverage_must_be_exact(self):
        (self.package / "evidence" / "late.txt").write_text("late\n", encoding="utf-8")
        with self.assertRaisesRegex(HandoffValidationError, "checksum coverage differs"):
            validate_directory(self.package)

    def test_changed_evidence_is_rejected(self):
        (self.package / "evidence" / "capture.json").write_text('{"changed": true}\n', encoding="utf-8")
        with self.assertRaisesRegex(HandoffValidationError, "checksum mismatch"):
            validate_directory(self.package)

    def test_invalid_environment_and_empty_evidence_are_rejected(self):
        (self.package / "ENVIRONMENT.json").write_text("[]", encoding="utf-8")
        self.write_hashes()
        with self.assertRaisesRegex(HandoffValidationError, "non-empty object"):
            validate_directory(self.package)

        (self.package / "ENVIRONMENT.json").write_text("{}", encoding="utf-8")
        (self.package / "evidence" / "capture.json").unlink()
        self.write_hashes()
        with self.assertRaisesRegex(HandoffValidationError, "evidence files"):
            validate_directory(self.package)

    def test_zip_must_match_package_root_and_bytes(self):
        archive_path = self.write_zip()
        with zipfile.ZipFile(archive_path, "a") as archive:
            archive.writestr(f"{self.package.name}/extra.txt", "unexpected")
        with self.assertRaisesRegex(HandoffValidationError, "ZIP contents differ"):
            validate_zip(self.package, archive_path)

    def test_zip_rejects_ambiguous_member_paths(self):
        archive_path = self.write_zip()
        with zipfile.ZipFile(archive_path, "a") as archive:
            archive.writestr(f"{self.package.name}/evidence//alias.json", "{}")
        with self.assertRaisesRegex(HandoffValidationError, "unsafe or duplicate ZIP member"):
            validate_zip(self.package, archive_path)


if __name__ == "__main__":
    unittest.main()
