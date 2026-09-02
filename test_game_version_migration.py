from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import random
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).parent / "tools" / "prepare_game_version_update.py"
SPEC = importlib.util.spec_from_file_location("prepare_game_version_update", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def unique_bytes(size: int, seed: int) -> bytes:
    return random.Random(seed).randbytes(size)


def write_manifest(
    directory: Path,
    *,
    version: tuple[int, int, int, int],
    sections: dict[str, tuple[int, bytes]],
) -> Path:
    directory.mkdir(parents=True)
    records = []
    for name, (rva, data) in sections.items():
        filename = f"sample{name}.bin"
        (directory / filename).write_bytes(data)
        records.append(
            {
                "name": name,
                "rva": f"0x{rva:X}",
                "size": len(data),
                "filename": filename,
                "sha256": hashlib.sha256(data).hexdigest().upper(),
            }
        )
    manifest = directory / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema": "nioh3-live-pe-section-dump/v1",
                "file_version": list(version),
                "file_version_text": ".".join(str(part) for part in version),
                "sections": records,
            }
        ),
        encoding="utf-8",
    )
    return manifest


class GameVersionMigrationTests(unittest.TestCase):
    def test_report_and_candidate_profile_remain_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temp:
            root = Path(raw_temp)
            text_rva = 0x1000
            rdata_rva = 0x9000
            baseline_text = unique_bytes(4096, 7)
            baseline_rdata = unique_bytes(4096, 11)
            insertion = unique_bytes(64, 19)
            target_text = baseline_text[:800] + insertion + baseline_text[800:]
            target_rdata = baseline_rdata[:800] + insertion + baseline_rdata[800:]
            baseline_manifest = write_manifest(
                root / "baseline",
                version=(2, 0, 0, 2),
                sections={
                    ".text": (text_rva, baseline_text),
                    ".rdata": (rdata_rva, baseline_rdata),
                },
            )
            target_manifest = write_manifest(
                root / "target",
                version=(2, 0, 1, 0),
                sections={
                    ".text": (text_rva, target_text),
                    ".rdata": (rdata_rva, target_rdata),
                },
            )
            text_site = text_rva + 1800
            rdata_site = rdata_rva + 2200
            profile_path = root / "baseline-profile.json"
            profile_path.write_text(
                json.dumps(
                    {
                        "schema": MODULE.PROFILE_SCHEMA,
                        "profile_id": "baseline",
                        "display_version": "PC baseline",
                        "file_version": [2, 0, 0, 2],
                        "section_dump": {"manifest": str(baseline_manifest)},
                        "text_sites": {
                            "worker": {
                                "rva": f"0x{text_site:X}",
                                "signature_size": 8,
                                "range_size": 32,
                            }
                        },
                        "data_sites": {
                            "manager": {"rva": "0x1234"}
                        },
                        "rdata_sites": {
                            "scalar": {"rva": f"0x{rdata_site:X}", "size": 4}
                        },
                        "resources": {"test": "baseline-resource"},
                    }
                ),
                encoding="utf-8",
            )

            report = MODULE.build_report(
                profile_path,
                target_manifest,
                radius=384,
                anchor_size=16,
                stride=16,
            )
            self.assertTrue(report["gates"]["all_anchor_relocations_resolved"])
            self.assertFalse(report["gates"]["product_enablement_allowed"])
            report["output_path"] = str(root / "report.json")
            candidate = MODULE.build_candidate_profile(
                profile_path,
                target_manifest,
                report,
                display_version="PC target",
                data_site_overrides={"manager": 0x5678},
            )

            self.assertEqual(candidate["text_sites"]["worker"]["rva"], "0x1748")
            self.assertEqual(candidate["rdata_sites"]["scalar"]["rva"], "0x98D8")
            self.assertEqual(candidate["data_sites"]["manager"]["rva"], "0x5678")
            self.assertEqual(candidate["approval_status"], "candidate")
            self.assertFalse(candidate["product_enablement_allowed"])

    def test_explicit_override_is_visible_and_requires_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temp:
            root = Path(raw_temp)
            baseline = unique_bytes(2048, 23)
            target = unique_bytes(2048, 29)
            baseline_manifest = write_manifest(
                root / "baseline",
                version=(1, 0, 0, 0),
                sections={".text": (0x1000, baseline), ".rdata": (0x8000, baseline)},
            )
            target_manifest = write_manifest(
                root / "target",
                version=(1, 0, 0, 1),
                sections={".text": (0x1000, target), ".rdata": (0x8000, target)},
            )
            profile_path = root / "profile.json"
            profile_path.write_text(
                json.dumps(
                    {
                        "schema": MODULE.PROFILE_SCHEMA,
                        "profile_id": "baseline",
                        "display_version": "baseline",
                        "file_version": [1, 0, 0, 0],
                        "section_dump": {"manifest": str(baseline_manifest)},
                        "text_sites": {},
                        "data_sites": {},
                        "rdata_sites": {"value": {"rva": "0x8100", "size": 4}},
                        "resources": {},
                    }
                ),
                encoding="utf-8",
            )
            report = MODULE.build_report(profile_path, target_manifest)
            MODULE.apply_relocation_overrides(
                report,
                profile_path,
                target_manifest,
                {"text_sites": {}, "rdata_sites": {"value": 0x8200}},
            )
            item = report["relocations"]["rdata_sites"][0]
            self.assertEqual(item["status"], "resolved_by_explicit_override")
            self.assertTrue(item["override_requires_independent_evidence"])
            self.assertEqual(item["target_rva"], "0x8200")


if __name__ == "__main__":
    unittest.main()
