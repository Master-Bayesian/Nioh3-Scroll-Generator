"""Write MANIFEST.json: SHA-256 of every package file and of the external dependencies the code reads."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


files = {str(p.relative_to(HERE)).replace("\\", "/"): {"sha256": sha(p), "size": p.stat().st_size}
         for p in sorted(HERE.rglob("*")) if p.is_file() and ".git" not in p.parts and p.name != "MANIFEST.json"}
external = {
    "pro_r2_package_runtime": "F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/project-source/runtime/r12_bind.py",
    "r13_main_replay": "F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/project-source/runtime/project-source/r13-main/replay.py",
    "grade_curve_binding": "F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/candidate-production/static-binding/evidence/grade-curves/BINDING.json",
    "grade_curve_resource": "F:/Nioh3_r16_build/deliverables/r16-equipment-execution-20261020/candidate-production/static-binding/evidence/grade-curves/resource01119.bin",
    "default_D_page_dump": "F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/evidence/r2-hits/PAGE_DUMP_pid29088.json",
    "studio_protected_worker": "C:/Users/oudeb/AppData/Local/Nioh3Studio/onefile/3f90d8b543f01c6ccbbef35f14c43775df7275e698c8ba69d6a8082846d16e31/worker/nioh3-protected-worker.exe",
}
ext = {k: {"path": v, "sha256": sha(v) if Path(v).exists() else None} for k, v in external.items()}
ext["r10_original_cache"] = {"path": "F:/Nioh3_r16_build/cache/original", "note": "materialized and membership-checked by r12_bind.prepare"}
(HERE / "MANIFEST.json").write_text(json.dumps({"game": "Nioh 3 PC 2.0.2.0", "exe_sha256": "e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130",
                                                "files": files, "external": ext}, indent=1) + "\n")
print(len(files), "files;", sum(v["sha256"] is not None for v in ext.values() if "sha256" in v), "external pinned")
