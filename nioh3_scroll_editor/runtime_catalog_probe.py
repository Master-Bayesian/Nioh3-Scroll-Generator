from __future__ import annotations

import argparse
import ctypes
import json
import sys
from ctypes import wintypes
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Iterator, Sequence

from .native import find_module_base, find_nioh3_pid


PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010
MEM_COMMIT = 0x1000
MEM_IMAGE = 0x1000000
MEM_MAPPED = 0x40000
MEM_PRIVATE = 0x20000
PAGE_GUARD = 0x100
PAGE_NOACCESS = 0x01
READABLE_PAGE_TYPES = {0x02, 0x04, 0x08, 0x20, 0x40, 0x80}
DEFAULT_CHUNK_SIZE = 4 * 1024 * 1024


class MEMORY_BASIC_INFORMATION(ctypes.Structure):
    _fields_ = [
        ("BaseAddress", wintypes.LPVOID),
        ("AllocationBase", wintypes.LPVOID),
        ("AllocationProtect", wintypes.DWORD),
        ("PartitionId", wintypes.WORD),
        ("RegionSize", ctypes.c_size_t),
        ("State", wintypes.DWORD),
        ("Protect", wintypes.DWORD),
        ("Type", wintypes.DWORD),
    ]


@dataclass(frozen=True)
class MemoryRegion:
    base: int
    size: int
    allocation_base: int
    protection: int
    memory_type: int


@dataclass(frozen=True)
class StringHit:
    term: str
    encoding: str
    address: str
    module_rva: str | None
    region_base: str
    region_size: int
    allocation_base: str
    protection: str
    memory_type: str
    context_hex: str


def _require_windows() -> None:
    if sys.platform != "win32":
        raise RuntimeError("Runtime catalog probing is supported only on Windows")


def _kernel32() -> ctypes.WinDLL:
    _require_windows()
    dll = ctypes.WinDLL("kernel32", use_last_error=True)
    dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    dll.OpenProcess.restype = wintypes.HANDLE
    dll.CloseHandle.argtypes = [wintypes.HANDLE]
    dll.CloseHandle.restype = wintypes.BOOL
    dll.VirtualQueryEx.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCVOID,
        ctypes.POINTER(MEMORY_BASIC_INFORMATION),
        ctypes.c_size_t,
    ]
    dll.VirtualQueryEx.restype = ctypes.c_size_t
    dll.ReadProcessMemory.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCVOID,
        wintypes.LPVOID,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    dll.ReadProcessMemory.restype = wintypes.BOOL
    return dll


def _is_readable(protection: int) -> bool:
    if protection & PAGE_GUARD or protection & PAGE_NOACCESS:
        return False
    return (protection & 0xFF) in READABLE_PAGE_TYPES


def iter_readable_regions(handle: int) -> Iterator[MemoryRegion]:
    dll = _kernel32()
    address = 0
    maximum_address = (1 << 47) - 1
    while address < maximum_address:
        mbi = MEMORY_BASIC_INFORMATION()
        result = dll.VirtualQueryEx(
            handle,
            ctypes.c_void_p(address),
            ctypes.byref(mbi),
            ctypes.sizeof(mbi),
        )
        if not result:
            break
        base = ctypes.cast(mbi.BaseAddress, ctypes.c_void_p).value or 0
        size = int(mbi.RegionSize)
        if size <= 0:
            break
        if mbi.State == MEM_COMMIT and _is_readable(mbi.Protect):
            yield MemoryRegion(
                base=base,
                size=size,
                allocation_base=(
                    ctypes.cast(mbi.AllocationBase, ctypes.c_void_p).value or 0
                ),
                protection=int(mbi.Protect),
                memory_type=int(mbi.Type),
            )
        next_address = base + size
        if next_address <= address:
            break
        address = next_address


def _read_process_memory(dll: ctypes.WinDLL, handle: int, address: int, size: int) -> bytes:
    buffer = ctypes.create_string_buffer(size)
    bytes_read = ctypes.c_size_t()
    ok = dll.ReadProcessMemory(
        handle,
        ctypes.c_void_p(address),
        buffer,
        size,
        ctypes.byref(bytes_read),
    )
    if not ok or bytes_read.value == 0:
        return b""
    return buffer.raw[: bytes_read.value]


def _encoded_patterns(terms: Iterable[str]) -> list[tuple[str, str, bytes]]:
    patterns: list[tuple[str, str, bytes]] = []
    for term in dict.fromkeys(terms):
        if not term:
            continue
        patterns.append((term, "utf-8", term.encode("utf-8")))
        patterns.append((term, "utf-16-le", term.encode("utf-16-le")))
    return patterns


def scan_process_strings(
    terms: Sequence[str],
    *,
    module_base: int,
    module_size: int,
    chunk_size: int = DEFAULT_CHUNK_SIZE,
    max_hits_per_pattern: int = 256,
) -> tuple[list[StringHit], dict[str, int]]:
    if chunk_size < 4096:
        raise ValueError("chunk_size must be at least one page")
    patterns = _encoded_patterns(terms)
    if not patterns:
        raise ValueError("at least one non-empty term is required")
    maximum_pattern_size = max(len(pattern) for _, _, pattern in patterns)
    overlap_size = maximum_pattern_size - 1
    pid = find_nioh3_pid()
    dll = _kernel32()
    handle = dll.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    hits: list[StringHit] = []
    counts = {f"{term}:{encoding}": 0 for term, encoding, _ in patterns}
    scanned_bytes = 0
    scanned_regions = 0
    try:
        for region in iter_readable_regions(handle):
            scanned_regions += 1
            region_end = region.base + region.size
            cursor = region.base
            previous_tail = b""
            while cursor < region_end:
                requested = min(chunk_size, region_end - cursor)
                block = _read_process_memory(dll, handle, cursor, requested)
                if not block:
                    previous_tail = b""
                    cursor += requested
                    continue
                scanned_bytes += len(block)
                haystack = previous_tail + block
                haystack_base = cursor - len(previous_tail)
                for term, encoding, pattern in patterns:
                    count_key = f"{term}:{encoding}"
                    if counts[count_key] >= max_hits_per_pattern:
                        continue
                    offset = 0
                    while counts[count_key] < max_hits_per_pattern:
                        found = haystack.find(pattern, offset)
                        if found < 0:
                            break
                        absolute = haystack_base + found
                        context_start = max(0, found - 32)
                        context_end = min(len(haystack), found + len(pattern) + 32)
                        module_rva = None
                        if module_base <= absolute < module_base + module_size:
                            module_rva = f"0x{absolute - module_base:X}"
                        hits.append(
                            StringHit(
                                term=term,
                                encoding=encoding,
                                address=f"0x{absolute:X}",
                                module_rva=module_rva,
                                region_base=f"0x{region.base:X}",
                                region_size=region.size,
                                allocation_base=f"0x{region.allocation_base:X}",
                                protection=f"0x{region.protection:X}",
                                memory_type={
                                    MEM_IMAGE: "MEM_IMAGE",
                                    MEM_MAPPED: "MEM_MAPPED",
                                    MEM_PRIVATE: "MEM_PRIVATE",
                                }.get(region.memory_type, f"0x{region.memory_type:X}"),
                                context_hex=haystack[context_start:context_end].hex(" "),
                            )
                        )
                        counts[count_key] += 1
                        offset = found + 1
                previous_tail = block[-overlap_size:] if overlap_size else b""
                cursor += len(block)
                if len(block) < requested:
                    cursor += requested - len(block)
    finally:
        dll.CloseHandle(handle)
    stats = {
        "pid": pid,
        "scanned_regions": scanned_regions,
        "scanned_bytes": scanned_bytes,
        "total_hits": len(hits),
    }
    return hits, stats


def _module_size(executable: Path) -> int:
    import pefile

    pe = pefile.PE(str(executable), fast_load=True)
    try:
        return int(pe.OPTIONAL_HEADER.SizeOfImage)
    finally:
        pe.close()


def write_probe_report(
    output: Path,
    terms: Sequence[str],
    executable: Path,
) -> dict[str, object]:
    module_base = find_module_base(find_nioh3_pid())
    module_size = _module_size(executable)
    hits, stats = scan_process_strings(
        terms,
        module_base=module_base,
        module_size=module_size,
    )
    report: dict[str, object] = {
        "schema": "nioh3-runtime-localization-string-probe/v1",
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "target": {
            "module": executable.name,
            "module_size": module_size,
            "module_base_recorded": False,
        },
        "terms": list(terms),
        "stats": {key: value for key, value in stats.items() if key != "pid"},
        "hits": [asdict(hit) for hit in hits],
        "interpretation": (
            "String addresses are diagnostic leads only. Product constants require an RVA/AOB "
            "for the resolver and confirmation after a second process launch."
        ),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return report


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Read-only runtime probe for Nioh 3 localization strings"
    )
    parser.add_argument("terms", nargs="+", help="Exact strings to locate")
    parser.add_argument(
        "--exe",
        type=Path,
        default=Path(r"D:\Steam\steamapps\common\Nioh3\Nioh3.exe"),
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    report = write_probe_report(args.output, args.terms, args.exe)
    print(json.dumps(report["stats"], ensure_ascii=False))
    for hit in report["hits"]:
        print(
            f"{hit['term']} {hit['encoding']} {hit['address']} "
            f"rva={hit['module_rva']} type={hit['memory_type']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
