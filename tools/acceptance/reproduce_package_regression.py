"""Reproduce old packaging failure and new gate using the immutable handoff.

Runs the original archived staging/wrapper implementation over exact delivered
payload bytes and isolated PE fixtures. No installed game or private save is used.
"""
from __future__ import annotations
import argparse
import io
import json
import platform
from pathlib import Path
import struct
import sys
import tempfile
import types
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools import package_tauri as patched_stage
from tools import build_tauri_onefile as patched_wrapper
from tools import verify_loadout_release as gate
from tools.tests.test_loadout_release import bound_scene, write_files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("handoff", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.handoff.resolve()
    source = root / "project-source/app-fbbac77.zip"
    outer = root / "app/Nioh3Studio-0.8.13-win-x64.exe"
    raw = outer.read_bytes()
    magic, length, signature = struct.unpack("<16sQ32s", raw[-56:])
    gate.need(magic == b"NIOH3_ONEFILE_V1", "not a one-file EXE")
    payload = raw[-56-length:-56]
    gate.need(gate.digest(payload) == signature.hex(), "invalid payload hash")
    original = {}
    with zipfile.ZipFile(source) as archive:
        commit = archive.comment.decode()
        for name in ("package_tauri", "build_tauri_onefile"):
            module = types.ModuleType("original_" + name)
            module.__file__ = str(Path(__file__).resolve().parents[1] / (name + ".py"))
            exec(compile(archive.read("tools/" + name + ".py"), module.__file__, "exec"), module.__dict__)
            original[name] = module
    report = {"scope": "actual original payload plus isolated fixture staging, no Windows numerical execution",
              "environment": platform.platform(), "source_commit": commit,
              "source_archive_sha256": gate.digest(source.read_bytes()), "outer_sha256": gate.digest(raw), "payload_sha256": gate.digest(payload)}
    with tempfile.TemporaryDirectory(prefix="nioh3-package-regression-") as directory:
        temp = Path(directory)
        zip_path = temp / "actual-embedded.zip"; zip_path.write_bytes(payload)
        for label, verifier in (("before", original["build_tauri_onefile"].verify_archive), ("after", patched_wrapper.verify_archive)):
            try:
                manifest, _ = verifier(zip_path)
                report[label] = {"accepted": True, "manifest_files": len(manifest["files"])}
            except ValueError as error:
                report[label] = {"accepted": False, "error": str(error)}
        fixtures = temp / "scene"
        write_files(fixtures, bound_scene())
        project = temp / "source"
        write_files(project / "bin", {name: b"fixture" for name in ("nioh3_seed_accelerator.dll", "nioh3_effect_preimage_accelerator.dll", "nioh3_seed_accelerator.build.json", "nioh3_effect_preimage_accelerator.build.json")})
        write_files(project / "data/loadout/kusarigama-strong", {"bundle.json": bound_scene()["bundle.json"]})
        report["staging"] = {}
        for label, stage in (("before", original["package_tauri"].stage_rust_runtime), ("after", patched_stage.stage_rust_runtime)):
            destination = temp / label
            stage(project, destination, fixtures)
            deployed = destination / gate.SCENE_ROOT / "kusarigama-strong"
            report["staging"][label] = {"device_copied": (deployed / "main_device.dll").is_file(), "contract_copied": (deployed / gate.CONTRACT).is_file()}
        with zipfile.ZipFile(io.BytesIO(payload)) as archive:
            prefix = gate.SCENE_ROOT + "kusarigama-strong/"
            files = {name[len(prefix):]: archive.read(name) for name in archive.namelist() if name.startswith(prefix)}
        layout = gate.native_layout((root / "native/project-source/prepared-main/main_gen.inc").read_bytes())
        try:
            gate.check_scene_layout(files, layout)
            report["dll_only_swap_layout"] = {"accepted": True}
        except ValueError as error:
            report["dll_only_swap_layout"] = {"accepted": False, "error": str(error)}
    report["passed"] = (report["before"]["accepted"] is True and report["after"]["accepted"] is False
                        and report["staging"]["before"]["device_copied"] is False and report["staging"]["after"]["device_copied"] is True
                        and report["staging"]["after"]["contract_copied"] is True and report["dll_only_swap_layout"]["accepted"] is False)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
