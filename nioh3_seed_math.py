from __future__ import annotations

"""Pure-Python helpers for Nioh 3 v2.00.02 scroll RNG inversion.

The scroll ID is installed directly as the state of a 32-bit LCG.  Each game
random draw advances the state and exposes only the high 16 bits as a float.
This module reproduces that arithmetic, including float32 rounding, and can
rewind the generator because the multiplier is odd and therefore invertible
modulo 2**32.
"""

import struct
from dataclasses import dataclass
from typing import Callable, Iterator

UINT32_MASK = 0xFFFFFFFF
MODULUS = 1 << 32
LCG_MULTIPLIER = 0x00010DCD
LCG_INCREMENT = 1
LCG_MULTIPLIER_INVERSE = pow(LCG_MULTIPLIER, -1, MODULUS)
NATURAL_ID_MASK = 0x0FFFFFFF


def f32(value: float) -> float:
    """Round a Python float to IEEE-754 binary32 exactly."""
    return struct.unpack("<f", struct.pack("<f", float(value)))[0]


_INV_65536_F32 = f32(1.0 / 65536.0)


def lcg_step(state: int) -> int:
    return (LCG_MULTIPLIER * (state & UINT32_MASK) + LCG_INCREMENT) & UINT32_MASK


def lcg_rewind(state_after_draw: int) -> int:
    return (
        LCG_MULTIPLIER_INVERSE
        * (((state_after_draw & UINT32_MASK) - LCG_INCREMENT) & UINT32_MASK)
    ) & UINT32_MASK


def lcg_advance(state: int, draws: int) -> int:
    if draws < 0:
        for _ in range(-draws):
            state = lcg_rewind(state)
        return state
    for _ in range(draws):
        state = lcg_step(state)
    return state


def game_random_int_from_u16(random_u16: int, count: int) -> int:
    """Reproduce 0x56C6A8/0x30DCD4 after the RNG high16 is known.

    The implementation deliberately uses float32 at each arithmetic step.
    """
    if not 0 <= random_u16 <= 0xFFFF:
        raise ValueError("random_u16 must fit in uint16")
    if not 1 <= count <= 0xFFFFFFFF:
        raise ValueError("count must be positive and fit in uint32")
    random_float = f32(f32(float(random_u16)) * _INV_65536_F32)
    scaled = f32(random_float * f32(float(count)))
    result = int(scaled)  # positive cvttss2si: truncation toward zero
    return min(result, count - 1)


def draw_u16(seed_or_state: int) -> tuple[int, int]:
    state = lcg_step(seed_or_state)
    return state, state >> 16


def draw_int(seed_or_state: int, count: int) -> tuple[int, int]:
    state, random_u16 = draw_u16(seed_or_state)
    return state, game_random_int_from_u16(random_u16, count)


def is_natural_scroll_id(seed: int) -> bool:
    """Necessary and sufficient output shape of +0x227A598 after masking."""
    seed &= UINT32_MASK
    return (seed & 0xF0000000) == 0 and (seed & 0xFFFF) != 0


def seed_from_state_after_draw(state_after_draw: int, draw_index: int = 1) -> int:
    if draw_index < 1:
        raise ValueError("draw_index must be at least 1")
    state = state_after_draw & UINT32_MASK
    for _ in range(draw_index):
        state = lcg_rewind(state)
    return state


def state_after_draw_from_seed(seed: int, draw_index: int = 1) -> int:
    if draw_index < 1:
        raise ValueError("draw_index must be at least 1")
    return lcg_advance(seed, draw_index)


@dataclass(frozen=True, slots=True)
class FirstDrawSeed:
    seed: int
    state1: int
    random_u16: int
    low16: int
    random_int: int


def natural_seed_for_first_u16(
    random_u16: int,
    *,
    random_int_count: int = 10_000,
    predicate: Callable[[int], bool] | None = None,
) -> FirstDrawSeed | None:
    """Find a natural-form ID whose first post-update high16 is random_u16."""
    if not 0 <= random_u16 <= 0xFFFF:
        raise ValueError("random_u16 must fit in uint16")
    sampled = game_random_int_from_u16(random_u16, random_int_count)
    if predicate is not None and not predicate(sampled):
        return None
    for low16 in range(0x10000):
        state1 = (random_u16 << 16) | low16
        seed = lcg_rewind(state1)
        if is_natural_scroll_id(seed):
            return FirstDrawSeed(seed, state1, random_u16, low16, sampled)
    return None


def iter_natural_seeds_for_first_u16(
    random_u16: int,
    *,
    random_int_count: int = 10_000,
    start_low16: int = 0,
) -> Iterator[FirstDrawSeed]:
    """Lazily yield every natural-form seed with this first RNG high16.

    The low 16 bits of the first post-update state remain free after fixing
    ``random_u16``.  Keeping that freedom is important: later game RNG draws
    (and therefore the non-grace scroll properties) can still differ.
    """
    if not 0 <= random_u16 <= 0xFFFF:
        raise ValueError("random_u16 must fit in uint16")
    if not 1 <= random_int_count <= 0xFFFFFFFF:
        raise ValueError("random_int_count must be positive and fit in uint32")
    if not isinstance(start_low16, int) or not 0 <= start_low16 <= 0x10000:
        raise ValueError("start_low16 must be between 0 and 65536")
    sampled = game_random_int_from_u16(random_u16, random_int_count)
    for low16 in range(start_low16, 0x10000):
        state1 = (random_u16 << 16) | low16
        seed = lcg_rewind(state1)
        if is_natural_scroll_id(seed):
            yield FirstDrawSeed(seed, state1, random_u16, low16, sampled)


@dataclass(frozen=True, slots=True)
class FallbackProbeSeed:
    seed: int
    state1: int
    state2: int
    first_u16: int
    second_u16: int
    state2_low16: int
    first_random_int: int


def natural_fallback_probe_for_second_u16(
    second_u16: int,
    *,
    first_count: int = 10_000,
    first_fail_threshold: int = 910,
) -> FallbackProbeSeed:
    """Construct a natural ID for one exact second-draw high16 bucket.

    It additionally forces the first RandomInt(first_count) to be >= the
    supplied threshold, so the priority fast path is not taken.
    """
    if not 0 <= second_u16 <= 0xFFFF:
        raise ValueError("second_u16 must fit in uint16")
    if not 0 <= first_fail_threshold <= first_count:
        raise ValueError("first_fail_threshold must be between 0 and first_count")
    for low16 in range(0x10000):
        state2 = (second_u16 << 16) | low16
        state1 = lcg_rewind(state2)
        seed = lcg_rewind(state1)
        first_u16 = state1 >> 16
        first_random_int = game_random_int_from_u16(first_u16, first_count)
        if is_natural_scroll_id(seed) and first_random_int >= first_fail_threshold:
            return FallbackProbeSeed(
                seed=seed,
                state1=state1,
                state2=state2,
                first_u16=first_u16,
                second_u16=second_u16,
                state2_low16=low16,
                first_random_int=first_random_int,
            )
    raise RuntimeError(
        f"No natural-form fallback probe exists for second_u16={second_u16}"
    )


def u16_buckets_for_random_int_range(
    *, count: int, minimum: int = 0, maximum: int | None = None
) -> list[int]:
    if maximum is None:
        maximum = count - 1
    if not 0 <= minimum <= maximum < count:
        raise ValueError("invalid random-int range")
    return [
        value
        for value in range(0x10000)
        if minimum <= game_random_int_from_u16(value, count) <= maximum
    ]


def compress_integer_runs(values: list[int]) -> list[tuple[int, int]]:
    if not values:
        return []
    ordered = sorted(set(values))
    runs: list[tuple[int, int]] = []
    start = previous = ordered[0]
    for value in ordered[1:]:
        if value == previous + 1:
            previous = value
            continue
        runs.append((start, previous))
        start = previous = value
    runs.append((start, previous))
    return runs


def iter_natural_priority_seeds(
    *, threshold: int = 910, count: int = 10_000
) -> Iterator[FirstDrawSeed]:
    for random_u16 in range(0x10000):
        if game_random_int_from_u16(random_u16, count) >= threshold:
            continue
        result = natural_seed_for_first_u16(
            random_u16,
            random_int_count=count,
            predicate=lambda value: value < threshold,
        )
        if result is not None:
            yield result
