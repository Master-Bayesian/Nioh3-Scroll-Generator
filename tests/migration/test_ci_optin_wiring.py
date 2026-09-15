"""Deterministic gate for the packaged Rust graph's CI wiring (no YAML dependency).

The packaged product selects the Rust worker from the staged manifest, so this
test asserts three things about the workflow files rather than trusting review:
the packaging job exists with the real build/stage/identity commands and an
explicit shared Cargo target, that job contains no PyInstaller step, and the
release workflow defaults to the Rust graph while keeping every shipped
identity, size-budget and signing step.
"""

from __future__ import annotations

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
TESTS_WORKFLOW = ROOT / ".github" / "workflows" / "tests.yml"
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"


def job_block(text: str, job: str) -> str:
    """Slice one top-level job block out of a workflow by indentation."""

    marker = f"\n  {job}:\n"
    start = text.index(marker)
    rest = text[start + len(marker) :]
    lines = rest.splitlines(keepends=True)
    body = []
    for line in lines:
        if line.strip() and not line.startswith("    "):
            break
        body.append(line)
    return "".join(body)


class CiOptInWiringTests(unittest.TestCase):
    def test_packaging_job_builds_stages_and_verifies_the_rust_workers(self) -> None:
        text = TESTS_WORKFLOW.read_text(encoding="utf-8")
        block = job_block(text, "rust-packaging")
        for expected in (
            "CARGO_TARGET_DIR = Join-Path $env:RUNNER_TEMP 'rust-packaging-cargo-target'",
            "--manifest-path crates/nioh3-worker/Cargo.toml --bin nioh3-readonly-worker",
            "--manifest-path crates/nioh3-protected/Cargo.toml --bin nioh3-protected-worker",
            "tests/migration/test_packaging_optin.py",
            "tests/migration/test_worker_identity_optin.py",
            "NIOH3_M4_IDENTITY_EVIDENCE",
            "runner.temp",
        ):
            self.assertIn(expected, block, f"the packaging job must contain {expected!r}")

    def test_packaging_job_carries_no_pyinstaller_artifact(self) -> None:
        block = job_block(TESTS_WORKFLOW.read_text(encoding="utf-8"), "rust-packaging")
        # The job may *describe* the absent artifacts, but it must not run them.
        for command in (
            "-m PyInstaller",
            "-m pyinstaller",
            "packaging/search-worker.spec",
            "packaging/protected-worker.spec",
            "write_v2_python_manifest.py",
        ):
            self.assertNotIn(command, block, f"the packaging job must not run {command!r}")

    def test_the_release_workflow_defaults_to_the_rust_graph(self) -> None:
        # The release workflow now packages the Rust worker by default: the
        # input defaults to `rust`, the Rust-only steps are conditional so the
        # development/parity `python` selection still has a complete step set,
        # and the build, archive and signing markers are all still present.
        release = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("m4-optin", release)
        self.assertIn("default: rust", release)
        self.assertIn("if: inputs.worker_backend == 'rust'", release)
        self.assertIn("build_tauri.ps1 -Python $env:NIOH3_PYTHON -Output", release)
        self.assertIn("-WorkerBackend ${{ inputs.worker_backend }}", release)
        self.assertIn("nioh3-search-worker.exe", release)
        # The published identity path is unchanged.
        self.assertIn("python tools/archive_frontend_v2.py deliverables/release/portable", release)
        self.assertIn("node tools/build_tauri_update_manifest.mjs $zip", release)
        tests = TESTS_WORKFLOW.read_text(encoding="utf-8")
        default_block = job_block(tests, "windows-tests")
        self.assertIn("./tools/run_python_tests.ps1", default_block)
        self.assertIn("crates/nioh3-protected/Cargo.toml", default_block)


if __name__ == "__main__":
    unittest.main()
