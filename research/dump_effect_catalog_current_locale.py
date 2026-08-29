from __future__ import annotations

import argparse
import ctypes
import json
import struct
import sys
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Iterator

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.native import find_module_base, find_nioh3_pid  # noqa: E402
from nioh3_scroll_editor.runtime_catalog_probe import (  # noqa: E402
    MEM_PRIVATE,
    PROCESS_QUERY_INFORMATION,
    PROCESS_VM_READ,
    MemoryRegion,
    _kernel32,
    _read_process_memory,
    iter_readable_regions,
)

GAME_VERSION = "2.00.02"
EFFECT_LOOKUP_RVA = 0x577808
EFFECT_LOOKUP_SIGNATURE = bytes.fromhex(
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57"
)
EFFECT_ROOT_POINTER_RVA = 0x45B1E00
EFFECT_ROW_SIZE = 0xD8
PREFIX_TABLE_POINTER_OFFSET = 0xA8
PREFIX_ROW_SIZE = 0x70
MAX_EFFECT_ROWS = 200_000
MAX_HASH_ENTRIES = 1_000_000
MAX_STRING_CODE_UNITS = 512
CURRENT_LANGUAGE_POOL_RADIUS = 768 * 1024

# These effect -> stable text-ID anchors were observed in the canonical
# localization pool and independently validated through the native two-level
# lookup chain in the current Chinese v2.00.02 runtime capture.
TEXT_ID_ANCHORS: dict[int, int] = {
    0x0000AE5A: 0x0349962F,  # 技之深奥
    0x0000BABD: 0x03630BA9,  # 月读的恩宠 (final-record context)
    0x0000A051: 0x036C0107,  # 对妖战术
}

# Discovery hint only: in the supplied launch, the canonical pool was near
# allocation_base + 0x1514F0000. It is always validated by native text IDs and
# never accepted as a product constant.
LOCALIZATION_POOL_RELATIVE_HINT = 0x1514F0000


@dataclass(frozen=True, slots=True)
class EffectRow:
    effect_id: int
    row_index: int
    row_address: int
    row: bytes


@dataclass(frozen=True, slots=True)
class PrefixRow:
    prefix_id: int
    row_index: int
    row_address: int
    row: bytes


@dataclass(frozen=True, slots=True)
class TextEntry:
    text_id: int
    address: int
    code_units_including_null: int
    text: str


class ProcessReader:
    def __init__(self) -> None:
        if sys.platform != "win32":
            raise RuntimeError("This catalog dumper must run on Windows")
        self.pid = find_nioh3_pid()
        self.module_base = find_module_base(self.pid)
        self.dll = _kernel32()
        self.handle = self.dll.OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            False,
            self.pid,
        )
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())

    def close(self) -> None:
        if self.handle:
            self.dll.CloseHandle(self.handle)
            self.handle = None

    def __enter__(self) -> "ProcessReader":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def read(self, address: int, size: int, *, exact: bool = True) -> bytes:
        data = _read_process_memory(self.dll, self.handle, address, size)
        if exact and len(data) != size:
            raise RuntimeError(
                f"ReadProcessMemory({address:#x}, {size:#x}) returned {len(data):#x} bytes"
            )
        return data

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.read(address, 4))[0]

    def u64(self, address: int) -> int:
        return struct.unpack("<Q", self.read(address, 8))[0]


def normalize_locale(value: str) -> str:
    value = value.strip().replace("_", "-")
    if not value:
        raise ValueError("locale cannot be empty")
    parts = value.split("-")
    if len(parts) == 1:
        return parts[0].lower()
    return "-".join((parts[0].lower(), parts[1].upper(), *parts[2:]))


def validate_version(reader: ProcessReader) -> None:
    actual = reader.read(reader.module_base + EFFECT_LOOKUP_RVA, len(EFFECT_LOOKUP_SIGNATURE))
    if actual != EFFECT_LOOKUP_SIGNATURE:
        raise RuntimeError(
            "Nioh3.exe does not match the verified v2.00.02 effect lookup signature; "
            f"expected {EFFECT_LOOKUP_SIGNATURE.hex(' ')}, got {actual.hex(' ')}"
        )


def enumerate_effect_rows(reader: ProcessReader) -> tuple[list[EffectRow], dict[str, int]]:
    root = reader.u64(reader.module_base + EFFECT_ROOT_POINTER_RVA)
    if root == 0:
        raise RuntimeError("effect root pointer is null; wait at the title screen until data loads")
    context = reader.u64(root + 0xC8)
    if context == 0:
        raise RuntimeError("effect table context is null")
    hash_object = reader.u64(context + 0x20)
    row_store = reader.u64(context)
    if hash_object == 0 or row_store == 0:
        raise RuntimeError("effect table pointers are incomplete")

    sentinel_key = reader.u32(hash_object + 0x04)
    entries_start = reader.u64(hash_object + 0x08)
    entries_end = reader.u64(hash_object + 0x10)
    if entries_end < entries_start or (entries_end - entries_start) % 8:
        raise RuntimeError("effect hash entry range is malformed")
    entry_count = (entries_end - entries_start) // 8
    if not 1 <= entry_count <= MAX_HASH_ENTRIES:
        raise RuntimeError(f"implausible effect hash entry count: {entry_count}")

    row_count = reader.u32(row_store + 0x04)
    if not 1 <= row_count <= MAX_EFFECT_ROWS:
        raise RuntimeError(f"implausible effect row count: {row_count}")
    row_base = row_store + 0x08

    raw_entries = reader.read(entries_start, entry_count * 8)
    key_to_index: dict[int, int] = {}
    duplicate_keys: Counter[int] = Counter()
    for offset in range(0, len(raw_entries), 8):
        effect_id, row_index = struct.unpack_from("<II", raw_entries, offset)
        if effect_id == sentinel_key or row_index >= row_count:
            continue
        if effect_id in key_to_index and key_to_index[effect_id] != row_index:
            duplicate_keys[effect_id] += 1
            continue
        key_to_index[effect_id] = row_index

    rows: list[EffectRow] = []
    for effect_id, row_index in sorted(key_to_index.items()):
        address = row_base + row_index * EFFECT_ROW_SIZE
        rows.append(
            EffectRow(
                effect_id=effect_id,
                row_index=row_index,
                row_address=address,
                row=reader.read(address, EFFECT_ROW_SIZE),
            )
        )
    metadata = {
        "root": root,
        "context": context,
        "hash_object": hash_object,
        "row_store": row_store,
        "hash_entry_count": entry_count,
        "row_count": row_count,
        "enumerated_effect_count": len(rows),
        "duplicate_key_count": len(duplicate_keys),
        "sentinel_key": sentinel_key,
    }
    return rows, metadata


def enumerate_prefix_rows(reader: ProcessReader) -> tuple[dict[int, PrefixRow], dict[str, int]]:
    """Enumerate the second-level prefix table used by +0x578680.

    The first dword of each 0xD8 effect row packs ``prefix_id << 16 | effect_id``.
    The stable localization text ID is not stored in that row. The game looks up
    the high-word prefix in the manager's +0xA8 table, whose 0x70-byte row stores
    the text ID. This mirrors the native lookup chain observed at UI callers:

        +0x577808 -> word [effect_row+2] -> +0x578680 -> dword [prefix_row+0x2C]
    """
    root = reader.u64(reader.module_base + EFFECT_ROOT_POINTER_RVA)
    if root == 0:
        raise RuntimeError("effect root pointer is null; wait at the title screen until data loads")
    context = reader.u64(root + PREFIX_TABLE_POINTER_OFFSET)
    if context == 0:
        raise RuntimeError("effect prefix-table context is null")
    hash_object = reader.u64(context + 0x20)
    row_store = reader.u64(context)
    if hash_object == 0 or row_store == 0:
        raise RuntimeError("effect prefix-table pointers are incomplete")

    sentinel_key = struct.unpack("<H", reader.read(hash_object + 0x04, 2))[0]
    entries_start = reader.u64(hash_object + 0x08)
    entries_end = reader.u64(hash_object + 0x10)
    if entries_end < entries_start or (entries_end - entries_start) % 8:
        raise RuntimeError("effect prefix hash entry range is malformed")
    entry_count = (entries_end - entries_start) // 8
    if not 1 <= entry_count <= MAX_HASH_ENTRIES:
        raise RuntimeError(f"implausible effect prefix hash entry count: {entry_count}")

    row_count = reader.u32(row_store + 0x04)
    if not 1 <= row_count <= MAX_EFFECT_ROWS:
        raise RuntimeError(f"implausible effect prefix row count: {row_count}")
    row_base = row_store + 0x08
    raw_entries = reader.read(entries_start, entry_count * 8)
    key_to_index: dict[int, int] = {}
    duplicate_keys: Counter[int] = Counter()
    for offset in range(0, len(raw_entries), 8):
        prefix_id = struct.unpack_from("<H", raw_entries, offset)[0]
        row_index = struct.unpack_from("<I", raw_entries, offset + 4)[0]
        if prefix_id == sentinel_key or row_index >= row_count:
            continue
        if prefix_id in key_to_index and key_to_index[prefix_id] != row_index:
            duplicate_keys[prefix_id] += 1
            continue
        key_to_index[prefix_id] = row_index

    rows = {
        prefix_id: PrefixRow(
            prefix_id=prefix_id,
            row_index=row_index,
            row_address=row_base + row_index * PREFIX_ROW_SIZE,
            row=reader.read(row_base + row_index * PREFIX_ROW_SIZE, PREFIX_ROW_SIZE),
        )
        for prefix_id, row_index in sorted(key_to_index.items())
    }
    metadata = {
        "context": context,
        "hash_object": hash_object,
        "row_store": row_store,
        "hash_entry_count": entry_count,
        "row_count": row_count,
        "enumerated_prefix_count": len(rows),
        "duplicate_key_count": len(duplicate_keys),
        "sentinel_key": sentinel_key,
    }
    return rows, metadata


def discover_text_id_offset(
    rows: Iterable[EffectRow],
    prefix_rows: dict[int, PrefixRow],
    *,
    override: int | None = None,
) -> tuple[int, dict[str, list[int]]]:
    if override is not None:
        if not 0 <= override <= PREFIX_ROW_SIZE - 4:
            raise ValueError("text-ID offset is outside the 0x70 effect-prefix row")
        return override, {"override": [override]}

    row_by_id = {row.effect_id: row for row in rows}
    missing = [effect_id for effect_id in TEXT_ID_ANCHORS if effect_id not in row_by_id]
    if missing:
        raise RuntimeError(
            "effect table is missing required text-ID anchor rows: "
            + ", ".join(f"0x{item:08X}" for item in missing)
        )

    per_anchor: dict[str, list[int]] = {}
    intersection: set[int] | None = None
    for effect_id, text_id in TEXT_ID_ANCHORS.items():
        effect_row = row_by_id[effect_id].row
        prefix_id = struct.unpack_from("<H", effect_row, 2)[0]
        prefix_row = prefix_rows.get(prefix_id)
        if prefix_row is None:
            raise RuntimeError(
                f"effect 0x{effect_id:08X} references missing prefix 0x{prefix_id:04X}"
            )
        row = prefix_row.row
        offsets = [
            offset
            for offset in range(0, PREFIX_ROW_SIZE - 3)
            if struct.unpack_from("<I", row, offset)[0] == text_id
        ]
        per_anchor[f"0x{effect_id:08X}->0x{text_id:08X}"] = offsets
        current = set(offsets)
        intersection = current if intersection is None else intersection & current

    candidates = sorted(intersection or ())
    if len(candidates) != 1:
        raise RuntimeError(
            "could not identify one unique effect-row text-ID offset; "
            f"common candidates={candidates}, per-anchor={per_anchor}. "
            "Pass --text-id-offset only after manually validating the row layout."
        )
    return candidates[0], per_anchor


def _plausible_text(text: str) -> bool:
    if not text or len(text) > MAX_STRING_CODE_UNITS:
        return False
    controls = sum(ord(ch) < 0x20 and ch not in "\t\r\n" for ch in text)
    return controls == 0 and "\ufffd" not in text


def parse_text_entry(reader: ProcessReader, address: int, expected_id: int | None = None) -> TextEntry | None:
    header = reader.read(address, 8, exact=False)
    if len(header) != 8:
        return None
    text_id, code_units = struct.unpack("<II", header)
    if expected_id is not None and text_id != expected_id:
        return None
    if not 2 <= code_units <= MAX_STRING_CODE_UNITS:
        return None
    raw = reader.read(address + 8, code_units * 2, exact=False)
    if len(raw) != code_units * 2 or raw[-2:] != b"\x00\x00":
        return None
    try:
        text = raw[:-2].decode("utf-16-le")
    except UnicodeDecodeError:
        return None
    if not _plausible_text(text):
        return None
    return TextEntry(text_id, address, code_units, text)


def _find_pattern_offsets(block: bytes, pattern: bytes) -> Iterator[int]:
    cursor = 0
    while True:
        found = block.find(pattern, cursor)
        if found < 0:
            return
        yield found
        cursor = found + 1


def _candidate_regions(reader: ProcessReader) -> list[MemoryRegion]:
    regions = list(iter_readable_regions(reader.handle))
    # Native strings in the supplied capture lived in a large RW MEM_PRIVATE
    # allocation. Prioritize that shape, but retain every readable region for
    # a validated fallback.
    return sorted(
        regions,
        key=lambda region: (
            region.memory_type == MEM_PRIVATE,
            region.size >= 64 * 1024 * 1024,
            region.size,
            region.base,
        ),
        reverse=True,
    )


def scan_range_for_anchor_entries(
    reader: ProcessReader,
    *,
    start: int,
    end: int,
    anchor_ids: set[int],
    chunk_size: int = 8 * 1024 * 1024,
) -> list[TextEntry]:
    if end <= start:
        return []
    patterns = {text_id: struct.pack("<I", text_id) for text_id in anchor_ids}
    found_entries: dict[tuple[int, int], TextEntry] = {}
    cursor = start
    overlap = 7
    tail = b""
    while cursor < end:
        requested = min(chunk_size, end - cursor)
        block = reader.read(cursor, requested, exact=False)
        if not block:
            cursor += requested
            tail = b""
            continue
        haystack = tail + block
        base = cursor - len(tail)
        for text_id, pattern in patterns.items():
            for offset in _find_pattern_offsets(haystack, pattern):
                address = base + offset
                entry = parse_text_entry(reader, address, expected_id=text_id)
                if entry is not None:
                    found_entries[(entry.text_id, entry.address)] = entry
        tail = block[-overlap:]
        cursor += len(block)
        if len(block) < requested:
            cursor += requested - len(block)
    return list(found_entries.values())


def _best_anchor_cluster(entries: Iterable[TextEntry], radius: int) -> tuple[int, set[int]] | None:
    ordered = sorted(entries, key=lambda item: item.address)
    best: tuple[int, set[int]] | None = None
    left = 0
    counter: Counter[int] = Counter()
    for right, entry in enumerate(ordered):
        counter[entry.text_id] += 1
        while ordered[right].address - ordered[left].address > radius:
            counter[ordered[left].text_id] -= 1
            if counter[ordered[left].text_id] == 0:
                del counter[ordered[left].text_id]
            left += 1
        distinct = set(counter)
        center = (ordered[left].address + ordered[right].address) // 2
        if best is None or len(distinct) > len(best[1]):
            best = (center, distinct)
    return best


def locate_localization_pool(
    reader: ProcessReader,
    regions: list[MemoryRegion],
    *,
    discovery_window_mb: int,
    fallback_scan_mb: int,
) -> tuple[MemoryRegion, int, list[TextEntry], str]:
    anchor_ids = set(TEXT_ID_ANCHORS.values())
    discovery_radius = discovery_window_mb * 1024 * 1024

    # Fast, validated discovery near the relative location observed in the
    # supplied v2.00.02 capture. This is only a hint; at least two stable text
    # IDs must decode in one small cluster before it is accepted.
    for region in regions:
        if region.memory_type != MEM_PRIVATE or region.size < 256 * 1024 * 1024:
            continue
        hinted = region.allocation_base + LOCALIZATION_POOL_RELATIVE_HINT
        if not region.base <= hinted < region.base + region.size:
            continue
        start = max(region.base, hinted - discovery_radius)
        end = min(region.base + region.size, hinted + discovery_radius)
        entries = scan_range_for_anchor_entries(
            reader, start=start, end=end, anchor_ids=anchor_ids
        )
        cluster = _best_anchor_cluster(entries, radius=2 * 1024 * 1024)
        if cluster and len(cluster[1]) >= 2:
            return region, cluster[0], entries, "validated-relative-hint"

    # Fallback: search the high-address tail of large private regions first.
    # This avoids the original 20 GB whole-process string scan while remaining
    # self-validating. Increase --fallback-scan-mb if a different launch layout
    # places the pool farther from the end.
    fallback_size = fallback_scan_mb * 1024 * 1024
    for region in regions:
        if region.memory_type != MEM_PRIVATE or region.size < 64 * 1024 * 1024:
            continue
        start = max(region.base, region.base + region.size - fallback_size)
        end = region.base + region.size
        entries = scan_range_for_anchor_entries(
            reader, start=start, end=end, anchor_ids=anchor_ids
        )
        cluster = _best_anchor_cluster(entries, radius=2 * 1024 * 1024)
        if cluster and len(cluster[1]) >= 2:
            return region, cluster[0], entries, "validated-high-tail-fallback"

    raise RuntimeError(
        "could not locate a localization-pool cluster containing at least two stable text IDs. "
        "Retry with a larger --fallback-scan-mb, after waiting at the title screen."
    )


def scan_localization_window(
    reader: ProcessReader,
    *,
    region: MemoryRegion,
    center: int,
    wanted_text_ids: set[int],
    window_mb: int,
    chunk_size: int = 8 * 1024 * 1024,
) -> tuple[dict[int, list[TextEntry]], tuple[int, int]]:
    radius = window_mb * 1024 * 1024 // 2
    start = max(region.base, center - radius)
    end = min(region.base + region.size, center + radius)
    found: dict[int, dict[int, TextEntry]] = defaultdict(dict)
    cursor = start
    overlap = 2 * MAX_STRING_CODE_UNITS + 8
    tail = b""

    while cursor < end and len(found) < len(wanted_text_ids):
        requested = min(chunk_size, end - cursor)
        block = reader.read(cursor, requested, exact=False)
        if not block:
            cursor += requested
            tail = b""
            continue
        haystack = tail + block
        haystack_base = cursor - len(tail)
        # Entries are observed at absolute two-byte alignment, not necessarily
        # four-byte alignment. The scan window can begin at an odd address, so
        # alignment must be derived from the absolute haystack base instead of
        # assuming offsets 0 and 2 inside the current block.
        first_aligned = (-haystack_base) & 0x3
        alignments = sorted({first_aligned, (first_aligned + 2) & 0x3})
        for alignment in alignments:
            usable = len(haystack) - alignment
            usable -= usable % 4
            if usable <= 0:
                continue
            for index, (value,) in enumerate(
                struct.iter_unpack("<I", haystack[alignment : alignment + usable])
            ):
                if value not in wanted_text_ids:
                    continue
                address = haystack_base + alignment + index * 4
                entry = parse_text_entry(reader, address, expected_id=value)
                if entry is not None:
                    found[value][address] = entry
        tail = block[-overlap:]
        cursor += len(block)
        if len(block) < requested:
            cursor += requested - len(block)

    return {text_id: list(items.values()) for text_id, items in found.items()}, (start, end)


def choose_name(
    entries: list[TextEntry],
    *,
    pool_center: int | None = None,
    pool_radius: int = CURRENT_LANGUAGE_POOL_RADIUS,
) -> tuple[str | None, list[str]]:
    candidates = entries
    if pool_center is not None:
        candidates = [
            entry
            for entry in entries
            if abs(entry.address - pool_center) <= pool_radius
        ]
    if not candidates:
        return None, []
    counts = Counter(entry.text for entry in candidates)
    name, _ = counts.most_common(1)[0]
    conflicts = sorted(text for text in counts if text != name)
    return name, conflicts


def build_report(
    *,
    locale: str,
    output: Path,
    text_id_offset_override: int | None,
    window_mb: int,
    discovery_window_mb: int,
    fallback_scan_mb: int,
) -> dict[str, object]:
    locale = normalize_locale(locale)
    with ProcessReader() as reader:
        validate_version(reader)
        rows, table_meta = enumerate_effect_rows(reader)
        prefix_rows, prefix_table_meta = enumerate_prefix_rows(reader)
        text_id_offset, offset_evidence = discover_text_id_offset(
            rows, prefix_rows, override=text_id_offset_override
        )
        row_items: list[tuple[EffectRow, int, int]] = []
        for row in rows:
            prefix_id = struct.unpack_from("<H", row.row, 2)[0]
            prefix_row = prefix_rows.get(prefix_id)
            if prefix_row is None:
                continue
            text_id = struct.unpack_from("<I", prefix_row.row, text_id_offset)[0]
            if text_id not in (0, 0xFFFFFFFF):
                row_items.append((row, prefix_id, text_id))
        wanted_text_ids = {text_id for _, _, text_id in row_items}
        regions = _candidate_regions(reader)
        pool_region, pool_center, anchors, discovery_mode = locate_localization_pool(
            reader,
            regions,
            discovery_window_mb=discovery_window_mb,
            fallback_scan_mb=fallback_scan_mb,
        )
        text_entries, scanned_window = scan_localization_window(
            reader,
            region=pool_region,
            center=pool_center,
            wanted_text_ids=wanted_text_ids,
            window_mb=window_mb,
        )

        effects: dict[str, object] = {}
        resolved = 0
        ambiguous = 0
        for row, prefix_id, text_id in row_items:
            chosen, conflicts = choose_name(
                text_entries.get(text_id, []),
                pool_center=pool_center,
            )
            if chosen is not None:
                resolved += 1
            if conflicts:
                ambiguous += 1
            effects[f"0x{row.effect_id:08X}"] = {
                "effect_id": row.effect_id,
                "row_index": row.row_index,
                "prefix_id": f"0x{prefix_id:04X}",
                "text_id": f"0x{text_id:08X}",
                "name": chosen,
                "locale": locale,
                "provenance": "native_localization_pool",
                "conflicting_native_strings": conflicts,
                "entry_addresses_recorded": False,
            }

        report: dict[str, object] = {
            "schema": "nioh3-effect-catalog-locale-capture/v1",
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "game_version": GAME_VERSION,
            "locale": locale,
            "effect_table": {
                key: (f"0x{value:X}" if key in {"root", "context", "hash_object", "row_store", "sentinel_key"} else value)
                for key, value in table_meta.items()
            },
            "prefix_table": {
                key: (f"0x{value:X}" if key in {"context", "hash_object", "row_store", "sentinel_key"} else value)
                for key, value in prefix_table_meta.items()
            },
            "text_id_field": {
                "row_offset": f"0x{text_id_offset:X}",
                "anchor_evidence": offset_evidence,
            },
            "localization_pool": {
                "discovery_mode": discovery_mode,
                "accepted_anchor_text_ids": sorted(
                    {f"0x{entry.text_id:08X}" for entry in anchors}
                ),
                "region_addresses_recorded": False,
                "scanned_window_size": scanned_window[1] - scanned_window[0],
                "current_language_pool_radius": CURRENT_LANGUAGE_POOL_RADIUS,
            },
            "coverage": {
                "effect_rows_with_text_id": len(row_items),
                "resolved_names": resolved,
                "unresolved_names": len(row_items) - resolved,
                "ambiguous_text_ids": ambiguous,
            },
            "effects": effects,
            "limitations": [
                "This captures the currently loaded language only.",
                "Switch the game language at the title screen and rerun for each locale; restart only if anchor strings remain in the previous language.",
                "Rarity-4 stage-one slot-5 codes remain transient tokens even if the same numeric ID has a final-record name.",
                "Addresses are deliberately omitted; only versioned IDs and native strings are retained.",
            ],
        }
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return report


def parse_int(value: str) -> int:
    return int(value, 0)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Dump Nioh 3 v2.00.02 effect IDs, stable text IDs, and the currently loaded native language"
    )
    parser.add_argument("--locale", required=True, help="Label for the language currently loaded by the game, e.g. zh-CN, en-US, ja-JP")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--text-id-offset", type=parse_int)
    parser.add_argument("--window-mb", type=int, default=128, help="Localization window centered on the validated pool cluster")
    parser.add_argument("--discovery-window-mb", type=int, default=96)
    parser.add_argument("--fallback-scan-mb", type=int, default=2048)
    args = parser.parse_args()
    if args.window_mb <= 0 or args.discovery_window_mb <= 0 or args.fallback_scan_mb <= 0:
        parser.error("memory scan sizes must be positive")
    report = build_report(
        locale=args.locale,
        output=args.output,
        text_id_offset_override=args.text_id_offset,
        window_mb=args.window_mb,
        discovery_window_mb=args.discovery_window_mb,
        fallback_scan_mb=args.fallback_scan_mb,
    )
    print(json.dumps(report["coverage"], ensure_ascii=False))
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
