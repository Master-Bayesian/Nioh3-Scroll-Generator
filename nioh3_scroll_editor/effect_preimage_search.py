"""Exact GPU-assisted search over compiled complete-effect preimages."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

from .effect_path_inverse import (
    CompiledEffectPlan,
    FullCompositionRequest,
    OneWildcardCompositionRequest,
    compile_full_composition_plans,
    compile_one_wildcard_composition_plans,
    verify_complete_matches,
    verify_one_wildcard_matches,
)
from .effect_preimage_accelerator import collect_effect_preimage_matches_d3d11
from .grace_map import GraceOutputMap


@dataclass(frozen=True, slots=True)
class FullCompositionPreimageMatch:
    seed: int
    pivot_trial: int


@dataclass(frozen=True, slots=True)
class FullCompositionPreimageProgress:
    inspected_through_trial: int
    family_size: int
    verified_match_count: int


@dataclass(frozen=True, slots=True)
class FullCompositionPreimagePage:
    matches: tuple[FullCompositionPreimageMatch, ...]
    start_after_trial: int
    next_start_after_trial: int
    family_size: int
    exhausted_family: bool


ProgressCallback = Callable[[FullCompositionPreimageProgress], None]
CancellationCheck = Callable[[], bool]


def _plan_offsets(plans: tuple[CompiledEffectPlan, ...]) -> tuple[int, ...]:
    offsets: list[int] = []
    running = 0
    for plan in plans:
        offsets.append(running)
        running += plan.pivot_state_count
    return tuple(offsets)


def collect_full_composition_preimage_page(
    request: FullCompositionRequest,
    *,
    page_size: int,
    special_mapping: GraceOutputMap | None = None,
    start_after_trial: int = 0,
    max_trials: int | None = None,
    chunk_trials: int = 256_000_000,
    progress: ProgressCallback | None = None,
    cancelled: CancellationCheck | None = None,
) -> FullCompositionPreimagePage | None:
    """Search a complete composition or return ``None`` without D3D11.

    The cursor is one-based and counts every state in the concatenated plan
    families. GPU predicates are never accepted directly: each hit passes the
    certified forward generator before it is returned.
    """

    if page_size <= 0:
        raise ValueError("page_size must be positive")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    if max_trials is not None and max_trials <= 0:
        raise ValueError("max_trials must be positive when supplied")
    if chunk_trials <= 0:
        raise ValueError("chunk_trials must be positive")
    plans = compile_full_composition_plans(
        request,
        special_mapping=special_mapping,
    )
    offsets = _plan_offsets(plans)
    family_size = sum(plan.pivot_state_count for plan in plans)
    cursor = min(start_after_trial, family_size)
    stop_cursor = family_size
    if max_trials is not None:
        stop_cursor = min(stop_cursor, cursor + max_trials)
    matches: list[FullCompositionPreimageMatch] = []

    for plan, plan_offset in zip(plans, offsets, strict=True):
        plan_stop = plan_offset + plan.pivot_state_count
        if cursor >= plan_stop or plan_offset >= stop_cursor:
            continue
        local_start = max(0, cursor - plan_offset)
        local_limit = min(plan.pivot_state_count, stop_cursor - plan_offset)
        while local_start < local_limit:
            if cancelled is not None and cancelled():
                return FullCompositionPreimagePage(
                    matches=tuple(matches),
                    start_after_trial=start_after_trial,
                    next_start_after_trial=cursor,
                    family_size=family_size,
                    exhausted_family=cursor >= family_size,
                )
            local_stop = min(local_limit, local_start + chunk_trials)
            accelerated = collect_effect_preimage_matches_d3d11(
                plan,
                start_trial=local_start,
                stop_trial=local_stop,
                output_capacity=max(100_000, page_size * 8),
            )
            if accelerated is None:
                return None
            verified = set(
                verify_complete_matches(
                    request,
                    (seed for seed, _trial in accelerated),
                )
            )
            for seed, local_trial in accelerated:
                if seed not in verified:
                    continue
                global_trial = plan_offset + local_trial + 1
                matches.append(
                    FullCompositionPreimageMatch(seed, global_trial)
                )
                if len(matches) >= page_size:
                    cursor = global_trial
                    if progress is not None:
                        progress(
                            FullCompositionPreimageProgress(
                                cursor,
                                family_size,
                                len(matches),
                            )
                        )
                    return FullCompositionPreimagePage(
                        matches=tuple(matches),
                        start_after_trial=start_after_trial,
                        next_start_after_trial=cursor,
                        family_size=family_size,
                        exhausted_family=cursor >= family_size,
                    )
            cursor = plan_offset + local_stop
            local_start = local_stop
            if progress is not None:
                progress(
                    FullCompositionPreimageProgress(
                        cursor,
                        family_size,
                        len(matches),
                    )
                )

    return FullCompositionPreimagePage(
        matches=tuple(matches),
        start_after_trial=start_after_trial,
        next_start_after_trial=cursor,
        family_size=family_size,
        exhausted_family=cursor >= family_size,
    )


def collect_one_wildcard_composition_preimage_page(
    request: OneWildcardCompositionRequest,
    *,
    page_size: int,
    special_mapping: GraceOutputMap | None = None,
    start_after_trial: int = 0,
    max_trials: int | None = None,
    chunk_trials: int = 256_000_000,
    progress: ProgressCallback | None = None,
    cancelled: CancellationCheck | None = None,
) -> FullCompositionPreimagePage | None:
    """Search all legal completions of one unspecified ordinary effect."""

    if page_size <= 0:
        raise ValueError("page_size must be positive")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    if max_trials is not None and max_trials <= 0:
        raise ValueError("max_trials must be positive when supplied")
    if chunk_trials <= 0:
        raise ValueError("chunk_trials must be positive")
    plans = compile_one_wildcard_composition_plans(
        request,
        special_mapping=special_mapping,
    )
    offsets = _plan_offsets(plans)
    family_size = sum(plan.pivot_state_count for plan in plans)
    cursor = min(start_after_trial, family_size)
    stop_cursor = family_size
    if max_trials is not None:
        stop_cursor = min(stop_cursor, cursor + max_trials)
    matches: list[FullCompositionPreimageMatch] = []

    for plan, plan_offset in zip(plans, offsets, strict=True):
        plan_stop = plan_offset + plan.pivot_state_count
        if cursor >= plan_stop or plan_offset >= stop_cursor:
            continue
        local_start = max(0, cursor - plan_offset)
        local_limit = min(plan.pivot_state_count, stop_cursor - plan_offset)
        while local_start < local_limit:
            if cancelled is not None and cancelled():
                return FullCompositionPreimagePage(
                    matches=tuple(matches),
                    start_after_trial=start_after_trial,
                    next_start_after_trial=cursor,
                    family_size=family_size,
                    exhausted_family=cursor >= family_size,
                )
            local_stop = min(local_limit, local_start + chunk_trials)
            accelerated = collect_effect_preimage_matches_d3d11(
                plan,
                start_trial=local_start,
                stop_trial=local_stop,
                output_capacity=max(100_000, page_size * 8),
            )
            if accelerated is None:
                return None
            verified = set(
                verify_one_wildcard_matches(
                    request,
                    (seed for seed, _trial in accelerated),
                )
            )
            for seed, local_trial in accelerated:
                if seed not in verified:
                    continue
                global_trial = plan_offset + local_trial + 1
                matches.append(FullCompositionPreimageMatch(seed, global_trial))
                if len(matches) >= page_size:
                    cursor = global_trial
                    if progress is not None:
                        progress(
                            FullCompositionPreimageProgress(
                                cursor,
                                family_size,
                                len(matches),
                            )
                        )
                    return FullCompositionPreimagePage(
                        matches=tuple(matches),
                        start_after_trial=start_after_trial,
                        next_start_after_trial=cursor,
                        family_size=family_size,
                        exhausted_family=cursor >= family_size,
                    )
            cursor = plan_offset + local_stop
            local_start = local_stop
            if progress is not None:
                progress(
                    FullCompositionPreimageProgress(
                        cursor,
                        family_size,
                        len(matches),
                    )
                )

    return FullCompositionPreimagePage(
        matches=tuple(matches),
        start_after_trial=start_after_trial,
        next_start_after_trial=cursor,
        family_size=family_size,
        exhausted_family=cursor >= family_size,
    )


__all__ = [
    "FullCompositionPreimageMatch",
    "FullCompositionPreimagePage",
    "FullCompositionPreimageProgress",
    "collect_full_composition_preimage_page",
    "collect_one_wildcard_composition_preimage_page",
]
