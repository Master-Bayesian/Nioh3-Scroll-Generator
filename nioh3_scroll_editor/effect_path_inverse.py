"""Path-specific inverse plans for complete NG3 ordinary effect sets.

The native effect generator does not assign one permanent RNG draw to each
effect. Candidate pools change after every accepted effect, and a successful
promotion trial inserts a seven-draw shuffle before the lotteries. This module
therefore compiles every legal output order into exact high-16 LCG intervals.

The plans are acceleration hints only. Every returned Seed must still pass the
certified forward generator before it is shown or written.
"""

from __future__ import annotations

from dataclasses import dataclass
from itertools import permutations
from typing import Iterable, Sequence

from nioh3_seed_math import (
    LCG_INCREMENT,
    LCG_MULTIPLIER,
    MODULUS,
    UINT32_MASK,
    game_random_int_from_u16,
    is_natural_scroll_id,
)
from emaki_exchange import CATEGORY_TO_TYPE

from .effect_generation_tables import (
    EffectGenerationTableIndex,
    WeightedEffectCandidate,
    load_default_effect_generation_tables,
)
from .effect_sequence import (
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity4_stage_one_effect_sequence,
    generate_rarity5_grace_effect_sequence,
)
from .grace_map import (
    GraceOutputMap,
    first_u16_ranges_for_grace,
    load_grace_output_map,
)


NG3_RECORD_TYPE = 0xE604


@dataclass(frozen=True, slots=True)
class U16Run:
    start: int
    end: int

    def __post_init__(self) -> None:
        if not 0 <= self.start <= self.end <= 0xFFFF:
            raise ValueError("invalid uint16 run")

    @property
    def bucket_count(self) -> int:
        return self.end - self.start + 1


@dataclass(frozen=True, slots=True)
class LotteryConstraint:
    source_slot: int
    draw_index: int
    effect_id: int
    candidate_count: int
    total_weight: int
    allowed_u16: tuple[U16Run, ...]


@dataclass(frozen=True, slots=True)
class CompiledEffectPath:
    """One exact output order and promotion outcome."""

    ordered_effect_ids: tuple[int, ...]
    promoted_slot: int | None
    constraints: tuple[LotteryConstraint, ...]


@dataclass(frozen=True, slots=True)
class FullCompositionRequest:
    rarity: int
    primary_effect_id: int
    secondary_effect_ids: tuple[int, ...]
    stage_special_effect_id: int | None = None
    natural_only: bool = True
    playthrough: int = 3

    def __post_init__(self) -> None:
        if self.rarity not in (3, 4, 5):
            raise ValueError("complete scroll inversion supports rarity 3, 4, or 5")
        if self.playthrough not in (3, 4, 5):
            raise ValueError("effect inversion supports playthrough 3, 4, or 5")
        if self.rarity in (3, 4) and self.playthrough != 3:
            raise ValueError("rarity 3/4 generation is currently certified for NG3")
        ordinary = (self.primary_effect_id, *self.secondary_effect_ids)
        expected_ordinary_count = 5 if self.rarity == 5 else 4
        if len(ordinary) != expected_ordinary_count or len(set(ordinary)) != len(ordinary):
            raise ValueError(
                f"a complete rarity-{self.rarity} ordinary set requires "
                f"{expected_ordinary_count} distinct IDs"
            )
        if self.rarity == 3 and self.stage_special_effect_id not in (None, 0x0001):
            raise ValueError("rarity 3 uses the fixed 0x0001 growing token")
        if self.rarity == 4 and self.stage_special_effect_id is None:
            raise ValueError("rarity 4 stage-one inversion requires its draw-1 token")
        if self.rarity == 5 and self.stage_special_effect_id is None:
            raise ValueError("rarity 5 inversion requires its draw-1 Grace")
        if any(
            not 0 <= value <= UINT32_MASK
            for value in (
                *ordinary,
                *(
                    (self.stage_special_effect_id,)
                    if self.stage_special_effect_id is not None
                    else ()
                ),
            )
        ):
            raise ValueError("effect IDs must fit in uint32")


@dataclass(frozen=True, slots=True)
class CompiledEffectPlan:
    """A finite pivot preimage plus exact path predicates."""

    request: FullCompositionRequest
    promotion_draw_index: int
    promotion_probability_percent: int
    shuffle_draw_start: int
    slot_limit: int
    shared_constraints: tuple[tuple[int, tuple[U16Run, ...]], ...]
    paths: tuple[CompiledEffectPath, ...]
    pivot_draw_index: int
    pivot_allowed_u16: tuple[U16Run, ...]
    pivot_affine_addend: int
    pivot_inverse_multiplier: int

    @property
    def pivot_state_count(self) -> int:
        return sum(run.bucket_count for run in self.pivot_allowed_u16) * 0x10000


def lcg_affine_for_draw(draw_index: int) -> tuple[int, int]:
    """Return ``(m, c)`` for ``state_k = m * seed + c mod 2^32``."""

    if draw_index < 0:
        raise ValueError("draw_index cannot be negative")
    multiplier = 1
    addend = 0
    for _ in range(draw_index):
        multiplier = (LCG_MULTIPLIER * multiplier) & UINT32_MASK
        addend = (LCG_MULTIPLIER * addend + LCG_INCREMENT) & UINT32_MASK
    return multiplier, addend


def seed_from_state_at_draw(state: int, draw_index: int) -> int:
    multiplier, addend = lcg_affine_for_draw(draw_index)
    inverse = pow(multiplier, -1, MODULUS)
    return (inverse * ((state - addend) & UINT32_MASK)) & UINT32_MASK


def _winner_for_u16(
    candidates: Sequence[WeightedEffectCandidate],
    random_u16: int,
) -> int | None:
    positive = tuple(candidate for candidate in candidates if candidate.weight)
    if not positive:
        return None
    total = sum(candidate.weight for candidate in positive) & UINT32_MASK
    upper_count = (total + 1) & UINT32_MASK
    if upper_count == 0:
        raise OverflowError("native total+1 wrapped to zero")
    ticket = min(game_random_int_from_u16(random_u16, upper_count), total)
    for candidate in positive:
        weight = candidate.weight & UINT32_MASK
        if ticket <= weight:
            return candidate.effect.effect_id
        ticket = (ticket - weight) & UINT32_MASK
    return None


def _first_u16_with_random_int_at_least(count: int, target: int) -> int:
    if target <= 0:
        return 0
    if target >= count:
        return 0x10000
    low, high = 0, 0x10000
    while low < high:
        middle = (low + high) // 2
        if game_random_int_from_u16(middle, count) >= target:
            high = middle
        else:
            low = middle + 1
    return low


def weighted_lottery_u16_runs(
    candidates: Sequence[WeightedEffectCandidate],
    target_effect_id: int,
) -> tuple[U16Run, ...]:
    """Invert one exact inclusive weighted lottery into high-16 intervals."""

    positive = tuple(candidate for candidate in candidates if candidate.weight)
    if not positive:
        return ()
    total = sum(candidate.weight for candidate in positive) & UINT32_MASK
    upper_count = (total + 1) & UINT32_MASK
    if upper_count == 0:
        raise OverflowError("native total+1 wrapped to zero")

    ticket_start = 0
    target_interval: tuple[int, int] | None = None
    for index, candidate in enumerate(positive):
        weight = candidate.weight & UINT32_MASK
        ticket_end = weight if index == 0 else ticket_start + weight - 1
        if candidate.effect.effect_id == target_effect_id:
            target_interval = (ticket_start, min(ticket_end, total))
            break
        ticket_start = ticket_end + 1
    if target_interval is None:
        return ()
    minimum, maximum = target_interval
    start = _first_u16_with_random_int_at_least(upper_count, minimum)
    stop = _first_u16_with_random_int_at_least(upper_count, maximum + 1)
    if start >= stop or start >= 0x10000:
        return ()
    end = stop - 1
    if (
        _winner_for_u16(positive, start) != target_effect_id
        or _winner_for_u16(positive, end) != target_effect_id
    ):
        raise AssertionError("weighted-lottery inverse boundary mismatch")
    return (U16Run(start, end),)


def _compile_paths_for_promotion_slot(
    request: FullCompositionRequest,
    *,
    promoted_slot: int | None,
    tables: EffectGenerationTableIndex,
) -> tuple[CompiledEffectPath, ...]:
    special_id = request.stage_special_effect_id or 0x0001
    if request.rarity == 3:
        source_slots = (0, 1, 2, 3)
        lottery_start_draw = 9 if promoted_slot is not None else 2
    elif request.rarity == 4:
        source_slots = (1, 2, 3, 4)
        lottery_start_draw = 10 if promoted_slot is not None else 3
    else:
        source_slots = (1, 2, 3, 4, 5)
        lottery_start_draw = 10 if promoted_slot is not None else 3
    draw_indexes = tuple(
        lottery_start_draw + 3 * index for index in range(len(source_slots))
    )
    record_type = CATEGORY_TO_TYPE[request.playthrough]

    built: list[CompiledEffectPath] = []
    for secondary_order in permutations(request.secondary_effect_ids):
        ordered = (request.primary_effect_id, *secondary_order)
        capacities = list(
            tables.category_capacities(
                record_type=record_type,
                rarity=request.rarity,
            )
        )
        accepted: list[int] = []
        constraints: list[LotteryConstraint] = []
        for position, (source_slot, effect_id, draw_index) in enumerate(
            zip(source_slots, ordered, draw_indexes, strict=True)
        ):
            pool = tables.weighted_candidate_pool(
                record_type=record_type,
                rarity=request.rarity,
                playthrough=3,
                destination_category_and_flags=0x40 if position == 0 else 0,
                destination_effect_flags=0x04 if source_slot == promoted_slot else 0,
                remaining_category_capacities=capacities,
                existing_effect_ids=accepted,
                special_effect_id=special_id,
            )
            runs = weighted_lottery_u16_runs(pool, effect_id)
            if not runs:
                break
            constraints.append(
                LotteryConstraint(
                    source_slot=source_slot,
                    draw_index=draw_index,
                    effect_id=effect_id,
                    candidate_count=len(pool),
                    total_weight=sum(candidate.weight for candidate in pool)
                    & UINT32_MASK,
                    allowed_u16=runs,
                )
            )
            accepted.append(effect_id)
            category = tables.group_for_effect(effect_id).category_key
            if capacities[category] <= 0:
                break
            capacities[category] -= 1
        if len(constraints) == len(source_slots):
            built.append(
                CompiledEffectPath(
                    ordered_effect_ids=ordered,
                    promoted_slot=promoted_slot,
                    constraints=tuple(constraints),
                )
            )
    return tuple(built)


def _build_plan(
    request: FullCompositionRequest,
    *,
    paths: tuple[CompiledEffectPath, ...],
    shared_constraints: tuple[tuple[int, tuple[U16Run, ...]], ...],
    promotion_draw_index: int,
    promotion_probability_percent: int,
    shuffle_draw_start: int,
    slot_limit: int,
    pivot_draw_index: int,
    pivot_allowed_u16: tuple[U16Run, ...],
) -> CompiledEffectPlan:
    multiplier, addend = lcg_affine_for_draw(pivot_draw_index)
    return CompiledEffectPlan(
        request=request,
        promotion_draw_index=promotion_draw_index,
        promotion_probability_percent=promotion_probability_percent,
        shuffle_draw_start=shuffle_draw_start,
        slot_limit=slot_limit,
        shared_constraints=shared_constraints,
        paths=paths,
        pivot_draw_index=pivot_draw_index,
        pivot_allowed_u16=pivot_allowed_u16,
        pivot_affine_addend=addend,
        pivot_inverse_multiplier=pow(multiplier, -1, MODULUS),
    )


def compile_full_composition_plans(
    request: FullCompositionRequest,
    *,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> tuple[CompiledEffectPlan, ...]:
    """Compile every exact promotion path for one complete ordinary set."""

    if tables is None:
        tables = load_default_effect_generation_tables()
    definition = tables.rarity_generation[request.rarity]
    if definition.promotion_trials != 1:
        raise ValueError("path inversion requires the verified one-trial layout")
    probability = int(definition.promotion_probability_percent)

    if request.rarity == 3:
        plans: list[CompiledEffectPlan] = []
        for promoted_slot in (None, 0, 1, 2, 3):
            paths = _compile_paths_for_promotion_slot(
                request,
                promoted_slot=promoted_slot,
                tables=tables,
            )
            if not paths:
                continue
            primary_constraints = {
                (
                    path.constraints[0].draw_index,
                    tuple(
                        (run.start, run.end)
                        for run in path.constraints[0].allowed_u16
                    ),
                )
                for path in paths
            }
            if len(primary_constraints) != 1:
                raise AssertionError("rarity-3 primary inverse changed across orders")
            pivot_draw, raw_runs = next(iter(primary_constraints))
            plans.append(
                _build_plan(
                    request,
                    paths=paths,
                    shared_constraints=(),
                    promotion_draw_index=1,
                    promotion_probability_percent=probability,
                    shuffle_draw_start=2,
                    slot_limit=4,
                    pivot_draw_index=pivot_draw,
                    pivot_allowed_u16=tuple(U16Run(*run) for run in raw_runs),
                )
            )
        if not plans:
            raise ValueError("the complete rarity-3 composition has no legal path")
        return tuple(plans)

    if special_mapping is None:
        if request.playthrough != 3:
            raise ValueError(
                "NG4/NG5 inversion requires a captured matching-context Grace map"
            )
        special_mapping = load_grace_output_map(rarity=request.rarity)
    special_id = request.stage_special_effect_id
    assert special_id is not None
    special_runs = tuple(
        U16Run(entry.start, entry.end)
        for entry in first_u16_ranges_for_grace(special_id, special_mapping)
    )
    if not special_runs:
        raise ValueError("the requested rarity-4 stage token has no draw-1 preimage")
    promoted_slots = (
        (None, 1, 2, 3, 4, 5)
        if request.rarity == 5
        else (None, 1, 2, 3, 4)
    )
    paths = tuple(
        path
        for promoted_slot in promoted_slots
        for path in _compile_paths_for_promotion_slot(
            request,
            promoted_slot=promoted_slot,
            tables=tables,
        )
    )
    if not paths:
        raise ValueError(
            f"the complete rarity-{request.rarity} composition has no legal path"
        )
    pivot_draw = 1
    pivot_runs = special_runs
    primary_constraints = {
        (
            path.constraints[0].draw_index,
            tuple(
                (run.start, run.end)
                for run in path.constraints[0].allowed_u16
            ),
        )
        for path in paths
    }
    if len(primary_constraints) == 1:
        primary_draw, primary_raw_runs = next(iter(primary_constraints))
        primary_runs = tuple(U16Run(*run) for run in primary_raw_runs)
        if sum(run.bucket_count for run in primary_runs) < sum(
            run.bucket_count for run in special_runs
        ):
            pivot_draw = primary_draw
            pivot_runs = primary_runs
    return (
        _build_plan(
            request,
            paths=paths,
            shared_constraints=((1, special_runs),),
            promotion_draw_index=2,
            promotion_probability_percent=probability,
            shuffle_draw_start=3,
            slot_limit=6 if request.rarity == 5 else 5,
            pivot_draw_index=pivot_draw,
            pivot_allowed_u16=pivot_runs,
        ),
    )


def _u16_in_runs(value: int, runs: Sequence[U16Run]) -> bool:
    return any(run.start <= value <= run.end for run in runs)


def _promoted_slot_from_draws(
    draws: Sequence[int],
    *,
    rarity: int,
    slot_limit: int,
) -> int:
    if len(draws) != 7:
        raise ValueError("the promotion shuffle consumes exactly seven draws")
    order = list(range(7))
    for position, random_u16 in enumerate(draws):
        swap_index = game_random_int_from_u16(random_u16, 7)
        order[position], order[swap_index] = order[swap_index], order[position]
    for slot_index in order:
        if slot_index >= slot_limit:
            continue
        if rarity in (4, 5) and slot_index == 0:
            continue
        return slot_index
    raise AssertionError("promotion shuffle had no eligible slot")


def seed_satisfies_compiled_plan(plan: CompiledEffectPlan, seed: int) -> bool:
    """Evaluate the compiled interval predicates for one Seed."""

    if plan.request.natural_only and not is_natural_scroll_id(seed):
        return False
    max_draw = max(
        (
            constraint.draw_index
            for path in plan.paths
            for constraint in path.constraints
        ),
        default=plan.promotion_draw_index,
    )
    outputs = [0] * (max_draw + 1)
    state = seed & UINT32_MASK
    for draw in range(1, max_draw + 1):
        state = (LCG_MULTIPLIER * state + LCG_INCREMENT) & UINT32_MASK
        outputs[draw] = state >> 16
    if any(
        not _u16_in_runs(outputs[draw_index], runs)
        for draw_index, runs in plan.shared_constraints
    ):
        return False
    promoted = (
        game_random_int_from_u16(
            outputs[plan.promotion_draw_index],
            10_000,
        )
        < plan.promotion_probability_percent * 100
    )
    promoted_slot = (
        _promoted_slot_from_draws(
            outputs[plan.shuffle_draw_start : plan.shuffle_draw_start + 7],
            rarity=plan.request.rarity,
            slot_limit=plan.slot_limit,
        )
        if promoted
        else None
    )
    for path in plan.paths:
        if path.promoted_slot != promoted_slot:
            continue
        if all(
            _u16_in_runs(outputs[item.draw_index], item.allowed_u16)
            for item in path.constraints
        ):
            return True
    return False


def verify_complete_matches(
    request: FullCompositionRequest,
    seeds: Iterable[int],
) -> tuple[int, ...]:
    """Apply the certified forward generator as the final acceptance gate."""

    expected_secondaries = frozenset(request.secondary_effect_ids)
    verified: list[int] = []
    for seed in seeds:
        if request.natural_only and not is_natural_scroll_id(seed):
            continue
        if request.rarity == 3:
            result = generate_ng3_rarity3_effect_sequence(seed)
        elif request.rarity == 4:
            result = generate_ng3_rarity4_stage_one_effect_sequence(seed)
        else:
            result = generate_rarity5_grace_effect_sequence(
                seed,
                playthrough=request.playthrough,
            )
        if (
            result.primary.effect_id == request.primary_effect_id
            and frozenset(effect.effect_id for effect in result.secondaries)
            == expected_secondaries
            and (
                request.rarity == 3
                or result.grace.effect_id == request.stage_special_effect_id
            )
        ):
            verified.append(seed)
    return tuple(sorted(set(verified)))


__all__ = [
    "CompiledEffectPath",
    "CompiledEffectPlan",
    "FullCompositionRequest",
    "LotteryConstraint",
    "U16Run",
    "compile_full_composition_plans",
    "lcg_affine_for_draw",
    "seed_from_state_at_draw",
    "seed_satisfies_compiled_plan",
    "verify_complete_matches",
    "weighted_lottery_u16_runs",
]
