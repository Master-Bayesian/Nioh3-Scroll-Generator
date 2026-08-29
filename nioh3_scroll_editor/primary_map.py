from __future__ import annotations

"""Representative draw-2 primary-effect maps conditioned on one Grace.

The map records one constructed low-16 representative for each draw-2
high-16 bucket.  Later offline reconstruction proved that primary output is
not invariant across the other low-16 states in that bucket.  These maps may
therefore be used only as candidate prefilters followed by exact full-seed
replay; they are not complete primary-effect inverse sets.
"""

import struct
import threading
import json
import os
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

from emaki_exchange import EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE, TYPE_TO_CATEGORY
from nioh3_seed_math import lcg_rewind, seed_from_state_after_draw

from .grace_map import (
    GraceOutputMap,
    first_u16_ranges_for_grace,
    grace_id_for_first_u16,
)
from .joint_solver import U16Runs


PRIMARY_MAP_SCHEMA = "nioh3-primary-effect-output-map/v1"


class BatchOracle(Protocol):
    max_batch_size: int

    def generate(
        self, source_records: list[bytes], *, timeout_ms: int = 60_000
    ) -> list[bytes]: ...


@dataclass(frozen=True, slots=True)
class PrimaryMapProgress:
    mapped_buckets: int
    total_buckets: int = 0x10000


@dataclass(frozen=True, slots=True)
class PrimaryOutputMap:
    """One representative output per draw-2 high-16 bucket, not a partition."""
    game_version: str
    record_type: int
    rarity: int
    playthrough: str
    grace_effect_id: int
    grace_effect_slot: int
    draw_index: int
    effects: tuple[tuple[int, U16Runs], ...]

    def runs_for_effects(self, effect_ids: frozenset[int]) -> U16Runs:
        available = dict(self.effects)
        missing = sorted(effect_id for effect_id in effect_ids if effect_id not in available)
        if missing:
            joined = ", ".join(f"0x{effect_id:04X}" for effect_id in missing)
            raise ValueError(f"主词条在当前恩宠/周目候选池中不存在：{joined}")
        return U16Runs.from_ranges(
            run
            for effect_id in effect_ids
            for run in available[effect_id].runs
        )

    @property
    def bucket_count(self) -> int:
        return sum(runs.bucket_count for _effect_id, runs in self.effects)


@dataclass(frozen=True, slots=True)
class PrimaryFirstDrawOutputMap:
    game_version: str
    record_type: int
    rarity: int
    category: int
    draw_index: int
    effects: tuple[tuple[int, U16Runs], ...]

    def runs_for_effects(self, effect_ids: frozenset[int]) -> U16Runs:
        available = dict(self.effects)
        missing = sorted(effect_id for effect_id in effect_ids if effect_id not in available)
        if missing:
            joined = ", ".join(f"0x{effect_id:04X}" for effect_id in missing)
            raise ValueError(f"主词条在所选周目/稀有度候选池中不存在：{joined}")
        return U16Runs.from_ranges(
            run
            for effect_id in effect_ids
            for run in available[effect_id].runs
        )

    @property
    def bucket_count(self) -> int:
        return sum(runs.bucket_count for _effect_id, runs in self.effects)


PrimaryMap = PrimaryOutputMap | PrimaryFirstDrawOutputMap


def _validate_complete_partition(mapping: PrimaryMap) -> None:
    """Require every uint16 draw bucket to belong to exactly one effect."""

    seen = bytearray(0x10000)
    for effect_id, runs in mapping.effects:
        if not 0 <= effect_id <= 0xFFFFFFFF:
            raise ValueError(f"effect ID does not fit in uint32: {effect_id}")
        for value in runs.iter_values():
            if seen[value]:
                raise ValueError(f"draw bucket 0x{value:04X} appears more than once")
            seen[value] = 1
    if mapping.bucket_count != 0x10000 or not all(seen):
        raise ValueError("primary output map does not cover all 65,536 draw buckets")


def primary_map_to_payload(
    mapping: PrimaryMap,
    *,
    context_fingerprint: str,
) -> dict[str, object]:
    """Serialize a certified draw map without process-specific addresses."""

    fingerprint = context_fingerprint.strip().lower()
    if len(fingerprint) != 64 or any(character not in "0123456789abcdef" for character in fingerprint):
        raise ValueError("context fingerprint must be a 64-character SHA-256 hex string")
    _validate_complete_partition(mapping)
    common: dict[str, object] = {
        "schema": PRIMARY_MAP_SCHEMA,
        "context_fingerprint": fingerprint,
        "game_version": mapping.game_version,
        "record_type": f"0x{mapping.record_type:04X}",
        "rarity": mapping.rarity,
        "draw_index": mapping.draw_index,
        "effects": [
            {
                "effect_id": f"0x{effect_id:08X}",
                "runs": [[start, end] for start, end in runs.runs],
            }
            for effect_id, runs in mapping.effects
        ],
    }
    if isinstance(mapping, PrimaryOutputMap):
        common.update(
            {
                "kind": "grace_conditioned_draw2",
                "playthrough": mapping.playthrough,
                "grace_effect_id": f"0x{mapping.grace_effect_id:08X}",
                "grace_effect_slot": mapping.grace_effect_slot,
            }
        )
    else:
        common.update(
            {
                "kind": "primary_draw1",
                "category": mapping.category,
            }
        )
    return common


def primary_map_from_payload(
    payload: dict[str, object],
    *,
    expected_context_fingerprint: str | None = None,
) -> PrimaryMap:
    """Load and validate a previously certified primary-effect draw map."""

    if payload.get("schema") != PRIMARY_MAP_SCHEMA:
        raise ValueError(f"unsupported primary-map schema: {payload.get('schema')!r}")
    fingerprint = str(payload.get("context_fingerprint", "")).lower()
    if expected_context_fingerprint is not None:
        expected = expected_context_fingerprint.strip().lower()
        if fingerprint != expected:
            raise ValueError("primary output map belongs to a different save context")

    effects_payload = payload.get("effects")
    if not isinstance(effects_payload, list):
        raise ValueError("primary output map has no effect partition")
    effects: list[tuple[int, U16Runs]] = []
    for item in effects_payload:
        if not isinstance(item, dict):
            raise ValueError("invalid primary output map effect entry")
        effect_id = int(str(item["effect_id"]), 0)
        runs_payload = item.get("runs")
        if not isinstance(runs_payload, list):
            raise ValueError("invalid primary output map run list")
        runs = U16Runs.from_ranges(
            (int(run[0]), int(run[1]))
            for run in runs_payload
            if isinstance(run, list) and len(run) == 2
        )
        effects.append((effect_id, runs))

    common = {
        "game_version": str(payload["game_version"]),
        "record_type": int(str(payload["record_type"]), 0),
        "rarity": int(payload["rarity"]),
        "draw_index": int(payload["draw_index"]),
        "effects": tuple(effects),
    }
    kind = payload.get("kind")
    if kind == "grace_conditioned_draw2":
        mapping: PrimaryMap = PrimaryOutputMap(
            **common,
            playthrough=str(payload["playthrough"]),
            grace_effect_id=int(str(payload["grace_effect_id"]), 0),
            grace_effect_slot=int(payload["grace_effect_slot"]),
        )
    elif kind == "primary_draw1":
        mapping = PrimaryFirstDrawOutputMap(
            **common,
            category=int(payload["category"]),
        )
    else:
        raise ValueError(f"unsupported primary output map kind: {kind!r}")
    _validate_complete_partition(mapping)
    return mapping


def save_primary_map(
    path: str | Path,
    mapping: PrimaryMap,
    *,
    context_fingerprint: str,
) -> Path:
    """Atomically persist a map so later game-closed solves can reuse it."""

    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    payload = primary_map_to_payload(
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


def load_primary_map(
    path: str | Path,
    *,
    expected_context_fingerprint: str | None = None,
) -> PrimaryMap:
    payload = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("primary output map root must be an object")
    return primary_map_from_payload(
        payload,
        expected_context_fingerprint=expected_context_fingerprint,
    )


def construct_conditioned_probe(
    second_u16: int,
    *,
    grace_effect_id: int,
    mapping: GraceOutputMap,
) -> int:
    """Construct a seed for one exact draw-2 bucket and target draw-1 Grace."""

    if not 0 <= second_u16 <= 0xFFFF:
        raise ValueError("second_u16 must fit in uint16")
    # Fail early if the requested Grace is absent from this measured context.
    first_u16_ranges_for_grace(grace_effect_id, mapping)
    for low16 in range(0x10000):
        state2 = (second_u16 << 16) | low16
        state1 = lcg_rewind(state2)
        if grace_id_for_first_u16(state1 >> 16, mapping) == grace_effect_id:
            return lcg_rewind(state1)
    raise RuntimeError(f"cannot construct a draw-2 probe for bucket {second_u16}")


def build_primary_output_map(
    oracle: BatchOracle,
    *,
    template: bytes,
    grace_effect_id: int,
    mapping: GraceOutputMap,
    rarity: int = 5,
    level: int = 180,
    recommended_level: int = 183,
    cancel_event: threading.Event | None = None,
    progress: Callable[[PrimaryMapProgress], None] | None = None,
) -> PrimaryOutputMap:
    """Capture one native representative for each draw-2 high-16 bucket."""

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    record_type = struct.unpack_from("<H", template, 0)[0]
    if record_type != mapping.record_type:
        raise ValueError(
            f"主词条映射要求 0x{mapping.record_type:04X} 模板，当前为 0x{record_type:04X}"
        )
    if rarity != mapping.rarity or rarity != 5 or mapping.effect_slot != 6:
        raise ValueError("联立主词条映射目前仅验证稀有度 5 / 第 6 槽恩宠")
    category = TYPE_TO_CATEGORY.get(record_type)
    valid_contexts = {"current-loaded-state"}
    if category in (3, 4, 5):
        valid_contexts.add(f"category-{category}-live-native")
    if mapping.playthrough not in valid_contexts:
        raise ValueError("联立主词条映射与所选绘卷类型的原生生成上下文不匹配")

    # Local import avoids a native.py -> primary_map.py -> native.py cycle.
    from .native import build_source_record

    grouped: dict[int, list[int]] = defaultdict(list)
    for start in range(0, 0x10000, oracle.max_batch_size):
        if cancel_event and cancel_event.is_set():
            raise RuntimeError("主词条映射已取消")
        stop = min(start + oracle.max_batch_size, 0x10000)
        buckets = range(start, stop)
        seeds = [
            construct_conditioned_probe(
                bucket,
                grace_effect_id=grace_effect_id,
                mapping=mapping,
            )
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
            raise RuntimeError("游戏原生生成器返回了错误数量的主词条映射记录")
        for bucket, seed, record in zip(buckets, seeds, records, strict=True):
            actual_seed = struct.unpack_from("<I", record, 0x20)[0]
            if actual_seed != seed:
                raise RuntimeError("游戏原生生成器改变了主词条映射探针 Seed")
            actual_grace = struct.unpack_from(
                "<I", record, EFFECT_START + (mapping.effect_slot - 1) * EFFECT_STRIDE + 4
            )[0]
            if actual_grace != grace_effect_id:
                raise RuntimeError(
                    "主词条映射与恩宠 first-u16 映射矛盾："
                    f"预期 0x{grace_effect_id:04X}，实际 0x{actual_grace:04X}"
                )
            primary_id = struct.unpack_from("<I", record, EFFECT_START + 4)[0]
            grouped[primary_id].append(bucket)
        if progress:
            progress(PrimaryMapProgress(mapped_buckets=stop))

    effects = tuple(
        (effect_id, U16Runs.from_values(buckets))
        for effect_id, buckets in sorted(grouped.items())
    )
    result = PrimaryOutputMap(
        game_version="2.00.02",
        record_type=record_type,
        rarity=rarity,
        playthrough=mapping.playthrough,
        grace_effect_id=grace_effect_id,
        grace_effect_slot=mapping.effect_slot,
        draw_index=2,
        effects=effects,
    )
    if result.bucket_count != 0x10000:
        raise RuntimeError("主词条映射未完整覆盖 65,536 个 draw-2 桶")
    return result


def build_primary_first_draw_output_map(
    oracle: BatchOracle,
    *,
    template: bytes,
    category: int,
    rarity: int = 5,
    level: int = 180,
    recommended_level: int = 183,
    cancel_event: threading.Event | None = None,
    progress: Callable[[PrimaryMapProgress], None] | None = None,
) -> PrimaryFirstDrawOutputMap:
    """Map the primary effect for every first-draw uint16 bucket."""

    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    if category not in (1, 2):
        raise ValueError("first-draw primary mapping is verified only for categories 1 and 2")

    from emaki_exchange import CATEGORY_TO_TYPE
    from .native import build_source_record

    record_type = struct.unpack_from("<H", template, 0)[0]
    expected_type = CATEGORY_TO_TYPE[category]
    if record_type != expected_type:
        raise ValueError(
            f"周目 {category} 主词条映射要求 0x{expected_type:04X} 模板，"
            f"当前为 0x{record_type:04X}"
        )

    grouped: dict[int, list[int]] = defaultdict(list)
    for start in range(0, 0x10000, oracle.max_batch_size):
        if cancel_event and cancel_event.is_set():
            raise RuntimeError("主词条映射已取消")
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
            raise RuntimeError("游戏原生生成器返回了错误数量的主词条映射记录")
        for bucket, seed, record in zip(buckets, seeds, records, strict=True):
            actual_seed = struct.unpack_from("<I", record, 0x20)[0]
            if actual_seed != seed:
                raise RuntimeError("游戏原生生成器改变了主词条映射探针 Seed")
            primary_id = struct.unpack_from("<I", record, EFFECT_START + 4)[0]
            grouped[primary_id].append(bucket)
        if progress:
            progress(PrimaryMapProgress(mapped_buckets=stop))

    result = PrimaryFirstDrawOutputMap(
        game_version="2.00.02",
        record_type=record_type,
        rarity=rarity,
        category=category,
        draw_index=1,
        effects=tuple(
            (effect_id, U16Runs.from_values(buckets))
            for effect_id, buckets in sorted(grouped.items())
        ),
    )
    if result.bucket_count != 0x10000:
        raise RuntimeError("主词条映射未完整覆盖 65,536 个 draw-1 桶")
    return result
