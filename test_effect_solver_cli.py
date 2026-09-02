from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

from nioh3_scroll_editor.seed_accelerator import cuda_seed_acceleration_available

from research.solve_effect_seed import (
    build_parser,
    candidate_batch_payload,
    load_auxiliary_name_indexes,
    load_effect_name_index,
    resolve_auxiliary_group,
    resolve_effect,
    resolve_effects,
)


class EffectSolverCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.index = load_effect_name_index()
        cls.auxiliary = load_auxiliary_name_indexes()

    def test_resolves_all_three_native_locales(self) -> None:
        expected = 0xAE5A
        self.assertEqual(resolve_effect("技之深奥", self.index), expected)
        self.assertEqual(resolve_effect("Ultimate Skill", self.index), expected)
        self.assertEqual(resolve_effect("技の深奥", self.index), expected)

    def test_resolves_numeric_ids(self) -> None:
        self.assertEqual(resolve_effect("0xAE5A", self.index), 0xAE5A)
        self.assertEqual(resolve_effect("44634", self.index), 0xAE5A)

    def test_resolves_repeated_conjunction_without_duplicates(self) -> None:
        self.assertEqual(
            resolve_effects(("技之深奥", "0xAE5A"), self.index),
            frozenset((0xAE5A,)),
        )

    def test_rejects_unknown_name(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown effect name"):
            resolve_effect("not-a-real-effect", self.index)

    def test_resolves_multikey_enemy_name_as_alternatives(self) -> None:
        self.assertEqual(
            resolve_auxiliary_group(
                "一目连",
                self.auxiliary.enemies,
                kind="enemy",
                max_value=0xFFFFFFFF,
            ),
            frozenset((0x0006DE91, 0x0008452A)),
        )

    def test_resolves_terrain_and_rule_raw_keys(self) -> None:
        self.assertEqual(
            resolve_auxiliary_group(
                "0x24", self.auxiliary.terrain, kind="terrain", max_value=0xFFFF
            ),
            frozenset((0x24,)),
        )
        self.assertEqual(
            resolve_auxiliary_group(
                "0x5132", self.auxiliary.rules, kind="rule", max_value=0xFFFF
            ),
            frozenset((0x5132,)),
        )

    def test_batch_payload_exposes_resume_cursor(self) -> None:
        payload = candidate_batch_payload(
            [
                {"seed": 10, "joint_search_trial": 12},
                {"seed": 20, "joint_search_trial": 34},
            ],
            verification="native-full-record",
            requested_results=2,
            resume_trial=0,
        )
        self.assertTrue(payload["found"])
        self.assertEqual(payload["result_count"], 2)
        self.assertEqual(payload["next_resume_trial"], 34)

    def test_parser_accepts_multiple_results(self) -> None:
        args = build_parser().parse_args(
            [
                "--playthrough",
                "3",
                "--grace",
                "0xBABD",
                "--max-results",
                "20",
            ]
        )
        self.assertEqual(args.max_results, 20)

    def test_ng3_rarity5_fixed_solver_needs_no_game_or_save(self) -> None:
        if not cuda_seed_acceleration_available():
            self.skipTest("no CUDA device for the fixed-draw product route")
        project_root = Path(__file__).resolve().parent
        child_environment = os.environ.copy()
        child_environment["PYTHONIOENCODING"] = "utf-8"
        completed = subprocess.run(
            [
                sys.executable,
                str(project_root / "research" / "solve_effect_seed.py"),
                "--fixed-only",
                "--playthrough",
                "3",
                "--rarity",
                "5",
                "--grace",
                "月读的恩宠",
                "--primary",
                "技之深奥",
                "--max-native-candidates",
                "5000",
            ],
            cwd=project_root,
            env=child_environment,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        payload = json.loads(completed.stdout)
        self.assertEqual(
            payload["verification"],
            "game-closed-ng3-r5-effect-sequence-and-auxiliary",
        )
        self.assertEqual(payload["effects"][0]["effect_id"], "0x0000AE5A")
        self.assertEqual(payload["effects"][5]["effect_id"], "0x0000BABD")
        self.assertEqual(len(bytes.fromhex(payload["effect_slots_hex"])), 7 * 0x18)


if __name__ == "__main__":
    unittest.main()
