"""Permanent adapter parity gate for the M2.1 effect-resource loader.

This test loads the shipped tables through the real production adapter
(`crates/nioh3-data` `load_effect_resource`, exercised by
`examples/effect_resource_digest.rs`) and asserts content-level agreement with
values derived on the Python side from the same shipped files and the retained
reference module.

It fails if a regression swaps two tables of equal length, which the
size/count-only structural test cannot detect: per-table row digests are
compared in file order, so `effect` and `level_curve` can never be exchanged
silently.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import unittest

from nioh3_scroll_editor.effect_generation_tables import (
    load_default_effect_generation_tables,
)
from tests.migration.cargo_target import resolved_cargo_target_dir


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
RESOURCE = DATA_ROOT / "r4_finalizer" / "pc_v2_00_02" / "resource_v1"
MANIFEST = json.loads((RESOURCE / "manifest.json").read_text(encoding="utf-8"))
HEADER_BYTES = 8


def adapter_report() -> dict:
    """Run the production adapter emitter and return its JSON report."""

    target = resolved_cargo_target_dir("m2-effect-parity")
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(ROOT / "crates" / "nioh3-data" / "Cargo.toml"),
            "--example",
            "effect_resource_digest",
            "--",
            str(DATA_ROOT),
        ],
        cwd=ROOT,
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "effect_resource_digest failed: "
            + (completed.stderr.strip() or completed.stdout.strip())
        )
    return json.loads(completed.stdout)


class EffectResourceAdapterParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.report = adapter_report()

    def test_adapter_rows_match_the_reference_files_in_order(self) -> None:
        tables = {entry["name"]: entry for entry in MANIFEST["tables"]}
        # One owner for the constant list: the shipped manifest itself.
        self.assertEqual(len(tables), 9)

        covered_rows = 0
        for name, entry in tables.items():
            with self.subTest(table=name):
                raw = (RESOURCE / entry["file"]["filename"]).read_bytes()
                rows = raw[HEADER_BYTES:]
                digest = hashlib.sha256(rows).hexdigest().upper()
                reported = self.report["tables"][name]
                self.assertEqual(reported["row_size"], entry["row_size"])
                self.assertEqual(reported["row_count"], entry["row_count"])
                self.assertEqual(
                    reported["sha256"],
                    digest,
                    f"{name}: adapter rows differ from the shipped file",
                )
                covered_rows += entry["row_count"]

        # Explicit swap guard: the two tables that a field-order regression
        # exchanged must still be distinguishable by content.
        effect = self.report["tables"]["effect"]
        level_curve = self.report["tables"]["level_curve"]
        self.assertEqual((effect["row_size"], effect["row_count"]), (0xD8, 3609))
        self.assertEqual((level_curve["row_size"], level_curve["row_count"]), (10, 501))
        self.assertNotEqual(effect["sha256"], level_curve["sha256"])
        self.assertEqual(len(self.report["rows_hex"]["effect"]) // 2, 0xD8 * 3609)
        self.assertEqual(len(self.report["rows_hex"]["level_curve"]) // 2, 10 * 501)

        # Decoded-row reference link: the retained Python index must agree on
        # the number of rows the adapter exposes for the two tables in question.
        index = load_default_effect_generation_tables()
        self.assertEqual(len(index.effects_by_id), effect["row_count"])
        self.assertEqual(covered_rows, sum(entry["row_count"] for entry in tables.values()))

    def test_grace_metadata_label_is_preserved_verbatim(self) -> None:
        for map_report in self.report["grace_maps"]:
            self.assertEqual(map_report["capture_state"], "current-loaded-state")
            self.assertEqual(map_report["record_type"], 0xE604)


if __name__ == "__main__":
    unittest.main()
