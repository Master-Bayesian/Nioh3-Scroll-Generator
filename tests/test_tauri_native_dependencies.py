from pathlib import Path
import tempfile
import unittest

from tests.test_tauri_onefile import launcher_fixture
from tools.verify_tauri_native_dependencies import inspect_native_imports, verify_runtime_native_dependencies


class NativeDependencyTests(unittest.TestCase):
    def test_normal_and_delayed_crt_imports_are_rejected_case_insensitively(self):
        for dll in ["VCRUNTIME140.dll", "vcruntime140_1.dll", "MSVCP140.dll", "MSVCR120.dll", "concrt140.dll"]:
            for delayed in [False, True]:
                with self.subTest(dll=dll, delayed=delayed):
                    with self.assertRaisesRegex(ValueError, "STANDALONE_EXTERNAL_CRT.*fixture.exe"):
                        inspect_native_imports(launcher_fixture(dll, delayed=delayed), "fixture.exe")

    def test_system_imports_remain_valid_and_reported(self):
        record = inspect_native_imports(launcher_fixture("KERNEL32.dll"), "fixture.exe")
        self.assertEqual(record["imports"], ["KERNEL32.dll"])
        self.assertEqual(record["delayedImports"], [])

    def test_runtime_gate_checks_workers_and_dlls_beyond_the_launcher(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Nioh3Studio.exe").write_bytes(launcher_fixture())
            (root / "worker").mkdir()
            helper = root / "worker/helper.dll"
            helper.write_bytes(launcher_fixture("MSVCP140.dll", delayed=True))
            with self.assertRaisesRegex(ValueError, "STANDALONE_EXTERNAL_CRT.*worker/helper.dll"):
                verify_runtime_native_dependencies(root)
            helper.write_bytes(launcher_fixture("d3d11.dll"))
            report = verify_runtime_native_dependencies(root)
            self.assertTrue(report["staticVcRuntime"])
            self.assertEqual(len(report["files"]), 2)
