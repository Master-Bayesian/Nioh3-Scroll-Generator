"""Deterministic gate for the release workflow's worker-backend branches.

This evaluates the workflow's **step graph** rather than counting substrings:
the steps are parsed out of the YAML, each step's `if:` is resolved for both
`worker_backend` values, and the resulting step lists are asserted. The packaged
resource names the Rust branch checks are cross-checked against the stager's own
declarations, so the workflow cannot drift from the tool it calls.

It also asserts the thing that matters most now that the Rust graph is the
shipped default: `rust` is the dispatch default, selecting `python` yields the
development/parity step set, and every retired-Python requirement (contracts,
locales, native manifest, tests, native faults, size budget, signing, one-file
identity) is retained on **both** branches.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
STAGER = ROOT / "tools" / "stage_rust_workers.py"

# The `if:` forms this gate can evaluate. Anything else fails the gate rather
# than being silently treated as always-on.
RUST_ONLY = "inputs.worker_backend == 'rust'"
PYTHON_ONLY = "inputs.worker_backend != 'rust'"
# Conditions that are backend-independent.
BACKEND_INDEPENDENT = ("always()", "failure()", "success()")

# Gates that must survive on either backend, with the marker that proves the
# command is really present in that step list.
RETAINED_GATES = {
    "contracts": "npm run contracts",
    "locale export": "python tools/export_v2_ui_locales.py",
    "locale audit": "node tools/audit_v2_ui_locales.mjs",
    "native build manifest": "python tools/verify_native_build_manifest.py",
    "test inventory": "python tools/write_test_inventory.py",
    "node tests": "npm test",
    "typecheck": "npm run typecheck",
    "native fault gate": "./tools/verify_native_faults.ps1",
    "tauri crate tests": "cargo test --locked --manifest-path apps/tauri/src-tauri/Cargo.toml",
    "launcher crate tests": "cargo test --locked --manifest-path apps/launcher/Cargo.toml",
    "clean checkout build": "build_tauri.ps1 -Python $env:NIOH3_PYTHON -Output",
    "archive identity": "python tools/archive_frontend_v2.py ",
    "one-file identity": "python tools/build_tauri_onefile.py ",
    "outer verification": "node apps/tauri/verify-onefile.mjs",
    "outer update verification": "node apps/tauri/verify-onefile-update.mjs",
    "outer rollback verification": "node apps/tauri/verify-onefile-rollback.mjs",
    "size budget and signing": "build_tauri_update_manifest.mjs $zip",
}

# The pre-Rust Python backend's own suites are the reference/legacy lane. They
# run on the `python` selection and in the Tests workflow, but they are never
# the arbiter of Rust product correctness and they must not run in the shipped
# `rust` selection, where they could only block or mask the real gates.
LEGACY_PYTHON_GATES = {
    "cpu-only old-backend regression": "python tools/run_cpu_only_tests.py",
    "full Python suite": "python -m unittest discover",
    "title-save research tests": "pytest -q tests\\test_title_save_observer.py",
}

# The crates the release actually ships must be gated on the release job
# itself, with the same commands the Tests workflow runs.
SHIPPED_CRATE_GATES = {
    "domain": "crates/nioh3-domain/Cargo.toml",
    "data": "crates/nioh3-data/Cargo.toml",
    "worker": "crates/nioh3-worker/Cargo.toml",
    "save": "crates/nioh3-save/Cargo.toml",
    "runtime": "crates/nioh3-runtime/Cargo.toml",
    "protected": "crates/nioh3-protected/Cargo.toml",
}


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


def step_is_active(step: dict, backend: str) -> bool:
    condition = step.get("if")
    if condition is None:
        return True
    if condition == RUST_ONLY:
        return backend == "rust"
    if condition == PYTHON_ONLY:
        return backend == "python"
    if condition in BACKEND_INDEPENDENT:
        return True
    raise AssertionError(f"unsupported step condition: {condition!r}")


def selected(steps: list[dict], backend: str) -> list[dict]:
    return [step for step in steps if step_is_active(step, backend)]


def joined(steps: list[dict]) -> str:
    """Every captured key of every step, so `env:`/`with:` count as content."""

    return "\n".join(
        f"{key}: {value}" for step in steps for key, value in step.items()
    )


def load_stager():
    spec = importlib.util.spec_from_file_location("stage_rust_workers", STAGER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class ReleaseOptInWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.steps = parse_steps(cls.text)

    def test_the_dispatch_input_defaults_to_the_shipped_backend(self) -> None:
        self.assertIn("worker_backend:", self.text)
        self.assertIn("default: rust", self.text)
        self.assertIn("type: choice", self.text)
        options = self.text.split("options:", 1)[1].split("\n", 4)
        self.assertIn("- python", "\n".join(options))
        self.assertIn("- rust", "\n".join(options))
        # Manual dispatch only: no schedule, push or pull_request trigger.
        on_block = self.text.split("permissions:", 1)[0]
        self.assertNotIn("push:", on_block)
        self.assertNotIn("pull_request:", on_block)
        self.assertNotIn("schedule:", on_block)

    def test_every_step_condition_is_one_the_gate_can_evaluate(self) -> None:
        for step in self.steps:
            condition = step.get("if")
            if condition is not None:
                self.assertIn(
                    condition,
                    (RUST_ONLY, PYTHON_ONLY, *BACKEND_INDEPENDENT),
                    step.get("name"),
                )

    def test_the_python_selection_is_the_shipped_step_set(self) -> None:
        python = joined(selected(self.steps, "python"))
        for name, marker in RETAINED_GATES.items():
            self.assertIn(marker, python, f"the python path lost {name}")
        # The shipped path never stages or builds the Rust workers.
        self.assertNotIn("stage_rust_workers.py", python)
        self.assertNotIn("crates/nioh3-protected/Cargo.toml", python)
        self.assertNotIn("crates/nioh3-worker/Cargo.toml", python)
        # The packaged Rust graph assertion is Rust-only.
        self.assertNotIn("OPTIN_MANIFEST_MISSING", python)

    def test_the_rust_selection_keeps_every_retired_python_gate(self) -> None:
        rust = joined(selected(self.steps, "rust"))
        for name, marker in RETAINED_GATES.items():
            self.assertIn(marker, rust, f"the rust path lost {name}")
        # The backend choice is passed through to the one build entry point.
        self.assertIn("-WorkerBackend ${{ inputs.worker_backend }}", rust)
        # The Rust branch validates the packaged resource graph itself.
        self.assertIn("OPTIN_MANIFEST_MISSING", rust)
        self.assertIn("OPTIN_PYINSTALLER_OUTPUT_PRESENT", rust)
        self.assertIn("OPTIN_PYTHON_RESOURCE_PRESENT", rust)
        # ...and then accepts the real product, not the graph alone: the host's
        # own resolution record, each packaged role's identity, and the shipped
        # frontend against the packaged graph.
        for gate in (
            "verify-host-package.mjs --package $portable --exe $exe",
            "verify-worker-identity.mjs --runtime $portable --role $role",
            "verify-packaged-frontend.mjs --package $portable --exe $exe",
            "foreach($role in @('offline_search','save','runtime'))",
            "--role $role",
        ):
            self.assertIn(gate, rust, f"the rust acceptance is missing {gate!r}")
        # The one-file gate asserts the staged Rust identity from inside the
        # single-file product.
        self.assertIn("NIOH3_WORKER_IDENTITY_OPT_IN", rust)
        self.assertIn("NIOH3_WORKER_IDENTITY_PROTECTED", rust)
        # The packaged-parity gate is backend-agnostic in the workflow: the
        # dispatch input is bound into its environment, and the gate itself
        # decides whether to derive the staged argv.
        self.assertIn("NIOH3_WORKER_BACKEND: ${{ inputs.worker_backend }}", rust)
        python = joined(selected(self.steps, "python"))
        self.assertIn("verify-onefile.mjs", python)
        self.assertIn("NIOH3_WORKER_BACKEND: ${{ inputs.worker_backend }}", python)

    def test_no_workflow_step_invokes_pyinstaller(self) -> None:
        # PyInstaller lives behind `build_tauri.ps1`'s python branch, so the
        # workflow never invokes it on either selection. The check is on
        # invocations, not on the word: the Rust branch legitimately *names* the
        # spec files it requires to be absent.
        for backend in ("python", "rust"):
            body = joined(selected(self.steps, backend)).lower()
            for invocation in (
                "-m pyinstaller",
                "pyinstaller --",
                "pyinstaller.exe",
                "pyinstaller.cmd",
            ):
                self.assertNotIn(
                    invocation, body, f"{backend} selection invokes {invocation!r}"
                )
        rust = joined(selected(self.steps, "rust")).lower()
        # Each spec name appears exactly once, as the exclusion the stager
        # declares - not as something to run.
        for spec in ("packaging/search-worker.spec", "packaging/protected-worker.spec"):
            self.assertEqual(
                rust.count(spec), 1, f"{spec} must appear only in the exclusion check"
            )

    def test_the_rust_selection_retires_the_legacy_lane_and_gates_the_shipped_crates(
        self,
    ) -> None:
        rust = joined(selected(self.steps, "rust"))
        python = joined(selected(self.steps, "python"))
        # The legacy lane is absent from the shipped selection rather than
        # skipped inside it, so it can neither block nor mask the crate gate.
        for name, marker in LEGACY_PYTHON_GATES.items():
            self.assertIn(marker, python, f"the python selection lost its {name}")
            self.assertNotIn(
                marker, rust, f"the shipped rust selection must not run the {name}"
            )
        for name, marker in SHIPPED_CRATE_GATES.items():
            self.assertIn(
                marker, rust, f"the rust selection does not gate the shipped {name} crate"
            )
        self.assertIn("Verify the shipped Rust crate suites", rust)
        # The release gate holds the crate *tests* only. Lint and formatting
        # live once, in the independent Tests `rust-crates` job, so the release
        # run does not pay for a duplicated `clippy`/`fmt` pass.
        self.assertNotIn("cargo clippy", rust)
        self.assertNotIn("cargo fmt", rust)

    def test_the_rust_graph_assertion_matches_the_stager_declarations(self) -> None:
        rust = joined(selected(self.steps, "rust"))
        stager = load_stager()
        # Every packaged binary name the stager produces must be checked, and
        # the exclusions it declares must be the ones the workflow requires.
        for _cargo, packaged, _crate in stager.ROLES:
            self.assertIn(
                packaged,
                rust,
                f"the workflow does not assert the packaged name {packaged}",
            )
        for spec in (
            "packaging/search-worker.spec",
            "packaging/protected-worker.spec",
        ):
            self.assertIn(spec, rust, f"the workflow does not require {spec} be excluded")
        self.assertIn("$manifest.backend -ne 'rust'", rust)

    def test_the_workflow_and_stager_agree_on_the_marker_files(self) -> None:
        rust = joined(selected(self.steps, "rust"))
        # The stager refuses to mix backends when these exist; the workflow
        # asserts the packaged output does not contain them.
        self.assertIn("python-build-environment.json", rust)
        self.assertIn("_internal", rust)


if __name__ == "__main__":
    unittest.main()
