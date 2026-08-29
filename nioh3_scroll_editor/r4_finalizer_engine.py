"""Offline PC v2.00.02 rarity-4 scroll completion finalizer.

The implementation mirrors the native completion path recovered at RVAs
0x22799A8 and 0x1109270.  It is intentionally restricted to the captured
playthrough-3, rarity-4 context until independent native parity coverage is
large enough to certify additional contexts.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import struct
from typing import Iterable

from .auxiliary_generation import generate_auxiliary_mode
from .effect_generation_tables import (
    EMPTY_EFFECT_ID,
    EffectGenerationTableIndex,
    EffectGroupDefinition,
    load_default_effect_generation_tables,
)
from .r4_finalizer_reference import (
    EFFECT_BASE,
    EFFECT_COUNT,
    EFFECT_STRIDE,
    RECORD_SIZE,
    EffectRow,
    EffectSlot,
    Lcg32,
    effect_weight,
    f32,
    f32_mul,
    make_finalizer_rng,
    type_class_for_record_type,
)


SUPPORTED_RECORD_TYPE = 0xE604
SUPPORTED_RARITY = 4
SUPPORTED_PLAYTHROUGH = 3


@dataclass(frozen=True, slots=True)
class FinalizerAttemptTrace:
    target_index: int
    assigned_category: int
    weight_slot: int
    pool_size: int
    total_weight: int
    selected_effect_id: int | None
    roll_percent: int | None
    accepted: bool
    final_rng_state: int


@dataclass(frozen=True, slots=True)
class CompletionResult:
    record: bytes
    accepted_index: int | None
    attempts: tuple[FinalizerAttemptTrace, ...]


def _u16(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def _u32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def _slot_offset(index: int) -> int:
    if not 0 <= index < EFFECT_COUNT:
        raise IndexError(index)
    return EFFECT_BASE + index * EFFECT_STRIDE


class R4FinalizerEngine:
    """Deterministic offline implementation of the captured R4 finalizer."""

    def __init__(
        self,
        tables: EffectGenerationTableIndex | None = None,
        *,
        playthrough: int = SUPPORTED_PLAYTHROUGH,
    ) -> None:
        if playthrough != SUPPORTED_PLAYTHROUGH:
            raise ValueError(
                "the R4 finalizer is certified only for playthrough 3"
            )
        self.tables = tables or load_default_effect_generation_tables()
        self.playthrough = playthrough
        self._category_rows = tuple(
            sorted(
                self.tables.categories_by_key.values(),
                key=lambda definition: definition.row_index,
            )
        )
        self._base_weighted_rows: dict[
            int,
            tuple[tuple[EffectRow, EffectGroupDefinition, int], ...],
        ] = {}

    @staticmethod
    def _validate_source(source: bytes) -> None:
        if len(source) != RECORD_SIZE:
            raise ValueError("source record must be exactly 0xE8 bytes")
        record_type = _u16(source, 0x00)
        rarity = source[0x30]
        if record_type != SUPPORTED_RECORD_TYPE or rarity != SUPPORTED_RARITY:
            raise ValueError(
                "R4 finalizer requires record type 0xE604 and rarity 4"
            )

    @staticmethod
    def _clear_target_slot(record: bytearray, target_index: int) -> None:
        """Mirror the field-specific clear at RVA 0x110985E."""

        offset = _slot_offset(target_index)
        struct.pack_into("<H", record, offset + 0x00, 0)
        struct.pack_into("<I", record, offset + 0x04, EMPTY_EFFECT_ID)
        struct.pack_into("<i", record, offset + 0x08, 0)
        struct.pack_into("<H", record, offset + 0x0C, 0)
        record[offset + 0x0E] = 0
        struct.pack_into(
            "<I",
            record,
            offset + 0x10,
            _u32(record, offset + 0x10) & 0xFFFE0000,
        )
        struct.pack_into("<I", record, offset + 0x14, 0)

    def _assign_categories(
        self,
        record: bytearray,
        *,
        slot_count: int,
        rarity: int,
        rng: Lcg32,
    ) -> None:
        """Mirror the mode-0x12 path of native RVA 0x3DADC4."""

        counts = [0] * 32
        for index in range(slot_count):
            slot = EffectSlot.parse(record, index)
            if slot.raw_id == EMPTY_EFFECT_ID:
                continue
            try:
                category = self.tables.group_for_effect(slot.raw_id).category_key
            except KeyError as error:
                raise ValueError(
                    f"unknown existing effect ID 0x{slot.raw_id:08X}"
                ) from error
            offset = _slot_offset(index)
            record[offset + 0x0D] = (
                record[offset + 0x0D] & 0xC0
            ) | (category & 0x3F)
            if not (slot.effect_flags & 0x03):
                counts[category] += 1

        for index in range(slot_count):
            slot = EffectSlot.parse(record, index)
            if slot.raw_id != EMPTY_EFFECT_ID:
                continue
            offset = _slot_offset(index)
            if slot.effect_flags & 0x40:
                record[offset + 0x0D] = (
                    record[offset + 0x0D] & 0xC0
                ) | 0x1A
                counts[0x1A] += 1
                continue

            candidates: list[tuple[int, int]] = []
            total_weight = 0
            for definition in self._category_rows:
                category = definition.category_key
                capacity = min(
                    definition.rarity_capacities[rarity],
                    definition.mode12_capacity,
                )
                count = counts[category]
                if capacity <= count:
                    continue
                multiplier = f32(1.0)
                if definition.mode12_count_multiplier_key:
                    multiplier = f32(
                        self.tables.category_count_multiplier(
                            definition.mode12_count_multiplier_key,
                            count,
                        )
                    )
                weight = int(
                    f32_mul(
                        f32(definition.mode12_lottery_weight),
                        multiplier,
                    )
                )
                candidates.append((category, weight))
                total_weight = (total_weight + weight) & 0xFFFFFFFF

            if not candidates:
                continue
            ticket = rng.random_inclusive(0, total_weight)
            for category, weight in candidates:
                if ticket <= weight:
                    record[offset + 0x0D] = (
                        record[offset + 0x0D] & 0xC0
                    ) | (category & 0x3F)
                    if not (record[offset + 0x0E] & 0x03):
                        counts[category] += 1
                    break
                ticket = (ticket - weight) & 0xFFFFFFFF

    def _remaining_category_capacities(self, record: bytes) -> tuple[int, ...]:
        rarity = record[0x30]
        record_type = _u16(record, 0x00)
        capacities = list(
            self.tables.category_capacities(
                record_type=record_type,
                rarity=rarity,
            )
        )
        for index in range(EFFECT_COUNT):
            slot = EffectSlot.parse(record, index)
            if not slot.prefix_id:
                continue
            category = slot.category
            if (
                category < len(capacities)
                and capacities[category]
                and not (slot.effect_flags & 0x03)
            ):
                capacities[category] -= 1
        return tuple(capacities)

    def _conflict_effect_ids(
        self,
        record: bytes,
        prior_effect_ids: Iterable[int],
    ) -> tuple[int, ...]:
        result: list[int] = []
        for index in range(EFFECT_COUNT):
            slot = EffectSlot.parse(record, index)
            if not slot.prefix_id or slot.raw_id == EMPTY_EFFECT_ID:
                continue
            if slot.raw_id not in self.tables.effects_by_id:
                raise ValueError(
                    f"unknown existing effect ID 0x{slot.raw_id:08X}"
                )
            result.append(slot.raw_id)
        for effect_id in prior_effect_ids:
            if effect_id == EMPTY_EFFECT_ID:
                continue
            if effect_id not in self.tables.effects_by_id:
                raise ValueError(f"unknown prior effect ID 0x{effect_id:08X}")
            result.append(effect_id)
        if len(result) > 14:
            raise ValueError("native finalizer conflict set exceeds 14 rows")
        return tuple(result)

    def _weight_slot(self, source: bytes, *, reveal: bool) -> int:
        item = self.tables.item(_u16(source, 0x00))
        weight_slot = 0x3C if reveal else item.field_15c
        displayed_seed = _u32(source, 0x20)
        auxiliary_mode = generate_auxiliary_mode(
            displayed_seed,
            resource=self.tables.resource,
        ).value
        matching = tuple(
            row
            for row in self.tables.resource.table("special_context").rows()
            if row[0x28] == auxiliary_mode
        )
        if len(matching) > 1:
            raise ValueError(
                f"auxiliary mode 0x{auxiliary_mode:02X} is not unique"
            )
        if matching and matching[0][0x2F] & 0x01:
            weight_slot = 0x3D + (1 if reveal else 0)
        return weight_slot

    def _candidate_pool(
        self,
        source: bytes,
        local: bytes,
        *,
        target_index: int,
        prior_effect_ids: Iterable[int],
        weight_slot: int,
    ) -> tuple[tuple[EffectRow, int], ...]:
        record_type = _u16(source, 0x00)
        rarity = source[0x30]
        source_effect_id = EffectSlot.parse(source, target_index).raw_id
        capacities = self._remaining_category_capacities(local)
        conflicts = self._conflict_effect_ids(local, prior_effect_ids)
        conflict_groups = tuple(
            self.tables.group_for_effect(existing_id) for existing_id in conflicts
        )

        base = self._base_weighted_rows.get(weight_slot)
        if base is None:
            progress = self.tables.resource.playthrough_progress(self.playthrough)
            type_class = type_class_for_record_type(record_type, rarity)
            built: list[tuple[EffectRow, EffectGroupDefinition, int]] = []
            effects = sorted(
                self.tables.effects_by_id.values(),
                key=lambda definition: definition.row_index,
            )
            for definition in effects:
                if definition.row_index == 0:
                    continue
                if not self.tables.candidate_context_allowed(
                    definition.effect_id,
                    record_type=record_type,
                ):
                    continue
                row = EffectRow(
                    self.tables.resource.table("effect").row(definition.row_index)
                )
                weight = effect_weight(
                    row,
                    weight_slot=weight_slot,
                    type_class=type_class,
                    rarity=rarity,
                    progress=progress,
                    extra_selector=0,
                    optional_multiplier_lookup=self.tables.optional_multiplier,
                )
                if weight:
                    built.append(
                        (
                            row,
                            self.tables.groups_by_key[definition.group_key],
                            weight,
                        )
                    )
            base = tuple(built)
            self._base_weighted_rows[weight_slot] = base

        result: list[tuple[EffectRow, int]] = []
        for row, group, weight in base:
            if row.raw_id == source_effect_id:
                continue
            category = group.category_key
            if category >= len(capacities) or capacities[category] == 0:
                continue
            if any(
                group.conflicts_with(existing_group)
                for existing_group in conflict_groups
            ):
                continue
            result.append((row, weight))
        return tuple(result)

    @staticmethod
    def _select_candidate(
        pool: tuple[tuple[EffectRow, int], ...],
        rng: Lcg32,
    ) -> tuple[EffectRow | None, int]:
        if not pool:
            return None, 0
        total = sum(weight for _, weight in pool) & 0xFFFFFFFF
        upper_count = (total + 1) & 0xFFFFFFFF
        if upper_count == 0:
            raise OverflowError("native candidate total+1 wrapped to zero")
        ticket = rng.random_int(upper_count)
        ticket = min(ticket, total)
        for row, weight in pool:
            if ticket <= weight:
                return row, total
            ticket = (ticket - weight) & 0xFFFFFFFF
        return None, total

    def _write_selected_effect(
        self,
        record: bytearray,
        *,
        target_index: int,
        row: EffectRow,
        roll_percent: int,
    ) -> None:
        offset = _slot_offset(target_index)
        group = self.tables.group_for_effect(row.raw_id)
        struct.pack_into("<H", record, offset + 0x00, group.group_key)
        struct.pack_into("<I", record, offset + 0x04, row.raw_id)
        struct.pack_into(
            "<i",
            record,
            offset + 0x08,
            self.tables.resolved_effect_value(
                row.raw_id,
                roll_percent=roll_percent,
                level=_u16(record, 0x06),
            ),
        )
        record[offset + 0x0C] = roll_percent & 0xFF
        record[offset + 0x0D] = (
            record[offset + 0x0D] & 0xC0
        ) | (group.category_key & 0x3F)
        completion_flag = (row.normalization_flags >> 1) & 0x04
        record[offset + 0x0E] = (
            record[offset + 0x0E] & ~0x04
        ) | completion_flag

    def finalize_effect(
        self,
        source: bytes,
        target_index: int,
        *,
        reveal: bool = True,
        prior_effect_ids: Iterable[int] = (),
        _resolved_weight_slot: int | None = None,
    ) -> tuple[bytes, FinalizerAttemptTrace]:
        """Run one native-equivalent per-effect completion attempt."""

        self._validate_source(source)
        _slot_offset(target_index)
        local = bytearray(source)
        slot_count = sum(
            EffectSlot.parse(source, index).prefix_id != 0
            for index in range(EFFECT_COUNT)
        )
        rng = make_finalizer_rng(source, target_index)
        self._clear_target_slot(local, target_index)
        self._assign_categories(
            local,
            slot_count=slot_count,
            rarity=source[0x30],
            rng=rng,
        )
        assigned_category = EffectSlot.parse(local, target_index).category
        weight_slot = (
            self._weight_slot(source, reveal=reveal)
            if _resolved_weight_slot is None
            else _resolved_weight_slot
        )

        selected: EffectRow | None = None
        total_weight = 0
        pool_size = 0
        for attempt in range(2):
            pool = self._candidate_pool(
                source,
                bytes(local),
                target_index=target_index,
                prior_effect_ids=prior_effect_ids,
                weight_slot=weight_slot,
            )
            pool_size = len(pool)
            selected, total_weight = self._select_candidate(pool, rng)
            if selected is not None:
                break
            offset = _slot_offset(target_index)
            if attempt == 0:
                local[offset + 0x0E] &= ~0x04
            else:
                self._clear_target_slot(local, target_index)

        roll_percent: int | None = None
        if selected is not None:
            roll_percent = self.tables.roll_effect_percentile(
                rarity=source[0x30],
                rng=rng,
            )
            self._write_selected_effect(
                local,
                target_index=target_index,
                row=selected,
                roll_percent=roll_percent,
            )

        slot = EffectSlot.parse(local, target_index)
        trace = FinalizerAttemptTrace(
            target_index=target_index,
            assigned_category=assigned_category,
            weight_slot=weight_slot,
            pool_size=pool_size,
            total_weight=total_weight,
            selected_effect_id=None if selected is None else selected.raw_id,
            roll_percent=roll_percent,
            accepted=slot.completion_candidate_is_accepted,
            final_rng_state=rng.state,
        )
        return bytes(local), trace

    def build_completion_candidate(
        self,
        source: bytes,
        target_index: int,
        *,
        reveal: bool = True,
        _resolved_weight_slot: int | None = None,
    ) -> tuple[bytes, FinalizerAttemptTrace]:
        """Mirror wrapper RVA 0x22799A8, including generated prior rows."""

        self._validate_source(source)
        weight_slot = (
            self._weight_slot(source, reveal=reveal)
            if _resolved_weight_slot is None
            else _resolved_weight_slot
        )
        prior_effect_ids: list[int] = []
        for index in range(target_index):
            slot = EffectSlot.parse(source, index)
            if not slot.wrapper_prior_effect_eligible:
                continue
            candidate, _ = self.finalize_effect(
                source,
                index,
                reveal=reveal,
                prior_effect_ids=prior_effect_ids,
                _resolved_weight_slot=weight_slot,
            )
            generated = EffectSlot.parse(candidate, index).raw_id
            if generated != EMPTY_EFFECT_ID:
                prior_effect_ids.append(generated)
        return self.finalize_effect(
            source,
            target_index,
            reveal=reveal,
            prior_effect_ids=prior_effect_ids,
            _resolved_weight_slot=weight_slot,
        )

    def finalize_completion(
        self,
        source: bytes,
        *,
        reveal: bool = True,
    ) -> CompletionResult:
        """Accept the first completed slot exactly like RVA 0x10280BD."""

        self._validate_source(source)
        weight_slot = self._weight_slot(source, reveal=reveal)
        attempts: list[FinalizerAttemptTrace] = []
        for index in range(EFFECT_COUNT):
            if not EffectSlot.parse(source, index).completion_loop_eligible:
                continue
            candidate, trace = self.build_completion_candidate(
                source,
                index,
                reveal=reveal,
                _resolved_weight_slot=weight_slot,
            )
            attempts.append(trace)
            if EffectSlot.parse(
                candidate,
                index,
            ).completion_candidate_is_accepted:
                return CompletionResult(candidate, index, tuple(attempts))
        return CompletionResult(bytes(source), None, tuple(attempts))


@lru_cache(maxsize=1)
def load_default_r4_finalizer_engine() -> R4FinalizerEngine:
    """Return the shared immutable-table engine used by bulk offline replay."""

    return R4FinalizerEngine()


__all__ = [
    "CompletionResult",
    "FinalizerAttemptTrace",
    "R4FinalizerEngine",
    "load_default_r4_finalizer_engine",
    "SUPPORTED_PLAYTHROUGH",
    "SUPPORTED_RARITY",
    "SUPPORTED_RECORD_TYPE",
]
