"""Run actual seed-search backends and retain exact, repeatable parity evidence.

Use tools/run_python_tests.ps1 -ScriptPath tools/verify_gpu_search_e2e.py.
The CPU oracle is the existing native implementation with its explicit opt-in.
GPU runs keep bulk CPU disabled; the auto-fallback run forces CUDA discovery to
fail while a real DirectCompute adapter remains available. No game is attached.
"""
from __future__ import annotations
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "tests"))
from cargo_target import cargo_target_dir, build_root


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def adapter(vendor: int) -> dict:
    library = ctypes.WinDLL(str(ROOT / "bin" / "nioh3_effect_preimage_accelerator.dll"))
    info = library.d3d11_effect_adapter_info
    info.argtypes = [ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint64), ctypes.POINTER(ctypes.c_uint64), ctypes.c_wchar_p, ctypes.c_uint32]
    info.restype = ctypes.c_int
    actual, device = ctypes.c_uint32(), ctypes.c_uint32()
    dedicated, shared = ctypes.c_uint64(), ctypes.c_uint64()
    description = ctypes.create_unicode_buffer(256)
    if not info(vendor, ctypes.byref(actual), ctypes.byref(device), ctypes.byref(dedicated), ctypes.byref(shared), description, 256):
        raise RuntimeError(f"No D3D11 hardware adapter for vendor 0x{vendor:04X}")
    probe = library.d3d11_seed_search_available
    probe.argtypes = [ctypes.c_uint32]
    probe.restype = ctypes.c_int
    if probe(vendor) != 1:
        raise RuntimeError(f"Seed compute self-test failed on {description.value}")
    return {"vendor_id": actual.value, "device_id": device.value, "description": description.value, "dedicated_bytes": dedicated.value, "shared_bytes": shared.value}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=build_root() / "deliverables" / "directcompute-seed-20261115" / "gpu-parity")
    parser.add_argument("--vendors", default="4318,4098")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    target = cargo_target_dir("tauri-target")
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--locked", "--release", "--manifest-path", str(ROOT / "crates/nioh3-worker/Cargo.toml"), "--example", "gpu_search_acceptance"], cwd=ROOT, env=environment, check=True)
    executable = target / "release" / "examples" / "gpu_search_acceptance.exe"
    vendors = [int(value, 0) for value in args.vendors.split(",")]
    adapters = [adapter(vendor) for vendor in vendors]
    cases = [("cpu", 0), ("cuda", 0)]
    cases += [("directcompute", vendor) for vendor in vendors]
    cases += [("auto-fallback", vendor) for vendor in vendors]
    reports, failures = [], []
    for mode, vendor in cases:
        name = f"{mode}-{vendor:04x}"
        path = args.output / f"{name}.json"
        child_environment = environment.copy()
        child_environment["NIOH3_D3D11_VENDOR"] = str(vendor)
        print(f"GPU_ACCEPTANCE_START {name}", flush=True)
        started = time.monotonic()
        completed = subprocess.run([str(executable), mode, str(path)], cwd=ROOT, env=child_environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace", timeout=180)
        (args.output / f"{name}.log").write_text(completed.stdout, encoding="utf-8")
        if completed.returncode:
            print(completed.stdout, flush=True)
            failures.append({"case": name, "exit_code": completed.returncode})
            break
        report = json.loads(path.read_text(encoding="utf-8"))
        if not report.get("overflow_refused"):
            raise RuntimeError("the output-capacity refusal control is missing")
        reports.append(report)
        print(f"GPU_ACCEPTANCE_DONE {name} {time.monotonic()-started:.3f}s", flush=True)
    if reports:
        expected = reports[0]["checks"]
        for report in reports[1:]:
            if len(report["checks"]) != len(expected):
                failures.append({"backend": report["backend"], "reason": "check count differs"})
                continue
            for before, after in zip(expected, report["checks"]):
                for key in ("name", "pairs", "selected", "stages", "cursor", "cancelled"):
                    if before.get(key) != after.get(key):
                        failures.append({"backend": report["backend"], "vendor": report["vendor"], "check": before["name"], "field": key, "expected_count": len(before.get(key, [])) if isinstance(before.get(key), list) else before.get(key), "actual_count": len(after.get(key, [])) if isinstance(after.get(key), list) else after.get(key)})
    summary = {"schema": "nioh3-gpu-search-e2e/v1", "passed": not failures and len(reports) == len(cases), "adapters": adapters, "cases_completed": len(reports), "checks_per_case": len(reports[0]["checks"]) if reports else 0, "failures": failures, "executable_sha256": sha256(executable), "helper_sha256": sha256(ROOT / "bin/nioh3_effect_preimage_accelerator.dll"), "source_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "evidence": {path.name: sha256(path) for path in args.output.iterdir() if path.suffix in (".json", ".log") and path.name != "summary.json"}}
    (args.output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: summary[key] for key in ("passed", "cases_completed", "checks_per_case", "failures")}, ensure_ascii=False), flush=True)
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
