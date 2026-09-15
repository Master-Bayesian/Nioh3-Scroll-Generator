"""Deterministic tests for the packaged Rust graph and the retained Python graph.

No full package build, no PyInstaller run and no real save or game process:
these tests drive the staging helper and the packager's layout functions over
small fixtures in the platform temp directory, and they assert both directions
of the contract — the packaged Rust graph ships the Rust workers without Python
resources, and the development/parity Python graph still assembles.
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "tools"


def load_tool(name: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def fixture_root() -> Path:
    """Big-fixture-free temp root, preferring the D: workspace when present."""

    preferred = Path("D:/Nioh3_v080_deliverables/tmp")
    try:
        preferred.mkdir(parents=True, exist_ok=True)
    except OSError:
        return Path(tempfile.mkdtemp(prefix="nioh3-packaging-"))
    return Path(tempfile.mkdtemp(prefix="nioh3-packaging-", dir=preferred))


class PackagingOptInTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = fixture_root()
        self.addCleanup(lambda: None)
        self.root = self.temp / "root"
        self.workers = self.temp / "workers"
        self.output = self.temp / "portable"
        self.binaries = self.temp / "cargo" / "release"
        self.binaries.mkdir(parents=True)
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "package.json").write_text(
            json.dumps(
                {
                    "name": "fixture",
                    "version": "0.0.0-test",
                    "dependencies": {},
                }
            ),
            encoding="utf-8",
        )
        (self.root / "bin").mkdir()
        for name in (
            "nioh3_seed_accelerator.dll",
            "nioh3_effect_preimage_accelerator.dll",
            "nioh3_seed_accelerator.build.json",
        ):
            (self.root / "bin" / name).write_bytes(("fixture:" + name).encode())
        (self.root / "nioh3_scroll_editor/data").mkdir(parents=True)
        (self.root / "nioh3_scroll_editor/data/tables.bin").write_bytes(b"tables")
        (self.root / "packages/contracts").mkdir(parents=True)
        (self.root / "packages/contracts/request.schema.json").write_text("{}", encoding="utf-8")
        (self.root / "packages/contracts/response.schema.json").write_text("{}", encoding="utf-8")
        (self.root / "third_party/nioh_savefile_decrypt").mkdir(parents=True)
        (self.root / "third_party/nioh_savefile_decrypt/LICENSE").write_text("MIT", encoding="utf-8")
        # The packager walks package.json dependencies and always checks the
        # Tauri API package, so the fixture supplies a licence-bearing stub.
        api = self.root / "node_modules/@tauri-apps/api"
        api.mkdir(parents=True)
        (api / "package.json").write_text(
            json.dumps({"name": "@tauri-apps/api", "version": "0.0.0", "license": "MIT"}),
            encoding="utf-8",
        )
        (api / "LICENSE").write_text("MIT", encoding="utf-8")
        # The packager resolves the host and launcher EXEs through the shared
        # Cargo target rule, so the fixture pins CARGO_TARGET_DIR to its own
        # isolated tree and lays the EXEs out exactly as cargo would.
        previous = os.environ.get("CARGO_TARGET_DIR")
        os.environ["CARGO_TARGET_DIR"] = str(self.temp / "cargo-target")

        def restore() -> None:
            if previous is None:
                os.environ.pop("CARGO_TARGET_DIR", None)
            else:
                os.environ["CARGO_TARGET_DIR"] = previous

        self.addCleanup(restore)
        self.stage = load_tool("stage_rust_workers")
        self.packager = load_tool("package_tauri")
        for workspace, name, body in (
            ("apps/tauri/src-tauri", "nioh3-studio.exe", b"broker"),
            ("apps/launcher", "Nioh3Launcher.exe", b"launcher"),
        ):
            target = self.packager.cargo_target_dir(self.root, workspace)
            target.mkdir(parents=True, exist_ok=True)
            (target / name).write_bytes(body)

    def stage_rust_workers(self) -> dict:
        (self.binaries / "nioh3-readonly-worker.exe").write_bytes(b"rust-readonly")
        (self.binaries / "nioh3-protected-worker.exe").write_bytes(b"rust-protected")
        return self.stage.stage(self.binaries.parent, self.workers, "release")

    def assemble(self, backend: str) -> dict:
        if backend == "rust":
            manifest = self.stage_rust_workers()
            _, _ = self.packager.worker_environment(self.workers, "rust")
            layout = self.packager.assemble_layout(self.root, self.output, self.workers, "rust")
            return {"manifest": manifest, "layout": layout}
        self.workers.mkdir(parents=True, exist_ok=True)
        (self.workers / "nioh3-search-worker.exe").write_bytes(b"py-search")
        (self.workers / "nioh3-protected-worker.exe").write_bytes(b"py-protected")
        (self.workers / "python-build-environment.json").write_text(
            json.dumps({"python": "3.12", "packages": []}), encoding="utf-8"
        )
        (self.workers / "licenses/python").mkdir(parents=True, exist_ok=True)
        self.packager.worker_environment(self.workers, "python")
        layout = self.packager.assemble_layout(self.root, self.output, self.workers, "python")
        return {"layout": layout}

    def test_rust_opt_in_ships_both_workers_and_the_resolved_runtime_assets(self) -> None:
        result = self.assemble("rust")
        manifest = result["manifest"]
        self.assertEqual(manifest["backend"], "rust")
        self.assertEqual(
            [entry["packagedName"] for entry in manifest["binaries"]],
            ["nioh3-search-worker.exe", "nioh3-protected-worker.exe"],
        )
        self.assertEqual(manifest["defaultGraph"], "rust")
        self.assertEqual(manifest["pythonGraph"], "development-parity-and-legacy-tk-only")
        for entry in manifest["binaries"]:
            self.assertEqual(len(entry["sha256"]), 64)
            self.assertTrue((self.output / "worker" / entry["packagedName"]).is_file())
        for name in (
            "nioh3_seed_accelerator.dll",
            "nioh3_effect_preimage_accelerator.dll",
            "nioh3_seed_accelerator.build.json",
        ):
            self.assertTrue((self.output / "worker/runtime/bin" / name).is_file())
        self.assertTrue(
            (
                self.output
                / "worker/runtime/nioh3_scroll_editor/data/tables.bin"
            ).is_file(),
            "the staged data root is what makes the worker find bin/ next to it",
        )
        self.assertEqual(
            result["layout"]["dataRoot"],
            "worker/runtime/nioh3_scroll_editor/data",
        )
        self.assertEqual(result["layout"]["contractDir"], "packages/contracts")

    def test_rust_opt_in_leaves_no_python_or_pyinstaller_resource(self) -> None:
        self.assemble("rust")
        worker_files = sorted(
            path.name for path in (self.output / "worker").rglob("*") if path.is_file()
        )
        self.assertNotIn("python-build-environment.json", worker_files)
        self.assertFalse((self.output / "worker/_internal").exists())
        self.assertTrue((self.output / "licenses/NO-PYTHON-RUNTIME.txt").is_file())
        self.assertFalse(
            any("Nioh_Savefile_decrypt" in name for name in worker_files),
            "the decryptor is not part of the Rust runtime (oracle-only decision)",
        )
        manifest = json.loads(
            (self.workers / "worker-backend.json").read_text(encoding="utf-8")
        )
        self.assertFalse(manifest["decryptor"]["shippedInRustGraph"])
        self.assertEqual(manifest["decryptor"]["decision"], "oracle_only")

    def test_rust_opt_in_refuses_pyinstaller_leftovers_and_missing_binaries(self) -> None:
        self.stage_rust_workers()
        (self.workers / "python-build-environment.json").write_text("{}", encoding="utf-8")
        with self.assertRaises(SystemExit):
            self.stage.stage(self.binaries.parent, self.workers, "release")
        (self.workers / "python-build-environment.json").unlink()
        # A staged directory without the explicit backend manifest must refuse.
        staged_manifest = self.workers / "worker-backend.json"
        preserved = staged_manifest.read_bytes()
        staged_manifest.unlink()
        with self.assertRaises(FileNotFoundError):
            self.packager.worker_environment(self.workers, "rust")
        staged_manifest.write_bytes(preserved)
        # A missing cargo binary must fail the staging step by name.
        (self.binaries / "nioh3-protected-worker.exe").unlink()
        with self.assertRaises(SystemExit):
            self.stage.stage(self.binaries.parent, self.workers, "release")

    def test_the_retained_python_graph_still_assembles(self) -> None:
        self.assemble("python")
        self.assertTrue((self.output / "worker/nioh3-search-worker.exe").is_file())
        self.assertFalse((self.output / "worker/runtime").exists())
        self.assertFalse((self.output / "worker/worker-backend.json").exists())
        environment, python_environment = self.packager.worker_environment(
            self.workers, "python"
        )
        self.assertEqual(environment, {})
        self.assertEqual(python_environment["python"], "3.12")

    def test_release_binaries_resolve_through_the_shared_cargo_target_rule(self) -> None:
        """A candidate build shares one Cargo target dir across every workspace."""

        configured = Path(os.environ["CARGO_TARGET_DIR"])
        self.assertEqual(
            self.packager.cargo_target_dir(self.root, "apps/tauri/src-tauri"),
            configured / "release",
        )
        self.assertEqual(
            self.packager.cargo_target_dir(self.root, "apps/launcher"),
            configured / "release",
        )
        previous = os.environ.pop("CARGO_TARGET_DIR")
        try:
            self.assertEqual(
                self.packager.cargo_target_dir(self.root, "apps/launcher"),
                self.root / "apps/launcher/target/release",
            )
        finally:
            os.environ["CARGO_TARGET_DIR"] = previous


if __name__ == "__main__":
    unittest.main()
