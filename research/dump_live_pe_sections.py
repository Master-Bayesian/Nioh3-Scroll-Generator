from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
import sys

import pefile

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.game_compatibility import _file_version
from nioh3_scroll_editor.native import find_module_base, find_nioh3_pid
from nioh3_scroll_editor.runtime_catalog_probe import (
    PROCESS_QUERY_INFORMATION,
    PROCESS_VM_READ,
    _kernel32,
    _read_process_memory,
)


def dump_sections(
    executable: Path,
    output_dir: Path,
    section_names: set[str],
) -> dict[str, object]:
    executable = executable.resolve()
    file_version = _file_version(executable)
    version_text = ".".join(str(part) for part in file_version)
    executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest().upper()
    pid = find_nioh3_pid()
    module_base = find_module_base(pid)
    dll = _kernel32()
    handle = dll.OpenProcess(
        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
        False,
        pid,
    )
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())

    pe = pefile.PE(str(executable), fast_load=True)
    try:
        output_dir.mkdir(parents=True, exist_ok=True)
        dumped: list[dict[str, object]] = []
        for section in pe.sections:
            name = section.Name.rstrip(b"\0").decode("ascii", errors="replace")
            if name not in section_names:
                continue
            rva = int(section.VirtualAddress)
            size = max(int(section.Misc_VirtualSize), int(section.SizeOfRawData))
            data = _read_process_memory(dll, handle, module_base + rva, size)
            if len(data) != size:
                raise RuntimeError(
                    f"ReadProcessMemory returned {len(data):#x} of {size:#x} bytes for {name}"
                )
            filename = f"Nioh3_v{version_text}{name}.bin"
            destination = output_dir / filename
            destination.write_bytes(data)
            dumped.append(
                {
                    "name": name,
                    "rva": f"0x{rva:X}",
                    "size": size,
                    "filename": filename,
                    "sha256": hashlib.sha256(data).hexdigest().upper(),
                }
            )
        missing = sorted(section_names - {str(item["name"]) for item in dumped})
        if missing:
            raise RuntimeError(f"PE sections not found: {', '.join(missing)}")
    finally:
        pe.close()
        dll.CloseHandle(handle)

    report: dict[str, object] = {
        "schema": "nioh3-live-pe-section-dump/v1",
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "file_version": list(file_version),
        "file_version_text": version_text,
        "module": executable.name,
        "executable": {
            "size": executable.stat().st_size,
            "sha256": executable_sha256,
        },
        "module_base_recorded": False,
        "process_id_recorded": False,
        "sections": dumped,
        "limitations": [
            "These bytes were read from the running game module after loader transformations.",
            "The dump contains game code and data, not save data or account identifiers.",
            "Do not distribute the raw sections publicly.",
        ],
    }
    manifest = output_dir / "manifest.json"
    manifest.write_text(json.dumps(report, indent=2), encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Read selected Nioh3.exe PE sections from the running process"
    )
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--section",
        action="append",
        dest="sections",
        default=[],
        help="PE section name to dump; repeat for multiple sections",
    )
    args = parser.parse_args()
    requested = set(args.sections or [".text", ".rdata", ".pdata"])
    report = dump_sections(args.exe, args.output_dir, requested)
    print(
        json.dumps(
            {
                "output_dir": str(args.output_dir),
                "sections": report["sections"],
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
