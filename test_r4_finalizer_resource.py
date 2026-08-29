from __future__ import annotations

import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

from nioh3_scroll_editor.r4_capture_validation import (
    validate_r4_capture_against_resource,
)
from nioh3_scroll_editor.r4_finalizer_resource import (
    DEFAULT_RESOURCE_ROOT,
    INVALID_BONUS_CURVE_ROW,
    REQUIRED_TABLES,
    R4FinalizerResourceBundle,
    ResourceIntegrityError,
    build_r4_finalizer_resource,
    load_default_r4_finalizer_resource,
)


def write_blob(root: Path, relative: str, data: bytes, address: int) -> dict[str, object]:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return {
        "filename": relative,
        "address": f"0x{address:016X}",
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest().upper(),
    }


class R4FinalizerResourceTests(unittest.TestCase):
    def make_capture(self, root: Path) -> None:
        all_blobs: list[dict[str, object]] = []
        tables: list[dict[str, object]] = []
        next_address = 0x0000000200000000
        for ordinal, name in enumerate(REQUIRED_TABLES):
            row = struct.pack("<I", 0x1000 + ordinal)
            store = struct.pack("<II", 0, 1) + row
            metadata = write_blob(
                root, f"tables/{name}/rows.bin", store, next_address
            )
            next_address += 0x1000
            all_blobs.append(metadata)
            tables.append(
                {
                    "name": name,
                    "purpose": f"test {name}",
                    "row_size": 4,
                    "row_count": 1,
                    "rows_blob": metadata,
                }
            )

        bonus_a = bytearray(0x58)
        struct.pack_into("<HH", bonus_a, 0x50, 23, 18)
        bonus_b = bytearray(0x58)
        struct.pack_into("<HH", bonus_b, 0x50, 99, 7)
        bonus_a_meta = write_blob(
            root, "tables/bonus_curve/rows/row_00000.bin", bytes(bonus_a), next_address
        )
        next_address += 0x1000
        bonus_b_meta = write_blob(
            root, "tables/bonus_curve/rows/row_00001.bin", bytes(bonus_b), next_address
        )
        all_blobs.extend((bonus_a_meta, bonus_b_meta))

        thresholds = bytearray(0xB0)
        for selector in range(1, 6):
            delta = 4 * (selector - 1)
            struct.pack_into("<I", thresholds, 0x4C + delta, 100 * selector)
            struct.pack_into("<I", thresholds, 0x6C + delta, 200 * selector)
            struct.pack_into("<I", thresholds, 0x8C + delta, 300 * selector)
        thresholds_meta = write_blob(
            root, "globals/playthrough_thresholds.bin", bytes(thresholds), next_address
        )
        next_address += 0x1000
        mode = bytearray(0x100)
        mode[0x3E:0x41] = bytes((1, 2, 3))
        mode_meta = write_blob(root, "globals/mode_context.bin", bytes(mode), next_address)
        all_blobs.extend((thresholds_meta, mode_meta))

        code_blob = write_blob(root, "code/finalizer.bin", b"CODE", next_address + 0x1000)
        all_blobs.append(code_blob)
        manifest = {
            "schema": "nioh3-r4-finalizer-runtime-tables/v1",
            "expected_game_version": "PC v2.00.02",
            "captured_at_utc": "2026-08-27T00:00:00+00:00",
            "pid": 1234,
            "module_base": "0x00007FF700000000",
            "pe": {"timestamp": "0x12345678", "size_of_image": 100},
            "code_signatures": {
                "finalizer": {
                    "rva": "0x1109270",
                    "expected": "AA BB",
                    "actual": "AA BB",
                    "matches": True,
                }
            },
            "code": [
                {
                    "name": "finalizer",
                    "begin_rva": "0x1109270",
                    "end_rva": "0x1109274",
                    "blob": code_blob,
                }
            ],
            "tables": tables,
            "bonus_curve": {
                "pointer_count": 4,
                "rows": [
                    {
                        "index": 0,
                        "valid": True,
                        "blob": bonus_a_meta,
                        "key": 23,
                        "sample_count": 18,
                    },
                    {
                        "index": 1,
                        "valid": True,
                        "duplicate_of": bonus_a_meta["filename"],
                    },
                    {"index": 2, "valid": False},
                    {
                        "index": 3,
                        "valid": True,
                        "blob": bonus_b_meta,
                        "key": 99,
                        "sample_count": 7,
                    },
                ],
            },
            "globals": {
                "playthrough_selector": {
                    "effective_selector": 3,
                    "threshold_blob": thresholds_meta,
                },
                "mode_context": {"blob": mode_meta},
            },
            "float_constants": {
                "one": {"rva": "0x10", "bits": "0x3F800000", "value": 1.0}
            },
            "all_blobs": all_blobs,
        }
        (root / "manifest.json").write_text(
            json.dumps(manifest, indent=2), encoding="utf-8"
        )

    def test_builds_pointer_free_resource_and_preserves_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = base / "capture"
            output = base / "resource"
            capture.mkdir()
            self.make_capture(capture)
            build_r4_finalizer_resource(capture, output, source_locale="zh-CN")

            resource = R4FinalizerResourceBundle(output)
            self.assertEqual(resource.manifest["source"]["locale"], "zh-CN")
            self.assertEqual(resource.table("effect").row(0), struct.pack("<I", 0x1005))
            self.assertEqual(resource.playthrough_progress(3), (300, 600, 900, 900))
            self.assertEqual(resource.mode_gate_bytes(), (1, 2, 3))
            self.assertEqual(resource.bonus_curve_entry(0).row_index, 0)
            self.assertEqual(resource.bonus_curve_entry(1).row_index, 0)
            self.assertIsNone(resource.bonus_curve_entry(2).row)
            self.assertEqual(resource.bonus_curve_entry(2).row_index, INVALID_BONUS_CURVE_ROW)
            self.assertEqual(resource.bonus_curve_entry(3).row_index, 1)

            manifest_text = (output / "manifest.json").read_text(encoding="utf-8").lower()
            for forbidden in ('"pid"', '"module_base"', '"address"', '"pointer"'):
                self.assertNotIn(forbidden, manifest_text)

    def test_refuses_existing_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = base / "capture"
            output = base / "resource"
            capture.mkdir()
            output.mkdir()
            self.make_capture(capture)
            with self.assertRaises(FileExistsError):
                build_r4_finalizer_resource(capture, output)

    def test_corrupt_source_fails_without_partial_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = base / "capture"
            output = base / "resource"
            capture.mkdir()
            self.make_capture(capture)
            path = capture / "tables/effect/rows.bin"
            path.write_bytes(path.read_bytes()[:-1] + b"\xFF")
            with self.assertRaises(Exception):
                build_r4_finalizer_resource(capture, output)
            self.assertFalse(output.exists())

    def test_resource_hash_mismatch_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = base / "capture"
            output = base / "resource"
            capture.mkdir()
            self.make_capture(capture)
            build_r4_finalizer_resource(capture, output)
            path = output / "tables/effect.bin"
            path.write_bytes(path.read_bytes()[:-1] + b"\xFF")
            with self.assertRaises(ResourceIntegrityError):
                R4FinalizerResourceBundle(output)

    def test_bundled_resource_is_complete(self) -> None:
        self.assertTrue(DEFAULT_RESOURCE_ROOT.is_dir())
        resource = load_default_r4_finalizer_resource()
        self.assertEqual(resource.manifest["game_version"], "PC v2.00.02")
        self.assertEqual(resource.manifest["source"]["locale"], "zh-CN")
        self.assertEqual(resource.manifest["source"]["effective_playthrough"], 3)
        self.assertEqual(resource.mode_gate_bytes(), (0, 0, 0))
        self.assertEqual(resource.playthrough_progress(3), (6510, 7710, 0, 7710))
        self.assertEqual(resource.table("effect").row_count, 3609)
        self.assertEqual(resource.manifest["bonus_curve"]["entry_count"], 662)

    def test_capture_stability_ignores_runtime_addresses(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            capture = base / "capture"
            resource_root = base / "resource"
            capture.mkdir()
            self.make_capture(capture)
            build_r4_finalizer_resource(capture, resource_root)

            manifest_path = capture / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["pid"] = 99999
            manifest["module_base"] = "0x00007FF712340000"
            for item in manifest["tables"]:
                item["rows_blob"]["address"] = "0x0000000312340000"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

            report = validate_r4_capture_against_resource(capture, resource_root)
            self.assertTrue(report["matches"], report["mismatches"])


if __name__ == "__main__":
    unittest.main()
