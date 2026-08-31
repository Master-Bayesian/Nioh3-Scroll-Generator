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
    ensure_managed_install,
    prepare_managed_update_script,
    release_version_tuple,
    validate_managed_install,
)
from nioh3_scroll_editor.app_settings import UPDATE_CHANNEL_BETA
from nioh3_scroll_editor.version import APP_ID
from tools.build_update_manifest import build_manifest


class MemoryResponse(io.BytesIO):
    def __enter__(self) -> "MemoryResponse":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


class UpdaterTests(unittest.TestCase):
    @staticmethod
    def _signed_manifest(
        asset: bytes,
        version: str = "1.2.0",
        *,
        private_key: Ed25519PrivateKey | None = None,
    ) -> tuple[dict[str, object], str]:
        private_key = private_key or Ed25519PrivateKey.generate()
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

    def test_release_version_order_includes_beta_rc_and_stable(self) -> None:
        self.assertLess(
            release_version_tuple("1.2.0-beta.1"),
            release_version_tuple("1.2.0-beta.2"),
        )
        self.assertLess(
            release_version_tuple("1.2.0-beta.9"),
            release_version_tuple("1.2.0-rc.1"),
        )
        self.assertLess(
            release_version_tuple("1.2.0-rc.2"),
            release_version_tuple("1.2.0"),
        )
        self.assertLess(
            release_version_tuple("1.2.0"),
            release_version_tuple("1.2.1-beta.1"),
        )

    def test_beta_channel_selects_newest_signed_prerelease(self) -> None:
        private_key = Ed25519PrivateKey.generate()
        stable_value, public_key = self._signed_manifest(
            b"stable",
            "1.2.0",
            private_key=private_key,
        )
        beta_value, _ = self._signed_manifest(
            b"beta",
            "1.3.0-beta.2",
            private_key=private_key,
        )
        release_index = [
            {
                "draft": False,
                "prerelease": True,
                "tag_name": "v1.3.0-beta.2",
                "assets": [
                    {
                        "name": "latest.json",
                        "browser_download_url": "https://example.invalid/beta.json",
                    }
                ],
            }
        ]
        payload_by_url = {
            "https://example.invalid/stable.json": json.dumps(stable_value).encode(),
            "https://api.github.com/repos/example/releases?per_page=20": json.dumps(
                release_index
            ).encode(),
            "https://example.invalid/beta.json": json.dumps(beta_value).encode(),
        }

        result = check_for_update(
            "1.2.0",
            "https://example.invalid/stable.json",
            public_key,
            channel=UPDATE_CHANNEL_BETA,
            releases_api_url=(
                "https://api.github.com/repos/example/releases?per_page=20"
            ),
            open_url=lambda url, _timeout: MemoryResponse(payload_by_url[url]),
        )

        self.assertTrue(result.update_available)
        self.assertEqual(result.channel, UPDATE_CHANNEL_BETA)
        self.assertEqual(result.manifest.version, "1.3.0-beta.2")

    def test_beta_channel_prefers_newer_stable_release(self) -> None:
        private_key = Ed25519PrivateKey.generate()
        stable_value, public_key = self._signed_manifest(
            b"stable",
            "1.3.0",
            private_key=private_key,
        )
        beta_value, _ = self._signed_manifest(
            b"beta",
            "1.3.0-beta.2",
            private_key=private_key,
        )
        release_index = [
            {
                "draft": False,
                "prerelease": True,
                "tag_name": "v1.3.0-beta.2",
                "assets": [
                    {
                        "name": "latest.json",
                        "browser_download_url": "https://example.invalid/beta.json",
                    }
                ],
            }
        ]
        payload_by_url = {
            "https://example.invalid/stable.json": json.dumps(stable_value).encode(),
            "https://api.github.com/repos/example/releases?per_page=20": json.dumps(
                release_index
            ).encode(),
            "https://example.invalid/beta.json": json.dumps(beta_value).encode(),
        }

        result = check_for_update(
            "1.2.9",
            "https://example.invalid/stable.json",
            public_key,
            channel=UPDATE_CHANNEL_BETA,
            releases_api_url=(
                "https://api.github.com/repos/example/releases?per_page=20"
            ),
            open_url=lambda url, _timeout: MemoryResponse(payload_by_url[url]),
        )

        self.assertEqual(result.manifest.version, "1.3.0")

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

    def test_release_manifest_builder_matches_runtime_verifier(self) -> None:
        private_key = Ed25519PrivateKey.generate()
        private_value = base64.b64encode(
            private_key.private_bytes_raw()
        ).decode("ascii")
        public_value = base64.b64encode(
            private_key.public_key().public_bytes_raw()
        ).decode("ascii")
        with tempfile.TemporaryDirectory() as directory:
            asset = Path(directory) / "Nioh3ScrollGenerator.exe"
            asset.write_bytes(b"signed release executable")
            value = build_manifest(
                version="1.2.0",
                asset=asset,
                asset_url="https://example.invalid/releases/v1.2.0/Nioh3ScrollGenerator.exe",
                notes="Release notes",
                private_key_base64=private_value,
                published_at_utc="2026-08-29T00:00:00Z",
            )
            payload = json.dumps(value).encode("utf-8")

            result = check_for_update(
                "1.1.9",
                "https://example.invalid/latest.json",
                public_value,
                open_url=lambda _url, _timeout: MemoryResponse(payload),
            )

            self.assertTrue(result.update_available)
            self.assertEqual(result.manifest.asset_sha256, hashlib.sha256(asset.read_bytes()).hexdigest().upper())

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
                        "channel": "stable",
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

    def test_portable_executable_self_enrollment_is_exact_and_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "Nioh3ScrollGenerator.exe"
            executable.write_bytes(b"portable executable")

            marker = ensure_managed_install(executable)
            original_marker = marker.read_bytes()

            self.assertEqual(validate_managed_install(executable), executable.resolve())
            self.assertEqual(ensure_managed_install(executable), marker)
            self.assertEqual(marker.read_bytes(), original_marker)
            value = json.loads(marker.read_text(encoding="utf-8"))
            self.assertEqual(value["channel"], "stable")
            self.assertEqual(value["executable"], executable.name)

    def test_self_enrollment_never_overwrites_an_invalid_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "Nioh3ScrollGenerator.exe"
            executable.write_bytes(b"portable executable")
            marker = executable.parent / MANAGED_INSTALL_MARKER
            marker.write_text('{"app_id":"another-application"}', encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "another application"):
                ensure_managed_install(executable)
            self.assertEqual(
                marker.read_text(encoding="utf-8"),
                '{"app_id":"another-application"}',
            )


if __name__ == "__main__":
    unittest.main()
