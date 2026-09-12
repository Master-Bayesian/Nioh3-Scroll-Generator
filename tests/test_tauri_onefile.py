"""The install-free wrapper accepts only a complete, manifest-verified payload."""
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

from tools.build_tauri_onefile import FOOTER, LAUNCHER_PATH, MAGIC, REQUIRED_PATHS, build_onefile, safe_path


def launcher_fixture():
    value = bytearray(128)
    value[:2] = b"MZ"
    struct.pack_into("<I", value, 0x3C, 64)
    value[64:68] = b"PE\0\0"
    struct.pack_into("<H", value, 68, 0x8664)
    return bytes(value)


class TauriOnefileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "runtime.zip"
        self.output = self.root / "Nioh3Studio-0.7.3-win-x64.exe"

    def fixture(self, *, changed_hash=False, dirty=False, extra=None, missing=None, bad_launcher=False):
        files = {name: launcher_fixture() if name == LAUNCHER_PATH else b"fixture:" + name.encode() for name in REQUIRED_PATHS}
        if missing:
            files.pop(missing)
        if bad_launcher:
            files[LAUNCHER_PATH] = b"not executable"
        entries = [{"path": name, "size": len(raw), "sha256": hashlib.sha256(raw).hexdigest()} for name, raw in sorted(files.items())]
        if changed_hash:
            entries[0]["sha256"] = "0" * 64
        manifest = {"schema": "nioh3-tauri-manifest/v1", "version": "0.7.3", "git": {"commit": "a" * 40, "dirty": dirty}, "files": entries}
        with zipfile.ZipFile(self.archive, "w", zipfile.ZIP_DEFLATED) as archive:
            for name, raw in files.items():
                archive.writestr(name, raw)
            archive.writestr("build-manifest.json", json.dumps(manifest))
            if extra:
                archive.writestr(extra, b"extra")
        return files

    def test_exact_wrapper_footer_payload_and_sidecar(self):
        files = self.fixture()
        payload = self.archive.read_bytes()
        report = build_onefile(self.archive, self.output)
        raw = self.output.read_bytes()
        self.assertEqual(FOOTER.size, 56)
        self.assertEqual(len(MAGIC), 16)
        self.assertEqual(raw[:len(files[LAUNCHER_PATH])], files[LAUNCHER_PATH])
        self.assertEqual(raw[len(files[LAUNCHER_PATH]):-FOOTER.size], payload)
        self.assertEqual(FOOTER.unpack(raw[-FOOTER.size:]), (MAGIC, len(payload), hashlib.sha256(payload).digest()))
        self.assertEqual(report["sourceCommit"], "a" * 40)
        self.assertEqual(report["sha256"], hashlib.sha256(raw).hexdigest())
        self.assertIn(report["sha256"], self.output.with_suffix(".exe.sha256").read_text())

    def test_dirty_source_refused_before_output(self):
        self.fixture(dirty=True)
        with self.assertRaisesRegex(ValueError, "clean source"):
            build_onefile(self.archive, self.output)
        self.assertFalse(self.output.exists())

    def test_corrupted_archived_member_refused(self):
        self.fixture(changed_hash=True)
        with self.assertRaisesRegex(ValueError, "hash differs"):
            build_onefile(self.archive, self.output)
        self.assertFalse(self.output.exists())

    def test_required_launcher_and_valid_pe_are_enforced(self):
        self.fixture(missing=LAUNCHER_PATH)
        with self.assertRaisesRegex(ValueError, "missing required"):
            build_onefile(self.archive, self.output)
        self.fixture(bad_launcher=True)
        with self.assertRaisesRegex(ValueError, "not a PE"):
            build_onefile(self.archive, self.output)

    def test_extra_unmanifested_file_and_case_alias_refused(self):
        for extra in ("user-state.json", "BUILD-MANIFEST.JSON", "../escaped.exe"):
            with self.subTest(extra=extra):
                self.fixture(extra=extra)
                with self.assertRaises(ValueError):
                    build_onefile(self.archive, self.output)
                self.assertFalse(self.output.exists())

    def test_existing_output_is_preserved(self):
        self.fixture()
        self.output.write_bytes(b"existing product")
        with self.assertRaises(FileExistsError):
            build_onefile(self.archive, self.output)
        self.assertEqual(self.output.read_bytes(), b"existing product")

    def test_windows_paths_reject_ambiguous_archive_names(self):
        for name in ("a//b", "a/./b", "a/../b", "/absolute", "C:/x", "NUL.txt", "com1/data", "a.", "a ", "a\\b", "file\0name"):
            with self.subTest(name=name):
                self.assertFalse(safe_path(name))
        self.assertTrue(safe_path("launcher/Nioh3Launcher.exe"))


if __name__ == "__main__":
    unittest.main()
