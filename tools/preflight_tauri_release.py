"""Run deterministic, read-only source checks for the Tauri release workflow."""

from __future__ import annotations

import argparse
import ast
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib
from typing import Any, Callable


SHA_PATTERN = re.compile(r"[0-9a-f]{40}")
VERSION_PATTERN = re.compile(r"\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?")
# The game-identity fixture must be created and read early *without* exporting
# the synthetic Steam root; matching the invocation keeps a prose mention of the
# switch from tripping the guard.
IDENTITY_EXPORT_PATTERN = re.compile(r"prepare_ci_game_identity\.ps1[^\n]*-ExportForActions")
TAURI_PACKAGE = "nioh3-studio"
LAUNCHER_PACKAGE = "nioh3-onefile-launcher"
ARTIFACT_STEM = "Nioh3Studio-${{ steps.version.outputs.value }}-win-x64"
EXPECTED_PROJECT_URL = "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator"
EXPECTED_PUBLIC_KEY_BASE64 = "c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU="


def _json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as stream:
        return json.load(stream)


def _toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as stream:
        return tomllib.load(stream)


def _python_constants(path: Path) -> dict[str, str]:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    constants: dict[str, str] = {}
    for node in tree.body:
        if not isinstance(node, ast.Assign) or len(node.targets) != 1:
            continue
        target = node.targets[0]
        if isinstance(target, ast.Name) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
            constants[target.id] = node.value.value
    return constants


def _locked_package_version(path: Path, package_name: str) -> str:
    matches = [entry["version"] for entry in _toml(path).get("package", []) if entry.get("name") == package_name]
    if len(matches) != 1:
        raise ValueError(f"Expected one {package_name!r} package in {path}")
    return str(matches[0])


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(root), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    return completed.stdout.strip()


def _check_versions(root: Path) -> dict[str, Any]:
    package = _json(root / "package.json")
    package_lock = _json(root / "package-lock.json")
    python_constants = _python_constants(root / "nioh3_scroll_editor/version.py")
    tauri_config = _json(root / "apps/tauri/src-tauri/tauri.conf.json")
    versions = {
        "package.json": str(package["version"]),
        "package-lock.json": str(package_lock["version"]),
        "package-lock.json#packages[empty]": str(package_lock["packages"][""]["version"]),
        "nioh3_scroll_editor/version.py": python_constants["APP_VERSION"],
        "apps/tauri/src-tauri/tauri.conf.json": str(tauri_config["version"]),
        "apps/tauri/src-tauri/Cargo.toml": str(_toml(root / "apps/tauri/src-tauri/Cargo.toml")["package"]["version"]),
        "apps/tauri/src-tauri/Cargo.lock": _locked_package_version(
            root / "apps/tauri/src-tauri/Cargo.lock", TAURI_PACKAGE
        ),
        "apps/launcher/Cargo.toml": str(_toml(root / "apps/launcher/Cargo.toml")["package"]["version"]),
        "apps/launcher/Cargo.lock": _locked_package_version(
            root / "apps/launcher/Cargo.lock", LAUNCHER_PACKAGE
        ),
    }
    unique = sorted(set(versions.values()))
    if len(unique) != 1 or VERSION_PATTERN.fullmatch(unique[0]) is None:
        raise ValueError(f"Release versions do not match: {versions}")
    return {"version": unique[0], "sources": versions}


def _check_update_identity(root: Path) -> dict[str, Any]:
    constants = _python_constants(root / "nioh3_scroll_editor/version.py")
    project_url = constants["PROJECT_GITHUB_URL"].rstrip("/")
    public_key = constants["UPDATE_PUBLIC_KEY_BASE64"]
    signer = (root / "tools/build_tauri_update_manifest.mjs").read_text(encoding="utf-8")
    updater = (root / "apps/tauri/src-tauri/src/update.rs").read_text(encoding="utf-8")
    expected_url = f"{project_url}/releases/download/v${{version}}/${{name}}"
    failures = []
    if project_url != EXPECTED_PROJECT_URL:
        failures.append("application repository differs from the official release repository")
    if public_key != EXPECTED_PUBLIC_KEY_BASE64:
        failures.append("application public key differs from the production release key")
    if f"Buffer.from('{public_key}','base64')" not in signer:
        failures.append("signer public key differs from application configuration")
    if expected_url not in signer:
        failures.append("signer repository URL differs from application configuration")
    if f'.decode("{public_key}")' not in updater:
        failures.append("Tauri updater public key differs from application configuration")
    repository_path = project_url.removeprefix("https://github.com")
    if f'"https://github.com{repository_path}/releases/download/v{{}}/{{}}"' not in updater:
        failures.append("Tauri updater repository URL differs from application configuration")
    if failures:
        raise ValueError("; ".join(failures))
    return {"projectUrl": project_url, "publicKeyBase64": public_key}


def _contains(text: str, marker: str | tuple[str, ...]) -> bool:
    """True when any accepted spelling of one contract marker is present."""

    candidates = marker if isinstance(marker, tuple) else (marker,)
    return any(candidate in text for candidate in candidates)


def _ordered(text: str, earlier: str, later: str) -> bool:
    """True when both markers exist and `earlier` precedes `later`."""

    first = text.find(earlier)
    second = text.find(later)
    return first != -1 and second != -1 and first < second


def _check_workflow(root: Path) -> dict[str, Any]:
    """The bounded release pipeline the workflow must still describe.

    This is the regular Rust/Tauri preparation graph: cheap source and identity
    checks, one shared external build root, the packaged acceptance legs, and a
    signing step that runs last. The full unit suites belong to the independent
    Tests workflow, so their commands are asserted *absent* here rather than
    counted: a release run that pays for them again is the regression.
    """

    workflow = (root / ".github/workflows/release.yml").read_text(encoding="utf-8").replace("\r\n", "\n")
    tests = (root / ".github/workflows/tests.yml").read_text(encoding="utf-8").replace("\r\n", "\n")
    required = {
        # `workflow_dispatch` may be written inline or as a mapping that carries
        # its inputs; both are manual-only and both are accepted here.
        "manual dispatch only": ("on: workflow_dispatch", "workflow_dispatch:"),
        "read-only contents permission": "contents: read",
        "clean source required": "NIOH3_REQUIRE_CLEAN_SOURCE: '1'",
        "archive identity": (
            "python tools/archive_frontend_v2.py deliverables/release/portable "
            f"deliverables/release/{ARTIFACT_STEM}.zip"
        ),
        "one-file identity": (
            f"python tools/build_tauri_onefile.py deliverables/release/{ARTIFACT_STEM}.zip "
            f"deliverables/release/{ARTIFACT_STEM}.exe"
        ),
        "signed ZIP identity": f"$zip='deliverables/release/{ARTIFACT_STEM}.zip'",
        "production manifest builder": "node tools/build_tauri_update_manifest.mjs $zip",
        "outer verification": "node apps/tauri/verify-onefile.mjs",
        "outer update verification": "node apps/tauri/verify-onefile-update.mjs",
        "outer rollback verification": "node apps/tauri/verify-onefile-rollback.mjs",
        "packaged frontend verification": "node apps/tauri/verify-packaged-frontend.mjs",
        "update manifest upload": "deliverables/release/tauri-update.json",
        "test inventory upload": "deliverables/release/test-inventory.json",
        # The bounded dispatch contract: one explicit, defaulted acceptance
        # profile rather than a full extended run on every preparation.
        "extended search input": "extended_search:",
        "boolean dispatch input": "type: boolean",
        "bounded profile default": "default: false",
        "explicit profile mapping": (
            "NIOH3_UI_PROFILE: ${{ inputs.extended_search && 'extended' || 'release' }}"
        ),
        "profile flag": "--profile $env:NIOH3_UI_PROFILE",
        # One shared external build root and Cargo target for every step, cached
        # by its real path instead of the unused workspace `target/` dirs.
        "shared build root": "NIOH3_BUILD_ROOT=$root",
        "shared Cargo target": "CARGO_TARGET_DIR=$target",
        "cached Cargo target": (
            "cache-directories: ${{ runner.temp }}/nioh3-release-build/build-cache/tauri-target"
        ),
        # Failure evidence: the exact unsigned candidate bytes and the
        # acceptance evidence are retained separately from the signed artifact.
        "durable failure candidate": "tauri-candidate-for-diagnosis",
        "retained acceptance evidence": "if: always()",
        # The synthetic Steam root is activated explicitly, after the build.
        "late game identity activation": "ProgramFiles(x86)=$programFiles",
    }
    # Commands the independent Tests workflow already runs. Finding them here
    # again means the release job re-acquired a duplicate unit pass or the
    # retired Python backend lane.
    retired = {
        "retired Python backend dispatch": "worker_backend",
        "duplicated npm unit suite": "npm test",
        "duplicated cargo unit suite": "cargo test",
        "legacy full Python suite": "python -m unittest",
        "legacy old-backend regression": "run_cpu_only_tests.py",
        "PyInstaller worker build": "-m pyinstaller",
    }
    # Cheap prerequisites and the shared environment must precede the expensive
    # build, and signing must follow every acceptance leg.
    order = {
        "hosted WebView2 before the build": ("./tools/prepare_webview2_test.ps1", "build_tauri.ps1"),
        "synthetic game identity before the build": ("prepare_ci_game_identity.ps1", "build_tauri.ps1"),
        "shared build root before the build": ("NIOH3_BUILD_ROOT=$root", "build_tauri.ps1"),
        "packaged frontend acceptance before signing": (
            "verify-packaged-frontend.mjs",
            "build_tauri_update_manifest.mjs",
        ),
        "one-file rollback acceptance before signing": (
            "verify-onefile-rollback.mjs",
            "build_tauri_update_manifest.mjs",
        ),
        "failure candidate retained after signing": (
            "build_tauri_update_manifest.mjs",
            "tauri-candidate-for-diagnosis",
        ),
        "game identity activated only after the build": (
            "build_tauri.ps1",
            "ProgramFiles(x86)=$programFiles",
        ),
    }
    independent = {
        "windows-tests": "windows-tests:",
        "rust-crates": "rust-crates:",
        "rust-packaging": "rust-packaging:",
    }

    failures = []
    missing = [name for name, marker in required.items() if not _contains(workflow, marker)]
    if missing:
        failures.append("Release workflow contract is missing: " + ", ".join(missing))
    returned = [name for name, marker in retired.items() if _contains(workflow, marker)]
    if returned:
        failures.append("Release workflow re-added retired or duplicated gates: " + ", ".join(returned))
    misordered = [
        name for name, (earlier, later) in order.items() if not _ordered(workflow, earlier, later)
    ]
    if misordered:
        failures.append("Release workflow step order is wrong: " + ", ".join(misordered))
    if IDENTITY_EXPORT_PATTERN.search(workflow) is not None:
        failures.append(
            "The synthetic game identity must be created and read early without "
            "-ExportForActions; exporting it as ProgramFiles(x86) before the build "
            "would repoint compiler and SDK discovery"
        )
    absent_jobs = [name for name, marker in independent.items() if not _contains(tests, marker)]
    if absent_jobs:
        failures.append(
            "The independent Tests workflow no longer owns the full unit lanes: "
            + ", ".join(absent_jobs)
        )
    if failures:
        raise ValueError("; ".join(failures))
    return {
        "artifactStem": ARTIFACT_STEM,
        "markers": sorted(required),
        "retiredAbsent": sorted(retired),
        "ordered": sorted(order),
        "independentTestsJobs": sorted(independent),
    }


def _check_git(root: Path, expected_sha: str | None, require_clean: bool) -> dict[str, Any]:
    top_level = Path(_git(root, "rev-parse", "--show-toplevel")).resolve()
    if os.path.normcase(str(top_level)) != os.path.normcase(str(root.resolve())):
        raise ValueError(f"Repository root mismatch: {top_level}")
    head = _git(root, "rev-parse", "HEAD")
    if SHA_PATTERN.fullmatch(head) is None:
        raise ValueError(f"HEAD is not a full commit SHA: {head!r}")
    if expected_sha is not None and head != expected_sha:
        raise ValueError(f"HEAD {head} does not equal expected candidate {expected_sha}")
    status = _git(root, "status", "--porcelain=v1", "--untracked-files=all")
    dirty_entries = len(status.splitlines()) if status else 0
    if require_clean and dirty_entries:
        raise ValueError(f"Release candidate checkout is dirty ({dirty_entries} status entries)")
    return {"head": head, "clean": dirty_entries == 0, "dirtyEntries": dirty_entries, "cleanRequired": require_clean}


def inspect_repository(root: Path, expected_sha: str | None = None, require_clean: bool = False) -> dict[str, Any]:
    root = root.resolve()
    checks: list[dict[str, Any]] = []

    def record(name: str, check: Callable[[], dict[str, Any]]) -> None:
        try:
            checks.append({"name": name, "ok": True, "details": check()})
        except (FileNotFoundError, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
            checks.append({"name": name, "ok": False, "error": str(error)})

    record("version-consistency", lambda: _check_versions(root))
    record("update-identity", lambda: _check_update_identity(root))
    record("release-workflow", lambda: _check_workflow(root))
    record("git-source", lambda: _check_git(root, expected_sha, require_clean))
    return {"schema": "nioh3-tauri-release-preflight/v1", "root": str(root), "ok": all(item["ok"] for item in checks), "checks": checks}


def _full_sha(value: str) -> str:
    if SHA_PATTERN.fullmatch(value) is None:
        raise argparse.ArgumentTypeError("expected a full lowercase 40-character commit SHA")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--expected-sha", type=_full_sha)
    parser.add_argument("--require-clean", action="store_true")
    args = parser.parse_args()
    report = inspect_repository(args.repo, args.expected_sha, args.require_clean)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
