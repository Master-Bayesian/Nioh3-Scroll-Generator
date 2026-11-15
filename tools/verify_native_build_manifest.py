"""Verify that the packaged Seed accelerator matches its tracked CUDA source."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import sys


EXPECTED_SCHEMA = "nioh3-native-build/v1"
EXPECTED_ABI = 2


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify(project_root: Path) -> dict[str, object]:
    source_path = project_root / "research" / "native_seed_accelerator.cu"
    binary_path = project_root / "bin" / "nioh3_seed_accelerator.dll"
    manifest_path = project_root / "bin" / "nioh3_seed_accelerator.build.json"
    for path in (source_path, binary_path, manifest_path):
        if not path.is_file():
            raise RuntimeError(f"required native build artifact is missing: {path}")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if manifest.get("schema") != EXPECTED_SCHEMA:
        raise RuntimeError("Seed accelerator build manifest schema is unsupported")
    if manifest.get("abi_version") != EXPECTED_ABI:
        raise RuntimeError("Seed accelerator manifest ABI does not match the wrapper")
    source_hash = sha256_file(source_path)
    binary_hash = sha256_file(binary_path)
    if manifest.get("source_sha256") != source_hash:
        raise RuntimeError("Seed accelerator source hash does not match its manifest")
    if manifest.get("binary_sha256") != binary_hash:
        raise RuntimeError("Seed accelerator DLL hash does not match its manifest")
    expected_build_id = f"sha256:{source_hash}"
    if manifest.get("build_id") != expected_build_id:
        raise RuntimeError("Seed accelerator manifest build identity is invalid")

    if sys.platform != "win32":
        raise RuntimeError("DLL ABI verification requires Windows")
    library = ctypes.WinDLL(str(binary_path))
    abi_function = library.seed_accelerator_abi_version
    abi_function.argtypes = ()
    abi_function.restype = ctypes.c_int
    build_function = library.seed_accelerator_build_id
    build_function.argtypes = ()
    build_function.restype = ctypes.c_char_p
    abi = int(abi_function())
    raw_build_id = build_function()
    build_id = raw_build_id.decode("ascii") if raw_build_id else ""
    if abi != EXPECTED_ABI:
        raise RuntimeError(f"Seed accelerator DLL exposes ABI {abi}, expected {EXPECTED_ABI}")
    if build_id != expected_build_id:
        raise RuntimeError("Seed accelerator DLL build identity does not match its source")
    directcompute = verify_directcompute(project_root)
    return {
        "directcompute": directcompute,
        "schema": EXPECTED_SCHEMA,
        "component": "nioh3_seed_accelerator",
        "abi_version": abi,
        "build_id": build_id,
        "source_sha256": source_hash,
        "binary_sha256": binary_hash,
    }


def verify_directcompute(project_root: Path) -> dict[str, object]:
    manifest_path = project_root / "bin/nioh3_effect_preimage_accelerator.build.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if manifest.get("schema") != "nioh3-directcompute-build/v1" or manifest.get("seed_search_abi") != 1:
        raise RuntimeError("DirectCompute build identity or ABI is unsupported")
    expected_sources = {
        "research/native_effect_preimage_accelerator.cpp",
        "research/native_directcompute_seed.inl",
        "research/native_seed_search.hlsl",
    }
    if set(manifest.get("sources", {})) != expected_sources:
        raise RuntimeError("DirectCompute manifest does not cover every native source")
    for relative, digest in manifest["sources"].items():
        if sha256_file(project_root / relative) != digest:
            raise RuntimeError(f"DirectCompute source differs from its manifest: {relative}")
    binary_path = project_root / "bin/nioh3_effect_preimage_accelerator.dll"
    digest = sha256_file(binary_path)
    if digest != manifest.get("binary_sha256"):
        raise RuntimeError("DirectCompute DLL differs from its build manifest")
    wrapper = (project_root / "crates/nioh3-worker/src/preimage.rs").read_text(encoding="utf-8")
    if f'"{digest}"' not in wrapper:
        raise RuntimeError("Rust DirectCompute artifact pin differs from the DLL")
    library = ctypes.WinDLL(str(binary_path))
    abi = library.d3d11_seed_search_abi_version
    abi.argtypes = ()
    abi.restype = ctypes.c_int
    if abi() != 1:
        raise RuntimeError("DirectCompute DLL exposes an unsupported seed-search ABI")
    for name in ("d3d11_seed_search_available", "execute_seed_search_d3d11"):
        getattr(library, name)
    # ABI checks are valid on hosted machines without a GPU. Actual device and
    # shader execution are accepted separately by verify_gpu_search_e2e.py.
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--project-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
    )
    args = parser.parse_args()
    print(json.dumps(verify(args.project_root.resolve()), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
