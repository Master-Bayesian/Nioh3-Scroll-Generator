"""The Rust opt-in package must carry every dependency's provenance.

The packaged Rust graph ships the two worker binaries, and those crates are
separate workspaces with their own lockfiles, so their dependency graph is not
the host's. This gate re-derives the **actual locked** third-party packages for
the worker crates and proves the packaging generator's inventory covers them -
by construction, not because the host happens to pull the same crates today.

It also pins the worker build provenance (lockfile digests, compiler, target,
feature set) and the refusal of a Python runtime manifest beside a Rust graph.
Nothing here is a full package build: it runs the project Python, the real
`cargo metadata --locked` for four manifests and a scratch output on D:.
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in __import__("sys").path:
    __import__("sys").path.insert(0, str(ROOT))

from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

SCRATCH_ROOT = Path(
    os.environ.get(
        "NIOH3_DEPENDENCY_COVERAGE_ROOT",
        r"D:\Nioh3_v080_deliverables\m3-protected-host\dependency-coverage",
    )
)


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class WorkerDependencyCoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        # `cargo metadata` writes nothing to a target directory, but the gates
        # share one rule: never resolve a Cargo target inside the checkout.
        os.environ.setdefault("CARGO_TARGET_DIR", resolved_cargo_target_dir("dependency-coverage"))
        cls.packaging = load_tool("package_tauri")
        cls.stager = load_tool("stage_rust_workers")
        SCRATCH_ROOT.mkdir(parents=True, exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(dir=str(SCRATCH_ROOT))
        cls.output = Path(cls.temp.name) / "licenses-out"
        cls.output.mkdir(parents=True, exist_ok=True)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def test_the_worker_crates_are_enumerated_explicitly_for_the_rust_backend(self) -> None:
        # Anti-incidental-coverage check: the worker crates must be named by the
        # generator, not reached only because the host graph overlaps them.
        self.assertEqual(
            self.packaging.WORKER_CRATES,
            ("crates/nioh3-worker/Cargo.toml", "crates/nioh3-protected/Cargo.toml"),
        )
        for crate in self.packaging.WORKER_CRATES:
            self.assertNotIn(crate, self.packaging.HOST_CRATES)
            self.assertTrue((ROOT / crate).is_file(), crate)

    def test_every_locked_worker_dependency_receives_a_notice(self) -> None:
        worker = self.packaging.rust_dependency_packages(ROOT, self.packaging.WORKER_CRATES)
        combined = self.packaging.rust_dependency_packages(
            ROOT, self.packaging.HOST_CRATES + self.packaging.WORKER_CRATES
        )
        self.assertGreater(len(worker), 0, "the worker graph resolved to nothing")
        produced = self.packaging.collect_rust_licenses(ROOT, self.output, combined)
        covered = {(entry["name"], entry["version"]) for entry in produced}
        for package in worker.values():
            self.assertIn(
                (package["name"], package["version"]),
                covered,
                f"{package['name']}-{package['version']} is locked in the worker graph "
                "but has no notice in the generated inventory",
            )
        # Every produced entry must point at a real copied file, so a manifest
        # cannot list a notice it never staged.
        for entry in produced:
            self.assertTrue(entry["notices"], entry["name"])
            for notice in entry["notices"]:
                self.assertTrue((self.output / notice).is_file(), notice)

    def test_the_rust_inventory_is_a_superset_of_the_python_one(self) -> None:
        host = self.packaging.rust_dependency_packages(ROOT, self.packaging.HOST_CRATES)
        combined = self.packaging.rust_dependency_packages(
            ROOT, self.packaging.HOST_CRATES + self.packaging.WORKER_CRATES
        )
        self.assertTrue(set(host) <= set(combined))
        # The python backend keeps the host-only set, so its manifest cannot
        # change just because the worker crates exist; only the rust graph adds
        # the worker workspaces to the attributed set.
        self.assertEqual(
            self.packaging.rust_license_manifests("python"), self.packaging.HOST_CRATES
        )
        self.assertEqual(
            self.packaging.rust_license_manifests("rust"),
            self.packaging.HOST_CRATES + self.packaging.WORKER_CRATES,
        )

    def test_a_dependency_without_a_notice_fails_the_inventory(self) -> None:
        bare = Path(self.temp.name) / "crate-without-notice"
        bare.mkdir(parents=True, exist_ok=True)
        (bare / "Cargo.toml").write_text("[package]\nname = \"bare\"\n", encoding="utf-8")
        fake = {
            "bare 1.0.0": {
                "name": "bare",
                "version": "1.0.0",
                "license": "MIT",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path": str(bare / "Cargo.toml"),
            }
        }
        with self.assertRaises(ValueError) as raised:
            self.packaging.collect_rust_licenses(ROOT, self.output, fake)
        self.assertIn("Rust license missing", str(raised.exception))

    def test_the_notice_fetcher_covers_the_same_workspaces_as_the_packager(self) -> None:
        # A worker-only dependency that publishes no licence file has to be
        # supplementable, so the fetcher has to walk the same graphs the
        # packager attributes instead of the Tauri host workspace alone.
        notices = load_tool("collect_tauri_notices")
        self.assertEqual(
            tuple(notices.MANIFESTS), self.packaging.rust_license_manifests("rust")
        )
        for crate in self.packaging.WORKER_CRATES:
            self.assertIn(crate, notices.MANIFESTS)

    def test_the_worker_build_environment_records_lock_toolchain_and_features(self) -> None:
        environment = self.stager.build_environment(ROOT, "release")
        self.assertEqual(environment["target"], "x86_64-pc-windows-msvc")
        self.assertEqual(environment["profile"], "release")
        # Production features only; the development features are named as excluded.
        self.assertEqual(environment["features"], [])
        for excluded in ("test-fake", "test-helper"):
            self.assertIn(excluded, environment["excludedFeatures"])
        self.assertIsNotNone(environment["rustc"], "rustc --version must be recorded")
        self.assertIsNotNone(environment["cargo"], "cargo --version must be recorded")
        self.assertRegex(environment["sourceCommit"], r"^[0-9a-f]{40}$")
        # The recorded command line must be the one the product build script
        # actually runs, and it must not enable a development feature - so the
        # "production features only" claim is checkable, not a comment.
        self.assertEqual(len(environment["buildCommands"]), len(self.stager.ROLES))
        build_script = (ROOT / "tools" / "build_tauri.ps1").read_text(encoding="utf-8")
        for command in environment["buildCommands"]:
            self.assertNotIn("--features", command)
            self.assertNotIn("--all-features", command)
            for token in command[1:]:
                self.assertIn(f"'{token}'", build_script, token)
        # Both worker workspaces carry their own lockfile, and the recorded
        # digest must be the digest of the file on disk right now.
        import hashlib

        for crate in self.stager.WORKER_CRATES:
            key = f"crates/{crate}/Cargo.lock"
            self.assertIn(key, environment["locks"], key)
            self.assertEqual(
                environment["locks"][key],
                hashlib.sha256((ROOT / key).read_bytes()).hexdigest(),
                key,
            )

    def test_the_staged_manifest_carries_the_build_environment(self) -> None:
        binaries = Path(self.temp.name) / "binaries"
        (binaries / "release").mkdir(parents=True, exist_ok=True)
        for cargo_name, _packaged, _crate in self.stager.ROLES:
            (binaries / "release" / cargo_name).write_bytes(b"stub")
        workers = Path(self.temp.name) / "workers"
        manifest = self.stager.stage(binaries, workers, "release", ROOT)
        self.assertEqual(manifest["backend"], "rust")
        self.assertIn("buildEnvironment", manifest)
        self.assertEqual(manifest["buildEnvironment"]["features"], [])
        self.assertEqual(len(manifest["buildEnvironment"]["locks"]), 2)
        for spec in ("packaging/search-worker.spec", "packaging/protected-worker.spec"):
            self.assertIn(spec, manifest["excludedProductionResources"])
        # The Python build inputs may be *named* only as declared exclusions
        # (that is how the manifest records what the Rust graph does not carry);
        # nothing else in the manifest may reference them, and no PyInstaller
        # artefact or private fixture path may appear at all.
        self.assertIn(
            "python-build-environment.json", manifest["excludedProductionResources"]
        )
        body = {key: value for key, value in manifest.items() if key != "excludedProductionResources"}
        text = json.dumps(body)
        for forbidden in (
            "PyInstaller",
            "search-worker.spec",
            "protected-worker.spec",
            "python-build-environment",
            "launch_search_worker.py",
            "launch_protected_worker.py",
            "research/",
            ".codex_tmp",
        ):
            self.assertNotIn(
                forbidden,
                text,
                f"the worker manifest must not reference {forbidden}",
            )
        # And the exclusion list itself must stay exactly the known Python-only
        # build inputs, so it cannot be used as a hiding place either.
        self.assertEqual(
            sorted(manifest["excludedProductionResources"]),
            sorted(
                (
                    "packaging/search-worker.spec",
                    "packaging/protected-worker.spec",
                    "launch_search_worker.py",
                    "launch_protected_worker.py",
                    "python-build-environment.json",
                )
            ),
        )

    def test_a_python_runtime_manifest_is_refused_beside_a_rust_graph(self) -> None:
        workers = Path(self.temp.name) / "mixed-workers"
        workers.mkdir(parents=True, exist_ok=True)
        (workers / "worker-backend.json").write_text(
            json.dumps({"schema": "nioh3-worker-backend/v1", "backend": "rust"}),
            encoding="utf-8",
        )
        (workers / "python-build-environment.json").write_text("{}", encoding="utf-8")
        with self.assertRaises(ValueError) as raised:
            self.packaging.worker_environment(workers, "rust")
        self.assertIn("PyInstaller", str(raised.exception))

    def test_the_rust_manifest_declares_no_python_build_environment(self) -> None:
        python_environment = {"pyinstaller": "6.0", "python": "3.12"}
        rust_manifest = self.packaging.dependency_manifest(
            "rust",
            [{"name": "js", "notices": ["licenses/js/LICENSE"]}],
            [{"name": "serde", "version": "1.0.0", "notices": ["licenses/rust/serde-1.0.0/LICENSE"]}],
            worker_manifest={"target": "x86_64-pc-windows-msvc"},
            worker_layout={"dataRoot": "worker/runtime/nioh3_scroll_editor/data"},
            python_environment=python_environment,
        )
        # A Rust graph records its own build environment and explicitly states
        # that no Python build environment exists - it must not inherit one.
        self.assertEqual(rust_manifest["workerBuildEnvironment"]["target"], "x86_64-pc-windows-msvc")
        self.assertIsNone(rust_manifest["pythonBuildEnvironment"])
        self.assertNotIn(python_environment, rust_manifest.values())
        # The shipped branch still records the Python environment verbatim.
        python_manifest = self.packaging.dependency_manifest(
            "python", [], [], python_environment=python_environment
        )
        self.assertEqual(python_manifest["pythonBuildEnvironment"], python_environment)
        self.assertNotIn("workerBuildEnvironment", python_manifest)


if __name__ == "__main__":
    unittest.main()
