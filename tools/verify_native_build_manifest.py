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
    return {
        "schema": EXPECTED_SCHEMA,
        "component": "nioh3_seed_accelerator",
        "abi_version": abi,
        "build_id": build_id,
        "source_sha256": source_hash,
        "binary_sha256": binary_hash,
    }


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
