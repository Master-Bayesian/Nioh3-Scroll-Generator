import base64
import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

from nioh3_scroll_editor.updater import (
    MANAGED_INSTALL_MARKER,
    DownloadedUpdate,
    UpdateManifest,
    check_for_update,
    download_update,
    prepare_managed_update_script,
)
from nioh3_scroll_editor.version import APP_ID


class MemoryResponse(io.BytesIO):
    def __enter__(self) -> "MemoryResponse":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


class UpdaterTests(unittest.TestCase):
    @staticmethod
    def _signed_manifest(asset: bytes, version: str = "1.2.0") -> tuple[dict[str, object], str]:
        private_key = Ed25519PrivateKey.generate()
        public_key = private_key.public_key().public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        manifest = UpdateManifest(
            version=version,
            published_at_utc="2026-08-29T00:00:00Z",
            notes="Test release",
            asset_name="Nioh3ScrollGenerator.exe",
            asset_url="https://example.invalid/Nioh3ScrollGenerator.exe",
            asset_size=len(asset),
            asset_sha256=hashlib.sha256(asset).hexdigest().upper(),
            signature_base64=base64.b64encode(bytes(64)).decode("ascii"),
        )
        signature = private_key.sign(manifest.signed_payload())
        value = {
            "schema": 1,
            "version": manifest.version,
            "published_at_utc": manifest.published_at_utc,
            "notes": manifest.notes,
            "asset": {
                "name": manifest.asset_name,
                "url": manifest.asset_url,
                "size": manifest.asset_size,
                "sha256": manifest.asset_sha256,
            },
            "signature": base64.b64encode(signature).decode("ascii"),
        }
        return value, base64.b64encode(public_key).decode("ascii")

    def test_signed_manifest_check_detects_newer_release(self) -> None:
        value, public_key = self._signed_manifest(b"release")
        payload = json.dumps(value).encode("utf-8")

        result = check_for_update(
            "1.1.9",
            "https://example.invalid/latest.json",
            public_key,
            open_url=lambda _url, _timeout: MemoryResponse(payload),
        )

        self.assertTrue(result.update_available)
        self.assertEqual(result.manifest.version, "1.2.0")

    def test_manifest_tampering_fails_signature_gate(self) -> None:
        value, public_key = self._signed_manifest(b"release")
        value["version"] = "9.9.9"
        payload = json.dumps(value).encode("utf-8")

        with self.assertRaisesRegex(ValueError, "signature"):
            check_for_update(
                "1.1.9",
                "https://example.invalid/latest.json",
                public_key,
                open_url=lambda _url, _timeout: MemoryResponse(payload),
            )

    def test_download_requires_exact_signed_size_and_hash(self) -> None:
        asset = b"signed executable bytes"
        value, _public_key = self._signed_manifest(asset)
        manifest = UpdateManifest.from_mapping(value)
        with tempfile.TemporaryDirectory() as directory:
            state_root = Path(directory)
            downloaded = download_update(
                manifest,
                state_root,
                open_url=lambda _url, _timeout: MemoryResponse(asset),
            )
            self.assertEqual(downloaded.path.read_bytes(), asset)

            bad_manifest = UpdateManifest(
                version="1.2.1",
                published_at_utc=manifest.published_at_utc,
                notes=manifest.notes,
                asset_name=manifest.asset_name,
                asset_url=manifest.asset_url,
                asset_size=len(asset),
                asset_sha256="0" * 64,
                signature_base64=manifest.signature_base64,
            )
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                download_update(
                    bad_manifest,
                    state_root,
                    open_url=lambda _url, _timeout: MemoryResponse(asset),
                )

    def test_managed_update_script_is_restricted_to_marked_install(self) -> None:
        asset = b"new executable"
        value, _public_key = self._signed_manifest(asset)
        manifest = UpdateManifest.from_mapping(value)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            state_root = root / "state"
            source = state_root / "updates" / manifest.version / manifest.asset_name
            source.parent.mkdir(parents=True)
            source.write_bytes(asset)
            downloaded = DownloadedUpdate(manifest=manifest, path=source)
            install_root = root / "managed"
            install_root.mkdir()
            executable = install_root / "Nioh3ScrollGenerator.exe"
            executable.write_bytes(b"old executable")

            with self.assertRaisesRegex(RuntimeError, "managed installer"):
                prepare_managed_update_script(
                    downloaded,
                    current_executable=executable,
                    state_root=state_root,
                )

            (install_root / MANAGED_INSTALL_MARKER).write_text(
                json.dumps(
                    {
                        "schema": 1,
                        "app_id": APP_ID,
                        "executable": executable.name,
                    }
                ),
                encoding="utf-8",
            )
            script = prepare_managed_update_script(
                downloaded,
                current_executable=executable,
                state_root=state_root,
            )
            text = script.read_text(encoding="utf-8-sig")
            self.assertIn("Wait-Process", text)
            self.assertIn("Get-FileHash -LiteralPath", text)
            self.assertIn("Move-Item -LiteralPath", text)
            self.assertNotIn(str(executable), text)


if __name__ == "__main__":
    unittest.main()
