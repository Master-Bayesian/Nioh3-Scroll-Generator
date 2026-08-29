"""Validated first-RNG-draw raw-output maps for Nioh 3 scrolls.

Historical function/class names still use ``Grace`` for compatibility, but the
measured namespaces are context-specific:
- PC 2.00.02, E604 rarity 5, current loaded progression, slot 6: verified Grace context.
- PC 2.00.02, E604 rarity 4, current loaded progression, slot 5: separately
  measured raw-output context. Its complete native name table is still pending.

Seed 183696634 proved that the R4 map is a stage-one code map rather than a
final-effect name map. Its installed candidate contained slot-5 0xBABD, while
the game-resolved record contained the complete 0xAE5A slot (技之深奥).
Direct final-record placement of 0xBABD still displayed 月读的恩宠. Both facts
coexist because the generation stage is part of the context.

Raw rarity 3 has no direct resolved special-result slot.  Experimental
growing-effect searches may reuse the rarity-4 raw-output map only as a
prediction for what slot-5 ``0x0001`` may resolve to after completion.
"""

from __future__ import annotations

import json
import os
import struct
import threading
from bisect import bisect_left
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Callable, Iterator, Protocol

from emaki_exchange import (
    CATEGORY_TO_TYPE,
    EFFECT_START,
    EFFECT_STRIDE,
    SCROLL_RECORD_SIZE,
)
from nioh3_seed_math import (
    FirstDrawSeed,
    iter_natural_seeds_for_first_u16,
    is_natural_scroll_id,
    lcg_step,
    seed_from_state_after_draw,
)

_DATA_DIR = Path(__file__).with_name("data")
_DATA_PATHS = {
    4: _DATA_DIR / "grace_output_map_e604_r4_current.json",
    5: _DATA_DIR / "grace_output_map_e604_r5_current.json",
}
_EXPECTED_CONTEXTS = {
    4: (0xE604, 4, "current-loaded-state", 5),
    5: (0xE604, 5, "current-loaded-state", 6),
}
_EXPECTED_VERSION = "2.00.02"
GRACE_MAP_CACHE_SCHEMA = "nioh3-grace-output-map-cache/v1"


@dataclass(frozen=True, slots=True)
class GraceRange:
    start: int
    end: int
    grace_id: int


@dataclass(frozen=True, slots=True)
class GraceOutputMap:
    record_type: int
    rarity: int
    playthrough: str
    effect_slot: int
    ranges: tuple[GraceRange, ...]


@dataclass(frozen=True, slots=True)
class GraceMapProgress:
    mapped_buckets: int
    total_buckets: int = 0x10000


def _validate_complete_mapping(mapping: GraceOutputMap) -> None:
    expected_start = 0
    for entry in mapping.ranges:
        if entry.start != expected_start or not entry.start <= entry.end <= 0xFFFF:
            raise ValueError("Grace output map is not a complete contiguous partition")
        if not 0 <= entry.grace_id <= 0xFFFFFFFF:
            raise ValueError("Grace output map effect ID does not fit in uint32")
        expected_start = entry.end + 1
    if expected_start != 0x10000:
        raise ValueError("Grace output map does not cover all 65,536 draw buckets")


def grace_map_to_cache_payload(
    mapping: GraceOutputMap,
    *,
    context_fingerprint: str,
) -> dict[str, object]:
    """Serialize one live-measured map under an exact save-context gate."""

    fingerprint = context_fingerprint.strip().lower()
    if len(fingerprint) != 64 or any(
        character not in "0123456789abcdef" for character in fingerprint
    ):
        raise ValueError("context fingerprint must be a 64-character SHA-256 hex string")
    _validate_complete_mapping(mapping)
    return {
        "schema": GRACE_MAP_CACHE_SCHEMA,
        "context_fingerprint": fingerprint,
        "game_version": _EXPECTED_VERSION,
        "record_type": f"0x{mapping.record_type:04X}",
        "rarity": mapping.rarity,
        "playthrough": mapping.playthrough,
        "effect_slot": mapping.effect_slot,
        "draw_index": 1,
        "ranges": [
            {
                "start": entry.start,
                "end": entry.end,
                "grace_id": f"0x{entry.grace_id:08X}",
            }
            for entry in mapping.ranges
        ],
    }


def grace_map_from_cache_payload(
    payload: dict[str, object],
    *,
    expected_context_fingerprint: str | None = None,
) -> GraceOutputMap:
    """Load a cached map while rejecting stale game/save contexts."""

    if payload.get("schema") != GRACE_MAP_CACHE_SCHEMA:
        raise ValueError(f"unsupported Grace-map cache schema: {payload.get('schema')!r}")
    if payload.get("game_version") != _EXPECTED_VERSION:
        raise ValueError("Grace-map cache belongs to another game version")
    fingerprint = str(payload.get("context_fingerprint", "")).lower()
    if expected_context_fingerprint is not None:
        expected = expected_context_fingerprint.strip().lower()
        if fingerprint != expected:
            raise ValueError("Grace output map belongs to a different save context")
    if int(payload.get("draw_index", 0)) != 1:
        raise ValueError("Grace-map cache is not a draw-1 partition")
    raw_ranges = payload.get("ranges")
    if not isinstance(raw_ranges, list):
        raise ValueError("Grace-map cache has no range partition")
    ranges: list[GraceRange] = []
    for item in raw_ranges:
        if not isinstance(item, dict):
            raise ValueError("invalid Grace-map cache range")
        ranges.append(
            GraceRange(
                start=int(item["start"]),
                end=int(item["end"]),
                grace_id=int(str(item["grace_id"]), 0),
            )
        )
    mapping = GraceOutputMap(
        record_type=int(str(payload["record_type"]), 0),
        rarity=int(payload["rarity"]),
        playthrough=str(payload["playthrough"]),
        effect_slot=int(payload["effect_slot"]),
        ranges=tuple(ranges),
    )
    _validate_complete_mapping(mapping)
    return mapping


def save_grace_map_cache(
    path: str | Path,
    mapping: GraceOutputMap,
    *,
    context_fingerprint: str,
) -> Path:
    """Atomically persist a live map for later process/game-closed reuse."""

    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    payload = grace_map_to_cache_payload(
        mapping,
        context_fingerprint=context_fingerprint,
    )
    data = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    temporary = target.with_name(f".{target.name}.{os.getpid()}.tmp")
    try:
        temporary.write_text(data, encoding="utf-8")
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)
    return target


def load_grace_map_cache(
    path: str | Path,
    *,
    expected_context_fingerprint: str | None = None,
) -> GraceOutputMap:
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("Grace-map cache root must be an object")
    return grace_map_from_cache_payload(
        payload,
        expected_context_fingerprint=expected_context_fingerprint,
    )


class BatchOracle(Protocol):
    max_batch_size: int

    def generate(
        self, source_records: list[bytes], *, timeout_ms: int = 60_000
    ) -> list[bytes]: ...


def build_live_grace_output_map(
    oracle: BatchOracle,
    *,
    template: bytes,
    category: int,
    rarity: int = 5,
    level: int = 180,
    recommended_level: int = 183,
    cancel_event: threading.Event | None = None,
    progress: Callable[[GraceMapProgress], None] | None = None,
) -> GraceOutputMap:
    """Measure a complete first-draw special-result map with the live game.

    Categories 4 and 5 exist in the v2.00.02 native type table but are not
    present in the supplied NG3 save.  Their synthetic templates are therefore
    experimental.  Every returned candidate is still regenerated and checked
    by the game's native generator before it can be shown or installed.
    """

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    if category not in (3, 4, 5):
        raise ValueError("live special-result mapping supports categories 3, 4, and 5")
    if rarity != 5:
        raise ValueError("experimental category-4/5 mapping currently supports rarity 5")
    record_type = struct.unpack_from("<H", template, 0)[0]
    expected_type = CATEGORY_TO_TYPE[category]
    if record_type != expected_type:
        raise ValueError(
            f"category {category} requires record type 0x{expected_type:04X}, "
            f"got 0x{record_type:04X}"
        )

    # Local import avoids a grace_map.py -> native.py -> grace_map.py cycle.
    from .native import build_source_record

    outputs: list[int] = []
    for start in range(0, 0x10000, oracle.max_batch_size):
        if cancel_event and cancel_event.is_set():
            raise RuntimeError("特殊结果映射已取消")
        stop = min(start + oracle.max_batch_size, 0x10000)
        buckets = range(start, stop)
        seeds = [
            seed_from_state_after_draw(bucket << 16, 1)
            for bucket in buckets
        ]
        sources = [
            build_source_record(
                template,
                seed=seed,
                rarity=rarity,
                level=level,
                recommended_level=recommended_level,
            )
            for seed in seeds
        ]
        records = oracle.generate(sources, timeout_ms=120_000)
        if len(records) != len(seeds):
            raise RuntimeError("游戏原生生成器返回了错误数量的特殊结果映射记录")
        for seed, record in zip(seeds, records, strict=True):
            if struct.unpack_from("<I", record, 0x20)[0] != seed:
                raise RuntimeError("游戏原生生成器改变了特殊结果映射探针 Seed")
            if struct.unpack_from("<H", record, 0)[0] != record_type:
                raise RuntimeError("游戏原生生成器改变了特殊结果映射记录类型")
            outputs.append(
                struct.unpack_from(
                    "<I", record, EFFECT_START + 5 * EFFECT_STRIDE + 4
                )[0]
            )
        if progress:
            progress(GraceMapProgress(mapped_buckets=stop))

    ranges: list[GraceRange] = []
    range_start = 0
    current_effect = outputs[0]
    for bucket, effect_id in enumerate(outputs[1:], start=1):
        if effect_id == current_effect:
            continue
        ranges.append(GraceRange(range_start, bucket - 1, current_effect))
        range_start = bucket
        current_effect = effect_id
    ranges.append(GraceRange(range_start, 0xFFFF, current_effect))
    return GraceOutputMap(
        record_type=record_type,
        rarity=rarity,
        playthrough=f"category-{category}-live-native",
        effect_slot=6,
        ranges=tuple(ranges),
    )


def _integer(value: object, field: str) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{field} must be an integer, not bool")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        try:
            return int(value, 0)
        except ValueError as error:
            raise ValueError(f"{field} is not a valid integer: {value!r}") from error
    raise ValueError(f"{field} must be an integer or integer string")


@lru_cache(maxsize=8)
def load_grace_output_map(
    path: str | Path | None = None,
    *,
    rarity: int = 5,
) -> GraceOutputMap:
    """Load and strictly validate a measured Grace-map context.

    ``rarity=5`` remains the default for backwards compatibility.  When an
    explicit ``path`` is supplied its context must still match the requested
    rarity; this prevents accidentally feeding an r4 map to an r5 scan or vice
    versa.
    """
    if rarity not in _DATA_PATHS:
        raise ValueError("validated Grace maps currently exist only for rarity 4 and 5")
    source = Path(path) if path is not None else _DATA_PATHS[rarity]
    try:
        raw = json.loads(source.read_text(encoding="utf-8"))
    except OSError as error:
        raise ValueError(f"cannot read grace output map {source}") from error
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid JSON in grace output map {source}") from error
    if not isinstance(raw, dict):
        raise ValueError("grace output map root must be an object")
    if raw.get("format") != "nioh3-grace-first-u16-map-v2":
        raise ValueError("unsupported grace output map format")
    if raw.get("game_version") != _EXPECTED_VERSION:
        raise ValueError("grace output map has an unverified game version")
    context = raw.get("context")
    if not isinstance(context, dict):
        raise ValueError("grace output map context must be an object")
    parsed_context = (
        _integer(context.get("record_type"), "context.record_type"),
        _integer(context.get("rarity"), "context.rarity"),
        context.get("playthrough"),
        _integer(context.get("effect_slot"), "context.effect_slot"),
    )
    expected = _EXPECTED_CONTEXTS[rarity]
    if parsed_context != expected:
        raise ValueError(
            "grace output map context is not the verified "
            f"E604/r{rarity}/slot{expected[3]} current-loaded-state context"
        )
    rng = raw.get("rng")
    if not isinstance(rng, dict) or (
        _integer(rng.get("multiplier"), "rng.multiplier"),
        _integer(rng.get("increment"), "rng.increment"),
        _integer(rng.get("inverse_multiplier"), "rng.inverse_multiplier"),
        rng.get("output"),
    ) != (0x10DCD, 1, 0xA5E2A705, "next_state >> 16"):
        raise ValueError("grace output map RNG metadata does not match the verified LCG")
    raw_ranges = raw.get("ranges")
    if not isinstance(raw_ranges, list) or not raw_ranges:
        raise ValueError("grace output map ranges must be a non-empty list")
    ranges: list[GraceRange] = []
    expected_start = 0
    for index, item in enumerate(raw_ranges):
        if not isinstance(item, dict):
            raise ValueError(f"ranges[{index}] must be an object")
        start = _integer(item.get("start"), f"ranges[{index}].start")
        end = _integer(item.get("end"), f"ranges[{index}].end")
        grace_id = _integer(item.get("grace_id"), f"ranges[{index}].grace_id")
        if not 0 <= start <= end <= 0xFFFF:
            raise ValueError(f"ranges[{index}] is outside uint16 or inverted")
        if start != expected_start:
            raise ValueError(f"ranges[{index}] is overlapping or leaves a gap")
        if not 0 <= grace_id <= 0xFFFFFFFF:
            raise ValueError(f"ranges[{index}].grace_id must fit in uint32")
        ranges.append(GraceRange(start, end, grace_id))
        expected_start = end + 1
    if ranges[-1].end != 0xFFFF:
        raise ValueError("grace output map must end at first_u16=65535")
    counts = raw.get("counts")
    if counts is not None:
        if not isinstance(counts, dict):
            raise ValueError("grace output map counts must be an object")
        computed = {
            f"0x{entry.grace_id:04X}": entry.end - entry.start + 1
            for entry in ranges
        }
        parsed_counts = {
            f"0x{_integer(key, 'counts key'):04X}": _integer(value, f"counts[{key!r}]")
            for key, value in counts.items()
        }
        if parsed_counts != computed:
            raise ValueError("grace output map counts do not match its ranges")
    return GraceOutputMap(*parsed_context, tuple(ranges))


def grace_id_for_first_u16(first_u16: int, mapping: GraceOutputMap) -> int:
    if not 0 <= first_u16 <= 0xFFFF:
        raise ValueError("first_u16 must fit in uint16")
    starts = tuple(entry.start for entry in mapping.ranges)
    index = bisect_left(starts, first_u16 + 1) - 1
    entry = mapping.ranges[index]
    if not entry.start <= first_u16 <= entry.end:
        raise ValueError("first_u16 is not covered by the grace output map")
    return entry.grace_id


def grace_id_for_seed(seed: int, mapping: GraceOutputMap) -> int:
    return grace_id_for_first_u16(lcg_step(seed) >> 16, mapping)


def first_u16_ranges_for_grace(
    grace_id: int, mapping: GraceOutputMap
) -> tuple[GraceRange, ...]:
    if not isinstance(grace_id, int) or isinstance(grace_id, bool):
        raise ValueError("grace_id must be an integer")
    result = tuple(entry for entry in mapping.ranges if entry.grace_id == grace_id)
    if not result:
        raise ValueError(f"grace ID 0x{grace_id:X} is not present in the grace output map")
    return result


def iter_natural_seeds_for_grace(
    grace_id: int,
    mapping: GraceOutputMap,
    *,
    max_results: int | None = None,
    start_after_seed: int | None = None,
) -> Iterator[FirstDrawSeed]:
    """Lazily enumerate natural IDs constrained to the requested Grace."""
    if max_results is not None and (not isinstance(max_results, int) or max_results < 0):
        raise ValueError("max_results must be a non-negative integer or None")
    if max_results == 0:
        return
    grace_ranges = first_u16_ranges_for_grace(grace_id, mapping)
    resume_range_index = 0
    resume_first_u16: int | None = None
    resume_low16 = 0
    if start_after_seed is not None:
        if not isinstance(start_after_seed, int) or not 0 <= start_after_seed <= 0xFFFFFFFF:
            raise ValueError("start_after_seed must fit in uint32 or be None")
        if not is_natural_scroll_id(start_after_seed):
            raise ValueError("start_after_seed must be a natural scroll ID")
        state1 = lcg_step(start_after_seed)
        resume_first_u16 = state1 >> 16
        if grace_id_for_first_u16(resume_first_u16, mapping) != grace_id:
            raise ValueError("start_after_seed does not belong to the requested grace")
        for index, grace_range in enumerate(grace_ranges):
            if grace_range.start <= resume_first_u16 <= grace_range.end:
                resume_range_index = index
                resume_low16 = (state1 & 0xFFFF) + 1
                break
        else:
            raise ValueError("start_after_seed is not covered by the requested grace")
    produced = 0
    for range_index, grace_range in enumerate(grace_ranges):
        if range_index < resume_range_index:
            continue
        first_start = grace_range.start
        if range_index == resume_range_index and resume_first_u16 is not None:
            first_start = resume_first_u16
        for first_u16 in range(first_start, grace_range.end + 1):
            low16_start = resume_low16 if first_u16 == resume_first_u16 else 0
            for result in iter_natural_seeds_for_first_u16(
                first_u16, start_low16=low16_start
            ):
                if max_results is not None and produced >= max_results:
                    return
                yield result
                produced += 1


def find_natural_seed_for_grace(grace_id: int, mapping: GraceOutputMap) -> FirstDrawSeed:
    result = next(iter_natural_seeds_for_grace(grace_id, mapping, max_results=1), None)
    if result is None:
        raise RuntimeError(f"no natural seed exists for grace ID 0x{grace_id:X}")
    return result
