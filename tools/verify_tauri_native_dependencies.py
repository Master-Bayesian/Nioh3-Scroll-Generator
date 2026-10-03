"""Reject product PE files that require separately installed VC runtime DLLs."""
from pathlib import Path
import pefile


def inspect_native_imports(raw: bytes, label: str) -> dict:
    with pefile.PE(data=raw, fast_load=True) as pe:
        pe.parse_data_directories(directories=[
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_IMPORT"],
            pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT"],
        ])
        imports = sorted(entry.dll.decode("ascii") for entry in getattr(pe, "DIRECTORY_ENTRY_IMPORT", []))
        delayed = sorted(entry.dll.decode("ascii") for entry in getattr(pe, "DIRECTORY_ENTRY_DELAY_IMPORT", []))
    external_crt = [name for name in imports + delayed
                    if name.lower().startswith(("vcruntime", "msvcp", "msvcr", "concrt"))]
    if external_crt:
        raise ValueError(f"STANDALONE_EXTERNAL_CRT: {label} imports {', '.join(external_crt)}; build the Windows executable with static CRT")
    return {"member": label, "imports": imports, "delayedImports": delayed}


def verify_runtime_native_dependencies(root: Path) -> dict:
    root = root.resolve(strict=True)
    members = sorted(path for path in root.rglob("*") if path.is_file() and path.suffix.lower() in (".exe", ".dll"))
    records = [inspect_native_imports(path.read_bytes(), path.relative_to(root).as_posix()) for path in members]
    return {"schema": "nioh3-native-dependencies/v1", "ok": True, "staticVcRuntime": True,
            "systemPrerequisites": ["Windows 10 or later system APIs and UCRT", "Microsoft Edge WebView2 Runtime"],
            "optionalAcceleration": "Installed GPU drivers; unavailable acceleration is explicitly reported by the worker",
            "files": records}
