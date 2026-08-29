from __future__ import annotations

import ctypes
import os
import faulthandler
import hashlib
import queue
import struct
import sys
import threading
import traceback
import webbrowser
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Callable, Mapping
from tkinter import (
    BOTH,
    Canvas,
    DISABLED,
    END,
    LEFT,
    RIGHT,
    WORD,
    BooleanVar,
    Listbox,
    StringVar,
    Text,
    Tk,
    Toplevel,
    PhotoImage,
    messagebox,
)
from tkinter import ttk

from .auxiliary_catalog import load_auxiliary_name_catalog
from .auxiliary_generation import (
    TERRAIN_DISPLAY_CRUCIBLE_KEY,
    TERRAIN_DISPLAY_SPECIAL_KEYS,
    AuxiliarySearchCriteria,
    SpecialRuleEntryResult,
    describe_special_rule,
    generate_complete_auxiliary,
    load_default_auxiliary_generation_tables,
)
from .auxiliary_feasibility import EnemyKeyRequirement, analyze_enemy_feasibility
from .catalog import (
    EFFECT_BY_ID,
    contextual_effect_name,
    effect_name,
    native_effect_definitions,
    searchable_scroll_effect_definitions,
    target_effects_for_rarity,
)
from .effect_seed_solver import (
    EffectSeedCandidate,
    EffectSeedIntersectionReport,
    EffectSeedRequest,
    IntersectionStageCount,
    collect_effect_seed_page,
    merge_intersection_reports,
    validate_effect_request_feasibility,
)
from .effect_sequence import (
    generate_ng3_certified_effect_sequence,
    generate_ng3_rarity34_primary_effect_ids,
    generate_ng3_rarity5_effect_sequence,
    generate_rarity5_any_grace_primary_effect_ids,
    generate_rarity5_grace_effect_sequence,
    generate_rarity5_grace_primary_effect_id,
    generate_rarity5_grace_primary_effect_ids,
)
from .models import (
    CandidateRecordStage,
    ScrollCandidate,
    candidate_has_expected_effect_count,
)
from .grace_map import (
    GraceMapProgress,
    GraceOutputMap,
    build_live_grace_output_map,
    first_u16_ranges_for_grace,
    load_grace_map_cache,
    load_grace_output_map,
    save_grace_map_cache,
)
from .game_compatibility import detect_game_compatibility
from .native import NativeBatchOracle, ScanProgress, build_source_record, scan_next_candidate
from .primary_map import (
    PrimaryFirstDrawOutputMap,
    PrimaryMapProgress,
    PrimaryOutputMap,
    build_primary_first_draw_output_map,
    build_primary_output_map,
    load_primary_map,
    save_primary_map,
)
from .savegame import (
    BackupEntry,
    LocalEffectEdit,
    SaveCrypto,
    SaveInstaller,
    SaveInventory,
    ScrollInventoryEntry,
    default_crypto_tool,
    discover_save_paths,
    list_backup_entries,
    move_backup_to_recycle_bin,
    patch_local_scroll_record,
)
from .seed_accelerator import (
    cuda_seed_acceleration_available,
    last_seed_acceleration_backend,
)
from .updater import (
    DownloadedUpdate,
    UpdateCheckResult,
    check_for_update,
    download_update,
    ensure_managed_install,
    launch_managed_update,
    prepare_managed_update_script,
)
from .version import (
    APP_AUTHORS,
    APP_VERSION,
    CONTACT_QQ_GROUP,
    PROJECT_GITHUB_URL,
    UPDATE_MANIFEST_URL,
    UPDATE_PUBLIC_KEY_BASE64,
)


RESEARCH_MODE = os.environ.get("NIOH3_SCROLL_RESEARCH_MODE", "").strip() == "1"
STARTUP_TRACE = os.environ.get("NIOH3_SCROLL_STARTUP_TRACE", "").strip() == "1"


def startup_trace(message: str) -> None:
    if STARTUP_TRACE:
        print(f"[startup] {message}", flush=True)


PLAYTHROUGH_CURRENT_LABEL = "当前周目"
PLAYTHROUGH_LABELS = (
    "一周目",
    "二周目",
    "三周目（顿悟）",
    "四周目（DLC2 未开放·研究）",
    "五周目（DLC2 未开放·研究）",
)
PLAYTHROUGH_BY_LABEL = {
    label: index
    for index, label in enumerate(PLAYTHROUGH_LABELS, start=1)
}
NO_GRACE_FILTER_LABEL = "不限制恩宠（允许最终无恩宠）"
NO_TERRAIN_FILTER_LABEL = "任意地形影响（不筛选）"
PRODUCT_RARITIES = (3, 4)
EFFECT_ROLL_FILTERS = (
    ("任意数值", 0),
    ("较高（抽取百分位 ≥ 80）", 80),
    ("很高（抽取百分位 ≥ 90）", 90),
    ("最高（抽取百分位 100）", 100),
)
EFFECT_ROLL_BY_LABEL = dict(EFFECT_ROLL_FILTERS)
SearchCriteria = tuple[
    frozenset[int],
    frozenset[int],
    int | None,
    int,
    int,
    int,
    AuxiliarySearchCriteria,
    int | None,
    tuple[tuple[int, int], ...],
]


@dataclass(frozen=True, slots=True)
class SearchBatchResult:
    candidates: tuple[ScrollCandidate, ...]
    requested_count: int
    next_start_after_trial: int | None = None
    intersection_report: EffectSeedIntersectionReport | None = None
    streamed: bool = False


@dataclass(frozen=True, slots=True)
class RuleFilterOption:
    """One selectable special-rule group or exact native value variant."""

    token: str
    name: str
    keys: frozenset[int]
    exact: bool
    label: str


def special_rule_variant_label(name: str, key: int, value_text: str) -> str:
    """Keep the rule name visible when an exact value variant is selected."""

    return f"{name}：{value_text}  [0x{key:04X}]"


def toggle_rule_filter_option(
    selected_tokens: set[str],
    token: str,
    family_by_token: Mapping[str, frozenset[str]],
) -> set[str]:
    """Toggle one rule while preserving selections from other rule families."""

    family = family_by_token.get(token)
    if family is None:
        raise KeyError(token)
    updated = set(selected_tokens)
    already_selected = token in updated
    updated.difference_update(family)
    if not already_selected:
        updated.add(token)
    return updated


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
) -> SearchBatchResult:
    """Run one exact NG3-NG5 rarity-5 search without a game process."""

    if request.playthrough not in (3, 4, 5) or request.rarity != 5:
        raise ValueError("offline Grace search requires NG3-NG5 rarity 5")
    completed_reports: list[EffectSeedIntersectionReport] = []
    matches = []
    materialized_by_trial: dict[int, ScrollCandidate] = {}
    active_cursor = start_after_trial

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
    while len(matches) < result_count and not (cancelled is not None and cancelled()):

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
            allow_full_seed_family=request.grace_effect_id is None,
            start_after_trial=active_cursor,
            max_trials=max_trials_per_batch,
            intersection_progress=report_progress,
            candidate_found=emit_match,
            cancelled=cancelled,
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
) -> SearchBatchResult:
    """Run one certified NG3 rarity-3/4/5 search without a game or save."""

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
        )
    if request.rarity == 3 and request.grace_effect_id is not None:
        raise ValueError("rarity-3 has no selectable final Grace")
    if request.rarity == 4 and request.grace_effect_id is not None:
        if grace_mapping is None or grace_mapping.rarity != 4:
            raise ValueError("rarity-4 final Grace filtering requires the R4 draw-1 map")

    completed_reports: list[EffectSeedIntersectionReport] = []
    matches = []
    materialized_by_trial: dict[int, ScrollCandidate] = {}
    active_cursor = start_after_trial

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
    while len(matches) < result_count and not (cancelled is not None and cancelled()):

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
            allow_full_seed_family=request.grace_effect_id is None,
            start_after_trial=active_cursor,
            max_trials=max_trials_per_batch,
            intersection_progress=report_progress,
            candidate_found=emit_match,
            cancelled=cancelled,
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


def playthrough_label(playthrough: int | None) -> str:
    return PLAYTHROUGH_CURRENT_LABEL if playthrough is None else PLAYTHROUGH_LABELS[playthrough - 1]


def is_game_closed_effect_context(criteria: SearchCriteria) -> bool:
    """Return whether the UI can solve effects without a game or save."""

    return criteria[7] == 3 and criteria[3] in (3, 4, 5)


def is_cached_game_closed_effect_context(criteria: SearchCriteria) -> bool:
    """Return whether a saved live map can make the context game-closed."""

    return (
        criteria[7] in (4, 5)
        and criteria[3] == 5
    )


def primary_map_cache_path(
    state_root: Path,
    *,
    save_fingerprint: str,
    playthrough: int,
    rarity: int,
    grace_effect_id: int | None,
) -> Path:
    kind = "draw1" if grace_effect_id is None else f"grace-{grace_effect_id:08X}-draw2"
    return (
        state_root
        / "primary-effect-maps"
        / f"{save_fingerprint.lower()}-p{playthrough}-r{rarity}-{kind}.json"
    )


def grace_map_cache_path(
    state_root: Path,
    *,
    save_fingerprint: str,
    playthrough: int,
    rarity: int,
) -> Path:
    return (
        state_root
        / "grace-output-maps"
        / f"{save_fingerprint.lower()}-p{playthrough}-r{rarity}-draw1.json"
    )


def format_special_rule_value(entry: SpecialRuleEntryResult) -> str:
    if entry.display_unit == "percent" and entry.display_value is not None:
        return f"{entry.display_value:+g}%"
    if entry.display_unit == "seconds" and entry.display_value is not None:
        return f"{entry.display_value:g} 秒"
    if entry.display_unit == "grade" and entry.display_grade:
        return entry.display_grade
    return ""


TITLE_SCREEN_ACK_TEXT = "我确认游戏当前位于标题界面"
TITLE_SCREEN_PROMPT_TEXT = (
    "写入存档前，请先让《仁王3》回到标题界面，避免游戏随后用内存中的旧状态覆盖修改。\n\n"
    "游戏不需要退出，也不需要断开网络。现在已经位于标题界面吗？"
)

QUICK_START_TEXT = """仁王3绘卷生成器 - 快速上手

搜索合法绘卷
1. 选择周目和稀有度。
2. 在“搜索所有词条”中输入名称并添加目标词条。默认第一项是主词条；可把主词条候选数设为 2 或 3（任一命中），也可勾选“主词条不限”。拖动或点击上下箭头可换序，点击 × 可删除。每项右侧可选任意、≥80、≥90或最高数值。
3. 按需选择恩宠、地形、敌人和特殊规则；不限制的项目保持“不限制”。稀有度4不限制恩宠时，结果可能保留恩宠，也可能最终没有恩宠。
4. 点击“计算候选 Seed”。符合条件的结果会边找到边显示。
5. 选择候选查看完整词条与数值。满意后先让游戏回到标题界面，勾选添加按钮左侧的确认框，再添加到存档。程序会自动备份。

已知 Seed
填写 Seed、周目和稀有度，再点击“生成并查看该 Seed”。该功能不应用上方筛选条件。

本地绘卷编辑
用于直接修改已有绘卷。修改只影响本机显示，通常不能传播给其他玩家。

提示
- “不限制恩宠”表示不把恩宠作为条件；稀有度4结果可能保留恩宠，也可能被完成器替换为普通词条。
- 特殊规则可以直接点击多选；展开后可选择具体数值变体。已选项会集中显示，可逐项 × 删除或一键清空。
- 不确定技术参数时保持默认值即可。
"""


FEATURE_GUIDE_TEXT = """按功能使用

一、搜索并添加可以传播的合法绘卷
1. 选择周目与稀有度。
2. 在目标组合中搜索词条，双击加入。默认第一项是主词条；主词条候选数可设为 2 或 3，表示任一命中。若只要求某个副词条而不限制主词条，勾选“主词条不限”。每项可独立设置最低抽取百分位。
3. 恩宠、地形、敌人和特殊规则都可以单独限制；特殊规则直接点击即可添加多条，不需要按 Ctrl。
4. 点击“计算候选 Seed”，结果会实时加入候选列表。
5. 比较词条、数值、敌人和规则。满意后让游戏回到标题界面，勾选添加按钮左侧的确认框并写入。

二、查看一个已知 Seed
填写 Seed、周目和稀有度，点击“生成并查看该 Seed”。这里不会应用目标组合中的筛选条件。

三、直接修改本地绘卷
切换到“本地绘卷编辑”，读取存档后选择绘卷与槽位。该功能适合只在本机使用的自定义词条；传播后接收方会按 Seed 重新生成。

四、备份与恢复
每次新增、修改、删除或恢复前，程序都会自动建立备份。可以在本地编辑页查看、恢复、删除备份，或打开备份文件夹。
"""


FAQ_TEXT = """常见问题

为什么添加按钮要求确认标题界面？
游戏在关卡或据点中可能把内存里的旧存档再次写回。添加前回到标题界面即可；不需要退出游戏，也不需要断开网络。未勾选时点击添加，程序会再次询问并可直接继续。

为什么界面只提供稀有度 3 和 4？
游戏开发方已预告将修复神宝绘卷传播问题，因此正式入口暂不再提供稀有度 5 的搜索、生成和写入。已经完成的研究代码仍保留用于回归，不作为用户功能。

如何选择或取消多个特殊规则？
直接点击规则即可逐条加入，不需要按 Ctrl。同一规则可以选择“任意变体”或一个精确数值变体；精确变体始终连同规则名称显示。已选规则下方可逐项点击 × 删除，也可以点击“清空已选规则”。未选择任何规则就表示不筛选。

目标组合是不是完整词条表？
目标组合显示当前周目与稀有度下逐项可生成的全部最终绘卷词条；它不是“任意组合都能共存”的承诺。恩宠在独立下拉框中选择。程序会先检查槽位、冲突组和类别容量，再用完整生成器确认 Seed。程序内还包含 3609 项原生名称目录，用于结果预览和本地编辑。

为什么有时仍显示十六进制编号？
当前版本已知的原生名称都会显示。只有游戏目录本身没有对应文本、或正在研究尚未确认的上下文时才保留十六进制编号，不会猜测名称。

本地直接修改的词条能传播吗？
通常不能。联机传播发送 Seed、稀有度等 canonical 字段，接收方会自行重建词条。要传播自定义结果，请使用“搜索合法绘卷”找到天然生成目标组合的 Seed。
"""


def user_facing_error_message(details: object) -> str:
    """Return the actionable final exception message without Python prefixes."""

    text = str(details).strip()
    final_line = text.splitlines()[-1] if text else "未知错误"
    for prefix in ("ValueError: ", "RuntimeError: ", "FileNotFoundError: "):
        if final_line.startswith(prefix):
            return final_line[len(prefix):]
    return final_line


TUTORIAL_TEXT = f"""仁王3绘卷生成器 Beta 使用教程

一、准备
1. 本工具仅支持《仁王3》PC v2.00.02。
2. 正式入口提供三周目稀有度3、4的 Seed 求解、单点预览和完整记录构造，均可离线完成，不需要启动游戏或读取存档。
3. 稀有度3、4各通过10,000个原生随机Seed的稳定完整记录对照。
4. 其他周目需要原生生成或准备写档时，让游戏返回标题界面，不需要关闭游戏或断开网络。
5. 启动生成器。程序会自动搜索存档；写档前需要选择账户，并勾选添加按钮左侧的标题界面确认框。

二、设置目标组合
1. 使用顶部的统一搜索框查找当前周目与稀有度下可生成的最终词条；双击或点击“添加选中词条”加入目标组合。
2. 默认第一项是主词条；“主词条候选数”可设为 1、2 或 3，前 N 项任一命中即可，其余是必需副词条。也可勾选“主词条不限”，让全部已选项只作为副词条条件。拖动或点击上下箭头可调整顺序，点击 × 可移除。稀有度3最多有3个普通副词条；稀有度4指定恩宠时最多3个，不限制恩宠时最多4个。
3. 恩宠使用独立下拉菜单筛选，不与普通词条混用。普通词条列表只表示逐项可达；软件会在计算前检查槽位、冲突组和类别容量，只有找到 Seed 后才确认整个组合合法。
   - 稀有度 4：第 5 槽先生成恩宠候选；完整 finalizer 可能保留该恩宠，也可能将它替换成普通词条。指定恩宠时只返回完成后仍保留该恩宠的最终记录。
   - 稀有度 3：第 5 槽固定为成长状态“未完成的杰作（画龙点睛）”，不作为普通副词条或可选恩宠。
4. 主、副词条都可以用中文名称、十六进制 ID 或小端字节搜索。每项右侧可设置“任意数值”、抽取百分位≥80、≥90或最高100；该条件参与精确 Seed 求解，不是只排序结果。
5. ID 0x0001 会显示为“未完成的杰作（画龙点睛）”；目前合法游戏状态中只确认稀有度 3 会出现该词条。它不作为普通副词条候选供直接拼接。
6. 旧版用同 Seed 的 R4 中间结果预测“画龙点睛恩宠”的路径已停用；R3 与 R4 现在分别运行各自经过原生对照的完整离线算法。
7. 稀有度和绘卷类型都会影响词条生成结果。周目选择会改用类型表中的 record type：一周目 0x1E82、二周目 0x516D、三周目 0xE604、四周目 0xDD82、五周目 0xD523。
8. 一、二周目没有三周目的恩宠槽；求解时必须至少选择一个主词条，程序直接求逆主词条 draw 1，再原生验证多个副词条。
9. 三周目稀有度4可指定恩宠，并必须经过完整 finalizer 重放确认恩宠没有被替换。
10. 等级、推荐等级和转手次数不直接参与副词条抽样；它们放在“绘卷属性”区域统一设置。
11. 地形影响可单选；特殊规则和出现敌人可多选为必含条件。特殊规则直接点击即可加入多条，不需要按 Ctrl；父项表示任意数值变体，展开后可精确选择 +50%、+65%、+80% 等原生变体。每个精确变体都会保留规则名称，已选项可逐项 × 删除或一键清空。
12. 地形、规则和敌人名称来自游戏当前版本的简中、日文、英文原生文本目录，不使用机器翻译；未知键会保留十六进制编号。
13. 四、五周目预计由 DLC2 开放，v2.00.02 当前无法正常进入。0xDD82/0xD523 仅证明游戏文件中存在潜在生成上下文，不证明未来 DLC 的最终算法；目前只允许研究预览，禁止写档和传播声明。

三、联立求解 Seed
1. 当前 Beta 只提供约束求解，不提供界面上的连续 Seed 扫描。一、二周目必须选择主词条；三周目至少选择一项词条、恩宠或辅助条件。
2. 指定恩宠时先数学求逆对应的完整 Seed 集合；稀有度4还会逐个重放 finalizer，只接受最终仍保留所选恩宠的记录。未指定恩宠的稀有度3、4从完整自然 Seed 数学族按批构造候选，并用 CUDA 批量预筛主词条。所有路径最终都离线重放权重池、冲突、晋升、重试、数值和规范化逻辑。
3. 主词条候选、多个指定副词条和每项数值门槛都在完整 Seed 重放结果上检查；不指定时可直接比较返回候选中的实际词条。
4. 地形、敌人和特殊规则同样由 Seed 离线生成并联合过滤；结构上不可能共存的敌人组合会在求解前直接报无解。
5. “候选数量”决定一次返回多少张可比较绘卷；“单批数学游标数”只是每个计算块的大小，不是总搜索上限。程序会自动继续后续块，直到得到所需数量、完整数学族耗尽或用户取消。
6. 支持 NVIDIA CUDA 时，数学候选构造会自动使用显卡；没有 CUDA 时回退到原生 CPU。无论使用哪种加速，候选最后都由完整离线生成器精确验证。
7. 点击“计算候选 Seed”后，每找到一张完整匹配就会立刻加入下方列表；需要更多结果时点击“计算下一批候选”，程序从精确数学游标继续，不会重复以前的候选。

四、已知 Seed 单点生成
1. 先选择绘卷类型/周目和稀有度，再在独立的“已知 Seed 单点生成”输入框填写 Seed，点击“生成并查看该 Seed”。五种绘卷类型分别是 0x1E82、0x516D、0xE604、0xDD82、0xD523。
2. 直接生成不会应用上方的筛选；三周目稀有度3、4均离线展示精确词条序列、抽取百分位和完整辅助结果，其他上下文使用游戏原生生成器。
3. 单点生成输入和上方联立求解互不影响，可用于核对任意已知 Seed。
4. 当前词条池内的 ID 显示中文名称；未知 ID 暂时显示十六进制编号。

五、查看计算结果
1. 每批返回多张匹配结果。三周目稀有度3、4离线预览显示全部最终词条的抽取百分位、原始数值、prefix、metadata 和 tail；完整记录生成均已通过各10,000个原生向量的稳定字节校验，可以在安装时安全物化。
2. 计算下一批时此前结果会保留，且不会抢走当前正在查看的候选。候选可按主词条数值、总抽取百分位或 Seed 排序；多选后可打开对比表。

六、添加到存档
1. 三周目稀有度3、4离线候选会在点击安装后才重新读取当前存档，绑定来源字段并分配新的内部序号；其他未通过完整记录门禁的预览仍禁止写入。
2. 每次添加前，程序都会自动备份主存档、游戏备份存档和系统存档。
3. 新绘卷只会写入最后一个已占用绘卷之后的下一个全零栏位，不会覆盖已有绘卷。
4. 写入后会修复校验和，并完成加密与精确回读验证。

七、本地绘卷编辑
1. 切换到“本地绘卷编辑”页，读取当前存档后可按物理栏位查看已有绘卷和全部七个词条槽。
2. 可以从完整官方词条目录选择 ID，也可以直接输入 raw ID、数值、prefix、metadata 和 tail。
3. 本地修改不会改变传播用的 canonical Seed/稀有度；接收方会重新生成，因此不会保留这些本地词条。
4. 删除绘卷会完整清零选中栏位，不移动或压缩其他栏位。编辑和删除前都会自动备份。

八、自动备份管理
1. 本地编辑页下方会列出本程序创建的备份，包括时间、账户、操作原因和主存档 SHA-256。
2. 恢复备份前，程序会先把当前主存档、游戏备份和系统存档保存成一个新的恢复点，再按备份中实际包含的文件恢复。
3. 删除自动备份使用 Windows 回收站，不会永久删除；也可以直接打开备份文件夹或当前账户的存档文件夹。

九、软件更新
1. 正式发布版会在启动后后台检查签名更新，也可以点击标题栏右侧的“检查更新”。
2. 更新清单使用 Ed25519 签名，下载文件必须同时通过签名清单中的大小和 SHA-256 校验。
3. 更新只会替换带有本程序受管理安装标记的当前 EXE；便携开发版和其他目录中的旧文件不会被自动删除。
4. 若正在计算或执行存档事务，已下载更新会等待该事务结束后才询问安装和重启。

备份位置：%LOCALAPPDATA%\\Nioh3ScrollGenerator\\backups

作者：{' & '.join(APP_AUTHORS)}
联系QQ群：{CONTACT_QQ_GROUP}
GitHub：{PROJECT_GITHUB_URL}
"""


def application_root() -> Path:
    frozen_root = getattr(sys, "_MEIPASS", None)
    if frozen_root:
        return Path(frozen_root)
    return Path(__file__).resolve().parents[1]


def application_title(*, research_mode: bool = RESEARCH_MODE) -> str:
    """Return the user-facing title without internal safety terminology."""

    suffix = "（研究模式）" if research_mode else ""
    return f"仁王3绘卷生成器 Beta{suffix}"


class ScrollEditorApp:
    def __init__(self, root: Tk) -> None:
        startup_trace("ScrollEditorApp initialization started")
        self.root = root
        self.root.title(application_title())
        screen_width = self.root.winfo_screenwidth()
        screen_height = self.root.winfo_screenheight()
        window_width = min(1680, max(800, screen_width - 40))
        window_height = min(1040, max(680, screen_height - 80))
        window_x = max(0, (screen_width - window_width) // 2)
        window_y = max(0, (screen_height - window_height) // 2)
        self.root.geometry(
            f"{window_width}x{window_height}+{window_x}+{window_y}"
        )
        self.root.minsize(
            min(1100, window_width),
            min(720, window_height),
        )
        self._configure_theme()
        self._configure_window_icon()
        startup_trace("theme and icon configured")

        self.events: queue.Queue[tuple[str, object]] = queue.Queue()
        self.worker: threading.Thread | None = None
        self.update_worker: threading.Thread | None = None
        self.pending_update: DownloadedUpdate | None = None
        self.update_setup_error: str | None = None
        self.cancel_event = threading.Event()
        self.candidates: list[ScrollCandidate] = []
        self.active_criteria: SearchCriteria | None = None
        self.last_search_seed: int | None = None
        self.last_joint_trial = 0
        self.active_streamed_count = 0
        self.primary_map_cache: dict[
            tuple[str, int, int, int], PrimaryOutputMap | PrimaryFirstDrawOutputMap
        ] = {}
        self.grace_map_cache: dict[tuple[str, int, int], GraceOutputMap] = {}
        self.auxiliary_names = load_auxiliary_name_catalog("zh-CN")
        startup_trace("auxiliary name catalog loaded")
        terrain_effect_keys = {
            TERRAIN_DISPLAY_CRUCIBLE_KEY,
            *TERRAIN_DISPLAY_SPECIAL_KEYS.values(),
        }
        self.terrain_effect_by_label = {
            f"{self.auxiliary_names.terrain_effect_name(key)} [0x{key:04X}]": key
            for key in terrain_effect_keys
        }
        self.terrain_filter = StringVar(value=NO_TERRAIN_FILTER_LABEL)
        self.rule_search = StringVar(value="")
        self.selected_rule_text = StringVar(value="要求包含：无（不筛选）")
        self.selected_rule_option_ids: set[str] = set()
        rule_groups = self.auxiliary_names.special_rule_key_groups()
        rule_tables = load_default_auxiliary_generation_tables()
        startup_trace("auxiliary generation tables loaded")
        self.rule_group_options: list[
            tuple[RuleFilterOption, tuple[RuleFilterOption, ...]]
        ] = []
        self.rule_option_by_token: dict[str, RuleFilterOption] = {}
        for name, keys in rule_groups.items():
            keys = frozenset(key for key in keys if key != 0)
            if not keys:
                continue
            group_token = f"any:{min(keys):04X}"
            group_option = RuleFilterOption(
                token=group_token,
                name=name,
                keys=keys,
                exact=False,
                label=f"{name}（任意变体）",
            )
            variants: list[RuleFilterOption] = []
            for key in sorted(keys):
                detail = describe_special_rule(key, tables=rule_tables)
                value_text = format_special_rule_value(detail) or "具体变体"
                variants.append(
                    RuleFilterOption(
                        token=f"exact:{key:04X}",
                        name=name,
                        keys=frozenset((key,)),
                        exact=True,
                        label=special_rule_variant_label(name, key, value_text),
                    )
                )
            variants.sort(key=lambda option: (option.label.casefold(), option.token))
            self.rule_group_options.append((group_option, tuple(variants)))
            self.rule_option_by_token[group_token] = group_option
            self.rule_option_by_token.update(
                (option.token, option) for option in variants
            )
        self.rule_group_options.sort(
            key=lambda item: (item[0].name.casefold(), item[0].token)
        )
        self.rule_family_by_token: dict[str, frozenset[str]] = {}
        for group, variants in self.rule_group_options:
            family = frozenset((group.token, *(option.token for option in variants)))
            for token in family:
                self.rule_family_by_token[token] = family
        self.visible_rule_tokens: set[str] = set()
        self.enemy_search = StringVar(value="")
        self.selected_enemy_text = StringVar(value="要求包含：无（不筛选）")
        self.selected_enemy_keys: set[int] = set()
        enemy_groups = self.auxiliary_names.enemy_key_groups()
        self.enemy_key_groups = {min(keys): keys for keys in enemy_groups.values()}
        self.enemy_options = [
            (
                min(keys),
                f"{name} "
                + (
                    f"[0x{min(keys):08X}]"
                    if len(keys) == 1
                    else f"[{len(keys)} 个原生变体]"
                ),
            )
            for name, keys in enemy_groups.items()
        ]
        self.enemy_options.sort(key=lambda item: (item[1].casefold(), item[0]))
        self.enemy_visible = list(self.enemy_options)

        self.save_choices: dict[str, Path] = {}
        self.save_account = StringVar(value="正在自动搜索存档……")
        self.title_ack = BooleanVar(value=False)
        self.rarity = StringVar(value="4")
        self.playthrough = StringVar(value=PLAYTHROUGH_LABELS[2])
        self.effect_search = StringVar(value="")
        self.primary_unconstrained = BooleanVar(value=False)
        self.primary_candidate_count = StringVar(value="1")
        self.selected_effect_ids: list[int] = []
        self.selected_effect_min_rolls: dict[int, int] = {}
        self.selected_primary_ids: set[int] = set()
        self.selected_secondary_ids: set[int] = set()
        self.search_effect_catalog = searchable_scroll_effect_definitions(3, 5)
        self.search_effect_by_id = {
            effect.effect_id: effect for effect in self.search_effect_catalog
        }
        self.effect_visible = list(self.search_effect_catalog)
        self.effect_catalog_summary = StringVar(
            value=(
                f"当前上下文逐项可生成词条：{len(self.search_effect_catalog)} 项；"
                "组合是否合法将在下方预检"
            )
        )
        self.combination_status = StringVar(value="组合状态：尚未选择词条条件。")
        self.effect_result_summary = StringVar(
            value=f"显示 {len(self.effect_visible)} / {len(self.search_effect_catalog)} 项"
        )
        self._dragged_effect_id: int | None = None
        self.grace_filter = StringVar(value=NO_GRACE_FILTER_LABEL)
        self.grace_search_hint = StringVar()
        self.calculation_mode_hint = StringVar()
        self.intersection_summary = StringVar(
            value="交集数量：选择条件后点击“计算候选 Seed”。"
        )
        self.special_id_by_label: dict[str, int] = {}
        self.direct_seed = StringVar(value="0")
        self.max_seeds = StringVar(value="1000000")
        self.result_count = StringVar(value="20")
        self.candidate_sort = StringVar(value="发现顺序")
        self.level = StringVar(value="180")
        self.recommended = StringVar(value="183")
        self.transfer_count = StringVar(value="0")
        self.status = StringVar(value="就绪")
        self.game_compatibility = detect_game_compatibility()
        self.game_compatibility_text = StringVar(
            value=self.game_compatibility.detail
        )
        self.local_inventory: SaveInventory | None = None
        self.local_entries: list[ScrollInventoryEntry] = []
        self.local_entry_by_iid: dict[str, ScrollInventoryEntry] = {}
        self.backup_entries: list[BackupEntry] = []
        self.backup_entry_by_iid: dict[str, BackupEntry] = {}
        self.local_effect_catalog = native_effect_definitions()
        self.local_effect_label_to_id = {
            effect.label: effect.effect_id for effect in self.local_effect_catalog
        }
        self.local_effect_search = StringVar(value="")
        self.local_effect_choice = StringVar(value="")
        self.local_effect_id = StringVar(value="")
        self.local_effect_value = StringVar(value="")
        self.local_effect_prefix = StringVar(value="")
        self.local_effect_metadata = StringVar(value="")
        self.local_effect_tail_0 = StringVar(value="")
        self.local_effect_tail_1 = StringVar(value="")

        self.grace_filter.trace_add("write", self._update_grace_search_hint)
        self.grace_filter.trace_add("write", self._update_calculation_controls)
        self.grace_filter.trace_add("write", self._mark_intersection_stale)
        self.grace_filter.trace_add("write", self._refresh_combination_status)
        self.primary_unconstrained.trace_add("write", self._on_primary_mode_changed)
        self.primary_candidate_count.trace_add("write", self._on_primary_mode_changed)
        self.terrain_filter.trace_add("write", self._mark_intersection_stale)
        self.rarity.trace_add("write", self._on_rarity_changed)
        self.playthrough.trace_add("write", self._on_playthrough_changed)
        self.effect_search.trace_add("write", self._filter_effect_catalog)
        self.local_effect_search.trace_add("write", self._filter_local_effect_catalog)
        self._build_ui()
        startup_trace("user interface built")
        self._update_grace_search_hint()
        self._update_calculation_controls()
        self._refresh_saves()
        startup_trace("save discovery completed")
        self.root.after(100, self._poll_events)
        if UPDATE_MANIFEST_URL and UPDATE_PUBLIC_KEY_BASE64:
            if getattr(sys, "frozen", False):
                try:
                    ensure_managed_install(Path(sys.executable))
                except Exception as error:
                    self.update_setup_error = str(error)
            self.root.after(1500, lambda: self._check_for_updates(manual=False))

    def _configure_theme(self) -> None:
        self.colors = {
            "canvas": "#0B0D0F",
            "surface": "#121518",
            "raised": "#181C20",
            "selected": "#20375E",
            "border": "#343A40",
            "text": "#EEE7D6",
            "muted": "#A7ABB0",
            "gold": "#D6A64B",
            "blue": "#4F73B8",
            "green": "#48A86B",
            "danger": "#C95252",
        }
        self.root.configure(background=self.colors["canvas"])
        self.root.option_add("*Font", ("Microsoft YaHei UI", 10))
        self.root.option_add("*Listbox.background", self.colors["surface"])
        self.root.option_add("*Listbox.foreground", self.colors["text"])
        self.root.option_add("*Listbox.selectBackground", self.colors["selected"])
        self.root.option_add("*Listbox.selectForeground", self.colors["text"])
        self.root.option_add("*Listbox.highlightBackground", self.colors["border"])
        self.root.option_add("*Listbox.highlightColor", self.colors["blue"])
        self.root.option_add("*Text.background", self.colors["surface"])
        self.root.option_add("*Text.foreground", self.colors["text"])
        self.root.option_add("*Text.insertBackground", self.colors["text"])

        self.style = ttk.Style(self.root)
        self.style.theme_use("clam")
        self.style.configure(".", background=self.colors["canvas"], foreground=self.colors["text"])
        self.style.configure("TFrame", background=self.colors["surface"])
        self.style.configure("Surface.TFrame", background=self.colors["surface"])
        self.style.configure("Rail.TFrame", background="#0E1114")
        self.style.configure("Header.TFrame", background="#0E1012")
        self.style.configure("TLabel", background=self.colors["surface"], foreground=self.colors["text"])
        self.style.configure("Surface.TLabel", background=self.colors["surface"], foreground=self.colors["text"])
        self.style.configure(
            "SelectedEffect.TFrame",
            background=self.colors["raised"],
            bordercolor=self.colors["border"],
            relief="solid",
        )
        self.style.configure(
            "SelectedEffect.TLabel",
            background=self.colors["raised"],
            foreground=self.colors["text"],
        )
        self.style.configure(
            "SelectedEffectRole.TLabel",
            background=self.colors["selected"],
            foreground="#FFFFFF",
            font=("Microsoft YaHei UI", 9, "bold"),
        )
        self.style.configure(
            "Title.TLabel",
            background="#0E1012",
            foreground=self.colors["text"],
            font=("Microsoft YaHei UI", 17, "bold"),
        )
        self.style.configure(
            "Ready.TLabel",
            background="#0E1012",
            foreground=self.colors["green"],
            font=("Microsoft YaHei UI", 10),
        )
        self.style.configure(
            "Footer.TLabel",
            background="#0E1012",
            foreground=self.colors["muted"],
            font=("Microsoft YaHei UI", 9),
        )
        self.style.configure(
            "TLabelframe",
            background=self.colors["surface"],
            foreground=self.colors["text"],
            bordercolor=self.colors["border"],
            relief="solid",
        )
        self.style.configure(
            "TLabelframe.Label",
            background=self.colors["surface"],
            foreground=self.colors["text"],
            font=("Microsoft YaHei UI", 10, "bold"),
        )
        self.style.configure(
            "TButton",
            background=self.colors["raised"],
            foreground=self.colors["text"],
            bordercolor=self.colors["border"],
            focusthickness=1,
            focuscolor=self.colors["blue"],
            padding=(10, 6),
        )
        self.style.map(
            "TButton",
            background=[("active", "#22282E"), ("disabled", "#15181B")],
            foreground=[("disabled", "#666B70")],
            bordercolor=[("focus", self.colors["blue"])],
        )
        self.style.configure(
            "Accent.TButton",
            background="#274B83",
            foreground="#FFFFFF",
            bordercolor=self.colors["blue"],
            padding=(12, 7),
        )
        self.style.map(
            "Accent.TButton",
            background=[("active", "#315D9F"), ("disabled", "#1A2533")],
        )
        self.style.configure(
            "Danger.TButton",
            background=self.colors["surface"],
            foreground="#FF8585",
            bordercolor=self.colors["danger"],
        )
        self.style.map("Danger.TButton", background=[("active", "#3A1D20")])
        self.style.configure(
            "Rail.TButton",
            background="#0E1114",
            foreground=self.colors["muted"],
            borderwidth=0,
            anchor="w",
            padding=(16, 13),
            font=("Microsoft YaHei UI", 10),
        )
        self.style.map("Rail.TButton", background=[("active", "#171C22")])
        self.style.configure(
            "RailActive.TButton",
            background=self.colors["selected"],
            foreground="#FFFFFF",
            bordercolor=self.colors["gold"],
            borderwidth=1,
            anchor="w",
            padding=(16, 13),
            font=("Microsoft YaHei UI", 10, "bold"),
        )
        self.style.map("RailActive.TButton", background=[("active", "#294673")])
        self.style.configure(
            "TEntry",
            fieldbackground=self.colors["raised"],
            foreground=self.colors["text"],
            bordercolor=self.colors["border"],
            insertcolor=self.colors["text"],
            padding=5,
        )
        self.style.configure(
            "TCombobox",
            fieldbackground=self.colors["raised"],
            background=self.colors["raised"],
            foreground=self.colors["text"],
            arrowcolor=self.colors["muted"],
            bordercolor=self.colors["border"],
            padding=4,
        )
        self.style.map(
            "TCombobox",
            fieldbackground=[("readonly", self.colors["raised"])],
            foreground=[("readonly", self.colors["text"])],
            selectbackground=[("readonly", self.colors["raised"])],
            selectforeground=[("readonly", self.colors["text"])],
        )
        self.style.configure(
            "Treeview",
            background=self.colors["surface"],
            fieldbackground=self.colors["surface"],
            foreground=self.colors["text"],
            bordercolor=self.colors["border"],
            rowheight=27,
        )
        self.style.map(
            "Treeview",
            background=[("selected", self.colors["selected"])],
            foreground=[("selected", "#FFFFFF")],
        )
        self.style.configure(
            "Treeview.Heading",
            background="#171B1F",
            foreground=self.colors["muted"],
            bordercolor=self.colors["border"],
            padding=(7, 6),
            font=("Microsoft YaHei UI", 9, "bold"),
        )
        self.style.map("Treeview.Heading", background=[("active", "#20262C")])
        self.style.configure(
            "TCheckbutton",
            background=self.colors["surface"],
            foreground=self.colors["text"],
        )
        self.style.map("TCheckbutton", background=[("active", self.colors["surface"])])
        self.style.configure("TSeparator", background=self.colors["border"])
        self.style.configure(
            "TScrollbar",
            background="#252A2F",
            troughcolor=self.colors["canvas"],
            bordercolor=self.colors["canvas"],
            arrowcolor=self.colors["muted"],
        )
        self.style.layout("Main.TNotebook.Tab", [])
        self.style.configure("Main.TNotebook", background=self.colors["canvas"], borderwidth=0)

    def _configure_window_icon(self) -> None:
        """Apply the bundled icon and native dark Windows title bar."""

        assets = application_root() / "assets"
        png_path = assets / "nioh3-scroll-generator-icon.png"
        ico_path = assets / "nioh3-scroll-generator.ico"
        self._window_icon_photo = None
        self._window_icon_handles: tuple[int, ...] = ()
        try:
            if png_path.is_file():
                self._window_icon_photo = PhotoImage(file=str(png_path))
                self.root.iconphoto(True, self._window_icon_photo)
        except Exception:
            pass
        try:
            if ico_path.is_file():
                self.root.iconbitmap(str(ico_path))
        except Exception:
            pass
        if os.name == "nt":
            self.root.after_idle(lambda: self._configure_windows_chrome(ico_path))

    def _configure_windows_chrome(self, ico_path: Path) -> None:
        """Set native HWND icons and dark caption while retaining system chrome."""

        try:
            user32 = ctypes.windll.user32
            root_hwnd = int(self.root.winfo_id())
            get_parent = user32.GetParent
            get_parent.restype = ctypes.c_void_p
            get_parent.argtypes = (ctypes.c_void_p,)
            parent_hwnd = int(get_parent(ctypes.c_void_p(root_hwnd)) or 0)
            hwnds = tuple(dict.fromkeys(hwnd for hwnd in (root_hwnd, parent_hwnd) if hwnd))

            enabled = ctypes.c_int(1)
            for hwnd in hwnds:
                try:
                    ctypes.windll.dwmapi.DwmSetWindowAttribute(
                        ctypes.c_void_p(hwnd),
                        20,  # DWMWA_USE_IMMERSIVE_DARK_MODE
                        ctypes.byref(enabled),
                        ctypes.sizeof(enabled),
                    )
                except Exception:
                    pass

            if not ico_path.is_file():
                return
            load_image = user32.LoadImageW
            load_image.restype = ctypes.c_void_p
            load_image.argtypes = (
                ctypes.c_void_p,
                ctypes.c_wchar_p,
                ctypes.c_uint,
                ctypes.c_int,
                ctypes.c_int,
                ctypes.c_uint,
            )
            icon_handles = tuple(
                int(handle)
                for handle in (
                    load_image(None, str(ico_path), 1, 32, 32, 0x10),
                    load_image(None, str(ico_path), 1, 16, 16, 0x10),
                )
                if handle
            )
            if not icon_handles:
                return
            large_icon = icon_handles[0]
            small_icon = icon_handles[-1]
            send_message = user32.SendMessageW
            send_message.restype = ctypes.c_ssize_t
            send_message.argtypes = (
                ctypes.c_void_p,
                ctypes.c_uint,
                ctypes.c_size_t,
                ctypes.c_ssize_t,
            )
            for hwnd in hwnds:
                send_message(
                    ctypes.c_void_p(hwnd), 0x0080, 1, large_icon
                )  # WM_SETICON/ICON_BIG
                send_message(
                    ctypes.c_void_p(hwnd), 0x0080, 0, small_icon
                )  # WM_SETICON/ICON_SMALL
            self._window_icon_handles = icon_handles
        except Exception:
            # Window decoration must never block save recovery or editor startup.
            return

    def _build_ui(self) -> None:
        shell = ttk.Frame(self.root)
        shell.pack(fill=BOTH, expand=True)

        header = ttk.Frame(shell, style="Header.TFrame", height=54)
        header.pack(fill="x")
        header.pack_propagate(False)
        ttk.Label(header, text="仁王3绘卷生成器", style="Title.TLabel").pack(
            side=LEFT, padx=(18, 24)
        )
        ttk.Label(
            header,
            text=(
                "●  CUDA 并行预筛 · CPU 精确重放"
                if cuda_seed_acceleration_available()
                else "●  原生 CPU 预筛 · 精确重放"
            ),
            style="Ready.TLabel",
        ).pack(side=LEFT)
        self.update_button = ttk.Button(
            header,
            text=f"v{APP_VERSION} · 检查更新",
            command=self._check_for_updates,
        )
        self.update_button.pack(side=RIGHT, padx=14, pady=9)

        body = ttk.Frame(shell)
        body.pack(fill=BOTH, expand=True)
        rail = ttk.Frame(body, style="Rail.TFrame", width=188)
        rail.pack(side=LEFT, fill="y")
        rail.pack_propagate(False)
        ttk.Label(
            rail,
            text="工作区",
            foreground=self.colors["muted"],
            background="#0E1114",
            font=("Microsoft YaHei UI", 9),
        ).pack(anchor="w", padx=18, pady=(20, 8))
        self.search_nav_button = ttk.Button(
            rail,
            text="⌕  搜索合法绘卷",
            style="RailActive.TButton",
            command=lambda: self._select_main_tab(0),
        )
        self.search_nav_button.pack(fill="x", padx=8)
        self.local_nav_button = ttk.Button(
            rail,
            text="✎  本地绘卷编辑",
            style="Rail.TButton",
            command=lambda: self._select_main_tab(1),
        )
        self.local_nav_button.pack(fill="x", padx=8, pady=(4, 0))

        self.notebook = ttk.Notebook(body, style="Main.TNotebook")
        self.notebook.pack(side=LEFT, fill=BOTH, expand=True)
        self.search_tab = ttk.Frame(self.notebook)
        self.local_editor_tab = ttk.Frame(self.notebook)
        self.notebook.add(self.search_tab, text="搜索合法绘卷")
        self.notebook.add(self.local_editor_tab, text="本地绘卷编辑")
        self.notebook.bind("<<NotebookTabChanged>>", self._sync_main_navigation)

        outer = ttk.Frame(self.search_tab, padding=12)
        outer.pack(fill=BOTH, expand=True)

        warning = ttk.LabelFrame(outer, text="使用说明", padding=10)
        warning.pack(fill="x")
        ttk.Label(
            warning,
            text=(
                "三周目稀有度3、4的 Seed 求解与预览可完全离线运行，不需要启动游戏或读取存档。"
                "其他周目、完整记录生成和写档仍只支持《仁王3》v2.00.02；写档前请让游戏回到标题界面。"
            ),
            foreground="#E0A15E",
            wraplength=1000,
        ).pack(anchor="w")
        ttk.Label(
            warning,
            textvariable=self.game_compatibility_text,
            foreground=(
                "#65C985"
                if self.game_compatibility.supported
                else (
                    "#F06D6D"
                    if self.game_compatibility.known_mismatch
                    else "#E0A15E"
                )
            ),
            wraplength=1000,
        ).pack(anchor="w", pady=(4, 0))
        save_row = ttk.Frame(outer, padding=(0, 10, 0, 6))
        save_row.pack(fill="x")
        ttk.Label(save_row, text="存档账户：").pack(side=LEFT)
        self.save_combo = ttk.Combobox(
            save_row,
            textvariable=self.save_account,
            state="readonly",
            width=38,
        )
        self.save_combo.pack(side=LEFT, padx=8)
        ttk.Button(save_row, text="使用教程", command=self._show_tutorial).pack(side=RIGHT)
        ttk.Button(save_row, text="重新搜索存档", command=self._refresh_saves).pack(side=RIGHT, padx=8)

        workspace = ttk.Panedwindow(outer, orient="horizontal")
        workspace.pack(fill=BOTH, expand=True)
        filter_host = ttk.Frame(workspace, width=500)
        results_column = ttk.Frame(workspace)
        workspace.add(filter_host, weight=0)
        workspace.add(results_column, weight=1)
        self.root.after_idle(lambda: workspace.sashpos(0, 500))

        filter_canvas = Canvas(
            filter_host,
            background=self.colors["surface"],
            highlightthickness=0,
            width=488,
        )
        filter_scrollbar = ttk.Scrollbar(
            filter_host,
            orient="vertical",
            command=filter_canvas.yview,
        )
        filter_canvas.configure(yscrollcommand=filter_scrollbar.set)
        filter_canvas.pack(side=LEFT, fill=BOTH, expand=True)
        filter_scrollbar.pack(side=RIGHT, fill="y")
        filter_column = ttk.Frame(filter_canvas, padding=(0, 0, 8, 0))
        filter_window = filter_canvas.create_window(
            (0, 0),
            window=filter_column,
            anchor="nw",
        )
        filter_column.bind(
            "<Configure>",
            lambda _event: filter_canvas.configure(
                scrollregion=filter_canvas.bbox("all")
            ),
        )
        filter_canvas.bind(
            "<Configure>",
            lambda event: filter_canvas.itemconfigure(filter_window, width=event.width),
        )

        selector = ttk.LabelFrame(filter_column, text="目标组合", padding=10)
        selector.pack(fill="x")
        ttk.Label(
            selector,
            textvariable=self.effect_catalog_summary,
        ).pack(anchor="w")
        effect_search_row = ttk.Frame(selector)
        effect_search_row.pack(fill="x", pady=(5, 3))
        ttk.Entry(effect_search_row, textvariable=self.effect_search).pack(
            side=LEFT, fill="x", expand=True
        )
        self.add_effect_button = ttk.Button(
            effect_search_row,
            text="添加选中词条",
            command=self._add_selected_effect_result,
        )
        self.add_effect_button.pack(side=RIGHT, padx=(6, 0))

        effect_result_frame = ttk.Frame(selector)
        effect_result_frame.pack(fill="x")
        self.effect_result_list = Listbox(
            effect_result_frame,
            selectmode="browse",
            exportselection=False,
            height=9,
        )
        effect_result_scrollbar = ttk.Scrollbar(
            effect_result_frame,
            orient="vertical",
            command=self.effect_result_list.yview,
        )
        self.effect_result_list.configure(yscrollcommand=effect_result_scrollbar.set)
        self.effect_result_list.pack(side=LEFT, fill="x", expand=True)
        effect_result_scrollbar.pack(side=RIGHT, fill="y")
        self.effect_result_list.bind(
            "<Double-Button-1>",
            lambda _event: self._add_selected_effect_result(),
        )
        self._populate_effect_result_list()
        ttk.Label(
            selector,
            textvariable=self.effect_result_summary,
            foreground=self.colors["muted"],
        ).pack(anchor="w", pady=(3, 0))

        ttk.Separator(selector).pack(fill="x", pady=9)
        ttk.Label(
            selector,
            text="已选词条（默认第一项是主词条，其余是必需副词条）",
        ).pack(anchor="w")
        ttk.Checkbutton(
            selector,
            text="主词条不限：下方所有已选词条都必须作为副词条出现",
            variable=self.primary_unconstrained,
        ).pack(anchor="w", pady=(4, 0))
        primary_mode_row = ttk.Frame(selector)
        primary_mode_row.pack(fill="x", pady=(4, 0))
        ttk.Label(primary_mode_row, text="主词条候选数").pack(side=LEFT)
        ttk.Combobox(
            primary_mode_row,
            textvariable=self.primary_candidate_count,
            values=("1", "2", "3"),
            state="readonly",
            width=5,
        ).pack(side=LEFT, padx=(6, 8))
        ttk.Label(
            primary_mode_row,
            text="前 N 项任一命中即可；拖动或用箭头调整角色",
            foreground=self.colors["muted"],
        ).pack(side=LEFT)
        self.selected_effects_frame = ttk.Frame(selector)
        self.selected_effects_frame.pack(fill="x", pady=(5, 0))
        self._render_selected_effects()
        ttk.Label(
            selector,
            textvariable=self.combination_status,
            foreground="#65C985",
            wraplength=450,
        ).pack(anchor="w", pady=(5, 0))

        rarity_frame = ttk.LabelFrame(selector, text="影响词条生成", padding=8)
        rarity_frame.pack(fill="x", pady=(10, 0))
        rarity_row = ttk.Frame(rarity_frame)
        rarity_row.pack(fill="x", pady=1)
        ttk.Label(rarity_row, text="稀有度", width=12).pack(side=LEFT)
        ttk.Combobox(
            rarity_row,
            textvariable=self.rarity,
            values=tuple(str(value) for value in PRODUCT_RARITIES),
            state="readonly",
            width=12,
        ).pack(side=RIGHT)
        playthrough_row = ttk.Frame(rarity_frame)
        playthrough_row.pack(fill="x", pady=1)
        ttk.Label(playthrough_row, text="绘卷类型/周目", width=12).pack(side=LEFT)
        ttk.Combobox(
            playthrough_row,
            textvariable=self.playthrough,
            values=PLAYTHROUGH_LABELS,
            state="readonly",
            width=14,
        ).pack(side=RIGHT)
        grace_row = ttk.Frame(rarity_frame)
        grace_row.pack(fill="x", pady=1)
        ttk.Label(grace_row, text="恩宠筛选", width=12).pack(side=LEFT)
        self.grace_combo = ttk.Combobox(
            grace_row,
            textvariable=self.grace_filter,
            values=(NO_GRACE_FILTER_LABEL,),
            state="readonly",
            width=30,
        )
        self.grace_combo.pack(side=RIGHT)
        self._refresh_special_filter_values()
        ttk.Label(
            rarity_frame,
            textvariable=self.grace_search_hint,
            foreground="#65C985",
            wraplength=290,
        ).pack(anchor="w", pady=(4, 0))

        properties = ttk.LabelFrame(
            filter_column, text="绘卷属性（不影响词条组合）", padding=8
        )
        properties.pack(fill="x", pady=(8, 0))
        self._entry_row(properties, "等级", self.level, 14)
        self._entry_row(properties, "推荐等级", self.recommended, 14)
        self._entry_row(properties, "转手次数", self.transfer_count, 14)

        auxiliary = ttk.LabelFrame(
            filter_column,
            text="敌人 / 特殊规则 / 地形影响（参与 Seed 联立筛选）",
            padding=8,
        )
        auxiliary.pack(fill="x", pady=(8, 0))

        terrain_frame = ttk.Frame(auxiliary)
        terrain_frame.pack(fill="x")
        ttk.Label(terrain_frame, text="地形影响").pack(anchor="w")
        terrain_values = (
            NO_TERRAIN_FILTER_LABEL,
            *sorted(self.terrain_effect_by_label, key=str.casefold),
        )
        ttk.Combobox(
            terrain_frame,
            textvariable=self.terrain_filter,
            values=terrain_values,
            state="readonly",
            width=30,
        ).pack(anchor="w", pady=(4, 0))

        rule_frame = ttk.Frame(auxiliary)
        rule_frame.pack(fill="x", pady=(10, 0))
        rule_header = ttk.Frame(rule_frame)
        rule_header.pack(fill="x")
        ttk.Label(rule_header, text="特殊规则必含（点击即可多选）").pack(side=LEFT)
        ttk.Button(
            rule_header,
            text="清空已选规则",
            command=self._clear_rule_selection,
        ).pack(side=RIGHT)
        ttk.Entry(rule_frame, textvariable=self.rule_search).pack(fill="x", pady=(4, 2))
        rule_list_frame = ttk.Frame(rule_frame)
        rule_list_frame.pack(fill="x")
        self.rule_tree = ttk.Treeview(
            rule_list_frame,
            show="tree",
            selectmode="extended",
            height=5,
        )
        rule_scrollbar = ttk.Scrollbar(
            rule_list_frame, orient="vertical", command=self.rule_tree.yview
        )
        self.rule_tree.configure(yscrollcommand=rule_scrollbar.set)
        self.rule_tree.pack(side=LEFT, fill="x", expand=True)
        rule_scrollbar.pack(side=RIGHT, fill="y")
        self.rule_tree.bind("<Button-1>", self._on_rule_click)
        self.rule_tree.bind("<space>", self._toggle_focused_rule)
        self.rule_tree.bind("<Return>", self._toggle_focused_rule)
        self.rule_search.trace_add("write", self._filter_rules)
        self._populate_rule_tree()
        ttk.Label(rule_frame, textvariable=self.selected_rule_text).pack(anchor="w")
        self.selected_rule_chips = ttk.Frame(rule_frame)
        self.selected_rule_chips.pack(fill="x", pady=(3, 0))
        self._update_rule_summary()

        enemy_frame = ttk.Frame(auxiliary)
        enemy_frame.pack(fill="x", pady=(10, 0))
        ttk.Label(enemy_frame, text="出现敌人必含（可多选）").pack(anchor="w")
        ttk.Entry(enemy_frame, textvariable=self.enemy_search).pack(fill="x", pady=(4, 2))
        enemy_list_frame = ttk.Frame(enemy_frame)
        enemy_list_frame.pack(fill="x")
        self.enemy_list = Listbox(
            enemy_list_frame,
            selectmode="multiple",
            exportselection=False,
            height=5,
        )
        for _key, label in self.enemy_visible:
            self.enemy_list.insert(END, label)
        enemy_scrollbar = ttk.Scrollbar(
            enemy_list_frame, orient="vertical", command=self.enemy_list.yview
        )
        self.enemy_list.configure(yscrollcommand=enemy_scrollbar.set)
        self.enemy_list.pack(side=LEFT, fill="x", expand=True)
        enemy_scrollbar.pack(side=RIGHT, fill="y")
        self.enemy_list.bind("<<ListboxSelect>>", self._on_enemy_select)
        self.enemy_search.trace_add("write", self._filter_enemies)
        ttk.Label(enemy_frame, textvariable=self.selected_enemy_text).pack(anchor="w")

        ttk.Label(
            filter_column,
            text=(
                "四、五周目尚未由 DLC2 开放：0xDD82/0xD523 只作为潜在上下文进行"
                "只读研究预览，不开放写档，也不代表未来 DLC 的最终生成结果。"
            ),
            foreground="#E0A15E",
            wraplength=1050,
        ).pack(anchor="w", pady=(4, 0))

        calculation = ttk.LabelFrame(
            results_column,
            text="Seed 计算与结果验证",
            padding=8,
        )
        calculation.pack(fill="x", pady=(8, 6))
        ttk.Label(
            calculation,
            textvariable=self.calculation_mode_hint,
            foreground="#83A7E8",
        ).pack(anchor="w", pady=(0, 6))
        ttk.Label(
            calculation,
            textvariable=self.intersection_summary,
            foreground="#65C985",
            wraplength=1100,
            justify=LEFT,
        ).pack(anchor="w", pady=(0, 6))
        ttk.Label(
            calculation,
            text="离线模式自动续批，直到结果够数、完整穷尽或取消。",
            foreground=self.colors["muted"],
        ).pack(anchor="w", pady=(0, 6))
        search_controls = ttk.Frame(calculation)
        search_controls.pack(fill="x")
        ttk.Label(search_controls, text="候选数量").pack(side=LEFT, padx=(0, 4))
        ttk.Entry(
            search_controls, textvariable=self.result_count, width=7
        ).pack(side=LEFT, padx=(0, 12))
        self.max_seeds_label = ttk.Label(search_controls, text="单批数学游标数")
        self.max_seeds_label.pack(side=LEFT, padx=(0, 4))
        self.max_seeds_entry = ttk.Entry(
            search_controls, textvariable=self.max_seeds, width=14
        )
        self.max_seeds_entry.pack(side=LEFT, padx=(0, 10))
        search_actions = ttk.Frame(calculation)
        search_actions.pack(fill="x", pady=(6, 0))
        self.find_button = ttk.Button(
            search_actions,
            text="计算候选 Seed",
            command=self._find_first,
            style="Accent.TButton",
        )
        self.find_button.pack(side=LEFT, padx=(0, 4))
        self.next_button = ttk.Button(
            search_actions, text="计算下一批候选", command=self._find_next, state="disabled"
        )
        self.next_button.pack(side=LEFT, padx=4)
        self.cancel_button = ttk.Button(
            search_actions, text="取消", command=self._cancel, state="disabled"
        )
        self.cancel_button.pack(side=LEFT, padx=4)
        direct_controls = ttk.Frame(calculation)
        direct_controls.pack(fill="x", pady=(8, 0))
        ttk.Label(direct_controls, text="已知 Seed 单点生成").pack(
            side=LEFT, padx=(0, 4)
        )
        ttk.Entry(direct_controls, textvariable=self.direct_seed, width=18).pack(
            side=LEFT, padx=(0, 8)
        )
        self.generate_button = ttk.Button(
            direct_controls, text="生成并查看该 Seed", command=self._generate_seed
        )
        self.generate_button.pack(side=LEFT)
        ttk.Label(
            direct_controls,
            text="此输入只用于单点生成，不参与上方联立求解。",
            foreground=self.colors["muted"],
        ).pack(side=LEFT, padx=10)

        candidate_frame = ttk.LabelFrame(results_column, text="计算结果", padding=10)
        candidate_frame.pack(fill=BOTH, expand=True)
        panes = ttk.Panedwindow(candidate_frame, orient="horizontal")
        panes.pack(fill=BOTH, expand=True)

        left = ttk.Frame(panes)
        right = ttk.Frame(panes)
        panes.add(left, weight=2)
        panes.add(right, weight=3)
        candidate_sort_row = ttk.Frame(left)
        candidate_sort_row.pack(fill="x", pady=(0, 6))
        ttk.Label(candidate_sort_row, text="排序").pack(side=LEFT)
        candidate_sort_combo = ttk.Combobox(
            candidate_sort_row,
            textvariable=self.candidate_sort,
            values=(
                "发现顺序",
                "主词条数值从高到低",
                "全部词条抽取百分位从高到低",
                "Seed 从小到大",
            ),
            state="readonly",
            width=28,
        )
        candidate_sort_combo.pack(side=LEFT, padx=(6, 0))
        candidate_sort_combo.bind(
            "<<ComboboxSelected>>",
            lambda _event: self._resort_candidates(),
        )
        candidate_list_frame = ttk.Frame(left)
        candidate_list_frame.pack(fill=BOTH, expand=True)
        self.candidate_list = Listbox(
            candidate_list_frame,
            exportselection=False,
            selectmode="extended",
            width=40,
        )
        candidate_xscrollbar = ttk.Scrollbar(
            candidate_list_frame,
            orient="horizontal",
            command=self.candidate_list.xview,
        )
        self.candidate_list.configure(xscrollcommand=candidate_xscrollbar.set)
        candidate_xscrollbar.pack(side="bottom", fill="x")
        self.candidate_list.pack(fill=BOTH, expand=True)
        self.candidate_list.bind("<<ListboxSelect>>", self._show_selected_candidate)
        candidate_actions = ttk.Frame(left)
        candidate_actions.pack(fill="x", pady=(6, 0))
        ttk.Button(
            candidate_actions,
            text="删除选中预览",
            command=self._delete_selected_candidates,
        ).pack(side=LEFT)
        ttk.Button(
            candidate_actions,
            text="比较选中",
            command=self._compare_selected_candidates,
        ).pack(side=LEFT, padx=(6, 0))
        ttk.Button(
            candidate_actions,
            text="清空预览",
            command=self._clear_candidates,
        ).pack(side=LEFT, padx=(6, 0))

        columns = ("slot", "role", "name", "id", "value", "metadata", "prefix", "tail")
        self.detail = ttk.Treeview(right, columns=columns, show="headings", height=10)
        headings = {
            "slot": "槽位",
            "role": "类型",
            "name": "词条",
            "id": "ID",
            "value": "数值（原始）",
            "metadata": "元数据",
            "prefix": "前缀",
            "tail": "尾部",
        }
        widths = {"slot": 42, "role": 70, "name": 190, "id": 72, "value": 120, "metadata": 100, "prefix": 90, "tail": 150}
        for column in columns:
            self.detail.heading(column, text=headings[column])
            self.detail.column(column, width=widths[column], anchor="w")
        detail_xscrollbar = ttk.Scrollbar(
            right,
            orient="horizontal",
            command=self.detail.xview,
        )
        self.detail.configure(xscrollcommand=detail_xscrollbar.set)
        detail_xscrollbar.pack(side="bottom", fill="x")
        self.detail.pack(fill=BOTH, expand=True)

        install = ttk.Frame(results_column, padding=(0, 10, 0, 0))
        install.pack(side="bottom", fill="x", before=candidate_frame)
        ttk.Checkbutton(
            install,
            text=TITLE_SCREEN_ACK_TEXT,
            variable=self.title_ack,
        ).pack(side=LEFT, padx=(12, 8))
        self.install_button = ttk.Button(
            install,
            text="把选中的候选添加到存档",
            command=self._install_selected,
            state="disabled",
            style="Accent.TButton",
        )
        self.install_button.pack(side=LEFT, padx=(0, 4))
        ttk.Label(
            install,
            text="添加到最后一张绘卷后的下一个全零栏位，不会覆盖现有绘卷。",
        ).pack(side=LEFT, padx=8)

        self._build_local_editor_tab(self.local_editor_tab)

        footer = ttk.Frame(shell, style="Header.TFrame", height=36)
        footer.pack(side="bottom", fill="x", before=body)
        footer.pack_propagate(False)
        ttk.Label(footer, textvariable=self.status, style="Footer.TLabel").pack(
            side=LEFT,
            padx=18,
        )
        ttk.Button(
            footer,
            text="GitHub",
            command=self._open_project_github,
        ).pack(side=RIGHT, padx=(6, 18), pady=4)
        ttk.Label(
            footer,
            text=f"作者：{' & '.join(APP_AUTHORS)}    QQ群：{CONTACT_QQ_GROUP}",
            style="Footer.TLabel",
        ).pack(side=RIGHT, padx=(8, 0))

    def _open_project_github(self) -> None:
        if not PROJECT_GITHUB_URL:
            messagebox.showinfo("GitHub", "GitHub 联系链接尚未配置。")
            return
        webbrowser.open(PROJECT_GITHUB_URL, new=2)

    def _select_main_tab(self, index: int) -> None:
        self.notebook.select(index)
        self._sync_main_navigation()

    def _sync_main_navigation(self, _event: object | None = None) -> None:
        selected = self.notebook.index(self.notebook.select())
        self.search_nav_button.configure(
            style="RailActive.TButton" if selected == 0 else "Rail.TButton"
        )
        self.local_nav_button.configure(
            style="RailActive.TButton" if selected == 1 else "Rail.TButton"
        )

    def _check_for_updates(self, manual: bool = True) -> None:
        if not UPDATE_MANIFEST_URL or not UPDATE_PUBLIC_KEY_BASE64:
            if manual:
                messagebox.showinfo(
                    "更新通道尚未启用",
                    "当前开发版尚未绑定正式 GitHub Releases 地址和发布公钥。",
                )
            return
        if self.update_setup_error:
            if manual:
                messagebox.showerror(
                    "无法启用自动更新",
                    "当前 EXE 所在目录无法建立受管理更新标记。请把 EXE 放到有写入权限的"
                    f"普通文件夹后重新启动。\n\n{self.update_setup_error}",
                )
            return
        if self.update_worker and self.update_worker.is_alive():
            return
        self.update_button.configure(state="disabled")
        self.status.set("正在检查签名更新清单……")

        def work() -> None:
            try:
                result = check_for_update(
                    APP_VERSION,
                    UPDATE_MANIFEST_URL,
                    UPDATE_PUBLIC_KEY_BASE64,
                )
                self.events.put(("update_check_complete", (result, manual)))
            except Exception:
                self.events.put(("update_error", (traceback.format_exc(), manual)))

        self.update_worker = threading.Thread(target=work, daemon=True)
        self.update_worker.start()

    def _download_available_update(self, result: UpdateCheckResult) -> None:
        self.update_button.configure(state="disabled")
        self.status.set(f"正在下载已签名版本 {result.manifest.version}……")

        def work() -> None:
            try:
                downloaded = download_update(
                    result.manifest,
                    self._backup_state_root(),
                )
                self.events.put(("update_download_complete", downloaded))
            except Exception:
                self.events.put(("update_error", (traceback.format_exc(), True)))

        self.update_worker = threading.Thread(target=work, daemon=True)
        self.update_worker.start()

    def _offer_downloaded_update_when_idle(self, downloaded: DownloadedUpdate) -> None:
        if self.worker and self.worker.is_alive():
            self.pending_update = downloaded
            self.status.set(
                f"版本 {downloaded.manifest.version} 已验证；等待当前存档/计算事务结束。"
            )
            self.root.after(500, self._offer_pending_update)
            return
        self.pending_update = None
        self.update_button.configure(state="normal")
        try:
            script = prepare_managed_update_script(
                downloaded,
                current_executable=Path(sys.executable),
                state_root=self._backup_state_root(),
            )
        except Exception as error:
            self.status.set(f"版本 {downloaded.manifest.version} 已下载并通过验证。")
            messagebox.showinfo(
                "更新已安全下载",
                f"新版本已通过签名和哈希验证，但当前运行位置不是受管理安装版，"
                f"无法自动替换。\n\n{error}\n\n文件：{downloaded.path}",
            )
            return
        if not messagebox.askyesno(
            "安装更新并重启",
            f"版本 {downloaded.manifest.version} 已通过 Ed25519 签名和 SHA-256 验证。\n\n"
            "现在关闭应用、替换受管理安装目录中的旧版本并重启吗？",
        ):
            self.status.set(f"版本 {downloaded.manifest.version} 已下载，等待安装。")
            return
        launch_managed_update(
            script,
            downloaded,
            current_executable=Path(sys.executable),
        )
        self.root.destroy()

    def _offer_pending_update(self) -> None:
        downloaded = self.pending_update
        if downloaded is None:
            return
        self._offer_downloaded_update_when_idle(downloaded)

    def _build_local_editor_tab(self, parent: ttk.Frame) -> None:
        outer = ttk.Frame(parent, padding=12)
        outer.pack(fill=BOTH, expand=True)

        warning = ttk.LabelFrame(outer, text="本地编辑说明", padding=10)
        warning.pack(fill="x")
        ttk.Label(
            warning,
            text=(
                "这里直接修改存档中的最终词条槽。修改结果可在本机使用，"
                "但传播时接收方会根据稀有度和 Seed 重新生成，不会保留这些改动。"
            ),
            foreground="#FF8585",
            wraplength=1080,
        ).pack(anchor="w")
        ttk.Checkbutton(
            warning,
            text=TITLE_SCREEN_ACK_TEXT,
            variable=self.title_ack,
        ).pack(anchor="w", pady=(6, 0))

        controls = ttk.Frame(outer, padding=(0, 8, 0, 8))
        controls.pack(fill="x")
        ttk.Label(controls, text="存档账户：").pack(side=LEFT)
        self.local_save_combo = ttk.Combobox(
            controls,
            textvariable=self.save_account,
            state="readonly",
            width=34,
        )
        self.local_save_combo.pack(side=LEFT, padx=(4, 8))
        ttk.Button(
            controls,
            text="读取当前绘卷",
            command=self._refresh_local_inventory,
        ).pack(side=LEFT)
        ttk.Button(
            controls,
            text="删除选中绘卷",
            command=self._delete_local_scrolls,
            style="Danger.TButton",
        ).pack(side=LEFT, padx=(8, 0))

        panes = ttk.Panedwindow(outer, orient="horizontal")
        panes.pack(fill=BOTH, expand=True)
        inventory_frame = ttk.LabelFrame(panes, text="当前存档绘卷", padding=8)
        effect_frame = ttk.LabelFrame(panes, text="词条槽", padding=8)
        editor_frame = ttk.LabelFrame(panes, text="本地字段编辑", padding=8)
        panes.add(inventory_frame, weight=2)
        panes.add(effect_frame, weight=3)
        panes.add(editor_frame, weight=3)

        inventory_columns = (
            "slot",
            "seed",
            "rarity",
            "playthrough",
            "record_type",
            "transfer",
        )
        self.local_scroll_tree = ttk.Treeview(
            inventory_frame,
            columns=inventory_columns,
            show="headings",
            selectmode="extended",
            height=11,
        )
        inventory_headings = {
            "slot": "栏位",
            "seed": "Seed",
            "rarity": "稀有度",
            "playthrough": "周目",
            "record_type": "类型",
            "transfer": "转手",
        }
        inventory_widths = {
            "slot": 54,
            "seed": 94,
            "rarity": 54,
            "playthrough": 72,
            "record_type": 72,
            "transfer": 72,
        }
        for column in inventory_columns:
            self.local_scroll_tree.heading(column, text=inventory_headings[column])
            self.local_scroll_tree.column(
                column,
                width=inventory_widths[column],
                anchor="w",
            )
        inventory_scrollbar = ttk.Scrollbar(
            inventory_frame,
            orient="vertical",
            command=self.local_scroll_tree.yview,
        )
        self.local_scroll_tree.configure(yscrollcommand=inventory_scrollbar.set)
        self.local_scroll_tree.pack(side=LEFT, fill=BOTH, expand=True)
        inventory_scrollbar.pack(side=RIGHT, fill="y")
        self.local_scroll_tree.bind(
            "<<TreeviewSelect>>",
            self._show_local_scroll_effects,
        )

        effect_columns = ("slot", "role", "name", "id", "value")
        self.local_effect_tree = ttk.Treeview(
            effect_frame,
            columns=effect_columns,
            show="headings",
            selectmode="browse",
            height=11,
        )
        effect_headings = {
            "slot": "槽",
            "role": "角色",
            "name": "词条",
            "id": "ID",
            "value": "数值",
        }
        effect_widths = {
            "slot": 42,
            "role": 64,
            "name": 190,
            "id": 82,
            "value": 86,
        }
        for column in effect_columns:
            self.local_effect_tree.heading(column, text=effect_headings[column])
            self.local_effect_tree.column(
                column,
                width=effect_widths[column],
                anchor="w",
            )
        effect_scrollbar = ttk.Scrollbar(
            effect_frame,
            orient="vertical",
            command=self.local_effect_tree.yview,
        )
        self.local_effect_tree.configure(yscrollcommand=effect_scrollbar.set)
        self.local_effect_tree.pack(side=LEFT, fill=BOTH, expand=True)
        effect_scrollbar.pack(side=RIGHT, fill="y")
        self.local_effect_tree.bind("<<TreeviewSelect>>", self._show_local_effect_fields)

        ttk.Label(editor_frame, text="搜索官方词条目录").pack(anchor="w")
        ttk.Entry(editor_frame, textvariable=self.local_effect_search).pack(
            fill="x",
            pady=(2, 4),
        )
        self.local_effect_combo = ttk.Combobox(
            editor_frame,
            textvariable=self.local_effect_choice,
            state="readonly",
            values=tuple(effect.label for effect in self.local_effect_catalog),
        )
        self.local_effect_combo.pack(fill="x")
        self.local_effect_combo.bind("<<ComboboxSelected>>", self._choose_local_effect)

        field_rows = (
            ("词条 ID", self.local_effect_id),
            ("数值 raw", self.local_effect_value),
            ("prefix", self.local_effect_prefix),
            ("metadata", self.local_effect_metadata),
            ("tail 0", self.local_effect_tail_0),
            ("tail 1", self.local_effect_tail_1),
        )
        for label, variable in field_rows:
            row = ttk.Frame(editor_frame)
            row.pack(fill="x", pady=2)
            ttk.Label(row, text=label, width=11).pack(side=LEFT)
            ttk.Entry(row, textvariable=variable).pack(side=LEFT, fill="x", expand=True)

        ttk.Label(
            editor_frame,
            text=(
                "只改词条 ID 和数值即可；其余高级字段默认保留。"
                "支持十进制和 0x 十六进制。"
            ),
            foreground=self.colors["muted"],
            wraplength=330,
        ).pack(anchor="w", pady=(8, 6))
        button_row = ttk.Frame(editor_frame)
        button_row.pack(fill="x")
        ttk.Button(
            button_row,
            text="保存本地修改",
            command=self._save_local_effect_edit,
            style="Accent.TButton",
        ).pack(side=LEFT)
        ttk.Button(
            button_row,
            text="清空该词条槽",
            command=self._clear_local_effect_slot,
            style="Danger.TButton",
        ).pack(side=LEFT, padx=(8, 0))

        backup_frame = ttk.LabelFrame(outer, text="自动备份", padding=8)
        backup_frame.pack(fill="x", pady=(8, 0))
        backup_columns = ("time", "account", "action", "files", "hash")
        self.backup_tree = ttk.Treeview(
            backup_frame,
            columns=backup_columns,
            show="headings",
            selectmode="extended",
            height=4,
        )
        backup_headings = {
            "time": "时间（UTC）",
            "account": "账户",
            "action": "原因",
            "files": "文件数",
            "hash": "主存档 SHA-256",
        }
        backup_widths = {
            "time": 160,
            "account": 150,
            "action": 155,
            "files": 62,
            "hash": 245,
        }
        for column in backup_columns:
            self.backup_tree.heading(column, text=backup_headings[column])
            self.backup_tree.column(
                column,
                width=backup_widths[column],
                anchor="w",
            )
        backup_scrollbar = ttk.Scrollbar(
            backup_frame,
            orient="vertical",
            command=self.backup_tree.yview,
        )
        self.backup_tree.configure(yscrollcommand=backup_scrollbar.set)
        self.backup_tree.pack(side=LEFT, fill="x", expand=True)
        backup_scrollbar.pack(side=LEFT, fill="y")

        backup_actions = ttk.Frame(backup_frame)
        backup_actions.pack(side=RIGHT, fill="y", padx=(8, 0))
        ttk.Button(
            backup_actions,
            text="刷新备份",
            command=self._refresh_backups,
        ).pack(fill="x")
        ttk.Button(
            backup_actions,
            text="恢复选中备份",
            command=self._restore_selected_backup,
            style="Accent.TButton",
        ).pack(fill="x", pady=(4, 0))
        ttk.Button(
            backup_actions,
            text="移入回收站",
            command=self._recycle_selected_backups,
            style="Danger.TButton",
        ).pack(fill="x", pady=(4, 0))
        ttk.Button(
            backup_actions,
            text="打开备份文件夹",
            command=self._open_backup_folder,
        ).pack(fill="x", pady=(4, 0))
        ttk.Button(
            backup_actions,
            text="打开存档文件夹",
            command=self._open_save_folder,
        ).pack(fill="x", pady=(4, 0))

    @staticmethod
    def _backup_state_root() -> Path:
        project_root = application_root()
        return Path(os.environ.get("LOCALAPPDATA", project_root)) / "Nioh3ScrollGenerator"

    @staticmethod
    def _backup_action_label(action: str) -> str:
        return {
            "scroll-install": "添加绘卷",
            "local-effect-edit": "本地词条编辑",
            "local-scroll-delete": "删除绘卷",
            "restore-backup": "恢复前检查点",
            "unknown": "旧版/未知操作",
        }.get(action, action)

    def _refresh_backups(self) -> None:
        if not hasattr(self, "backup_tree"):
            return
        try:
            entries = list_backup_entries(self._backup_state_root())
        except Exception as error:
            messagebox.showerror("读取备份失败", str(error))
            return
        self.backup_entries = list(entries)
        self.backup_entry_by_iid.clear()
        current_items = self.backup_tree.get_children()
        if current_items:
            self.backup_tree.delete(*current_items)
        for index, entry in enumerate(entries):
            iid = f"backup-{index}"
            self.backup_entry_by_iid[iid] = entry
            self.backup_tree.insert(
                "",
                END,
                iid=iid,
                values=(
                    entry.timestamp,
                    entry.account_id if entry.account_id is not None else "未知",
                    self._backup_action_label(entry.action),
                    entry.file_count,
                    entry.main_save_sha256 or "无主存档",
                ),
            )

    def _selected_backup_entries(self) -> tuple[BackupEntry, ...]:
        return tuple(
            self.backup_entry_by_iid[iid]
            for iid in self.backup_tree.selection()
            if iid in self.backup_entry_by_iid
        )

    def _restore_selected_backup(self) -> None:
        selected = self._selected_backup_entries()
        if len(selected) != 1:
            messagebox.showerror("请选择一个备份", "恢复时必须且只能选择一个备份")
            return
        if not self._confirm_title_screen_if_needed("恢复备份"):
            return
        entry = selected[0]
        if not messagebox.askyesno(
            "确认恢复整个存档",
            f"确定恢复备份 {entry.timestamp} 吗？\n\n"
            "备份中包含的主存档、游戏备份和系统存档会按原角色恢复。"
            "覆盖前会先把当前文件再保存为一个新的恢复点。",
        ):
            return
        try:
            result = self._local_installer().restore_backup(entry.directory)
        except Exception as error:
            messagebox.showerror("恢复备份失败", str(error))
            return
        self.status.set(
            f"已恢复备份 {entry.timestamp}；恢复前检查点：{result.checkpoint_directory.name}"
        )
        self._refresh_backups()
        self._refresh_local_inventory()

    def _recycle_selected_backups(self) -> None:
        selected = self._selected_backup_entries()
        if not selected:
            messagebox.showerror("未选择备份", "请先选择至少一个要移入回收站的备份")
            return
        if not messagebox.askyesno(
            "确认删除自动备份",
            f"确定把选中的 {len(selected)} 个备份目录移入 Windows 回收站吗？",
        ):
            return
        try:
            for entry in selected:
                move_backup_to_recycle_bin(self._backup_state_root(), entry.directory)
        except Exception as error:
            messagebox.showerror("删除备份失败", str(error))
            self._refresh_backups()
            return
        self.status.set(f"已把 {len(selected)} 个自动备份移入回收站")
        self._refresh_backups()

    @staticmethod
    def _open_directory(path: Path) -> None:
        if os.name != "nt":
            raise OSError("该快捷入口只支持 Windows")
        os.startfile(path.resolve())

    def _open_backup_folder(self) -> None:
        try:
            directory = self._backup_state_root() / "backups"
            directory.mkdir(parents=True, exist_ok=True)
            self._open_directory(directory)
        except Exception as error:
            messagebox.showerror("打开备份文件夹失败", str(error))

    def _open_save_folder(self) -> None:
        try:
            self._open_directory(self._selected_save_path().parent)
        except Exception as error:
            messagebox.showerror("打开存档文件夹失败", str(error))

    def _filter_local_effect_catalog(self, *_args: object) -> None:
        if not hasattr(self, "local_effect_combo"):
            return
        query = self.local_effect_search.get().strip().casefold()
        values = tuple(
            effect.label
            for effect in self.local_effect_catalog
            if not query
            or query in effect.name.casefold()
            or query in f"0x{effect.effect_id:08X}".casefold()
            or query in f"{effect.effect_id:08X}".casefold()
        )
        self.local_effect_combo.configure(values=values)
        if self.local_effect_choice.get() not in values:
            self.local_effect_choice.set("")

    def _choose_local_effect(self, _event: object | None = None) -> None:
        effect_id = self.local_effect_label_to_id.get(self.local_effect_choice.get())
        if effect_id is not None:
            self.local_effect_id.set(f"0x{effect_id:08X}")

    @staticmethod
    def _parse_local_u32(value: str, field_name: str) -> int:
        normalized = value.strip()
        if not normalized:
            raise ValueError(f"{field_name}不能为空")
        if normalized.lower().startswith("0x"):
            parsed = int(normalized, 16)
        elif any(character in "abcdefABCDEF" for character in normalized):
            parsed = int(normalized, 16)
        else:
            parsed = int(normalized, 10)
        if not 0 <= parsed <= 0xFFFFFFFF:
            raise ValueError(f"{field_name}必须在 0 到 0xFFFFFFFF 之间")
        return parsed

    def _local_installer(self) -> SaveInstaller:
        save_path = self._selected_save_path()
        project_root = application_root()
        return SaveInstaller(
            save_path=save_path,
            crypto=SaveCrypto(default_crypto_tool(project_root)),
            state_root=self._backup_state_root(),
        )

    def _refresh_local_inventory(self) -> None:
        try:
            inventory = self._local_installer().capture_inventory()
        except Exception as error:
            messagebox.showerror("读取绘卷失败", str(error))
            return
        self.local_inventory = inventory
        self.local_entries = list(inventory.scroll_entries())
        self.local_entry_by_iid.clear()
        scroll_items = self.local_scroll_tree.get_children()
        if scroll_items:
            self.local_scroll_tree.delete(*scroll_items)
        effect_items = self.local_effect_tree.get_children()
        if effect_items:
            self.local_effect_tree.delete(*effect_items)
        for entry in self.local_entries:
            iid = f"slot-{entry.slot_index}"
            self.local_entry_by_iid[iid] = entry
            playthrough = (
                playthrough_label(entry.playthrough)
                if entry.playthrough is not None
                else "未知/异常"
            )
            self.local_scroll_tree.insert(
                "",
                END,
                iid=iid,
                values=(
                    entry.slot_index,
                    entry.seed,
                    entry.rarity,
                    playthrough,
                    f"0x{entry.record_type:04X}",
                    entry.transfer_count,
                ),
            )
        self.status.set(f"已读取 {len(self.local_entries)} 个占用绘卷栏位")

    def _selected_local_entries(self) -> tuple[ScrollInventoryEntry, ...]:
        return tuple(
            self.local_entry_by_iid[iid]
            for iid in self.local_scroll_tree.selection()
            if iid in self.local_entry_by_iid
        )

    def _show_local_scroll_effects(self, _event: object | None = None) -> None:
        selected = self._selected_local_entries()
        effect_items = self.local_effect_tree.get_children()
        if effect_items:
            self.local_effect_tree.delete(*effect_items)
        if not selected:
            return
        entry = selected[0]
        for index, effect in enumerate(entry.candidate.effects):
            if index == 0:
                role = "主词条"
            elif entry.candidate.grace_slot_index == index:
                role = "恩宠"
            elif entry.rarity == 3 and index == 4:
                role = "成长槽"
            else:
                role = "副词条"
            self.local_effect_tree.insert(
                "",
                END,
                iid=f"effect-{index}",
                values=(
                    index + 1,
                    role,
                    entry.candidate.display_name(effect),
                    f"0x{effect.effect_id:08X}",
                    f"0x{effect.value:08X}",
                ),
            )

    def _selected_local_effect(self) -> tuple[ScrollInventoryEntry, int] | None:
        entries = self._selected_local_entries()
        effect_selection = self.local_effect_tree.selection()
        if len(entries) != 1 or not effect_selection:
            return None
        try:
            effect_index = int(effect_selection[0].split("-", 1)[1])
        except (IndexError, ValueError):
            return None
        return entries[0], effect_index

    def _show_local_effect_fields(self, _event: object | None = None) -> None:
        selected = self._selected_local_effect()
        if selected is None:
            return
        entry, effect_index = selected
        offset = 0x34 + effect_index * 0x18
        prefix, effect_id, value, metadata, tail_0, tail_1 = struct.unpack_from(
            "<6I",
            entry.record,
            offset,
        )
        self.local_effect_id.set(f"0x{effect_id:08X}")
        self.local_effect_value.set(f"0x{value:08X}")
        self.local_effect_prefix.set(f"0x{prefix:08X}")
        self.local_effect_metadata.set(f"0x{metadata:08X}")
        self.local_effect_tail_0.set(f"0x{tail_0:08X}")
        self.local_effect_tail_1.set(f"0x{tail_1:08X}")
        self.local_effect_choice.set("")

    def _save_local_effect_edit(self) -> None:
        selected = self._selected_local_effect()
        if selected is None:
            messagebox.showerror("未选择词条", "请先选择一张绘卷和一个词条槽")
            return
        entry, effect_index = selected
        try:
            edit = LocalEffectEdit(
                slot_index=effect_index,
                effect_id=self._parse_local_u32(self.local_effect_id.get(), "词条 ID"),
                value=self._parse_local_u32(self.local_effect_value.get(), "数值"),
                prefix=self._parse_local_u32(self.local_effect_prefix.get(), "prefix"),
                metadata=self._parse_local_u32(self.local_effect_metadata.get(), "metadata"),
                tail_0=self._parse_local_u32(self.local_effect_tail_0.get(), "tail 0"),
                tail_1=self._parse_local_u32(self.local_effect_tail_1.get(), "tail 1"),
            )
            replacement = patch_local_scroll_record(entry.record, (edit,))
        except Exception as error:
            messagebox.showerror("本地编辑字段无效", str(error))
            return
        if replacement == entry.record:
            messagebox.showinfo("没有变化", "当前字段与存档记录完全相同")
            return
        if not self._confirm_title_screen_if_needed("修改本地绘卷"):
            return
        if not messagebox.askyesno(
            "确认本地修改",
            f"确定修改栏位 {entry.slot_index} 的第 {effect_index + 1} 个词条吗？\n\n"
            "该修改只在本机存档生效，传播后接收方不会保留。写入前会自动备份。",
        ):
            return
        try:
            result = self._local_installer().edit_many(
                ((entry.slot_index, entry.record, replacement),),
                action="local-effect-edit",
                metadata={
                    "local_only": True,
                    "effect_slot": effect_index,
                    "seed": entry.seed,
                },
            )
        except Exception as error:
            messagebox.showerror("本地修改失败", str(error))
            return
        self.status.set(
            f"已修改栏位 {entry.slot_index}；备份：{result.backup_directory.name}"
        )
        self._refresh_backups()
        self._refresh_local_inventory()

    def _clear_local_effect_slot(self) -> None:
        selected = self._selected_local_effect()
        if selected is None:
            messagebox.showerror("未选择词条", "请先选择一张绘卷和一个词条槽")
            return
        self.local_effect_id.set("0xFFFFFFFF")
        self.local_effect_value.set("0")
        self.local_effect_prefix.set("0")
        self.local_effect_metadata.set("0")
        self.local_effect_tail_0.set("0")
        self.local_effect_tail_1.set("0")

    def _delete_local_scrolls(self) -> None:
        selected = self._selected_local_entries()
        if not selected:
            messagebox.showerror("未选择绘卷", "请先选择至少一张要删除的绘卷")
            return
        if not self._confirm_title_screen_if_needed("删除本地绘卷"):
            return
        slot_text = ", ".join(str(entry.slot_index) for entry in selected)
        if not messagebox.askyesno(
            "确认删除绘卷",
            f"确定清零栏位 {slot_text} 吗？\n\n"
            "不会移动其他栏位；删除前会自动备份当前存档。",
        ):
            return
        try:
            result = self._local_installer().delete_many(selected)
        except Exception as error:
            messagebox.showerror("删除绘卷失败", str(error))
            return
        self.status.set(
            f"已清零 {len(result.slot_indices)} 个绘卷栏位；备份：{result.backup_directory.name}"
        )
        self._refresh_backups()
        self._refresh_local_inventory()

    @staticmethod
    def _entry_row(parent: ttk.Frame, label: str, variable: StringVar, width: int) -> None:
        row = ttk.Frame(parent)
        row.pack(fill="x", pady=1)
        ttk.Label(row, text=label, width=12).pack(side=LEFT)
        ttk.Entry(row, textvariable=variable, width=width).pack(side=RIGHT)

    @staticmethod
    def _inline_entry(parent: ttk.Frame, label: str, variable: StringVar, width: int) -> None:
        ttk.Label(parent, text=label).pack(side=LEFT, padx=(0, 4))
        ttk.Entry(parent, textvariable=variable, width=width).pack(side=LEFT, padx=(0, 10))

    @staticmethod
    def _effect_matches(effect: object, query: str) -> bool:
        normalized = query.strip().casefold()
        if not normalized:
            return True
        effect_id = effect.effect_id
        searchable = (
            effect.name,
            f"0x{effect_id:04X}",
            f"{effect_id:04X}",
            effect_id.to_bytes(4, "little").hex(" ").upper(),
        )
        return any(normalized in value.casefold() for value in searchable)

    def _populate_effect_result_list(self) -> None:
        if not hasattr(self, "effect_result_list"):
            return
        self.effect_result_list.delete(0, END)
        for effect in self.effect_visible:
            self.effect_result_list.insert(END, effect.label)
        if self.effect_visible:
            self.effect_result_list.selection_set(0)
        if hasattr(self, "effect_result_summary"):
            self.effect_result_summary.set(
                f"显示 {len(self.effect_visible)} / {len(self.search_effect_catalog)} 项"
            )

    def _filter_effect_catalog(self, *_args: object) -> None:
        self.effect_visible = [
            effect
            for effect in self.search_effect_catalog
            if self._effect_matches(effect, self.effect_search.get())
        ]
        self._populate_effect_result_list()

    def _sync_effect_roles(self) -> None:
        if self.primary_unconstrained.get():
            self.selected_primary_ids = set()
            self.selected_secondary_ids = set(self.selected_effect_ids)
        else:
            try:
                primary_count = max(1, int(self.primary_candidate_count.get(), 10))
            except ValueError:
                primary_count = 1
            self.selected_primary_ids = set(
                self.selected_effect_ids[:primary_count]
            )
            self.selected_secondary_ids = set(
                self.selected_effect_ids[primary_count:]
            )

    def _max_selected_effect_count(self) -> int:
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            rarity = 4
        grace_is_required = self.grace_filter.get() != NO_GRACE_FILTER_LABEL
        max_secondaries = 3 if rarity == 3 or grace_is_required else 4
        try:
            primary_count = max(1, int(self.primary_candidate_count.get(), 10))
        except ValueError:
            primary_count = 1
        return max_secondaries + (
            0 if self.primary_unconstrained.get() else primary_count
        )

    def _on_primary_mode_changed(self, *_args: object) -> None:
        self._sync_effect_roles()
        self._render_selected_effects()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _refresh_combination_status(self, *_args: object) -> None:
        if not hasattr(self, "combination_status"):
            return
        self._sync_effect_roles()
        try:
            rarity = int(self.rarity.get(), 0)
            playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
            if playthrough is None or rarity not in PRODUCT_RARITIES:
                raise ValueError("请先选择有效的周目和稀有度")
            grace_id = self.special_id_by_label.get(self.grace_filter.get())
            request = EffectSeedRequest(
                playthrough=playthrough,
                rarity=rarity,
                primary_effect_ids=frozenset(self.selected_primary_ids),
                required_secondary_ids=frozenset(self.selected_secondary_ids),
                grace_effect_id=grace_id,
                minimum_roll_percent_by_effect_id=tuple(
                    sorted(self.selected_effect_min_rolls.items())
                ),
            )
            validate_effect_request_feasibility(request)
        except (KeyError, ValueError) as error:
            self.combination_status.set(f"组合状态：原生结构无解 — {error}")
            return
        if not self.selected_effect_ids and grace_id is None:
            self.combination_status.set("组合状态：尚未选择词条条件。")
        else:
            self.combination_status.set(
                "组合状态：已通过槽位、冲突组和类别容量预检；仍需计算 Seed 才能确认有解。"
            )

    def _add_selected_effect_result(self) -> None:
        selection = self.effect_result_list.curselection()
        if not selection:
            self.status.set("请先从搜索结果中选择一个词条。")
            return
        effect_id = self.effect_visible[selection[0]].effect_id
        if effect_id in self.selected_effect_ids:
            self.status.set("该词条已在目标组合中。")
            return
        max_count = self._max_selected_effect_count()
        if len(self.selected_effect_ids) >= max_count:
            role_text = "必需副词条" if self.primary_unconstrained.get() else "词条"
            self.status.set(
                f"当前稀有度/恩宠结构最多选择 {max_count} 个{role_text}；"
                "继续添加在原生结构中必定无解。"
            )
            return
        self.selected_effect_ids.append(effect_id)
        self._sync_effect_roles()
        self._render_selected_effects()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _remove_selected_effect(self, effect_id: int) -> None:
        try:
            self.selected_effect_ids.remove(effect_id)
        except ValueError:
            return
        self.selected_effect_min_rolls.pop(effect_id, None)
        self._sync_effect_roles()
        self._render_selected_effects()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _set_effect_minimum_roll(self, effect_id: int, label: str) -> None:
        minimum_roll = EFFECT_ROLL_BY_LABEL.get(label)
        if minimum_roll is None:
            return
        if minimum_roll:
            self.selected_effect_min_rolls[effect_id] = minimum_roll
        else:
            self.selected_effect_min_rolls.pop(effect_id, None)
        self._mark_intersection_stale()
        self._refresh_combination_status()

    def _start_effect_drag(self, effect_id: int) -> None:
        self._dragged_effect_id = effect_id

    def _finish_effect_drag(self, _event: object | None = None) -> None:
        effect_id = self._dragged_effect_id
        self._dragged_effect_id = None
        if effect_id is None or effect_id not in self.selected_effect_ids:
            return
        pointer_y = self.root.winfo_pointery()
        target_index = len(self._selected_effect_row_widgets) - 1
        for index, row in enumerate(self._selected_effect_row_widgets):
            midpoint = row.winfo_rooty() + row.winfo_height() // 2
            if pointer_y < midpoint:
                target_index = index
                break
        source_index = self.selected_effect_ids.index(effect_id)
        if source_index == target_index:
            return
        self.selected_effect_ids.pop(source_index)
        self.selected_effect_ids.insert(target_index, effect_id)
        self._sync_effect_roles()
        self._render_selected_effects()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _move_selected_effect(self, effect_id: int, delta: int) -> None:
        try:
            source_index = self.selected_effect_ids.index(effect_id)
        except ValueError:
            return
        target_index = max(
            0,
            min(len(self.selected_effect_ids) - 1, source_index + delta),
        )
        if target_index == source_index:
            return
        self.selected_effect_ids.pop(source_index)
        self.selected_effect_ids.insert(target_index, effect_id)
        self._sync_effect_roles()
        self._render_selected_effects()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _render_selected_effects(self) -> None:
        if not hasattr(self, "selected_effects_frame"):
            return
        for child in self.selected_effects_frame.winfo_children():
            child.destroy()
        self._selected_effect_row_widgets: list[ttk.Frame] = []
        if not self.selected_effect_ids:
            ttk.Label(
                self.selected_effects_frame,
                text="尚未添加词条；不指定主词条时可仅用恩宠或辅助条件求解。",
                foreground=self.colors["muted"],
                wraplength=430,
            ).pack(anchor="w")
            return
        for index, effect_id in enumerate(self.selected_effect_ids):
            effect = self.search_effect_by_id[effect_id]
            row = ttk.Frame(self.selected_effects_frame, style="SelectedEffect.TFrame")
            row.pack(fill="x", pady=2)
            self._selected_effect_row_widgets.append(row)
            try:
                primary_count = max(1, int(self.primary_candidate_count.get(), 10))
            except ValueError:
                primary_count = 1
            role = (
                "副"
                if self.primary_unconstrained.get()
                else ("主候选" if index < primary_count else "副")
            )
            role_label = ttk.Label(
                row,
                text=f"{role} {index + 1}",
                style="SelectedEffectRole.TLabel",
                width=7,
                anchor="center",
            )
            role_label.pack(side=LEFT, padx=(6, 8), pady=5)
            name_label = ttk.Label(
                row,
                text=f"{effect.name}  [{effect.hex_id}]",
                style="SelectedEffect.TLabel",
            )
            name_label.pack(side=LEFT, fill="x", expand=True, pady=5)
            roll_label = next(
                label
                for label, minimum in EFFECT_ROLL_FILTERS
                if minimum == self.selected_effect_min_rolls.get(effect_id, 0)
            )
            roll_variable = StringVar(value=roll_label)
            roll_combo = ttk.Combobox(
                row,
                textvariable=roll_variable,
                values=tuple(label for label, _minimum in EFFECT_ROLL_FILTERS),
                state="readonly",
                width=24,
            )
            roll_combo.pack(side=RIGHT, padx=(4, 0), pady=2)
            roll_combo.bind(
                "<<ComboboxSelected>>",
                lambda _event, value=effect_id, variable=roll_variable: (
                    self._set_effect_minimum_roll(value, variable.get())
                ),
            )
            ttk.Button(
                row,
                text="×",
                width=3,
                command=lambda value=effect_id: self._remove_selected_effect(value),
            ).pack(side=RIGHT, padx=4, pady=2)
            ttk.Button(
                row,
                text="↓",
                width=3,
                state="normal" if index < len(self.selected_effect_ids) - 1 else "disabled",
                command=lambda value=effect_id: self._move_selected_effect(value, 1),
            ).pack(side=RIGHT, padx=(1, 0), pady=2)
            ttk.Button(
                row,
                text="↑",
                width=3,
                state="normal" if index > 0 else "disabled",
                command=lambda value=effect_id: self._move_selected_effect(value, -1),
            ).pack(side=RIGHT, padx=(4, 0), pady=2)
            for widget in (row, role_label, name_label):
                widget.bind(
                    "<ButtonPress-1>",
                    lambda _event, value=effect_id: self._start_effect_drag(value),
                )
                widget.bind("<ButtonRelease-1>", self._finish_effect_drag)

    @staticmethod
    def _key_option_matches(option: tuple[int, str], query: str) -> bool:
        normalized = query.strip().casefold()
        if not normalized:
            return True
        key, label = option
        searchable = (label, f"0x{key:X}", f"{key:X}")
        return any(normalized in value.casefold() for value in searchable)

    def _populate_rule_tree(self) -> None:
        if not hasattr(self, "rule_tree"):
            return
        query = self.rule_search.get().strip().casefold()
        self.rule_tree.delete(*self.rule_tree.get_children())
        self.visible_rule_tokens.clear()
        for group, variants in self.rule_group_options:
            group_matches = not query or query in group.name.casefold()
            matched_variants = tuple(
                option
                for option in variants
                if not query
                or query in option.label.casefold()
                or query in option.name.casefold()
            )
            if not group_matches and not matched_variants:
                continue
            shown_variants = variants if group_matches else matched_variants
            self.rule_tree.insert(
                "",
                END,
                iid=group.token,
                text=group.label,
                open=bool(query),
            )
            self.visible_rule_tokens.add(group.token)
            for option in shown_variants:
                self.rule_tree.insert(
                    group.token,
                    END,
                    iid=option.token,
                    text=option.label,
                )
                self.visible_rule_tokens.add(option.token)
        self._restore_rule_selection()

    def _restore_rule_selection(self) -> None:
        if not hasattr(self, "rule_tree"):
            return
        visible_selected = tuple(
            token
            for token in self.selected_rule_option_ids
            if token in self.visible_rule_tokens
        )
        self.rule_tree.selection_set(visible_selected)

    def _update_rule_summary(self) -> None:
        options = sorted(
            (
                self.rule_option_by_token[token]
                for token in self.selected_rule_option_ids
            ),
            key=lambda option: (option.name.casefold(), option.label.casefold()),
        )
        labels = [option.label for option in options]
        summary = "、".join(labels[:3])
        if len(labels) > 3:
            summary += f" 等{len(labels)}项"
        self.selected_rule_text.set(
            "特殊规则必须包含：" + (summary or "未选择（不筛选）")
        )
        if not hasattr(self, "selected_rule_chips"):
            return
        for child in self.selected_rule_chips.winfo_children():
            child.destroy()
        for option in options:
            ttk.Button(
                self.selected_rule_chips,
                text=f"{option.label}  ×",
                command=lambda token=option.token: self._remove_rule_selection(token),
            ).pack(fill="x", pady=1)

    def _toggle_rule_selection(self, token: str) -> None:
        self.selected_rule_option_ids = toggle_rule_filter_option(
            self.selected_rule_option_ids,
            token,
            self.rule_family_by_token,
        )
        self._restore_rule_selection()
        self._update_rule_summary()
        self._mark_intersection_stale()

    def _remove_rule_selection(self, token: str) -> None:
        self.selected_rule_option_ids.discard(token)
        self._restore_rule_selection()
        self._update_rule_summary()
        self._mark_intersection_stale()

    def _clear_rule_selection(self) -> None:
        self.selected_rule_option_ids.clear()
        self._restore_rule_selection()
        self._update_rule_summary()
        self._mark_intersection_stale()

    def _on_rule_click(self, event: object) -> str | None:
        x = int(getattr(event, "x"))
        y = int(getattr(event, "y"))
        token = self.rule_tree.identify_row(y)
        if not token:
            return None
        if "indicator" in self.rule_tree.identify_element(x, y):
            return None
        self.rule_tree.focus(token)
        self._toggle_rule_selection(token)
        return "break"

    def _toggle_focused_rule(self, _event: object | None = None) -> str:
        token = self.rule_tree.focus()
        if token:
            self._toggle_rule_selection(token)
        return "break"

    def _filter_rules(self, *_args: object) -> None:
        self._populate_rule_tree()

    def _sync_enemy_selection(self) -> None:
        visible_keys = {key for key, _label in self.enemy_visible}
        selected_visible = {
            self.enemy_visible[index][0] for index in self.enemy_list.curselection()
        }
        self.selected_enemy_keys = (
            self.selected_enemy_keys - visible_keys
        ) | selected_visible
        self._update_enemy_summary()

    def _restore_enemy_selection(self) -> None:
        self.enemy_list.selection_clear(0, END)
        for index, (key, _label) in enumerate(self.enemy_visible):
            if key in self.selected_enemy_keys:
                self.enemy_list.selection_set(index)

    def _update_enemy_summary(self) -> None:
        labels = [
            label.rsplit(" [", 1)[0]
            for key, label in self.enemy_options
            if key in self.selected_enemy_keys
        ]
        summary = "、".join(labels[:3])
        if len(labels) > 3:
            summary += f" 等{len(labels)}项"
        self.selected_enemy_text.set("出现敌人必须包含：" + (summary or "无"))

    def _on_enemy_select(self, _event: object | None = None) -> None:
        self._sync_enemy_selection()
        self._mark_intersection_stale()

    def _filter_enemies(self, *_args: object) -> None:
        self._sync_enemy_selection()
        self.enemy_visible = [
            option
            for option in self.enemy_options
            if self._key_option_matches(option, self.enemy_search.get())
        ]
        self.enemy_list.delete(0, END)
        for _key, label in self.enemy_visible:
            self.enemy_list.insert(END, label)
        self._restore_enemy_selection()

    def _show_tutorial(self) -> None:
        window = Toplevel(self.root)
        window.title("仁王3绘卷生成器 - 使用教程")
        window.geometry("780x650")
        window.transient(self.root)
        notebook = ttk.Notebook(window)
        notebook.pack(fill=BOTH, expand=True, padx=12, pady=12)

        def add_page(label: str, content: str) -> None:
            frame = ttk.Frame(notebook, padding=8)
            notebook.add(frame, text=label)
            text = Text(frame, wrap=WORD, padx=12, pady=12)
            scrollbar = ttk.Scrollbar(frame, orient="vertical", command=text.yview)
            text.configure(yscrollcommand=scrollbar.set)
            text.pack(side=LEFT, fill=BOTH, expand=True)
            scrollbar.pack(side=RIGHT, fill="y")
            text.insert("1.0", content)
            text.tag_configure(
                "tutorial_title",
                font=("Microsoft YaHei UI", 16, "bold"),
                foreground=self.colors["accent"],
                spacing3=10,
            )
            text.tag_configure(
                "tutorial_heading",
                font=("Microsoft YaHei UI", 11, "bold"),
                foreground=self.colors["text"],
                spacing1=10,
                spacing3=4,
            )
            text.tag_add("tutorial_title", "1.0", "1.end")
            for line_number, line in enumerate(content.splitlines(), start=1):
                stripped = line.strip()
                if stripped.startswith(
                    ("一、", "二、", "三、", "四、", "五、", "六、", "七、", "八、", "九、")
                ) or (stripped.endswith("？") and not stripped[:1].isdigit()):
                    text.tag_add(
                        "tutorial_heading",
                        f"{line_number}.0",
                        f"{line_number}.end",
                    )
            text.configure(state=DISABLED)

        add_page("5步上手", QUICK_START_TEXT)
        add_page("按功能使用", FEATURE_GUIDE_TEXT)
        add_page("常见问题", FAQ_TEXT)
        add_page("技术说明", TUTORIAL_TEXT)

    def _refresh_saves(self) -> None:
        paths = discover_save_paths()
        choices: dict[str, Path] = {}
        for path in paths:
            try:
                account_id = path.parents[1].name
            except IndexError:
                continue
            choices[f"Steam ID：{account_id}"] = path
        self.save_choices = choices
        labels = list(choices)
        self.save_combo.configure(values=labels)
        if hasattr(self, "local_save_combo"):
            self.local_save_combo.configure(values=labels)
        if labels:
            self.save_account.set(labels[0])
            self.status.set(f"已自动找到 {len(labels)} 个存档账户")
        else:
            self.save_account.set("未找到《仁王3》存档")
            self.status.set("未找到存档；请先在游戏中建立存档后重新搜索")
        self._refresh_backups()

    def _selected_save_path(self) -> Path:
        path = self.save_choices.get(self.save_account.get())
        if path is None or not path.is_file():
            raise FileNotFoundError("未找到可用的《仁王3》存档，请点击“重新搜索”")
        return path

    def _refresh_effect_catalog_values(self) -> None:
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            return
        playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
        if playthrough is None or rarity not in PRODUCT_RARITIES:
            return
        catalog = searchable_scroll_effect_definitions(playthrough, rarity)
        available_ids = {effect.effect_id for effect in catalog}
        self.search_effect_catalog = catalog
        self.effect_catalog_summary.set(
            f"当前上下文逐项可生成词条：{len(catalog)} 项；"
            "组合是否合法将在下方预检"
        )
        self.search_effect_by_id = {effect.effect_id: effect for effect in catalog}
        self.selected_effect_ids = [
            effect_id
            for effect_id in self.selected_effect_ids
            if effect_id in available_ids
        ]
        self.selected_effect_min_rolls = {
            effect_id: minimum
            for effect_id, minimum in self.selected_effect_min_rolls.items()
            if effect_id in available_ids
        }
        self._sync_effect_roles()
        self._filter_effect_catalog()
        self._render_selected_effects()
        self._refresh_combination_status()

    def _on_rarity_changed(self, *_: object) -> None:
        self._refresh_effect_catalog_values()
        self._refresh_special_filter_values()
        self._update_grace_search_hint()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _on_playthrough_changed(self, *_: object) -> None:
        self._refresh_effect_catalog_values()
        self._refresh_special_filter_values()
        self._update_grace_search_hint()
        self._refresh_combination_status()
        self._mark_intersection_stale()
        self._update_calculation_controls()

    def _mark_intersection_stale(self, *_: object) -> None:
        if hasattr(self, "intersection_summary"):
            self.intersection_summary.set(
                "交集数量：条件已变化；计算后按实际优化筛选顺序统计累计交集。"
            )

    @staticmethod
    def _deduplicate_names(names: list[str]) -> str:
        unique = list(dict.fromkeys(names))
        return " / ".join(unique)

    def _intersection_stage_label(self, stage: IntersectionStageCount) -> str:
        if stage.kind in {"grace", "primary", "secondary"}:
            names = [
                f"{effect_name(value)} [0x{value:04X}]"
                for value in stage.values
            ]
            role = {
                "grace": "恩宠",
                "primary": "主词条",
                "secondary": "副词条",
            }[stage.kind]
            return f"{role} {self._deduplicate_names(names)}"
        if stage.kind == "value":
            effect_id, minimum_roll = stage.values
            return (
                f"数值 {effect_name(effect_id)} [0x{effect_id:04X}] "
                f"抽取百分位≥{minimum_roll}"
            )
        if stage.kind == "terrain":
            names = [
                f"{self.auxiliary_names.terrain_effect_name(value)} [0x{value:04X}]"
                for value in stage.values
            ]
            return f"地形 {self._deduplicate_names(names)}"
        if stage.kind == "terrain_row":
            rows = "/".join(str(value) for value in stage.values)
            return f"地形原生行 {rows}"
        if stage.kind == "rule":
            names = [
                f"{self.auxiliary_names.special_rule_name(value)} [0x{value:04X}]"
                for value in stage.values
            ]
            return f"特殊规则 {self._deduplicate_names(names)}"
        if stage.kind == "enemy":
            names = [
                f"{self.auxiliary_names.enemy_name(value)} [0x{value:08X}]"
                for value in stage.values
            ]
            return f"敌人 {self._deduplicate_names(names)}"
        return stage.kind

    def _format_intersection_report(
        self,
        report: EffectSeedIntersectionReport,
    ) -> str:
        if report.is_global_total:
            scope = "完整 Seed 空间精确总数"
        else:
            scope = (
                "当前已检查范围"
                f"（数学游标 {report.start_after_trial:,} → "
                f"{report.inspected_through_trial:,} / {report.family_size:,}）"
            )
        parts = [
            f"{self._intersection_stage_label(stage)}：{stage.count:,}"
            for stage in report.stages
        ]
        if not parts:
            parts.append(f"固定前像：{report.fixed_seed_count:,}")
        return "交集数量｜" + scope + "：\n" + "  →  ".join(parts)

    def _update_calculation_controls(self, *_: object) -> None:
        if not hasattr(self, "find_button"):
            return
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            rarity = -1
        playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
        has_special = self.grace_filter.get() != NO_GRACE_FILTER_LABEL
        grace_mode = has_special and (
            (rarity == 4 and playthrough == 3)
            or (rarity == 5 and playthrough in (3, 4, 5))
        )
        certified_ng3_mode = playthrough == 3 and rarity in (3, 4, 5)
        primary_only_mode = (
            playthrough in (1, 2)
            and rarity in (3, 4, 5)
            and bool(self.selected_primary_ids)
        )
        inverse_mode = grace_mode or primary_only_mode or certified_ng3_mode
        joint_mode = grace_mode and bool(self.selected_primary_ids)

        if certified_ng3_mode:
            self.calculation_mode_hint.set(
                "离线精确求解：CUDA 分块构造 Seed 前像并批量预筛主词条，"
                "再由 CPU 精确重放完整词条、地形、敌人与特殊规则；恩宠可指定或任意。"
            )
        elif joint_mode:
            self.calculation_mode_hint.set(
                "实验联立：先限制恩宠，再由游戏原生生成器复核主词条和多个副词条。"
            )
        elif inverse_mode:
            if primary_only_mode:
                self.calculation_mode_hint.set(
                    "精确求解：直接求逆主词条 draw 1，再原生验证多个副词条。"
                )
            else:
                self.calculation_mode_hint.set(
                    "约束求解：先求逆恩宠对应的 Seed 集合，再原生验证主词条和副词条。"
                )
        else:
            requirement = (
                "请选择至少一个主词条作为可逆约束"
                if playthrough in (1, 2)
                else "请选择至少一个词条或辅助条件"
            )
            self.calculation_mode_hint.set(
                f"{requirement}；本版本不提供无约束 Seed 遍历。"
            )
        busy = bool(self.worker and self.worker.is_alive())
        self.find_button.configure(
            text="计算候选 Seed",
            state="normal" if inverse_mode and not busy else "disabled",
        )
        self.next_button.configure(text="计算下一批候选")

    def _refresh_special_filter_values(self) -> None:
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            rarity = -1
        playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
        effects = (
            target_effects_for_rarity(
                rarity,
                include_transient_stage_one=RESEARCH_MODE,
            )
            if (
                (rarity == 4 and playthrough == 3)
                or (rarity == 5 and playthrough in (3, 4, 5))
            )
            else ()
        )
        self.special_id_by_label = {effect.label: effect.effect_id for effect in effects}
        values = (NO_GRACE_FILTER_LABEL, *self.special_id_by_label.keys())
        if hasattr(self, "grace_combo"):
            self.grace_combo.configure(values=values)
        current = self.grace_filter.get()
        if current != NO_GRACE_FILTER_LABEL and current not in self.special_id_by_label:
            self.grace_filter.set(NO_GRACE_FILTER_LABEL)

    def _parse_criteria(self) -> SearchCriteria:
        self._sync_effect_roles()
        self._sync_enemy_selection()
        primary = frozenset(self.selected_primary_ids)
        required_secondary_ids = frozenset(self.selected_secondary_ids)
        rarity, level, recommended, playthrough = self._parse_generation_fields()
        grace_label = self.grace_filter.get()
        if grace_label == NO_GRACE_FILTER_LABEL:
            grace_effect_id = None
        elif grace_label in self.special_id_by_label:
            grace_effect_id = self.special_id_by_label[grace_label]
        else:
            raise ValueError("请选择当前稀有度下有效的恩宠")
        if grace_effect_id is not None:
            if rarity not in (4, 5):
                raise ValueError("只有稀有度4和5具备可筛选的最终恩宠结果")
            if playthrough not in (3, 4, 5):
                raise ValueError("恩宠筛选仅适用于三至五周目绘卷")
            if rarity == 4 and playthrough != 3:
                raise ValueError("稀有度4恩宠 finalizer 当前只完成三周目离线认证")
            if playthrough == 3:
                mapping = load_grace_output_map(rarity=rarity)
                try:
                    first_u16_ranges_for_grace(grace_effect_id, mapping)
                except ValueError as error:
                    raise ValueError(
                        f"所选恩宠 0x{grace_effect_id:04X} 不存在于稀有度{rarity}的已测映射中"
                    ) from error
        terrain_label = self.terrain_filter.get()
        if terrain_label == NO_TERRAIN_FILTER_LABEL:
            terrain_effects = frozenset()
        elif terrain_label in self.terrain_effect_by_label:
            terrain_effects = frozenset((self.terrain_effect_by_label[terrain_label],))
        else:
            raise ValueError("请选择有效的地形影响筛选项")
        auxiliary_criteria = AuxiliarySearchCriteria(
            required_terrain_effect_keys=terrain_effects,
            required_special_rule_key_groups=tuple(
                self.rule_option_by_token[token].keys
                for token in sorted(self.selected_rule_option_ids)
            ),
            required_enemy_lookup_key_groups=tuple(
                self.enemy_key_groups[key] for key in sorted(self.selected_enemy_keys)
            ),
        )
        if self.selected_enemy_keys:
            enemy_requirements = tuple(
                EnemyKeyRequirement(
                    next(
                        label.rsplit(" [", 1)[0]
                        for option_key, label in self.enemy_options
                        if option_key == key
                    ),
                    self.enemy_key_groups[key],
                )
                for key in sorted(self.selected_enemy_keys)
            )
            feasibility = analyze_enemy_feasibility(
                enemy_requirements,
                playthrough=playthrough,
            )
            if not feasibility.possible:
                explanation = "；".join(feasibility.reasons)
                raise ValueError(f"所选敌人组合在原生生成结构中无解：{explanation}")
        validate_effect_request_feasibility(
            EffectSeedRequest(
                playthrough=playthrough,
                rarity=rarity,
                primary_effect_ids=primary,
                required_secondary_ids=required_secondary_ids,
                grace_effect_id=grace_effect_id,
                auxiliary_criteria=auxiliary_criteria,
                minimum_roll_percent_by_effect_id=tuple(
                    sorted(self.selected_effect_min_rolls.items())
                ),
            )
        )
        return (
            primary,
            required_secondary_ids,
            grace_effect_id,
            rarity,
            level,
            recommended,
            auxiliary_criteria,
            playthrough,
            tuple(sorted(self.selected_effect_min_rolls.items())),
        )

    def _update_grace_search_hint(self, *_: object) -> None:
        """Explain verified and experimental Grace modes without overclaiming."""
        if self.grace_filter.get() == NO_GRACE_FILTER_LABEL:
            playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
            if playthrough in (1, 2):
                self.grace_search_hint.set(
                    "一、二周目没有三周目恩宠槽；选择主词条后直接求逆 draw 1。"
                )
            elif playthrough == 3:
                try:
                    rarity = int(self.rarity.get(), 0)
                except ValueError:
                    rarity = -1
                if rarity == 3:
                    self.grace_search_hint.set(
                        "稀有度3固定含有画龙点睛成长槽；它不是可选恩宠，按主词条、副词条和其他条件求解。"
                    )
                elif rarity == 4:
                    self.grace_search_hint.set(
                        "稀有度4的第5槽可能保留为最终恩宠，也可能被 finalizer 替换为普通词条；"
                        "不指定恩宠时两种合法结果都会保留。"
                    )
                else:
                    self.grace_search_hint.set(
                        "稀有度5可指定恩宠，也可选择任意恩宠后仅按词条、敌人、规则和地形求解。"
                    )
            else:
                self.grace_search_hint.set(
                    "四、五周目尚未开放，仅供潜在 record type 只读研究；恩宠可指定或任意。"
                )
            return
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            self.grace_search_hint.set("已选恩宠：请先填写有效稀有度。")
            return
        if rarity in (4, 5):
            playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
            if rarity == 4 and playthrough == 3:
                self.grace_search_hint.set(
                    "三周目稀有度4：先求逆恩宠 draw-1 前像，再离线重放完整 finalizer；"
                    "只有最终第5槽仍是所选恩宠的 Seed 才会进入结果。"
                )
            elif playthrough == 3:
                self.grace_search_hint.set(
                    "三周目稀有度5：离线求逆恩宠 Seed 集合，再精确重放主词条、副词条、"
                    "地形、敌人与特殊规则；无需启动游戏或读取存档。"
                )
            else:
                self.grace_search_hint.set(
                    "四、五周目未开放研究：首次建立潜在 record type 的 draw-1 全桶表，"
                    "之后仅作离线预览；不开放写档或传播声明。"
                )
        else:
            self.grace_search_hint.set("已选恩宠：当前仅支持稀有度4或5。")

    def _parse_generation_fields(self) -> tuple[int, int, int, int | None]:
        self._ensure_supported_game_version()
        rarity = int(self.rarity.get(), 0)
        level = int(self.level.get(), 0)
        recommended = int(self.recommended.get(), 0)
        if self.playthrough.get() not in PLAYTHROUGH_BY_LABEL:
            raise ValueError("请选择有效的周目")
        playthrough = PLAYTHROUGH_BY_LABEL[self.playthrough.get()]
        if rarity not in PRODUCT_RARITIES:
            raise ValueError("当前正式入口仅提供稀有度3、4绘卷")
        if not 0 <= level <= 65535 or not 0 <= recommended <= 65535:
            raise ValueError("等级必须在 0 到 65535 之间")
        return rarity, level, recommended, playthrough

    def _ensure_supported_game_version(self) -> None:
        if self.game_compatibility.known_mismatch:
            raise ValueError(self.game_compatibility.detail)

    def _require_ready(self) -> Path:
        self._ensure_supported_game_version()
        if not self.title_ack.get():
            raise ValueError("请先确认游戏位于标题界面")
        return self._selected_save_path()

    def _confirm_title_screen_if_needed(self, action: str) -> bool:
        if self.title_ack.get():
            return True
        confirmed = messagebox.askyesno(
            f"{action}前确认",
            TITLE_SCREEN_PROMPT_TEXT,
        )
        if confirmed:
            self.title_ack.set(True)
        return confirmed

    def _search_save_path(self, criteria: SearchCriteria) -> Path | None:
        if is_game_closed_effect_context(criteria):
            return None
        if is_cached_game_closed_effect_context(criteria):
            # The save identifies the exact captured map context. The game and
            # title-screen acknowledgement are required only when that cache is
            # absent and a new native capture must be made.
            return self._selected_save_path()
        return self._require_ready()

    def _parse_search_limits(self) -> tuple[int, int]:
        result_count = int(self.result_count.get(), 0)
        if not 1 <= result_count <= 200:
            raise ValueError("候选数量必须在 1 到 200 之间")
        max_seeds = int(self.max_seeds.get(), 0)
        if not 1 <= max_seeds <= 0x100000000:
            raise ValueError("最大候选验证数必须在 1 到 4294967296 之间")
        return result_count, max_seeds

    def _find_first(self) -> None:
        try:
            criteria = self._parse_criteria()
            save_path = self._search_save_path(criteria)
            (
                primary,
                secondary,
                grace,
                rarity,
                _level,
                _recommended,
                auxiliary_criteria,
                playthrough,
                _minimum_rolls,
            ) = criteria
            if playthrough in (1, 2) and not primary:
                raise ValueError("一、二周目联立求解必须至少选择一个主词条")
            if (
                playthrough == 3
                and rarity in (3, 4, 5)
                and not primary
                and not secondary
                and grace is None
                and auxiliary_criteria.is_empty
            ):
                raise ValueError(
                    "请至少选择一个主词条、副词条、恩宠、地形、敌人或特殊规则条件"
                )
            result_count, max_seeds = self._parse_search_limits()
        except Exception as error:
            messagebox.showerror("计算条件无效", str(error))
            return
        self.candidates.clear()
        self.last_search_seed = None
        self.last_joint_trial = 0
        self.candidate_list.delete(0, END)
        self._clear_details()
        self.active_criteria = criteria
        self._start_search(save_path, max_seeds, criteria, result_count=result_count)

    def _generate_seed(self) -> None:
        try:
            seed = int(self.direct_seed.get(), 0)
            if not 0 <= seed <= 0xFFFFFFFF:
                raise ValueError("种子必须在 0 到 4294967295 之间")
            rarity, level, recommended, playthrough = self._parse_generation_fields()
            criteria: SearchCriteria = (
                frozenset(),
                frozenset(),
                None,
                rarity,
                level,
                recommended,
                AuxiliarySearchCriteria(),
                playthrough,
                (),
            )
            (
                _primary,
                _required_secondary_ids,
                _grace_effect_id,
                rarity,
                level,
                recommended,
                _auxiliary_criteria,
                playthrough,
                _minimum_rolls,
            ) = criteria
            save_path = self._search_save_path(criteria)
        except Exception as error:
            messagebox.showerror("生成参数无效", str(error))
            return
        if self.worker and self.worker.is_alive():
            return
        self.cancel_event.clear()
        self.active_streamed_count = 0
        self._set_busy(True)
        selected_playthrough = playthrough_label(playthrough)
        native_ready = self.title_ack.get()
        if is_game_closed_effect_context(criteria):
            self.status.set(
                f"正在离线精确生成种子 {seed} 的词条、地形、敌人与规则（{selected_playthrough}）……"
            )
        elif is_cached_game_closed_effect_context(criteria):
            self.status.set(
                f"正在读取{selected_playthrough}映射缓存并离线生成；缓存缺失时才连接游戏……"
            )
        else:
            self.status.set(
                f"正在使用游戏原生生成器生成种子 {seed}（{selected_playthrough}）……"
            )

        def work() -> None:
            try:
                if is_game_closed_effect_context(criteria):
                    sequence = generate_ng3_certified_effect_sequence(
                        seed,
                        rarity=rarity,
                        level=level,
                    )
                    auxiliary = generate_complete_auxiliary(seed, 3)
                    candidate = ScrollCandidate.from_effect_sequence(
                        sequence,
                        auxiliary=auxiliary,
                    )
                    self.events.put(("generate_complete", candidate))
                    return
                if save_path is None:
                    raise RuntimeError("native generation requires a selected save")
                project_root = application_root()
                crypto = SaveCrypto(default_crypto_tool(project_root))
                state_root = Path(os.environ.get("LOCALAPPDATA", project_root)) / "Nioh3ScrollGenerator"
                installer = SaveInstaller(save_path=save_path, crypto=crypto, state_root=state_root)
                inventory = installer.capture_inventory()
                save_fingerprint = hashlib.sha256(inventory.decrypted).hexdigest()
                if is_cached_game_closed_effect_context(criteria):
                    grace_key = (save_fingerprint, playthrough, rarity)
                    grace_mapping = self.grace_map_cache.get(grace_key)
                    cache_path = grace_map_cache_path(
                        state_root,
                        save_fingerprint=save_fingerprint,
                        playthrough=playthrough,
                        rarity=rarity,
                    )
                    if grace_mapping is None and cache_path.is_file():
                        grace_mapping = load_grace_map_cache(
                            cache_path,
                            expected_context_fingerprint=save_fingerprint,
                        )
                    if grace_mapping is not None:
                        self.grace_map_cache[grace_key] = grace_mapping
                        sequence = generate_rarity5_grace_effect_sequence(
                            seed,
                            playthrough=playthrough,
                            level=level,
                            grace_mapping=grace_mapping,
                        )
                        auxiliary = generate_complete_auxiliary(seed, playthrough)
                        candidate = ScrollCandidate.from_effect_sequence(
                            sequence,
                            auxiliary=auxiliary,
                        )
                        self.events.put(("generate_complete", candidate))
                        return
                    if not native_ready:
                        raise ValueError(
                            f"尚无{selected_playthrough}的完整恩宠映射缓存；"
                            "请启动游戏并回到标题界面，勾选确认后先执行一次求解以建立缓存"
                        )
                template = inventory.template_record_for_playthrough(playthrough)
                with NativeBatchOracle(max_batch_size=1) as oracle:
                    source = build_source_record(
                        template,
                        seed=seed,
                        rarity=rarity,
                        level=level,
                        recommended_level=recommended,
                    )
                    record = oracle.generate([source])[0]
                candidate = ScrollCandidate.from_record(
                    record,
                    playthrough=playthrough,
                    record_stage=(
                        CandidateRecordStage.FINAL_RECORD
                        if rarity == 5
                        else CandidateRecordStage.NATIVE_STAGE_ONE
                    ),
                )
                if not candidate_has_expected_effect_count(candidate, rarity):
                    raise RuntimeError("游戏原生生成结果的词条数量与稀有度不一致")
                candidate = self._attach_auxiliary(candidate, playthrough)
                self.events.put(("generate_complete", candidate))
            except Exception:
                self.events.put(("error", traceback.format_exc()))

        self.worker = threading.Thread(target=work, daemon=True)
        self.worker.start()

    def _find_next(self) -> None:
        if (
            self.active_criteria is None
            or (self.last_search_seed is None and self.last_joint_trial == 0)
        ):
            return
        try:
            current = self._parse_criteria()
            save_path = self._search_save_path(current)
            if current != self.active_criteria:
                raise ValueError("计算条件已经改变，请点击主计算按钮重新开始")
            (
                primary,
                _secondary,
                grace,
                _rarity,
                _level,
                _recommended,
                _auxiliary_criteria,
                playthrough,
                _minimum_rolls,
            ) = current
            if playthrough in (1, 2) and not primary:
                raise ValueError("一、二周目联立求解必须至少选择一个主词条")
            result_count, max_seeds = self._parse_search_limits()
        except Exception as error:
            messagebox.showerror("计算条件无效", str(error))
            return
        grace_effect_id = current[2]
        exact_constraint_mode = is_game_closed_effect_context(current) or (
            grace_effect_id is not None and current[3] == 5 and bool(current[0])
        ) or (current[7] in (1, 2) and bool(current[0]))
        if exact_constraint_mode:
            self._start_search(
                save_path,
                max_seeds=max_seeds,
                criteria=current,
                result_count=result_count,
                joint_start_after_trial=self.last_joint_trial,
            )
        else:
            # All Grace-constrained searches now use a mathematical seed
            # iterator (R3 borrows R4 only as an experimental prediction).
            # Resume from the exact opaque seed cursor, never seed + step.
            self._start_search(
                save_path,
                max_seeds=max_seeds,
                criteria=current,
                result_count=result_count,
                grace_start_after_seed=self.last_search_seed,
            )

    def _start_search(
        self,
        save_path: Path | None,
        max_seeds: int,
        criteria: SearchCriteria,
        result_count: int,
        grace_start_after_seed: int | None = None,
        joint_start_after_trial: int = 0,
    ) -> None:
        if self.worker and self.worker.is_alive():
            return
        self.cancel_event.clear()
        self.active_streamed_count = 0
        self._set_busy(True)
        selected_playthrough = playthrough_label(criteria[7])
        grace_effect_id = criteria[2]
        rarity = criteria[3]
        target_playthrough = criteria[7]
        native_ready = self.title_ack.get()
        primary_only_mode = (
            target_playthrough in (1, 2)
            and grace_effect_id is None
            and bool(criteria[0])
        )
        joint_mode = (
            grace_effect_id is not None and rarity in (4, 5) and bool(criteria[0])
        ) or primary_only_mode
        if is_game_closed_effect_context(criteria) or is_cached_game_closed_effect_context(
            criteria
        ):
            self.intersection_summary.set(
                "交集数量：按实际优化筛选顺序统计；后一个数量是与前面全部条件的累计交集。"
            )
            if joint_start_after_trial > 0:
                self.status.set(
                    "正在从上一离线游标继续求下一批完整 Seed"
                    + ("（CUDA）……" if cuda_seed_acceleration_available() else "（CPU）……")
                )
            else:
                self.status.set(
                    "正在离线联合过滤词条、地形、敌人与特殊规则"
                    + ("（CUDA）……" if cuda_seed_acceleration_available() else "（CPU）……")
                )
        elif primary_only_mode:
            if joint_start_after_trial > 0:
                self.status.set(f"{selected_playthrough}：正在继续求解其他主词条 Seed……")
            else:
                self.status.set(f"{selected_playthrough}：正在求逆主词条 draw 1……")
        elif rarity == 3:
            if grace_start_after_seed is None:
                self.status.set(
                    "实验加速：按R4映射数学限定生成阶段结果码，再筛稀有度3“未完成的杰作”……"
                )
            else:
                self.status.set("实验加速：正从上一R3画龙点睛候选继续计算 Seed……")
        elif rarity == 4:
            if grace_start_after_seed is None:
                self.status.set("稀有度4：正在按R4 first-u16映射数学限定非最终结果码……")
            else:
                self.status.set("稀有度4：正从上一完成态候选继续计算 Seed……")
        else:
            if joint_mode and joint_start_after_trial > 0:
                self.status.set("正在从上一计算游标继续求下一组 Seed……")
            elif joint_mode:
                self.status.set("正在联立计算恩宠 draw 1 与主词条 draw 2 的 Seed 交集……")
            elif grace_start_after_seed is None:
                self.status.set("正在数学计算符合恩宠条件的 Seed……")
            else:
                self.status.set("正在继续计算下一组恩宠 Seed……")

        def work() -> None:
            try:
                (
                    primary,
                    required_secondary_ids,
                    grace_effect_id,
                    rarity,
                    level,
                    recommended,
                    auxiliary_criteria,
                    playthrough,
                    minimum_rolls,
                ) = criteria
                project_root = application_root()
                state_root = (
                    Path(os.environ.get("LOCALAPPDATA", project_root))
                    / "Nioh3ScrollGenerator"
                )
                installer = None
                inventory = None
                save_fingerprint = None
                offline_grace_mapping = None
                if is_game_closed_effect_context(criteria) and rarity in (4, 5):
                    offline_grace_mapping = load_grace_output_map(rarity=rarity)
                elif is_cached_game_closed_effect_context(criteria):
                    if save_path is None:
                        raise RuntimeError("cached offline solving requires a selected save")
                    crypto = SaveCrypto(default_crypto_tool(project_root))
                    installer = SaveInstaller(
                        save_path=save_path,
                        crypto=crypto,
                        state_root=state_root,
                    )
                    inventory = installer.capture_inventory()
                    save_fingerprint = hashlib.sha256(inventory.decrypted).hexdigest()
                    grace_key = (save_fingerprint, playthrough, rarity)
                    offline_grace_mapping = self.grace_map_cache.get(grace_key)
                    cache_path = grace_map_cache_path(
                        state_root,
                        save_fingerprint=save_fingerprint,
                        playthrough=playthrough,
                        rarity=rarity,
                    )
                    if offline_grace_mapping is None and cache_path.is_file():
                        offline_grace_mapping = load_grace_map_cache(
                            cache_path,
                            expected_context_fingerprint=save_fingerprint,
                        )
                    if offline_grace_mapping is not None:
                        self.grace_map_cache[grace_key] = offline_grace_mapping

                if is_game_closed_effect_context(criteria):
                    request = EffectSeedRequest(
                        playthrough=playthrough,
                        rarity=rarity,
                        primary_effect_ids=primary,
                        required_secondary_ids=required_secondary_ids,
                        grace_effect_id=grace_effect_id,
                        auxiliary_criteria=auxiliary_criteria,
                        minimum_roll_percent_by_effect_id=minimum_rolls,
                    )

                    def intersection_progress(
                        update: EffectSeedIntersectionReport,
                    ) -> None:
                        self.events.put(("intersection_progress", update))

                    result = collect_offline_ng3_search_batch(
                        request,
                        grace_mapping=offline_grace_mapping,
                        level=level,
                        result_count=result_count,
                        max_trials_per_batch=max_seeds,
                        start_after_trial=joint_start_after_trial,
                        intersection_progress=intersection_progress,
                        candidate_found=lambda candidate: self.events.put(
                            ("candidate_found", candidate)
                        ),
                        cancelled=self.cancel_event.is_set,
                    )
                    self.events.put(("search_complete", result))
                    return
                if offline_grace_mapping is not None:
                    request = EffectSeedRequest(
                        playthrough=playthrough,
                        rarity=5,
                        primary_effect_ids=primary,
                        required_secondary_ids=required_secondary_ids,
                        grace_effect_id=grace_effect_id,
                        auxiliary_criteria=auxiliary_criteria,
                        minimum_roll_percent_by_effect_id=minimum_rolls,
                    )

                    def intersection_progress(
                        update: EffectSeedIntersectionReport,
                    ) -> None:
                        self.events.put(("intersection_progress", update))

                    result = collect_offline_rarity5_search_batch(
                        request,
                        grace_mapping=offline_grace_mapping,
                        level=level,
                        result_count=result_count,
                        max_trials_per_batch=max_seeds,
                        start_after_trial=joint_start_after_trial,
                        intersection_progress=intersection_progress,
                        candidate_found=lambda candidate: self.events.put(
                            ("candidate_found", candidate)
                        ),
                        cancelled=self.cancel_event.is_set,
                    )
                    self.events.put(("search_complete", result))
                    return
                if is_cached_game_closed_effect_context(criteria) and not native_ready:
                    raise ValueError(
                        f"尚无{selected_playthrough}的完整恩宠映射缓存；"
                        "请启动游戏并回到标题界面，勾选确认后执行一次求解以建立缓存"
                    )
                if save_path is None:
                    raise RuntimeError("native solving requires a selected save")
                if installer is None or inventory is None:
                    crypto = SaveCrypto(default_crypto_tool(project_root))
                    installer = SaveInstaller(
                        save_path=save_path,
                        crypto=crypto,
                        state_root=state_root,
                    )
                    inventory = installer.capture_inventory()
                if save_fingerprint is None:
                    save_fingerprint = hashlib.sha256(inventory.decrypted).hexdigest()
                template = inventory.template_record_for_playthrough(playthrough)

                def progress(update: ScanProgress) -> None:
                    self.events.put(("progress", update))

                with NativeBatchOracle(max_batch_size=2048) as oracle:
                    primary_output_map = None
                    primary_first_output_map = None
                    grace_output_map = None
                    if rarity == 5 and playthrough in (3, 4, 5):
                        if playthrough in (4, 5):
                            grace_key = (save_fingerprint, playthrough, rarity)
                            grace_output_map = self.grace_map_cache.get(grace_key)
                            grace_cache_path = grace_map_cache_path(
                                state_root,
                                save_fingerprint=save_fingerprint,
                                playthrough=playthrough,
                                rarity=rarity,
                            )
                            if grace_output_map is None and grace_cache_path.is_file():
                                grace_output_map = load_grace_map_cache(
                                    grace_cache_path,
                                    expected_context_fingerprint=save_fingerprint,
                                )
                            if grace_output_map is None:

                                def grace_map_progress(update: GraceMapProgress) -> None:
                                    self.events.put(("grace_map_progress", update))

                                grace_output_map = build_live_grace_output_map(
                                    oracle,
                                    template=template,
                                    category=playthrough,
                                    rarity=rarity,
                                    level=level,
                                    recommended_level=recommended,
                                    cancel_event=self.cancel_event,
                                    progress=grace_map_progress,
                                )
                                save_grace_map_cache(
                                    grace_cache_path,
                                    grace_output_map,
                                    context_fingerprint=save_fingerprint,
                                )
                            self.grace_map_cache[grace_key] = grace_output_map
                            if grace_effect_id is not None:
                                try:
                                    first_u16_ranges_for_grace(
                                        grace_effect_id, grace_output_map
                                    )
                                except ValueError as error:
                                    raise ValueError(
                                        f"所选恩宠 0x{grace_effect_id:04X} "
                                        f"不在{playthrough}周目原生结果池中"
                                    ) from error
                        else:
                            map_rarity = 4 if rarity == 3 else rarity
                            grace_output_map = load_grace_output_map(rarity=map_rarity)
                    if (
                        playthrough in (4, 5)
                        and rarity == 5
                    ):
                        if grace_output_map is None:
                            raise RuntimeError("native Grace-map capture returned no mapping")
                        request = EffectSeedRequest(
                            playthrough=playthrough,
                            rarity=5,
                            primary_effect_ids=primary,
                            required_secondary_ids=required_secondary_ids,
                            grace_effect_id=grace_effect_id,
                            auxiliary_criteria=auxiliary_criteria,
                            minimum_roll_percent_by_effect_id=minimum_rolls,
                        )

                        def captured_intersection_progress(
                            update: EffectSeedIntersectionReport,
                        ) -> None:
                            self.events.put(("intersection_progress", update))

                        # The native process is needed only to capture the map.
                        # Release its handle and remote scratch allocation before
                        # the potentially long game-closed solver runs.
                        oracle.close()
                        result = collect_offline_rarity5_search_batch(
                            request,
                            grace_mapping=grace_output_map,
                            level=level,
                            result_count=result_count,
                            max_trials_per_batch=max_seeds,
                            start_after_trial=joint_start_after_trial,
                            intersection_progress=captured_intersection_progress,
                            candidate_found=lambda candidate: self.events.put(
                                ("candidate_found", candidate)
                            ),
                            cancelled=self.cancel_event.is_set,
                        )
                        self.events.put(("search_complete", result))
                        return
                    if grace_effect_id is not None and rarity == 5 and primary:
                        map_key = (save_fingerprint, playthrough, grace_effect_id, rarity)
                        primary_output_map = self.primary_map_cache.get(map_key)
                        if primary_output_map is None:
                            cache_path = primary_map_cache_path(
                                state_root,
                                save_fingerprint=save_fingerprint,
                                playthrough=playthrough,
                                rarity=rarity,
                                grace_effect_id=grace_effect_id,
                            )
                            if cache_path.is_file():
                                cached = load_primary_map(
                                    cache_path,
                                    expected_context_fingerprint=save_fingerprint,
                                )
                                if not isinstance(cached, PrimaryOutputMap):
                                    raise ValueError("磁盘缓存不是恩宠条件下的主词条 draw-2 映射")
                                primary_output_map = cached
                        if primary_output_map is None:
                            assert grace_output_map is not None

                            def map_progress(update: PrimaryMapProgress) -> None:
                                self.events.put(("map_progress", update))

                            primary_output_map = build_primary_output_map(
                                oracle,
                                template=template,
                                grace_effect_id=grace_effect_id,
                                mapping=grace_output_map,
                                rarity=rarity,
                                level=level,
                                recommended_level=recommended,
                                cancel_event=self.cancel_event,
                                progress=map_progress,
                            )
                            save_primary_map(
                                cache_path,
                                primary_output_map,
                                context_fingerprint=save_fingerprint,
                            )
                        self.primary_map_cache[map_key] = primary_output_map
                    elif playthrough in (1, 2) and primary:
                        map_key = (save_fingerprint, playthrough, 0, rarity)
                        primary_first_output_map = self.primary_map_cache.get(map_key)
                        if primary_first_output_map is None:
                            cache_path = primary_map_cache_path(
                                state_root,
                                save_fingerprint=save_fingerprint,
                                playthrough=playthrough,
                                rarity=rarity,
                                grace_effect_id=None,
                            )
                            if cache_path.is_file():
                                cached = load_primary_map(
                                    cache_path,
                                    expected_context_fingerprint=save_fingerprint,
                                )
                                if not isinstance(cached, PrimaryFirstDrawOutputMap):
                                    raise ValueError("磁盘缓存不是主词条 draw-1 映射")
                                primary_first_output_map = cached
                        if primary_first_output_map is None:

                            def map_progress(update: PrimaryMapProgress) -> None:
                                self.events.put(("map_progress", update))

                            primary_first_output_map = build_primary_first_draw_output_map(
                                oracle,
                                template=template,
                                category=playthrough,
                                rarity=rarity,
                                level=level,
                                recommended_level=recommended,
                                cancel_event=self.cancel_event,
                                progress=map_progress,
                            )
                            save_primary_map(
                                cache_path,
                                primary_first_output_map,
                                context_fingerprint=save_fingerprint,
                            )
                        self.primary_map_cache[map_key] = primary_first_output_map
                    scan_kwargs: dict[str, object] = {
                        "oracle": oracle,
                        "template": template,
                        "start_seed": 0,
                        "primary_effect_ids": primary,
                        "required_secondary_ids": required_secondary_ids,
                        "grace_effect_id": grace_effect_id,
                        "rarity": rarity,
                        "level": level,
                        "recommended_level": recommended,
                        "playthrough": playthrough,
                        "auxiliary_criteria": auxiliary_criteria,
                        "seed_step": 1,
                        "max_seeds": max_seeds,
                        "cancel_event": self.cancel_event,
                        "progress": progress,
                    }
                    if grace_output_map is not None:
                        scan_kwargs["grace_output_map"] = grace_output_map
                    if primary_output_map is not None:
                        scan_kwargs["primary_output_map"] = primary_output_map
                    elif primary_first_output_map is not None:
                        scan_kwargs["primary_first_output_map"] = primary_first_output_map
                    uses_primary_map = (
                        primary_output_map is not None
                        or primary_first_output_map is not None
                    )
                    active_joint_cursor = joint_start_after_trial
                    active_grace_cursor = grace_start_after_seed
                    candidates: list[ScrollCandidate] = []
                    for _ in range(result_count):
                        if uses_primary_map:
                            scan_kwargs["joint_start_after_trial"] = active_joint_cursor
                            scan_kwargs.pop("grace_start_after_seed", None)
                        elif grace_effect_id is not None:
                            scan_kwargs["grace_start_after_seed"] = active_grace_cursor
                            scan_kwargs.pop("joint_start_after_trial", None)
                        candidate = scan_next_candidate(**scan_kwargs)
                        if candidate is None:
                            break
                        candidate = replace(candidate, playthrough=playthrough)
                        candidate = self._attach_auxiliary(candidate, playthrough)
                        candidates.append(candidate)
                        self.events.put(("candidate_found", candidate))
                        if uses_primary_map:
                            if (
                                candidate.joint_search_trial is None
                                or candidate.joint_search_trial <= active_joint_cursor
                            ):
                                raise RuntimeError("数学候选游标没有前进")
                            active_joint_cursor = candidate.joint_search_trial
                        else:
                            active_grace_cursor = candidate.seed
                self.events.put(
                    (
                        "search_complete",
                        SearchBatchResult(
                            tuple(candidates),
                            result_count,
                            streamed=True,
                        ),
                    )
                )
            except Exception:
                self.events.put(("error", traceback.format_exc()))

        self.worker = threading.Thread(target=work, daemon=True)
        self.worker.start()

    def _cancel(self) -> None:
        self.cancel_event.set()
        self.status.set("已请求取消，正在等待当前批次结束……")

    def _set_busy(self, busy: bool) -> None:
        try:
            rarity = int(self.rarity.get(), 0)
        except ValueError:
            rarity = -1
        playthrough = PLAYTHROUGH_BY_LABEL.get(self.playthrough.get())
        has_ng3_rarity34_constraint = bool(
            self.selected_primary_ids
            or self.selected_secondary_ids
            or self.selected_rule_option_ids
            or self.selected_enemy_keys
            or self.terrain_filter.get() != NO_TERRAIN_FILTER_LABEL
        )
        can_solve = rarity in (3, 4, 5) and (
            (
                playthrough == 3
                and (
                    self.grace_filter.get() != NO_GRACE_FILTER_LABEL
                    or has_ng3_rarity34_constraint
                )
            )
            or (
                playthrough in (4, 5)
                and rarity == 5
            )
            or (playthrough in (1, 2) and bool(self.selected_primary_ids))
        )
        self.find_button.configure(
            state="disabled" if busy or not can_solve else "normal"
        )
        self.generate_button.configure(state="disabled" if busy else "normal")
        self.next_button.configure(
            state=(
                "disabled"
                if (
                    busy
                    or self.active_criteria is None
                    or (
                        self.last_search_seed is None
                        and self.last_joint_trial == 0
                    )
                )
                else "normal"
            )
        )
        self.cancel_button.configure(state="normal" if busy else "disabled")
        self.install_button.configure(
            state="disabled" if busy or not self.candidate_list.curselection() else "normal"
        )

    def _poll_events(self) -> None:
        try:
            while True:
                event, payload = self.events.get_nowait()
                if event == "progress":
                    update = payload
                    assert isinstance(update, ScanProgress)
                    self.status.set(
                        f"已原生验证 {update.scanned:,} 个候选；当前 Seed {update.current_seed}"
                    )
                elif event == "map_progress":
                    update = payload
                    assert isinstance(update, PrimaryMapProgress)
                    self.status.set(
                        "正在建立所选周目的主词条抽取映射："
                        f"{update.mapped_buckets:,} / {update.total_buckets:,}"
                    )
                elif event == "grace_map_progress":
                    update = payload
                    assert isinstance(update, GraceMapProgress)
                    self.status.set(
                        "实验模式：正在建立所选四/五周目类型的恩宠映射："
                        f"{update.mapped_buckets:,} / {update.total_buckets:,}"
                    )
                elif event == "intersection_progress":
                    update = payload
                    assert isinstance(update, EffectSeedIntersectionReport)
                    self.intersection_summary.set(
                        self._format_intersection_report(update)
                    )
                elif event == "candidate_found":
                    candidate = payload
                    assert isinstance(candidate, ScrollCandidate)
                    self._append_candidate(candidate)
                    self.active_streamed_count += 1
                    self.install_button.configure(state="disabled")
                    backend = last_seed_acceleration_backend()
                    backend_text = "CUDA" if backend == "cuda" else "CPU"
                    self.status.set(
                        f"已实时找到 {self.active_streamed_count} 个匹配 Seed；"
                        f"{backend_text} 预筛后正在继续精确验证……"
                    )
                elif event == "search_complete":
                    self._set_busy(False)
                    assert isinstance(payload, SearchBatchResult)
                    if payload.intersection_report is not None:
                        self.intersection_summary.set(
                            self._format_intersection_report(
                                payload.intersection_report
                            )
                        )
                    if payload.next_start_after_trial is not None:
                        self.last_joint_trial = payload.next_start_after_trial
                    if not payload.candidates:
                        if payload.next_start_after_trial is not None:
                            self.next_button.configure(state="normal")
                            self.status.set(
                                "本次候选预算内没有匹配 Seed；可从当前数学游标继续下一批。"
                            )
                        else:
                            self.status.set("本次计算/验证预算内没有匹配 Seed，或者任务已取消。")
                    else:
                        if not payload.streamed:
                            for candidate in payload.candidates:
                                self._append_candidate(candidate)
                        last_candidate = payload.candidates[-1]
                        self.last_search_seed = last_candidate.seed
                        if payload.next_start_after_trial is None:
                            self.last_joint_trial = last_candidate.joint_search_trial or 0
                        self.next_button.configure(state="normal")
                        backend = last_seed_acceleration_backend()
                        backend_text = {
                            "cuda": "CUDA",
                            "native_cpu": "原生 CPU",
                            "python": "Python CPU",
                        }.get(backend, "CPU")
                        self.status.set(
                            f"本批已找到 {len(payload.candidates)} / {payload.requested_count} 个匹配 Seed；"
                            f"预筛后端：{backend_text}。可逐张比较，或继续计算下一批。"
                        )
                elif event == "generate_complete":
                    self._set_busy(False)
                    candidate = payload
                    if candidate is None:
                        self.status.set("该种子不符合当前词条条件，或原生生成结果不完整。")
                    else:
                        assert isinstance(candidate, ScrollCandidate)
                        self._append_candidate(candidate, validates_combination=False)
                        self.status.set(f"已直接生成种子 {candidate.seed}。")
                elif event == "install_complete":
                    self._set_busy(False)
                    result = payload
                    self._refresh_backups()
                    self.status.set(
                        f"已写入槽位 {result.slot_index}；备份已创建"
                    )
                    messagebox.showinfo(
                        "写入完成",
                        f"新绘卷已添加到槽位 {result.slot_index}。\n\n"
                        "备份和安装报告已保存到程序数据目录。",
                    )
                elif event == "update_check_complete":
                    self.update_button.configure(state="normal")
                    result, manual = payload
                    assert isinstance(result, UpdateCheckResult)
                    if result.update_available:
                        notes = result.manifest.notes.strip() or "未提供更新说明。"
                        if messagebox.askyesno(
                            "发现新版本",
                            f"当前版本：{result.current_version}\n"
                            f"最新版本：{result.manifest.version}\n\n"
                            f"{notes}\n\n下载并验证该更新吗？",
                        ):
                            self._download_available_update(result)
                        else:
                            self.status.set("已跳过本次更新。")
                    else:
                        self.status.set(f"当前已是最新版本 {result.current_version}。")
                        if manual:
                            messagebox.showinfo("没有可用更新", "当前已是最新版本。")
                elif event == "update_download_complete":
                    assert isinstance(payload, DownloadedUpdate)
                    self._offer_downloaded_update_when_idle(payload)
                elif event == "update_error":
                    self.update_button.configure(state="normal")
                    details, manual = payload
                    final_line = (
                        str(details).strip().splitlines()[-1]
                        if str(details).strip()
                        else "未知错误"
                    )
                    self.status.set("更新检查或下载失败；现有版本未改变。")
                    if manual:
                        messagebox.showerror("更新失败", final_line)
                elif event == "error":
                    self._set_busy(False)
                    details = str(payload)
                    self.status.set("操作失败；存档未被修改。")
                    messagebox.showerror(
                        "操作失败",
                        user_facing_error_message(details),
                    )
        except queue.Empty:
            pass
        self.root.after(100, self._poll_events)

    def _show_selected_candidate(self, _event: object | None = None) -> None:
        selection = self.candidate_list.curselection()
        if not selection:
            self.install_button.configure(state="disabled")
            return
        self._show_candidate(self.candidates[selection[0]])
        if not (self.worker and self.worker.is_alive()):
            candidate = self.candidates[selection[0]]
            self.install_button.configure(
                state="disabled" if candidate.install_blocker else "normal"
            )

    def _delete_selected_candidates(self) -> None:
        selected = tuple(self.candidate_list.curselection())
        if not selected:
            return
        for index in reversed(selected):
            del self.candidates[index]
            self.candidate_list.delete(index)
        if not self.candidates:
            self._clear_details()
            self.install_button.configure(state="disabled")
            self._refresh_combination_status()
            return
        next_index = min(selected[0], len(self.candidates) - 1)
        self.candidate_list.selection_set(next_index)
        self.candidate_list.see(next_index)
        self._show_selected_candidate()

    def _compare_selected_candidates(self) -> None:
        selected = tuple(self.candidate_list.curselection())
        if len(selected) < 2:
            messagebox.showinfo("比较候选", "请在候选列表中至少选择两张绘卷。")
            return
        window = Toplevel(self.root)
        window.title("候选绘卷对比")
        window.geometry("1180x520")
        window.transient(self.root)
        columns = (
            "seed",
            "primary",
            "secondaries",
            "grace",
            "terrain",
            "rules",
            "enemies",
        )
        table = ttk.Treeview(window, columns=columns, show="headings")
        headings = {
            "seed": "Seed",
            "primary": "主词条（数值/百分位）",
            "secondaries": "副词条",
            "grace": "恩宠",
            "terrain": "地形",
            "rules": "特殊规则",
            "enemies": "敌人",
        }
        widths = {
            "seed": 100,
            "primary": 210,
            "secondaries": 300,
            "grace": 150,
            "terrain": 100,
            "rules": 260,
            "enemies": 260,
        }
        for column in columns:
            table.heading(column, text=headings[column])
            table.column(column, width=widths[column], anchor="w")
        for index in selected:
            candidate = self.candidates[index]
            auxiliary = candidate.auxiliary
            terrain = ""
            rules = ""
            enemies = ""
            if auxiliary is not None:
                terrain = "/".join(
                    self.auxiliary_names.terrain_effect_name(key)
                    for key in auxiliary.terrain.display_effect_keys
                )
                rules = " / ".join(
                    (
                        f"{self.auxiliary_names.special_rule_name(entry.key)} "
                        f"{format_special_rule_value(entry)}"
                    ).strip()
                    for entry in auxiliary.special_rules.entries
                )
                enemies = " / ".join(
                    self.auxiliary_names.enemy_name(entry.lookup_key)
                    for group in auxiliary.enemies.groups
                    for entry in group.entries
                )
            table.insert(
                "",
                END,
                values=(
                    candidate.seed,
                    (
                        f"{candidate.display_name(candidate.primary)} "
                        f"({candidate.primary.value}/{candidate.primary.roll_percent})"
                    ),
                    " / ".join(
                        f"{candidate.display_name(effect)} "
                        f"({effect.value}/{effect.roll_percent})"
                        for effect in candidate.secondaries
                    ),
                    (
                        candidate.display_name(candidate.grace)
                        if candidate.grace is not None
                        else "无"
                    ),
                    terrain,
                    rules,
                    enemies,
                ),
            )
        xscroll = ttk.Scrollbar(window, orient="horizontal", command=table.xview)
        yscroll = ttk.Scrollbar(window, orient="vertical", command=table.yview)
        table.configure(xscrollcommand=xscroll.set, yscrollcommand=yscroll.set)
        xscroll.pack(side="bottom", fill="x")
        yscroll.pack(side=RIGHT, fill="y")
        table.pack(fill=BOTH, expand=True, padx=10, pady=10)

    def _clear_candidates(self) -> None:
        self.candidates.clear()
        self.candidate_list.delete(0, END)
        self._clear_details()
        self.install_button.configure(state="disabled")
        self._refresh_combination_status()

    def _candidate_summary(self, candidate: ScrollCandidate) -> str:
        candidate_playthrough = playthrough_label(candidate.playthrough)
        primary_roll = (
            f"@{candidate.primary.roll_percent}"
            if candidate.record_stage is CandidateRecordStage.EFFECT_SEQUENCE_ONLY
            else ""
        )
        parts = [
            f"主：{candidate.display_name(candidate.primary)}{primary_roll}"
        ]
        secondary_names = [
            candidate.display_name(effect) for effect in candidate.secondaries
        ]
        if secondary_names:
            secondary_summary = "/".join(secondary_names[:2])
            if len(secondary_names) > 2:
                secondary_summary += f" 等{len(secondary_names)}项"
            parts.append(f"副：{secondary_summary}")
        if candidate.grace is not None:
            parts.append(f"恩宠：{candidate.display_name(candidate.grace)}")
        elif candidate.rarity == 4:
            parts.append("恩宠：无（已被完成器替换）")
        if candidate.auxiliary is not None:
            terrain_names = [
                self.auxiliary_names.terrain_effect_name(key)
                for key in candidate.auxiliary.terrain.display_effect_keys
            ]
            if terrain_names:
                parts.append(f"地形：{terrain_names[0]}")
            rule_names = [
                self.auxiliary_names.special_rule_name(entry.key)
                for entry in candidate.auxiliary.special_rules.entries
            ]
            if rule_names:
                rule_summary = "/".join(rule_names[:2])
                if len(rule_names) > 2:
                    rule_summary += f" 等{len(rule_names)}项"
                parts.append(f"规则：{rule_summary}")
            enemy_names = [
                self.auxiliary_names.enemy_name(entry.lookup_key)
                for group in candidate.auxiliary.enemies.groups
                for entry in group.entries
            ]
            if enemy_names:
                enemy_summary = "/".join(enemy_names[:2])
                if len(enemy_names) > 2:
                    enemy_summary += f" 等{len(enemy_names)}项"
                parts.append(f"敌人：{enemy_summary}")
        return (
            f"{candidate_playthrough} · Seed {candidate.seed} · "
            + " · ".join(parts)
        )

    def _candidate_sort_key(self, candidate: ScrollCandidate) -> tuple[object, ...]:
        mode = self.candidate_sort.get()
        if mode == "主词条数值从高到低":
            return (-candidate.primary.value, -candidate.primary.roll_percent, candidate.seed)
        if mode == "全部词条抽取百分位从高到低":
            total = sum(effect.roll_percent for effect in candidate.effects)
            return (-total, -candidate.primary.roll_percent, candidate.seed)
        if mode == "Seed 从小到大":
            return (candidate.seed,)
        return (candidate.joint_search_trial or 0, candidate.seed)

    def _resort_candidates(self) -> None:
        if not hasattr(self, "candidate_list") or not self.candidates:
            return
        selected = {
            id(self.candidates[index]) for index in self.candidate_list.curselection()
        }
        self.candidates.sort(key=self._candidate_sort_key)
        self.candidate_list.delete(0, END)
        for index, candidate in enumerate(self.candidates):
            self.candidate_list.insert(END, self._candidate_summary(candidate))
            if id(candidate) in selected:
                self.candidate_list.selection_set(index)
        if selected:
            self._show_selected_candidate()

    def _append_candidate(
        self,
        candidate: ScrollCandidate,
        *,
        validates_combination: bool = True,
    ) -> None:
        had_selection = bool(self.candidate_list.curselection())
        self.candidates.append(candidate)
        if self.candidate_sort.get() == "发现顺序":
            self.candidate_list.insert(END, self._candidate_summary(candidate))
        else:
            self._resort_candidates()
        index = len(self.candidates) - 1
        if len(self.candidates) == 1 and not had_selection:
            self.candidate_list.selection_set(index)
            self.candidate_list.see(index)
            self._show_candidate(candidate)
            self.install_button.configure(
                state="disabled" if candidate.install_blocker else "normal"
            )
        if validates_combination:
            self.combination_status.set(
                f"组合状态：已找到 {len(self.candidates)} 个经完整离线重放验证的合法 Seed。"
            )

    def _show_candidate(self, candidate: ScrollCandidate) -> None:
        self._clear_details()
        for effect in candidate.effects[:6]:
            display_name = candidate.display_name(effect)
            unresolved = effect.slot in candidate.unresolved_effect_slots
            if unresolved:
                role = "生成阶段结果码"
            elif effect.slot == 1:
                role = "主词条"
            elif candidate.rarity == 3 and effect.slot == 5:
                role = "成长/画龙点睛"
            elif candidate.grace_slot_index == effect.slot - 1:
                role = "恩宠"
            else:
                role = "副词条"
            if candidate.record_stage is CandidateRecordStage.EFFECT_SEQUENCE_ONLY:
                value_text = (
                    f"{effect.value}（抽取百分位 {effect.roll_percent}）"
                )
                metadata_text = f"0x{effect.metadata:08X}"
                prefix_text = f"0x{effect.prefix:08X}"
                tail_text = f"0x{effect.tail_0:X}, 0x{effect.tail_1:X}"
            else:
                value_text = f"{effect.value} (0x{effect.value:X})"
                metadata_text = f"{effect.metadata} (0x{effect.metadata:X})"
                prefix_text = f"0x{effect.prefix:X}"
                tail_text = f"0x{effect.tail_0:X}, 0x{effect.tail_1:X}"
            self.detail.insert(
                "",
                END,
                values=(
                    effect.slot,
                    role,
                    display_name,
                    f"0x{effect.effect_id:04X}",
                    value_text,
                    metadata_text,
                    prefix_text,
                    tail_text,
                ),
            )
        if candidate.predicted_growth_grace_id is not None:
            predicted_name = contextual_effect_name(
                candidate.predicted_growth_grace_id,
                rarity=4,
                slot=5,
                native_stage_one=True,
            )
            self.detail.insert(
                "",
                END,
                values=(
                    "—",
                    "实验预测",
                    f"画龙点睛后 → {predicted_name}",
                    f"0x{candidate.predicted_growth_grace_id:04X}",
                    "同Seed R4 第5槽原始结果码",
                    "非R3记录字段",
                    "—",
                    "待实机完成验证",
                ),
            )
        if candidate.auxiliary is not None:
            auxiliary = candidate.auxiliary
            terrain_names = [
                self.auxiliary_names.terrain_effect_name(key)
                for key in auxiliary.terrain.display_effect_keys
            ]
            self.detail.insert(
                "",
                END,
                values=(
                    "—",
                    "地形影响",
                    " / ".join(terrain_names) if terrain_names else "无",
                    (
                        "/".join(
                            f"0x{key:04X}"
                            for key in auxiliary.terrain.display_effect_keys
                        )
                        or "—"
                    ),
                    f"value 0x{auxiliary.terrain.value:02X}",
                    f"mode 0x{auxiliary.mode.value:02X}",
                    "offline parity",
                    (
                        f"class {auxiliary.mode.branch_class}; "
                        f"aux row {auxiliary.terrain.selected_row_index}"
                    ),
                ),
            )
            for group_index, group in enumerate(auxiliary.enemies.groups, start=1):
                names = [
                    self.auxiliary_names.enemy_name(entry.lookup_key)
                    for entry in group.entries
                ]
                keys = [f"0x{entry.lookup_key:08X}" for entry in group.entries]
                roles = [str(entry.role) for entry in group.entries]
                scratch = [
                    f"0x{entry.scratch_rule_key:04X}"
                    for entry in group.entries
                    if entry.scratch_rule_key != 0xFFFF
                ]
                self.detail.insert(
                    "",
                    END,
                    values=(
                        group_index,
                        "出现敌人",
                        " / ".join(names),
                        " / ".join(keys),
                        f"budget {group.source_budget:g}",
                        "roles " + "/".join(roles),
                        "offline parity",
                        "scratch " + ("/".join(scratch) if scratch else "none"),
                    ),
                )
            for rule_index, entry in enumerate(
                auxiliary.special_rules.entries,
                start=1,
            ):
                value_text = format_special_rule_value(entry)
                qualifier = (
                    f"{entry.qualifier_kind} 0x{entry.qualifier_key:X}"
                    if entry.qualifier_kind and entry.qualifier_key is not None
                    else "none"
                )
                self.detail.insert(
                    "",
                    END,
                    values=(
                        rule_index,
                        "特殊规则",
                        self.auxiliary_names.special_rule_name(entry.key),
                        f"0x{entry.key:04X}",
                        value_text or "—",
                        (
                            f"raw {entry.raw_value:g} @+0x{entry.value_source_offset:02X}"
                            if entry.raw_value is not None
                            and entry.value_source_offset is not None
                            else "—"
                        ),
                        "offline parity",
                        f"ordered; qualifier {qualifier}",
                    ),
                )
        elif candidate.auxiliary_error:
            self.detail.insert(
                "",
                END,
                values=(
                    "—",
                    "辅助生成",
                    "离线辅助结果生成失败",
                    "—",
                    "—",
                    "—",
                    "—",
                    candidate.auxiliary_error,
                ),
            )

    @staticmethod
    def _attach_auxiliary(
        candidate: ScrollCandidate,
        playthrough: int,
    ) -> ScrollCandidate:
        if candidate.auxiliary is not None:
            return candidate
        try:
            auxiliary = generate_complete_auxiliary(candidate.seed, playthrough)
        except Exception as error:
            return replace(candidate, auxiliary_error=str(error))
        return replace(candidate, auxiliary=auxiliary, auxiliary_error=None)

    def _clear_details(self) -> None:
        for item in self.detail.get_children():
            self.detail.delete(item)

    def _install_selected(self) -> None:
        selection = self.candidate_list.curselection()
        if not selection:
            return
        if not self._confirm_title_screen_if_needed("添加绘卷"):
            return
        try:
            save_path = self._require_ready()
            level = int(self.level.get(), 0)
            recommended_level = int(self.recommended.get(), 0)
            transfer_count = int(self.transfer_count.get(), 0)
            if not 0 <= level <= 0xFFFF:
                raise ValueError("等级必须在 0 到 65535 之间")
            if not 0 <= recommended_level <= 0xFFFF:
                raise ValueError("推荐等级必须在 0 到 65535 之间")
            if not 0 <= transfer_count <= 0xFFFFFFFF:
                raise ValueError("转手次数必须在 0 到 4294967295 之间")
        except Exception as error:
            messagebox.showerror("写入参数无效", str(error))
            return
        candidate = self.candidates[selection[0]]
        if candidate.install_blocker:
            messagebox.showerror("候选尚未完成最终解析", candidate.install_blocker)
            return
        candidate_playthrough = playthrough_label(candidate.playthrough)
        experimental_notice = (
            "\n\n实验提示：四、五周目尚未由 DLC2 开放，当前候选仅供研究预览，禁止写档。"
            if candidate.playthrough in (4, 5)
            else ""
        )
        if not messagebox.askyesno(
            "确认写入存档",
            f"确定把种子 {candidate.seed}（{candidate_playthrough}生成上下文）作为新绘卷添加到存档吗？\n\n"
            "程序会在写入时重新读取当前存档、绑定合法模板和新的内部序号，"
            "然后自动备份并写入下一个绘卷栏位；不会覆盖任何现有绘卷。"
            + experimental_notice,
        ):
            return
        self._set_busy(True)
        self.status.set("正在备份并准备新的绘卷记录……")

        def work() -> None:
            try:
                project_root = application_root()
                crypto = SaveCrypto(default_crypto_tool(project_root))
                state_root = self._backup_state_root()
                installer = SaveInstaller(save_path=save_path, crypto=crypto, state_root=state_root)
                if candidate.can_materialize_for_install:
                    result = installer.install_effect_sequence_candidate(
                        candidate,
                        level=level,
                        recommended_level=recommended_level,
                        transfer_count=transfer_count,
                    )
                else:
                    result = installer.install(
                        candidate.record,
                        transfer_count=transfer_count,
                    )
                self.events.put(("install_complete", result))
            except Exception:
                self.events.put(("error", traceback.format_exc()))

        self.worker = threading.Thread(target=work, daemon=True)
        self.worker.start()


def main() -> int:
    if STARTUP_TRACE:
        faulthandler.enable()
        faulthandler.dump_traceback_later(10, repeat=True)
        startup_trace("process entry")
    root = Tk()
    startup_trace("Tk root created")
    try:
        ttk.Style(root).theme_use("vista")
    except Exception:
        pass
    ScrollEditorApp(root)
    startup_trace("entering Tk main loop")
    root.mainloop()
    return 0
