"""Deterministic gate for the bounded regular release workflow.

The release job prepares one signed Rust/Tauri candidate from one exact commit.
It is deliberately *not* a second run of the full unit suites and no longer
dispatches the retired Python worker graph, so this gate parses the workflow's
**step graph** out of the YAML and asserts three things a comment cannot:

1. the bounded acceptance legs and the signing step are present, and signing
   follows every acceptance leg;
2. the cheap prerequisites and the shared external build root precede the
   expensive build, and the packaged resources the workflow checks still match
   `tools/stage_rust_workers.py`'s own declarations;
3. no duplicated unit suite, legacy Python lane or PyInstaller invocation came
   back: those belong to the independent Tests workflow.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
STAGER = ROOT / "tools" / "stage_rust_workers.py"

# The only `if:` forms the bounded workflow may carry; anything else would be an
# unevaluated condition rather than a documented step.
BACKEND_INDEPENDENT = ("always()", "failure()", "success()")

# Cheap prerequisites, resolved before the expensive build, with the marker that
# proves the step exists and the step it must precede.
PREREQUISITES = {
    "hosted WebView2 runtime": "./tools/prepare_webview2_test.ps1",
    "synthetic game identity": "prepare_ci_game_identity.ps1",
    "shared external build root": "NIOH3_BUILD_ROOT=$root",
}
BUILD_STEP = "build_tauri.ps1"

# The packaged acceptance legs the regular pipeline still owns.
ACCEPTANCE_GATES = {
    "packaged parity": "npm run test:packaged",
    "packaged host": "node apps/tauri/verify.mjs",
    "add layout": "node apps/tauri/verify-add-layout.mjs",
    "update check": "node apps/tauri/verify-update.mjs",
    "host package record": "node apps/tauri/verify-host-package.mjs",
    "packaged worker identity": "node apps/tauri/verify-worker-identity.mjs",
    "packaged frontend": "node apps/tauri/verify-packaged-frontend.mjs",
    "one-file launch": "node apps/tauri/verify-onefile.mjs",
    "one-file update": "node apps/tauri/verify-onefile-update.mjs",
    "one-file rollback": "node apps/tauri/verify-onefile-rollback.mjs",
}

# Source identity checks that stay on the release job.
SOURCE_GATES = {
    "contracts": "npm run contracts",
    "locale export": "python tools/export_v2_ui_locales.py",
    "locale audit": "node tools/audit_v2_ui_locales.mjs",
    "native build manifest": "python tools/verify_native_build_manifest.py",
    "test inventory": "python tools/write_test_inventory.py",
    "typecheck": "npm run typecheck",
    "native fault gate": "./tools/verify_native_faults.ps1",
}

# Commands the independent Tests workflow owns. Finding one of these in the
# release workflow again means the release job re-acquired a duplicate unit
# pass or the retired development/parity backend lane.
RETIRED_GATES = {
    "retired python backend dispatch": "worker_backend",
    "node unit suite": "npm test",
    "cargo unit suite": "cargo test",
    "full Python suite": "python -m unittest",
    "old-backend regression": "python tools/run_cpu_only_tests.py",
    "title-save research tests": "pytest -q tests\\test_title_save_observer.py",
}

SIGNING_STEP = "build_tauri_update_manifest.mjs $zip"
FAILURE_ARTIFACT = "tauri-candidate-for-diagnosis"

# The fixture is invoked without its environment-export switch; matching the
# invocation keeps the surrounding prose from tripping the guard.
IDENTITY_EXPORT = re.compile(r"prepare_ci_game_identity\.ps1[^\n]*-ExportForActions")


def parse_steps(text: str) -> list[dict]:
    """Slice `jobs.release.steps` into dicts of their scalar keys."""

    offset = text.index("\n  release:\n")
    body = text[offset:]
    steps_start = body.index("\n    steps:\n") + len("\n    steps:\n")
    steps: list[dict] = []
    current: dict | None = None
    block_key: str | None = None
    for line in body[steps_start:].splitlines():
        if line.startswith("      - "):
            current = {}
            steps.append(current)
            block_key = None
            rest = line[len("      - ") :]
            if ":" in rest:
                key, _, value = rest.partition(":")
                current[key.strip()] = value.strip()
            continue
        if current is None:
            continue
        if not line.strip():
            block_key = None if block_key is None else block_key
            continue
        indent = len(line) - len(line.lstrip(" "))
        if indent == 8 and ":" in line:
            key, _, value = line.strip().partition(":")
            key = key.strip()
            value = value.strip()
            if value in ("|", ">", "|-", ">-"):
                current[key] = ""
                block_key = key
            else:
                current[key] = value
                block_key = key
            continue
        if block_key is not None and indent >= 10:
            current[block_key] = (current.get(block_key, "") + "\n" + line.strip()).strip()
    return steps


def joined(steps: list[dict]) -> str:
    """Every captured key of every step, so `env:`/`with:` count as content."""

    return "\n".join(f"{key}: {value}" for step in steps for key, value in step.items())


def load_stager():
    spec = importlib.util.spec_from_file_location("stage_rust_workers", STAGER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class ReleaseWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.steps = parse_steps(cls.text)
        cls.body = joined(cls.steps)

    def test_the_workflow_is_manual_dispatch_only(self) -> None:
        on_block = self.text.split("permissions:", 1)[0]
        self.assertIn("workflow_dispatch:", on_block)
        for trigger in ("push:", "pull_request:", "schedule:"):
            self.assertNotIn(trigger, on_block)

    def test_every_step_condition_is_one_the_gate_can_evaluate(self) -> None:
        for step in self.steps:
            condition = step.get("if")
            if condition is not None:
                self.assertIn(condition, BACKEND_INDEPENDENT, step.get("name"))

    def test_the_bounded_acceptance_graph_is_present(self) -> None:
        for name, marker in {**SOURCE_GATES, **ACCEPTANCE_GATES}.items():
            self.assertIn(marker, self.body, f"the release job lost {name}")
        # The shipped Rust graph is asserted on the packaged output.
        for marker in ("OPTIN_MANIFEST_MISSING", "OPTIN_PYTHON_RESOURCE_PRESENT", "OPTIN_PYINSTALLER_OUTPUT_PRESENT"):
            self.assertIn(marker, self.body)
        self.assertIn("-WorkerBackend rust", self.body)
        self.assertIn("NIOH3_WORKER_BACKEND: rust", self.body)

    def test_cheap_prerequisites_precede_the_expensive_build(self) -> None:
        build = self.body.index(BUILD_STEP)
        for name, marker in PREREQUISITES.items():
            self.assertIn(marker, self.body, f"the release job lost the {name} step")
            self.assertLess(self.body.index(marker), build, f"{name} must precede the build")
        # The synthetic Steam root is activated only after the build: exporting
        # it as `ProgramFiles(x86)` earlier would repoint the compiler and SDK
        # discovery Cargo/MSVC rely on.
        self.assertIsNone(
            IDENTITY_EXPORT.search(self.body),
            "the synthetic game identity must not be exported before the build",
        )
        self.assertIn("ProgramFiles(x86)=$programFiles", self.body)
        self.assertLess(
            build,
            self.body.index("ProgramFiles(x86)=$programFiles"),
            "the synthetic game identity must be activated after the build",
        )

    def test_signing_follows_every_acceptance_leg(self) -> None:
        signing = self.body.index(SIGNING_STEP)
        for name, marker in ACCEPTANCE_GATES.items():
            self.assertLess(self.body.index(marker), signing, f"{name} must precede signing")
        # The exact unsigned candidate is retained for retest only after the
        # signed artifact exists, so a failure never publishes signed bytes.
        self.assertLess(signing, self.body.index(FAILURE_ARTIFACT))

    def test_the_dispatch_profile_is_explicit_and_bounded(self) -> None:
        self.assertIn("extended_search:", self.text)
        self.assertIn("type: boolean", self.text)
        self.assertIn("default: false", self.text)
        self.assertIn(
            "NIOH3_UI_PROFILE: ${{ inputs.extended_search && 'extended' || 'release' }}",
            self.text,
        )
        self.assertIn("--profile $env:NIOH3_UI_PROFILE", self.body)

    def test_the_shared_external_cargo_target_is_mapped(self) -> None:
        self.assertIn("NIOH3_BUILD_ROOT=$root", self.body)
        self.assertIn("CARGO_TARGET_DIR=$target", self.body)
        self.assertIn(
            "cache-directories: ${{ runner.temp }}/nioh3-release-build/build-cache/tauri-target",
            self.body,
        )

    def test_the_duplicated_unit_suites_and_legacy_lane_stay_retired(self) -> None:
        for name, marker in RETIRED_GATES.items():
            self.assertNotIn(marker, self.body, f"the release job re-added the {name}")

    def test_no_workflow_step_invokes_pyinstaller(self) -> None:
        # PyInstaller lives behind `build_tauri.ps1`'s python branch, so the
        # workflow never invokes it. The check is on invocations, not on the
        # word: the Rust graph legitimately *names* the spec files it requires
        # to be absent.
        body = self.body.lower()
        for invocation in ("-m pyinstaller", "pyinstaller --", "pyinstaller.exe", "pyinstaller.cmd"):
            self.assertNotIn(invocation, body, f"the release job invokes {invocation!r}")
        for spec in ("packaging/search-worker.spec", "packaging/protected-worker.spec"):
            self.assertEqual(
                self.body.count(spec), 1, f"{spec} must appear only in the exclusion check"
            )

    def test_the_rust_graph_assertion_matches_the_stager_declarations(self) -> None:
        stager = load_stager()
        # Every packaged binary name the stager produces must be checked, and
        # the exclusions it declares must be the ones the workflow requires.
        for _cargo, packaged, _crate in stager.ROLES:
            self.assertIn(
                packaged,
                self.body,
                f"the workflow does not assert the packaged name {packaged}",
            )
        for spec in ("packaging/search-worker.spec", "packaging/protected-worker.spec"):
            self.assertIn(spec, self.body, f"the workflow does not require {spec} be excluded")
        self.assertIn("$manifest.backend -ne 'rust'", self.body)

    def test_the_workflow_and_stager_agree_on_the_marker_files(self) -> None:
        # The stager refuses to mix backends when these exist; the workflow
        # asserts the packaged output does not contain them.
        self.assertIn("python-build-environment.json", self.body)
        self.assertIn("_internal", self.body)


if __name__ == "__main__":
    unittest.main()
