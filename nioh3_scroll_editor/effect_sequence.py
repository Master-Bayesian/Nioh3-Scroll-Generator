"""Game-closed NG3 rarity-5 effect selection for PC v2.00.02.

This module reproduces the native Grace insertion, promoted-slot selection,
per-slot weighted candidate lottery, conflict checks, category capacities,
rarity percentile rolls, recovered base resolved-value formula, challenge
attempt count, and template-bound complete 0xE8 materialization.
"""

from __future__ import annotations

from array import array
from dataclasses import dataclass
from functools import lru_cache
import struct
import sys

from emaki_exchange import CATEGORY_TO_TYPE, EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE

from .effect_generation_tables import (
    EffectGenerationTableIndex,
    load_default_effect_generation_tables,
)
from .grace_map import (
    GraceOutputMap,
    grace_id_for_first_u16,
    load_grace_output_map,
)
from .r4_finalizer_reference import Lcg32
from .seed_accelerator import (
    build_weighted_effect_lookup_native,
    collect_ng3_r4_primary_pivot_seeds_native,
    generate_ng3_context_primary_effect_ids_native,
    generate_ng3_primary_effect_ids_native,
    generate_ng3_r4_multi_context_primary_effect_ids_native,
    last_seed_acceleration_backend,
    last_cuda_acceleration_failure,
)
from nioh3_seed_math import game_random_int_from_u16


NG3_RECORD_TYPE = 0xE604
RARITY_GROWING = 3
RARITY_FINALIZABLE = 4
RARITY_DIVINE = 5
EFFECT_SLOT_COUNT = 7
CHALLENGE_COUNT_SEED_MASK = 0x001FC07F
MIN_CHALLENGE_ATTEMPTS = 4
MAX_CHALLENGE_ATTEMPTS = 7


class EffectSequenceGenerationError(RuntimeError):
    """Raised when the recovered effect path cannot continue exactly."""


def derive_challenge_count_seed(displayed_seed: int) -> int:
    """Reproduce the scoped Seed transformation at native RVA 0x10283FA."""

    if not 0 <= displayed_seed <= 0xFFFFFFFF:
        raise ValueError("displayed_seed must fit in uint32")
    return (
        ((displayed_seed & CHALLENGE_COUNT_SEED_MASK) << 7)
        | ((displayed_seed >> 7) & CHALLENGE_COUNT_SEED_MASK)
    ) & 0xFFFFFFFF


def generate_challenge_attempt_count(displayed_seed: int) -> int:
    """Generate record byte +0x33 through native RVAs 0x10283F0/0x227FB10."""

    rng = Lcg32(derive_challenge_count_seed(displayed_seed))
    span = MAX_CHALLENGE_ATTEMPTS - MIN_CHALLENGE_ATTEMPTS + 1
    return MIN_CHALLENGE_ATTEMPTS + rng.random_int(span)


@dataclass(frozen=True, slots=True)
class GeneratedEffect:
    slot: int
    source_index: int
    effect_id: int
    roll_percent: int
    category_and_flags: int
    effect_flags: int
    candidate_count: int
    resolved_value: int
    prefix_word: int

    @property
    def category(self) -> int:
        return self.category_and_flags & 0x3F


@dataclass(frozen=True, slots=True)
class EffectSequenceResult:
    seed: int
    record_type: int
    rarity: int
    playthrough: int
    level: int
    effects: tuple[GeneratedEffect, ...]
    promoted_source_indexes: tuple[int, ...]
    random_draws: int
    final_rng_state: int
    terminal_is_special: bool = True

    @property
    def primary(self) -> GeneratedEffect:
        return self.effects[0]

    @property
    def secondaries(self) -> tuple[GeneratedEffect, ...]:
        return self.effects[1:-1] if self.terminal_is_special else self.effects[1:]

    @property
    def grace(self) -> GeneratedEffect:
        return self.effects[-1]

    @property
    def special(self) -> GeneratedEffect:
        """Return the context-specific terminal special slot."""

        return self.effects[-1]


@dataclass(frozen=True, slots=True)
class Rarity4GracePrediction:
    """Exact final Grace disposition for one NG3 rarity-4 Seed.

    Rarity 4 first assigns a Grace to physical effect slot 5. The completion
    finalizer can preserve that slot or replace it with an ordinary effect; it
    cannot draw a different Grace from its candidate pool in PC v2.00.02.
    """

    seed: int
    first_draw_u16: int
    stage_one_grace_id: int
    final_grace_id: int | None
    final_grace_slot_index: int | None
    accepted_index: int | None
    attempted_indexes: tuple[int, ...]
    selected_effect_ids: tuple[int | None, ...]
    stage_one_effect_ids: tuple[int, ...]
    final_effect_ids: tuple[int, ...]

    @property
    def retained(self) -> bool:
        return self.final_grace_id == self.stage_one_grace_id

    @property
    def replaced(self) -> bool:
        return self.final_grace_id is None


def _validate_grace_mapping(
    mapping: GraceOutputMap,
    *,
    playthrough: int = 3,
) -> None:
    if playthrough not in (3, 4, 5):
        raise ValueError("Grace effect generation requires playthrough 3, 4, or 5")
    expected_type = CATEGORY_TO_TYPE[playthrough]
    actual = (mapping.record_type, mapping.rarity, mapping.effect_slot)
    expected = (expected_type, RARITY_DIVINE, 6)
    if actual != expected:
        raise ValueError(f"unsupported Grace mapping context: {actual!r}")


def _validate_rarity4_stage_mapping(mapping: GraceOutputMap) -> None:
    actual = (mapping.record_type, mapping.rarity, mapping.effect_slot)
    expected = (NG3_RECORD_TYPE, RARITY_FINALIZABLE, 5)
    if actual != expected:
        raise ValueError(f"unsupported rarity-4 stage-one mapping context: {actual!r}")


def generate_ng3_rarity4_stage_one_effect_sequence(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> EffectSequenceResult:
    """Generate the exact NG3 rarity-4 native stage-one effect sequence.

    The fifth effect is a transient completion token. It must be passed through
    the rarity-4 finalizer before the record can be treated as canonical or
    installed.
    """

    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    if not 0 <= level <= 0xFFFF:
        raise ValueError("level must fit in uint16")
    if tables is None:
        tables = load_default_effect_generation_tables()
    if special_mapping is None:
        special_mapping = load_grace_output_map(rarity=RARITY_FINALIZABLE)
    _validate_rarity4_stage_mapping(special_mapping)

    rng = Lcg32(seed)
    special_id = grace_id_for_first_u16(rng.next_u16(), special_mapping)
    special_category = tables.category_for_effect(special_id).category_key
    source_effect_flags = (0x02, 0, 0, 0, 0, 0, 0)
    promoted = tables.select_promoted_slot_indexes(
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_FINALIZABLE,
        rng=rng,
        effect_flags=source_effect_flags,
        slot_limit=5,
    )
    promoted_set = frozenset(promoted)
    capacities = list(
        tables.category_capacities(
            record_type=NG3_RECORD_TYPE,
            rarity=RARITY_FINALIZABLE,
        )
    )
    accepted: list[int] = []
    generated: list[GeneratedEffect] = []

    for source_index in range(1, 5):
        effect_flags = 0x04 if source_index in promoted_set else 0
        category_and_flags = 0x40 if source_index == 1 else 0
        pool = tables.weighted_candidate_pool(
            record_type=NG3_RECORD_TYPE,
            rarity=RARITY_FINALIZABLE,
            playthrough=3,
            destination_category_and_flags=category_and_flags,
            destination_effect_flags=effect_flags,
            remaining_category_capacities=capacities,
            existing_effect_ids=accepted,
            special_effect_id=special_id,
        )
        selected = tables.select_weighted_candidate(pool, rng=rng)
        if selected is None:
            raise EffectSequenceGenerationError(
                "native rarity-4 candidate pool became empty"
            )
        roll_percent = tables.roll_effect_percentile(
            rarity=RARITY_FINALIZABLE,
            rng=rng,
        )
        effect_id = selected.effect.effect_id
        category = tables.group_for_effect(effect_id).category_key
        generated.append(
            GeneratedEffect(
                slot=source_index,
                source_index=source_index,
                effect_id=effect_id,
                roll_percent=roll_percent,
                category_and_flags=category_and_flags | category,
                effect_flags=effect_flags,
                candidate_count=len(pool),
                resolved_value=tables.resolved_effect_value(
                    effect_id,
                    roll_percent=roll_percent,
                    level=level,
                ),
                prefix_word=selected.effect.group_key,
            )
        )
        accepted.append(effect_id)
        if capacities[category] <= 0:
            raise EffectSequenceGenerationError(
                f"category 0x{category:02X} capacity underflow"
            )
        capacities[category] -= 1

    generated.append(
        GeneratedEffect(
            slot=5,
            source_index=0,
            effect_id=special_id,
            roll_percent=0,
            category_and_flags=special_category,
            effect_flags=0x02,
            candidate_count=0,
            resolved_value=tables.resolved_effect_value(
                special_id,
                roll_percent=0,
                level=level,
            ),
            prefix_word=tables.effect(special_id).group_key,
        )
    )
    random_draws = 1 + 1 + (7 if promoted else 0) + 4 * 3
    return EffectSequenceResult(
        seed=seed,
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_FINALIZABLE,
        playthrough=3,
        level=level,
        effects=tuple(generated),
        promoted_source_indexes=promoted,
        random_draws=random_draws,
        final_rng_state=rng.state,
    )


def generate_ng3_rarity3_effect_sequence(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
) -> EffectSequenceResult:
    """Generate the observed NG3 rarity-3 canonical effect sequence.

    Rarity 3 uses four ordinary effects followed by the fixed growing token
    ``0x0001``. Unlike rarity 4, the token is already present in revealed saved
    records and no first-draw special-result lottery is consumed.
    """

    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    if not 0 <= level <= 0xFFFF:
        raise ValueError("level must fit in uint16")
    if tables is None:
        tables = load_default_effect_generation_tables()

    special_id = 0x0001
    special_effect_flags = 0x84
    rng = Lcg32(seed)
    source_effect_flags = (0, 0, 0, 0, special_effect_flags, 0, 0)
    promoted = tables.select_promoted_slot_indexes(
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_GROWING,
        rng=rng,
        effect_flags=source_effect_flags,
        # The fixed growing token occupies serialized slot 5, but it is not a
        # source slot for the native promotion shuffle.  If index 4 is allowed
        # into the shuffle, seeds whose first accepted index is 4 incorrectly
        # consume the promotion instead of advancing to the next ordinary slot.
        slot_limit=4,
    )
    promoted_set = frozenset(promoted)
    capacities = list(
        tables.category_capacities(
            record_type=NG3_RECORD_TYPE,
            rarity=RARITY_GROWING,
        )
    )
    accepted: list[int] = []
    generated: list[GeneratedEffect] = []

    for source_index in range(4):
        effect_flags = 0x04 if source_index in promoted_set else 0
        category_and_flags = 0x40 if source_index == 0 else 0
        pool = tables.weighted_candidate_pool(
            record_type=NG3_RECORD_TYPE,
            rarity=RARITY_GROWING,
            playthrough=3,
            destination_category_and_flags=category_and_flags,
            destination_effect_flags=effect_flags,
            remaining_category_capacities=capacities,
            existing_effect_ids=accepted,
            special_effect_id=special_id,
        )
        selected = tables.select_weighted_candidate(pool, rng=rng)
        if selected is None:
            raise EffectSequenceGenerationError(
                "native rarity-3 candidate pool became empty"
            )
        roll_percent = tables.roll_effect_percentile(
            rarity=RARITY_GROWING,
            rng=rng,
        )
        effect_id = selected.effect.effect_id
        category = tables.group_for_effect(effect_id).category_key
        generated.append(
            GeneratedEffect(
                slot=source_index + 1,
                source_index=source_index,
                effect_id=effect_id,
                roll_percent=roll_percent,
                category_and_flags=category_and_flags | category,
                effect_flags=effect_flags,
                candidate_count=len(pool),
                resolved_value=tables.resolved_effect_value(
                    effect_id,
                    roll_percent=roll_percent,
                    level=level,
                ),
                prefix_word=selected.effect.group_key,
            )
        )
        accepted.append(effect_id)
        if capacities[category] <= 0:
            raise EffectSequenceGenerationError(
                f"category 0x{category:02X} capacity underflow"
            )
        capacities[category] -= 1

    special_category = tables.category_for_effect(special_id).category_key
    generated.append(
        GeneratedEffect(
            slot=5,
            source_index=4,
            effect_id=special_id,
            roll_percent=0,
            category_and_flags=special_category,
            effect_flags=special_effect_flags,
            candidate_count=0,
            resolved_value=0,
            prefix_word=tables.effect(special_id).group_key,
        )
    )
    random_draws = 1 + (7 if promoted else 0) + 4 * 3
    return EffectSequenceResult(
        seed=seed,
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_GROWING,
        playthrough=3,
        level=level,
        effects=tuple(generated),
        promoted_source_indexes=promoted,
        random_draws=random_draws,
        final_rng_state=rng.state,
    )


def generate_rarity5_grace_effect_sequence(
    seed: int,
    *,
    playthrough: int,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    grace_mapping: GraceOutputMap | None = None,
) -> EffectSequenceResult:
    """Generate ordered rarity-5 effects for one NG3-NG5 Grace context.

    The returned order is the normalized display/save order: primary,
    ordinary effects (including any promoted effect), then Grace.
    """

    if playthrough not in (3, 4, 5):
        raise ValueError("Grace effect generation requires playthrough 3, 4, or 5")
    record_type = CATEGORY_TO_TYPE[playthrough]
    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    if not 0 <= level <= 0xFFFF:
        raise ValueError("level must fit in uint16")
    if tables is None:
        tables = load_default_effect_generation_tables()
    if grace_mapping is None:
        if playthrough != 3:
            raise ValueError("NG4/NG5 effect generation requires a captured Grace map")
        grace_mapping = load_grace_output_map(rarity=RARITY_DIVINE)
    _validate_grace_mapping(grace_mapping, playthrough=playthrough)

    rng = Lcg32(seed)
    grace_id = grace_id_for_first_u16(rng.next_u16(), grace_mapping)
    grace_category = tables.category_for_effect(grace_id).category_key

    source_effect_flags = (0x02, 0, 0, 0, 0, 0, 0)
    promoted = tables.select_promoted_slot_indexes(
        record_type=record_type,
        rarity=RARITY_DIVINE,
        rng=rng,
        effect_flags=source_effect_flags,
        slot_limit=6,
    )
    promoted_set = frozenset(promoted)
    capacities = list(
        tables.category_capacities(
            record_type=record_type,
            rarity=RARITY_DIVINE,
        )
    )
    accepted: list[int] = []
    generated: list[GeneratedEffect] = []

    for source_index in range(1, 6):
        effect_flags = 0x04 if source_index in promoted_set else 0
        category_and_flags = 0x40 if source_index == 1 else 0
        pool = tables.weighted_candidate_pool(
            record_type=record_type,
            rarity=RARITY_DIVINE,
            playthrough=playthrough,
            destination_category_and_flags=category_and_flags,
            destination_effect_flags=effect_flags,
            remaining_category_capacities=capacities,
            existing_effect_ids=accepted,
            special_effect_id=grace_id,
        )
        selected = tables.select_weighted_candidate(pool, rng=rng)
        if selected is None:
            raise EffectSequenceGenerationError(
                "native candidate pool became empty before the recovered retry path"
            )
        roll_percent = tables.roll_effect_percentile(
            rarity=RARITY_DIVINE,
            rng=rng,
        )
        effect_id = selected.effect.effect_id
        category = tables.group_for_effect(effect_id).category_key
        category_and_flags |= category
        generated.append(
            GeneratedEffect(
                slot=source_index,
                source_index=source_index,
                effect_id=effect_id,
                roll_percent=roll_percent,
                category_and_flags=category_and_flags,
                effect_flags=effect_flags,
                candidate_count=len(pool),
                resolved_value=tables.resolved_effect_value(
                    effect_id,
                    roll_percent=roll_percent,
                    level=level,
                ),
                prefix_word=selected.effect.group_key,
            )
        )
        accepted.append(effect_id)
        if capacities[category] <= 0:
            raise EffectSequenceGenerationError(
                f"category 0x{category:02X} capacity underflow"
            )
        capacities[category] -= 1

    generated.append(
        GeneratedEffect(
            slot=6,
            source_index=0,
            effect_id=grace_id,
            roll_percent=0,
            category_and_flags=grace_category,
            effect_flags=0x02,
            candidate_count=0,
            resolved_value=tables.resolved_effect_value(
                grace_id,
                roll_percent=0,
                level=level,
            ),
            prefix_word=tables.effect(grace_id).group_key,
        )
    )
    # One Grace draw, one promotion trial, seven shuffle draws only when the
    # trial succeeds, then one lottery plus two percentile draws per effect.
    random_draws = 1 + 1 + (7 if promoted else 0) + 5 * 3
    return EffectSequenceResult(
        seed=seed,
        record_type=record_type,
        rarity=RARITY_DIVINE,
        playthrough=playthrough,
        level=level,
        effects=tuple(generated),
        promoted_source_indexes=promoted,
        random_draws=random_draws,
        final_rng_state=rng.state,
    )


def generate_ng3_rarity5_effect_sequence(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    grace_mapping: GraceOutputMap | None = None,
) -> EffectSequenceResult:
    """Backward-compatible verified NG3 rarity-5 entry point."""

    return generate_rarity5_grace_effect_sequence(
        seed,
        playthrough=3,
        level=level,
        tables=tables,
        grace_mapping=grace_mapping,
    )


def generate_ng3_rarity5_primary_effect(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    grace_mapping: GraceOutputMap | None = None,
) -> GeneratedEffect:
    """Generate only the exact primary effect and stop before later slots."""

    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    if not 0 <= level <= 0xFFFF:
        raise ValueError("level must fit in uint16")
    if tables is None:
        tables = load_default_effect_generation_tables()
    if grace_mapping is None:
        grace_mapping = load_grace_output_map(rarity=RARITY_DIVINE)
    _validate_grace_mapping(grace_mapping)

    rng = Lcg32(seed)
    grace_id = grace_id_for_first_u16(rng.next_u16(), grace_mapping)
    promoted = tables.select_promoted_slot_indexes(
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_DIVINE,
        rng=rng,
        effect_flags=(0x02, 0, 0, 0, 0, 0, 0),
        slot_limit=6,
    )
    effect_flags = 0x04 if 1 in promoted else 0
    pool = tables.weighted_candidate_pool(
        record_type=NG3_RECORD_TYPE,
        rarity=RARITY_DIVINE,
        playthrough=3,
        destination_category_and_flags=0x40,
        destination_effect_flags=effect_flags,
        remaining_category_capacities=tables.category_capacities(
            record_type=NG3_RECORD_TYPE,
            rarity=RARITY_DIVINE,
        ),
        existing_effect_ids=(),
        special_effect_id=grace_id,
    )
    selected = tables.select_weighted_candidate(pool, rng=rng)
    if selected is None:
        raise EffectSequenceGenerationError("native primary candidate pool is empty")
    roll_percent = tables.roll_effect_percentile(rarity=RARITY_DIVINE, rng=rng)
    effect_id = selected.effect.effect_id
    category = tables.group_for_effect(effect_id).category_key
    return GeneratedEffect(
        slot=1,
        source_index=1,
        effect_id=effect_id,
        roll_percent=roll_percent,
        category_and_flags=0x40 | category,
        effect_flags=effect_flags,
        candidate_count=len(pool),
        resolved_value=tables.resolved_effect_value(
            effect_id,
            roll_percent=roll_percent,
            level=level,
        ),
        prefix_word=selected.effect.group_key,
    )


@lru_cache(maxsize=32)
def _default_primary_pool(
    grace_id: int,
    promoted: bool,
    record_type: int = NG3_RECORD_TYPE,
    playthrough: int = 3,
) -> tuple[tuple[int, int], ...]:
    """Cache the exact seed-invariant primary lottery for one Grace/path."""

    tables = load_default_effect_generation_tables()
    pool = tables.weighted_candidate_pool(
        record_type=record_type,
        rarity=RARITY_DIVINE,
        playthrough=playthrough,
        destination_category_and_flags=0x40,
        destination_effect_flags=0x04 if promoted else 0,
        remaining_category_capacities=tables.category_capacities(
            record_type=record_type,
            rarity=RARITY_DIVINE,
        ),
        existing_effect_ids=(),
        special_effect_id=grace_id,
    )
    if not pool:
        raise EffectSequenceGenerationError("native primary candidate pool is empty")
    return tuple((candidate.effect.effect_id, candidate.weight) for candidate in pool)


@lru_cache(maxsize=32)
def _default_primary_effect_lookup(
    grace_id: int,
    promoted: bool,
    record_type: int = NG3_RECORD_TYPE,
    playthrough: int = 3,
) -> tuple[int, ...]:
    """Map every possible native u16 lottery draw to its exact effect ID."""

    pool = _default_primary_pool(grace_id, promoted, record_type, playthrough)
    native = build_weighted_effect_lookup_native(pool)
    if native is not None:
        return native
    total = sum(weight for _effect_id, weight in pool) & 0xFFFFFFFF
    upper_count = (total + 1) & 0xFFFFFFFF
    if upper_count == 0:
        raise OverflowError("native primary lottery total wrapped to zero")
    output: list[int] = []
    for value in range(0x10000):
        ticket = min(game_random_int_from_u16(value, upper_count), total)
        for effect_id, weight in pool:
            if ticket <= weight:
                output.append(effect_id)
                break
            ticket = (ticket - weight) & 0xFFFFFFFF
        else:
            raise EffectSequenceGenerationError(
                "native primary weighted lottery had no winner"
            )
    return tuple(output)


@lru_cache(maxsize=8)
def _random_int_u8_lookup(count: int) -> bytes:
    if not 1 <= count <= 0x100:
        raise ValueError("u8 lookup count must be in 1..256")
    return bytes(game_random_int_from_u16(value, count) for value in range(0x10000))


@lru_cache(maxsize=8)
def _promotion_success_lookup(probability_percent: int = 50) -> bytes:
    if not 0 <= probability_percent <= 100:
        raise ValueError("promotion probability must be in 0..100")
    threshold = probability_percent * 100
    return bytes(
        game_random_int_from_u16(value, 10_000) < threshold
        for value in range(0x10000)
    )


@lru_cache(maxsize=128)
def _ng3_rarity34_primary_pool(
    rarity: int,
    special_id: int,
    promoted: bool,
) -> tuple[tuple[int, int], ...]:
    if rarity not in (RARITY_GROWING, RARITY_FINALIZABLE):
        raise ValueError("contextual NG3 primary pools support rarity 3 or 4")
    tables = load_default_effect_generation_tables()
    pool = tables.weighted_candidate_pool(
        record_type=NG3_RECORD_TYPE,
        rarity=rarity,
        playthrough=3,
        destination_category_and_flags=0x40,
        destination_effect_flags=0x04 if promoted else 0,
        remaining_category_capacities=tables.category_capacities(
            record_type=NG3_RECORD_TYPE,
            rarity=rarity,
        ),
        existing_effect_ids=(),
        special_effect_id=special_id,
    )
    if not pool:
        raise EffectSequenceGenerationError("native rarity-3/4 primary pool is empty")
    return tuple((candidate.effect.effect_id, candidate.weight) for candidate in pool)


@lru_cache(maxsize=256)
def _ng3_rarity34_primary_lookup(
    rarity: int,
    special_id: int,
    promoted: bool,
) -> tuple[int, ...]:
    pool = _ng3_rarity34_primary_pool(rarity, special_id, promoted)
    native = build_weighted_effect_lookup_native(pool)
    if native is not None:
        return native
    total = sum(weight for _effect_id, weight in pool) & 0xFFFFFFFF
    upper_count = (total + 1) & 0xFFFFFFFF
    if upper_count == 0:
        raise OverflowError("native rarity-3/4 primary lottery total wrapped to zero")
    output: list[int] = []
    for value in range(0x10000):
        ticket = min(game_random_int_from_u16(value, upper_count), total)
        for effect_id, weight in pool:
            if ticket <= weight:
                output.append(effect_id)
                break
            ticket = (ticket - weight) & 0xFFFFFFFF
        else:
            raise EffectSequenceGenerationError(
                "native rarity-3/4 primary weighted lottery had no winner"
            )
    return tuple(output)


@lru_cache(maxsize=8)
def _ng3_r4_multi_primary_configuration(
    special_mapping: GraceOutputMap,
) -> tuple[bytes, int, bytes, bytes]:
    """Build one compact native configuration for every R4 special context."""

    _validate_rarity4_stage_mapping(special_mapping)
    special_ids = tuple(dict.fromkeys(entry.grace_id for entry in special_mapping.ranges))
    if not special_ids or len(special_ids) > 0x100:
        raise EffectSequenceGenerationError("R4 special context count is invalid")
    context_indexes = {special_id: index for index, special_id in enumerate(special_ids)}
    context_by_first_u16 = bytearray(0x10000)
    for entry in special_mapping.ranges:
        context_by_first_u16[entry.start : entry.end + 1] = bytes(
            (context_indexes[entry.grace_id],)
        ) * (entry.end - entry.start + 1)

    normal_lookups = array("I")
    promoted_lookups = array("I")
    for special_id in special_ids:
        normal_lookups.extend(
            _ng3_rarity34_primary_lookup(RARITY_FINALIZABLE, special_id, False)
        )
        promoted_lookups.extend(
            _ng3_rarity34_primary_lookup(RARITY_FINALIZABLE, special_id, True)
        )
    if normal_lookups.itemsize != 4 or promoted_lookups.itemsize != 4:
        raise RuntimeError("native uint32 lookup arrays require four-byte elements")
    if sys.byteorder != "little":
        normal_lookups.byteswap()
        promoted_lookups.byteswap()
    return (
        bytes(context_by_first_u16),
        len(special_ids),
        normal_lookups.tobytes(),
        promoted_lookups.tobytes(),
    )


def collect_ng3_r4_primary_pivot_seeds(
    values: tuple[int, ...],
    *,
    start_index: int,
    stop_index: int,
    low16_stride: int,
    primary_effect_ids: frozenset[int],
    special_mapping: GraceOutputMap | None = None,
    require_cuda: bool = False,
) -> tuple[tuple[int, int], ...] | None:
    """CUDA-compact one R4 pivot chunk before Python exact replay."""

    if special_mapping is None:
        special_mapping = load_grace_output_map(rarity=RARITY_FINALIZABLE)
    (
        context_by_first_u16,
        context_count,
        normal_lookups,
        promoted_lookups,
    ) = _ng3_r4_multi_primary_configuration(special_mapping)
    result = collect_ng3_r4_primary_pivot_seeds_native(
        values,
        start_index=start_index,
        stop_index=stop_index,
        low16_stride=low16_stride,
        allowed_effect_ids=primary_effect_ids,
        context_by_first_u16=context_by_first_u16,
        context_count=context_count,
        normal_lookups=normal_lookups,
        promoted_lookups=promoted_lookups,
        promotion_success_lookup=_promotion_success_lookup(30),
        random7_lookup=_random_int_u8_lookup(7),
    )
    if require_cuda and (
        result is None or last_seed_acceleration_backend() != "cuda"
    ):
        failure = last_cuda_acceleration_failure()
        detail = (
            f" Native CUDA stage: {failure[0]}; error code: {failure[1]}."
            if failure is not None
            else ""
        )
        raise RuntimeError(
            "CUDA rarity-4 primary pivot matcher is unavailable; "
            "CPU fallback is disabled."
            + detail
        )
    return result


def generate_ng3_rarity34_primary_effect_ids(
    seeds: tuple[int, ...],
    *,
    rarity: int,
    special_mapping: GraceOutputMap | None = None,
) -> tuple[int, ...]:
    """Return exact batched NG3 rarity-3/4 primary IDs with CUDA when available."""

    if rarity not in (RARITY_GROWING, RARITY_FINALIZABLE):
        raise ValueError("rarity must be 3 or 4")
    if not seeds:
        return ()
    if any(not 0 <= seed <= 0xFFFFFFFF for seed in seeds):
        raise ValueError("every seed must fit in uint32")
    if rarity == RARITY_GROWING:
        grouped: dict[int, list[tuple[int, int]]] = {0x0001: list(enumerate(seeds))}
        pre_promotion_draws = 0
        promotion_probability = 10
        slot_limit = 4
        excluded_slot_mask = 0
        primary_source_index = 0
    else:
        if special_mapping is None:
            special_mapping = load_grace_output_map(rarity=RARITY_FINALIZABLE)
        _validate_rarity4_stage_mapping(special_mapping)
        (
            context_by_first_u16,
            context_count,
            normal_lookups,
            promoted_lookups,
        ) = _ng3_r4_multi_primary_configuration(special_mapping)
        generated = generate_ng3_r4_multi_context_primary_effect_ids_native(
            seeds,
            context_by_first_u16=context_by_first_u16,
            context_count=context_count,
            normal_lookups=normal_lookups,
            promoted_lookups=promoted_lookups,
            promotion_success_lookup=_promotion_success_lookup(30),
            random7_lookup=_random_int_u8_lookup(7),
        )
        if generated is not None:
            return generated
        grouped = {}
        for index, seed in enumerate(seeds):
            rng = Lcg32(seed)
            special_id = grace_id_for_first_u16(rng.next_u16(), special_mapping)
            grouped.setdefault(special_id, []).append((index, seed))
        pre_promotion_draws = 1
        promotion_probability = 30
        slot_limit = 5
        excluded_slot_mask = 0x01
        primary_source_index = 1

    output = [0] * len(seeds)
    for special_id, indexed_seeds in grouped.items():
        group_seeds = tuple(seed for _index, seed in indexed_seeds)
        generated = generate_ng3_context_primary_effect_ids_native(
            group_seeds,
            normal_lookup=_ng3_rarity34_primary_lookup(rarity, special_id, False),
            promoted_lookup=_ng3_rarity34_primary_lookup(rarity, special_id, True),
            promotion_success_lookup=_promotion_success_lookup(promotion_probability),
            random7_lookup=_random_int_u8_lookup(7),
            pre_promotion_draws=pre_promotion_draws,
            slot_limit=slot_limit,
            excluded_slot_mask=excluded_slot_mask,
            primary_source_index=primary_source_index,
        )
        if generated is None:
            generator = (
                generate_ng3_rarity3_effect_sequence
                if rarity == RARITY_GROWING
                else generate_ng3_rarity4_stage_one_effect_sequence
            )
            generated = tuple(generator(seed).primary.effect_id for seed in group_seeds)
        for (index, _seed), effect_id in zip(indexed_seeds, generated, strict=True):
            output[index] = effect_id
    return tuple(output)


def generate_rarity5_grace_primary_effect_id(
    seed: int,
    *,
    playthrough: int,
    grace_mapping: GraceOutputMap | None = None,
) -> int:
    """Return the exact primary ID through a cached path-specific lottery.

    The primary pool is invariant for a fixed Grace and promotion status. This
    avoids rebuilding and conflict-filtering the same 40-row pool for every
    Seed while retaining the native promotion trial, seven-draw shuffle, and
    inclusive weighted lottery semantics.
    """

    if playthrough not in (3, 4, 5):
        raise ValueError("Grace primary generation requires playthrough 3, 4, or 5")
    record_type = CATEGORY_TO_TYPE[playthrough]
    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    if grace_mapping is None:
        if playthrough != 3:
            raise ValueError("NG4/NG5 primary generation requires a captured Grace map")
        grace_mapping = load_grace_output_map(rarity=RARITY_DIVINE)
    _validate_grace_mapping(grace_mapping, playthrough=playthrough)

    rng = Lcg32(seed)
    grace_id = grace_id_for_first_u16(rng.next_u16(), grace_mapping)
    promotion_ticket = game_random_int_from_u16(rng.next_u16(), 10_000)
    promoted = promotion_ticket < 5_000
    primary_promoted = False
    if promoted:
        order = list(range(7))
        for position in range(7):
            swap_index = game_random_int_from_u16(rng.next_u16(), 7)
            order[position], order[swap_index] = order[swap_index], order[position]
        # Slot index 0 is the unpromotable Grace source, index 6 is outside the
        # six-slot limit. Rarity 5 has exactly one promotion trial.
        selected_index = next(index for index in order if 0 < index < 6)
        primary_promoted = selected_index == 1

    pool = _default_primary_pool(
        grace_id,
        primary_promoted,
        record_type,
        playthrough,
    )
    total = sum(weight for _effect_id, weight in pool) & 0xFFFFFFFF
    upper_count = (total + 1) & 0xFFFFFFFF
    if upper_count == 0:
        raise OverflowError("native primary lottery total wrapped to zero")
    ticket = game_random_int_from_u16(rng.next_u16(), upper_count)
    ticket = min(ticket, total)
    for effect_id, weight in pool:
        if ticket <= weight:
            return effect_id
        ticket = (ticket - weight) & 0xFFFFFFFF
    raise EffectSequenceGenerationError("native primary weighted lottery had no winner")


def generate_ng3_rarity5_primary_effect_id(
    seed: int,
    *,
    grace_mapping: GraceOutputMap | None = None,
) -> int:
    """Backward-compatible verified NG3 primary-ID entry point."""

    return generate_rarity5_grace_primary_effect_id(
        seed,
        playthrough=3,
        grace_mapping=grace_mapping,
    )


def generate_rarity5_grace_primary_effect_ids(
    seeds: tuple[int, ...],
    *,
    playthrough: int,
    grace_id: int,
    grace_mapping: GraceOutputMap | None = None,
) -> tuple[int, ...]:
    """Return exact primary IDs for one fixed-Grace Seed batch.

    The caller must provide Seeds from the inverse family for ``grace_id``.
    The native implementation uses CUDA when available and a native CPU loop
    otherwise. Batches are deliberately capped by the native boundary so a
    solver cannot accidentally materialize an unbounded candidate family.
    """

    if playthrough not in (3, 4, 5):
        raise ValueError("Grace primary generation requires playthrough 3, 4, or 5")
    record_type = CATEGORY_TO_TYPE[playthrough]
    if not seeds:
        return ()
    if any(not 0 <= seed <= 0xFFFFFFFF for seed in seeds):
        raise ValueError("every seed must fit in uint32")
    if not 0 <= grace_id <= 0xFFFFFFFF:
        raise ValueError("grace_id must fit in uint32")
    if grace_mapping is None:
        if playthrough != 3:
            raise ValueError("NG4/NG5 primary generation requires a captured Grace map")
        grace_mapping = load_grace_output_map(rarity=RARITY_DIVINE)
    _validate_grace_mapping(grace_mapping, playthrough=playthrough)

    native = generate_ng3_primary_effect_ids_native(
        seeds,
        normal_lookup=_default_primary_effect_lookup(
            grace_id, False, record_type, playthrough
        ),
        promoted_lookup=_default_primary_effect_lookup(
            grace_id, True, record_type, playthrough
        ),
        promotion_success_lookup=_promotion_success_lookup(50),
        random7_lookup=_random_int_u8_lookup(7),
    )
    if native is not None:
        return native
    return tuple(
        generate_rarity5_grace_primary_effect_id(
            seed,
            playthrough=playthrough,
            grace_mapping=grace_mapping,
        )
        for seed in seeds
    )


def generate_rarity5_any_grace_primary_effect_ids(
    seeds: tuple[int, ...],
    *,
    playthrough: int,
    grace_mapping: GraceOutputMap | None = None,
) -> tuple[int, ...]:
    """Return exact primary IDs for a batch without constraining its Grace.

    Seeds are grouped by their native draw-1 Grace. Each group then uses the
    existing CUDA-capable fixed-Grace kernel, preserving exact game behavior
    while allowing product searches whose Grace filter is set to any.
    """

    if not seeds:
        return ()
    if grace_mapping is None:
        if playthrough != 3:
            raise ValueError("NG4/NG5 primary generation requires a captured Grace map")
        grace_mapping = load_grace_output_map(rarity=RARITY_DIVINE)
    _validate_grace_mapping(grace_mapping, playthrough=playthrough)
    grouped: dict[int, list[tuple[int, int]]] = {}
    for index, seed in enumerate(seeds):
        rng = Lcg32(seed)
        grace_id = grace_id_for_first_u16(rng.next_u16(), grace_mapping)
        grouped.setdefault(grace_id, []).append((index, seed))
    output = [0] * len(seeds)
    for grace_id, indexed_seeds in grouped.items():
        group_seeds = tuple(seed for _index, seed in indexed_seeds)
        generated = generate_rarity5_grace_primary_effect_ids(
            group_seeds,
            playthrough=playthrough,
            grace_id=grace_id,
            grace_mapping=grace_mapping,
        )
        for (index, _seed), effect_id in zip(indexed_seeds, generated, strict=True):
            output[index] = effect_id
    return tuple(output)


def generate_ng3_rarity5_primary_effect_ids(
    seeds: tuple[int, ...],
    *,
    grace_id: int,
    grace_mapping: GraceOutputMap | None = None,
) -> tuple[int, ...]:
    """Backward-compatible verified NG3 batched primary-ID entry point."""

    return generate_rarity5_grace_primary_effect_ids(
        seeds,
        playthrough=3,
        grace_id=grace_id,
        grace_mapping=grace_mapping,
    )


def serialize_rarity5_grace_effect_slots(
    result: EffectSequenceResult,
    *,
    tables: EffectGenerationTableIndex | None = None,
) -> bytes:
    """Serialize seven canonical slots for an NG3-NG5 rarity-5 result."""

    if (
        result.playthrough not in (3, 4, 5)
        or result.record_type != CATEGORY_TO_TYPE[result.playthrough]
        or result.rarity != RARITY_DIVINE
        or len(result.effects) != 6
    ):
        raise ValueError("unsupported effect-sequence context")
    if tables is None:
        tables = load_default_effect_generation_tables()
    output = bytearray(EFFECT_SLOT_COUNT * EFFECT_STRIDE)
    for index, effect in enumerate(result.effects):
        metadata = (
            effect.roll_percent
            | (effect.category_and_flags << 8)
            | (effect.effect_flags << 16)
        )
        struct.pack_into(
            "<6I",
            output,
            index * EFFECT_STRIDE,
            effect.prefix_word,
            effect.effect_id,
            effect.resolved_value & 0xFFFFFFFF,
            metadata,
            0,
            0,
        )
    struct.pack_into(
        "<6I",
        output,
        6 * EFFECT_STRIDE,
        0,
        0xFFFFFFFF,
        0,
        0,
        0,
        0,
    )
    return bytes(output)


def serialize_ng3_rarity4_stage_one_effect_slots(
    result: EffectSequenceResult,
) -> bytes:
    """Serialize the seven slots of an NG3 rarity-4 stage-one result."""

    if (
        result.playthrough != 3
        or result.record_type != NG3_RECORD_TYPE
        or result.rarity != RARITY_FINALIZABLE
        or len(result.effects) != 5
    ):
        raise ValueError("unsupported rarity-4 stage-one context")
    output = bytearray(EFFECT_SLOT_COUNT * EFFECT_STRIDE)
    for index, effect in enumerate(result.effects):
        metadata = (
            effect.roll_percent
            | (effect.category_and_flags << 8)
            | (effect.effect_flags << 16)
        )
        struct.pack_into(
            "<6I",
            output,
            index * EFFECT_STRIDE,
            effect.prefix_word,
            effect.effect_id,
            effect.resolved_value & 0xFFFFFFFF,
            metadata,
            0,
            0,
        )
    for index in range(len(result.effects), EFFECT_SLOT_COUNT):
        struct.pack_into(
            "<6I",
            output,
            index * EFFECT_STRIDE,
            0,
            0xFFFFFFFF,
            0,
            0,
            0,
            0,
        )
    return bytes(output)


def serialize_ng3_rarity3_effect_slots(result: EffectSequenceResult) -> bytes:
    """Serialize the observed seven-slot NG3 rarity-3 canonical layout."""

    if (
        result.playthrough != 3
        or result.record_type != NG3_RECORD_TYPE
        or result.rarity != RARITY_GROWING
        or len(result.effects) != 5
    ):
        raise ValueError("unsupported rarity-3 effect context")
    output = bytearray(EFFECT_SLOT_COUNT * EFFECT_STRIDE)
    for index, effect in enumerate(result.effects):
        metadata = (
            effect.roll_percent
            | (effect.category_and_flags << 8)
            | (effect.effect_flags << 16)
        )
        struct.pack_into(
            "<6I",
            output,
            index * EFFECT_STRIDE,
            effect.prefix_word,
            effect.effect_id,
            effect.resolved_value & 0xFFFFFFFF,
            metadata,
            0,
            0,
        )
    for index in range(len(result.effects), EFFECT_SLOT_COUNT):
        struct.pack_into(
            "<6I",
            output,
            index * EFFECT_STRIDE,
            0,
            0xFFFFFFFF,
            0,
            0,
            0,
            0,
        )
    return bytes(output)


def serialize_ng3_rarity5_effect_slots(
    result: EffectSequenceResult,
    *,
    tables: EffectGenerationTableIndex | None = None,
) -> bytes:
    """Backward-compatible verified NG3 serializer."""

    if result.playthrough != 3 or result.record_type != NG3_RECORD_TYPE:
        raise ValueError("unsupported effect-sequence context")
    return serialize_rarity5_grace_effect_slots(result, tables=tables)


def materialize_ng3_rarity5_record(
    template: bytes,
    *,
    seed: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
    tables: EffectGenerationTableIndex | None = None,
    grace_mapping: GraceOutputMap | None = None,
) -> tuple[bytes, EffectSequenceResult]:
    """Materialize a canonical record while preserving template lineage fields."""

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    if struct.unpack_from("<H", template, 0)[0] != NG3_RECORD_TYPE:
        raise ValueError("template must be a native NG3 0xE604 record")
    for name, value, maximum in (
        ("seed", seed, 0xFFFFFFFF),
        ("level", level, 0xFFFF),
        ("recommended_level", recommended_level, 0xFFFF),
        ("transfer_count", transfer_count, 0xFFFFFFFF),
        ("generation_serial", generation_serial, 0xFFFFFFFF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name} is outside its supported range")
    if tables is None:
        tables = load_default_effect_generation_tables()
    result = generate_ng3_rarity5_effect_sequence(
        seed,
        level=level,
        tables=tables,
        grace_mapping=grace_mapping,
    )
    record = bytearray(template)
    struct.pack_into("<H", record, 0x06, level)
    struct.pack_into("<H", record, 0x08, level)
    struct.pack_into("<H", record, 0x10, recommended_level)
    struct.pack_into("<H", record, 0x12, recommended_level)
    struct.pack_into("<I", record, 0x20, seed)
    struct.pack_into("<I", record, 0x28, generation_serial)
    record[0x30] = RARITY_DIVINE
    record[0x31] = RARITY_DIVINE
    record[0x33] = generate_challenge_attempt_count(seed)
    record[EFFECT_START : EFFECT_START + EFFECT_SLOT_COUNT * EFFECT_STRIDE] = (
        serialize_ng3_rarity5_effect_slots(result, tables=tables)
    )
    struct.pack_into("<I", record, 0xDC, transfer_count)
    return bytes(record), result


def materialize_ng3_rarity4_stage_one_record(
    template: bytes,
    *,
    seed: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> tuple[bytes, EffectSequenceResult]:
    """Materialize a deterministic NG3 rarity-4 stage-one record.

    This output is an internal native generation stage and is deliberately not
    a safe install artifact. Use ``materialize_ng3_rarity4_final_record`` for a
    canonical completed record.
    """

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    if struct.unpack_from("<H", template, 0)[0] != NG3_RECORD_TYPE:
        raise ValueError("template must be a native NG3 0xE604 record")
    for name, value, maximum in (
        ("seed", seed, 0xFFFFFFFF),
        ("level", level, 0xFFFF),
        ("recommended_level", recommended_level, 0xFFFF),
        ("transfer_count", transfer_count, 0xFFFFFFFF),
        ("generation_serial", generation_serial, 0xFFFFFFFF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name} is outside its supported range")
    if tables is None:
        tables = load_default_effect_generation_tables()
    result = generate_ng3_rarity4_stage_one_effect_sequence(
        seed,
        level=level,
        tables=tables,
        special_mapping=special_mapping,
    )
    record = bytearray(template)
    # +0x0C is the R4 completion salt, not a lineage field. Existing completed
    # scrolls can carry a non-zero value here; inheriting it from the donor
    # template changes the finalizer RNG stream and makes installation disagree
    # with the game-closed preview. A newly generated stage-one record starts
    # with the canonical zero salt used by the native receive path.
    struct.pack_into("<H", record, 0x0C, 0)
    struct.pack_into("<H", record, 0x06, level)
    struct.pack_into("<H", record, 0x08, level)
    struct.pack_into("<H", record, 0x10, recommended_level)
    struct.pack_into("<H", record, 0x12, recommended_level)
    struct.pack_into("<I", record, 0x20, seed)
    struct.pack_into("<I", record, 0x28, generation_serial)
    record[0x30] = RARITY_FINALIZABLE
    record[0x31] = RARITY_FINALIZABLE
    record[0x33] = generate_challenge_attempt_count(seed)
    record[EFFECT_START : EFFECT_START + EFFECT_SLOT_COUNT * EFFECT_STRIDE] = (
        serialize_ng3_rarity4_stage_one_effect_slots(result)
    )
    struct.pack_into("<I", record, 0xDC, transfer_count)
    return bytes(record), result


def materialize_ng3_rarity3_record(
    template: bytes,
    *,
    seed: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
    tables: EffectGenerationTableIndex | None = None,
) -> tuple[bytes, EffectSequenceResult]:
    """Materialize the observed NG3 rarity-3 canonical record layout."""

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    if struct.unpack_from("<H", template, 0)[0] != NG3_RECORD_TYPE:
        raise ValueError("template must be a native NG3 0xE604 record")
    for name, value, maximum in (
        ("seed", seed, 0xFFFFFFFF),
        ("level", level, 0xFFFF),
        ("recommended_level", recommended_level, 0xFFFF),
        ("transfer_count", transfer_count, 0xFFFFFFFF),
        ("generation_serial", generation_serial, 0xFFFFFFFF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name} is outside its supported range")
    if tables is None:
        tables = load_default_effect_generation_tables()
    result = generate_ng3_rarity3_effect_sequence(
        seed,
        level=level,
        tables=tables,
    )
    record = bytearray(template)
    struct.pack_into("<H", record, 0x06, level)
    struct.pack_into("<H", record, 0x08, level)
    struct.pack_into("<H", record, 0x10, recommended_level)
    struct.pack_into("<H", record, 0x12, recommended_level)
    struct.pack_into("<I", record, 0x20, seed)
    struct.pack_into("<I", record, 0x28, generation_serial)
    record[0x30] = RARITY_GROWING
    record[0x31] = RARITY_GROWING
    record[0x33] = generate_challenge_attempt_count(seed)
    record[EFFECT_START : EFFECT_START + EFFECT_SLOT_COUNT * EFFECT_STRIDE] = (
        serialize_ng3_rarity3_effect_slots(result)
    )
    struct.pack_into("<I", record, 0xDC, transfer_count)
    return bytes(record), result


def _final_effect_sequence_from_record(
    record: bytes,
    *,
    seed: int,
    rarity: int,
    level: int,
    effect_count: int,
    final_rng_state: int,
    terminal_is_special: bool,
) -> EffectSequenceResult:
    effects: list[GeneratedEffect] = []
    promoted_indexes: list[int] = []
    for index in range(effect_count):
        offset = EFFECT_START + index * EFFECT_STRIDE
        prefix, effect_id, value, metadata, _tail_0, _tail_1 = struct.unpack_from(
            "<6I", record, offset
        )
        effect_flags = (metadata >> 16) & 0xFF
        if effect_flags & 0x04:
            promoted_indexes.append(index)
        effects.append(
            GeneratedEffect(
                slot=index + 1,
                source_index=index,
                effect_id=effect_id,
                roll_percent=metadata & 0xFF,
                category_and_flags=(metadata >> 8) & 0xFF,
                effect_flags=effect_flags,
                candidate_count=0,
                resolved_value=value,
                prefix_word=prefix,
            )
        )
    return EffectSequenceResult(
        seed=seed,
        record_type=NG3_RECORD_TYPE,
        rarity=rarity,
        playthrough=3,
        level=level,
        effects=tuple(effects),
        promoted_source_indexes=tuple(promoted_indexes),
        random_draws=0,
        final_rng_state=final_rng_state,
        terminal_is_special=terminal_is_special,
    )


def materialize_ng3_rarity4_final_record(
    template: bytes,
    *,
    seed: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> tuple[bytes, EffectSequenceResult]:
    """Generate and complete one canonical NG3 rarity-4 record offline."""

    if tables is None:
        tables = load_default_effect_generation_tables()
    stage_one, sequence = materialize_ng3_rarity4_stage_one_record(
        template,
        seed=seed,
        level=level,
        recommended_level=recommended_level,
        transfer_count=transfer_count,
        generation_serial=generation_serial,
        tables=tables,
        special_mapping=special_mapping,
    )
    # Local import keeps the finalizer independent from the stage generator.
    from .r4_finalizer_engine import load_default_r4_finalizer_engine

    default_tables = load_default_effect_generation_tables()
    if tables is default_tables:
        finalizer = load_default_r4_finalizer_engine()
    else:
        from .r4_finalizer_engine import R4FinalizerEngine

        finalizer = R4FinalizerEngine(tables=tables)
    completed = finalizer.finalize_completion(stage_one)
    final_state = (
        completed.attempts[-1].final_rng_state
        if completed.attempts
        else sequence.final_rng_state
    )
    final_sequence = _final_effect_sequence_from_record(
        completed.record,
        seed=seed,
        rarity=RARITY_FINALIZABLE,
        level=level,
        effect_count=5,
        final_rng_state=final_state,
        # R4 stage one reserves physical slot 5 for a Grace candidate.  The
        # completion loop can finish any earlier ordinary slot; in that case
        # the terminal Grace survives into the canonical final record.  Only
        # an accepted completion at zero-based index 4 replaces it with an
        # ordinary effect.
        terminal_is_special=completed.accepted_index != 4,
    )
    return completed.record, final_sequence


def generate_ng3_rarity4_final_effect_sequence(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> EffectSequenceResult:
    """Generate one final NG3 rarity-4 effect sequence without a game or save."""

    template = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", template, 0x00, NG3_RECORD_TYPE)
    _record, result = materialize_ng3_rarity4_final_record(
        bytes(template),
        seed=seed,
        level=level,
        recommended_level=0,
        transfer_count=0,
        generation_serial=0,
        tables=tables,
        special_mapping=special_mapping,
    )
    return result


def predict_ng3_rarity4_final_grace(
    seed: int,
    *,
    level: int = 180,
    tables: EffectGenerationTableIndex | None = None,
    special_mapping: GraceOutputMap | None = None,
) -> Rarity4GracePrediction:
    """Predict the final NG3 rarity-4 Grace exactly without a game process.

    The draw-1 map identifies the stage-one Grace. Exact finalizer replay then
    determines whether completion accepts physical slot 5 and replaces that
    Grace. The current native table excludes every mapped Grace ID from the
    finalizer's ordinary candidate namespace, so a final Grace can only be the
    original stage-one Grace or absent.
    """

    if tables is None:
        tables = load_default_effect_generation_tables()
    if special_mapping is None:
        special_mapping = load_grace_output_map(rarity=RARITY_FINALIZABLE)
    _validate_rarity4_stage_mapping(special_mapping)

    first_draw_rng = Lcg32(seed)
    first_draw_u16 = first_draw_rng.next_u16()
    template = bytearray(SCROLL_RECORD_SIZE)
    struct.pack_into("<H", template, 0x00, NG3_RECORD_TYPE)
    stage_one, stage_sequence = materialize_ng3_rarity4_stage_one_record(
        bytes(template),
        seed=seed,
        level=level,
        recommended_level=0,
        transfer_count=0,
        generation_serial=0,
        tables=tables,
        special_mapping=special_mapping,
    )

    from .r4_finalizer_engine import (
        R4FinalizerEngine,
        load_default_r4_finalizer_engine,
    )

    default_tables = load_default_effect_generation_tables()
    finalizer = (
        load_default_r4_finalizer_engine()
        if tables is default_tables
        else R4FinalizerEngine(tables=tables)
    )
    completion = finalizer.finalize_completion(stage_one)
    effect_count = len(stage_sequence.effects)
    stage_effect_ids = tuple(effect.effect_id for effect in stage_sequence.effects)
    final_effect_ids = tuple(
        struct.unpack_from(
            "<I",
            completion.record,
            EFFECT_START + index * EFFECT_STRIDE + 0x04,
        )[0]
        for index in range(effect_count)
    )
    stage_grace_id = stage_sequence.grace.effect_id
    mapped_grace_ids = frozenset(
        entry.grace_id for entry in special_mapping.ranges
    )
    final_grace_indexes = tuple(
        index
        for index, effect_id in enumerate(final_effect_ids)
        if effect_id in mapped_grace_ids
    )
    if len(final_grace_indexes) > 1:
        raise AssertionError("rarity-4 finalizer produced multiple Grace slots")
    final_grace_slot_index = (
        final_grace_indexes[0] if final_grace_indexes else None
    )
    final_grace_id = (
        final_effect_ids[final_grace_slot_index]
        if final_grace_slot_index is not None
        else None
    )
    expected_final_grace_id = (
        None if completion.accepted_index == 4 else stage_grace_id
    )
    if final_grace_id != expected_final_grace_id:
        raise AssertionError(
            "rarity-4 finalizer violated the certified Grace preservation invariant"
        )

    return Rarity4GracePrediction(
        seed=seed,
        first_draw_u16=first_draw_u16,
        stage_one_grace_id=stage_grace_id,
        final_grace_id=final_grace_id,
        final_grace_slot_index=final_grace_slot_index,
        accepted_index=completion.accepted_index,
        attempted_indexes=tuple(
            attempt.target_index for attempt in completion.attempts
        ),
        selected_effect_ids=tuple(
            attempt.selected_effect_id for attempt in completion.attempts
        ),
        stage_one_effect_ids=stage_effect_ids,
        final_effect_ids=final_effect_ids,
    )


def generate_ng3_certified_effect_sequence(
    seed: int,
    *,
    rarity: int,
    level: int = 180,
) -> EffectSequenceResult:
    """Generate a certified PC v2.00.02 NG3 rarity-3/4/5 effect sequence."""

    if rarity == RARITY_GROWING:
        return generate_ng3_rarity3_effect_sequence(seed, level=level)
    if rarity == RARITY_FINALIZABLE:
        return generate_ng3_rarity4_final_effect_sequence(seed, level=level)
    if rarity == RARITY_DIVINE:
        return generate_ng3_rarity5_effect_sequence(seed, level=level)
    raise ValueError("certified game-closed NG3 generation supports rarity 3, 4, or 5")


def materialize_ng3_certified_record(
    template: bytes,
    *,
    seed: int,
    rarity: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
) -> tuple[bytes, EffectSequenceResult]:
    """Bind one certified NG3 rarity-3/4/5 result to a real save template."""

    common = {
        "seed": seed,
        "level": level,
        "recommended_level": recommended_level,
        "transfer_count": transfer_count,
        "generation_serial": generation_serial,
    }
    if rarity == RARITY_GROWING:
        return materialize_ng3_rarity3_record(template, **common)
    if rarity == RARITY_FINALIZABLE:
        return materialize_ng3_rarity4_final_record(template, **common)
    if rarity == RARITY_DIVINE:
        return materialize_ng3_rarity5_record(template, **common)
    raise ValueError("certified NG3 materialization supports rarity 3, 4, or 5")


def materialize_ng3_certified_install_record(
    template: bytes,
    *,
    seed: int,
    rarity: int,
    level: int,
    recommended_level: int,
    transfer_count: int,
    generation_serial: int,
) -> tuple[bytes, EffectSequenceResult]:
    """Build the record the game must receive plus its post-reveal preview.

    Rarity-4 acquisition has two native stages. The save must receive the
    stage-one record so the game's reveal path runs the completion pass exactly
    once. Writing the already completed record makes the game complete it a
    second time and can change another effect slot, including the Grace slot.

    Other certified rarities are already stored in their installable form.
    """

    common = {
        "seed": seed,
        "level": level,
        "recommended_level": recommended_level,
        "transfer_count": transfer_count,
        "generation_serial": generation_serial,
    }
    if rarity != RARITY_FINALIZABLE:
        return materialize_ng3_certified_record(template, rarity=rarity, **common)

    install_record, _stage_one = materialize_ng3_rarity4_stage_one_record(
        template,
        **common,
    )
    _completed_record, completed_preview = materialize_ng3_rarity4_final_record(
        template,
        **common,
    )
    return install_record, completed_preview


__all__ = [
    "EffectSequenceGenerationError",
    "EffectSequenceResult",
    "GeneratedEffect",
    "Rarity4GracePrediction",
    "NG3_RECORD_TYPE",
    "RARITY_GROWING",
    "RARITY_FINALIZABLE",
    "RARITY_DIVINE",
    "derive_challenge_count_seed",
    "generate_challenge_attempt_count",
    "collect_ng3_r4_primary_pivot_seeds",
    "generate_ng3_rarity5_effect_sequence",
    "generate_ng3_rarity5_primary_effect",
    "generate_ng3_rarity5_primary_effect_id",
    "generate_ng3_rarity5_primary_effect_ids",
    "generate_ng3_rarity3_effect_sequence",
    "generate_ng3_rarity34_primary_effect_ids",
    "generate_ng3_rarity4_final_effect_sequence",
    "generate_ng3_rarity4_stage_one_effect_sequence",
    "predict_ng3_rarity4_final_grace",
    "generate_ng3_certified_effect_sequence",
    "generate_rarity5_grace_effect_sequence",
    "generate_rarity5_any_grace_primary_effect_ids",
    "generate_rarity5_grace_primary_effect_id",
    "generate_rarity5_grace_primary_effect_ids",
    "materialize_ng3_rarity5_record",
    "materialize_ng3_rarity3_record",
    "materialize_ng3_rarity4_final_record",
    "materialize_ng3_rarity4_stage_one_record",
    "materialize_ng3_certified_install_record",
    "materialize_ng3_certified_record",
    "serialize_ng3_rarity4_stage_one_effect_slots",
    "serialize_ng3_rarity3_effect_slots",
    "serialize_rarity5_grace_effect_slots",
    "serialize_ng3_rarity5_effect_slots",
]
