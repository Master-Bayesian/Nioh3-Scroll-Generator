"""Optional native acceleration for exact pivot-family construction."""

from __future__ import annotations

import ctypes
from functools import lru_cache
import os
from pathlib import Path
import sys


ERROR_RESULT = 0xFFFFFFFFFFFFFFFF


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


def native_seed_acceleration_available() -> bool:
    return _load_accelerator() is not None


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


def cuda_seed_acceleration_available() -> bool:
    library = _load_accelerator()
    return library is not None and bool(library.cuda_seed_acceleration_available())


def last_seed_acceleration_backend() -> str:
    library = _load_accelerator()
    if library is None:
        return "python"
    backend = library.seed_accelerator_last_backend()
    return {1: "cuda", 0: "native_cpu"}.get(backend, "not_used")


__all__ = [
    "collect_natural_pivot_seeds",
    "cuda_seed_acceleration_available",
    "generate_ng3_context_primary_effect_ids_native",
    "generate_ng3_primary_effect_ids_native",
    "last_seed_acceleration_backend",
    "native_seed_acceleration_available",
]
