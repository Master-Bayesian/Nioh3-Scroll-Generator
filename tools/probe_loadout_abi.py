"""Query the exact Windows scorer copied from a package, in a fresh process.

This only probes exported ABI numbers. Numerical compatibility is separately
established by the real packaged worker; it is never inferred from ABI equality.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("library", type=Path)
    parser.add_argument("backend", choices=("cpu", "card"))
    args = parser.parse_args()
    if sys.platform != "win32":
        print(json.dumps({"passed": False, "error": "Windows x64 required"}))
        return 1
    path = args.library.resolve(strict=True)
    expected_name = "main_cpu.dll" if args.backend == "cpu" else "main_device.dll"
    if path.name != expected_name:
        raise ValueError("unexpected scorer name")
    with os.add_dll_directory(str(path.parent)):
        # Same restricted dependency search policy as gpu.rs.
        dll = ctypes.CDLL(str(path), winmode=0x00000100 | 0x00000800)
        observations = {}
        for name in ("nk_context_abi", "nk_main_" + ("cpu" if args.backend == "cpu" else "device") + "_components_abi"):
            function = getattr(dll, name)
            function.argtypes = []
            function.restype = ctypes.c_uint32
            observations[name] = function()
    expected = {"nk_context_abi": 2, "nk_main_" + ("cpu" if args.backend == "cpu" else "device") + "_components_abi": 1}
    passed = observations == expected
    print(json.dumps({"passed": passed, "path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "observed_abis": observations}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
