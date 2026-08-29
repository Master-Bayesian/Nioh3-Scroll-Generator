from __future__ import annotations

"""Exact fixed-draw constraints for the Nioh 3 scroll RNG.

The displayed scroll seed is the initial state of a 32-bit LCG.  A fixed
generator draw can therefore be represented as a set of allowed high-16-bit
buckets.  The inverse family of the smallest constraint is enumerated and all
other fixed draws are checked against the same seed.
"""

from dataclasses import dataclass
from math import gcd
from typing import Iterable, Iterator, Sequence

from nioh3_seed_math import (
    is_natural_scroll_id,
    seed_from_state_after_draw,
    state_after_draw_from_seed,
)
from .seed_accelerator import collect_natural_pivot_seeds


NATIVE_ACCELERATOR_CHUNK_TRIALS = 1_000_000


@dataclass(frozen=True, slots=True)
class U16Runs:
    """Sorted, disjoint inclusive ranges in the uint16 output domain."""

    runs: tuple[tuple[int, int], ...]

    def __post_init__(self) -> None:
        previous_end = -2
        for start, end in self.runs:
            if not 0 <= start <= end <= 0xFFFF:
                raise ValueError(f"invalid uint16 run: {(start, end)!r}")
            if start <= previous_end + 1:
                raise ValueError("runs must be sorted, disjoint, and nonadjacent")
            previous_end = end

    @classmethod
    def from_ranges(cls, ranges: Iterable[tuple[int, int]]) -> "U16Runs":
        ordered = sorted((int(start), int(end)) for start, end in ranges)
        merged: list[list[int]] = []
        for start, end in ordered:
            if not 0 <= start <= end <= 0xFFFF:
                raise ValueError(f"invalid uint16 range: {(start, end)!r}")
            if merged and start <= merged[-1][1] + 1:
                merged[-1][1] = max(merged[-1][1], end)
            else:
                merged.append([start, end])
        return cls(tuple((start, end) for start, end in merged))

    @classmethod
    def from_values(cls, values: Iterable[int]) -> "U16Runs":
        ordered = sorted(set(int(value) for value in values))
        if not ordered:
            return cls(())
        return cls.from_ranges((value, value) for value in ordered)

    @property
    def bucket_count(self) -> int:
        return sum(end - start + 1 for start, end in self.runs)

    def contains(self, value: int) -> bool:
        if not 0 <= value <= 0xFFFF:
            return False
        for start, end in self.runs:
            if value < start:
                return False
            if value <= end:
                return True
        return False

    def iter_values(self) -> Iterator[int]:
        for start, end in self.runs:
            yield from range(start, end + 1)


@dataclass(frozen=True, slots=True)
class DrawConstraint:
    name: str
    draw_index: int
    allowed_u16: U16Runs

    def __post_init__(self) -> None:
        if self.draw_index <= 0:
            raise ValueError("draw_index must be positive")
        if self.allowed_u16.bucket_count == 0:
            raise ValueError("constraint cannot be empty")

    def matches(self, seed: int) -> bool:
        state = state_after_draw_from_seed(seed, self.draw_index)
        return self.allowed_u16.contains(state >> 16)


@dataclass(frozen=True, slots=True)
class SeedSolution:
    seed: int
    pivot_name: str
    pivot_trial: int
    pivot_u16: int
    pivot_state_low16: int


def _permuted_values(runs: U16Runs) -> tuple[int, ...]:
    values = tuple(runs.iter_values())
    if not values:
        return values
    stride = 0x9E37 % len(values) or 1
    if stride % 2 == 0:
        stride += 1
    while gcd(stride, len(values)) != 1:
        stride += 2
    return tuple(values[(index * stride) % len(values)] for index in range(len(values)))


def choose_pivot(constraints: Sequence[DrawConstraint]) -> DrawConstraint:
    if not constraints:
        raise ValueError("at least one constraint is required")
    return min(
        constraints,
        key=lambda item: (item.allowed_u16.bucket_count, -item.draw_index, item.name),
    )


def iter_constraint_intersection(
    constraints: Sequence[DrawConstraint],
    *,
    natural_only: bool = True,
    start_after_trial: int = 0,
    max_trials: int | None = None,
    low16_stride: int = 0x9E37,
    use_native_acceleration: bool = True,
) -> Iterator[SeedSolution]:
    """Yield exact intersections with an O(1) resumable pivot cursor.

    ``pivot_trial`` counts every state in the pivot inverse family, including
    states rejected by the natural-ID shape.  That definition lets a later UI
    search resume without replaying prior mathematical work.
    """

    constraints = tuple(constraints)
    if not constraints:
        raise ValueError("at least one constraint is required")
    if len({item.name for item in constraints}) != len(constraints):
        raise ValueError("constraint names must be unique")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    if max_trials is not None and max_trials <= 0:
        raise ValueError("max_trials must be positive when supplied")
    if not 1 <= low16_stride <= 0xFFFF or low16_stride % 2 == 0:
        raise ValueError("low16_stride must be an odd uint16")

    pivot = choose_pivot(constraints)
    others = tuple(item for item in constraints if item is not pivot)
    values = _permuted_values(pivot.allowed_u16)
    family_size = len(values) * 0x10000
    first_index = min(start_after_trial, family_size)
    stop_index = family_size
    if max_trials is not None:
        stop_index = min(stop_index, first_index + max_trials)

    if natural_only and use_native_acceleration:
        native_was_available = False
        chunk_start = first_index
        while chunk_start < stop_index:
            chunk_stop = min(
                stop_index,
                chunk_start + NATIVE_ACCELERATOR_CHUNK_TRIALS,
            )
            accelerated = collect_natural_pivot_seeds(
                values,
                start_index=chunk_start,
                stop_index=chunk_stop,
                low16_stride=low16_stride,
                draw_index=pivot.draw_index,
            )
            if accelerated is None:
                break
            native_was_available = True
            for seed, pivot_trial in accelerated:
                flat_index = pivot_trial - 1
                low_index, bucket_index = divmod(flat_index, len(values))
                low16 = (low_index * low16_stride) & 0xFFFF
                rotation = low_index % len(values)
                u16 = values[(rotation + bucket_index) % len(values)]
                if not all(item.matches(seed) for item in others):
                    continue
                yield SeedSolution(
                    seed=seed,
                    pivot_name=pivot.name,
                    pivot_trial=pivot_trial,
                    pivot_u16=u16,
                    pivot_state_low16=low16,
                )
            chunk_start = chunk_stop
        if native_was_available and chunk_start >= stop_index:
            return

    for flat_index in range(first_index, stop_index):
        low_index, bucket_index = divmod(flat_index, len(values))
        low16 = (low_index * low16_stride) & 0xFFFF
        rotation = low_index % len(values)
        u16 = values[(rotation + bucket_index) % len(values)]
        state = (u16 << 16) | low16
        seed = seed_from_state_after_draw(state, pivot.draw_index)
        if natural_only and not is_natural_scroll_id(seed):
            continue
        if not all(item.matches(seed) for item in others):
            continue
        yield SeedSolution(
            seed=seed,
            pivot_name=pivot.name,
            pivot_trial=flat_index + 1,
            pivot_u16=u16,
            pivot_state_low16=low16,
        )
