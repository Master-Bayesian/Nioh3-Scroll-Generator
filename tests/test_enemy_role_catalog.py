from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from tools.export_enemy_role_catalog import (
    DEFAULT_OUTPUT_ROOT,
    _sha256 as role_catalog_source_sha256,
    export,
)
from tools.export_enemy_combination_guide import (
    build_combination_payload,
    export as export_enemy_combinations,
)
from tools.export_knowledge_catalog_manifest import (
    DEFAULT_OUTPUT as DEFAULT_KNOWLEDGE_MANIFEST,
    _sha256 as knowledge_catalog_source_sha256,
    export as export_knowledge_manifest,
)
from nioh3_scroll_editor.auxiliary_feasibility import (
    viable_enemy_branch_classes,
)


class EnemyRoleCatalogTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.payload = json.loads(
            (DEFAULT_OUTPUT_ROOT / "enemy-roles.json").read_text(encoding="utf-8")
        )

    def test_native_candidate_table_is_complete(self) -> None:
        self.assertEqual(
            self.payload["schema"], "nioh3-scroll-enemy-role-catalog/v1"
        )
        self.assertEqual(len(self.payload["rows"]), 487)
        self.assertEqual(
            self.payload["role_counts"],
            {"0": 156, "1": 124, "2": 92, "3": 31, "4": 41, "5": 43},
        )
        self.assertEqual(
            self.payload["source"]["candidate_table"]["sha256"],
            "EFB0672B86A5F87D419D752E3B2FD75A51F5182CC1F809705C1627B0D4E9785D",
        )

    def test_text_source_fingerprints_ignore_platform_newlines(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            lf_path = root / "lf.json"
            crlf_path = root / "crlf.json"
            lf_path.write_bytes(b'{\n  "value": 1\n}\n')
            crlf_path.write_bytes(b'{\r\n  "value": 1\r\n}\r\n')

            self.assertEqual(
                role_catalog_source_sha256(lf_path),
                role_catalog_source_sha256(crlf_path),
            )
            self.assertEqual(
                knowledge_catalog_source_sha256(lf_path),
                knowledge_catalog_source_sha256(crlf_path),
            )

    def test_impossible_combination_roles_are_preserved(self) -> None:
        rows_by_key = {row["lookup_key"]: row for row in self.payload["rows"]}
        expected = {
            "0x0006DE91": (292, 1, "一目连"),
            "0x000F1A7F": (394, 5, "德川国松"),
            "0x00041A50": (451, 5, "德川庆喜"),
        }
        for key, (row_index, role, name) in expected.items():
            row = rows_by_key[key]
            self.assertEqual(row["row_index"], row_index)
            self.assertEqual(row["role"], role)
            self.assertEqual(row["names"]["zh-CN"], name)

    def test_display_name_is_not_treated_as_a_global_role(self) -> None:
        roles = {
            row["role"]
            for row in self.payload["rows"]
            if row["names"]["zh-CN"] == "金井半兵卫"
        }
        self.assertEqual(roles, {4, 5})

    def test_player_combination_catalog_has_all_display_identities(self) -> None:
        payload = build_combination_payload()
        self.assertEqual(
            payload["schema"], "nioh3-scroll-enemy-combination-catalog/v1"
        )
        self.assertEqual(payload["scope"]["recovered_branch_classes"], [0, 1, 2])
        self.assertEqual(len(payload["display_entries"]), 148)
        self.assertEqual(len(payload["unavailable_display_entries"]), 69)
        self.assertEqual(
            payload["scope"]["native_localization_display_entry_count"], 211
        )
        self.assertEqual(
            {
                family: details["display_entry_count"]
                for family, details in payload["family_definitions"].items()
            },
            {"O": 64, "A": 41, "B": 43, "A/B": 0},
        )
        self.assertEqual(
            sum(entry["candidate_count"] for entry in payload["display_entries"]),
            487,
        )
        variants = {
            entry["names"]["zh-CN"]: entry
            for entry in payload["display_entries"]
        }
        self.assertEqual(len(payload["display_entries"]), 148)
        self.assertEqual(
            variants["武田信玄（人形）"]["candidate_keys"],
            ["0x00071ED1"],
        )
        self.assertEqual(
            variants["比留呼（江户妖怪形态）"]["candidate_keys"],
            ["0x00093F79"],
        )
        self.assertEqual(
            variants["金井半兵卫（妖怪形态）"]["candidate_keys"],
            ["0x000179F7"],
        )
        yui = next(
            entry
            for entry in payload["display_entries"]
            if entry["names"]["zh-CN"] == "由井正雪"
        )
        self.assertEqual(yui["player_family"], "B")
        self.assertEqual(yui["enabled_playthroughs_union"], [3, 4, 5])

    def test_structural_classes_include_native_capacity_constraints(self) -> None:
        dedicated_costs = {
            row["cost"]
            for row in self.payload["rows"]
            if row["role"] in (4, 5)
        }
        self.assertEqual(dedicated_costs, {4.0})
        self.assertEqual(viable_enemy_branch_classes(({1}, {5})), (1,))
        self.assertEqual(viable_enemy_branch_classes(({1}, {5}, {5})), ())
        self.assertEqual(viable_enemy_branch_classes(({4}, {5})), (0,))
        self.assertEqual(viable_enemy_branch_classes(({4}, {4})), (0,))
        self.assertEqual(viable_enemy_branch_classes(({4}, {4}, {4})), ())
        self.assertEqual(viable_enemy_branch_classes(({5}, {5}, {5})), (0,))
        self.assertEqual(viable_enemy_branch_classes(({5}, {5}, {5}, {5})), ())
        self.assertEqual(viable_enemy_branch_classes(({0}, {2})), (1, 2))
        self.assertEqual(viable_enemy_branch_classes(({4, 5}, {1})), (1,))

    def test_export_is_deterministic_and_current(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            output_root = Path(temp_dir)
            export(output_root)
            for name in ("enemy-roles.json", "enemy-roles.csv", "enemy-roles.md"):
                self.assertEqual(
                    (output_root / name).read_bytes(),
                    (DEFAULT_OUTPUT_ROOT / name).read_bytes(),
                )

    def test_player_combination_export_is_deterministic_and_current(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            output_root = Path(temp_dir)
            export_enemy_combinations(output_root)
            for name in (
                "enemy-combinations.json",
                "enemy-combinations.csv",
                "enemy-combinations.md",
                "enemy-combinations.zh-CN.md",
                "enemy-unavailable.csv",
            ):
                self.assertEqual(
                    (output_root / name).read_bytes(),
                    (DEFAULT_OUTPUT_ROOT / name).read_bytes(),
                )

    def test_versioned_catalog_manifest_is_deterministic_and_current(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            output = Path(temp_dir) / "catalog-manifest.json"
            manifest = export_knowledge_manifest(output)
            self.assertEqual(manifest["game_version"], "PC v2.00.02")
            self.assertEqual(
                manifest["catalogs"]["final_effects"]["effect_count"], 3609
            )
            self.assertEqual(
                manifest["catalogs"]["enemy_combinations"]["display_entries"],
                148,
            )
            self.assertEqual(
                output.read_bytes(), DEFAULT_KNOWLEDGE_MANIFEST.read_bytes()
            )


if __name__ == "__main__":
    unittest.main()
