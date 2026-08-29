"""Loader and integrity checker for dump_r4_finalizer_tables.py captures.

This module is deliberately format-focused.  It verifies every captured blob,
exposes versioned fixed-stride tables, and reconstructs the four-value
playthrough threshold vector used by the R4 finalizer.  It does not guess table
semantics that have not yet been recovered.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import struct
from typing import Any, Iterator

EXPECTED_SCHEMA = "nioh3-r4-finalizer-runtime-tables/v1"


def _u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


@dataclass(frozen=True, slots=True)
class FixedStrideTable:
    name: str
    row_size: int
    row_count: int
    row_store: bytes

    def __post_init__(self) -> None:
        expected = 8 + self.row_size * self.row_count
        if len(self.row_store) != expected:
            raise ValueError(
                f"{self.name}: row-store size mismatch: expected {expected:#x}, "
                f"got {len(self.row_store):#x}"
            )
        embedded_count = _u32(self.row_store, 4)
        if embedded_count != self.row_count:
            raise ValueError(
                f"{self.name}: manifest count {self.row_count} != embedded count "
                f"{embedded_count}"
            )

    def row(self, index: int) -> bytes:
        if not 0 <= index < self.row_count:
            raise IndexError(index)
        start = 8 + index * self.row_size
        return self.row_store[start : start + self.row_size]

    def rows(self) -> Iterator[bytes]:
        for index in range(self.row_count):
            yield self.row(index)

    def find_u16(self, value: int, *, offset: int = 0) -> list[int]:
        if not 0 <= offset <= self.row_size - 2:
            raise ValueError("u16 offset is outside the row")
        needle = value & 0xFFFF
        return [
            index
            for index, row in enumerate(self.rows())
            if struct.unpack_from("<H", row, offset)[0] == needle
        ]

    def find_u32(self, value: int, *, offset: int = 0) -> list[int]:
        if not 0 <= offset <= self.row_size - 4:
            raise ValueError("u32 offset is outside the row")
        needle = value & 0xFFFFFFFF
        return [
            index
            for index, row in enumerate(self.rows())
            if struct.unpack_from("<I", row, offset)[0] == needle
        ]


class CaptureIntegrityError(RuntimeError):
    pass


class R4FinalizerTableBundle:
    def __init__(self, root: str | Path, *, verify: bool = True):
        self.root = Path(root)
        self.manifest_path = self.root / "manifest.json"
        self.manifest: dict[str, Any] = json.loads(
            self.manifest_path.read_text(encoding="utf-8")
        )
        if self.manifest.get("schema") != EXPECTED_SCHEMA:
            raise ValueError(
                f"unsupported capture schema: {self.manifest.get('schema')!r}"
            )
        if verify:
            self.verify_all_blobs()
        self._tables = {
            str(item["name"]): item for item in self.manifest.get("tables", [])
        }

    def verify_all_blobs(self) -> None:
        for item in self.manifest.get("all_blobs", []):
            relative = Path(str(item["filename"]))
            path = self.root / relative
            if not path.is_file():
                raise CaptureIntegrityError(f"missing capture blob: {relative}")
            data = path.read_bytes()
            expected_size = int(item["size"])
            if len(data) != expected_size:
                raise CaptureIntegrityError(
                    f"{relative}: expected {expected_size:#x} bytes, got {len(data):#x}"
                )
            digest = hashlib.sha256(data).hexdigest().upper()
            expected_digest = str(item["sha256"]).upper()
            if digest != expected_digest:
                raise CaptureIntegrityError(
                    f"{relative}: SHA-256 mismatch: {digest} != {expected_digest}"
                )

    @lru_cache(maxsize=None)
    def table(self, name: str) -> FixedStrideTable:
        try:
            meta = self._tables[name]
        except KeyError as error:
            raise KeyError(f"capture has no table named {name!r}") from error
        rows_meta = meta["rows_blob"]
        data = (self.root / str(rows_meta["filename"])).read_bytes()
        return FixedStrideTable(
            name=name,
            row_size=int(meta["row_size"]),
            row_count=int(meta["row_count"]),
            row_store=data,
        )

    @property
    def effective_playthrough(self) -> int:
        return int(
            self.manifest["globals"]["playthrough_selector"]["effective_selector"]
        )

    def playthrough_progress(self, selector: int | None = None) -> tuple[int, int, int, int]:
        """Return the exact 4-int progress vector consumed by the weight gate.

        The first three values come from the captured playthrough-threshold
        object.  The fourth is the maximum of those three, matching RVA
        0x578CD4.  Selectors are 1..5; omitted means the captured effective one.
        """
        if selector is None:
            selector = self.effective_playthrough
        if not 1 <= selector <= 5:
            raise ValueError("playthrough selector must be in 1..5")
        blob_meta = self.manifest["globals"]["playthrough_selector"]["threshold_blob"]
        data = (self.root / str(blob_meta["filename"])).read_bytes()
        delta = 4 * (selector - 1)
        values = (
            _u32(data, 0x4C + delta),
            _u32(data, 0x6C + delta),
            _u32(data, 0x8C + delta),
        )
        return (*values, max(values))

    def mode_gate_bytes(self) -> tuple[int, int, int] | None:
        meta = self.manifest["globals"]["mode_context"].get("blob")
        if not meta:
            return None
        data = (self.root / str(meta["filename"])).read_bytes()
        if len(data) <= 0x40:
            raise CaptureIntegrityError("mode context blob is too short")
        return data[0x3E], data[0x3F], data[0x40]


__all__ = [
    "CaptureIntegrityError",
    "EXPECTED_SCHEMA",
    "FixedStrideTable",
    "R4FinalizerTableBundle",
]
