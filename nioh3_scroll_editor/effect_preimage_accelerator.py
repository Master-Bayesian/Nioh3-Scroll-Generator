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

from .effect_path_inverse import (
    CompiledEffectPath,
    CompiledEffectPlan,
    FullCompositionRequest,
    LotteryConstraint,
    U16Run,
    lcg_affine_for_draw,
)


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


class _EffectCandidateInput(ctypes.Structure):
    _fields_ = (
        ("effect_id", ctypes.c_uint32),
        ("group_key", ctypes.c_uint32),
        ("category_key", ctypes.c_uint32),
        ("conflict_mask_0", ctypes.c_uint32),
        ("conflict_mask_1", ctypes.c_uint32),
        ("normal_weight", ctypes.c_uint32),
        ("promoted_weight", ctypes.c_uint32),
        ("final_weight_common", ctypes.c_uint32),
        ("final_weight_special", ctypes.c_uint32),
        ("completion_candidate", ctypes.c_uint32),
        ("value_one_roll_mask", ctypes.c_uint32),
    )


class _SpecialGroupInput(ctypes.Structure):
    _fields_ = (
        ("group_key", ctypes.c_uint32),
        ("conflict_mask_0", ctypes.c_uint32),
        ("conflict_mask_1", ctypes.c_uint32),
        ("effect_id", ctypes.c_uint32),
    )


if ctypes.sizeof(_EffectPathInput) != 112:
    raise RuntimeError("Direct3D path descriptor ABI mismatch")
if ctypes.sizeof(_EffectCandidateInput) != 44:
    raise RuntimeError("Direct3D effect candidate ABI mismatch")
if ctypes.sizeof(_SpecialGroupInput) != 16:
    raise RuntimeError("Direct3D special group ABI mismatch")


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
    match_effects = library.match_effect_constraints_d3d11
    match_effects.argtypes = (
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(_EffectCandidateInput),
        ctypes.c_uint32,
        ctypes.POINTER(_SpecialGroupInput),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.c_uint32,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
    )
    match_effects.restype = ctypes.c_int
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
    if not library.d3d11_effect_acceleration_available(preferred):
        return False
    return _d3d11_effect_self_test(preferred)


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


def collect_fixed_draw_pivot_seeds_d3d11(
    pivot_values: tuple[int, ...],
    *,
    start_index: int,
    stop_index: int,
    low16_stride: int,
    pivot_draw_index: int,
    other_constraints: tuple[
        tuple[int, tuple[tuple[int, int], ...]], ...
    ] = (),
) -> tuple[tuple[int, int], ...] | None:
    """Return natural Seeds for one generic fixed-draw family through D3D11.

    The returned one-based cursor uses pivot-value-major order.  It is an
    opaque product cursor and intentionally differs from the legacy native
    CPU/CUDA low16-major cursor.
    """

    del low16_stride
    if not pivot_values:
        raise ValueError("fixed-draw pivot values cannot be empty")
    if any(not 0 <= value <= 0xFFFF for value in pivot_values):
        raise ValueError("fixed-draw pivot values must fit in uint16")
    family_size = len(pivot_values) * 0x10000
    if not 0 <= start_index <= stop_index <= family_size:
        raise ValueError("invalid fixed-draw pivot range")
    if stop_index - start_index > 8_000_000:
        raise ValueError("fixed-draw DirectCompute chunks cannot exceed 8,000,000 trials")

    constraints = tuple(
        LotteryConstraint(
            source_slot=0,
            draw_index=draw_index,
            effect_id=0,
            candidate_count=0,
            total_weight=0,
            allowed_u16=tuple(U16Run(start, end) for start, end in runs),
        )
        for draw_index, runs in other_constraints
    )
    if not constraints:
        constraints = (
            LotteryConstraint(
                source_slot=0,
                draw_index=1,
                effect_id=0,
                candidate_count=0,
                total_weight=0,
                allowed_u16=(U16Run(0, 0xFFFF),),
            ),
        )
    maximum_draw = max(item.draw_index for item in constraints)
    if maximum_draw >= 32:
        raise ValueError("fixed-draw DirectCompute supports draw indexes below 32")
    multiplier, addend = lcg_affine_for_draw(pivot_draw_index)
    request = FullCompositionRequest(
        rarity=3,
        primary_effect_id=0,
        secondary_effect_ids=(1, 2, 3),
        natural_only=True,
        playthrough=3,
    )
    plan = CompiledEffectPlan(
        request=request,
        promotion_draw_index=1,
        promotion_probability_percent=0,
        shuffle_draw_start=1,
        slot_limit=4,
        shared_constraints=(),
        paths=(
            CompiledEffectPath(
                ordered_effect_ids=(),
                promoted_slot=None,
                constraints=constraints,
            ),
        ),
        pivot_draw_index=pivot_draw_index,
        pivot_allowed_u16=tuple(U16Run(value, value) for value in pivot_values),
        pivot_affine_addend=addend,
        pivot_inverse_multiplier=pow(multiplier, -1, 1 << 32),
    )
    accelerated = collect_effect_preimage_matches_d3d11(
        plan,
        start_trial=start_index,
        stop_trial=stop_index,
        output_capacity=max(100_000, (stop_index - start_index + 7) // 8),
    )
    if accelerated is None:
        return None
    return tuple((seed, trial + 1) for seed, trial in accelerated)


def match_effect_constraints_d3d11(
    seeds: tuple[int, ...],
    *,
    candidates: tuple[
        tuple[int, int, int, int, int, int, int, int, int, int, int], ...
    ],
    special_groups: tuple[tuple[int, int, int, int], ...],
    category_capacities: tuple[int, ...],
    criterion_groups: tuple[tuple[int, frozenset[int]], ...],
    rarity: int,
    ordinary_slot_count: int,
    slot_limit: int,
    promotion_threshold: int,
    consumes_special_draw: bool,
    minimum_roll_percent: int,
    maximum_roll_percent: int,
    apply_r4_finalizer: bool,
    auxiliary_mode_threshold: int,
    vendor_id: int | None = None,
) -> tuple[int, ...] | None:
    """Forward-generate ordinary effects on any D3D11 hardware backend."""

    library = _load_accelerator()
    if library is None:
        return None
    if not seeds or len(seeds) > 1_000_000:
        raise ValueError("DirectCompute effect batches require 1..1,000,000 Seeds")
    if not candidates or len(candidates) > 4096:
        raise ValueError("DirectCompute effect candidate table is invalid")
    if len(special_groups) not in (1, 0x10000):
        raise ValueError("special group lookup must contain 1 or 65,536 entries")
    if len(category_capacities) != 32:
        raise ValueError("category capacities must contain 32 entries")
    if not criterion_groups or len(criterion_groups) > 32:
        raise ValueError("effect matching requires 1..32 criterion groups")
    flattened_keys: list[int] = []
    offsets = [0]
    kinds: list[int] = []
    for kind, group in criterion_groups:
        if kind not in (0, 1, 2) or not group:
            raise ValueError("invalid effect criterion group")
        flattened_keys.extend(sorted(group))
        offsets.append(len(flattened_keys))
        kinds.append(kind)
    seed_array = (ctypes.c_uint32 * len(seeds))(*seeds)
    candidate_array = (_EffectCandidateInput * len(candidates))(
        *(_EffectCandidateInput(*row) for row in candidates)
    )
    special_array = (_SpecialGroupInput * len(special_groups))(
        *(_SpecialGroupInput(*row) for row in special_groups)
    )
    capacity_array = (ctypes.c_uint32 * 32)(*category_capacities)
    key_array = (ctypes.c_uint32 * len(flattened_keys))(*flattened_keys)
    offset_array = (ctypes.c_uint32 * len(offsets))(*offsets)
    kind_array = (ctypes.c_uint32 * len(kinds))(*kinds)
    output_array = (ctypes.c_uint32 * len(seeds))()
    actual_vendor = ctypes.c_uint32()
    preferred = _configured_vendor_id() if vendor_id is None else vendor_id
    result = library.match_effect_constraints_d3d11(
        seed_array,
        len(seeds),
        candidate_array,
        len(candidates),
        special_array,
        len(special_groups),
        capacity_array,
        key_array,
        len(flattened_keys),
        offset_array,
        kind_array,
        len(criterion_groups),
        rarity,
        ordinary_slot_count,
        slot_limit,
        promotion_threshold,
        int(consumes_special_draw),
        minimum_roll_percent,
        maximum_roll_percent,
        int(apply_r4_finalizer),
        auxiliary_mode_threshold,
        preferred,
        output_array,
        ctypes.byref(actual_vendor),
    )
    if result == -2:
        return None
    if result != 1:
        raise RuntimeError(
            f"Direct3D 11 effect matcher failed with native status {result}"
        )
    global _last_d3d11_vendor_id
    _last_d3d11_vendor_id = actual_vendor.value
    return tuple(int(value) for value in output_array)


def last_effect_preimage_backend() -> str:
    if _last_d3d11_vendor_id is None:
        return "not_used"
    return {
        AMD_VENDOR_ID: "d3d11_amd",
        NVIDIA_VENDOR_ID: "d3d11_nvidia",
        INTEL_VENDOR_ID: "d3d11_intel",
    }.get(_last_d3d11_vendor_id, "d3d11_other")


def reset_effect_preimage_backend() -> None:
    global _last_d3d11_vendor_id
    _last_d3d11_vendor_id = None


@lru_cache(maxsize=8)
def _d3d11_effect_self_test(vendor_id: int) -> bool:
    """Reject adapters that create a device but cannot execute both shaders."""

    global _last_d3d11_vendor_id
    previous_vendor_id = _last_d3d11_vendor_id
    try:
        pivot_state = (LCG_MULTIPLIER + LCG_INCREMENT) & 0xFFFFFFFF
        pivot_low16 = pivot_state & 0xFFFF
        preimage = collect_fixed_draw_pivot_seeds_d3d11(
            (pivot_state >> 16,),
            start_index=pivot_low16,
            stop_index=pivot_low16 + 1,
            low16_stride=1,
            pivot_draw_index=1,
        )
        if preimage != ((1, pivot_low16 + 1),):
            return False

        # The R4 finalizer is the most demanding forward-filter path and has
        # exposed virtual adapters that report D3D11 support but return empty
        # output buffers. Validate it before advertising the backend.
        from .effect_batch_filter import match_partial_effect_constraints_batch
        from .grace_map import load_grace_output_map

        filtered = match_partial_effect_constraints_batch(
            (3,),
            playthrough=3,
            rarity=4,
            primary_effect_ids=frozenset(),
            required_secondary_ids=frozenset((0xDFF0,)),
            required_secondary_id_groups=(),
            special_mapping=load_grace_output_map(rarity=4),
        )
        return bool(
            filtered is not None
            and filtered.target_mask == 1
            and filtered.masks == (1,)
        )
    except (OSError, RuntimeError, ValueError):
        return False
    finally:
        _last_d3d11_vendor_id = previous_vendor_id


__all__ = [
    "AMD_VENDOR_ID",
    "D3D11AdapterInfo",
    "INTEL_VENDOR_ID",
    "NVIDIA_VENDOR_ID",
    "collect_effect_preimage_matches_d3d11",
    "collect_fixed_draw_pivot_seeds_d3d11",
    "d3d11_effect_adapter_info",
    "d3d11_effect_acceleration_available",
    "last_effect_preimage_backend",
    "match_effect_constraints_d3d11",
    "plan_trial_for_seed",
    "reset_effect_preimage_backend",
]
