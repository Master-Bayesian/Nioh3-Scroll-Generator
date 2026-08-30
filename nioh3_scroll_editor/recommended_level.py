"""Native recommended-level normalization and display prediction."""

from __future__ import annotations

import bisect
import json
import math
import struct
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path


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
