"""DirectCompute forward filtering for partial ordinary-effect requests."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache

from emaki_exchange import CATEGORY_TO_TYPE

from .effect_generation_tables import load_default_effect_generation_tables
from .effect_preimage_accelerator import match_effect_constraints_d3d11
from .grace_map import GraceOutputMap
from .r4_finalizer_reference import EffectRow, effect_weight, type_class_for_record_type


PC_V2_00_02_AUXILIARY_MODE_THRESHOLD = 2000


@dataclass(frozen=True, slots=True)
class EffectBatchFilterResult:
    masks: tuple[int, ...]
    target_mask: int


@lru_cache(maxsize=8)
def _candidate_configuration(
    playthrough: int,
    rarity: int,
    level: int,
) -> tuple[
    tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, int], ...
    ],
    tuple[int, ...],
    int,
    int,
    int,
]:
    if playthrough not in (3, 4, 5):
        raise ValueError("DirectCompute partial-effect filtering requires NG3-NG5")
    if rarity not in (3, 4, 5):
        raise ValueError(
            "DirectCompute partial-effect filtering supports rarity 3, 4, or 5"
        )
    if not 0 <= level <= 0xFFFF:
        raise ValueError("effect filtering level must fit in uint16")
    tables = load_default_effect_generation_tables()
    record_type = CATEGORY_TO_TYPE[playthrough]
    candidates: list[
        tuple[int, int, int, int, int, int, int, int, int, int, int]
    ] = []
    finalizer_progress = tables.resource.playthrough_progress(playthrough)
    type_class = type_class_for_record_type(record_type, rarity)
    rarity_definition = tables.rarity_generation[rarity]
    for effect in sorted(
        tables.effects_by_id.values(),
        key=lambda definition: definition.row_index,
    ):
        if effect.row_index == 0 or not tables.candidate_context_allowed(
            effect.effect_id,
            record_type=record_type,
        ):
            continue
        weight = tables.native_effect_weight(
            effect.effect_id,
            record_type=record_type,
            rarity=rarity,
            playthrough=playthrough,
            restricted_destination_slot=False,
        )
        promoted_weight = weight
        final_weight_common = 0
        final_weight_special = 0
        if playthrough == 3 and rarity == 4:
            row = EffectRow(
                tables.resource.table("effect").row(effect.row_index)
            )
            final_weight_common = effect_weight(
                row,
                weight_slot=0x3C,
                type_class=type_class,
                rarity=rarity,
                progress=finalizer_progress,
                optional_multiplier_lookup=tables.optional_multiplier,
            )
            final_weight_special = effect_weight(
                row,
                weight_slot=0x3E,
                type_class=type_class,
                rarity=rarity,
                progress=finalizer_progress,
                optional_multiplier_lookup=tables.optional_multiplier,
            )
        if not (weight or promoted_weight or final_weight_common or final_weight_special):
            continue
        group = tables.groups_by_key[effect.group_key]
        promoted = bool(effect.normalization_flags & 0x08)
        value_one_roll_mask = 0
        if playthrough == 3 and rarity == 4:
            for roll_percent in range(
                rarity_definition.minimum_roll_percent,
                rarity_definition.maximum_roll_percent + 1,
            ):
                if tables.resolved_effect_value(
                    effect.effect_id,
                    roll_percent=roll_percent,
                    level=level,
                ) == 1:
                    value_one_roll_mask |= 1 << (
                        roll_percent - rarity_definition.minimum_roll_percent
                    )
        candidates.append(
            (
                effect.effect_id,
                group.group_key,
                group.category_key,
                group.conflict_mask_0,
                group.conflict_mask_1,
                0 if promoted else weight,
                promoted_weight if promoted else 0,
                final_weight_common,
                final_weight_special,
                int(bool(effect.normalization_flags & 0x08)),
                value_one_roll_mask,
            )
        )
    return (
        tuple(candidates),
        tables.category_capacities(record_type=record_type, rarity=rarity),
        int(rarity_definition.promotion_probability_percent * 100),
        rarity_definition.minimum_roll_percent,
        rarity_definition.maximum_roll_percent,
    )


@lru_cache(maxsize=8)
def _special_group_lookup(
    rarity: int,
    special_mapping: GraceOutputMap | None,
) -> tuple[tuple[int, int, int, int], ...]:
    tables = load_default_effect_generation_tables()
    if rarity == 3:
        group = tables.group_for_effect(0x0001)
        return (
            (
                group.group_key,
                group.conflict_mask_0,
                group.conflict_mask_1,
                0x0001,
            ),
        )
    if special_mapping is None:
        raise ValueError("rarity-4/5 effect filtering requires a Grace output map")
    groups: list[tuple[int, int, int, int] | None] = [None] * 0x10000
    for entry in special_mapping.ranges:
        group = tables.group_for_effect(entry.grace_id)
        packed = (
            group.group_key,
            group.conflict_mask_0,
            group.conflict_mask_1,
            entry.grace_id,
        )
        for value in range(entry.start, entry.end + 1):
            groups[value] = packed
    if any(group is None for group in groups):
        raise ValueError("Grace output map does not cover every first-draw bucket")
    return tuple(group for group in groups if group is not None)


def _merged_rarity4_criterion_groups(
    *,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
    required_secondary_id_groups: tuple[frozenset[int], ...],
) -> tuple[tuple[int, frozenset[int]], ...]:
    """Merge overlapping final requirements for the lossless N-1 filter."""

    pending: list[set[int]] = []
    if primary_effect_ids:
        pending.append(set(primary_effect_ids))
    pending.extend({effect_id} for effect_id in sorted(required_secondary_ids))
    pending.extend(set(group) for group in required_secondary_id_groups)
    components: list[set[int]] = []
    for values in pending:
        merged = set(values)
        retained: list[set[int]] = []
        changed = True
        while changed:
            changed = False
            retained.clear()
            for component in components:
                if merged.intersection(component):
                    merged.update(component)
                    changed = True
                else:
                    retained.append(component)
            components = list(retained)
        components.append(merged)
    return tuple(
        (2, frozenset(component))
        for component in sorted(
            components,
            key=lambda group: (min(group), len(group), tuple(sorted(group))),
        )
    )


def match_partial_effect_constraints_batch(
    seeds: tuple[int, ...],
    *,
    playthrough: int,
    rarity: int,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
    required_secondary_id_groups: tuple[frozenset[int], ...],
    special_mapping: GraceOutputMap | None,
    level: int = 180,
) -> EffectBatchFilterResult | None:
    """Return per-Seed masks for an exact partial-effect forward pass."""

    exact_groups: list[tuple[int, frozenset[int]]] = []
    if primary_effect_ids:
        exact_groups.append((0, primary_effect_ids))
    ordinary_kind = 1 if primary_effect_ids else 2
    exact_groups.extend(
        (ordinary_kind, frozenset((effect_id,)))
        for effect_id in sorted(required_secondary_ids)
    )
    exact_groups.extend(
        (ordinary_kind, group) for group in required_secondary_id_groups
    )
    if not exact_groups:
        return None
    groups = exact_groups
    use_exact_r4_finalizer = False
    minimum_matched_groups: int | None = None
    if playthrough == 3 and rarity == 4:
        merged_groups = list(
            _merged_rarity4_criterion_groups(
                primary_effect_ids=primary_effect_ids,
                required_secondary_ids=required_secondary_ids,
                required_secondary_id_groups=required_secondary_id_groups,
            )
        )
        if len(merged_groups) >= 3:
            groups = merged_groups
            minimum_matched_groups = len(groups) - 1
        else:
            use_exact_r4_finalizer = True
    (
        candidates,
        capacities,
        promotion_threshold,
        minimum_roll_percent,
        maximum_roll_percent,
    ) = _candidate_configuration(
        playthrough,
        rarity,
        level,
    )
    masks = match_effect_constraints_d3d11(
        seeds,
        candidates=candidates,
        special_groups=_special_group_lookup(rarity, special_mapping),
        category_capacities=capacities,
        criterion_groups=tuple(groups),
        rarity=rarity,
        ordinary_slot_count=5 if rarity == 5 else 4,
        slot_limit=6 if rarity == 5 else (5 if rarity == 4 else 4),
        promotion_threshold=promotion_threshold,
        consumes_special_draw=rarity in (4, 5),
        minimum_roll_percent=minimum_roll_percent,
        maximum_roll_percent=maximum_roll_percent,
        apply_r4_finalizer=use_exact_r4_finalizer,
        auxiliary_mode_threshold=PC_V2_00_02_AUXILIARY_MODE_THRESHOLD,
    )
    if masks is None:
        return None
    target_mask = (1 << len(groups)) - 1
    if minimum_matched_groups is not None:
        masks = tuple(
            target_mask if mask.bit_count() >= minimum_matched_groups else 0
            for mask in masks
        )
    return EffectBatchFilterResult(masks=masks, target_mask=target_mask)


__all__ = [
    "EffectBatchFilterResult",
    "match_partial_effect_constraints_batch",
]
