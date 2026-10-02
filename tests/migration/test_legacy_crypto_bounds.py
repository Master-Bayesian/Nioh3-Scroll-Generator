"""Opt-in byte parity for the rebuilt legacy helper using synthetic containers.

First run tools/test_legacy_crypto_bounds.ps1 -Sanitize -BuildHelper, then set
NIOH3_LEGACY_CRYPTO_HELPER to that executable and run through run_python_tests.ps1.
The compiled sanitizer harness owns counter/header/body memory-boundary coverage;
this gate exercises the actual CLI and the retained Python SaveCrypto entry point.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor.savegame import SaveCrypto  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402
from tests.migration.test_save_read_parity import (  # noqa: E402
    SAVE_CRYPTO,
    require_save_oracle,
)

HEADER_BYTES = 0x158
BODY_BYTES = {"user": 0x900058, "system": 0x39620}


def synthetic_plain(kind: str, salt: int) -> bytes:
    size = HEADER_BYTES + BODY_BYTES[kind]
    pattern = bytes((index * 13 + salt) & 0xFF for index in range(256))
    plain = bytearray((pattern * ((size + 255) // 256))[:size])
    magic = b"RNNUSR" if kind == "user" else b"NIOHSYS"
    plain[: len(magic)] = magic
    # Deliberately nonzero: all codecs must discard the untransformed USR tail.
    if kind == "user":
        plain[-8:] = b"TRAILER!"
    return bytes(plain)


@unittest.skipUnless(
    os.environ.get("NIOH3_LEGACY_CRYPTO_HELPER"),
    "opt-in compiled helper parity: set NIOH3_LEGACY_CRYPTO_HELPER",
)
class LegacyCryptoBoundsParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.helper = Path(os.environ["NIOH3_LEGACY_CRYPTO_HELPER"]).resolve()
        if not cls.helper.is_file():
            raise AssertionError(f"rebuilt helper missing: {cls.helper}")
        if cls.helper == SAVE_CRYPTO.resolve():
            raise AssertionError("parity requires the rebuilt helper, not the shipped oracle")
        require_save_oracle()
        cls.fixed = SaveCrypto(cls.helper)
        cls.shipped = SaveCrypto(SAVE_CRYPTO)
        cls.target = Path(resolved_cargo_target_dir())
        result = subprocess.run(
            [
                "cargo", "build", "--offline", "--quiet", "--manifest-path",
                str(ROOT / "crates/nioh3-save/Cargo.toml"),
                "--example", "save_transaction",
            ],
            cwd=ROOT,
            env={**os.environ, "CARGO_TARGET_DIR": str(cls.target)},
            capture_output=True,
            text=True,
            timeout=240,
            check=False,
        )
        if result.returncode != 0:
            raise AssertionError(f"Rust codec build failed:\n{result.stdout}\n{result.stderr}")
        cls.rust = cls.target / "debug/examples/save_transaction.exe"
        if not cls.rust.is_file():
            raise AssertionError(f"Rust codec executable missing: {cls.rust}")
        print(f"REBUILT_HELPER_SHA256={hashlib.sha256(cls.helper.read_bytes()).hexdigest()}")

    def rust_transform(self, source: Path, output: Path, *, encrypt: bool) -> None:
        command = "encrypt-container" if encrypt else "decrypt-container"
        input_flag = "--write-file" if encrypt else "--container-file"
        result = subprocess.run(
            [
                str(self.rust), command, "--state-root", str(source.parent / "state"),
                "--save-path", str(source),
                input_flag, str(source), "--output-file", str(output),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=90,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + "\n" + result.stderr)
        self.assertTrue(output.is_file(), "Rust codec did not create output")

    def assert_codec_parity(self, kind: str) -> None:
        for salt in (9, 0xB7):
            with self.subTest(kind=kind, salt=salt):
                with tempfile.TemporaryDirectory(prefix=f"legacy-crypto-{kind}-") as directory:
                    work = Path(directory)
                    plain = synthetic_plain(kind, salt)
                    expected = plain if kind == "system" else plain[:-8] + bytes(8)
                    source = work / "synthetic-clear.bin"
                    source.write_bytes(plain)
                    fixed_container = work / "fixed-container.bin"
                    shipped_container = work / "shipped-container.bin"
                    rust_container = work / "rust-container.bin"
                    if kind == "user":
                        self.fixed.encrypt(source, fixed_container)
                        self.shipped.encrypt(source, shipped_container)
                    else:
                        self.fixed.transform(source, fixed_container)
                        self.shipped.transform(source, shipped_container)
                    self.rust_transform(source, rust_container, encrypt=True)
                    container = fixed_container.read_bytes()
                    self.assertEqual(len(container), len(plain))
                    self.assertNotEqual(container, plain)
                    self.assertEqual(container, shipped_container.read_bytes(), "shipped CLI encrypt parity")
                    self.assertEqual(container, rust_container.read_bytes(), "Rust encrypt parity")
                    if kind == "user":
                        self.assertEqual(container[-8:], bytes(8), "USR trailer encrypt semantics")
                    # Each implementation decodes the other implementation's output.
                    fixed_plain = work / "fixed-plain.bin"
                    shipped_plain = work / "shipped-plain.bin"
                    rust_plain = work / "rust-plain.bin"
                    if kind == "user":
                        self.fixed.decrypt(rust_container, fixed_plain)
                        self.shipped.decrypt(fixed_container, shipped_plain)
                    else:
                        self.fixed.transform(rust_container, fixed_plain)
                        self.shipped.transform(fixed_container, shipped_plain)
                    self.rust_transform(shipped_container, rust_plain, encrypt=False)
                    self.assertEqual(fixed_plain.read_bytes(), expected, "rebuilt helper decrypt bytes")
                    self.assertEqual(shipped_plain.read_bytes(), expected, "retained Python/shipped helper bytes")
                    self.assertEqual(rust_plain.read_bytes(), expected, "Rust decrypt bytes")
                    print(
                        f"PASS {kind} salt={salt:#x} bytes={len(container)} "
                        f"container_sha256={hashlib.sha256(container).hexdigest()} "
                        "rebuilt/shipped/Python/Rust encrypt+decrypt byte parity"
                    )

    def test_user_container_cli_python_and_rust_parity(self) -> None:
        self.assert_codec_parity("user")

    def test_system_container_cli_python_and_rust_parity(self) -> None:
        self.assert_codec_parity("system")


if __name__ == "__main__":
    unittest.main()