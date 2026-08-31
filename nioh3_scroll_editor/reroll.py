"""Offline research model for Nioh 3 scroll reroll candidates.

The control flow in this module follows the PC v2.00.02 native path rooted at
RVA 0x20C4BD0.  It is deliberately separate from save installation: reroll
prediction still needs one small per-save eligibility set used by native RVA
0x2167804 before it can be called parity-certified.
"""

from __future__ import annotations

from dataclasses import dataclass
import struct
from typing import Iterable

from .effect_generation_tables import (
    EMPTY_EFFECT_ID,
    EffectDefinition,
    EffectGenerationTableIndex,
    load_default_effect_generation_tables,
)
from .r4_finalizer_reference import (
    EFFECT_BASE,
    EFFECT_COUNT,
    EFFECT_STRIDE,
    LCG_MASK,
    RECORD_SIZE,
    EffectSlot,
    Lcg32,
)


RECORD_TYPE_TO_PLAYTHROUGH = {
    0x1E82: 1,
    0x516D: 2,
    0xE604: 3,
    0xDD82: 4,
    0xD523: 5,
}
NATIVE_REROLL_CANDIDATE_COUNT = 5


def _u16(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def _u32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def _slot_offset(index: int) -> int:
    if not 0 <= index < EFFECT_COUNT:
        raise IndexError(index)
    return EFFECT_BASE + index * EFFECT_STRIDE


@dataclass(frozen=True, slots=True)
class RerollCandidate:
    """One native-order candidate emitted by RVA 0x20C4BD0."""

    ordinal: int
    effect_id: int
    group_key: int
    category_key: int
    weight: int
    roll_percent: int
    resolved_value: int
    rng_state_after: int


@dataclass(frozen=True, slots=True)
class RerollPrediction:
    """Candidate list plus the evidence boundary for one reroll counter."""

    displayed_seed: int
    reroll_counter: int
    selected_slot_index: int
    playthrough: int
    rng_seed: int
    rng_state_after_warmup: int
    initial_pool_size: int
    initial_total_weight: int
    candidates: tuple[RerollCandidate, ...]
    final_rng_state: int
    dynamic_gate_group_keys: tuple[int, ...]
    context_complete: bool


def derive_reroll_rng_seed(record: bytes | bytearray) -> int:
    """Return the exact seed passed to RVA 0x9DA3F8 by RVA 0x20C4BD0."""

    if len(record) != RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    return (_u32(record, 0x20) + _u16(record, 0x0C)) & LCG_MASK


def advance_reroll_counter(
    record: bytes | bytearray,
    steps: int = 1,
) -> bytes:
    """Advance record ``+0x0C`` with the native uint16 wraparound.

    Native RVAs 0x20C1718, 0x20C1764 and 0x20DD008 each increment this field
    once on their respective successful refresh, accept, and completion paths.
    """

    if len(record) != RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    if steps < 0:
        raise ValueError("steps cannot be negative")
    result = bytearray(record)
    struct.pack_into("<H", result, 0x0C, (_u16(result, 0x0C) + steps) & 0xFFFF)
    return bytes(result)


def _resolve_playthrough(record: bytes, playthrough: int | None) -> int:
    record_type = _u16(record, 0x00)
    expected = RECORD_TYPE_TO_PLAYTHROUGH.get(record_type)
    if expected is None:
        raise ValueError(f"unsupported scroll record type 0x{record_type:04X}")
    if playthrough is None:
        return expected
    if playthrough != expected:
        raise ValueError(
            f"record type 0x{record_type:04X} belongs to playthrough {expected}, "
            f"not {playthrough}"
        )
    return playthrough


def _existing_effect_ids(
    record: bytes,
    selected_slot_index: int,
    tables: EffectGenerationTableIndex,
) -> tuple[int, ...]:
    result: list[int] = []
    for index in range(EFFECT_COUNT):
        if index == selected_slot_index:
            continue
        effect_id = EffectSlot.parse(record, index).raw_id
        if effect_id == EMPTY_EFFECT_ID:
            continue
        if effect_id not in tables.effects_by_id:
            raise ValueError(
                f"slot {index + 1} contains unknown or intermediate effect "
                f"0x{effect_id:08X}; reroll prediction requires a final record"
            )
        result.append(effect_id)
    return tuple(result)


def _reroll_candidate_pool(
    record: bytes,
    selected_slot_index: int,
    *,
    playthrough: int,
    tables: EffectGenerationTableIndex,
    dynamic_gate_group_keys: frozenset[int] | None,
) -> tuple[tuple[tuple[EffectDefinition, int], ...], tuple[int, ...]]:
    """Mirror the candidate-enumeration half of native RVA 0x20C4BD0."""

    record_type = _u16(record, 0x00)
    rarity = record[0x30]
    selected_id = EffectSlot.parse(record, selected_slot_index).raw_id
    if selected_id == EMPTY_EFFECT_ID or selected_id not in tables.effects_by_id:
        raise ValueError("the selected reroll slot must contain a known final effect")
    selected_group_key = tables.effect(selected_id).group_key

    capacities = list(
        tables.category_capacities(record_type=record_type, rarity=rarity)
    )
    existing_ids = _existing_effect_ids(record, selected_slot_index, tables)
    for effect_id in existing_ids:
        category = tables.group_for_effect(effect_id).category_key
        if 0 <= category < len(capacities) and capacities[category]:
            capacities[category] -= 1

    pool: list[tuple[EffectDefinition, int]] = []
    conditional_groups: set[int] = set()
    ordered_effects = sorted(
        tables.effects_by_id.values(),
        key=lambda definition: definition.row_index,
    )
    for effect in ordered_effects:
        if effect.row_index == 0:
            continue
        group = tables.groups_by_key[effect.group_key]
        if not group.group_key or group.group_key == selected_group_key:
            continue
        category = group.category_key
        if not 0 <= category < len(capacities) or capacities[category] == 0:
            continue
        # RVA 0x20C4DC3 rejects this flag unconditionally.
        if effect.flags & 0x0200:
            continue
        if not tables.candidate_context_allowed(
            effect.effect_id,
            record_type=record_type,
        ):
            continue
        if not tables.is_compatible(
            effect.effect_id,
            existing_effect_ids=existing_ids,
        ):
            continue
        weight = tables.native_effect_weight(
            effect.effect_id,
            record_type=record_type,
            rarity=rarity,
            playthrough=playthrough,
        )
        if weight:
            # Flag 0x0100 bypasses the save-scoped group eligibility lookup at
            # RVA 0x2167804.  Record only otherwise viable conditional rows so
            # the incomplete-context report stays actionable.
            if not effect.flags & 0x0100:
                conditional_groups.add(group.group_key)
                if (
                    dynamic_gate_group_keys is None
                    or group.group_key not in dynamic_gate_group_keys
                ):
                    continue
            pool.append((effect, weight))
    return tuple(pool), tuple(sorted(conditional_groups))


def _select_candidate_index(
    pool: list[tuple[EffectDefinition, int]],
    total_weight: int,
    rng: Lcg32,
) -> int | None:
    if not pool:
        return None
    ticket = rng.random_inclusive(0, total_weight)
    for index, (_, weight) in enumerate(pool):
        if ticket <= weight:
            return index
        ticket = (ticket - weight) & LCG_MASK
    return None


def predict_reroll_candidates(
    record: bytes,
    selected_slot_index: int,
    *,
    playthrough: int | None = None,
    tables: EffectGenerationTableIndex | None = None,
    dynamic_gate_group_keys: Iterable[int] | None = None,
) -> RerollPrediction:
    """Predict the five native reroll choices for the current record state.

    ``dynamic_gate_group_keys`` is the save-scoped set queried by RVA
    0x2167804.  Passing ``None`` excludes those conditional rows and marks the
    result incomplete.  Passing an explicit set, including an empty set,
    represents a complete captured context.
    """

    if len(record) != RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    _slot_offset(selected_slot_index)
    resolved_playthrough = _resolve_playthrough(record, playthrough)
    index = tables or load_default_effect_generation_tables()
    explicit_dynamic_groups = (
        None
        if dynamic_gate_group_keys is None
        else frozenset(int(value) & 0xFFFF for value in dynamic_gate_group_keys)
    )
    base_pool, conditional_groups = _reroll_candidate_pool(
        record,
        selected_slot_index,
        playthrough=resolved_playthrough,
        tables=index,
        dynamic_gate_group_keys=explicit_dynamic_groups,
    )

    rng_seed = derive_reroll_rng_seed(record)
    rng = Lcg32(rng_seed)
    # Native RVA 0x20C4ED5 consumes one 0x10000-sized draw before the first
    # weighted ticket.  Its return value is intentionally unused.
    rng.random_int(0x10000)
    state_after_warmup = rng.state

    pool = list(base_pool)
    initial_total = sum(weight for _, weight in pool) & LCG_MASK
    candidates: list[RerollCandidate] = []
    for ordinal in range(1, NATIVE_REROLL_CANDIDATE_COUNT + 1):
        if not pool:
            break
        total_weight = sum(weight for _, weight in pool) & LCG_MASK
        selected_index = _select_candidate_index(pool, total_weight, rng)
        if selected_index is None:
            break
        effect, weight = pool[selected_index]
        roll_percent = index.roll_effect_percentile(rarity=record[0x30], rng=rng)
        group = index.groups_by_key[effect.group_key]
        candidates.append(
            RerollCandidate(
                ordinal=ordinal,
                effect_id=effect.effect_id,
                group_key=effect.group_key,
                category_key=group.category_key,
                weight=weight,
                roll_percent=roll_percent,
                resolved_value=index.resolved_effect_value(
                    effect.effect_id,
                    roll_percent=roll_percent,
                    level=_u16(record, 0x06),
                ),
                rng_state_after=rng.state,
            )
        )
        # Native RVA 0x20C4FB8 removes every variant sharing row +0x02,
        # preventing a second candidate from the same effect group.
        pool = [item for item in pool if item[0].group_key != effect.group_key]

    return RerollPrediction(
        displayed_seed=_u32(record, 0x20),
        reroll_counter=_u16(record, 0x0C),
        selected_slot_index=selected_slot_index,
        playthrough=resolved_playthrough,
        rng_seed=rng_seed,
        rng_state_after_warmup=state_after_warmup,
        initial_pool_size=len(base_pool),
        initial_total_weight=initial_total,
        candidates=tuple(candidates),
        final_rng_state=rng.state,
        dynamic_gate_group_keys=conditional_groups,
        context_complete=explicit_dynamic_groups is not None,
    )


def simulate_accept_candidate(
    record: bytes,
    selected_slot_index: int,
    candidate: RerollCandidate,
) -> bytes:
    """Apply one predicted candidate for sequence simulation only.

    This mirrors the fields needed by later candidate-pool construction and the
    counter increment at RVA 0x20C17EA.  It is not an installable canonicalizer.
    """

    if len(record) != RECORD_SIZE:
        raise ValueError("record must be exactly 0xE8 bytes")
    offset = _slot_offset(selected_slot_index)
    result = bytearray(record)
    struct.pack_into("<H", result, offset + 0x00, candidate.group_key)
    struct.pack_into("<I", result, offset + 0x04, candidate.effect_id)
    struct.pack_into("<i", result, offset + 0x08, candidate.resolved_value)
    result[offset + 0x0C] = min(candidate.roll_percent, 100)
    result[offset + 0x0D] = (result[offset + 0x0D] & 0xC0) | (
        candidate.category_key & 0x3F
    )
    return advance_reroll_counter(result)


__all__ = [
    "NATIVE_REROLL_CANDIDATE_COUNT",
    "RECORD_TYPE_TO_PLAYTHROUGH",
    "RerollCandidate",
    "RerollPrediction",
    "advance_reroll_counter",
    "derive_reroll_rng_seed",
    "predict_reroll_candidates",
    "simulate_accept_candidate",
]
