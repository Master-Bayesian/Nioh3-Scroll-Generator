"""Explicit generation-table injection into the search collector and worker jobs.

The default remains the shipped PC v2.00.02 baseline; the v2.02 resource is
reached only when a caller injects it, which is what the worker bootstrap does
for a verified PC v2.02 installation.
"""

from __future__ import annotations

import struct
from pathlib import Path

from nioh3_scroll_editor.effect_generation_tables import (
    effect_generation_tables_for_game_version,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
from nioh3_scroll_editor.search_jobs import SearchJobs

V202 = (2, 0, 2, 0)


def test_generator_accepts_injected_tables() -> None:
    default = generate_ng3_certified_effect_sequence(10030700, rarity=4, level=180)
    injected = generate_ng3_certified_effect_sequence(
        10030700,
        rarity=4,
        level=180,
        tables=effect_generation_tables_for_game_version(V202),
    )
    assert type(injected) is type(default)
    assert injected.seed == default.seed


def test_injected_v202_tables_change_the_optional_multiplier_domain() -> None:
    shipped = load_default_effect_generation_tables()
    candidate = effect_generation_tables_for_game_version(V202)
    assert len(shipped.optional_multipliers_by_key) == 2951
    assert len(candidate.optional_multipliers_by_key) == 2954
    for key in (0x3472, 0xAA65, 0xD56F):
        assert key in candidate.optional_multipliers_by_key
        assert key not in shipped.optional_multipliers_by_key


def test_search_jobs_defaults_to_the_shipped_baseline() -> None:
    jobs = SearchJobs()
    assert jobs.generation_tables is None


def _parameters() -> dict:
    """The canonical valid ``search.start`` params used by the schema tests."""

    from nioh3_scroll_editor.core_services import CandidateApplicationService

    return {
        'query': {'playthrough': 3, 'rarity': 4, 'level': 180,
                  'primary_effect_ids': [0xAE5A], 'required_secondary_ids': [],
                  'required_secondary_id_groups': [], 'grace_effect_id': None,
                  'minimum_roll_percent_by_effect_id': [],
                  'auxiliary': {key: [] for key in (
                      'required_terrain_effect_keys', 'required_terrain_effect_key_groups',
                      'required_special_rule_keys', 'required_special_rule_key_groups',
                      'required_enemy_lookup_keys', 'required_enemy_lookup_key_groups')}},
        'context_digest': CandidateApplicationService().context.context_digest,
        'result_count': 1, 'page_trials': 1, 'job_trials': 1,
        'allow_cpu_fallback': False, 'resume_token': None,
    }


def test_search_jobs_carries_injected_v202_tables_to_the_collector() -> None:
    candidate = effect_generation_tables_for_game_version(V202)
    seen = {}

    def collector(request, **kwargs):
        seen.update(kwargs)
        raise RuntimeError('stop after capturing the collector arguments')

    jobs = SearchJobs(collector=collector, generation_tables=candidate)
    assert jobs.generation_tables is candidate

    jobs.start(_parameters())
    jobs.thread.join(timeout=30)
    assert seen.get('tables') is candidate
    assert jobs.job['state'] == 'failed'
