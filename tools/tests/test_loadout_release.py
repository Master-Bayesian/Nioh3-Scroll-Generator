"""Counterexamples for the actual staging/wrapping/final-payload release path.

PE fixtures exercise real PE export parsing; they contain no executable scoring
code and do not certify numerical compatibility. Windows execution has a separate
host runner and cannot be replaced by these tests.
"""
from __future__ import annotations

import copy
import hashlib
import io
import json
import os
import subprocess
import sys
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

from tools import verify_loadout_release as gate
from tools import package_tauri
from tools import build_tauri_onefile

COMMIT = "a" * 40
NATIVE = "b" * 40


def pe_fixture(exports: set[str]) -> bytes:
    raw = bytearray(0x2200)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", raw, 0x84, 0x8664, 1, 0, 0, 0, 240, 0x2022)
    optional = 0x98
    struct.pack_into("<H", raw, optional, 0x20b)
    struct.pack_into("<Q", raw, optional + 24, 0x180000000)
    struct.pack_into("<II", raw, optional + 32, 0x1000, 0x200)
    struct.pack_into("<II", raw, optional + 56, 0x4000, 0x200)
    struct.pack_into("<I", raw, optional + 108, 16)
    struct.pack_into("<II", raw, optional + 112, 0x1000, 0x1000)
    section = optional + 240
    struct.pack_into("<8sIIIIIIHHI", raw, section, b".rdata\0\0", 0x2000, 0x1000, 0x2000, 0x200, 0, 0, 0, 0, 0x40000040)
    symbols = sorted(exports)
    struct.pack_into("<IIHHIIIIIII", raw, 0x200, 0, 0, 0, 0, 0x1040, 1, len(symbols), len(symbols), 0x1080, 0x1100, 0x1180)
    raw[0x240:0x24c] = b"fixture.dll\0"
    cursor = 0x400
    for i, name in enumerate(symbols):
        struct.pack_into("<I", raw, 0x280 + i * 4, 0x1f00)
        struct.pack_into("<I", raw, 0x300 + i * 4, cursor + 0xe00)
        struct.pack_into("<H", raw, 0x380 + i * 2, i)
        value = name.encode() + b"\0"
        raw[cursor:cursor + len(value)] = value
        cursor += len(value)
    return bytes(raw)


def scene_files() -> dict[str, bytes]:
    def section(name, data):
        return struct.pack("<I", len(name)) + name.encode() + struct.pack("<I", len(data)) + data
    frame = b"".join(section(name, value) for name, value in {
        "stat": bytes(0x900), "d0": bytes(0xb00), "s_base": bytes(0x900), "d_base": bytes(0xb00),
        "addresses": struct.pack("<9Q", 0x10000, 0x20000, 0x30000, 0x10000, 1, 2, 3, 0, 0),
    }.items())
    files = {name: b"fixture" for name in gate.REQUIRED}
    files["bundle.json"] = json.dumps({"id": "kusarigama-strong"}).encode()
    files["frame.bin"] = frame
    files["hits.txt"] = b"1\n30298\n0\n"
    files["event_plan.json"] = json.dumps({"main_address": 0x30000, "hit_values_sha256": gate.digest(files["hits.txt"])}).encode()
    for name, symbols in gate.EXPORTS.items():
        files[name] = pe_fixture(symbols)
    return files


def bound_scene() -> dict[str, bytes]:
    files = scene_files()
    contract = {"schema": gate.SCHEMA, "scene_id": "kusarigama-strong", "app_source_commit": COMMIT, "native_source_commit": NATIVE,
                "files": gate.inventory(files), "abi": gate.ABIS, "kernel_layout": {"MAIN_MODULE": 0x180000000, "MAIN_ENTRY_RSP": 0x40000, "MAIN_SCORE": 0x30218,
                "domain": [[0x10000, 0x10900], [0x20000, 0x20b00], [0x30000, 0x30300]], "generated_main_sha256": "c"*64}, "compatibility_status": "host_e2e_required"}
    files[gate.CONTRACT] = json.dumps(contract).encode()
    return files


def write_files(path: Path, files: dict[str, bytes]):
    for name, data in files.items():
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)


def portable(path: Path, scenes: dict[str, bytes]):
    files = {name: pe_fixture(set()) if name.endswith(".exe") else b"{}" for name in build_tauri_onefile.REQUIRED_PATHS}
    files.update({gate.SCENE_ROOT + "kusarigama-strong/" + name: body for name, body in scenes.items()})
    manifest = {"schema": "nioh3-tauri-manifest/v1", "version": "0.8.13", "git": {"commit": COMMIT, "dirty": False},
                "files": [{"path": name, "size": len(body), "sha256": gate.digest(body)} for name, body in files.items()]}
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, body in files.items(): archive.writestr(name, body)
        archive.writestr("build-manifest.json", json.dumps(manifest))


class LoadoutReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="loadout-package-gate-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_layout_export_keeps_addresses_and_removes_character_evidence(self):
        files = scene_files()
        original = gate.sections(files["frame.bin"])
        original["addresses"] = original["addresses"][:56] + struct.pack("<QQ",2335,2876)
        original["stat"] = b"P" * 0x900
        original["source_rebuild"] = b'{"reference_inventory":[12,34]}'
        files["frame.bin"] = b"".join(
            struct.pack("<I", len(name)) + name.encode() + struct.pack("<I", len(value)) + value
            for name, value in original.items())
        files["event_plan.json"] = json.dumps({
            "mode": {"kind": "fixed_contacts"}, "templates": [{"packet": {"kind": "prepared_actor"}}],
            "single_use": None}).encode()
        files["gpu_setup.txt"] = b"348 340 0\n0\n5 45 15 200 5 60 155\n"
        write_files(self.root / "captured", files)
        gate.export_layout(self.root / "captured", self.root / "export")
        public = gate.sections((self.root / "export/frame.bin").read_bytes())
        self.assertEqual(public["addresses"][:56], original["addresses"][:56])
        self.assertEqual(public["addresses"][56:], bytes(16))
        self.assertEqual(public["stat"], bytes(0x900))
        self.assertNotIn("source_rebuild", public)
        self.assertEqual((self.root / "captured/frame.bin").read_bytes(), files["frame.bin"])
        self.assertNotIn(b"348", (self.root / "export/gpu_setup.txt").read_bytes())

    def test_static_identity_is_not_numerical_acceptance(self):
        report = gate.verify_scene(bound_scene(), "kusarigama-strong", COMMIT)
        self.assertTrue(report["host_e2e_required"])
        self.assertIn("nk_main_device_open", report["exports"]["main_device.dll"])

    def test_static_cli_rejects_a_package_omitting_the_entire_optimizer(self):
        package = self.root / "empty-scene.zip"
        portable(package, {})
        result = subprocess.run([sys.executable, gate.__file__, "check", str(package)],
                                capture_output=True, text=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("at least one shipped scene", json.loads(result.stdout)["error"])

    def test_old_cpu_only_package_is_rejected_before_wrapping(self):
        files = bound_scene(); del files["main_device.dll"]
        target = self.root / "candidate.zip"; portable(target, files)
        with self.assertRaisesRegex(ValueError, "missing runtime files: main_device.dll"):
            build_tauri_onefile.build_onefile(target, self.root / "candidate.exe")
        self.assertFalse((self.root / "candidate.exe").exists())

    def test_actual_stage_copies_device_and_contract(self):
        source = self.root / "arbitrary-export-name"
        write_files(source, bound_scene())
        root = self.root / "source"
        write_files(root / "bin", {name: b"helper" for name in ("nioh3_seed_accelerator.dll", "nioh3_effect_preimage_accelerator.dll", "nioh3_seed_accelerator.build.json", "nioh3_effect_preimage_accelerator.build.json")})
        write_files(root / "data/loadout/kusarigama-strong", {"bundle.json": bound_scene()["bundle.json"]})
        result = package_tauri.stage_rust_runtime(root, self.root / "output", source)
        for name in ("main_cpu.dll", "main_device.dll", gate.CONTRACT):
            destination = gate.SCENE_ROOT + "kusarigama-strong/" + name
            self.assertIn(destination, result["staged"])
            self.assertEqual((self.root / "output" / destination).read_bytes(), bound_scene()[name])

    def test_full_final_exe_is_reopened_after_build(self):
        archive = self.root / "candidate.zip"; portable(archive, bound_scene())
        exe = self.root / "candidate.exe"
        result = build_tauri_onefile.build_onefile(archive, exe)
        self.assertEqual(result["loadout"]["artifact_sha256"], gate.digest(exe.read_bytes()))
        self.assertTrue(gate.artifact_report(exe)["scenes"][0]["host_e2e_required"])

    def test_shared_wrong_scene_still_fails_if_both_dll_hashes_are_correct(self):
        files = bound_scene()
        frame = files["frame.bin"]
        files["frame.bin"] = frame.replace(struct.pack("<Q", 0x30000), struct.pack("<Q", 0x90000))
        # Deliberately re-sign the scene inventory: identity is internally
        # consistent, but compiled MAIN and scene address spaces still differ.
        contract = json.loads(files[gate.CONTRACT]); contract["files"] = gate.inventory(files)
        files[gate.CONTRACT] = json.dumps(contract).encode()
        with self.assertRaisesRegex(ValueError, "MAIN address differs"):
            gate.verify_scene(files, "kusarigama-strong", COMMIT)

    def test_missing_or_changed_any_scene_file_is_rejected(self):
        original = bound_scene()
        for name in sorted(gate.REQUIRED):
            with self.subTest(file=name):
                files = original.copy(); files[name] += b"changed"
                with self.assertRaisesRegex(ValueError, "differ from bound"):
                    gate.verify_scene(files, "kusarigama-strong", COMMIT)

    def test_private_context_is_never_copied_into_public_runtime(self):
        files = bound_scene()
        files["context/image.bin"] = b"private captured memory"
        contract = json.loads(files[gate.CONTRACT]); contract["files"] = gate.inventory(files)
        files[gate.CONTRACT] = json.dumps(contract).encode()
        with self.assertRaisesRegex(ValueError, "non-deployable or stale files"):
            gate.verify_scene(files, "kusarigama-strong", COMMIT)

    def test_missing_component_export_and_cross_commit_rejected(self):
        files = bound_scene(); files["main_device.dll"] = pe_fixture({"nk_context_abi"})
        contract = json.loads(files[gate.CONTRACT]); contract["files"] = gate.inventory(files); files[gate.CONTRACT] = json.dumps(contract).encode()
        with self.assertRaisesRegex(ValueError, "missing required ABI exports"):
            gate.verify_scene(files, "kusarigama-strong", COMMIT)
        with self.assertRaisesRegex(ValueError, "another app source commit"):
            gate.verify_scene(bound_scene(), "kusarigama-strong", "d"*40)

    def test_event_plan_hash_and_domains_are_not_only_file_hash_checks(self):
        for mutation, message in ((lambda f: f.update({"hits.txt": b"1\n99999\n0\n"}), "hit address outside"),
                                  (lambda f: f.update({"event_plan.json": b'{"main_address":196608,"hit_values_sha256":"bad"}'}), "hit values are stale")):
            files = bound_scene(); mutation(files)
            contract = json.loads(files[gate.CONTRACT]); contract["files"] = gate.inventory(files); files[gate.CONTRACT] = json.dumps(contract).encode()
            with self.assertRaisesRegex(ValueError, message): gate.verify_scene(files, "kusarigama-strong", COMMIT)

    def test_external_host_receipt_cannot_be_reused_for_another_exe(self):
        artifact = {"artifact_kind": "onefile", "artifact_sha256": "a"*64, "source_commit": COMMIT}
        receipt = {"schema": gate.HOST_SCHEMA, "passed": True, "outer_sha256": "b"*64, "source_commit": COMMIT,
                   "mode": "player", "release_ready": True, "private_resources_copied": False,
                   "fresh_install": {"passed": True, "source": "live", "searches_complete": True, "private_resources_copied": False, "provisioned_by": "product"}}
        with self.assertRaisesRegex(ValueError, "another EXE"):
            gate.verify_host_receipt(artifact, receipt)


    def test_host_release_requires_real_execution_fresh_install_and_nonempty_scene(self):
        # Synthetic JSON exercises the receipt validator only, NOT Windows.
        scene = gate.verify_scene(bound_scene(), "kusarigama-strong", COMMIT)
        artifact = {"artifact_kind": "onefile", "artifact_sha256": "a"*64, "source_commit": COMMIT, "scenes": [scene]}
        runs = []
        for backend in ("cpu", "card"):
            name = "main_cpu.dll" if backend == "cpu" else "main_device.dll"
            execution = {"schema": "nioh3-loadout-execution/v1", "scope": "prepared_exact_sweep", "backend": backend,
                         "planned": 40, "completed": 40, "native_calls": 1 if backend == "card" else 0,
                         "submitted_contacts": 40, "completed_contacts": 40, "failed_batches": 0, "reuse_enabled": False,
                         "cpu_executed_candidates": 2, "cpu_certified_candidates": 38}
            runs.append({"requested_backend": backend, "actual_backend": backend, "complete": True, "planned": 40,
                         "searched": 40, "answered": 40, "no_path": 0, "refused": 0, "source_uncomputed": 0,
                         "loaded_library": {"sha256": scene["files"][name]["sha256"], "scene_files_match_payload": True, "worker_from_payload_cache": True},
                         "observed_abis": {"nk_context_abi": 2, "nk_main_" + ("cpu" if backend == "cpu" else "device") + "_components_abi": 1},
                         "execution_receipt": execution, "winner_components_matched": True, "winner_component_bits": [1,2,3],
                         "rank_rows_matched": 20, "reference_sha256": "c"*64, "rank_sha256": "d"*64})
        receipt = {"schema": gate.HOST_SCHEMA, "passed": True, "outer_sha256": "a"*64, "source_commit": COMMIT,
                   "mode": "player", "release_ready": True, "private_resources_copied": False,
                   "fresh_install": {"passed": True, "source": "live", "searches_complete": True, "private_resources_copied": False, "provisioned_by": "product"},
                   "environment": {"platform": "win32"}, "runtime_overrides": [],
                   "scenes": [{"scene_id": "kusarigama-strong", "files": scene["files"], "runs": runs}],
                   "no_private_profile": {"ran": True, "false_complete": False}}
        gate.verify_host_receipt(artifact, receipt)
        captured = copy.deepcopy(artifact)
        captured["scenes"][0]["layout"]["frame_sections"].append("source_rebuild")
        with self.assertRaisesRegex(ValueError, "character-bound source program"):
            gate.verify_host_receipt(captured, receipt)
        with_seeds=copy.deepcopy(artifact)
        with_seeds["scenes"][0]["layout"]["empty_actor_buffers_and_seeds"]=False
        with self.assertRaisesRegex(ValueError,"character stat, damage or seed"):
            gate.verify_host_receipt(with_seeds,receipt)
        mutations = [
            ("engineering", lambda r: r.update(mode="engineering-fixture", private_resources_copied=True)),
            ("no-fresh-install", lambda r: r["fresh_install"].update(passed=False)),
            ("fallback", lambda r: r["scenes"][0]["runs"][1].update(actual_backend="cpu")),
            ("wrong-abi", lambda r: r["scenes"][0]["runs"][1]["observed_abis"].update(nk_context_abi=1)),
            ("loaded-but-unused", lambda r: r["scenes"][0]["runs"][1]["execution_receipt"].update(native_calls=0)),
            ("partial-submit", lambda r: r["scenes"][0]["runs"][1]["execution_receipt"].update(submitted_contacts=39)),
            ("refused", lambda r: r["scenes"][0]["runs"][0].update(refused=1)),
            ("stale-worker", lambda r: r["scenes"][0]["runs"][0]["loaded_library"].update(worker_from_payload_cache=False)),
            ("missing-components", lambda r: r["scenes"][0]["runs"][0].update(winner_components_matched=False)),
            ("boolean-counter", lambda r: r["scenes"][0]["runs"][0].update(answered=True)),
        ]
        for label, mutate in mutations:
            with self.subTest(control=label):
                broken = copy.deepcopy(receipt); mutate(broken)
                with self.assertRaises(ValueError): gate.verify_host_receipt(artifact, broken)
        empty = {**artifact, "scenes": []}; no_scene = {**receipt, "scenes": []}
        with self.assertRaisesRegex(ValueError, "at least one shipped scene"):
            gate.verify_host_receipt(empty, no_scene)


if __name__ == "__main__":
    unittest.main()
