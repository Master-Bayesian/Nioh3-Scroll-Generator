"""Game-closed Seed construction for fixed-draw and replayed effects.

Grace constraints are exact first-draw inverse sets.  Historical conditioned
primary maps contain one representative per draw-2 high-16 bucket and are only
candidate prefilters: primary and secondary output must be checked by an exact
effect-sequence or final-record generator.  Without one, path-dependent
secondary requests fail closed.
"""

from __future__ import annotations

from dataclasses import dataclass
from itertools import islice
from typing import Callable, Iterator

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE

from .auxiliary_generation import (
    AuxiliarySearchCriteria,
    CompleteAuxiliaryResult,
    generate_complete_auxiliary,
    generate_matching_auxiliary,
)
from .grace_map import GraceOutputMap, first_u16_ranges_for_grace
from .effect_sequence import EffectSequenceResult, GeneratedEffect
from .joint_solver import (
    DrawConstraint,
    SeedSolution,
    U16Runs,
    choose_pivot,
    iter_constraint_intersection,
)
from .models import ScrollCandidate, candidate_matches
from .primary_map import PrimaryFirstDrawOutputMap, PrimaryOutputMap


class OfflineEffectReplayUnavailable(RuntimeError):
    """Raised when path-dependent effects were requested without a final generator."""


@dataclass(frozen=True, slots=True)
class EffectSeedRequest:
    playthrough: int
    rarity: int
    primary_effect_ids: frozenset[int] = frozenset()
    required_secondary_ids: frozenset[int] = frozenset()
    grace_effect_id: int | None = None
    auxiliary_criteria: AuxiliarySearchCriteria = AuxiliarySearchCriteria()
    natural_only: bool = True

    def __post_init__(self) -> None:
        if not 1 <= self.playthrough <= 5:
            raise ValueError("playthrough must be in 1..5")
        if not 0 <= self.rarity <= 5:
            raise ValueError("rarity must be in 0..5")
        effect_ids = self.primary_effect_ids | self.required_secondary_ids
        if self.grace_effect_id is not None:
            effect_ids |= frozenset((self.grace_effect_id,))
        if any(not 0 <= effect_id <= 0xFFFFFFFF for effect_id in effect_ids):
            raise ValueError("effect IDs must fit in uint32")


@dataclass(frozen=True, slots=True)
class EffectSeedCandidate:
    seed: int
    pivot_trial: int
    fixed_draws: tuple[tuple[str, int], ...]
    auxiliary: CompleteAuxiliaryResult | None = None
    record: ScrollCandidate | None = None
    effect_sequence: EffectSequenceResult | None = None


@dataclass(frozen=True, slots=True)
class IntersectionStageCount:
    """Cumulative survivors after one user-selected constraint."""

    kind: str
    values: tuple[int, ...]
    count: int


@dataclass(frozen=True, slots=True)
class EffectSeedIntersectionReport:
    """Exact cumulative counts for the inspected portion of a pivot family."""

    start_after_trial: int
    inspected_through_trial: int
    family_size: int
    fixed_seed_count: int
    stages: tuple[IntersectionStageCount, ...]
    complete_match_count: int
    exhausted_family: bool

    @property
    def is_global_total(self) -> bool:
        return self.start_after_trial == 0 and self.exhausted_family


@dataclass(frozen=True, slots=True)
class EffectSeedPage:
    """One deterministic page of exact Seed candidates.

    ``next_start_after_trial`` is an opaque cursor in the mathematical pivot
    family.  It is deliberately not a Seed increment and can be persisted by
    callers to request the next non-overlapping page.
    """

    candidates: tuple[EffectSeedCandidate, ...]
    start_after_trial: int
    next_start_after_trial: int
    intersection_report: EffectSeedIntersectionReport | None = None

    @property
    def is_empty(self) -> bool:
        return not self.candidates


FinalRecordGenerator = Callable[[int], bytes]
EffectSequenceGenerator = Callable[[int], EffectSequenceResult]
PrimaryEffectGenerator = Callable[[int], GeneratedEffect]
PrimaryEffectIdGenerator = Callable[[int], int]
PrimaryEffectIdBatchGenerator = Callable[[tuple[int, ...]], tuple[int, ...]]
IntersectionProgressCallback = Callable[[EffectSeedIntersectionReport], None]
CancellationCheck = Callable[[], bool]


def _iter_solution_primary_ids(
    solutions: Iterator[SeedSolution],
    batch_generator: PrimaryEffectIdBatchGenerator | None,
    *,
    batch_size: int = 16_384,
) -> Iterator[tuple[SeedSolution, int | None]]:
    """Attach prefetched primary IDs without materializing a whole family."""

    if batch_generator is None:
        for solution in solutions:
            yield solution, None
        return
    while True:
        batch = tuple(islice(solutions, batch_size))
        if not batch:
            return
        effect_ids = batch_generator(tuple(solution.seed for solution in batch))
        if len(effect_ids) != len(batch):
            raise ValueError("primary batch generator returned the wrong result count")
        yield from zip(batch, effect_ids, strict=True)


def merge_intersection_reports(
    reports: tuple[EffectSeedIntersectionReport, ...],
) -> EffectSeedIntersectionReport:
    """Merge adjacent exact ranges from the same ordered constraint pipeline."""

    if not reports:
        raise ValueError("at least one intersection report is required")
    first = reports[0]
    expected_start = first.start_after_trial
    stage_identity = tuple((stage.kind, stage.values) for stage in first.stages)
    for report in reports:
        if report.start_after_trial != expected_start:
            raise ValueError("intersection report ranges are not adjacent")
        if report.family_size != first.family_size:
            raise ValueError("intersection reports use different pivot families")
        if tuple((stage.kind, stage.values) for stage in report.stages) != stage_identity:
            raise ValueError("intersection reports use different constraint stages")
        expected_start = report.inspected_through_trial
    return EffectSeedIntersectionReport(
        start_after_trial=first.start_after_trial,
        inspected_through_trial=reports[-1].inspected_through_trial,
        family_size=first.family_size,
        fixed_seed_count=sum(report.fixed_seed_count for report in reports),
        stages=tuple(
            IntersectionStageCount(
                first.stages[index].kind,
                first.stages[index].values,
                sum(report.stages[index].count for report in reports),
            )
            for index in range(len(first.stages))
        ),
        complete_match_count=sum(report.complete_match_count for report in reports),
        exhausted_family=reports[-1].exhausted_family,
    )


@dataclass(frozen=True, slots=True)
class _IntersectionStageSpec:
    kind: str
    values: tuple[int, ...]


class _IntersectionCounter:
    """Mutable counter shared by one exact, bounded solver pass."""

    def __init__(
        self,
        request: EffectSeedRequest,
        *,
        start_after_trial: int,
        family_size: int,
        progress: IntersectionProgressCallback | None,
        progress_interval: int,
    ) -> None:
        if progress_interval <= 0:
            raise ValueError("intersection progress interval must be positive")
        self.start_after_trial = start_after_trial
        self.family_size = family_size
        self.progress = progress
        self.progress_interval = progress_interval
        self.fixed_seed_count = 0
        self.complete_match_count = 0
        self.inspected_through_trial = start_after_trial
        self.specs = self._build_specs(request)
        self.counts = [0] * len(self.specs)

    @staticmethod
    def _build_specs(request: EffectSeedRequest) -> tuple[_IntersectionStageSpec, ...]:
        specs: list[_IntersectionStageSpec] = []
        if request.grace_effect_id is not None:
            specs.append(_IntersectionStageSpec("grace", (request.grace_effect_id,)))
        if request.primary_effect_ids:
            specs.append(
                _IntersectionStageSpec("primary", tuple(sorted(request.primary_effect_ids)))
            )
        specs.extend(
            _IntersectionStageSpec("secondary", (effect_id,))
            for effect_id in sorted(request.required_secondary_ids)
        )
        criteria = request.auxiliary_criteria
        specs.extend(
            _IntersectionStageSpec("terrain", (key,))
            for key in sorted(criteria.required_terrain_effect_keys)
        )
        specs.extend(
            _IntersectionStageSpec("terrain", tuple(sorted(group)))
            for group in criteria.required_terrain_effect_key_groups
        )
        if criteria.terrain_row_indices:
            specs.append(
                _IntersectionStageSpec(
                    "terrain_row",
                    tuple(sorted(criteria.terrain_row_indices)),
                )
            )
        specs.extend(
            _IntersectionStageSpec("rule", (key,))
            for key in sorted(criteria.required_special_rule_keys)
        )
        specs.extend(
            _IntersectionStageSpec("rule", tuple(sorted(group)))
            for group in criteria.required_special_rule_key_groups
        )
        specs.extend(
            _IntersectionStageSpec("enemy", (key,))
            for key in sorted(criteria.required_enemy_lookup_keys)
        )
        specs.extend(
            _IntersectionStageSpec("enemy", tuple(sorted(group)))
            for group in criteria.required_enemy_lookup_key_groups
        )
        return tuple(specs)

    def observe_fixed_seed(self, pivot_trial: int) -> None:
        self.fixed_seed_count += 1
        self.inspected_through_trial = max(self.inspected_through_trial, pivot_trial)
        if self.specs and self.specs[0].kind == "grace":
            self.counts[0] += 1
        if self.progress and self.fixed_seed_count % self.progress_interval == 0:
            self.progress(self.snapshot(exhausted_family=False))

    def accept_primary(self, primary_effect_id: int) -> bool:
        for index, spec in enumerate(self.specs):
            if spec.kind != "primary":
                continue
            if primary_effect_id not in spec.values:
                return False
            self.counts[index] += 1
        return True

    def accept_secondaries(self, secondary_effect_ids: frozenset[int]) -> bool:
        for index, spec in enumerate(self.specs):
            if spec.kind != "secondary":
                continue
            if spec.values[0] not in secondary_effect_ids:
                return False
            self.counts[index] += 1
        return True

    def accept_effects(
        self,
        *,
        primary_effect_id: int,
        secondary_effect_ids: frozenset[int],
    ) -> bool:
        return self.accept_primary(primary_effect_id) and self.accept_secondaries(
            secondary_effect_ids
        )

    def accept_auxiliary(self, result: CompleteAuxiliaryResult) -> bool:
        terrain_keys = frozenset(result.terrain.display_effect_keys)
        rule_keys = frozenset(key for key in result.special_rules.keys if key)
        enemy_keys = frozenset(
            entry.lookup_key
            for group in result.enemies.groups
            for entry in group.entries
        )
        for index, spec in enumerate(self.specs):
            if spec.kind == "terrain":
                accepted = bool(terrain_keys.intersection(spec.values))
            elif spec.kind == "terrain_row":
                accepted = result.terrain.selected_row_index in spec.values
            elif spec.kind == "rule":
                accepted = bool(rule_keys.intersection(spec.values))
            elif spec.kind == "enemy":
                accepted = bool(enemy_keys.intersection(spec.values))
            else:
                continue
            if not accepted:
                return False
            self.counts[index] += 1
        return True

    def record_complete_match(self) -> None:
        self.complete_match_count += 1

    def snapshot(
        self,
        *,
        inspected_through_trial: int | None = None,
        exhausted_family: bool,
    ) -> EffectSeedIntersectionReport:
        through = (
            self.inspected_through_trial
            if inspected_through_trial is None
            else inspected_through_trial
        )
        return EffectSeedIntersectionReport(
            start_after_trial=self.start_after_trial,
            inspected_through_trial=through,
            family_size=self.family_size,
            fixed_seed_count=self.fixed_seed_count,
            stages=tuple(
                IntersectionStageCount(spec.kind, spec.values, count)
                for spec, count in zip(self.specs, self.counts, strict=True)
            ),
            complete_match_count=self.complete_match_count,
            exhausted_family=exhausted_family,
        )


def _terrain_only_criteria(
    criteria: AuxiliarySearchCriteria,
) -> AuxiliarySearchCriteria:
    """Return the cheap first-stage auxiliary filter for exact counting."""

    return AuxiliarySearchCriteria(
        required_terrain_effect_keys=criteria.required_terrain_effect_keys,
        required_terrain_effect_key_groups=criteria.required_terrain_effect_key_groups,
        terrain_row_indices=criteria.terrain_row_indices,
    )


def _validate_primary_draw1_map(
    request: EffectSeedRequest,
    mapping: PrimaryFirstDrawOutputMap,
) -> None:
    if request.playthrough not in (1, 2):
        raise ValueError("draw-1 primary maps apply only to playthroughs 1 and 2")
    if mapping.game_version != "2.00.02":
        raise ValueError("primary map game version is not PC v2.00.02")
    if mapping.category != request.playthrough:
        raise ValueError("primary map playthrough does not match the request")
    if mapping.rarity != request.rarity or mapping.draw_index != 1:
        raise ValueError("primary map rarity/draw does not match the request")


def _validate_conditioned_primary_map(
    request: EffectSeedRequest,
    mapping: PrimaryOutputMap,
    grace_mapping: GraceOutputMap,
) -> None:
    if request.grace_effect_id is None:
        raise ValueError("conditioned primary maps require a Grace constraint")
    if mapping.game_version != "2.00.02":
        raise ValueError("primary map game version is not PC v2.00.02")
    if mapping.rarity != request.rarity or mapping.draw_index != 2:
        raise ValueError("conditioned primary map rarity/draw does not match the request")
    if mapping.grace_effect_id != request.grace_effect_id:
        raise ValueError("conditioned primary map belongs to a different Grace")
    if mapping.record_type != grace_mapping.record_type:
        raise ValueError("primary and Grace maps use different record types")
    if mapping.grace_effect_slot != grace_mapping.effect_slot:
        raise ValueError("primary and Grace maps use different Grace slots")


def fixed_draw_constraints(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None = None,
    primary_mapping: PrimaryOutputMap | None = None,
    primary_first_mapping: PrimaryFirstDrawOutputMap | None = None,
    replay_primary: bool = False,
    allow_full_seed_family: bool = False,
) -> tuple[DrawConstraint, ...]:
    """Build the exact draw constraints supported by current evidence."""

    if primary_mapping is not None and primary_first_mapping is not None:
        raise ValueError("supply exactly one primary map kind")
    constraints: list[DrawConstraint] = []

    if request.grace_effect_id is not None:
        if grace_mapping is None:
            raise ValueError("a Grace output map is required for Grace inversion")
        if grace_mapping.rarity != request.rarity:
            raise ValueError("Grace map rarity does not match the request")
        grace_runs = U16Runs.from_ranges(
            (entry.start, entry.end)
            for entry in first_u16_ranges_for_grace(
                request.grace_effect_id,
                grace_mapping,
            )
        )
        constraints.append(DrawConstraint("grace", 1, grace_runs))

    if request.primary_effect_ids and not replay_primary:
        if primary_mapping is not None:
            if grace_mapping is None:
                raise ValueError("conditioned primary inversion requires a Grace map")
            _validate_conditioned_primary_map(request, primary_mapping, grace_mapping)
            constraints.append(
                DrawConstraint(
                    "primary",
                    2,
                    primary_mapping.runs_for_effects(request.primary_effect_ids),
                )
            )
        elif primary_first_mapping is not None:
            _validate_primary_draw1_map(request, primary_first_mapping)
            constraints.append(
                DrawConstraint(
                    "primary",
                    1,
                    primary_first_mapping.runs_for_effects(request.primary_effect_ids),
                )
            )
        else:
            raise ValueError("a certified primary output map is required")

    if not constraints and allow_full_seed_family:
        constraints.append(
            DrawConstraint("seed_space", 1, U16Runs(((0x0000, 0xFFFF),)))
        )
    if not constraints:
        raise ValueError("at least one certified fixed-draw effect constraint is required")
    return tuple(constraints)


def _verify_final_record(
    seed: int,
    request: EffectSeedRequest,
    generator: FinalRecordGenerator,
    grace_slot: int | None,
) -> ScrollCandidate | None:
    record = generator(seed)
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError("offline final-record generator returned a non-0xE8 record")
    candidate = ScrollCandidate.from_record(record, playthrough=request.playthrough)
    if candidate.seed != seed:
        raise ValueError("offline final-record generator changed the requested Seed")
    if candidate.rarity != request.rarity:
        raise ValueError("offline final-record generator changed the requested rarity")
    if not candidate_matches(
        candidate,
        primary_effect_ids=request.primary_effect_ids,
        required_secondary_ids=request.required_secondary_ids,
    ):
        return None
    if request.grace_effect_id is not None:
        if grace_slot is None or not 1 <= grace_slot <= 7:
            raise ValueError("Grace verification requires a valid effect slot")
        offset = EFFECT_START + (grace_slot - 1) * EFFECT_STRIDE + 4
        actual_grace = int.from_bytes(record[offset : offset + 4], "little")
        if actual_grace != request.grace_effect_id:
            return None
    return candidate


def _verify_effect_sequence(
    seed: int,
    request: EffectSeedRequest,
    generator: EffectSequenceGenerator,
) -> EffectSequenceResult | None:
    result = generator(seed)
    if result.seed != seed:
        raise ValueError("offline effect-sequence generator changed the requested Seed")
    if result.playthrough != request.playthrough or result.rarity != request.rarity:
        raise ValueError("offline effect-sequence generator changed the request context")
    if (
        request.primary_effect_ids
        and result.primary.effect_id not in request.primary_effect_ids
    ):
        return None
    actual_secondary_ids = frozenset(
        effect.effect_id for effect in result.secondaries
    )
    if not request.required_secondary_ids.issubset(actual_secondary_ids):
        return None
    if (
        request.grace_effect_id is not None
        and result.grace.effect_id != request.grace_effect_id
    ):
        return None
    return result


def _generate_effect_sequence_checked(
    seed: int,
    request: EffectSeedRequest,
    generator: EffectSequenceGenerator,
) -> EffectSequenceResult:
    """Generate one sequence while validating context but not user filters."""

    result = generator(seed)
    if result.seed != seed:
        raise ValueError("offline effect-sequence generator changed the requested Seed")
    if result.playthrough != request.playthrough or result.rarity != request.rarity:
        raise ValueError("offline effect-sequence generator changed the request context")
    if (
        request.grace_effect_id is not None
        and result.grace.effect_id != request.grace_effect_id
    ):
        raise ValueError("fixed Grace inverse produced a mismatched exact replay")
    return result


def iter_effect_seed_candidates(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None = None,
    primary_mapping: PrimaryOutputMap | None = None,
    primary_first_mapping: PrimaryFirstDrawOutputMap | None = None,
    final_record_generator: FinalRecordGenerator | None = None,
    effect_sequence_generator: EffectSequenceGenerator | None = None,
    primary_effect_generator: PrimaryEffectGenerator | None = None,
    primary_effect_id_generator: PrimaryEffectIdGenerator | None = None,
    primary_effect_id_batch_generator: PrimaryEffectIdBatchGenerator | None = None,
    allow_full_seed_family: bool = False,
    start_after_trial: int = 0,
    max_trials: int | None = None,
    _intersection_counter: _IntersectionCounter | None = None,
    cancelled: CancellationCheck | None = None,
) -> Iterator[EffectSeedCandidate]:
    """Yield exact Seed candidates without connecting to a game process."""

    if (
        request.required_secondary_ids
        and final_record_generator is None
        and effect_sequence_generator is None
    ):
        raise OfflineEffectReplayUnavailable(
            "secondary effects are path-dependent and require the offline final-record generator"
        )
    constraints = fixed_draw_constraints(
        request,
        grace_mapping=grace_mapping,
        primary_mapping=primary_mapping,
        primary_first_mapping=primary_first_mapping,
        replay_primary=(
            effect_sequence_generator is not None
            and request.playthrough in (3, 4, 5)
            and request.rarity in (3, 4, 5)
        ),
        allow_full_seed_family=allow_full_seed_family,
    )
    solutions: Iterator[SeedSolution] = iter_constraint_intersection(
        constraints,
        natural_only=request.natural_only,
        start_after_trial=start_after_trial,
        max_trials=max_trials,
    )
    grace_slot = grace_mapping.effect_slot if grace_mapping is not None else None
    fixed_draws = tuple((constraint.name, constraint.draw_index) for constraint in constraints)
    batch_primary = primary_effect_id_batch_generator if request.primary_effect_ids else None
    for solution, prefetched_primary_effect_id in _iter_solution_primary_ids(
        solutions,
        batch_primary,
    ):
        if cancelled is not None and cancelled():
            break
        if _intersection_counter is not None:
            if effect_sequence_generator is None:
                raise ValueError(
                    "intersection counting requires an exact effect-sequence generator"
                )
            _intersection_counter.observe_fixed_seed(solution.pivot_trial)
            effect_sequence = None
            has_primary_fast_path = request.primary_effect_ids and (
                prefetched_primary_effect_id is not None
                or primary_effect_id_batch_generator is not None
                or primary_effect_id_generator is not None
                or primary_effect_generator is not None
            )
            if has_primary_fast_path:
                primary_effect_id = (
                    prefetched_primary_effect_id
                    if prefetched_primary_effect_id is not None
                    else (
                        primary_effect_id_generator(solution.seed)
                        if primary_effect_id_generator is not None
                        else primary_effect_generator(solution.seed).effect_id
                    )
                )
                if not _intersection_counter.accept_primary(primary_effect_id):
                    continue
                if request.required_secondary_ids:
                    effect_sequence = _generate_effect_sequence_checked(
                        solution.seed,
                        request,
                        effect_sequence_generator,
                    )
                    if effect_sequence.primary.effect_id != primary_effect_id:
                        raise AssertionError(
                            "primary batch path disagreed with the exact effect sequence"
                        )
                    if not _intersection_counter.accept_secondaries(
                        frozenset(
                            effect.effect_id for effect in effect_sequence.secondaries
                        )
                    ):
                        continue
            elif request.primary_effect_ids or request.required_secondary_ids:
                effect_sequence = _generate_effect_sequence_checked(
                    solution.seed,
                    request,
                    effect_sequence_generator,
                )
                if not _intersection_counter.accept_effects(
                    primary_effect_id=effect_sequence.primary.effect_id,
                    secondary_effect_ids=frozenset(
                        effect.effect_id for effect in effect_sequence.secondaries
                    ),
                ):
                    continue

            auxiliary = None
            if not request.auxiliary_criteria.is_empty:
                # Terrain is independent and substantially cheaper than enemy
                # and rule generation. Reject terrain misses before building
                # the remaining auxiliary record while preserving exact
                # cumulative counts for every accepted terrain Seed.
                auxiliary = generate_matching_auxiliary(
                    solution.seed,
                    request.playthrough,
                    criteria=_terrain_only_criteria(request.auxiliary_criteria),
                )
                if auxiliary is None:
                    continue
                if not _intersection_counter.accept_auxiliary(auxiliary):
                    continue

            if effect_sequence is None:
                effect_sequence = _generate_effect_sequence_checked(
                    solution.seed,
                    request,
                    effect_sequence_generator,
                )
                if (
                    request.primary_effect_ids
                    and effect_sequence.primary.effect_id
                    not in request.primary_effect_ids
                ):
                    raise AssertionError(
                        "primary fast path disagreed with the exact effect sequence"
                    )
            _intersection_counter.record_complete_match()
            yield EffectSeedCandidate(
                seed=solution.seed,
                pivot_trial=solution.pivot_trial,
                fixed_draws=fixed_draws,
                auxiliary=auxiliary,
                effect_sequence=effect_sequence,
            )
            continue

        candidate = None
        effect_sequence = None
        has_effect_replay_filter = bool(
            request.primary_effect_ids or request.required_secondary_ids
        )
        used_primary_fast_path = False
        # Primary/secondary filters are usually much more selective and cheaper
        # than enemy/rule generation. Apply them first when present. With no
        # effect filter, defer replay until after the auxiliary constraints so
        # rejected Seeds do not pay for an unused effect preview.
        if (
            request.primary_effect_ids
            and not request.required_secondary_ids
            and (
                prefetched_primary_effect_id is not None
                or primary_effect_id_batch_generator is not None
                or primary_effect_id_generator is not None
                or primary_effect_generator is not None
            )
        ):
            primary_effect_id = (
                prefetched_primary_effect_id
                if prefetched_primary_effect_id is not None
                else (
                    primary_effect_id_generator(solution.seed)
                    if primary_effect_id_generator is not None
                    else primary_effect_generator(solution.seed).effect_id
                )
            )
            if primary_effect_id not in request.primary_effect_ids:
                continue
            used_primary_fast_path = True
        elif has_effect_replay_filter:
            if final_record_generator is not None:
                candidate = _verify_final_record(
                    solution.seed,
                    request,
                    final_record_generator,
                    grace_slot,
                )
                if candidate is None:
                    continue
            elif effect_sequence_generator is not None:
                effect_sequence = _verify_effect_sequence(
                    solution.seed,
                    request,
                    effect_sequence_generator,
                )
                if effect_sequence is None:
                    continue
        auxiliary = None
        if not request.auxiliary_criteria.is_empty:
            auxiliary = generate_matching_auxiliary(
                solution.seed,
                request.playthrough,
                criteria=request.auxiliary_criteria,
            )
            if auxiliary is None:
                continue
        if candidate is None and effect_sequence is None and final_record_generator is not None:
            candidate = _verify_final_record(
                solution.seed,
                request,
                final_record_generator,
                grace_slot,
            )
            if candidate is None:
                continue
        elif candidate is None and effect_sequence is None and effect_sequence_generator is not None:
            effect_sequence = _verify_effect_sequence(
                solution.seed,
                request,
                effect_sequence_generator,
            )
            if effect_sequence is None:
                continue
        if used_primary_fast_path and effect_sequence is None and candidate is None:
            raise AssertionError("primary fast path did not materialize a result preview")
        yield EffectSeedCandidate(
            seed=solution.seed,
            pivot_trial=solution.pivot_trial,
            fixed_draws=fixed_draws,
            auxiliary=auxiliary,
            record=candidate,
            effect_sequence=effect_sequence,
        )


def collect_effect_seed_page(
    request: EffectSeedRequest,
    *,
    page_size: int,
    grace_mapping: GraceOutputMap | None = None,
    primary_mapping: PrimaryOutputMap | None = None,
    primary_first_mapping: PrimaryFirstDrawOutputMap | None = None,
    final_record_generator: FinalRecordGenerator | None = None,
    effect_sequence_generator: EffectSequenceGenerator | None = None,
    primary_effect_generator: PrimaryEffectGenerator | None = None,
    primary_effect_id_generator: PrimaryEffectIdGenerator | None = None,
    primary_effect_id_batch_generator: PrimaryEffectIdBatchGenerator | None = None,
    allow_full_seed_family: bool = False,
    start_after_trial: int = 0,
    max_trials: int | None = None,
    intersection_progress: IntersectionProgressCallback | None = None,
    intersection_progress_interval: int = 4096,
    cancelled: CancellationCheck | None = None,
) -> EffectSeedPage:
    """Collect a bounded, non-overlapping page from the exact candidate stream."""

    if page_size <= 0:
        raise ValueError("page_size must be positive")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    constraints = fixed_draw_constraints(
        request,
        grace_mapping=grace_mapping,
        primary_mapping=primary_mapping,
        primary_first_mapping=primary_first_mapping,
        replay_primary=(
            effect_sequence_generator is not None
            and request.playthrough in (3, 4, 5)
            and request.rarity in (3, 4, 5)
        ),
        allow_full_seed_family=allow_full_seed_family,
    )
    pivot_family_size = choose_pivot(constraints).allowed_u16.bucket_count * 0x10000
    supports_intersection_report = (
        effect_sequence_generator is not None
        and request.playthrough in (3, 4, 5)
        and request.rarity in (3, 4, 5)
        and (request.grace_effect_id is not None or allow_full_seed_family)
    )
    counter = (
        _IntersectionCounter(
            request,
            start_after_trial=start_after_trial,
            family_size=pivot_family_size,
            progress=intersection_progress,
            progress_interval=intersection_progress_interval,
        )
        if supports_intersection_report
        else None
    )
    iterator = iter_effect_seed_candidates(
        request,
        grace_mapping=grace_mapping,
        primary_mapping=primary_mapping,
        primary_first_mapping=primary_first_mapping,
        final_record_generator=final_record_generator,
        effect_sequence_generator=effect_sequence_generator,
        primary_effect_generator=primary_effect_generator,
        primary_effect_id_generator=primary_effect_id_generator,
        primary_effect_id_batch_generator=primary_effect_id_batch_generator,
        allow_full_seed_family=allow_full_seed_family,
        start_after_trial=start_after_trial,
        max_trials=max_trials,
        _intersection_counter=counter,
        cancelled=cancelled,
    )
    collected: list[EffectSeedCandidate] = []
    for candidate in iterator:
        collected.append(candidate)
        if len(collected) == page_size:
            break
    candidates = tuple(collected)
    was_cancelled = cancelled is not None and cancelled()
    if len(candidates) == page_size:
        next_start_after_trial = candidates[-1].pivot_trial
    elif was_cancelled:
        next_start_after_trial = (
            counter.inspected_through_trial
            if counter is not None
            else (candidates[-1].pivot_trial if candidates else start_after_trial)
        )
    else:
        requested_stop = pivot_family_size
        if max_trials is not None:
            requested_stop = min(
                pivot_family_size,
                start_after_trial + max_trials,
            )
        next_start_after_trial = requested_stop

    report = None
    if counter is not None:
        report = counter.snapshot(
            inspected_through_trial=next_start_after_trial,
            exhausted_family=next_start_after_trial >= pivot_family_size,
        )
        if intersection_progress is not None:
            intersection_progress(report)
    return EffectSeedPage(
        candidates=candidates,
        start_after_trial=start_after_trial,
        next_start_after_trial=next_start_after_trial,
        intersection_report=report,
    )


__all__ = [
    "EffectSeedCandidate",
    "EffectSeedIntersectionReport",
    "EffectSeedPage",
    "EffectSeedRequest",
    "EffectSequenceGenerator",
    "FinalRecordGenerator",
    "IntersectionProgressCallback",
    "PrimaryEffectIdGenerator",
    "PrimaryEffectIdBatchGenerator",
    "IntersectionStageCount",
    "OfflineEffectReplayUnavailable",
    "PrimaryEffectGenerator",
    "collect_effect_seed_page",
    "fixed_draw_constraints",
    "iter_effect_seed_candidates",
    "merge_intersection_reports",
]
