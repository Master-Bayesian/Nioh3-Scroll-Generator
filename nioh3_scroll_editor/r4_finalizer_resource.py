"""Pointer-free, versioned resources for the offline R4 finalizer.

The runtime capture contains process-specific addresses and table manager
contexts.  Those values are useful as capture provenance, but they must never
become application data.  This module extracts only deterministic table rows,
bonus-curve rows, playthrough progress vectors, mode-gate bytes, constants and
build identity hashes.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile
from typing import Any

from .r4_table_bundle import FixedStrideTable, R4FinalizerTableBundle


RESOURCE_SCHEMA = "nioh3-r4-finalizer-resource/v1"
DEFAULT_RESOURCE_ROOT = (
    Path(__file__).resolve().parent
    / "data"
    / "r4_finalizer"
    / "pc_v2_00_02"
    / "resource_v1"
)
REQUIRED_TABLES = (
    "item",
    "effect_group",
    "category",
    "category_count_multiplier",
    "level_curve",
    "effect",
    "optional_multiplier",
    "rarity_roll",
    "special_context",
)
BONUS_CURVE_ROW_SIZE = 0x58
INVALID_BONUS_CURVE_ROW = 0xFFFFFFFF


class ResourceIntegrityError(RuntimeError):
    """Raised when a derived resource is missing or fails integrity checks."""


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def _safe_relative_path(value: object) -> Path:
    relative = Path(str(value))
    if relative.is_absolute() or ".." in relative.parts:
        raise ResourceIntegrityError(f"unsafe resource path: {relative}")
    return relative


def _file_record(relative: str, data: bytes) -> dict[str, object]:
    return {
        "filename": relative.replace(os.sep, "/"),
        "size": len(data),
        "sha256": _sha256(data),
    }


def _write_file(root: Path, relative: str, data: bytes) -> dict[str, object]:
    path = root / _safe_relative_path(relative)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return _file_record(relative, data)


def _parse_pointer_candidate(value: object) -> int | None:
    if not isinstance(value, str) or not value.startswith("0x"):
        return None
    try:
        parsed = int(value, 16)
    except ValueError:
        return None
    if 0x10000 <= parsed < (1 << 47):
        return parsed
    return None


def _runtime_addresses(manifest: dict[str, Any]) -> set[int]:
    """Collect process-specific addresses so copied payloads can reject them."""

    result: set[int] = set()

    def visit(value: object, key: str = "") -> None:
        if isinstance(value, dict):
            for child_key, child_value in value.items():
                visit(child_value, str(child_key).lower())
            return
        if isinstance(value, list):
            for child in value:
                visit(child, key)
            return
        is_runtime_key = (
            key == "module_base"
            or key == "address"
            or key == "pointer"
            or key.endswith("_pointer")
            or key in {"pointers_begin", "pointers_end", "entries_begin", "entries_end"}
        )
        if is_runtime_key:
            parsed = _parse_pointer_candidate(value)
            if parsed is not None:
                result.add(parsed)

    visit(manifest)
    return result


def _reject_embedded_runtime_addresses(
    files: list[tuple[str, bytes]], runtime_addresses: set[int]
) -> None:
    for relative, data in files:
        for address in runtime_addresses:
            needle = struct.pack("<Q", address)
            if needle in data:
                raise ResourceIntegrityError(
                    f"{relative} still contains captured runtime address 0x{address:016X}"
                )


def _copy_code_identity(source_manifest: dict[str, Any]) -> dict[str, Any]:
    signatures: dict[str, Any] = {}
    for name, item in source_manifest.get("code_signatures", {}).items():
        signatures[str(name)] = {
            "rva": str(item["rva"]),
            "expected": str(item["expected"]),
            "actual": str(item["actual"]),
            "matches": bool(item["matches"]),
        }
    ranges: list[dict[str, Any]] = []
    for item in source_manifest.get("code", []):
        blob = item["blob"]
        ranges.append(
            {
                "name": str(item["name"]),
                "begin_rva": str(item["begin_rva"]),
                "end_rva": str(item["end_rva"]),
                "size": int(blob["size"]),
                "sha256": str(blob["sha256"]).upper(),
            }
        )
    return {"signatures": signatures, "ranges": ranges}


def _copy_float_constants(source_manifest: dict[str, Any]) -> dict[str, Any]:
    constants: dict[str, Any] = {}
    for name, item in source_manifest.get("float_constants", {}).items():
        constants[str(name)] = {
            "bits": str(item["bits"]),
            "value": float(item["value"]),
        }
    return constants


def _bonus_curve_payload(
    source: R4FinalizerTableBundle,
) -> tuple[bytes, bytes, int]:
    metadata = source.manifest["bonus_curve"]
    rows = list(metadata.get("rows", []))
    if len(rows) != int(metadata["pointer_count"]):
        raise ResourceIntegrityError("bonus-curve row count does not match capture")

    source_rows: dict[str, bytes] = {}
    ordered_names: list[str] = []
    for item in rows:
        blob = item.get("blob")
        if not blob:
            continue
        name = str(blob["filename"]).replace("\\", "/")
        if name in source_rows:
            raise ResourceIntegrityError(f"duplicate bonus-curve blob declaration: {name}")
        relative = _safe_relative_path(name)
        data = (source.root / relative).read_bytes()
        if len(data) != BONUS_CURVE_ROW_SIZE:
            raise ResourceIntegrityError(
                f"{name}: expected {BONUS_CURVE_ROW_SIZE:#x} bytes, got {len(data):#x}"
            )
        if int(item.get("key", -1)) != struct.unpack_from("<H", data, 0x50)[0]:
            raise ResourceIntegrityError(f"{name}: bonus-curve key mismatch")
        if int(item.get("sample_count", -1)) != struct.unpack_from("<H", data, 0x52)[0]:
            raise ResourceIntegrityError(f"{name}: bonus-curve sample count mismatch")
        source_rows[name] = data
        ordered_names.append(name)

    row_number = {name: index for index, name in enumerate(ordered_names)}
    index_values: list[int] = []
    for item in rows:
        if not bool(item.get("valid", False)):
            index_values.append(INVALID_BONUS_CURVE_ROW)
            continue
        if item.get("blob"):
            source_name = str(item["blob"]["filename"]).replace("\\", "/")
        elif item.get("duplicate_of"):
            source_name = str(item["duplicate_of"]).replace("\\", "/")
        else:
            raise ResourceIntegrityError("valid bonus-curve entry has no source row")
        try:
            index_values.append(row_number[source_name])
        except KeyError as error:
            raise ResourceIntegrityError(
                f"bonus-curve entry references unknown row: {source_name}"
            ) from error

    rows_payload = b"".join(source_rows[name] for name in ordered_names)
    index_payload = struct.pack(f"<{len(index_values)}I", *index_values)
    return rows_payload, index_payload, len(ordered_names)


def _playthrough_payload(source: R4FinalizerTableBundle) -> bytes:
    values: list[int] = []
    for selector in range(1, 6):
        values.extend(source.playthrough_progress(selector))
    return struct.pack("<20I", *values)


def build_r4_finalizer_resource(
    capture_root: str | Path,
    output_root: str | Path,
    *,
    source_locale: str = "unknown",
) -> Path:
    """Build a transactional, pointer-free resource from a verified capture."""

    capture = Path(capture_root)
    output = Path(output_root)
    if output.exists():
        raise FileExistsError(f"refusing to overwrite existing output: {output}")

    source = R4FinalizerTableBundle(capture, verify=True)
    source_manifest = source.manifest
    source_manifest_bytes = source.manifest_path.read_bytes()
    runtime_addresses = _runtime_addresses(source_manifest)

    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=f".{output.name}.tmp-", dir=str(output.parent))
    )
    try:
        file_records: list[dict[str, object]] = []
        payloads_for_address_check: list[tuple[str, bytes]] = []
        table_records: list[dict[str, Any]] = []

        source_table_metadata = {
            str(item["name"]): item for item in source_manifest.get("tables", [])
        }
        for name in REQUIRED_TABLES:
            table = source.table(name)
            relative = f"tables/{name}.bin"
            data = table.row_store
            file_meta = _write_file(staging, relative, data)
            file_records.append(file_meta)
            payloads_for_address_check.append((relative, data))
            table_records.append(
                {
                    "name": name,
                    "purpose": str(source_table_metadata[name].get("purpose", "")),
                    "row_size": table.row_size,
                    "row_count": table.row_count,
                    "file": file_meta,
                }
            )

        bonus_rows, bonus_index, unique_bonus_rows = _bonus_curve_payload(source)
        bonus_rows_meta = _write_file(staging, "bonus_curve/rows.bin", bonus_rows)
        bonus_index_meta = _write_file(staging, "bonus_curve/index.bin", bonus_index)
        file_records.extend((bonus_rows_meta, bonus_index_meta))
        payloads_for_address_check.extend(
            (("bonus_curve/rows.bin", bonus_rows), ("bonus_curve/index.bin", bonus_index))
        )

        progress = _playthrough_payload(source)
        progress_meta = _write_file(staging, "globals/playthrough_progress.bin", progress)
        file_records.append(progress_meta)
        payloads_for_address_check.append(("globals/playthrough_progress.bin", progress))

        _reject_embedded_runtime_addresses(payloads_for_address_check, runtime_addresses)

        manifest: dict[str, Any] = {
            "schema": RESOURCE_SCHEMA,
            "game_version": str(source_manifest.get("expected_game_version", "unknown")),
            "source": {
                "capture_schema": str(source_manifest.get("schema", "unknown")),
                "capture_manifest_sha256": _sha256(source_manifest_bytes),
                "captured_at_utc": str(source_manifest.get("captured_at_utc", "unknown")),
                "locale": source_locale,
                "effective_playthrough": source.effective_playthrough,
                "pe": dict(source_manifest.get("pe", {})),
            },
            "code_identity": _copy_code_identity(source_manifest),
            "float_constants": _copy_float_constants(source_manifest),
            "tables": table_records,
            "bonus_curve": {
                "row_size": BONUS_CURVE_ROW_SIZE,
                "entry_count": len(source_manifest["bonus_curve"]["rows"]),
                "unique_row_count": unique_bonus_rows,
                "invalid_row_index": INVALID_BONUS_CURVE_ROW,
                "rows_file": bonus_rows_meta,
                "index_file": bonus_index_meta,
            },
            "playthrough": {
                "selector_min": 1,
                "selector_max": 5,
                "values_per_selector": 4,
                "file": progress_meta,
            },
            "mode_gate_bytes": list(source.mode_gate_bytes() or ()),
            "files": file_records,
            "safety": {
                "process_specific_metadata_omitted": True,
                "runtime_references_omitted": True,
                "source_payloads_scanned": len(payloads_for_address_check),
            },
        }
        (staging / "manifest.json").write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        R4FinalizerResourceBundle(staging, verify=True)
        staging.rename(output)
        return output
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


@dataclass(frozen=True, slots=True)
class BonusCurveEntry:
    entry_index: int
    row_index: int
    row: bytes | None


class R4FinalizerResourceBundle:
    def __init__(self, root: str | Path, *, verify: bool = True):
        self.root = Path(root)
        self.manifest_path = self.root / "manifest.json"
        self.manifest: dict[str, Any] = json.loads(
            self.manifest_path.read_text(encoding="utf-8")
        )
        if self.manifest.get("schema") != RESOURCE_SCHEMA:
            raise ValueError(
                f"unsupported R4 finalizer resource schema: {self.manifest.get('schema')!r}"
            )
        self._tables = {
            str(item["name"]): item for item in self.manifest.get("tables", [])
        }
        if verify:
            self.verify_all_files()
            self._validate_shapes()

    def _read_declared_file(self, metadata: dict[str, Any]) -> bytes:
        relative = _safe_relative_path(metadata["filename"])
        return (self.root / relative).read_bytes()

    def verify_all_files(self) -> None:
        seen: set[str] = set()
        for item in self.manifest.get("files", []):
            relative = _safe_relative_path(item["filename"])
            key = str(relative).replace("\\", "/")
            if key in seen:
                raise ResourceIntegrityError(f"duplicate resource file: {key}")
            seen.add(key)
            path = self.root / relative
            if not path.is_file():
                raise ResourceIntegrityError(f"missing resource file: {key}")
            data = path.read_bytes()
            if len(data) != int(item["size"]):
                raise ResourceIntegrityError(f"{key}: resource file size mismatch")
            if _sha256(data) != str(item["sha256"]).upper():
                raise ResourceIntegrityError(f"{key}: resource file SHA-256 mismatch")

    def _validate_shapes(self) -> None:
        for name in REQUIRED_TABLES:
            self.table(name)
        bonus = self.manifest["bonus_curve"]
        rows = self._read_declared_file(bonus["rows_file"])
        index = self._read_declared_file(bonus["index_file"])
        if len(rows) != int(bonus["row_size"]) * int(bonus["unique_row_count"]):
            raise ResourceIntegrityError("bonus-curve row payload size mismatch")
        if len(index) != 4 * int(bonus["entry_count"]):
            raise ResourceIntegrityError("bonus-curve index payload size mismatch")
        progress = self._read_declared_file(self.manifest["playthrough"]["file"])
        expected_progress = (
            (int(self.manifest["playthrough"]["selector_max"]) - int(self.manifest["playthrough"]["selector_min"]) + 1)
            * int(self.manifest["playthrough"]["values_per_selector"])
            * 4
        )
        if len(progress) != expected_progress:
            raise ResourceIntegrityError("playthrough progress payload size mismatch")

    @lru_cache(maxsize=None)
    def table(self, name: str) -> FixedStrideTable:
        try:
            metadata = self._tables[name]
        except KeyError as error:
            raise KeyError(f"resource has no table named {name!r}") from error
        return FixedStrideTable(
            name=name,
            row_size=int(metadata["row_size"]),
            row_count=int(metadata["row_count"]),
            row_store=self._read_declared_file(metadata["file"]),
        )

    def bonus_curve_entry(self, entry_index: int) -> BonusCurveEntry:
        metadata = self.manifest["bonus_curve"]
        entry_count = int(metadata["entry_count"])
        if not 0 <= entry_index < entry_count:
            raise IndexError(entry_index)
        index_data = self._read_declared_file(metadata["index_file"])
        row_index = struct.unpack_from("<I", index_data, entry_index * 4)[0]
        invalid = int(metadata["invalid_row_index"])
        if row_index == invalid:
            return BonusCurveEntry(entry_index, row_index, None)
        unique_count = int(metadata["unique_row_count"])
        if row_index >= unique_count:
            raise ResourceIntegrityError(
                f"bonus-curve entry {entry_index} references row {row_index}, "
                f"but only {unique_count} rows exist"
            )
        row_size = int(metadata["row_size"])
        rows = self._read_declared_file(metadata["rows_file"])
        start = row_index * row_size
        return BonusCurveEntry(entry_index, row_index, rows[start : start + row_size])

    def playthrough_progress(self, selector: int) -> tuple[int, int, int, int]:
        metadata = self.manifest["playthrough"]
        minimum = int(metadata["selector_min"])
        maximum = int(metadata["selector_max"])
        if not minimum <= selector <= maximum:
            raise ValueError(f"playthrough selector must be in {minimum}..{maximum}")
        count = int(metadata["values_per_selector"])
        if count != 4:
            raise ResourceIntegrityError("unsupported playthrough vector width")
        data = self._read_declared_file(metadata["file"])
        offset = (selector - minimum) * count * 4
        return struct.unpack_from("<4I", data, offset)

    def mode_gate_bytes(self) -> tuple[int, int, int] | None:
        values = self.manifest.get("mode_gate_bytes", [])
        if not values:
            return None
        if len(values) != 3 or any(not 0 <= int(value) <= 0xFF for value in values):
            raise ResourceIntegrityError("invalid mode-gate byte vector")
        return tuple(int(value) for value in values)  # type: ignore[return-value]


@lru_cache(maxsize=2)
def load_default_r4_finalizer_resource(*, verify: bool = True) -> R4FinalizerResourceBundle:
    """Load the bundled PC v2.00.02 resource used by the offline engine."""

    return R4FinalizerResourceBundle(DEFAULT_RESOURCE_ROOT, verify=verify)


__all__ = [
    "BONUS_CURVE_ROW_SIZE",
    "BonusCurveEntry",
    "DEFAULT_RESOURCE_ROOT",
    "INVALID_BONUS_CURVE_ROW",
    "REQUIRED_TABLES",
    "RESOURCE_SCHEMA",
    "R4FinalizerResourceBundle",
    "ResourceIntegrityError",
    "build_r4_finalizer_resource",
    "load_default_r4_finalizer_resource",
]
