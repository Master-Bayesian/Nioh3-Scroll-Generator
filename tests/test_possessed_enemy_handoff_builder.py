from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest

from tools import build_possessed_enemy_assignment_handoff as builder
from tools import build_possessed_enemy_materials_handoff as retired_builder
from tools.validate_possessed_enemy_handoff import ValidationError, validate_forbidden_files


ROOT = Path(__file__).resolve().parents[1]
SPEC = (
    ROOT
    / "research"
    / "possessed_enemy_capture"
    / "handoff_specs"
    / "pc_v2.01_20260913_v1.json"
)


class PossessedEnemyHandoffBuilderTests(unittest.TestCase):
    def test_spec_names_an_immutable_curated_package(self) -> None:
        spec = json.loads(SPEC.read_text(encoding="utf-8"))
        self.assertEqual(
            spec["delivery_name"],
            "Nioh3_PC_v2.01_Possessed_Enemy_Assignment_Pro_Handoff_20260913_v1",
        )
        self.assertEqual(len(spec["control_runs"]), 5)
        self.assertEqual(len(set(spec["control_runs"])), 5)
        serialized = json.dumps(spec)
        self.assertNotIn(".codex_tmp", serialized)
        self.assertNotIn("20260910_v2", serialized)
        self.assertEqual(len(spec["runtime_section_hashes"]), 3)
        self.assertTrue(spec["private_analysis_only"])

    def test_task_requires_static_provenance_before_one_live_validation(self) -> None:
        self.assertIn("earliest causal source", builder.TASK)
        self.assertIn("owner-observed", builder.TASK)
        self.assertIn("one targeted live validation", builder.TASK)
        self.assertIn("Do not build a product forward oracle or inverse solver", builder.TASK)
        self.assertNotIn("repeat the selector experiment", builder.TASK)

    def test_retired_builder_cannot_overwrite_historical_package(self) -> None:
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            result = retired_builder.main()
        self.assertEqual(result, 2)
        self.assertIn("intentionally cannot", stderr.getvalue())
        self.assertIn("build_possessed_enemy_assignment_handoff.py", stderr.getvalue())

    def test_theme_validator_rejects_old_project_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "project").mkdir()
            (root / "project" / "stale.txt").write_text("stale", encoding="utf-8")
            with self.assertRaises(ValidationError):
                validate_forbidden_files(root)


if __name__ == "__main__":
    unittest.main()
