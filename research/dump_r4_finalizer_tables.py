from __future__ import annotations

"""Read-only, targeted runtime capture for a profiled Nioh 3 R4 finalizer.

The script does not write to the game process or to any save file.  It captures
only the parameter tables and small global contexts directly read by the
completion finalizer described by an explicit game-version research profile.
"""

import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
import shutil
import struct
import sys
import tempfile
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

try:
    from nioh3_scroll_editor.game_compatibility import _file_version
    from nioh3_scroll_editor.native import find_module_base, find_nioh3_pid
    from nioh3_scroll_editor.runtime_catalog_probe import (
        PROCESS_QUERY_INFORMATION,
        PROCESS_VM_READ,
        _kernel32,
        _read_process_memory,
    )
except ImportError as error:  # pragma: no cover - exercised on the user's PC
    raise RuntimeError(
        "Could not import the project source. Keep this script inside the "
        "repository research directory, or add the project root to PYTHONPATH."
    ) from error


PROFILE_SCHEMA = "nioh3-game-version-research-profile/v1"
DEFAULT_PROFILE = (
    PROJECT_ROOT
    / "nioh3_scroll_editor"
    / "data"
    / "game_versions"
    / "pc_v2_00_02.research.json"
)

PROCESS_READ_ACCESS = PROCESS_QUERY_INFORMATION | PROCESS_VM_READ
MAX_CAPTURE_BYTES = 128 * 1024 * 1024
MAX_ROWS = 1_000_000
MAX_POINTER_ROWS = 100_000


@dataclass(frozen=True, slots=True)
class TableSpec:
    name: str
    manager_offset: int
    row_size: int
    purpose: str


TABLE_SPECS: tuple[TableSpec, ...] = (
    TableSpec(
        "auxiliary_rule_conflict",
        0x020,
        0x018,
        "special-rule compatibility and conflict rows",
    ),
    TableSpec("item", 0x068, 0x1A0, "record-type/item rows"),
    TableSpec("effect_group", 0x0A8, 0x070, "prefix, category, text ID, conflicts"),
    TableSpec("category", 0x0B0, 0x06C, "category capacity and category lottery"),
    TableSpec("category_count_multiplier", 0x0B8, 0x020, "category-count multipliers"),
    TableSpec("level_curve", 0x0C0, 0x00A, "numeric effect level curves"),
    TableSpec("effect", 0x0C8, 0x0D8, "effect candidates, weights and formulas"),
    TableSpec("optional_multiplier", 0x230, 0x020, "keyed scalar/count multipliers"),
    TableSpec("rarity_roll", 0x9B0, 0x0F8, "numeric roll min/max by rarity"),
    TableSpec(
        "auxiliary_enemy_candidate",
        0xA80,
        0x01C,
        "ordered enemy-generation candidate rows",
    ),
    TableSpec(
        "auxiliary_terrain",
        0xA88,
        0x034,
        "terrain-generation rows and enemy terrain constraints",
    ),
    TableSpec("special_context", 0xA98, 0x030, "context rows used by special weight columns"),
    TableSpec(
        "scroll_special_rule",
        0xAA0,
        0x038,
        "ordered special-rule generation rows",
    ),
)


@dataclass(slots=True)
class BlobRecord:
    filename: str
    address: str
    size: int
    sha256: str


class ProcessReader:
    def __init__(self, pid: int):
        self.pid = pid
        self.dll = _kernel32()
        self.handle = self.dll.OpenProcess(PROCESS_READ_ACCESS, False, pid)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())

    def close(self) -> None:
        if self.handle:
            self.dll.CloseHandle(self.handle)
            self.handle = None

    def __enter__(self) -> "ProcessReader":
        return self

    def __exit__(self, exc_type, exc, traceback) -> None:
        self.close()

    def read(self, address: int, size: int) -> bytes:
        if size < 0 or size > MAX_CAPTURE_BYTES:
            raise ValueError(f"unsafe read size: {size:#x}")
        data = _read_process_memory(self.dll, self.handle, address, size)
        if len(data) != size:
            raise RuntimeError(
                f"ReadProcessMemory short read at 0x{address:016X}: "
                f"wanted {size:#x}, got {len(data):#x}"
            )
        return data

    def u8(self, address: int) -> int:
        return self.read(address, 1)[0]

    def u16(self, address: int) -> int:
        return struct.unpack("<H", self.read(address, 2))[0]

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.read(address, 4))[0]

    def u64(self, address: int) -> int:
        return struct.unpack("<Q", self.read(address, 8))[0]

    def executable_path(self) -> Path:
        capacity = wintypes.DWORD(32768)
        buffer = ctypes.create_unicode_buffer(capacity.value)
        query = self.dll.QueryFullProcessImageNameW
        query.argtypes = (
            wintypes.HANDLE,
            wintypes.DWORD,
            wintypes.LPWSTR,
            ctypes.POINTER(wintypes.DWORD),
        )
        query.restype = wintypes.BOOL
        if not query(self.handle, 0, buffer, ctypes.byref(capacity)):
            raise ctypes.WinError(ctypes.get_last_error())
        return Path(buffer.value).resolve()


class CaptureWriter:
    def __init__(self, output: Path):
        self.output = output
        self.output.mkdir(parents=True, exist_ok=False)
        self.blobs: list[BlobRecord] = []

    def blob(self, relative: str, address: int, data: bytes) -> BlobRecord:
        path = self.output / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        record = BlobRecord(
            filename=relative.replace(os.sep, "/"),
            address=f"0x{address:016X}",
            size=len(data),
            sha256=hashlib.sha256(data).hexdigest().upper(),
        )
        self.blobs.append(record)
        return record


class CaptureError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class RuntimeProfile:
    path: Path
    payload: dict[str, Any]
    display_version: str
    file_version: tuple[int, int, int, int]
    manifest_path: Path
    manifest: dict[str, Any]
    section_data: dict[str, tuple[int, bytes]]
    text_sites: dict[str, dict[str, Any]]
    data_sites: dict[str, dict[str, Any]]
    rdata_sites: dict[str, dict[str, Any]]


def parse_int(value: object) -> int:
    if isinstance(value, int):
        return value
    return int(str(value), 0)


def resolve_project_path(value: object) -> Path:
    path = Path(str(value))
    return path if path.is_absolute() else PROJECT_ROOT / path


def load_runtime_profile(path: Path) -> RuntimeProfile:
    path = path.resolve()
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("schema") != PROFILE_SCHEMA:
        raise ValueError(f"unsupported profile schema: {path}")
    file_version = tuple(int(part) for part in payload["file_version"])
    if len(file_version) != 4:
        raise ValueError("profile file_version must contain four integers")
    manifest_path = resolve_project_path(payload["section_dump"]["manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    raw_manifest_version = manifest.get("file_version")
    if raw_manifest_version is not None:
        manifest_version = tuple(int(part) for part in raw_manifest_version)
        if manifest_version != file_version:
            raise ValueError("profile and section manifest file versions differ")
    else:
        legacy_version = str(manifest.get("game_version", ""))
        expected_legacy = ".".join(str(part) for part in file_version)
        if legacy_version != expected_legacy:
            raise ValueError("profile and legacy section manifest versions differ")
    section_data: dict[str, tuple[int, bytes]] = {}
    for raw_section in manifest["sections"]:
        section_path = manifest_path.parent / raw_section["filename"]
        data = section_path.read_bytes()
        digest = hashlib.sha256(data).hexdigest().upper()
        if digest != str(raw_section["sha256"]).upper():
            raise ValueError(f"section hash mismatch: {section_path}")
        section_data[str(raw_section["name"])] = (
            parse_int(raw_section["rva"]),
            data,
        )
    for required in (".text", ".rdata"):
        if required not in section_data:
            raise ValueError(f"profile section dump is missing {required}")
    groups: dict[str, dict[str, dict[str, Any]]] = {}
    for group_name in ("text_sites", "data_sites", "rdata_sites"):
        group = payload.get(group_name, {})
        if not isinstance(group, dict):
            raise ValueError(f"profile {group_name} must be an object")
        groups[group_name] = group
    return RuntimeProfile(
        path=path,
        payload=payload,
        display_version=str(payload["display_version"]),
        file_version=file_version,
        manifest_path=manifest_path,
        manifest=manifest,
        section_data=section_data,
        text_sites=groups["text_sites"],
        data_sites=groups["data_sites"],
        rdata_sites=groups["rdata_sites"],
    )


def required_site_rva(
    profile: RuntimeProfile,
    group_name: str,
    name: str,
) -> int:
    group = getattr(profile, group_name)
    raw_site = group.get(name)
    if raw_site is None or raw_site.get("rva") is None:
        raise CaptureError(
            f"profile site {group_name}.{name} is unresolved; capture refused"
        )
    return parse_int(raw_site["rva"])


def is_probable_user_pointer(value: int) -> bool:
    return 0x10000 <= value < (1 << 47)


def checked_span(begin: int, end: int, *, alignment: int = 1, limit: int = MAX_CAPTURE_BYTES) -> int:
    if not is_probable_user_pointer(begin) or not is_probable_user_pointer(end):
        raise CaptureError(f"implausible pointer span: 0x{begin:X}..0x{end:X}")
    if end < begin:
        raise CaptureError(f"descending pointer span: 0x{begin:X}..0x{end:X}")
    size = end - begin
    if size > limit:
        raise CaptureError(f"pointer span exceeds limit: {size:#x}")
    if alignment > 1 and size % alignment:
        raise CaptureError(f"pointer span is not aligned to {alignment}: {size:#x}")
    return size


def parse_pe_identity(reader: ProcessReader, module_base: int) -> dict[str, Any]:
    dos = reader.read(module_base, 0x100)
    if dos[:2] != b"MZ":
        raise CaptureError("Nioh3.exe base does not contain an MZ header")
    e_lfanew = struct.unpack_from("<I", dos, 0x3C)[0]
    nt = reader.read(module_base + e_lfanew, 0x108)
    if nt[:4] != b"PE\0\0":
        raise CaptureError("invalid PE signature")
    machine, section_count, timestamp = struct.unpack_from("<HHI", nt, 4)
    optional_magic = struct.unpack_from("<H", nt, 0x18)[0]
    size_of_image = struct.unpack_from("<I", nt, 0x18 + 0x38)[0]
    return {
        "machine": f"0x{machine:04X}",
        "section_count": section_count,
        "timestamp": f"0x{timestamp:08X}",
        "optional_header_magic": f"0x{optional_magic:04X}",
        "size_of_image": size_of_image,
    }


def expected_site_signature(
    profile: RuntimeProfile,
    section_name: str,
    raw_site: dict[str, Any],
) -> bytes:
    captured = raw_site.get("captured_signature")
    if captured:
        return bytes.fromhex(str(captured))
    section_rva, section = profile.section_data[section_name]
    site_rva = parse_int(raw_site["rva"])
    size = parse_int(
        raw_site.get("signature_size", raw_site.get("size", 16))
    )
    offset = site_rva - section_rva
    expected = section[offset : offset + size]
    if len(expected) != size:
        raise CaptureError(
            f"profile site 0x{site_rva:X} is outside captured {section_name}"
        )
    return expected


def validate_code_signatures(
    reader: ProcessReader,
    module_base: int,
    profile: RuntimeProfile,
) -> dict[str, Any]:
    signatures: dict[str, tuple[int, bytes]] = {}
    for name, raw_site in profile.text_sites.items():
        if raw_site.get("rva") is None:
            raise CaptureError(f"profile text site {name} is unresolved")
        rva = parse_int(raw_site["rva"])
        signatures[name] = (
            rva,
            expected_site_signature(profile, ".text", raw_site),
        )
    results: dict[str, Any] = {}
    for name, (rva, expected) in signatures.items():
        actual = reader.read(module_base + rva, len(expected))
        results[name] = {
            "rva": f"0x{rva:X}",
            "expected": expected.hex(" ").upper(),
            "actual": actual.hex(" ").upper(),
            "matches": actual == expected,
        }
    if not all(item["matches"] for item in results.values()):
        raise CaptureError(
            f"Code signature mismatch for {profile.display_version}; "
            "do not capture or use tables from an unprofiled build."
        )
    return results


def capture_generic_table(
    reader: ProcessReader,
    writer: CaptureWriter,
    manager: int,
    spec: TableSpec,
) -> dict[str, Any]:
    context = reader.u64(manager + spec.manager_offset)
    if not is_probable_user_pointer(context):
        raise CaptureError(
            f"{spec.name}: invalid context pointer 0x{context:016X} "
            f"from manager+0x{spec.manager_offset:X}"
        )
    prefix = f"tables/{spec.name}"
    context_blob = writer.blob(
        f"{prefix}/context.bin", context, reader.read(context, 0x40)
    )

    row_store = reader.u64(context)
    if not is_probable_user_pointer(row_store):
        raise CaptureError(f"{spec.name}: invalid row-store pointer 0x{row_store:016X}")
    row_count = reader.u32(row_store + 4)
    if row_count > MAX_ROWS:
        raise CaptureError(f"{spec.name}: implausible row count {row_count}")
    row_bytes = row_count * spec.row_size
    if row_bytes > MAX_CAPTURE_BYTES - 8:
        raise CaptureError(f"{spec.name}: row bytes exceed capture limit")
    row_store_blob = writer.blob(
        f"{prefix}/rows.bin",
        row_store,
        reader.read(row_store, 8 + row_bytes),
    )

    hash_context = reader.u64(context + 0x20)
    hash_info: dict[str, Any] | None = None
    if is_probable_user_pointer(hash_context):
        hash_context_data = reader.read(hash_context, 0x20)
        hash_context_blob = writer.blob(
            f"{prefix}/hash_context.bin",
            hash_context,
            hash_context_data,
        )
        begin = struct.unpack_from("<Q", hash_context_data, 8)[0]
        end = struct.unpack_from("<Q", hash_context_data, 0x10)[0]
        try:
            entry_bytes = checked_span(
                begin, end, alignment=8, limit=MAX_CAPTURE_BYTES
            )
        except CaptureError as error:
            # Some auxiliary hash indices are initialized only after loading a
            # save. The fixed row store above remains complete and is the only
            # representation consumed by the offline resource.
            hash_info = {
                "available": False,
                "reason": str(error),
                "context_pointer": f"0x{hash_context:016X}",
                "context_blob": asdict(hash_context_blob),
            }
        else:
            entries_blob = writer.blob(
                f"{prefix}/hash_entries.bin", begin, reader.read(begin, entry_bytes)
            )
            hash_info = {
                "available": True,
                "context_pointer": f"0x{hash_context:016X}",
                "context_blob": asdict(hash_context_blob),
                "entries_begin": f"0x{begin:016X}",
                "entries_end": f"0x{end:016X}",
                "entry_count": entry_bytes // 8,
                "entries_blob": asdict(entries_blob),
            }

    return {
        "name": spec.name,
        "purpose": spec.purpose,
        "manager_offset": f"0x{spec.manager_offset:X}",
        "context_pointer": f"0x{context:016X}",
        "context_blob": asdict(context_blob),
        "row_store_pointer": f"0x{row_store:016X}",
        "row_size": spec.row_size,
        "row_count": row_count,
        "rows_blob": asdict(row_store_blob),
        "hash": hash_info,
    }


def capture_bonus_curve_table(
    reader: ProcessReader,
    writer: CaptureWriter,
    manager: int,
) -> dict[str, Any]:
    """Capture manager+0x718, whose rows are referenced by a sorted pointer vector."""
    obj = reader.u64(manager + 0x718)
    if not is_probable_user_pointer(obj):
        raise CaptureError(f"bonus curve object pointer is invalid: 0x{obj:016X}")
    obj_blob = writer.blob("tables/bonus_curve/object.bin", obj, reader.read(obj, 0x40))
    begin = reader.u64(obj + 0x18)
    end = reader.u64(obj + 0x20)
    vector_size = checked_span(begin, end, alignment=8, limit=MAX_CAPTURE_BYTES)
    pointer_count = vector_size // 8
    if pointer_count > MAX_POINTER_ROWS:
        raise CaptureError(f"bonus curve pointer count is implausible: {pointer_count}")
    pointer_bytes = reader.read(begin, vector_size)
    vector_blob = writer.blob("tables/bonus_curve/pointers.bin", begin, pointer_bytes)
    pointers = struct.unpack(f"<{pointer_count}Q", pointer_bytes) if pointer_count else ()
    unique_rows: dict[int, str] = {}
    row_records: list[dict[str, Any]] = []
    for index, pointer in enumerate(pointers):
        if not is_probable_user_pointer(pointer):
            row_records.append({"index": index, "pointer": f"0x{pointer:016X}", "valid": False})
            continue
        if pointer not in unique_rows:
            relative = f"tables/bonus_curve/rows/row_{len(unique_rows):05d}.bin"
            blob = writer.blob(relative, pointer, reader.read(pointer, 0x58))
            unique_rows[pointer] = relative
            row_records.append(
                {
                    "index": index,
                    "pointer": f"0x{pointer:016X}",
                    "valid": True,
                    "blob": asdict(blob),
                    "key": reader.u16(pointer + 0x50),
                    "sample_count": reader.u16(pointer + 0x52),
                }
            )
        else:
            row_records.append(
                {
                    "index": index,
                    "pointer": f"0x{pointer:016X}",
                    "valid": True,
                    "duplicate_of": unique_rows[pointer],
                }
            )
    return {
        "name": "bonus_curve",
        "manager_offset": "0x718",
        "purpose": "level/key interpolation used by RVA 0x5715FC",
        "object_pointer": f"0x{obj:016X}",
        "object_blob": asdict(obj_blob),
        "pointers_begin": f"0x{begin:016X}",
        "pointers_end": f"0x{end:016X}",
        "pointer_count": pointer_count,
        "pointer_blob": asdict(vector_blob),
        "rows": row_records,
    }


def capture_global_contexts(
    reader: ProcessReader,
    writer: CaptureWriter,
    module_base: int,
    profile: RuntimeProfile,
) -> dict[str, Any]:
    selector_pointer_rva = required_site_rva(
        profile, "data_sites", "playthrough_selector_pointer"
    )
    mode_pointer_rva = required_site_rva(profile, "data_sites", "mode_pointer")
    selector = reader.u64(module_base + selector_pointer_rva)
    if not is_probable_user_pointer(selector):
        raise CaptureError(f"invalid playthrough selector pointer: 0x{selector:016X}")
    selector_blob = writer.blob(
        "globals/playthrough_selector.bin", selector, reader.read(selector, 0x40)
    )
    thresholds = reader.u64(selector + 8)
    if not is_probable_user_pointer(thresholds):
        raise CaptureError(f"invalid playthrough threshold pointer: 0x{thresholds:016X}")
    threshold_blob = writer.blob(
        "globals/playthrough_thresholds.bin", thresholds, reader.read(thresholds, 0xB0)
    )

    mode = reader.u64(module_base + mode_pointer_rva)
    mode_info: dict[str, Any]
    if is_probable_user_pointer(mode):
        mode_blob = writer.blob("globals/mode_context.bin", mode, reader.read(mode, 0x100))
        mode_info = {"pointer": f"0x{mode:016X}", "blob": asdict(mode_blob)}
    else:
        mode_info = {"pointer": f"0x{mode:016X}", "blob": None}

    selector_value = reader.u8(selector)
    live_selector = reader.u8(thresholds + 0x10) if selector_value == 6 else selector_value
    return {
        "playthrough_selector": {
            "pointer_rva": f"0x{selector_pointer_rva:X}",
            "pointer": f"0x{selector:016X}",
            "selector_value": selector_value,
            "effective_selector": live_selector,
            "selector_blob": asdict(selector_blob),
            "threshold_pointer": f"0x{thresholds:016X}",
            "threshold_blob": asdict(threshold_blob),
        },
        "mode_context": {
            "pointer_rva": f"0x{mode_pointer_rva:X}",
            **mode_info,
        },
    }


def capture_code_and_constants(
    reader: ProcessReader,
    writer: CaptureWriter,
    module_base: int,
    profile: RuntimeProfile,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    code: list[dict[str, Any]] = []
    for name, raw_site in profile.text_sites.items():
        if "range_size" not in raw_site:
            continue
        begin = required_site_rva(profile, "text_sites", name)
        end = begin + parse_int(raw_site["range_size"])
        blob = writer.blob(
            f"code/{name}_{begin:08X}_{end:08X}.bin",
            module_base + begin,
            reader.read(module_base + begin, end - begin),
        )
        code.append({"name": name, "begin_rva": f"0x{begin:X}", "end_rva": f"0x{end:X}", "blob": asdict(blob)})

    constants: dict[str, Any] = {}
    raw = bytearray()
    first_constant_rva: int | None = None
    for name, raw_site in profile.rdata_sites.items():
        rva = required_site_rva(profile, "rdata_sites", name)
        if first_constant_rva is None:
            first_constant_rva = rva
        value_bytes = reader.read(module_base + rva, 4)
        value = struct.unpack("<f", value_bytes)[0]
        constants[name] = {"rva": f"0x{rva:X}", "value": value, "bits": f"0x{struct.unpack('<I', value_bytes)[0]:08X}"}
        raw.extend(value_bytes)
    if first_constant_rva is not None:
        writer.blob(
            "code/float_constants.bin",
            module_base + first_constant_rva,
            bytes(raw),
        )
    return code, constants


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Read-only targeted capture of a profiled Nioh 3 R4 finalizer"
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.output.exists():
        raise FileExistsError(f"refusing to overwrite existing output: {args.output}")
    profile = load_runtime_profile(args.profile)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    staging_parent = Path(
        tempfile.mkdtemp(prefix=f".{args.output.name}.tmp-", dir=str(args.output.parent))
    )
    staging_output = staging_parent / "capture"
    try:
        pid = find_nioh3_pid()
        module_base = find_module_base(pid)
        with ProcessReader(pid) as reader:
            pe = parse_pe_identity(reader, module_base)
            executable = reader.executable_path()
            live_file_version = _file_version(executable)
            if live_file_version != profile.file_version:
                raise CaptureError(
                    f"running game version {live_file_version} does not match "
                    f"profile {profile.file_version}"
                )
            executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest().upper()
            expected_executable = profile.manifest.get("executable", {})
            expected_sha256 = str(expected_executable.get("sha256", "")).upper()
            if expected_sha256 and executable_sha256 != expected_sha256:
                raise CaptureError(
                    "running executable hash does not match the profiled section dump"
                )
            signatures = validate_code_signatures(reader, module_base, profile)
            writer = CaptureWriter(staging_output)
            manager_pointer_rva = required_site_rva(
                profile, "data_sites", "parameter_manager_pointer"
            )
            manager = reader.u64(module_base + manager_pointer_rva)
            if not is_probable_user_pointer(manager):
                raise CaptureError(
                    f"invalid parameter manager pointer: 0x{manager:016X}"
                )
            manager_blob = writer.blob(
                "globals/parameter_manager.bin", manager, reader.read(manager, 0xB00)
            )

            tables = [
                capture_generic_table(reader, writer, manager, spec)
                for spec in TABLE_SPECS
            ]
            bonus_curve = capture_bonus_curve_table(reader, writer, manager)
            globals_report = capture_global_contexts(
                reader, writer, module_base, profile
            )
            code, constants = capture_code_and_constants(
                reader, writer, module_base, profile
            )

            manifest = {
                "schema": "nioh3-r4-finalizer-runtime-tables/v1",
                "expected_game_version": profile.display_version,
                "file_version": list(profile.file_version),
                "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                "pid": pid,
                "module_base": f"0x{module_base:016X}",
                "executable": {
                    "path": str(executable),
                    "size": executable.stat().st_size,
                    "sha256": executable_sha256,
                },
                "research_profile": {
                    "path": str(profile.path),
                    "profile_id": profile.payload["profile_id"],
                    "approval_status": profile.payload.get("approval_status", "baseline"),
                    "product_enablement_allowed": bool(
                        profile.payload.get("product_enablement_allowed", True)
                    ),
                    "section_manifest": str(profile.manifest_path),
                },
                "pe": pe,
                "code_signatures": signatures,
                "parameter_manager": {
                    "pointer_rva": f"0x{manager_pointer_rva:X}",
                    "pointer": f"0x{manager:016X}",
                    "blob": asdict(manager_blob),
                },
                "tables": tables,
                "bonus_curve": bonus_curve,
                "globals": globals_report,
                "code": code,
                "float_constants": constants,
                "all_blobs": [asdict(blob) for blob in writer.blobs],
                "safety": {
                    "process_access": "PROCESS_QUERY_INFORMATION | PROCESS_VM_READ",
                    "writes_to_game": False,
                    "writes_to_save": False,
                },
            }
            (writer.output / "manifest.json").write_text(
                json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
            )

        staging_output.rename(args.output)
        staging_parent.rmdir()
        print(
            json.dumps(
                {
                    "output": str(args.output.resolve()),
                    "table_count": len(TABLE_SPECS) + 1,
                    "blob_count": len(writer.blobs),
                    "effective_playthrough": manifest["globals"][
                        "playthrough_selector"
                    ]["effective_selector"],
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0
    except Exception:
        shutil.rmtree(staging_parent, ignore_errors=True)
        raise


if __name__ == "__main__":
    raise SystemExit(main())
