"""Contract regression for large special-rule family alternatives."""
from __future__ import annotations

import json
from pathlib import Path

import unittest

from nioh3_scroll_editor.worker_contracts import RequestError, validate_request


def search_request(rule_keys: list[int]) -> dict:
    return {
        "protocol": 1,
        "id": "rule-family-contract",
        "method": "search.start",
        "params": {
            "query": {
                "playthrough": 3,
                "rarity": 4,
                "level": 180,
                "primary_effect_ids": [],
                "required_secondary_ids": [],
                "required_secondary_id_groups": [],
                "grace_effect_id": None,
                "minimum_roll_percent_by_effect_id": [],
                "auxiliary": {
                    "required_terrain_effect_keys": [],
                    "required_terrain_effect_key_groups": [],
                    "required_special_rule_keys": [],
                    "required_special_rule_key_groups": [rule_keys],
                    "required_enemy_lookup_keys": [],
                    "required_enemy_lookup_key_groups": [],
                },
            },
            "context_digest": "0" * 64,
            "result_count": 25,
            "page_trials": 1_000_000,
            "job_trials": 10_000_000,
            "allow_cpu_fallback": False,
            "resume_token": None,
        },
    }


class RuleFamilyContractTests(unittest.TestCase):
    def test_complete_one_difficulty_family_fits_the_search_contract(self) -> None:
        catalog = json.loads(
            (Path(__file__).parents[1] / "apps/workshop/catalog.json").read_text(
                encoding="utf-8"
            )
        )
        keys = sorted(
            {
                key
                for rule in catalog["rules"]
                if rule["category"] == "一难横行"
                for key in rule["keys"]
            }
        )
        self.assertEqual(len(keys), 69)
        validate_request(search_request(keys))

    def test_special_rule_family_contract_remains_bounded(self) -> None:
        with self.assertRaises(RequestError):
            validate_request(search_request(list(range(257))))
