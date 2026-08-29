from __future__ import annotations

import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

from nioh3_scroll_editor.r4_table_bundle import (
    CaptureIntegrityError,
    R4FinalizerTableBundle,
)


def blob_record(root: Path, relative: str, data: bytes) -> dict[str, object]:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {
        "filename": relative,
        "address": "0x0000000000000000",
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest().upper(),
    }


class BundleTests(unittest.TestCase):
    def make_capture(self, root: Path) -> None:
        rows = struct.pack("<II", 0, 2) + bytes.fromhex("34120000 78560000")
        rows_meta = blob_record(root, "tables/effect/rows.bin", rows)
        thresholds = bytearray(0xB0)
        for selector in range(1, 6):
            delta = 4 * (selector - 1)
            struct.pack_into("<I", thresholds, 0x4C + delta, 1000 * selector)
            struct.pack_into("<I", thresholds, 0x6C + delta, 2000 * selector)
            struct.pack_into("<I", thresholds, 0x8C + delta, 3000 * selector)
        threshold_meta = blob_record(
            root, "globals/playthrough_thresholds.bin", bytes(thresholds)
        )
        mode = bytearray(0x100)
        mode[0x3E:0x41] = b"\x01\x02\x03"
        mode_meta = blob_record(root, "globals/mode_context.bin", bytes(mode))
        manifest = {
            "schema": "nioh3-r4-finalizer-runtime-tables/v1",
            "tables": [
                {
                    "name": "effect",
                    "row_size": 4,
                    "row_count": 2,
                    "rows_blob": rows_meta,
                }
            ],
            "globals": {
                "playthrough_selector": {
                    "effective_selector": 3,
                    "threshold_blob": threshold_meta,
                },
                "mode_context": {"blob": mode_meta},
            },
            "all_blobs": [rows_meta, threshold_meta, mode_meta],
        }
        (root / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")

    def test_load_verify_and_rows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.make_capture(root)
            bundle = R4FinalizerTableBundle(root)
            table = bundle.table("effect")
            self.assertEqual(table.row(0), bytes.fromhex("34120000"))
            self.assertEqual(table.find_u16(0x5678), [1])
            self.assertEqual(bundle.playthrough_progress(), (3000, 6000, 9000, 9000))
            self.assertEqual(bundle.mode_gate_bytes(), (1, 2, 3))

    def test_hash_mismatch_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.make_capture(root)
            path = root / "tables/effect/rows.bin"
            path.write_bytes(path.read_bytes()[:-1] + b"\xFF")
            with self.assertRaises(CaptureIntegrityError):
                R4FinalizerTableBundle(root)


if __name__ == "__main__":
    unittest.main()
