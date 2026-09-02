"""Optional native acceleration for exact pivot-family construction."""

from __future__ import annotations

import ctypes
from dataclasses import dataclass
from functools import lru_cache
import os
from pathlib import Path
import sys


ERROR_RESULT = 0xFFFFFFFFFFFFFFFF
CUDA_FAILURE_STAGE_NAMES = {
    1: "device discovery",
    2: "compatibility self-test launch",
    3: "compatibility self-test synchronization",
    10: "R4 pivot allocation",
    11: "R4 pivot upload",
    12: "R4 pivot kernel launch",
    13: "R4 pivot kernel synchronization",
    14: "R4 pivot result-count download",
    15: "R4 pivot result download",
    16: "R4 pivot result capacity",
}


@dataclass(frozen=True, slots=True)
class AuxiliaryPivotMatchPage:
    """Exact auxiliary survivors and cumulative native stage counts."""

    matches: tuple[tuple[int, int], ...]
    stage_counts: tuple[int, ...]
    backend: str


def _application_root() -> Path:
    frozen_root = getattr(sys, "_MEIPASS", None)
    if frozen_root:
        return Path(frozen_root)
    return Path(__file__).resolve().parents[1]


@lru_cache(maxsize=1)
def _load_accelerator() -> ctypes.WinDLL | None:
    if os.name != "nt":
        return None
    override = os.environ.get("NIOH3_SEED_ACCELERATOR", "").strip()
    path = Path(override) if override else _application_root() / "bin" / "nioh3_seed_accelerator.dll"
    if not path.is_file():
        return None
    library = ctypes.WinDLL(str(path))
    function = library.collect_natural_pivot_seeds
    function.argtypes = (
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.c_uint64,
        ctypes.c_uint64,
        ctypes.c_uint16,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_uint64,
    )
    function.restype = ctypes.c_uint64
    cuda_available = library.cuda_seed_acceleration_available
    cuda_available.argtypes = ()
    cuda_available.restype = ctypes.c_int
    last_backend = library.seed_accelerator_last_backend
    last_backend.argtypes = ()
    last_backend.restype = ctypes.c_int
    last_cuda_error = library.seed_accelerator_last_cuda_error
    last_cuda_error.argtypes = ()
    last_cuda_error.restype = ctypes.c_int
    last_cuda_stage = library.seed_accelerator_last_cuda_stage
    last_cuda_stage.argtypes = ()
    last_cuda_stage.restype = ctypes.c_int
    weighted_lookup = library.build_weighted_effect_lookup
    weighted_lookup.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
    )
    weighted_lookup.restype = ctypes.c_int
    primary_ids = library.generate_ng3_primary_effect_ids
    primary_ids.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint32),
    )
    primary_ids.restype = ctypes.c_int
    contextual_primary_ids = library.generate_ng3_primary_effect_ids_context
    contextual_primary_ids.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_uint32,
        ctypes.c_uint8,
        ctypes.c_uint8,
        ctypes.c_uint8,
        ctypes.POINTER(ctypes.c_uint32),
    )
    contextual_primary_ids.restype = ctypes.c_int
    multi_context_primary_ids = library.generate_ng3_r4_primary_effect_ids_multi
    multi_context_primary_ids.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint32),
    )
    multi_context_primary_ids.restype = ctypes.c_int
    r4_primary_pivot = library.collect_ng3_r4_primary_pivot_seeds
    r4_primary_pivot.argtypes = (
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.c_uint64,
        ctypes.c_uint64,
        ctypes.c_uint16,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_uint64,
    )
    r4_primary_pivot.restype = ctypes.c_uint64
    terrain_rows = library.generate_terrain_row_indices
    terrain_rows.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
    )
    terrain_rows.restype = ctypes.c_int
    enemy_matches = library.match_enemy_constraints
    enemy_matches.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.c_uint8,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint8,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
    )
    enemy_matches.restype = ctypes.c_int
    rule_matches = library.match_special_rule_constraints
    rule_matches.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint64,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
    )
    rule_matches.restype = ctypes.c_int
    auxiliary_pivot_matches = library.collect_auxiliary_pivot_matches
    auxiliary_pivot_matches.argtypes = (
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.c_uint64,
        ctypes.c_uint64,
        ctypes.c_uint16,
        ctypes.c_uint32,
        ctypes.c_uint8,
        ctypes.c_int,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint8),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_uint8,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_void_p,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_uint64,
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_uint32,
    )
    auxiliary_pivot_matches.restype = ctypes.c_uint64
    return library


def collect_natural_pivot_seeds(
    values: tuple[int, ...],
    *,
    start_index: int,
    stop_index: int,
    low16_stride: int,
    draw_index: int = 1,
) -> tuple[tuple[int, int], ...] | None:
    """Return ``(seed, one-based trial)`` pairs, or None without the DLL."""

    library = _load_accelerator()
    if library is None:
        return None
    if not values or not 0 <= start_index <= stop_index:
        raise ValueError("invalid native pivot range")
    if not 1 <= draw_index <= 64:
        raise ValueError("native pivot draw index must be in 1..64")
    capacity = stop_index - start_index
    if capacity == 0:
        return ()
    if capacity > 1_000_000:
        raise ValueError("native pivot calls must not exceed 1,000,000 trials")
    value_array = (ctypes.c_uint16 * len(values))(*values)
    seed_array = (ctypes.c_uint32 * capacity)()
    trial_array = (ctypes.c_uint64 * capacity)()
    count = library.collect_natural_pivot_seeds(
        value_array,
        len(values),
        start_index,
        stop_index,
        low16_stride,
        draw_index,
        seed_array,
        trial_array,
        capacity,
    )
    if count == ERROR_RESULT or count > capacity:
        raise RuntimeError("native Seed accelerator rejected a valid pivot range")
    # CUDA threads append atomically and therefore do not have a stable output
    # order. Sorting by the canonical mathematical cursor preserves exact
    # pagination and makes GPU and CPU output byte-for-byte deterministic.
    return tuple(
        sorted(
            ((seed_array[index], trial_array[index]) for index in range(count)),
            key=lambda item: item[1],
        )
    )


def _flatten_uint32_groups(
    groups: tuple[frozenset[int], ...],
) -> tuple[tuple[int, ...], tuple[int, ...]]:
    flattened: list[int] = []
    offsets = [0]
    for group in groups:
        if not group:
            raise ValueError("native criterion groups cannot be empty")
        flattened.extend(sorted(group))
        offsets.append(len(flattened))
    if len(flattened) > 0xFFFF:
        raise ValueError("native criterion alternatives exceed the uint16 ABI")
    return tuple(flattened), tuple(offsets)


def collect_auxiliary_pivot_matches_native(
    values: tuple[int, ...],
    *,
    start_index: int,
    stop_index: int,
    low16_stride: int,
    draw_index: int,
    playthrough: int,
    mode_threshold: int,
    filtered_terrain_rows: tuple[int, ...],
    terrain_row_count: int,
    allowed_terrain_rows: bytes,
    has_terrain_constraint: bool,
    descriptor_thresholds: tuple[int, int, int],
    selector_threshold: int,
    role_five_threshold: int,
    selector_value: int,
    enemy_rows: bytes,
    terrains: bytes,
    contexts: bytes,
    enemy_criterion_groups: tuple[frozenset[int], ...],
    enemy_group_count: int,
    scratch_group_count: int,
    rule_rows: bytes,
    rule_criterion_groups: tuple[frozenset[int], ...],
    output_capacity: int = 1_000_000,
) -> AuxiliaryPivotMatchPage | None:
    """Collect exact auxiliary matches without returning intermediate Seeds."""

    library = _load_accelerator()
    if library is None:
        return None
    item_count = stop_index - start_index
    if not values or not 0 <= start_index <= stop_index:
        raise ValueError("invalid native auxiliary pivot range")
    if item_count > 8_000_000:
        raise ValueError("native auxiliary pivot calls must not exceed 8,000,000 trials")
    if not 1 <= draw_index <= 64:
        raise ValueError("native pivot draw index must be in 1..64")
    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if len(filtered_terrain_rows) == 0 or terrain_row_count <= 0:
        raise ValueError("native terrain configuration cannot be empty")
    if len(allowed_terrain_rows) != terrain_row_count:
        raise ValueError("terrain allow-mask must contain one byte per native row")
    if len(enemy_rows) % 18 or not enemy_rows:
        raise ValueError("packed enemy rows must use the 18-byte native ABI")
    if len(terrains) % 5 or len(terrains) // 5 != terrain_row_count:
        raise ValueError("packed terrain rows must use the 5-byte native ABI")
    if len(contexts) % 22 or not contexts:
        raise ValueError("packed contexts must use the 22-byte native ABI")
    if enemy_group_count + scratch_group_count != len(enemy_criterion_groups):
        raise ValueError("native enemy and scratch group counts do not match")
    if len(enemy_criterion_groups) > 32 or len(rule_criterion_groups) > 32:
        raise ValueError("native auxiliary matching supports at most 32 groups")
    if bool(rule_criterion_groups) != bool(rule_rows):
        raise ValueError("native rule rows and criterion groups must be supplied together")
    if rule_rows and len(rule_rows) % 16:
        raise ValueError("packed special-rule rows must use the 16-byte native ABI")
    if output_capacity <= 0:
        raise ValueError("native auxiliary output capacity must be positive")

    enemy_keys, enemy_offsets = _flatten_uint32_groups(enemy_criterion_groups)
    rule_keys32, rule_offsets = _flatten_uint32_groups(rule_criterion_groups)
    if any(not 0 <= key <= 0xFFFF for key in rule_keys32):
        raise ValueError("special-rule keys must fit in uint16")
    stage_count = 1 + int(has_terrain_constraint) + enemy_group_count + len(
        rule_criterion_groups
    )
    value_array = (ctypes.c_uint16 * len(values))(*values)
    filtered_array = (ctypes.c_uint32 * len(filtered_terrain_rows))(
        *filtered_terrain_rows
    )
    allowed_array = (ctypes.c_uint8 * terrain_row_count).from_buffer_copy(
        allowed_terrain_rows
    )
    threshold_array = (ctypes.c_int * 3)(*descriptor_thresholds)
    enemy_buffer = ctypes.create_string_buffer(enemy_rows)
    terrain_buffer = ctypes.create_string_buffer(terrains)
    context_buffer = ctypes.create_string_buffer(contexts)
    enemy_key_array = (
        (ctypes.c_uint32 * len(enemy_keys))(*enemy_keys) if enemy_keys else None
    )
    enemy_offset_array = (
        (ctypes.c_uint16 * len(enemy_offsets))(*enemy_offsets)
        if enemy_criterion_groups
        else None
    )
    rule_buffer = ctypes.create_string_buffer(rule_rows) if rule_rows else None
    rule_key_array = (
        (ctypes.c_uint16 * len(rule_keys32))(*rule_keys32) if rule_keys32 else None
    )
    rule_offset_array = (
        (ctypes.c_uint16 * len(rule_offsets))(*rule_offsets)
        if rule_criterion_groups
        else None
    )
    capacity = min(output_capacity, max(1, item_count))
    output_seeds = (ctypes.c_uint32 * capacity)()
    output_trials = (ctypes.c_uint64 * capacity)()
    output_stage_counts = (ctypes.c_uint64 * stage_count)()
    count = library.collect_auxiliary_pivot_matches(
        value_array,
        len(values),
        start_index,
        stop_index,
        low16_stride,
        draw_index,
        playthrough,
        mode_threshold,
        filtered_array,
        len(filtered_terrain_rows),
        terrain_row_count,
        allowed_array,
        int(has_terrain_constraint),
        threshold_array,
        selector_threshold,
        role_five_threshold,
        selector_value,
        ctypes.cast(enemy_buffer, ctypes.c_void_p),
        len(enemy_rows) // 18,
        ctypes.cast(terrain_buffer, ctypes.c_void_p),
        len(terrains) // 5,
        ctypes.cast(context_buffer, ctypes.c_void_p),
        len(contexts) // 22,
        enemy_key_array,
        len(enemy_keys),
        enemy_offset_array,
        enemy_group_count,
        scratch_group_count,
        ctypes.cast(rule_buffer, ctypes.c_void_p) if rule_buffer else None,
        len(rule_rows) // 16,
        rule_key_array,
        len(rule_keys32),
        rule_offset_array,
        len(rule_criterion_groups),
        output_seeds,
        output_trials,
        capacity,
        output_stage_counts,
        stage_count,
    )
    if count == ERROR_RESULT or count > capacity:
        raise RuntimeError("native auxiliary pivot matcher rejected valid input")
    matches = tuple(
        sorted(
            (
                (int(output_seeds[index]), int(output_trials[index]))
                for index in range(count)
            ),
            key=lambda item: item[1],
        )
    )
    return AuxiliaryPivotMatchPage(
        matches=matches,
        stage_counts=tuple(int(value) for value in output_stage_counts),
        backend=last_seed_acceleration_backend(),
    )


def native_seed_acceleration_available() -> bool:
    return _load_accelerator() is not None


def build_weighted_effect_lookup_native(
    entries: tuple[tuple[int, int], ...],
) -> tuple[int, ...] | None:
    """Build the exact 65,536-entry inclusive weighted lottery lookup."""

    library = _load_accelerator()
    if library is None:
        return None
    if not entries or len(entries) > 4096:
        raise ValueError("weighted lookup requires 1..4,096 entries")
    if any(
        not 0 <= effect_id <= 0xFFFFFFFF or not 0 <= weight <= 0xFFFFFFFF
        for effect_id, weight in entries
    ):
        raise ValueError("weighted lookup entries must fit in uint32")
    effect_ids = (ctypes.c_uint32 * len(entries))(
        *(effect_id for effect_id, _weight in entries)
    )
    weights = (ctypes.c_uint32 * len(entries))(
        *(weight for _effect_id, weight in entries)
    )
    output = (ctypes.c_uint32 * 0x10000)()
    result = library.build_weighted_effect_lookup(
        effect_ids,
        weights,
        len(entries),
        output,
    )
    if result != 0:
        raise RuntimeError("native weighted lookup builder rejected valid input")
    return tuple(output)


def generate_ng3_primary_effect_ids_native(
    seeds: tuple[int, ...],
    *,
    normal_lookup: tuple[int, ...],
    promoted_lookup: tuple[int, ...],
    promotion_success_lookup: bytes,
    random7_lookup: bytes,
) -> tuple[int, ...] | None:
    """Batch exact NG3 primary IDs through CUDA with native CPU fallback."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000:
        raise ValueError("native primary batches must contain 1..1,000,000 Seeds")
    if len(normal_lookup) != 0x10000 or len(promoted_lookup) != 0x10000:
        raise ValueError("primary lottery lookup tables must contain 65,536 entries")
    if len(promotion_success_lookup) != 0x10000 or len(random7_lookup) != 0x10000:
        raise ValueError("primary path lookup tables must contain 65,536 entries")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    normal_array = (ctypes.c_uint32 * 0x10000)(*normal_lookup)
    promoted_array = (ctypes.c_uint32 * 0x10000)(*promoted_lookup)
    promotion_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(promotion_success_lookup)
    random7_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(random7_lookup)
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.generate_ng3_primary_effect_ids(
        seed_array,
        len(seeds),
        normal_array,
        promoted_array,
        promotion_array,
        random7_array,
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native primary batch accelerator rejected valid input")
    return tuple(output_array)


def generate_ng3_context_primary_effect_ids_native(
    seeds: tuple[int, ...],
    *,
    normal_lookup: tuple[int, ...],
    promoted_lookup: tuple[int, ...],
    promotion_success_lookup: bytes,
    random7_lookup: bytes,
    pre_promotion_draws: int,
    slot_limit: int,
    excluded_slot_mask: int,
    primary_source_index: int,
) -> tuple[int, ...] | None:
    """Batch exact primary IDs for a recovered rarity-specific source layout."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000:
        raise ValueError("native primary batches must contain 1..1,000,000 Seeds")
    if len(normal_lookup) != 0x10000 or len(promoted_lookup) != 0x10000:
        raise ValueError("primary lottery lookup tables must contain 65,536 entries")
    if len(promotion_success_lookup) != 0x10000 or len(random7_lookup) != 0x10000:
        raise ValueError("primary path lookup tables must contain 65,536 entries")
    if not 0 <= pre_promotion_draws <= 64:
        raise ValueError("pre_promotion_draws must be in 0..64")
    if not 1 <= slot_limit <= 7:
        raise ValueError("slot_limit must be in 1..7")
    if not 0 <= excluded_slot_mask <= 0x7F:
        raise ValueError("excluded_slot_mask must be a seven-bit mask")
    if not 0 <= primary_source_index < slot_limit:
        raise ValueError("primary_source_index must identify a source slot")
    if excluded_slot_mask & (1 << primary_source_index):
        raise ValueError("the primary source slot cannot be excluded")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    normal_array = (ctypes.c_uint32 * 0x10000)(*normal_lookup)
    promoted_array = (ctypes.c_uint32 * 0x10000)(*promoted_lookup)
    promotion_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(promotion_success_lookup)
    random7_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(random7_lookup)
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.generate_ng3_primary_effect_ids_context(
        seed_array,
        len(seeds),
        normal_array,
        promoted_array,
        promotion_array,
        random7_array,
        pre_promotion_draws,
        slot_limit,
        excluded_slot_mask,
        primary_source_index,
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native contextual primary batch accelerator rejected valid input")
    return tuple(output_array)


def generate_ng3_r4_multi_context_primary_effect_ids_native(
    seeds: tuple[int, ...],
    *,
    context_by_first_u16: bytes,
    context_count: int,
    normal_lookups: bytes,
    promoted_lookups: bytes,
    promotion_success_lookup: bytes,
    random7_lookup: bytes,
) -> tuple[int, ...] | None:
    """Generate all R4 primary IDs in one native multi-context pass."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000:
        raise ValueError("native R4 primary batches require 1..1,000,000 Seeds")
    if len(context_by_first_u16) != 0x10000:
        raise ValueError("R4 context lookup must contain 65,536 bytes")
    if not 1 <= context_count <= 0x100:
        raise ValueError("R4 context count must be in 1..256")
    expected_lookup_size = context_count * 0x10000 * ctypes.sizeof(ctypes.c_uint32)
    if len(normal_lookups) != expected_lookup_size or len(promoted_lookups) != expected_lookup_size:
        raise ValueError("R4 weighted lookup matrices have the wrong size")
    if len(promotion_success_lookup) != 0x10000 or len(random7_lookup) != 0x10000:
        raise ValueError("R4 path lookups must contain 65,536 bytes")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    context_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(
        context_by_first_u16
    )
    normal_array = (ctypes.c_uint32 * (context_count * 0x10000)).from_buffer_copy(
        normal_lookups
    )
    promoted_array = (
        ctypes.c_uint32 * (context_count * 0x10000)
    ).from_buffer_copy(promoted_lookups)
    promotion_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(
        promotion_success_lookup
    )
    random7_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(random7_lookup)
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.generate_ng3_r4_primary_effect_ids_multi(
        seed_array,
        len(seeds),
        context_array,
        context_count,
        normal_array,
        promoted_array,
        promotion_array,
        random7_array,
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native R4 multi-context primary generator rejected input")
    return tuple(output_array)


def collect_ng3_r4_primary_pivot_seeds_native(
    values: tuple[int, ...],
    *,
    start_index: int,
    stop_index: int,
    low16_stride: int,
    allowed_effect_ids: frozenset[int],
    context_by_first_u16: bytes,
    context_count: int,
    normal_lookups: bytes,
    promoted_lookups: bytes,
    promotion_success_lookup: bytes,
    random7_lookup: bytes,
) -> tuple[tuple[int, int], ...] | None:
    """Collect natural R4 pivot Seeds whose exact primary is selected."""

    library = _load_accelerator()
    if library is None:
        return None
    trial_count = stop_index - start_index
    if not values or not 0 <= start_index <= stop_index or trial_count > 50_000_000:
        raise ValueError("invalid native R4 primary pivot range")
    if not 1 <= low16_stride <= 0xFFFF or low16_stride % 2 == 0:
        raise ValueError("low16_stride must be an odd uint16")
    if not allowed_effect_ids:
        raise ValueError("at least one R4 primary effect must be selected")
    if len(context_by_first_u16) != 0x10000 or not 1 <= context_count <= 0x100:
        raise ValueError("invalid R4 primary context lookup")
    matrix_size = context_count * 0x10000 * ctypes.sizeof(ctypes.c_uint32)
    if len(normal_lookups) != matrix_size or len(promoted_lookups) != matrix_size:
        raise ValueError("invalid R4 primary lookup matrices")
    if len(promotion_success_lookup) != 0x10000 or len(random7_lookup) != 0x10000:
        raise ValueError("invalid R4 path lookups")
    if trial_count == 0:
        return ()
    # Natural scroll IDs occupy one sixteenth of the uint32 domain. Four
    # million entries safely cover a 50-million-trial chunk while keeping the
    # host/device result buffers bounded even for broad primary selections.
    capacity = min(trial_count, 4_000_000)
    value_array = (ctypes.c_uint16 * len(values))(*values)
    allowed = tuple(sorted(allowed_effect_ids))
    allowed_array = (ctypes.c_uint32 * len(allowed))(*allowed)
    context_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(
        context_by_first_u16
    )
    normal_array = (ctypes.c_uint32 * (context_count * 0x10000)).from_buffer_copy(
        normal_lookups
    )
    promoted_array = (
        ctypes.c_uint32 * (context_count * 0x10000)
    ).from_buffer_copy(promoted_lookups)
    promotion_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(
        promotion_success_lookup
    )
    random7_array = (ctypes.c_uint8 * 0x10000).from_buffer_copy(random7_lookup)
    seed_array = (ctypes.c_uint32 * capacity)()
    trial_array = (ctypes.c_uint64 * capacity)()
    count = library.collect_ng3_r4_primary_pivot_seeds(
        value_array,
        len(values),
        start_index,
        stop_index,
        low16_stride,
        allowed_array,
        len(allowed),
        context_array,
        context_count,
        normal_array,
        promoted_array,
        promotion_array,
        random7_array,
        seed_array,
        trial_array,
        capacity,
    )
    if count == ERROR_RESULT or count > capacity:
        raise RuntimeError("native R4 primary pivot collector rejected valid input")
    return tuple(
        sorted(
            ((seed_array[index], trial_array[index]) for index in range(count)),
            key=lambda item: item[1],
        )
    )


def generate_terrain_row_indices_native(
    seeds: tuple[int, ...],
    *,
    mode_threshold: int,
    filtered_row_indices: tuple[int, ...],
    terrain_row_count: int,
) -> tuple[int, ...] | None:
    """Batch exact terrain-row selection through CUDA with native CPU fallback."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000:
        raise ValueError("native terrain batches must contain 1..1,000,000 Seeds")
    if not filtered_row_indices:
        raise ValueError("filtered terrain rows cannot be empty")
    if terrain_row_count <= 0:
        raise ValueError("terrain row count must be positive")
    if any(not 0 <= row < terrain_row_count for row in filtered_row_indices):
        raise ValueError("filtered terrain row is outside the native table")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    filtered_array = (ctypes.c_uint32 * len(filtered_row_indices))(
        *filtered_row_indices
    )
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.generate_terrain_row_indices(
        seed_array,
        len(seeds),
        mode_threshold,
        filtered_array,
        len(filtered_row_indices),
        terrain_row_count,
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native terrain batch accelerator rejected valid input")
    return tuple(output_array)


def match_enemy_constraints_native(
    seeds: tuple[int, ...],
    terrain_rows: tuple[int, ...],
    *,
    playthrough: int,
    mode_threshold: int,
    descriptor_thresholds: tuple[int, int, int],
    selector_threshold: int,
    role_five_threshold: int,
    selector_value: int,
    enemy_rows: bytes,
    terrains: bytes,
    contexts: bytes,
    criterion_groups: tuple[frozenset[int], ...],
) -> tuple[int, ...] | None:
    """Return one exact bit mask for every native enemy-criterion group."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000 or len(seeds) != len(terrain_rows):
        raise ValueError("native enemy batches require matching 1..1,000,000 inputs")
    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if not criterion_groups or len(criterion_groups) > 32:
        raise ValueError("native enemy matching requires 1..32 criterion groups")
    if any(not group for group in criterion_groups):
        raise ValueError("native enemy criterion groups cannot be empty")
    if len(enemy_rows) % 18 or not enemy_rows:
        raise ValueError("packed enemy rows must use the 18-byte native ABI")
    if len(terrains) % 5 or not terrains:
        raise ValueError("packed terrain rows must use the 5-byte native ABI")
    if len(contexts) % 22 or not contexts:
        raise ValueError("packed contexts must use the 22-byte native ABI")
    flattened_keys: list[int] = []
    group_offsets = [0]
    for group in criterion_groups:
        flattened_keys.extend(sorted(group))
        group_offsets.append(len(flattened_keys))
    if len(flattened_keys) > 0xFFFF:
        raise ValueError("native enemy alternatives exceed the uint16 ABI")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    terrain_row_array = (ctypes.c_uint32 * len(terrain_rows))(*terrain_rows)
    threshold_array = (ctypes.c_int * 3)(*descriptor_thresholds)
    enemy_buffer = ctypes.create_string_buffer(enemy_rows)
    terrain_buffer = ctypes.create_string_buffer(terrains)
    context_buffer = ctypes.create_string_buffer(contexts)
    key_array = (ctypes.c_uint32 * len(flattened_keys))(*flattened_keys)
    offset_array = (ctypes.c_uint16 * len(group_offsets))(*group_offsets)
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.match_enemy_constraints(
        seed_array,
        terrain_row_array,
        len(seeds),
        playthrough,
        mode_threshold,
        threshold_array,
        selector_threshold,
        role_five_threshold,
        selector_value,
        ctypes.cast(enemy_buffer, ctypes.c_void_p),
        len(enemy_rows) // 18,
        ctypes.cast(terrain_buffer, ctypes.c_void_p),
        len(terrains) // 5,
        ctypes.cast(context_buffer, ctypes.c_void_p),
        len(contexts) // 22,
        key_array,
        len(flattened_keys),
        offset_array,
        len(criterion_groups),
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native enemy matcher rejected valid input")
    return tuple(int(value) for value in output_array)


def match_special_rule_constraints_native(
    seeds: tuple[int, ...],
    scratch_masks: tuple[int, ...],
    *,
    rule_rows: bytes,
    criterion_groups: tuple[frozenset[int], ...],
) -> tuple[int, ...] | None:
    """Return exact special-rule masks through CUDA or native CPU."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000 or len(seeds) != len(scratch_masks):
        raise ValueError(
            "native special-rule batches require matching 1..1,000,000 inputs"
        )
    if len(rule_rows) % 16 or not rule_rows:
        raise ValueError("packed special-rule rows must use the 16-byte native ABI")
    if not criterion_groups or len(criterion_groups) > 32:
        raise ValueError(
            "native special-rule matching requires 1..32 criterion groups"
        )
    if any(not group for group in criterion_groups):
        raise ValueError("native special-rule criterion groups cannot be empty")
    flattened_keys: list[int] = []
    group_offsets = [0]
    for group in criterion_groups:
        flattened_keys.extend(sorted(group))
        group_offsets.append(len(flattened_keys))
    if len(flattened_keys) > 0xFFFF:
        raise ValueError("native special-rule alternatives exceed the uint16 ABI")
    if any(not 0 <= key <= 0xFFFF for key in flattened_keys):
        raise ValueError("special-rule keys must fit in uint16")
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    scratch_array = (ctypes.c_uint32 * len(scratch_masks))(*scratch_masks)
    rule_buffer = ctypes.create_string_buffer(rule_rows)
    key_array = (ctypes.c_uint16 * len(flattened_keys))(*flattened_keys)
    offset_array = (ctypes.c_uint16 * len(group_offsets))(*group_offsets)
    output_array = (ctypes.c_uint32 * len(seeds))()
    result = library.match_special_rule_constraints(
        seed_array,
        scratch_array,
        len(seeds),
        ctypes.cast(rule_buffer, ctypes.c_void_p),
        len(rule_rows) // 16,
        key_array,
        len(flattened_keys),
        offset_array,
        len(criterion_groups),
        output_array,
    )
    if result not in (0, 1):
        raise RuntimeError("native special-rule matcher rejected valid input")
    return tuple(int(value) for value in output_array)


def cuda_seed_acceleration_available() -> bool:
    library = _load_accelerator()
    return library is not None and bool(library.cuda_seed_acceleration_available())


def last_seed_acceleration_backend() -> str:
    library = _load_accelerator()
    if library is None:
        return "python"
    backend = library.seed_accelerator_last_backend()
    return {1: "cuda", 0: "native_cpu"}.get(backend, "not_used")


def last_cuda_acceleration_failure() -> tuple[str, int] | None:
    """Return the last native CUDA failure stage and numeric runtime code."""

    library = _load_accelerator()
    if library is None:
        return None
    stage = int(library.seed_accelerator_last_cuda_stage())
    error = int(library.seed_accelerator_last_cuda_error())
    if stage == 0 and error == 0:
        return None
    return CUDA_FAILURE_STAGE_NAMES.get(stage, f"unknown stage {stage}"), error


__all__ = [
    "AuxiliaryPivotMatchPage",
    "build_weighted_effect_lookup_native",
    "collect_ng3_r4_primary_pivot_seeds_native",
    "collect_natural_pivot_seeds",
    "collect_auxiliary_pivot_matches_native",
    "cuda_seed_acceleration_available",
    "generate_ng3_context_primary_effect_ids_native",
    "generate_ng3_r4_multi_context_primary_effect_ids_native",
    "generate_ng3_primary_effect_ids_native",
    "generate_terrain_row_indices_native",
    "match_enemy_constraints_native",
    "match_special_rule_constraints_native",
    "last_seed_acceleration_backend",
    "last_cuda_acceleration_failure",
    "native_seed_acceleration_available",
]
