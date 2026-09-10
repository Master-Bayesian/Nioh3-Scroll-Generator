"""Native recommended-level normalization and display prediction."""

from __future__ import annotations

import bisect
import json
import math
import struct
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Literal


RESOURCE_PATH = Path(__file__).resolve().parent / "data" / "recommended_level_curve.json"


@dataclass(frozen=True, slots=True)
class RecommendedLevelPrediction:
    requested_internal_level: int
    canonical_internal_level: int
    displayed_level: int
    minimum_internal_level: int
    maximum_internal_level: int

    @property
    def was_clamped(self) -> bool:
        return self.requested_internal_level != self.canonical_internal_level


@dataclass(frozen=True, slots=True)
class RecommendedLevelResolution:
    """Exact inverse result; an unavailable target never selects a nearby raw value."""

    requested_displayed_level: int
    status: Literal["exact", "out_of_range", "unreachable"]
    canonical_internal_levels: tuple[int, ...]
    selected_internal_level: int | None


@dataclass(frozen=True, slots=True)
class RecommendedLevelCurve:
    minimum_internal_level: int
    maximum_internal_level: int
    points: tuple[tuple[int, int], ...]

    def canonical_internal_level(self, requested: int) -> int:
        return min(max(requested, self.minimum_internal_level), self.maximum_internal_level)

    def displayed_level(self, internal_level: int) -> int:
        inputs = tuple(point[0] for point in self.points)
        upper_index = bisect.bisect_right(inputs, internal_level)
        if upper_index == 0:
            return self.points[0][1]
        if upper_index >= len(self.points):
            return self.points[-1][1]
        lower_input, lower_display = self.points[upper_index - 1]
        upper_input, upper_display = self.points[upper_index]
        if upper_input <= lower_input:
            return lower_display
        factor = _float32(
            _float32(float(internal_level - lower_input))
            / _float32(float(upper_input - lower_input))
        )
        delta = _float32(_float32(float(upper_display - lower_display)) * factor)
        return lower_display + math.trunc(delta)

    def predict(self, requested: int) -> RecommendedLevelPrediction:
        canonical = self.canonical_internal_level(requested)
        return RecommendedLevelPrediction(
            requested_internal_level=requested,
            canonical_internal_level=canonical,
            displayed_level=self.displayed_level(canonical),
            minimum_internal_level=self.minimum_internal_level,
            maximum_internal_level=self.maximum_internal_level,
        )

    def resolve_displayed_level(self, requested: int) -> RecommendedLevelResolution:
        """Enumerate exact canonical inputs using the unchanged float32 forward curve.

        Only integers are accepted (booleans are not levels). Missing targets are
        distinguished from targets outside the supported displayed range. Exact
        targets select the lowest matching raw value deterministically; all
        alternatives remain available to callers. No input is clamped or rounded.
        """
        if not isinstance(requested, int) or isinstance(requested, bool):
            raise TypeError("displayed recommended level must be an integer")
        values = _canonical_displayed_levels(self)
        matches = tuple(self.minimum_internal_level + index
                        for index, displayed in enumerate(values) if displayed == requested)
        status = "exact" if matches else (
            "out_of_range" if requested < min(values) or requested > max(values) else "unreachable"
        )
        return RecommendedLevelResolution(requested, status, matches, matches[0] if matches else None)

    def displayed_level_bounds(self) -> tuple[int, int]:
        values = _canonical_displayed_levels(self)
        return min(values), max(values)


@lru_cache(maxsize=8)
def _canonical_displayed_levels(curve: RecommendedLevelCurve) -> tuple[int, ...]:
    return tuple(curve.displayed_level(raw) for raw in
                 range(curve.minimum_internal_level, curve.maximum_internal_level + 1))


def _float32(value: float) -> float:
    return struct.unpack("<f", struct.pack("<f", value))[0]


@lru_cache(maxsize=1)
def native_recommended_level_curve() -> RecommendedLevelCurve:
    value = json.loads(RESOURCE_PATH.read_text(encoding="utf-8"))
    if value.get("schema") != "nioh3-recommended-level-curve/v1":
        raise ValueError("unsupported recommended-level curve schema")
    constructor = value["record_constructor"]
    points = tuple((int(item[0]), int(item[1])) for item in value["curve"]["points"])
    if len(points) < 2 or any(points[index][0] < points[index - 1][0] for index in range(1, len(points))):
        raise ValueError("recommended-level curve points are invalid")
    return RecommendedLevelCurve(
        minimum_internal_level=int(constructor["minimum_internal_level"]),
        maximum_internal_level=int(constructor["maximum_internal_level"]),
        points=points,
    )


def predict_recommended_level(requested: int) -> RecommendedLevelPrediction:
    return native_recommended_level_curve().predict(requested)


def resolve_recommended_level(displayed_level: int) -> RecommendedLevelResolution:
    """Resolve a displayed target against the captured native curve, without writes."""
    return native_recommended_level_curve().resolve_displayed_level(displayed_level)
