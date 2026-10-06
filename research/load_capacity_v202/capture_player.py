"""Bounded, read-only player-object capture for PC v2.02 load-capacity research."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import struct
import sys
from ctypes import wintypes
from datetime import datetime, timezone
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.game_compatibility import _file_version
from nioh3_scroll_editor.process_memory_readonly import ProcessReader

PLAYER_POINTER_RVA = 0x4751850
PLAYER_VTABLE_RVA = 0x402DA20
STAMINA_OFFSET = 0x9C
EXPECTED_FILE_VERSION = (2, 0, 2, 0)


def _process_executable(reader: ProcessReader) -> Path:
    query = reader.dll.QueryFullProcessImageNameW
    query.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
    query.restype = wintypes.BOOL
    buffer = ctypes.create_unicode_buffer(32768)
    length = wintypes.DWORD(len(buffer))
    if not query(reader.handle, 0, buffer, ctypes.byref(length)):
        raise ctypes.WinError(ctypes.get_last_error())
    return Path(buffer.value).resolve()


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def capture(args: argparse.Namespace) -> dict[str, object]:
    if args.output.exists():
        raise FileExistsError(f"Refusing to replace capture: {args.output}")
    if not 0x100 <= args.size <= 0x4000 or args.size % 16:
        raise ValueError("--size must be a 16-byte multiple from 0x100 to 0x4000")
    with ProcessReader() as reader:
        executable = _process_executable(reader)
        version = _file_version(executable)
        if version != EXPECTED_FILE_VERSION:
            raise RuntimeError(f"Expected PC v2.02 file version {EXPECTED_FILE_VERSION}, found {version}")
        birth_before = reader.creation_time()
        player = reader.u64(reader.module_base + PLAYER_POINTER_RVA)
        if not 0x10000 <= player < (1 << 47):
            raise RuntimeError(f"Invalid player pointer: {player:#x}")
        vtable = reader.u64(player)
        if vtable != reader.module_base + PLAYER_VTABLE_RVA:
            raise RuntimeError(
                f"Player vtable mismatch: {vtable:#x} != {reader.module_base + PLAYER_VTABLE_RVA:#x}"
            )
        stamina_before = reader.u32(player + STAMINA_OFFSET)
        raw = reader.read(player, args.size)
        stamina_after = reader.u32(player + STAMINA_OFFSET)
        if reader.creation_time() != birth_before or stamina_before != stamina_after:
            raise RuntimeError("Process identity or stamina changed during capture")
        if struct.unpack_from("<Q", raw)[0] != vtable:
            raise RuntimeError("Player object changed during capture")
        if args.stamina is not None and stamina_before != args.stamina:
            raise RuntimeError(f"Stamina mismatch: observed {stamina_before}, expected {args.stamina}")
        executable_hash = _sha256(executable)
        if reader.creation_time() != birth_before:
            raise RuntimeError("Process identity changed while hashing executable")
        report: dict[str, object] = {
            "schema": "nioh3-load-capacity-player-capture/v1",
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "read_only": True,
            "pid": reader.pid,
            "process_birth_filetime": birth_before,
            "executable": str(executable),
            "executable_size": executable.stat().st_size,
            "executable_sha256": executable_hash,
            "file_version": list(version),
            "module_base": f"0x{reader.module_base:X}",
            "player_pointer_rva": f"0x{PLAYER_POINTER_RVA:X}",
            "player_address": f"0x{player:X}",
            "vtable_rva": f"0x{PLAYER_VTABLE_RVA:X}",
            "stamina_offset": f"0x{STAMINA_OFFSET:X}",
            "stamina": stamina_before,
            "style": args.style,
            "ui_capacity": args.ui_capacity,
            "modifier_notes": args.modifier_notes,
            "snapshot_size": len(raw),
            "snapshot_sha256": hashlib.sha256(raw).hexdigest(),
            "snapshot_file": args.output.with_suffix(".bin").name,
            "note": "UI value and modifier notes are operator observations, not memory-derived facts.",
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    binary = args.output.with_suffix(".bin")
    if binary.exists():
        raise FileExistsError(f"Refusing to replace capture: {binary}")
    binary.write_bytes(raw)
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--style", required=True, choices=("samurai", "ninja"))
    parser.add_argument("--ui-capacity", required=True, type=float)
    parser.add_argument("--stamina", type=int, help="Optional expected in-game stamina value")
    parser.add_argument("--modifier-notes", required=True, help="Record load-capacity modifiers, or 'none'")
    parser.add_argument("--size", type=lambda value: int(value, 0), default=0x1000)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = capture(args)
    print(json.dumps({key: report[key] for key in ("style", "stamina", "ui_capacity", "pid", "snapshot_file")}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
