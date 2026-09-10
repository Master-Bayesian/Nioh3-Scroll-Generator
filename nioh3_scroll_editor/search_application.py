"""Shared offline search orchestration. No widgets, saves, or game handles."""
from __future__ import annotations

from dataclasses import dataclass
from typing import Callable
from .auxiliary_generation import generate_complete_auxiliary, generate_matching_auxiliary
from .catalog import contextual_effect_name
from .effect_seed_solver import EffectSeedCandidate, EffectSeedIntersectionReport, EffectSeedRequest, collect_auxiliary_only_seed_page, collect_effect_seed_page, merge_intersection_reports
from .effect_batch_filter import match_partial_effect_constraints_batch
from .effect_path_inverse import FullCompositionRequest, OneWildcardCompositionRequest, compile_full_composition_plans, compile_one_wildcard_composition_plans
from .effect_preimage_accelerator import d3d11_effect_acceleration_available, reset_effect_preimage_backend
from .effect_preimage_search import collect_full_composition_preimage_page, collect_one_wildcard_composition_preimage_page
from .effect_sequence import EffectSequenceResult, collect_ng3_r4_primary_pivot_seeds, generate_ng3_certified_effect_sequence, generate_ng3_rarity34_primary_effect_ids, generate_rarity5_any_grace_primary_effect_ids, generate_rarity5_grace_effect_sequence, generate_rarity5_grace_primary_effect_id, generate_rarity5_grace_primary_effect_ids
from .models import CandidateRecordStage, ScrollCandidate, candidate_matches
from .grace_map import GraceOutputMap
from .seed_accelerator import cuda_seed_acceleration_available

def require_search_candidate_ready(candidate: ScrollCandidate) -> ScrollCandidate:
    """Reject rarity-4 stage-one records at the application search boundary."""

    if (
        candidate.rarity == 4
        and candidate.record_stage is CandidateRecordStage.NATIVE_STAGE_ONE
    ):
        raise RuntimeError(
            "稀有度4搜索结果仍是原生待揭露中间态，已拒绝加入候选列表"
        )
    return candidate

@dataclass(frozen=True, slots=True)
class SearchBatchResult:
    candidates: tuple[ScrollCandidate, ...]
    requested_count: int
    next_start_after_trial: int | None = None
    intersection_report: EffectSeedIntersectionReport | None = None
    streamed: bool = False

def collect_search_pages_until_requested(
    collector: Callable[[int, int], SearchBatchResult],
    *,
    result_count: int,
    start_after_trial: int = 0,
    cancelled: Callable[[], bool] | None = None,
) -> SearchBatchResult:
    """Continue bounded solver pages until the UI request is satisfied."""

    if result_count <= 0:
        raise ValueError("result_count must be positive")
    if start_after_trial < 0:
        raise ValueError("start_after_trial cannot be negative")
    active_cursor = start_after_trial
    candidates: list[ScrollCandidate] = []
    reports: list[EffectSeedIntersectionReport] = []
    streamed = False
    while len(candidates) < result_count:
        if cancelled is not None and cancelled():
            break
        page = collector(result_count - len(candidates), active_cursor)
        candidates.extend(page.candidates)
        streamed = streamed or page.streamed
        if page.intersection_report is not None:
            reports.append(page.intersection_report)
            if page.intersection_report.exhausted_family:
                active_cursor = page.next_start_after_trial or active_cursor
                break
        next_cursor = page.next_start_after_trial
        if next_cursor is None or next_cursor <= active_cursor:
            break
        active_cursor = next_cursor
    combined_report = (
        merge_intersection_reports(tuple(reports))
        if len(reports) > 1
        else reports[0] if reports else None
    )
    return SearchBatchResult(
        candidates=tuple(candidates[:result_count]),
        requested_count=result_count,
        next_start_after_trial=active_cursor,
        intersection_report=combined_report,
        streamed=streamed,
    )

def _complete_preimage_requests(
    request: EffectSeedRequest,
) -> tuple[FullCompositionRequest, ...]:
    """Return every exact primary assignment for one complete ordinary set.

    An unrestricted primary means every selected ordinary effect may occupy
    slot 1.  Earlier versions treated that UI mode as ineligible for the
    complete-composition inverse, which silently sent large rarity-4 Grace
    families through the generic Python replay loop.
    """

    if request.required_secondary_id_groups:
        return ()
    if request.rarity == 4:
        # The R4 finalizer can replace one stage-one ordinary effect with a
        # different final effect. Inverting only the requested final set as a
        # stage-one set is fast but incomplete. Product R4 searches therefore
        # use the finalizer-aware DirectCompute forward matcher below.
        return ()
    expected_ordinary = {3: 4, 4: 4, 5: 5}.get(request.rarity)
    if expected_ordinary is None:
        return ()
    if request.rarity == 3 and request.grace_effect_id is not None:
        return ()
    if request.rarity == 4 and request.grace_effect_id is None:
        # Without a requested retained Grace, the finalizer may replace the
        # transient fifth slot with another ordinary effect.  Four selected
        # IDs are therefore not a complete final composition.
        return ()
    if request.rarity == 5 and request.grace_effect_id is None:
        return ()

    if request.primary_effect_ids:
        primary_options = tuple(sorted(request.primary_effect_ids))
    elif len(request.required_secondary_ids) == expected_ordinary:
        primary_options = tuple(sorted(request.required_secondary_ids))
    else:
        return ()

    inverse_requests: list[FullCompositionRequest] = []
    for primary_effect_id in primary_options:
        secondary_ids = set(request.required_secondary_ids)
        secondary_ids.discard(primary_effect_id)
        if len(secondary_ids) != expected_ordinary - 1:
            continue
        try:
            inverse_request = FullCompositionRequest(
                rarity=request.rarity,
                primary_effect_id=primary_effect_id,
                secondary_effect_ids=tuple(sorted(secondary_ids)),
                stage_special_effect_id=request.grace_effect_id,
                natural_only=request.natural_only,
                playthrough=request.playthrough,
            )
        except ValueError:
            continue
        if inverse_request not in inverse_requests:
            inverse_requests.append(inverse_request)
    return tuple(inverse_requests)

def _complete_preimage_request(
    request: EffectSeedRequest,
) -> FullCompositionRequest | None:
    """Return the sole exact-primary request kept for compatibility tests."""

    inverse_requests = _complete_preimage_requests(request)
    return inverse_requests[0] if len(inverse_requests) == 1 else None

def _one_wildcard_preimage_request(
    request: EffectSeedRequest,
) -> OneWildcardCompositionRequest | None:
    """Return the common unrestricted-primary request with one open slot."""

    if (
        request.primary_effect_ids
        or request.required_secondary_id_groups
        or request.rarity != 5
        or request.grace_effect_id is None
    ):
        return None
    expected_required_count = 3 if request.rarity == 4 else 4
    if len(request.required_secondary_ids) != expected_required_count:
        return None
    try:
        return OneWildcardCompositionRequest(
            rarity=request.rarity,
            required_effect_ids=tuple(sorted(request.required_secondary_ids)),
            stage_special_effect_id=request.grace_effect_id,
            natural_only=request.natural_only,
            playthrough=request.playthrough,
        )
    except ValueError:
        return None

def _legal_complete_preimage_layouts(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None,
) -> tuple[tuple[FullCompositionRequest, int], ...] | None:
    """Compile exact paths and reject an exhausted complete composition."""

    inverse_requests = _complete_preimage_requests(request)
    if not inverse_requests:
        return None
    layouts: list[tuple[FullCompositionRequest, int]] = []
    for inverse_request in inverse_requests:
        try:
            plans = compile_full_composition_plans(
                inverse_request,
                special_mapping=grace_mapping,
            )
        except ValueError:
            continue
        family_size = sum(plan.pivot_state_count for plan in plans)
        if family_size:
            layouts.append((inverse_request, family_size))
    if layouts:
        return tuple(layouts)

    selected_ids = sorted(
        request.primary_effect_ids | request.required_secondary_ids
    )
    selected_text = "、".join(
        f"{contextual_effect_name(effect_id, rarity=request.rarity, slot=1)} "
        f"[0x{effect_id:04X}]"
        for effect_id in selected_ids
    )
    raise ValueError(
        "所选完整词条组合在原生逐槽抽取路径中无解："
        "无论把哪一项作为主词条，后续候选池都会在生成完成前变为空。"
        f"当前组合：{selected_text}。"
    )

def _sequence_satisfies_roll_filters(
    sequence,
    minimum_rolls: tuple[tuple[int, int], ...],
) -> bool:
    ordinary = (sequence.primary, *sequence.secondaries)
    return all(
        any(
            effect.effect_id == effect_id and effect.roll_percent >= minimum_roll
            for effect in ordinary
        )
        for effect_id, minimum_roll in minimum_rolls
    )

def collect_offline_complete_preimage_search_batch(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None,
    level: int,
    result_count: int,
    max_trials_per_batch: int,
    start_after_trial: int = 0,
    candidate_found: Callable[[ScrollCandidate], None] | None = None,
    cancelled: Callable[[], bool] | None = None,
) -> SearchBatchResult | None:
    """Use the path inverse when the user specified every ordinary slot."""

    layouts = _legal_complete_preimage_layouts(
        request,
        grace_mapping=grace_mapping,
    )
    if layouts is None:
        return None
    if not d3d11_effect_acceleration_available():
        raise RuntimeError(
            "当前完整词条条件需要 DirectCompute GPU 求解，但没有可用的硬件后端；"
            "已停止计算，不会回退到慢速 CPU/Python。"
        )
    total_family_size = sum(family_size for _request, family_size in layouts)
    active_cursor = min(start_after_trial, total_family_size)
    budget_stop = min(
        total_family_size,
        active_cursor + max_trials_per_batch,
    )
    candidates: list[ScrollCandidate] = []
    while (
        len(candidates) < result_count
        and active_cursor < budget_stop
        and not (cancelled is not None and cancelled())
    ):
        layout_offset = 0
        active_layout: tuple[FullCompositionRequest, int] | None = None
        for inverse_request, family_size in layouts:
            if active_cursor < layout_offset + family_size:
                active_layout = (inverse_request, family_size)
                break
            layout_offset += family_size
        if active_layout is None:
            break
        inverse_request, family_size = active_layout
        local_cursor = max(0, active_cursor - layout_offset)
        page = collect_full_composition_preimage_page(
            inverse_request,
            page_size=max(64, (result_count - len(candidates)) * 8),
            special_mapping=grace_mapping,
            start_after_trial=local_cursor,
            max_trials=min(
                family_size - local_cursor,
                budget_stop - active_cursor,
            ),
            cancelled=cancelled,
        )
        if page is None:
            raise RuntimeError(
                "DirectCompute GPU 求解器在计算中不可用；已停止计算，"
                "不会回退到慢速 CPU/Python。"
            )
        previous_cursor = active_cursor
        active_cursor = layout_offset + page.next_start_after_trial
        for match in page.matches:
            if cancelled is not None and cancelled():
                active_cursor = layout_offset + match.pivot_trial - 1
                break
            sequence = (
                generate_ng3_certified_effect_sequence(
                    match.seed,
                    rarity=request.rarity,
                    level=level,
                )
                if request.playthrough == 3
                else generate_rarity5_grace_effect_sequence(
                    match.seed,
                    playthrough=request.playthrough,
                    level=level,
                    grace_mapping=grace_mapping,
                )
            )
            if not _sequence_satisfies_roll_filters(
                sequence,
                request.minimum_roll_percent_by_effect_id,
            ):
                continue
            if request.grace_effect_id is not None and (
                not sequence.terminal_is_special
                or sequence.grace.effect_id != request.grace_effect_id
            ):
                continue
            auxiliary = generate_matching_auxiliary(
                match.seed,
                request.playthrough,
                criteria=request.auxiliary_criteria,
            )
            if auxiliary is None:
                continue
            candidate = ScrollCandidate.from_effect_sequence(
                sequence,
                auxiliary=auxiliary,
                joint_search_trial=layout_offset + match.pivot_trial,
            )
            if not candidate_matches(
                candidate,
                primary_effect_ids=request.primary_effect_ids,
                required_secondary_ids=request.required_secondary_ids,
                required_secondary_id_groups=request.required_secondary_id_groups,
            ):
                continue
            candidates.append(candidate)
            if candidate_found is not None:
                candidate_found(candidate)
            if len(candidates) >= result_count:
                active_cursor = layout_offset + match.pivot_trial
                break
        if active_cursor <= previous_cursor:
            break
    return SearchBatchResult(
        candidates=tuple(candidates),
        requested_count=result_count,
        next_start_after_trial=active_cursor,
        streamed=candidate_found is not None,
    )

def collect_offline_one_wildcard_preimage_search_batch(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None,
    level: int,
    result_count: int,
    max_trials_per_batch: int,
    start_after_trial: int = 0,
    candidate_found: Callable[[ScrollCandidate], None] | None = None,
    cancelled: Callable[[], bool] | None = None,
) -> SearchBatchResult | None:
    """Use one GPU family when exactly one ordinary effect is unrestricted."""

    inverse_request = _one_wildcard_preimage_request(request)
    if inverse_request is None:
        return None
    try:
        plans = compile_one_wildcard_composition_plans(
            inverse_request,
            special_mapping=grace_mapping,
        )
    except ValueError as error:
        selected_text = "、".join(
            f"{contextual_effect_name(effect_id, rarity=request.rarity, slot=1)} "
            f"[0x{effect_id:04X}]"
            for effect_id in sorted(request.required_secondary_ids)
        )
        raise ValueError(
            "所选词条无法与任意第四词条组成原生合法绘卷："
            f"{selected_text}。"
        ) from error
    if not d3d11_effect_acceleration_available():
        raise RuntimeError(
            "当前部分词条条件需要 DirectCompute GPU 求解，但没有可用的硬件后端；"
            "已停止计算，不会回退到慢速 CPU/Python。"
        )
    family_size = sum(plan.pivot_state_count for plan in plans)
    active_cursor = min(start_after_trial, family_size)
    budget_stop = min(family_size, active_cursor + max_trials_per_batch)
    candidates: list[ScrollCandidate] = []
    while (
        len(candidates) < result_count
        and active_cursor < budget_stop
        and not (cancelled is not None and cancelled())
    ):
        page = collect_one_wildcard_composition_preimage_page(
            inverse_request,
            page_size=max(64, (result_count - len(candidates)) * 8),
            special_mapping=grace_mapping,
            start_after_trial=active_cursor,
            max_trials=budget_stop - active_cursor,
            cancelled=cancelled,
        )
        if page is None:
            raise RuntimeError(
                "DirectCompute GPU 求解器在计算中不可用；已停止计算，"
                "不会回退到慢速 CPU/Python。"
            )
        previous_cursor = active_cursor
        active_cursor = page.next_start_after_trial
        for match in page.matches:
            if cancelled is not None and cancelled():
                active_cursor = match.pivot_trial - 1
                break
            sequence = (
                generate_ng3_certified_effect_sequence(
                    match.seed,
                    rarity=request.rarity,
                    level=level,
                )
                if request.playthrough == 3
                else generate_rarity5_grace_effect_sequence(
                    match.seed,
                    playthrough=request.playthrough,
                    level=level,
                    grace_mapping=grace_mapping,
                )
            )
            if not _sequence_satisfies_roll_filters(
                sequence,
                request.minimum_roll_percent_by_effect_id,
            ):
                continue
            if (
                request.grace_effect_id is not None
                and (
                    not sequence.terminal_is_special
                    or sequence.grace.effect_id != request.grace_effect_id
                )
            ):
                continue
            auxiliary = generate_matching_auxiliary(
                match.seed,
                request.playthrough,
                criteria=request.auxiliary_criteria,
            )
            if auxiliary is None:
                continue
            candidate = ScrollCandidate.from_effect_sequence(
                sequence,
                auxiliary=auxiliary,
                joint_search_trial=match.pivot_trial,
            )
            if not candidate_matches(
                candidate,
                primary_effect_ids=request.primary_effect_ids,
                required_secondary_ids=request.required_secondary_ids,
                required_secondary_id_groups=request.required_secondary_id_groups,
            ):
                continue
            candidates.append(candidate)
            if candidate_found is not None:
                candidate_found(candidate)
            if len(candidates) >= result_count:
                active_cursor = match.pivot_trial
                break
        if active_cursor <= previous_cursor:
            break
    return SearchBatchResult(
        candidates=tuple(candidates),
        requested_count=result_count,
        next_start_after_trial=active_cursor,
        streamed=candidate_found is not None,
    )

def require_accelerated_generic_search(
    request: EffectSeedRequest,
    *,
    allow_cpu_fallback: bool = False,
) -> None:
    """Reject generic searches that would silently enter bulk Python replay."""

    criteria = request.auxiliary_criteria
    if not (
        d3d11_effect_acceleration_available()
        or cuda_seed_acceleration_available()
    ) and not allow_cpu_fallback:
        raise RuntimeError(
            "没有检测到可用的 GPU 搜索后端；已停止计算，不会回退到 CPU/Python。"
        )
    requires_cuda_prefilter = not criteria.is_empty
    if (
        requires_cuda_prefilter
        and not cuda_seed_acceleration_available()
        and not allow_cpu_fallback
    ):
        raise RuntimeError(
            "当前地形、敌人或特殊规则路径需要 CUDA 批量前筛，但 CUDA 不可用；"
            "已停止计算，不会回退到慢速 CPU。"
        )

def request_is_auxiliary_only(request: EffectSeedRequest) -> bool:
    """Return whether every selected condition belongs to auxiliary output."""

    return bool(
        not request.auxiliary_criteria.is_empty
        and not request.primary_effect_ids
        and not request.required_secondary_ids
        and not request.required_secondary_id_groups
        and request.grace_effect_id is None
        and not request.minimum_roll_percent_by_effect_id
    )

def collect_offline_auxiliary_only_search_batch(
    request: EffectSeedRequest,
    *,
    effect_sequence_generator: Callable[[int], EffectSequenceResult],
    result_count: int,
    max_trials_per_batch: int,
    start_after_trial: int,
    intersection_progress: Callable[[EffectSeedIntersectionReport], None] | None,
    candidate_found: Callable[[ScrollCandidate], None] | None,
    cancelled: Callable[[], bool] | None,
) -> SearchBatchResult:
    """Materialize one fused auxiliary-only search page."""

    materialized_by_trial: dict[int, ScrollCandidate] = {}

    def materialize(match: EffectSeedCandidate) -> ScrollCandidate:
        cached = materialized_by_trial.get(match.pivot_trial)
        if cached is not None:
            return cached
        if match.effect_sequence is None or match.auxiliary is None:
            raise RuntimeError("fused auxiliary solver returned an incomplete preview")
        candidate = ScrollCandidate.from_effect_sequence(
            match.effect_sequence,
            auxiliary=match.auxiliary,
            joint_search_trial=match.pivot_trial,
        )
        materialized_by_trial[match.pivot_trial] = candidate
        return candidate

    page = collect_auxiliary_only_seed_page(
        request,
        page_size=result_count,
        effect_sequence_generator=effect_sequence_generator,
        start_after_trial=start_after_trial,
        max_trials=max_trials_per_batch,
        intersection_progress=intersection_progress,
        candidate_found=(
            (lambda match: candidate_found(materialize(match)))
            if candidate_found is not None
            else None
        ),
        cancelled=cancelled,
    )
    return SearchBatchResult(
        candidates=tuple(materialize(match) for match in page.candidates),
        requested_count=result_count,
        next_start_after_trial=page.next_start_after_trial,
        intersection_report=page.intersection_report,
        streamed=candidate_found is not None,
    )

def partial_effect_batch_generator(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None,
    level: int,
    allow_cpu_fallback: bool = False,
) -> Callable[[tuple[int, ...]], tuple[tuple[int, ...], int] | None] | None:
    """Build a fail-closed D3D11 forward filter for partial effect requests."""

    if not (
        request.primary_effect_ids
        or request.required_secondary_ids
        or request.required_secondary_id_groups
    ):
        return None
    if (
        request.primary_effect_ids
        and not request.required_secondary_ids
        and not request.required_secondary_id_groups
        and cuda_seed_acceleration_available()
    ):
        return None
    if request.rarity not in (3, 4, 5):
        return None

    def generate(seeds: tuple[int, ...]) -> tuple[tuple[int, ...], int] | None:
        result = match_partial_effect_constraints_batch(
            seeds,
            playthrough=request.playthrough,
            rarity=request.rarity,
            primary_effect_ids=request.primary_effect_ids,
            required_secondary_ids=request.required_secondary_ids,
            required_secondary_id_groups=request.required_secondary_id_groups,
            special_mapping=grace_mapping,
            level=level,
        )
        if result is None:
            if allow_cpu_fallback:
                return None
            raise RuntimeError(
                "DirectCompute partial-effect matcher is unavailable; "
                "CPU fallback is disabled"
            )
        return result.masks, result.target_mask

    return generate

def collect_offline_rarity5_search_batch(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap,
    level: int,
    result_count: int,
    max_trials_per_batch: int,
    start_after_trial: int = 0,
    intersection_progress: Callable[[EffectSeedIntersectionReport], None] | None = None,
    candidate_found: Callable[[ScrollCandidate], None] | None = None,
    cancelled: Callable[[], bool] | None = None,
    allow_cpu_fallback: bool = False,
) -> SearchBatchResult:
    """Run one exact NG3-NG5 rarity-5 search without a game process."""

    reset_effect_preimage_backend()
    if request.playthrough not in (3, 4, 5) or request.rarity != 5:
        raise ValueError("offline Grace search requires NG3-NG5 rarity 5")
    require_accelerated_generic_search(
        request,
        allow_cpu_fallback=allow_cpu_fallback,
    )
    if request_is_auxiliary_only(request):
        return collect_offline_auxiliary_only_search_batch(
            request,
            effect_sequence_generator=lambda seed: generate_rarity5_grace_effect_sequence(
                seed,
                playthrough=request.playthrough,
                level=level,
                grace_mapping=grace_mapping,
            ),
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            intersection_progress=intersection_progress,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
    accelerated = (
        collect_offline_complete_preimage_search_batch(
            request,
            grace_mapping=grace_mapping,
            level=level,
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
        if d3d11_effect_acceleration_available() or not allow_cpu_fallback
        else None
    )
    if accelerated is not None:
        return accelerated
    accelerated = (
        collect_offline_one_wildcard_preimage_search_batch(
            request,
            grace_mapping=grace_mapping,
            level=level,
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
        if d3d11_effect_acceleration_available() or not allow_cpu_fallback
        else None
    )
    if accelerated is not None:
        return accelerated
    completed_reports: list[EffectSeedIntersectionReport] = []
    matches = []
    materialized_by_trial: dict[int, ScrollCandidate] = {}
    active_cursor = start_after_trial
    budget_stop = start_after_trial + max_trials_per_batch

    def materialize_match(match: EffectSeedCandidate) -> ScrollCandidate:
        cached = materialized_by_trial.get(match.pivot_trial)
        if cached is not None:
            return cached
        if match.effect_sequence is None:
            raise RuntimeError("offline solver returned no effect sequence")
        auxiliary = match.auxiliary or generate_complete_auxiliary(
            match.seed,
            request.playthrough,
        )
        candidate = ScrollCandidate.from_effect_sequence(
            match.effect_sequence,
            auxiliary=auxiliary,
            joint_search_trial=match.pivot_trial,
        )
        materialized_by_trial[match.pivot_trial] = candidate
        return candidate

    def emit_match(match: EffectSeedCandidate) -> None:
        if candidate_found is not None:
            candidate_found(materialize_match(match))
    while (
        len(matches) < result_count
        and active_cursor < budget_stop
        and not (cancelled is not None and cancelled())
    ):

        def report_progress(update: EffectSeedIntersectionReport) -> None:
            reports = (*completed_reports, update)
            combined = merge_intersection_reports(reports) if len(reports) > 1 else update
            if intersection_progress is not None:
                intersection_progress(combined)

        page = collect_effect_seed_page(
            request,
            page_size=result_count - len(matches),
            grace_mapping=grace_mapping,
            effect_sequence_generator=lambda seed: generate_rarity5_grace_effect_sequence(
                seed,
                playthrough=request.playthrough,
                level=level,
                grace_mapping=grace_mapping,
            ),
            primary_effect_id_generator=lambda seed: (
                generate_rarity5_grace_primary_effect_id(
                    seed,
                    playthrough=request.playthrough,
                    grace_mapping=grace_mapping,
                )
            ),
            primary_effect_id_batch_generator=lambda seeds: (
                generate_rarity5_grace_primary_effect_ids(
                    seeds,
                    playthrough=request.playthrough,
                    grace_id=request.grace_effect_id,
                    grace_mapping=grace_mapping,
                )
                if request.grace_effect_id is not None
                else generate_rarity5_any_grace_primary_effect_ids(
                    seeds,
                    playthrough=request.playthrough,
                    grace_mapping=grace_mapping,
                )
            ),
            effect_constraint_mask_batch_generator=partial_effect_batch_generator(
                request,
                grace_mapping=grace_mapping,
                level=level,
                allow_cpu_fallback=allow_cpu_fallback,
            ),
            allow_full_seed_family=request.grace_effect_id is None,
            start_after_trial=active_cursor,
            max_trials=budget_stop - active_cursor,
            intersection_progress=report_progress,
            candidate_found=emit_match,
            cancelled=cancelled,
            # CUDA has substantially lower setup cost for pivot enumeration on
            # NVIDIA. DirectCompute remains the cross-vendor path when CUDA is
            # unavailable; neither route is allowed to fall back to bulk CPU.
            prefer_d3d11_fixed_draw=not cuda_seed_acceleration_available(),
            allow_cpu_fallback=allow_cpu_fallback,
        )
        matches.extend(page.candidates)
        if page.intersection_report is not None:
            completed_reports.append(page.intersection_report)
        previous_cursor = active_cursor
        active_cursor = page.next_start_after_trial
        if (
            (cancelled is not None and cancelled())
            or active_cursor
            >= (
                page.intersection_report.family_size
                if page.intersection_report is not None
                else active_cursor
            )
            or active_cursor <= previous_cursor
        ):
            break

    candidates: list[ScrollCandidate] = []
    for match in matches:
        candidates.append(materialize_match(match))
    combined_report = (
        merge_intersection_reports(tuple(completed_reports))
        if completed_reports
        else None
    )
    return SearchBatchResult(
        tuple(candidates),
        result_count,
        next_start_after_trial=active_cursor,
        intersection_report=combined_report,
        streamed=candidate_found is not None,
    )

def collect_offline_ng3_search_batch(
    request: EffectSeedRequest,
    *,
    grace_mapping: GraceOutputMap | None,
    level: int,
    result_count: int,
    max_trials_per_batch: int,
    start_after_trial: int = 0,
    intersection_progress: Callable[[EffectSeedIntersectionReport], None] | None = None,
    candidate_found: Callable[[ScrollCandidate], None] | None = None,
    cancelled: Callable[[], bool] | None = None,
    allow_cpu_fallback: bool = False,
) -> SearchBatchResult:
    """Run one certified NG3 rarity-3/4/5 search without a game or save."""

    reset_effect_preimage_backend()
    if request.playthrough != 3 or request.rarity not in (3, 4, 5):
        raise ValueError("offline NG3 search requires playthrough 3 and rarity 3, 4, or 5")
    if request.rarity == 5:
        if grace_mapping is None:
            raise ValueError("rarity-5 offline search requires the certified Grace map")
        return collect_offline_rarity5_search_batch(
            request,
            grace_mapping=grace_mapping,
            level=level,
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            intersection_progress=intersection_progress,
            candidate_found=candidate_found,
            cancelled=cancelled,
            allow_cpu_fallback=allow_cpu_fallback,
        )
    if request.rarity == 3 and request.grace_effect_id is not None:
        raise ValueError("rarity-3 has no selectable final Grace")
    if request.rarity == 4 and request.grace_effect_id is not None:
        if grace_mapping is None or grace_mapping.rarity != 4:
            raise ValueError("rarity-4 final Grace filtering requires the R4 draw-1 map")
    require_accelerated_generic_search(
        request,
        allow_cpu_fallback=allow_cpu_fallback,
    )
    if request_is_auxiliary_only(request):
        return collect_offline_auxiliary_only_search_batch(
            request,
            effect_sequence_generator=lambda seed: generate_ng3_certified_effect_sequence(
                seed,
                rarity=request.rarity,
                level=level,
            ),
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            intersection_progress=intersection_progress,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
    accelerated = (
        collect_offline_complete_preimage_search_batch(
            request,
            grace_mapping=grace_mapping,
            level=level,
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
        if d3d11_effect_acceleration_available() or not allow_cpu_fallback
        else None
    )
    if accelerated is not None:
        return accelerated
    accelerated = (
        collect_offline_one_wildcard_preimage_search_batch(
            request,
            grace_mapping=grace_mapping,
            level=level,
            result_count=result_count,
            max_trials_per_batch=max_trials_per_batch,
            start_after_trial=start_after_trial,
            candidate_found=candidate_found,
            cancelled=cancelled,
        )
        if d3d11_effect_acceleration_available() or not allow_cpu_fallback
        else None
    )
    if accelerated is not None:
        return accelerated

    completed_reports: list[EffectSeedIntersectionReport] = []
    matches = []
    materialized_by_trial: dict[int, ScrollCandidate] = {}
    active_cursor = start_after_trial
    budget_stop = start_after_trial + max_trials_per_batch

    def materialize_match(match: EffectSeedCandidate) -> ScrollCandidate:
        cached = materialized_by_trial.get(match.pivot_trial)
        if cached is not None:
            return cached
        if match.effect_sequence is None:
            raise RuntimeError("offline NG3 solver returned no effect sequence")
        auxiliary = match.auxiliary or generate_complete_auxiliary(match.seed, 3)
        candidate = ScrollCandidate.from_effect_sequence(
            match.effect_sequence,
            auxiliary=auxiliary,
            joint_search_trial=match.pivot_trial,
        )
        materialized_by_trial[match.pivot_trial] = candidate
        return candidate

    def emit_match(match: EffectSeedCandidate) -> None:
        if candidate_found is not None:
            candidate_found(materialize_match(match))
    while (
        len(matches) < result_count
        and active_cursor < budget_stop
        and not (cancelled is not None and cancelled())
    ):

        def report_progress(update: EffectSeedIntersectionReport) -> None:
            reports = (*completed_reports, update)
            combined = merge_intersection_reports(reports) if len(reports) > 1 else update
            if intersection_progress is not None:
                intersection_progress(combined)

        page = collect_effect_seed_page(
            request,
            page_size=result_count - len(matches),
            grace_mapping=grace_mapping,
            effect_sequence_generator=lambda seed: generate_ng3_certified_effect_sequence(
                seed,
                rarity=request.rarity,
                level=level,
            ),
            primary_effect_id_batch_generator=lambda seeds: (
                generate_ng3_rarity34_primary_effect_ids(
                    seeds,
                    rarity=request.rarity,
                )
            ),
            effect_constraint_mask_batch_generator=partial_effect_batch_generator(
                request,
                grace_mapping=grace_mapping,
                level=level,
                allow_cpu_fallback=allow_cpu_fallback,
            ),
            allow_full_seed_family=request.grace_effect_id is None,
            start_after_trial=active_cursor,
            max_trials=budget_stop - active_cursor,
            intersection_progress=report_progress,
            candidate_found=emit_match,
            cancelled=cancelled,
            pivot_seed_collector=(
                (
                    lambda values, start_index, stop_index, low16_stride: (
                        collect_ng3_r4_primary_pivot_seeds(
                            values,
                            start_index=start_index,
                            stop_index=stop_index,
                            low16_stride=low16_stride,
                            primary_effect_ids=request.primary_effect_ids,
                            special_mapping=grace_mapping,
                            require_cuda=not allow_cpu_fallback,
                        )
                    )
                )
                if (
                    request.rarity == 4
                    and request.primary_effect_ids
                    and cuda_seed_acceleration_available()
                )
                else None
            ),
            pivot_seed_collector_chunk_trials=50_000_000,
            # CUDA has substantially lower setup cost for pivot enumeration on
            # NVIDIA. DirectCompute remains the cross-vendor path when CUDA is
            # unavailable; neither route may fall back to bulk CPU.
            prefer_d3d11_fixed_draw=not cuda_seed_acceleration_available(),
            allow_cpu_fallback=allow_cpu_fallback,
        )
        matches.extend(page.candidates)
        if page.intersection_report is not None:
            completed_reports.append(page.intersection_report)
        previous_cursor = active_cursor
        active_cursor = page.next_start_after_trial
        if (
            (cancelled is not None and cancelled())
            or active_cursor
            >= (
                page.intersection_report.family_size
                if page.intersection_report is not None
                else active_cursor
            )
            or active_cursor <= previous_cursor
        ):
            break

    candidates: list[ScrollCandidate] = []
    for match in matches:
        candidates.append(materialize_match(match))
    combined_report = (
        merge_intersection_reports(tuple(completed_reports))
        if completed_reports
        else None
    )
    return SearchBatchResult(
        tuple(candidates),
        result_count,
        next_start_after_trial=active_cursor,
        intersection_report=combined_report,
        streamed=candidate_found is not None,
    )
