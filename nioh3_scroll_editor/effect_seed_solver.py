"""Game-closed Seed construction for fixed-draw and replayed effects.

Grace constraints are exact first-draw inverse sets.  Historical conditioned
primary maps contain one representative per draw-2 high-16 bucket and are only
candidate prefilters: primary and secondary output must be checked by an exact
effect-sequence or final-record generator.  Without one, path-dependent
secondary requests fail closed.
"""

from __future__ import annotations

from dataclasses import dataclass
from itertools import islice, product
from typing import Callable, Iterator

from emaki_exchange import SCROLL_RECORD_SIZE

from .auxiliary_generation import (
    AuxiliarySearchCriteria,
    CompleteAuxiliaryResult,
    generate_complete_auxiliary,
    generate_enemy_match_masks_batch,
    generate_matching_auxiliary,
    generate_terrain_row_indices_batch,
    terrain_row_matches_criteria,
)
from .grace_map import GraceOutputMap, first_u16_ranges_for_grace
from .effect_sequence import EffectSequenceResult, GeneratedEffect
from .effect_generation_tables import (
    SCROLL_RECORD_TYPES,
    load_default_effect_generation_tables,
)
from .joint_solver import (
    DrawConstraint,
    SeedSolution,
    U16Runs,
    choose_pivot,
    iter_constraint_intersection,
)
from .models import (
    ScrollCandidate,
    candidate_matches,
    effective_required_secondary_ids,
)
from .primary_map import PrimaryFirstDrawOutputMap, PrimaryOutputMap


class OfflineEffectReplayUnavailable(RuntimeError):
    """Raised when path-dependent effects were requested without a final generator."""


@dataclass(frozen=True, slots=True)
class EffectSeedRequest:
    playthrough: int
    rarity: int
    primary_effect_ids: frozenset[int] = frozenset()
    required_secondary_ids: frozenset[int] = frozenset()
    required_secondary_id_groups: tuple[frozenset[int], ...] = ()
    grace_effect_id: int | None = None
    auxiliary_criteria: AuxiliarySearchCriteria = AuxiliarySearchCriteria()
    minimum_roll_percent_by_effect_id: tuple[tuple[int, int], ...] = ()
    natural_only: bool = True

    def __post_init__(self) -> None:
        if not 1 <= self.playthrough <= 5:
            raise ValueError("playthrough must be in 1..5")
        if not 0 <= self.rarity <= 5:
            raise ValueError("rarity must be in 0..5")
        if any(not group for group in self.required_secondary_id_groups):
            raise ValueError("secondary any-of groups cannot be empty")
        grouped_ids: set[int] = set()
        for group in self.required_secondary_id_groups:
            overlap = grouped_ids.intersection(group)
            if overlap:
                raise ValueError("secondary any-of groups must not overlap")
            grouped_ids.update(group)
        if grouped_ids.intersection(self.required_secondary_ids):
            raise ValueError(
                "a secondary effect cannot be both mandatory and in an any-of group"
            )
        effect_ids = (
            self.primary_effect_ids
            | self.required_secondary_ids
            | frozenset(grouped_ids)
        )
        if self.grace_effect_id is not None:
            effect_ids |= frozenset((self.grace_effect_id,))
        if any(not 0 <= effect_id <= 0xFFFFFFFF for effect_id in effect_ids):
            raise ValueError("effect IDs must fit in uint32")
        seen_roll_effect_ids: set[int] = set()
        for effect_id, minimum_roll in self.minimum_roll_percent_by_effect_id:
            if effect_id in seen_roll_effect_ids:
                raise ValueError("roll constraints must contain unique effect IDs")
            seen_roll_effect_ids.add(effect_id)
            if effect_id not in effect_ids:
                raise ValueError("roll constraints require a selected effect ID")
            if effect_id in grouped_ids:
                raise ValueError(
                    "secondary any-of groups currently require arbitrary values"
                )
            if not 0 <= minimum_roll <= 100:
                raise ValueError("minimum roll percent must be in 0..100")


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
CandidateFoundCallback = Callable[[EffectSeedCandidate], None]
PivotSeedCollector = Callable[..., tuple[tuple[int, int], ...] | None]


def validate_effect_request_feasibility(request: EffectSeedRequest) -> None:
    """Reject combinations that cannot fit the native PC v2.00.02 layout.

    This is a structural preflight, not a claim that every request that passes
    has a solution. It catches deterministic failures before a Seed family is
    opened: slot count, unavailable table rows, effect conflicts and category
    capacity. Path-dependent weighted lotteries are still decided by exact
    replay.
    """

    tables = load_default_effect_generation_tables()
    record_type = SCROLL_RECORD_TYPES[request.playthrough - 1]

    max_secondaries = {
        3: 3,
        4: 3 if request.grace_effect_id is not None else 4,
        5: 4,
    }.get(request.rarity)
    if max_secondaries is None:
        return

    primary_options: tuple[int | None, ...] = (
        tuple(sorted(request.primary_effect_ids))
        if request.primary_effect_ids
        else (None,)
    )
    capacities = tables.category_capacities(
        record_type=record_type,
        rarity=request.rarity,
    )

    def effect_label(effect_id: int) -> str:
        # Import locally so the low-level solver stays independent from UI
        # catalog initialization until a human-readable error is required.
        from .catalog import contextual_effect_name

        name = contextual_effect_name(
            effect_id,
            rarity=request.rarity,
            slot=1,
        )
        return f"{name} [0x{effect_id:04X}]"

    def validate_option(
        primary_id: int | None,
        grouped_choices: tuple[int, ...],
    ) -> str | None:
        effective_secondaries = set(
            effective_required_secondary_ids(
                primary_id=primary_id,
                primary_effect_ids=request.primary_effect_ids,
                required_secondary_ids=request.required_secondary_ids,
            )
            if primary_id is not None
            else request.required_secondary_ids
        )
        effective_secondaries.update(grouped_choices)
        if primary_id is not None and not request.primary_effect_ids:
            # With an unconstrained primary, every selected ordinary effect
            # means "must appear somewhere".  One of those selections may
            # legally occupy the actual primary slot.
            effective_secondaries.discard(primary_id)
        if len(effective_secondaries) > max_secondaries:
            return (
                f"需要 {len(effective_secondaries)} 个副词条，但当前结构最多只有 "
                f"{max_secondaries} 个普通副词条槽"
            )

        ordinary_ids = set(effective_secondaries)
        if primary_id is not None:
            ordinary_ids.add(primary_id)
        all_ids = set(ordinary_ids)
        if request.grace_effect_id is not None:
            all_ids.add(request.grace_effect_id)

        for effect_id in sorted(all_ids):
            definition = tables.effects_by_id.get(effect_id)
            if definition is None:
                return f"词条 0x{effect_id:04X} 不在当前原生参数表中"
            if effect_id in ordinary_ids:
                if not tables.candidate_context_allowed(
                    effect_id,
                    record_type=record_type,
                ):
                    return f"词条 0x{effect_id:04X} 在当前绘卷类型中不可生成"
                if not tables.native_effect_weight(
                    effect_id,
                    record_type=record_type,
                    rarity=request.rarity,
                    playthrough=request.playthrough,
                ):
                    return f"词条 0x{effect_id:04X} 在当前周目/稀有度权重为 0"

        if request.rarity == 5:
            promoted_only = tuple(
                effect_id
                for effect_id in sorted(effective_secondaries)
                if (
                    (definition := tables.effects_by_id.get(effect_id)) is not None
                    and bool(definition.normalization_flags & 0x08)
                )
            )
            if promoted_only:
                formatted = "、".join(
                    effect_label(effect_id) for effect_id in promoted_only
                )
                return (
                    "稀有度5只有一个升格/深奥槽，该槽会成为主词条；"
                    f"所选副词条 {formatted} 只能出现在这个槽位"
                )

        ordered_ids = sorted(all_ids)
        for index, left_id in enumerate(ordered_ids):
            for right_id in ordered_ids[index + 1 :]:
                if tables.effects_conflict(left_id, right_id):
                    return (
                        f"{effect_label(left_id)} 与 {effect_label(right_id)} "
                        "属于原生冲突组，不能同时出现"
                    )

        category_counts = [0] * len(capacities)
        for effect_id in ordinary_ids:
            category = tables.group_for_effect(effect_id).category_key
            category_counts[category] += 1
        for category, count in enumerate(category_counts):
            if count > capacities[category]:
                category_effect_ids = tuple(
                    sorted(
                        effect_id
                        for effect_id in ordinary_ids
                        if tables.group_for_effect(effect_id).category_key == category
                    )
                )
                selected_labels = "、".join(
                    effect_label(effect_id) for effect_id in category_effect_ids
                )
                excess = count - capacities[category]
                return (
                    f"以下已选词条共用原生类别 0x{category:02X}："
                    f"{selected_labels}。该类别最多容纳 {capacities[category]} 个，"
                    f"当前选择了 {count} 个；请至少移除 {excess} 个"
                )
        return None

    grouped_choice_sets: tuple[tuple[int, ...], ...] = (
        tuple(product(*(tuple(sorted(group)) for group in request.required_secondary_id_groups)))
        if request.required_secondary_id_groups
        else ((),)
    )
    option_errors: list[str | None] = []
    for grouped_choices in grouped_choice_sets:
        if request.primary_effect_ids:
            option_primary_ids = primary_options
        else:
            required_anywhere = tuple(
                sorted(set(request.required_secondary_ids).union(grouped_choices))
            )
            # None represents an unselected primary.  The remaining choices
            # cover every selected effect that could instead consume it.
            option_primary_ids = (None, *required_anywhere)
        option_errors.extend(
            validate_option(primary_id, grouped_choices)
            for primary_id in option_primary_ids
        )
    if option_errors and all(error is not None for error in option_errors):
        detail = next(error for error in option_errors if error is not None)
        raise ValueError(f"所选词条组合在原生生成结构中无解：{detail}。")

    return


def _has_terrain_constraints(criteria: AuxiliarySearchCriteria) -> bool:
    return bool(
        criteria.required_terrain_effect_keys
        or criteria.required_terrain_effect_key_groups
        or criteria.terrain_row_indices
    )


def _enemy_constraint_group_count(criteria: AuxiliarySearchCriteria) -> int:
    return (
        len(criteria.required_enemy_lookup_keys)
        + len(criteria.required_enemy_lookup_key_groups)
    )


def _iter_solution_prefetch(
    solutions: Iterator[SeedSolution],
    batch_generator: PrimaryEffectIdBatchGenerator | None,
    primary_effect_ids: frozenset[int],
    terrain_criteria: AuxiliarySearchCriteria,
    playthrough: int,
    *,
    batch_size: int = 65_536,
) -> Iterator[tuple[SeedSolution, int | None, int | None, int | None]]:
    """Attach native primary, terrain, and enemy results to a Seed stream."""

    enemy_group_count = _enemy_constraint_group_count(terrain_criteria)
    prefetch_terrain = _has_terrain_constraints(terrain_criteria) or bool(
        enemy_group_count
    )
    if batch_generator is None and not prefetch_terrain:
        for solution in solutions:
            yield solution, None, None, None
        return
    while True:
        batch = tuple(islice(solutions, batch_size))
        if not batch:
            return
        seeds = tuple(solution.seed for solution in batch)
        effect_ids: tuple[int | None, ...]
        if batch_generator is None:
            effect_ids = (None,) * len(batch)
        else:
            generated_ids = batch_generator(seeds)
            if len(generated_ids) != len(batch):
                raise ValueError("primary batch generator returned the wrong result count")
            effect_ids = tuple(generated_ids)
        terrain_rows: tuple[int | None, ...]
        if prefetch_terrain:
            generated_rows = generate_terrain_row_indices_batch(seeds)
            if len(generated_rows) != len(batch):
                raise ValueError("terrain batch generator returned the wrong result count")
            terrain_rows = tuple(generated_rows)
        else:
            terrain_rows = (None,) * len(batch)
        enemy_masks: tuple[int | None, ...]
        if enemy_group_count:
            if any(row is None for row in terrain_rows):
                raise AssertionError("native enemy matching requires terrain rows")
            eligible_indices = tuple(
                index
                for index, (effect_id, terrain_row) in enumerate(
                    zip(effect_ids, terrain_rows, strict=True)
                )
                if (
                    (not primary_effect_ids or effect_id in primary_effect_ids)
                    and (
                        not _has_terrain_constraints(terrain_criteria)
                        or terrain_row_matches_criteria(
                            int(terrain_row),
                            terrain_criteria,
                        )
                    )
                )
            )
            scattered_masks: list[int | None] = [None] * len(batch)
            if eligible_indices:
                generated_masks = generate_enemy_match_masks_batch(
                    tuple(seeds[index] for index in eligible_indices),
                    tuple(int(terrain_rows[index]) for index in eligible_indices),
                    playthrough,
                    criteria=terrain_criteria,
                )
                if len(generated_masks) != len(eligible_indices):
                    raise ValueError(
                        "enemy batch generator returned the wrong result count"
                    )
                for index, mask in zip(
                    eligible_indices,
                    generated_masks,
                    strict=True,
                ):
                    scattered_masks[index] = mask
            enemy_masks = tuple(scattered_masks)
        else:
            enemy_masks = (None,) * len(batch)
        yield from zip(batch, effect_ids, terrain_rows, enemy_masks, strict=True)


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
        self.grace_requires_replay = (
            request.rarity == 4 and request.grace_effect_id is not None
        )
        self.primary_is_unconstrained = not request.primary_effect_ids
        self.optional_primary_value_ids = (
            request.primary_effect_ids - request.required_secondary_ids
        )

    @staticmethod
    def _build_specs(request: EffectSeedRequest) -> tuple[_IntersectionStageSpec, ...]:
        specs: list[_IntersectionStageSpec] = []
        if request.grace_effect_id is not None and request.rarity != 4:
            specs.append(_IntersectionStageSpec("grace", (request.grace_effect_id,)))
        if request.primary_effect_ids:
            specs.append(
                _IntersectionStageSpec("primary", tuple(sorted(request.primary_effect_ids)))
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
            _IntersectionStageSpec("enemy", (key,))
            for key in sorted(criteria.required_enemy_lookup_keys)
        )
        specs.extend(
            _IntersectionStageSpec("enemy", tuple(sorted(group)))
            for group in criteria.required_enemy_lookup_key_groups
        )
        specs.extend(
            _IntersectionStageSpec("rule", (key,))
            for key in sorted(criteria.required_special_rule_keys)
        )
        specs.extend(
            _IntersectionStageSpec("rule", tuple(sorted(group)))
            for group in criteria.required_special_rule_key_groups
        )
        if request.grace_effect_id is not None and request.rarity == 4:
            specs.append(_IntersectionStageSpec("grace", (request.grace_effect_id,)))
        ordinary_kind = "ordinary" if not request.primary_effect_ids else "secondary"
        ordinary_any_kind = (
            "ordinary_any" if not request.primary_effect_ids else "secondary_any"
        )
        specs.extend(
            _IntersectionStageSpec(ordinary_kind, (effect_id,))
            for effect_id in sorted(request.required_secondary_ids)
        )
        specs.extend(
            _IntersectionStageSpec(ordinary_any_kind, tuple(sorted(group)))
            for group in request.required_secondary_id_groups
        )
        specs.extend(
            _IntersectionStageSpec("value", (effect_id, minimum_roll))
            for effect_id, minimum_roll in sorted(
                request.minimum_roll_percent_by_effect_id
            )
            if minimum_roll > 0
        )
        return tuple(specs)

    def observe_fixed_seed(self, pivot_trial: int) -> None:
        self.fixed_seed_count += 1
        self.inspected_through_trial = max(self.inspected_through_trial, pivot_trial)
        if (
            self.specs
            and self.specs[0].kind == "grace"
            and not self.grace_requires_replay
        ):
            self.counts[0] += 1
        if self.progress and self.fixed_seed_count % self.progress_interval == 0:
            self.progress(self.snapshot(exhausted_family=False))

    def accept_grace(self, result: EffectSequenceResult) -> bool:
        """Count the final R4 Grace after exact finalizer replay."""

        if not self.grace_requires_replay:
            return True
        spec_index = next(
            index for index, spec in enumerate(self.specs) if spec.kind == "grace"
        )
        if (
            not result.terminal_is_special
            or result.grace.effect_id not in self.specs[spec_index].values
        ):
            return False
        self.counts[spec_index] += 1
        return True

    def accept_primary(self, primary_effect_id: int) -> bool:
        for index, spec in enumerate(self.specs):
            if spec.kind != "primary":
                continue
            if primary_effect_id not in spec.values:
                return False
            self.counts[index] += 1
        return True

    def accept_secondaries(
        self,
        secondary_effect_ids: frozenset[int],
        *,
        primary_effect_id: int | None = None,
    ) -> bool:
        ordinary_effect_ids = set(secondary_effect_ids)
        if self.primary_is_unconstrained and primary_effect_id is not None:
            ordinary_effect_ids.add(primary_effect_id)
        for index, spec in enumerate(self.specs):
            if spec.kind not in (
                "secondary",
                "secondary_any",
                "ordinary",
                "ordinary_any",
            ):
                continue
            if spec.kind in ("secondary", "ordinary") and spec.values[0] not in ordinary_effect_ids:
                return False
            if spec.kind in ("secondary_any", "ordinary_any") and not ordinary_effect_ids.intersection(
                spec.values
            ):
                return False
            self.counts[index] += 1
        return True

    def accept_values(self, result: EffectSequenceResult) -> bool:
        ordinary = (result.primary, *result.secondaries)
        for index, spec in enumerate(self.specs):
            if spec.kind != "value":
                continue
            effect_id, minimum_roll = spec.values
            matching = tuple(
                effect
                for effect in ordinary
                if effect.effect_id == effect_id
            )
            if not matching and effect_id in self.optional_primary_value_ids:
                self.counts[index] += 1
                continue
            if not any(
                effect.effect_id == effect_id
                and effect.roll_percent >= minimum_roll
                for effect in matching
            ):
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
            secondary_effect_ids,
            primary_effect_id=primary_effect_id,
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

    def accept_auxiliary_stage(self, kind: str, result: object) -> bool:
        """Count one native auxiliary stage in its actual execution order."""

        if kind == "terrain":
            terrain = result
            terrain_keys = frozenset(terrain.display_effect_keys)
            for index, spec in enumerate(self.specs):
                if spec.kind == "terrain":
                    accepted = bool(terrain_keys.intersection(spec.values))
                elif spec.kind == "terrain_row":
                    accepted = terrain.selected_row_index in spec.values
                else:
                    continue
                if not accepted:
                    return False
                self.counts[index] += 1
            return True
        if kind == "enemy":
            enemies = result
            enemy_keys = frozenset(
                entry.lookup_key
                for group in enemies.groups
                for entry in group.entries
            )
            for index, spec in enumerate(self.specs):
                if spec.kind != "enemy":
                    continue
                if not enemy_keys.intersection(spec.values):
                    return False
                self.counts[index] += 1
            return True
        if kind == "rule":
            special_rules = result
            rule_keys = frozenset(key for key in special_rules.keys if key)
            for index, spec in enumerate(self.specs):
                if spec.kind != "rule":
                    continue
                if not rule_keys.intersection(spec.values):
                    return False
                self.counts[index] += 1
            return True
        raise ValueError(f"unknown auxiliary stage: {kind}")

    def accept_prefetched_enemy_mask(self, matched_mask: int) -> bool:
        """Count native-batched enemy groups in the same order as the UI."""

        group_index = 0
        for index, spec in enumerate(self.specs):
            if spec.kind != "enemy":
                continue
            if (matched_mask & (1 << group_index)) == 0:
                return False
            self.counts[index] += 1
            group_index += 1
        return True

    def accept_prefetched_terrain(self) -> None:
        """Count a terrain row after the complete native terrain gate passed."""

        for index, spec in enumerate(self.specs):
            if spec.kind in ("terrain", "terrain_row"):
                self.counts[index] += 1

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
        required_secondary_id_groups=request.required_secondary_id_groups,
    ):
        return None
    if request.grace_effect_id is not None:
        if grace_slot is None or not 1 <= grace_slot <= 7:
            raise ValueError("Grace verification requires a valid effect slot")
        if (
            candidate.grace_slot_index != grace_slot - 1
            or candidate.grace is None
            or candidate.grace.effect_id != request.grace_effect_id
        ):
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
    ordinary_match_ids = set(actual_secondary_ids)
    if not request.primary_effect_ids:
        ordinary_match_ids.add(result.primary.effect_id)
    if not request.required_secondary_ids.issubset(ordinary_match_ids):
        return None
    if any(
        not group.intersection(ordinary_match_ids)
        for group in request.required_secondary_id_groups
    ):
        return None
    if (
        request.grace_effect_id is not None
        and (
            not result.terminal_is_special
            or result.grace.effect_id != request.grace_effect_id
        )
    ):
        return None
    if request.minimum_roll_percent_by_effect_id:
        ordinary = (result.primary, *result.secondaries)
        for effect_id, minimum_roll in request.minimum_roll_percent_by_effect_id:
            matching = tuple(
                effect for effect in ordinary if effect.effect_id == effect_id
            )
            if (
                not matching
                and effect_id in request.primary_effect_ids
                and effect_id not in request.required_secondary_ids
            ):
                continue
            if not any(
                effect.effect_id == effect_id
                and effect.roll_percent >= minimum_roll
                for effect in matching
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
        and request.rarity == 5
        and (
            not result.terminal_is_special
            or result.grace.effect_id != request.grace_effect_id
        )
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
    pivot_seed_collector: PivotSeedCollector | None = None,
    pivot_seed_collector_chunk_trials: int = 1_000_000,
) -> Iterator[EffectSeedCandidate]:
    """Yield exact Seed candidates without connecting to a game process."""

    validate_effect_request_feasibility(request)
    if (
        (request.required_secondary_ids or request.required_secondary_id_groups)
        and final_record_generator is None
        and effect_sequence_generator is None
    ):
        raise OfflineEffectReplayUnavailable(
            "secondary effects are path-dependent and require the offline final-record generator"
        )
    if (
        request.minimum_roll_percent_by_effect_id
        and effect_sequence_generator is None
    ):
        raise OfflineEffectReplayUnavailable(
            "roll constraints require an exact effect-sequence generator"
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
        pivot_seed_collector=pivot_seed_collector,
        pivot_seed_collector_chunk_trials=pivot_seed_collector_chunk_trials,
    )
    grace_slot = grace_mapping.effect_slot if grace_mapping is not None else None
    fixed_draws = tuple((constraint.name, constraint.draw_index) for constraint in constraints)
    batch_primary = primary_effect_id_batch_generator if request.primary_effect_ids else None
    for (
        solution,
        prefetched_primary_effect_id,
        prefetched_terrain_row,
        prefetched_enemy_mask,
    ) in _iter_solution_prefetch(
        solutions,
        batch_primary,
        request.primary_effect_ids,
        request.auxiliary_criteria,
        request.playthrough,
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
            elif request.primary_effect_ids:
                if effect_sequence is None:
                    effect_sequence = _generate_effect_sequence_checked(
                        solution.seed,
                        request,
                        effect_sequence_generator,
                    )
                if not _intersection_counter.accept_primary(
                    effect_sequence.primary.effect_id
                ):
                    continue

            auxiliary = None
            if not request.auxiliary_criteria.is_empty:
                if prefetched_terrain_row is not None:
                    if not terrain_row_matches_criteria(
                        prefetched_terrain_row,
                        request.auxiliary_criteria,
                    ):
                        continue
                    _intersection_counter.accept_prefetched_terrain()
                if (
                    prefetched_enemy_mask is not None
                    and not _intersection_counter.accept_prefetched_enemy_mask(
                        prefetched_enemy_mask
                    )
                ):
                    continue
                # Terrain is independent and substantially cheaper than enemy
                # and rule generation. Reject terrain misses before building
                # the remaining auxiliary record while preserving exact
                # cumulative counts for every accepted terrain Seed.
                auxiliary = generate_matching_auxiliary(
                    solution.seed,
                    request.playthrough,
                    criteria=request.auxiliary_criteria,
                    stage_acceptor=(
                        lambda kind, result: (
                            True
                            if (
                                (kind == "terrain" and prefetched_terrain_row is not None)
                                or (kind == "enemy" and prefetched_enemy_mask is not None)
                            )
                            else _intersection_counter.accept_auxiliary_stage(kind, result)
                        )
                    ),
                )
                if auxiliary is None:
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
            if _intersection_counter.grace_requires_replay:
                if not _intersection_counter.accept_grace(effect_sequence):
                    continue
            if request.required_secondary_ids or request.required_secondary_id_groups:
                if not _intersection_counter.accept_secondaries(
                    frozenset(
                        effect.effect_id for effect in effect_sequence.secondaries
                    ),
                    primary_effect_id=effect_sequence.primary.effect_id,
                ):
                    continue
            if request.minimum_roll_percent_by_effect_id:
                if not _intersection_counter.accept_values(effect_sequence):
                    continue
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
            request.primary_effect_ids
            or request.required_secondary_ids
            or request.required_secondary_id_groups
            or request.grace_effect_id is not None
        )
        used_primary_fast_path = False
        # Primary/secondary filters are usually much more selective and cheaper
        # than enemy/rule generation. Apply them first when present. With no
        # effect filter, defer replay until after the auxiliary constraints so
        # rejected Seeds do not pay for an unused effect preview.
        if (
            request.primary_effect_ids
            and not request.required_secondary_ids
            and not request.required_secondary_id_groups
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
            if (
                prefetched_terrain_row is not None
                and not terrain_row_matches_criteria(
                    prefetched_terrain_row,
                    request.auxiliary_criteria,
                )
            ):
                continue
            if prefetched_enemy_mask is not None:
                enemy_group_count = _enemy_constraint_group_count(
                    request.auxiliary_criteria
                )
                target_enemy_mask = (1 << enemy_group_count) - 1
                if prefetched_enemy_mask != target_enemy_mask:
                    continue
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
    candidate_found: CandidateFoundCallback | None = None,
    cancelled: CancellationCheck | None = None,
    pivot_seed_collector: PivotSeedCollector | None = None,
    pivot_seed_collector_chunk_trials: int = 1_000_000,
) -> EffectSeedPage:
    """Collect a bounded, non-overlapping page from the exact candidate stream."""

    if page_size <= 0:
        raise ValueError("page_size must be positive")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    validate_effect_request_feasibility(request)
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
        pivot_seed_collector=pivot_seed_collector,
        pivot_seed_collector_chunk_trials=pivot_seed_collector_chunk_trials,
    )
    collected: list[EffectSeedCandidate] = []
    for candidate in iterator:
        collected.append(candidate)
        if candidate_found is not None:
            candidate_found(candidate)
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
    "CandidateFoundCallback",
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
    "validate_effect_request_feasibility",
]
