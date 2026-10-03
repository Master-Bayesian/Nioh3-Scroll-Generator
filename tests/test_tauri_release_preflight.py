from __future__ import annotations

import json
from pathlib import Path
import tempfile
import textwrap
import unittest
from unittest.mock import patch

from tools.preflight_tauri_release import inspect_repository, _check_signing_boundary


SHA = "a" * 40
PUBLIC_KEY = "c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU="
PROJECT_URL = "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator"
ARTIFACT = "Nioh3Studio-${{ steps.version.outputs.value }}-win-x64"


class TauriReleasePreflightTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self._write_fixture()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _write(self, relative: str, content: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8", newline="\n")

    def _write_json(self, relative: str, value: object) -> None:
        self._write(relative, json.dumps(value))

    def _write_fixture(self) -> None:
        version = "0.7.3"
        self._write_json("package.json", {"version": version})
        self._write_json("package-lock.json", {"version": version, "packages": {"": {"version": version}}})
        self._write(
            "nioh3_scroll_editor/version.py",
            textwrap.dedent(
                f'''\
                APP_VERSION = "{version}"
                PROJECT_GITHUB_URL = "{PROJECT_URL}"
                UPDATE_PUBLIC_KEY_BASE64 = "{PUBLIC_KEY}"
                '''
            ),
        )
        self._write_json("apps/tauri/src-tauri/tauri.conf.json", {"version": version})
        self._write("apps/tauri/src-tauri/Cargo.toml", f'[package]\nname = "nioh3-studio"\nversion = "{version}"\n')
        self._write("apps/tauri/src-tauri/Cargo.lock", f'[[package]]\nname = "nioh3-studio"\nversion = "{version}"\n')
        self._write("apps/launcher/Cargo.toml", f'[package]\nname = "nioh3-onefile-launcher"\nversion = "{version}"\n')
        self._write("apps/launcher/Cargo.lock", f'[[package]]\nname = "nioh3-onefile-launcher"\nversion = "{version}"\n')
        self._write(
            "tools/build_tauri_update_manifest.mjs",
            f"Buffer.from('{PUBLIC_KEY}','base64'); `{PROJECT_URL}/releases/download/v${{version}}/${{name}}`;",
        )
        self._write(
            "apps/tauri/src-tauri/src/update.rs",
            f'.decode("{PUBLIC_KEY}"); "{PROJECT_URL}/releases/download/v{{}}/{{}}";',
        )
        # The independent Tests workflow keeps its own full unit lanes; the
        # release job must not re-acquire them.
        self._write(
            ".github/workflows/tests.yml",
            textwrap.dedent(
                """\
                name: Tests
                jobs:
                  windows-tests:
                    runs-on: windows-latest
                  rust-crates:
                    runs-on: windows-latest
                  rust-packaging:
                    runs-on: windows-latest
                """
            ),
        )
        self._write(
            ".github/workflows/release.yml",
            textwrap.dedent(
                """\
                on:
                  workflow_dispatch:
                    inputs:
                      extended_search:
                        default: false
                        type: boolean
                permissions:
                  contents: read
                env:
                  NIOH3_REQUIRE_CLEAN_SOURCE: '1'
                jobs:
                  release:
                    if: ${{ github.repository == 'Master-Bayesian/Nioh3-Scroll-Generator' && github.ref == 'refs/heads/main' }}
                    environment: production-signing
                    env:
                      NIOH3_UI_PROFILE: ${{ inputs.extended_search && 'extended' || 'release' }}
                    steps:
                      - name: Check out the candidate source
                        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262
                        with:
                          ref: ${{ github.sha }}
                          persist-credentials: false
                      - name: External build root
                        run: |
                          NIOH3_BUILD_ROOT=$root
                          CARGO_TARGET_DIR=$target
                      - name: Hosted WebView2 runtime
                        run: ./tools/prepare_webview2_test.ps1
                      - name: Synthetic game identity
                        run: ./tools/prepare_ci_game_identity.ps1 -Root $fixtureRoot
                      - name: Shared external Cargo cache
                        uses: Swatinem/rust-cache@v2
                        with:
                          cache-directories: ${{ runner.temp }}/nioh3-release-build/build-cache/tauri-target
                      - name: Build the candidate
                        run: ./tools/build_tauri.ps1 -Python $env:NIOH3_PYTHON -Output deliverables/release/portable
                      - name: Activate the synthetic game identity
                        run: |
                          ProgramFiles(x86)=$programFiles
                      - run: python tools/archive_frontend_v2.py deliverables/release/portable deliverables/release/@ARTIFACT@.zip
                      - run: python tools/build_tauri_onefile.py deliverables/release/@ARTIFACT@.zip deliverables/release/@ARTIFACT@.exe
                      - run: node apps/tauri/verify-packaged-frontend.mjs --profile $env:NIOH3_UI_PROFILE
                      - run: node apps/tauri/verify-onefile.mjs
                      - run: node apps/tauri/verify-onefile-update.mjs
                      - run: node apps/tauri/verify-onefile-rollback.mjs
                      - name: Enforce the download budget and sign the update manifest
                        env:
                          NIOH3_UPDATE_PRIVATE_KEY_BASE64: ${{ secrets.UPDATE_SIGNING_PRIVATE_KEY_BASE64 }}
                        run: |
                          $zip='deliverables/release/@ARTIFACT@.zip'
                          node tools/build_tauri_update_manifest.mjs $zip '0.7.3' notes.md deliverables/release/tauri-update.json
                      - uses: actions/upload-artifact@v4
                        with:
                          name: tauri-candidate-for-diagnosis
                      - if: always()
                        run: |
                          deliverables/release/tauri-update.json
                          deliverables/release/test-inventory.json
                """
            ).replace("@ARTIFACT@", ARTIFACT),
        )

    @patch("tools.preflight_tauri_release._git")
    def test_rejects_untrusted_signing_sources_and_key_exposure(self, git_mock) -> None:
        git_mock.side_effect = self._git_result
        relative = ".github/workflows/release.yml"
        original = (self.root / relative).read_text(encoding="utf-8")
        mutations = {
            "tag ref": ("refs/heads/main", "refs/tags/main"),
            "arbitrary ref": ("refs/heads/main", "refs/heads/candidate"),
            "missing environment": ("environment: production-signing", "environment: other"),
            "moving checkout": ("ref: ${{ github.sha }}", "ref: main"),
            "candidate checkout": ("ref: ${{ github.sha }}", "ref: ${{ inputs.ref }}"),
            "persisted credentials": ("persist-credentials: false", "persist-credentials: true"),
        }
        for label, (before, after) in mutations.items():
            with self.subTest(label=label):
                self._write(relative, original.replace(before, after))
                report = inspect_repository(self.root)
                check = next(item for item in report["checks"] if item["name"] == "signing-source-boundary")
                self.assertFalse(check["ok"], report)
        self._write(relative, original)
        self._write(".github/workflows/untrusted.yml", "env: ${{ secrets.UPDATE_SIGNING_PRIVATE_KEY_BASE64 }}")
        report = inspect_repository(self.root)
        check = next(item for item in report["checks"] if item["name"] == "signing-source-boundary")
        self.assertFalse(check["ok"], report)
        self.assertIn("Unexpected signing key exposure", check["error"])

    def test_actual_repository_signing_boundary(self) -> None:
        result = _check_signing_boundary(Path(__file__).resolve().parents[1])
        self.assertFalse(result["onlineConfigurationVerified"])

    @staticmethod
    def _git_result(_root: Path, *args: str) -> str:
        if args == ("rev-parse", "--show-toplevel"):
            return str(_root)
        if args == ("rev-parse", "HEAD"):
            return SHA
        if args == ("status", "--porcelain=v1", "--untracked-files=all"):
            return ""
        raise AssertionError(args)

    @patch("tools.preflight_tauri_release._git")
    def test_accepts_matching_release_identity(self, git_mock) -> None:
        git_mock.side_effect = self._git_result
        report = inspect_repository(self.root, expected_sha=SHA, require_clean=True)
        self.assertTrue(report["ok"], report)
        self.assertTrue(all(check["ok"] for check in report["checks"]))

    @patch("tools.preflight_tauri_release._git")
    def test_rejects_version_mismatch(self, git_mock) -> None:
        git_mock.side_effect = self._git_result
        self._write_json("apps/tauri/src-tauri/tauri.conf.json", {"version": "0.7.4"})
        report = inspect_repository(self.root)
        check = next(item for item in report["checks"] if item["name"] == "version-consistency")
        self.assertFalse(report["ok"])
        self.assertFalse(check["ok"])
        self.assertIn("do not match", check["error"])

    @patch("tools.preflight_tauri_release._git")
    def test_rejects_update_public_key_drift(self, git_mock) -> None:
        git_mock.side_effect = self._git_result
        self._write("apps/tauri/src-tauri/src/update.rs", '.decode("different");')
        report = inspect_repository(self.root)
        check = next(item for item in report["checks"] if item["name"] == "update-identity")
        self.assertFalse(check["ok"])
        self.assertIn("public key differs", check["error"])

    @patch("tools.preflight_tauri_release._git")
    def test_rejects_coordinated_release_identity_drift(self, git_mock) -> None:
        git_mock.side_effect = self._git_result
        replacement_key = "ZGlmZmVyZW50LXByb2R1Y3Rpb24ta2V5LWlkZW50aXR5ISE="
        replacement_url = "https://github.com/example/fork"
        self._write(
            "nioh3_scroll_editor/version.py",
            textwrap.dedent(
                f'''\
                APP_VERSION = "0.7.3"
                PROJECT_GITHUB_URL = "{replacement_url}"
                UPDATE_PUBLIC_KEY_BASE64 = "{replacement_key}"
                '''
            ),
        )
        self._write(
            "tools/build_tauri_update_manifest.mjs",
            f"Buffer.from('{replacement_key}','base64'); `{replacement_url}/releases/download/v${{version}}/${{name}}`;",
        )
        self._write(
            "apps/tauri/src-tauri/src/update.rs",
            f'.decode("{replacement_key}"); "{replacement_url}/releases/download/v{{}}/{{}}";',
        )
        report = inspect_repository(self.root)
        check = next(item for item in report["checks"] if item["name"] == "update-identity")
        self.assertFalse(check["ok"])
        self.assertIn("official release repository", check["error"])
        self.assertIn("production release key", check["error"])

    @patch("tools.preflight_tauri_release._git")
    def test_clean_candidate_gate_rejects_dirty_checkout(self, git_mock) -> None:
        def dirty_git(root: Path, *args: str) -> str:
            if args == ("status", "--porcelain=v1", "--untracked-files=all"):
                return "?? local-output.zip"
            return self._git_result(root, *args)

        git_mock.side_effect = dirty_git
        report = inspect_repository(self.root, expected_sha=SHA, require_clean=True)
        check = next(item for item in report["checks"] if item["name"] == "git-source")
        self.assertFalse(check["ok"])
        self.assertIn("dirty", check["error"])


if __name__ == "__main__":
    unittest.main()
