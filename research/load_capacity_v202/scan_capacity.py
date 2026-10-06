"""Bounded numeric memory search and same-address verification; read-only."""

from __future__ import annotations

import argparse
import json
import math
import struct
from datetime import datetime, timezone
from pathlib import Path

from capture_player import EXPECTED_FILE_VERSION, _process_executable, _sha256
from nioh3_scroll_editor.game_compatibility import _file_version
from nioh3_scroll_editor.process_memory_readonly import ProcessReader
from nioh3_scroll_editor.runtime_catalog_probe import (
    MEM_PRIVATE,
    MEM_MAPPED,
    iter_readable_regions,
    _read_process_memory,
)

PATTERNS = {
    "f32": lambda value: struct.pack("<f", value),
    "f64": lambda value: struct.pack("<d", value),
    "u32_x10": lambda value: struct.pack("<I", round(value * 10)),
    "u32_x100": lambda value: struct.pack("<I", round(value * 100)),
    "u32_x1000": lambda value: struct.pack("<I", round(value * 1000)),
}


def _identity(reader: ProcessReader) -> dict[str, object]:
    executable = _process_executable(reader)
    version = _file_version(executable)
    if version != EXPECTED_FILE_VERSION:
        raise RuntimeError(f"Expected PC v2.02 file version {EXPECTED_FILE_VERSION}, found {version}")
    return {
        "pid": reader.pid,
        "process_birth_filetime": reader.creation_time(),
        "executable": str(executable),
        "executable_sha256": _sha256(executable),
        "file_version": list(version),
        "module_base": f"0x{reader.module_base:X}",
    }


def scan(reader: ProcessReader, value: float, max_bytes: int, max_hits: int, types: list[str]) -> dict[str, object]:
    patterns = {kind: PATTERNS[kind](value) for kind in types}
    regions = list(iter_readable_regions(reader.handle))
    regions.sort(key=lambda region: (0 if region.memory_type == MEM_PRIVATE else 1 if region.memory_type == MEM_MAPPED else 2, region.base))
    hits: list[dict[str, object]] = []
    scanned_bytes = 0
    scanned_regions = 0
    unreadable_chunks = 0
    truncated = False
    chunk_size = 4 * 1024 * 1024
    for region in regions:
        if scanned_bytes >= max_bytes or len(hits) >= max_hits:
            truncated = True
            break
        scanned_regions += 1
        cursor = region.base
        prior = b""
        end = region.base + region.size
        while cursor < end and scanned_bytes < max_bytes and len(hits) < max_hits:
            requested = min(chunk_size, end - cursor, max_bytes - scanned_bytes)
            block = _read_process_memory(reader.dll, reader.handle, cursor, requested)
            if not block:
                unreadable_chunks += 1
                prior = b""
                cursor += requested
                continue
            scanned_bytes += len(block)
            haystack = prior + block
            haystack_base = cursor - len(prior)
            for kind, pattern in patterns.items():
                offset = 0
                while len(hits) < max_hits:
                    found = haystack.find(pattern, offset)
                    if found < 0:
                        break
                    address = haystack_base + found
                    if address % len(pattern) == 0:
                        hits.append({
                            "address": f"0x{address:X}",
                            "type": kind,
                            "region_base": f"0x{region.base:X}",
                            "region_size": region.size,
                            "region_type": f"0x{region.memory_type:X}",
                            "region_protection": f"0x{region.protection:X}",
                            "raw_hex": pattern.hex(),
                        })
                    offset = found + 1
            prior = haystack[-7:]
            cursor += requested
        if cursor < end:
            truncated = True
    return {
        "search_value": value,
        "search_types": types,
        "max_scan_bytes": max_bytes,
        "max_hits": max_hits,
        "readable_region_count": len(regions),
        "scanned_regions": scanned_regions,
        "scanned_bytes": scanned_bytes,
        "unreadable_chunks": unreadable_chunks,
        "truncated": truncated,
        "hits": hits,
    }


def scan_f32_range(reader: ProcessReader, value: float, tolerance: float, max_bytes: int, max_hits: int) -> dict[str, object]:
    """Vectorized search for an internal float that rounds to the UI value."""
    import numpy as np

    regions = list(iter_readable_regions(reader.handle))
    regions.sort(key=lambda region: (0 if region.memory_type == MEM_PRIVATE else 1 if region.memory_type == MEM_MAPPED else 2, region.base))
    hits: list[dict[str, object]] = []
    scanned_bytes = 0
    scanned_regions = 0
    unreadable_chunks = 0
    chunk_size = 4 * 1024 * 1024
    for region in regions:
        if scanned_bytes >= max_bytes or len(hits) >= max_hits:
            break
        scanned_regions += 1
        cursor = region.base
        end = region.base + region.size
        while cursor < end and scanned_bytes < max_bytes and len(hits) < max_hits:
            requested = min(chunk_size, end - cursor, max_bytes - scanned_bytes)
            block = _read_process_memory(reader.dll, reader.handle, cursor, requested)
            if not block:
                unreadable_chunks += 1
                cursor += requested
                continue
            scanned_bytes += len(block)
            aligned_start = (-cursor) & 3
            usable_size = (len(block) - aligned_start) & ~3
            if usable_size:
                values = np.frombuffer(block, dtype="<f4", count=usable_size // 4, offset=aligned_start)
                indices = np.flatnonzero((values >= value - tolerance) & (values < value + tolerance))
                for index in indices[: max_hits - len(hits)]:
                    address = cursor + aligned_start + int(index) * 4
                    hits.append({
                        "address": f"0x{address:X}",
                        "type": "f32",
                        "value": float(values[index]),
                        "region_base": f"0x{region.base:X}",
                        "region_size": region.size,
                        "region_type": f"0x{region.memory_type:X}",
                        "region_protection": f"0x{region.protection:X}",
                    })
            cursor += requested
    return {
        "search_value": value,
        "search_type": "f32_range",
        "tolerance": tolerance,
        "max_scan_bytes": max_bytes,
        "max_hits": max_hits,
        "readable_region_count": len(regions),
        "scanned_regions": scanned_regions,
        "scanned_bytes": scanned_bytes,
        "unreadable_chunks": unreadable_chunks,
        "truncated": scanned_bytes >= max_bytes or len(hits) >= max_hits,
        "hits": hits,
    }


def _decode(kind: str, raw: bytes) -> float:
    if kind == "f32":
        return struct.unpack("<f", raw)[0]
    if kind == "f64":
        return struct.unpack("<d", raw)[0]
    scale = {"u32_x10": 10, "u32_x100": 100, "u32_x1000": 1000}[kind]
    return struct.unpack("<I", raw)[0] / scale


def verify(reader: ProcessReader, source: dict[str, object], value: float) -> dict[str, object]:
    expected_identity = source["identity"]
    current_identity = _identity(reader)
    for key in ("process_birth_filetime", "executable_sha256", "module_base"):
        if current_identity[key] != expected_identity[key]:
            raise RuntimeError(f"Process identity changed: {key}")
    results = []
    for hit in source["scan"]["hits"]:
        kind = str(hit["type"])
        size = 8 if kind == "f64" else 4
        try:
            raw = reader.read(int(str(hit["address"]), 16), size)
            decoded = _decode(kind, raw)
            matches = math.isfinite(decoded) and abs(decoded - value) <= 0.051
            results.append({"address": hit["address"], "type": kind, "value": decoded, "raw_hex": raw.hex(), "matches_ui": matches})
        except (OSError, RuntimeError):
            results.append({"address": hit["address"], "type": kind, "unreadable": True, "matches_ui": False})
    return {
        "expected_ui_value": value,
        "source_capture": source["captured_at_utc"],
        "source_search_value": source["scan"]["search_value"],
        "identity": current_identity,
        "read_only": True,
        "verified_at_utc": datetime.now(timezone.utc).isoformat(),
        "results": results,
        "matches": [item for item in results if item["matches_ui"]],
        "checked_count": len(results),
        "unreadable_count": sum(bool(item.get("unreadable")) for item in results),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    search_parser = sub.add_parser("scan")
    search_parser.add_argument("--value", type=float, required=True)
    search_parser.add_argument("--max-bytes", type=lambda value: int(value, 0), default=4 * 1024**3)
    search_parser.add_argument("--max-hits", type=int, default=10000)
    search_parser.add_argument("--types", default=",".join(PATTERNS), help="Comma-separated numeric representations")
    search_parser.add_argument("--output", type=Path, required=True)
    range_parser = sub.add_parser("scan-range")
    range_parser.add_argument("--value", type=float, required=True)
    range_parser.add_argument("--tolerance", type=float, default=0.05)
    range_parser.add_argument("--max-bytes", type=lambda value: int(value, 0), default=4 * 1024**3)
    range_parser.add_argument("--max-hits", type=int, default=10000)
    range_parser.add_argument("--output", type=Path, required=True)
    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("--source", type=Path, required=True)
    verify_parser.add_argument("--value", type=float, required=True)
    verify_parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not math.isfinite(args.value) or args.value <= 0:
        raise ValueError("Capacity value must be finite and positive")
    if args.output.exists():
        raise FileExistsError(f"Refusing to replace output: {args.output}")
    if args.command in ("scan", "scan-range") and (not 0 < args.max_bytes <= 16 * 1024**3 or not 0 < args.max_hits <= 100000):
        raise ValueError("Scan bounds exceed allowed limits")
    if args.command == "scan-range" and not 0 < args.tolerance <= 0.5:
        raise ValueError("Range tolerance must be positive and at most 0.5")
    types = [item.strip() for item in args.types.split(",")] if args.command == "scan" else []
    if args.command == "scan" and (not types or any(item not in PATTERNS for item in types)):
        raise ValueError("Unknown or empty numeric representation list")
    with ProcessReader() as reader:
        identity = _identity(reader)
        if args.command in ("scan", "scan-range"):
            report = {
                "schema": "nioh3-load-capacity-numeric-scan/v1",
                "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                "read_only": True,
                "identity": identity,
                "scan": scan(reader, args.value, args.max_bytes, args.max_hits, types)
                if args.command == "scan" else scan_f32_range(reader, args.value, args.tolerance, args.max_bytes, args.max_hits),
            }
        else:
            source = json.loads(args.source.read_text(encoding="utf-8"))
            if source.get("schema") != "nioh3-load-capacity-numeric-scan/v1":
                raise ValueError("Unsupported source scan")
            report = {"schema": "nioh3-load-capacity-candidate-verification/v1", **verify(reader, source, args.value)}
        if reader.creation_time() != identity["process_birth_filetime"]:
            raise RuntimeError("Process identity changed during scan")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    summary = report.get("scan", report)
    print(json.dumps({key: summary[key] for key in ("scanned_bytes", "scanned_regions", "truncated") if key in summary} | {"hits": len(summary.get("hits", summary.get("matches", [])))}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
