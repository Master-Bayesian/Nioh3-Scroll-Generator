"""Typed indexes for the captured PC v2.00.02 effect-generation tables.

This module implements only table semantics supported by static control-flow
evidence.  It does not perform RNG selection or claim complete offline record
generation.  In particular, effect compatibility mirrors RVAs 0x5778C0 and
0x578C40: equal group keys and intersecting conflict masks are rejected.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import struct
from typing import Iterable

from .r4_finalizer_reference import (
    EffectRow,
    Lcg32,
    curve_scale_from_raw_table,
    effect_weight,
    f32,
    f32_mul,
    roll_percentile,
    resolved_base_value,
    type_class_for_record_type,
)
from .r4_finalizer_resource import (
    R4FinalizerResourceBundle,
    load_default_r4_finalizer_resource,
)


SCROLL_RECORD_TYPES = (0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523)
SCROLL_ITEM_MODE = 0x12
EMPTY_EFFECT_ID = 0xFFFFFFFF


def _u16(row: bytes, offset: int) -> int:
    return struct.unpack_from("<H", row, offset)[0]


def _u32(row: bytes, offset: int) -> int:
    return struct.unpack_from("<I", row, offset)[0]


@dataclass(frozen=True, slots=True)
class ScrollItemDefinition:
    row_index: int
    record_type: int
    field_154: int
    field_15c: int
    mode: int
    candidate_item_flags: int


@dataclass(frozen=True, slots=True)
class EffectGroupDefinition:
    row_index: int
    group_key: int
    category_key: int
    conflict_mask_0: int
    conflict_mask_1: int

    def conflicts_with(self, other: "EffectGroupDefinition") -> bool:
        return (
            self.group_key == other.group_key
            or bool(self.conflict_mask_0 & other.conflict_mask_0)
            or bool(self.conflict_mask_1 & other.conflict_mask_1)
        )


@dataclass(frozen=True, slots=True)
class EffectDefinition:
    row_index: int
    effect_id: int
    group_key: int
    flags: int
    normalization_flags: int
    progress_threshold: int
    alternate_threshold: int
    lottery_weights: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class CategoryDefinition:
    row_index: int
    category_key: int
    rarity_capacities: tuple[int, ...]
    mode12_lottery_weight: int
    mode12_capacity: int
    mode12_count_multiplier_key: int


@dataclass(frozen=True, slots=True)
class CategoryCountMultiplierDefinition:
    row_index: int
    lookup_key: int
    multipliers: tuple[float, ...]


@dataclass(frozen=True, slots=True)
class OptionalMultiplierDefinition:
    row_index: int
    lookup_key: int
    multiplier: float


@dataclass(frozen=True, slots=True)
class RarityGenerationDefinition:
    rarity: int
    minimum_roll_percent: int
    maximum_roll_percent: int
    base_slot_count: int
    total_slot_count: int
    promotion_trials: int
    promotion_probability_percent: float


@dataclass(frozen=True, slots=True)
class WeightedEffectCandidate:
    effect: EffectDefinition
    weight: int


class EffectGenerationTableIndex:
    """Verified lookup indexes over pointer-free native table captures."""

    def __init__(self, resource: R4FinalizerResourceBundle):
        self.resource = resource
        self.items_by_record_type = self._index_scroll_items()
        self.categories_by_key = self._index_categories()
        self.category_count_multipliers_by_key = (
            self._index_category_count_multipliers()
        )
        self.groups_by_key = self._index_effect_groups()
        self.effects_by_id = self._index_effects()
        self.optional_multipliers_by_key = self._index_optional_multipliers()
        self.rarity_generation = self._index_rarity_generation()
        self._base_candidate_pool_cache: dict[
            tuple[int, int, int, bool, int, bool, int, int],
            tuple[WeightedEffectCandidate, ...],
        ] = {}
        missing_groups = {
            effect.group_key
            for effect in self.effects_by_id.values()
            if effect.group_key not in self.groups_by_key
        }
        if missing_groups:
            sample = ", ".join(f"0x{key:04X}" for key in sorted(missing_groups)[:8])
            raise ValueError(f"effect table references unknown groups: {sample}")

    @staticmethod
    def _insert_unique(target: dict[int, object], key: int, value: object, label: str) -> None:
        if key in target:
            raise ValueError(f"duplicate {label} key 0x{key:X}")
        target[key] = value

    def _index_scroll_items(self) -> dict[int, ScrollItemDefinition]:
        result: dict[int, ScrollItemDefinition] = {}
        for row_index, row in enumerate(self.resource.table("item").rows()):
            record_type = _u16(row, 0x152)
            if record_type not in SCROLL_RECORD_TYPES:
                continue
            item = ScrollItemDefinition(
                row_index=row_index,
                record_type=record_type,
                field_154=_u32(row, 0x154),
                field_15c=_u32(row, 0x15C),
                mode=row[0x182],
                candidate_item_flags=_u32(row, 0xB0),
            )
            self._insert_unique(result, record_type, item, "scroll item")
        missing = set(SCROLL_RECORD_TYPES) - result.keys()
        if missing:
            sample = ", ".join(f"0x{key:04X}" for key in sorted(missing))
            raise ValueError(f"missing scroll item rows: {sample}")
        return result

    def _index_effect_groups(self) -> dict[int, EffectGroupDefinition]:
        result: dict[int, EffectGroupDefinition] = {}
        for row_index, row in enumerate(self.resource.table("effect_group").rows()):
            group = EffectGroupDefinition(
                row_index=row_index,
                group_key=_u16(row, 0x0C),
                category_key=_u16(row, 0x24),
                conflict_mask_0=_u32(row, 0x54),
                conflict_mask_1=_u32(row, 0x58),
            )
            self._insert_unique(result, group.group_key, group, "effect-group")
        return result

    def _index_categories(self) -> dict[int, CategoryDefinition]:
        result: dict[int, CategoryDefinition] = {}
        for row_index, row in enumerate(self.resource.table("category").rows()):
            definition = CategoryDefinition(
                row_index=row_index,
                category_key=_u16(row, 0x08),
                rarity_capacities=tuple(_u16(row, 0x18 + rarity * 2) for rarity in range(6)),
                # RVA 0x3DBE9C selects the mode-0x12 triplet at +0x5A.
                mode12_lottery_weight=_u16(row, 0x5A),
                mode12_capacity=_u16(row, 0x5C),
                mode12_count_multiplier_key=_u16(row, 0x5E),
            )
            self._insert_unique(
                result,
                definition.category_key,
                definition,
                "category",
            )
        return result

    def _index_category_count_multipliers(
        self,
    ) -> dict[int, CategoryCountMultiplierDefinition]:
        result: dict[int, CategoryCountMultiplierDefinition] = {}
        for row_index, row in enumerate(
            self.resource.table("category_count_multiplier").rows()
        ):
            definition = CategoryCountMultiplierDefinition(
                row_index=row_index,
                lookup_key=_u32(row, 0x1C),
                multipliers=struct.unpack_from("<7f", row, 0x00),
            )
            self._insert_unique(
                result,
                definition.lookup_key,
                definition,
                "category-count-multiplier",
            )
        return result

    def _index_effects(self) -> dict[int, EffectDefinition]:
        result: dict[int, EffectDefinition] = {}
        for row_index, row in enumerate(self.resource.table("effect").rows()):
            effect = EffectDefinition(
                row_index=row_index,
                effect_id=_u16(row, 0x00),
                group_key=_u16(row, 0x02),
                flags=_u32(row, 0x1C),
                normalization_flags=_u32(row, 0x20),
                progress_threshold=_u16(row, 0x54),
                alternate_threshold=_u16(row, 0x56),
                lottery_weights=struct.unpack_from("<64H", row, 0x58),
            )
            self._insert_unique(result, effect.effect_id, effect, "effect")
        return result

    def _index_optional_multipliers(self) -> dict[int, OptionalMultiplierDefinition]:
        result: dict[int, OptionalMultiplierDefinition] = {}
        for row_index, row in enumerate(self.resource.table("optional_multiplier").rows()):
            definition = OptionalMultiplierDefinition(
                row_index=row_index,
                lookup_key=_u32(row, 0x14),
                multiplier=struct.unpack_from("<f", row, 0x18)[0],
            )
            self._insert_unique(
                result,
                definition.lookup_key,
                definition,
                "optional-multiplier",
            )
        return result

    def _index_rarity_generation(self) -> dict[int, RarityGenerationDefinition]:
        table = self.resource.table("rarity_roll")
        if table.row_count != 6:
            raise ValueError("rarity-roll table must contain exactly six rows")
        result: dict[int, RarityGenerationDefinition] = {}
        for rarity, row in enumerate(table.rows()):
            result[rarity] = RarityGenerationDefinition(
                rarity=rarity,
                minimum_roll_percent=_u32(row, 0x1C),
                maximum_roll_percent=_u32(row, 0x20),
                base_slot_count=_u32(row, 0x44),
                total_slot_count=_u32(row, 0x4C),
                promotion_trials=_u32(row, 0x58),
                promotion_probability_percent=struct.unpack_from("<f", row, 0xDC)[0],
            )
        return result

    def item(self, record_type: int) -> ScrollItemDefinition:
        try:
            return self.items_by_record_type[record_type]
        except KeyError as error:
            raise KeyError(f"unknown scroll record type 0x{record_type:04X}") from error

    def effect(self, effect_id: int) -> EffectDefinition:
        try:
            return self.effects_by_id[effect_id]
        except KeyError as error:
            raise KeyError(f"unknown effect ID 0x{effect_id:08X}") from error

    def group_for_effect(self, effect_id: int) -> EffectGroupDefinition:
        return self.groups_by_key[self.effect(effect_id).group_key]

    def category(self, category_key: int) -> CategoryDefinition:
        try:
            return self.categories_by_key[category_key]
        except KeyError as error:
            raise KeyError(f"unknown category key 0x{category_key:04X}") from error

    def category_for_effect(self, effect_id: int) -> CategoryDefinition:
        return self.category(self.group_for_effect(effect_id).category_key)

    def resolved_effect_value(
        self,
        effect_id: int,
        *,
        roll_percent: int,
        level: int,
    ) -> int:
        """Evaluate the recovered base normalization formula at RVA 0x571478."""

        if not 0 <= roll_percent <= 0xFF:
            raise ValueError("roll_percent must fit in uint8")
        if not 0 <= level <= 0xFFFF:
            raise ValueError("level must fit in uint16")
        effect = self.effect(effect_id)
        row = EffectRow(self.resource.table("effect").row(effect.row_index))
        curve_table = self.resource.table("level_curve").row_store
        return resolved_base_value(
            row,
            roll_percent,
            level,
            lambda curve_level, selector: curve_scale_from_raw_table(
                curve_table,
                curve_level,
                selector,
            ),
        )

    def optional_multiplier(self, lookup_key: int) -> float:
        try:
            return self.optional_multipliers_by_key[lookup_key].multiplier
        except KeyError as error:
            raise KeyError(
                f"unknown optional-multiplier key 0x{lookup_key:08X}"
            ) from error

    def category_count_multiplier(self, lookup_key: int, count: int) -> float:
        """Return the exact category-count multiplier used by RVA 0x3DADC4."""

        try:
            definition = self.category_count_multipliers_by_key[lookup_key]
        except KeyError as error:
            raise KeyError(
                f"unknown category-count-multiplier key 0x{lookup_key:04X}"
            ) from error
        if count < 0:
            raise ValueError("category count cannot be negative")
        index = count if count < len(definition.multipliers) else 0
        return definition.multipliers[index]

    def effects_conflict(self, left_effect_id: int, right_effect_id: int) -> bool:
        return self.group_for_effect(left_effect_id).conflicts_with(
            self.group_for_effect(right_effect_id)
        )

    def candidate_context_allowed(
        self,
        effect_id: int,
        *,
        record_type: int,
        alternate_runtime_context: bool = False,
    ) -> bool:
        """Mirror the table/flag portion of RVA 0x5788FC.

        The two external runtime predicates at RVAs 0x29859C and 0x2985C4 are
        represented by ``alternate_runtime_context``.  False is the ordinary
        title-screen/native batch context used by current scroll generation.
        """

        effect = self.effect(effect_id)
        item = self.item(record_type)
        required_context_bit = 0x80 if alternate_runtime_context else 0x40
        if not effect.flags & required_context_bit:
            return False
        if effect.flags & 0x04 and not item.candidate_item_flags & 0x0800:
            return False
        if effect.flags & 0x08 and not item.candidate_item_flags & 0x1000:
            return False
        return True

    def is_compatible(
        self,
        candidate_effect_id: int,
        *,
        existing_effect_ids: Iterable[int] = (),
        special_effect_id: int | None = None,
    ) -> bool:
        candidate_group = self.group_for_effect(candidate_effect_id)
        for existing_effect_id in existing_effect_ids:
            if candidate_group.conflicts_with(self.group_for_effect(existing_effect_id)):
                return False
        if special_effect_id not in (None, EMPTY_EFFECT_ID):
            if candidate_group.conflicts_with(self.group_for_effect(special_effect_id)):
                return False
        return True

    def native_effect_weight(
        self,
        effect_id: int,
        *,
        record_type: int,
        rarity: int,
        playthrough: int,
        restricted_destination_slot: bool = False,
        extra_selector: int = 0,
        rarity5_type_floor: int = 0,
    ) -> int:
        """Evaluate RVA 0x57896C for a captured scroll-generation context.

        Normal mode-0x12 scroll slots use the item row's `+0x15C` selector.
        The native generator substitutes selector `0x29` when destination slot
        flag `0x40` is set.  Callers must supply that slot state explicitly.
        """

        item = self.item(record_type)
        if item.mode != SCROLL_ITEM_MODE:
            raise ValueError(
                f"record type 0x{record_type:04X} is not a mode-0x12 scroll item"
            )
        if not 0 <= rarity <= 5:
            raise ValueError("rarity must be in 0..5")
        raw = self.resource.table("effect").row(self.effect(effect_id).row_index)
        row = EffectRow(raw)
        weight_slot = 0x29 if restricted_destination_slot else item.field_15c
        return effect_weight(
            row,
            weight_slot=weight_slot,
            type_class=type_class_for_record_type(
                record_type,
                rarity,
                rarity5_floor=rarity5_type_floor,
            ),
            rarity=rarity,
            progress=self.resource.playthrough_progress(playthrough),
            extra_selector=extra_selector,
            optional_multiplier_lookup=self.optional_multiplier,
        )

    def category_capacities(self, *, record_type: int, rarity: int) -> tuple[int, ...]:
        """Return the exact 32-byte capacity vector built by RVA 0x91B6E8."""

        item = self.item(record_type)
        if item.mode != SCROLL_ITEM_MODE:
            raise ValueError("category capacities support only mode-0x12 scrolls")
        if not 0 <= rarity <= 5:
            raise ValueError("rarity must be in 0..5")
        capacities = [0] * 32
        for definition in self.categories_by_key.values():
            key = definition.category_key
            if not 0 <= key < len(capacities):
                raise ValueError(f"category key 0x{key:04X} is outside the native vector")
            base = definition.rarity_capacities[rarity]
            capacities[key] = (
                base
                if key == 0x1A
                else min(base, definition.mode12_capacity)
            )
        return tuple(capacities)

    def weighted_candidate_pool(
        self,
        *,
        record_type: int,
        rarity: int,
        playthrough: int,
        destination_category_and_flags: int,
        destination_effect_flags: int,
        remaining_category_capacities: Iterable[int],
        existing_effect_ids: Iterable[int] = (),
        special_effect_id: int | None = None,
        alternate_runtime_context: bool = False,
        extra_selector: int = 0,
        rarity5_type_floor: int = 0,
    ) -> tuple[WeightedEffectCandidate, ...]:
        """Build the statically recovered portion of the per-slot native pool.

        This covers RVAs 0x57818D..0x57825B except the optional `0x572C20`
        filter used when destination effect flag `0x40` is set.  That state is
        rejected until the helper is recovered.
        """

        if not 0 <= destination_category_and_flags <= 0xFF:
            raise ValueError("destination category/flags must fit in uint8")
        if not 0 <= destination_effect_flags <= 0xFF:
            raise ValueError("destination effect flags must fit in uint8")
        if destination_effect_flags & 0x40:
            raise NotImplementedError(
                "destination effect flag 0x40 requires unrecovered RVA 0x572C20"
            )
        capacities = tuple(remaining_category_capacities)
        if len(capacities) != 32 or any(not 0 <= value <= 0xFF for value in capacities):
            raise ValueError("remaining category capacities must be 32 uint8 values")
        existing = tuple(existing_effect_ids)
        promoted = bool(destination_effect_flags & 0x04)
        requested_category = destination_category_and_flags & 0x3F
        cache_key = (
            record_type,
            rarity,
            playthrough,
            promoted,
            requested_category if not promoted else 0,
            alternate_runtime_context,
            extra_selector,
            rarity5_type_floor,
        )
        base_pool = self._base_candidate_pool_cache.get(cache_key)
        if base_pool is None:
            built: list[WeightedEffectCandidate] = []
            for effect in self.effects_by_id.values():
                if effect.row_index == 0:
                    continue
                group = self.groups_by_key[effect.group_key]
                category_key = group.category_key
                supports_promoted_slot = bool(effect.normalization_flags & 0x08)
                if supports_promoted_slot != promoted:
                    continue
                if (
                    not promoted
                    and requested_category
                    and requested_category != category_key
                ):
                    continue
                if not self.candidate_context_allowed(
                    effect.effect_id,
                    record_type=record_type,
                    alternate_runtime_context=alternate_runtime_context,
                ):
                    continue
                weight = self.native_effect_weight(
                    effect.effect_id,
                    record_type=record_type,
                    rarity=rarity,
                    playthrough=playthrough,
                    restricted_destination_slot=False,
                    extra_selector=extra_selector,
                    rarity5_type_floor=rarity5_type_floor,
                )
                if weight:
                    built.append(WeightedEffectCandidate(effect, weight))
            base_pool = tuple(built)
            self._base_candidate_pool_cache[cache_key] = base_pool

        result: list[WeightedEffectCandidate] = []
        for candidate in base_pool:
            effect = candidate.effect
            group = self.groups_by_key[effect.group_key]
            category_key = group.category_key
            if not 0 <= category_key < len(capacities) or capacities[category_key] == 0:
                continue
            if not self.is_compatible(
                effect.effect_id,
                existing_effect_ids=existing,
                special_effect_id=special_effect_id,
            ):
                continue
            result.append(candidate)
        return tuple(result)

    @staticmethod
    def select_weighted_candidate(
        candidates: Iterable[WeightedEffectCandidate],
        *,
        rng: Lcg32,
    ) -> WeightedEffectCandidate | None:
        """Mirror the inclusive weighted lottery at RVA 0x57830D..0x57833A."""

        positive = tuple(candidate for candidate in candidates if candidate.weight)
        if not positive:
            return None
        total = sum(candidate.weight for candidate in positive) & 0xFFFFFFFF
        upper_count = (total + 1) & 0xFFFFFFFF
        if upper_count == 0:
            raise OverflowError(
                "native total+1 wrapped to zero; unsupported pathological table"
            )
        ticket = rng.random_int(upper_count)
        ticket = min(ticket, total)
        for candidate in positive:
            weight = candidate.weight & 0xFFFFFFFF
            if ticket <= weight:
                return candidate
            ticket = (ticket - weight) & 0xFFFFFFFF
        return None

    def roll_effect_percentile(
        self,
        *,
        rarity: int,
        rng: Lcg32,
        use_fixed_maximum: bool = False,
    ) -> int:
        """Mirror RVA 0x980D58 for the captured rarity-roll table.

        Fresh mode-0x12 generation passes ``use_fixed_maximum=False``.  The
        alternate native call mode returns the row's maximum byte directly.
        """

        try:
            definition = self.rarity_generation[rarity]
        except KeyError as error:
            raise ValueError("rarity must be in 0..5") from error
        if use_fixed_maximum:
            return definition.maximum_roll_percent & 0xFF
        return roll_percentile(
            definition.minimum_roll_percent,
            definition.maximum_roll_percent,
            rng,
        )

    def select_promoted_slot_indexes(
        self,
        *,
        record_type: int,
        rarity: int,
        rng: Lcg32,
        category_and_flags: Iterable[int] = (0, 0, 0, 0, 0, 0, 0),
        effect_flags: Iterable[int] = (0, 0, 0, 0, 0, 0, 0),
        slot_limit: int | None = None,
        rarity5_type_floor: int = 0,
    ) -> tuple[int, ...]:
        """Mirror the standard mode-0x12 path at RVA 0x110EE26..0x110EFAD.

        Descriptor flag overrides at `r13+8` are intentionally excluded until
        their external context tables are fully captured.  This method covers
        the ordinary fresh-scroll path where those flags are zero.
        """

        item = self.item(record_type)
        if item.mode != SCROLL_ITEM_MODE:
            raise ValueError("promoted-slot selection supports only mode-0x12 scrolls")
        try:
            definition = self.rarity_generation[rarity]
        except KeyError as error:
            raise ValueError("rarity must be in 0..5") from error
        categories = tuple(category_and_flags)
        effects = tuple(effect_flags)
        if len(categories) != 7 or len(effects) != 7:
            raise ValueError("slot flag vectors must contain exactly seven entries")
        if any(not 0 <= value <= 0xFF for value in (*categories, *effects)):
            raise ValueError("slot flags must fit in uint8")
        if slot_limit is None:
            slot_limit = definition.total_slot_count
        if not 0 <= slot_limit <= 7:
            raise ValueError("slot_limit must be in 0..7")

        promoted_count = 0
        threshold = int(
            f32_mul(f32(definition.promotion_probability_percent), f32(100.0))
        )
        for _ in range(definition.promotion_trials):
            ticket = min(int(f32_mul(rng.next_float01(), f32(10000.0))), 9999)
            if ticket < threshold:
                promoted_count += 1

        type_class = type_class_for_record_type(
            record_type,
            rarity,
            rarity5_floor=rarity5_type_floor,
        )
        if type_class < 3 or promoted_count <= 0:
            return ()

        order = list(range(7))
        for position in range(7):
            swap_index = min(int(f32_mul(rng.next_float01(), f32(7.0))), 6)
            order[position], order[swap_index] = order[swap_index], order[position]

        selected: list[int] = []
        for slot_index in order:
            if slot_index >= slot_limit:
                continue
            # Mode 0x12 explicitly permits category flag 0x40 here.  Effect
            # flags 0x01/0x02 still identify slots that cannot be promoted.
            if effects[slot_index] & 0x03:
                continue
            selected.append(slot_index)
            if len(selected) == promoted_count:
                break
        return tuple(selected)


@lru_cache(maxsize=1)
def load_default_effect_generation_tables() -> EffectGenerationTableIndex:
    return EffectGenerationTableIndex(load_default_r4_finalizer_resource())


__all__ = [
    "EMPTY_EFFECT_ID",
    "SCROLL_ITEM_MODE",
    "SCROLL_RECORD_TYPES",
    "CategoryDefinition",
    "EffectDefinition",
    "EffectGenerationTableIndex",
    "EffectGroupDefinition",
    "OptionalMultiplierDefinition",
    "RarityGenerationDefinition",
    "ScrollItemDefinition",
    "WeightedEffectCandidate",
    "load_default_effect_generation_tables",
]
