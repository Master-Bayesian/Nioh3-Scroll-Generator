"""Optional Direct3D 11 scanner for compiled effect preimage plans.

Direct3D 11 compute shaders run on NVIDIA, AMD, and Intel Windows drivers. The
scanner is deliberately table-free: Python compiles exact path intervals, the
GPU evaluates those predicates, and the certified CPU generator replays every
reported Seed before product use.
"""

from __future__ import annotations

import ctypes
from dataclasses import dataclass
from functools import lru_cache
from itertools import product
import os
from pathlib import Path
import sys

from nioh3_seed_math import LCG_INCREMENT, LCG_MULTIPLIER

from .effect_path_inverse import CompiledEffectPlan, U16Run


ERROR_RESULT = 0xFFFFFFFFFFFFFFFF
AMD_VENDOR_ID = 0x1002
NVIDIA_VENDOR_ID = 0x10DE
INTEL_VENDOR_ID = 0x8086
_last_d3d11_vendor_id: int | None = None


@dataclass(frozen=True, slots=True)
class D3D11AdapterInfo:
    description: str
    vendor_id: int
    device_id: int
    dedicated_video_memory: int
    shared_system_memory: int

    @property
    def reports_dedicated_video_memory(self) -> bool:
        return self.dedicated_video_memory > 0


class _PathConstraintInput(ctypes.Structure):
    _fields_ = (
        ("draw_index", ctypes.c_uint32),
        ("start_u16", ctypes.c_uint32),
        ("end_u16", ctypes.c_uint32),
        ("reserved", ctypes.c_uint32),
    )


class _EffectPathInput(ctypes.Structure):
    _fields_ = (
        ("promoted_slot", ctypes.c_int32),
        ("constraint_count", ctypes.c_uint32),
        ("reserved0", ctypes.c_uint32),
        ("reserved1", ctypes.c_uint32),
        ("constraints", _PathConstraintInput * 6),
    )


if ctypes.sizeof(_EffectPathInput) != 112:
    raise RuntimeError("Direct3D path descriptor ABI mismatch")


def _application_root() -> Path:
    frozen_root = getattr(sys, "_MEIPASS", None)
    if frozen_root:
        return Path(frozen_root)
    return Path(__file__).resolve().parents[1]


@lru_cache(maxsize=1)
def _load_accelerator() -> ctypes.WinDLL | None:
    if os.name != "nt":
        return None
    override = os.environ.get("NIOH3_EFFECT_PREIMAGE_ACCELERATOR", "").strip()
    path = (
        Path(override)
        if override
        else _application_root() / "bin" / "nioh3_effect_preimage_accelerator.dll"
    )
    if not path.is_file():
        return None
    try:
        library = ctypes.WinDLL(str(path))
    except OSError:
        return None
    available = library.d3d11_effect_acceleration_available
    available.argtypes = (ctypes.c_uint32,)
    available.restype = ctypes.c_int
    adapter_info = library.d3d11_effect_adapter_info
    adapter_info.argtypes = (
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.POINTER(ctypes.c_wchar),
        ctypes.c_uint32,
    )
    adapter_info.restype = ctypes.c_int
    collect = library.collect_effect_preimage_matches_d3d11
    collect.argtypes = (
        ctypes.POINTER(ctypes.c_uint16),
        ctypes.c_uint32,
        ctypes.c_uint64,
        ctypes.c_uint64,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.POINTER(_EffectPathInput),
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint64),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
    )
    collect.restype = ctypes.c_uint64
    return library


def _configured_vendor_id() -> int:
    raw = os.environ.get("NIOH3_D3D11_VENDOR", "").strip()
    if not raw:
        return 0
    value = int(raw, 0)
    if not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("NIOH3_D3D11_VENDOR must fit in uint32")
    return value


def d3d11_effect_acceleration_available(*, vendor_id: int | None = None) -> bool:
    library = _load_accelerator()
    if library is None:
        return False
    preferred = _configured_vendor_id() if vendor_id is None else vendor_id
    return bool(library.d3d11_effect_acceleration_available(preferred))


def d3d11_effect_adapter_info(
    *,
    vendor_id: int | None = None,
) -> D3D11AdapterInfo | None:
    library = _load_accelerator()
    if library is None:
        return None
    preferred = _configured_vendor_id() if vendor_id is None else vendor_id
    actual_vendor = ctypes.c_uint32()
    device_id = ctypes.c_uint32()
    dedicated = ctypes.c_uint64()
    shared = ctypes.c_uint64()
    description = ctypes.create_unicode_buffer(128)
    if not library.d3d11_effect_adapter_info(
        preferred,
        ctypes.byref(actual_vendor),
        ctypes.byref(device_id),
        ctypes.byref(dedicated),
        ctypes.byref(shared),
        description,
        len(description),
    ):
        return None
    return D3D11AdapterInfo(
        description=description.value,
        vendor_id=actual_vendor.value,
        device_id=device_id.value,
        dedicated_video_memory=dedicated.value,
        shared_system_memory=shared.value,
    )


def _flatten_u16_runs(runs: tuple[U16Run, ...]) -> tuple[int, ...]:
    return tuple(
        value
        for run in runs
        for value in range(run.start, run.end + 1)
    )


def _expand_paths(plan: CompiledEffectPlan) -> tuple[_EffectPathInput, ...]:
    expanded: list[_EffectPathInput] = []
    for path in plan.paths:
        constraints = (
            *plan.shared_constraints,
            *((item.draw_index, item.allowed_u16) for item in path.constraints),
        )
        if len(constraints) > 6:
            raise ValueError("native effect plans support at most six draw constraints")
        for selected_runs in product(*(runs for _draw, runs in constraints)):
            descriptor = _EffectPathInput()
            descriptor.promoted_slot = -1 if path.promoted_slot is None else path.promoted_slot
            descriptor.constraint_count = len(constraints)
            for index, ((draw_index, _runs), run) in enumerate(
                zip(constraints, selected_runs, strict=True)
            ):
                descriptor.constraints[index] = _PathConstraintInput(
                    draw_index,
                    run.start,
                    run.end,
                    0,
                )
            expanded.append(descriptor)
    if not expanded:
        raise ValueError("compiled effect plan contains no native paths")
    return tuple(expanded)


def plan_trial_for_seed(plan: CompiledEffectPlan, seed: int) -> int:
    """Return the zero-based pivot-family trial for a Seed in this plan."""

    state = seed & 0xFFFFFFFF
    for _ in range(plan.pivot_draw_index):
        state = (LCG_MULTIPLIER * state + LCG_INCREMENT) & 0xFFFFFFFF
    high = state >> 16
    values = _flatten_u16_runs(plan.pivot_allowed_u16)
    try:
        high_index = values.index(high)
    except ValueError as error:
        raise ValueError("Seed is outside the compiled pivot preimage") from error
    return high_index * 0x10000 + (state & 0xFFFF)


def collect_effect_preimage_matches_d3d11(
    plan: CompiledEffectPlan,
    *,
    start_trial: int = 0,
    stop_trial: int | None = None,
    output_capacity: int = 100_000,
    vendor_id: int | None = None,
) -> tuple[tuple[int, int], ...] | None:
    """Return ``(Seed, zero-based plan trial)`` pairs, or ``None`` unavailable."""

    library = _load_accelerator()
    if library is None:
        return None
    pivot_values = _flatten_u16_runs(plan.pivot_allowed_u16)
    family_size = len(pivot_values) * 0x10000
    if stop_trial is None:
        stop_trial = family_size
    if not 0 <= start_trial <= stop_trial <= family_size:
        raise ValueError("invalid compiled effect pivot range")
    if not 1 <= output_capacity <= 1_000_000:
        raise ValueError("output_capacity must be in 1..1,000,000")
    if start_trial == stop_trial:
        return ()

    native_paths = _expand_paths(plan)
    pivot_array = (ctypes.c_uint16 * len(pivot_values))(*pivot_values)
    path_array = (_EffectPathInput * len(native_paths))(*native_paths)
    seed_array = (ctypes.c_uint32 * output_capacity)()
    trial_array = (ctypes.c_uint64 * output_capacity)()
    actual_vendor = ctypes.c_uint32()
    maximum_draw = max(
        item.draw_index
        for path in plan.paths
        for item in path.constraints
    )
    preferred = _configured_vendor_id() if vendor_id is None else vendor_id
    count = library.collect_effect_preimage_matches_d3d11(
        pivot_array,
        len(pivot_values),
        start_trial,
        stop_trial,
        plan.pivot_draw_index,
        plan.pivot_affine_addend,
        plan.pivot_inverse_multiplier,
        plan.promotion_draw_index,
        plan.promotion_probability_percent * 100,
        plan.shuffle_draw_start,
        plan.request.rarity,
        plan.slot_limit,
        maximum_draw,
        path_array,
        len(native_paths),
        preferred,
        seed_array,
        trial_array,
        output_capacity,
        ctypes.byref(actual_vendor),
    )
    if count == ERROR_RESULT:
        raise RuntimeError("Direct3D 11 effect preimage accelerator rejected the plan")
    global _last_d3d11_vendor_id
    _last_d3d11_vendor_id = actual_vendor.value
    return tuple(
        sorted(
            (
                (seed_array[index], trial_array[index])
                for index in range(count)
            ),
            key=lambda item: item[1],
        )
    )


def last_effect_preimage_backend() -> str:
    return {
        AMD_VENDOR_ID: "d3d11_amd",
        NVIDIA_VENDOR_ID: "d3d11_nvidia",
        INTEL_VENDOR_ID: "d3d11_intel",
    }.get(_last_d3d11_vendor_id, "not_used")


def reset_effect_preimage_backend() -> None:
    global _last_d3d11_vendor_id
    _last_d3d11_vendor_id = None


__all__ = [
    "AMD_VENDOR_ID",
    "D3D11AdapterInfo",
    "INTEL_VENDOR_ID",
    "NVIDIA_VENDOR_ID",
    "collect_effect_preimage_matches_d3d11",
    "d3d11_effect_adapter_info",
    "d3d11_effect_acceleration_available",
    "last_effect_preimage_backend",
    "plan_trial_for_seed",
    "reset_effect_preimage_backend",
]
