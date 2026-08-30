"""Exhaust the reported R4 rule/enemy/primary combination offline."""

from __future__ import annotations

import time
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from nioh3_scroll_editor.auxiliary_generation import AuxiliarySearchCriteria
from nioh3_scroll_editor.effect_seed_solver import (
    EffectSeedIntersectionReport,
    EffectSeedRequest,
    collect_effect_seed_page,
)
from nioh3_scroll_editor.effect_sequence import (
    collect_ng3_r4_primary_pivot_seeds,
    generate_ng3_certified_effect_sequence,
    generate_ng3_rarity34_primary_effect_ids,
)


def main() -> None:
    criteria = AuxiliarySearchCriteria(
        required_special_rule_keys=frozenset((0x2FEA, 0x7EF1)),
        required_enemy_lookup_keys=frozenset((0x40A3B,)),
        required_enemy_lookup_key_groups=(frozenset((0x202A7, 0xD35E1)),),
    )
    request = EffectSeedRequest(
        playthrough=3,
        rarity=4,
        primary_effect_ids=frozenset((0x774F,)),
        auxiliary_criteria=criteria,
    )
    started = time.perf_counter()
    last_reported_cursor = 0

    def progress(report: EffectSeedIntersectionReport) -> None:
        nonlocal last_reported_cursor
        if (
            report.inspected_through_trial - last_reported_cursor < 100_000_000
            and not report.exhausted_family
        ):
            return
        last_reported_cursor = report.inspected_through_trial
        print(
            {
                "cursor": report.inspected_through_trial,
                "family_size": report.family_size,
                "stage_counts": [stage.count for stage in report.stages],
                "complete_matches": report.complete_match_count,
                "elapsed_seconds": time.perf_counter() - started,
            },
            flush=True,
        )

    page = collect_effect_seed_page(
        request,
        page_size=1,
        effect_sequence_generator=lambda seed: generate_ng3_certified_effect_sequence(
            seed,
            rarity=4,
            level=180,
        ),
        primary_effect_id_batch_generator=lambda seeds: (
            generate_ng3_rarity34_primary_effect_ids(seeds, rarity=4)
        ),
        allow_full_seed_family=True,
        max_trials=0x1_0000_0000,
        intersection_progress=progress,
        pivot_seed_collector=(
            lambda values, start_index, stop_index, low16_stride: (
                collect_ng3_r4_primary_pivot_seeds(
                    values,
                    start_index=start_index,
                    stop_index=stop_index,
                    low16_stride=low16_stride,
                    primary_effect_ids=request.primary_effect_ids,
                )
            )
        ),
        pivot_seed_collector_chunk_trials=50_000_000,
    )
    print(
        {
            "candidate_seeds": [candidate.seed for candidate in page.candidates],
            "next_cursor": page.next_start_after_trial,
            "exhausted": bool(
                page.intersection_report
                and page.intersection_report.exhausted_family
            ),
            "elapsed_seconds": time.perf_counter() - started,
        },
        flush=True,
    )


if __name__ == "__main__":
    main()
