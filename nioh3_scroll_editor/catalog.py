from __future__ import annotations

import json
import locale as _locale
import os
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path


@dataclass(frozen=True, slots=True)
class EffectDefinition:
    effect_id: int
    name: str

    @property
    def hex_id(self) -> str:
        return f"0x{self.effect_id:04X}"

    @property
    def label(self) -> str:
        return f"{self.name}  [{self.hex_id}]"


# IDs are decoded as little-endian uint32 values from the user's byte strings.
# Names below are limited to mappings confirmed by the user's in-game UI,
# stable CT identifiers, or provided save-record ground truth.  The former
# 0xFBEE -> 武之深奥 mapping came from comparing a transient native candidate
# with a different installed record.  In the supplied NG3 save, 0xFBEE always
# has the numeric 5.3%-5.7% shape of 防御精力消耗降低, while the final records
# use 0xDFF0 for 武之深奥 and 0x23E8 for 刚之深奥.
BETA_EFFECTS: tuple[EffectDefinition, ...] = (
    EffectDefinition(0x4647, "伤害反映（忍术威力）"),
    EffectDefinition(0xA051, "对妖战术"),
    EffectDefinition(0xA73D, "体力"),
    EffectDefinition(0x190A, "阴阳术伤害"),
    EffectDefinition(0x2B06, "咒之深奥"),
    EffectDefinition(0xB613, "心之深奥"),
    EffectDefinition(0xDFF0, "武之深奥"),
    EffectDefinition(0x583B, "负面效果持续时间缩短"),
    EffectDefinition(0xF9BE, "精髓并存（共通）"),
    EffectDefinition(0xEA74, "精髓并存（忍者）"),
    EffectDefinition(0x47BC, "合轴可继承稀有度"),
    # User in-game UI mapping, 2026-08-26. Byte strings are decoded as LE u32.
    EffectDefinition(0x2EFC, "远距离伤害"),
    EffectDefinition(0x9A3D, "强攻击精力消耗降低"),
    EffectDefinition(0x6CE3, "不消耗使役符"),
    EffectDefinition(0x6BEB, "近距离攻击精力伤害"),
    EffectDefinition(0xEA53, "坚忍度"),
    EffectDefinition(0x512D, "精髓并存武士"),
    EffectDefinition(0x7499, "对人战术"),
    EffectDefinition(0x3E7A, "精力恢复速度"),
    EffectDefinition(0xB82B, "敌人精力耗尽时赋予受到伤害增加"),
    EffectDefinition(0xD40A, "近距离攻击精力消耗降低"),
    EffectDefinition(0x6E2B, "不消耗仙药"),
    EffectDefinition(0xBC51, "精华槽增加量"),
    EffectDefinition(0x3A8E, "武技精力伤害"),
    EffectDefinition(0xCE1A, "武技伤害"),
    EffectDefinition(0x3F41, "属性攻击伤害降低"),
    EffectDefinition(0xAE5A, "技之深奥"),
    EffectDefinition(0xA0A7, "近距离攻击打倒敌人时恢复体力"),
    EffectDefinition(0xEF97, "速攻击伤害"),
    EffectDefinition(0x28D1, "九十九化身持续时间延长"),
    EffectDefinition(0x5CAC, "闪避动作精力消耗降低"),
    EffectDefinition(0x1355, "灵力增加量"),
    EffectDefinition(0xD411, "冲刺精力消耗降低"),
    EffectDefinition(0x6AAF, "智之深奥"),
    EffectDefinition(0xDAC2, "体之深奥"),
    EffectDefinition(0x23E8, "刚之深奥"),
    EffectDefinition(0xFBEE, "防御精力消耗降低"),
    EffectDefinition(0x600F, "伤害反映（阴阳术术力）"),
    EffectDefinition(0x8184, "装备品掉落率"),
    EffectDefinition(0xDB20, "近距离攻击伤害"),
)


def _normalize_locale_tag(value: str) -> str:
    value = value.strip().replace("_", "-")
    if not value:
        return "zh-CN"
    parts = value.split("-")
    language = parts[0].lower()
    if len(parts) == 1:
        return language
    return "-".join((language, parts[1].upper(), *parts[2:]))


def _preferred_effect_locale() -> str:
    explicit = os.environ.get("NIOH3_SCROLL_LOCALE", "").strip()
    if explicit:
        return _normalize_locale_tag(explicit)
    detected = _locale.getlocale()[0] or "zh-CN"
    return _normalize_locale_tag(detected)


def _load_multilingual_effect_names() -> dict[int, dict[str, str]]:
    """Load final-effect names captured from the native localization pool."""
    path = Path(__file__).parent / "data" / "effect_names_multilingual.json"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError):
        return {}
    if payload.get("schema") != "nioh3-effect-localization-catalog/v1":
        return {}
    result: dict[int, dict[str, str]] = {}
    for raw_id, item in payload.get("effects", {}).items():
        try:
            effect_id = int(str(raw_id), 0)
        except ValueError:
            continue
        names = item.get("names", {}) if isinstance(item, dict) else {}
        normalized: dict[str, str] = {}
        for raw_locale, raw_name in names.items():
            name = str(raw_name).strip()
            if name:
                normalized[_normalize_locale_tag(str(raw_locale))] = name
        if normalized:
            result[effect_id] = normalized
    return result


_NATIVE_NAMES_BY_EFFECT = _load_multilingual_effect_names()
_PREFERRED_EFFECT_LOCALE = _preferred_effect_locale()
_CURATED_EFFECT_NAMES = {
    effect.effect_id: effect.name for effect in BETA_EFFECTS
}


def native_effect_name(effect_id: int, locale: str | None = None) -> str | None:
    """Return one native final-effect name for the requested locale."""

    names = _NATIVE_NAMES_BY_EFFECT.get(effect_id)
    if not names:
        return None
    preferred_locale = _normalize_locale_tag(locale or _PREFERRED_EFFECT_LOCALE)
    exact = names.get(preferred_locale)
    if exact:
        return exact
    language = preferred_locale.split("-", 1)[0]
    same_language = next(
        (
            name
            for locale, name in names.items()
            if locale.split("-", 1)[0] == language
        ),
        None,
    )
    if same_language:
        return same_language
    return names.get("zh-CN") or names.get("en-US") or next(iter(names.values()))


def _native_name_for_effect(effect_id: int) -> str | None:
    return native_effect_name(effect_id)


def _player_ready_effect_name(effect_id: int) -> str | None:
    name = _native_name_for_effect(effect_id)
    if not name:
        return _CURATED_EFFECT_NAMES.get(effect_id)
    # Native strings that contain format tokens are sentence templates, not
    # player-ready effect names.  Their arguments live in separate parameter
    # rows.  Never expose raw control markup such as
    # ``^09~BUFF~{}^09~~`` in the product UI; use a verified contextual
    # fallback when one exists and otherwise keep the effect unnamed.
    unresolved_markers = ("{}", "~BUFF~", "~DEBUFF~", "^09", "\ufffd")
    if any(marker in name for marker in unresolved_markers):
        return _CURATED_EFFECT_NAMES.get(effect_id) or name
    return name


# Native-resolver names take precedence for final effects only. Stage-one
# token rendering is handled separately by contextual_effect_name().
BETA_EFFECTS = tuple(
    EffectDefinition(
        effect.effect_id,
        _player_ready_effect_name(effect.effect_id) or effect.name,
    )
    for effect in BETA_EFFECTS
)

# Raw special-output codes observed in the current E604 / loaded-playthrough
# generator. Rarity 5 exposes an 11-ID canonical Grace subset in slot 6.
# Rarity 4 exposes 21 transient slot-5 codes, but seed 183696634 proved that a
# transient code is not necessarily the final effect ID: stage-1 0xBABD was
# replaced by final 0xAE5A when the game resolved the installed scroll.
OBSERVED_GRACE_IDS: tuple[int, ...] = (
    0x6553,
    0xCE68,
    0xBABD,
    0xEEEA,
    0x16E2,
    0x4192,
    0x47EC,
    0x4FE4,
    0xEB61,
    0x23E5,
    0x2AE6,
    0x8CCC,
    0xB24F,
    0x5012,
    0x7BEA,
    0x590C,
    0x4FA3,
    0xB1E9,
    0xE8EB,
    0x7ECE,
    0x71F6,
)


def _load_grace_names() -> dict[int, str]:
    path = Path(__file__).parent / "data" / "grace_names_zh_cn.json"
    try:
        loaded = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError):
        return {}
    names: dict[int, str] = {}
    for identifier, name in loaded.items():
        normalized_name = str(name).strip()
        if not normalized_name:
            continue
        try:
            effect_id = int(str(identifier), 0)
        except ValueError:
            continue
        if 0 <= effect_id <= 0xFFFFFFFF:
            names[effect_id] = normalized_name
    return names


_GRACE_NAMES = _load_grace_names()
for _grace_id in tuple(_GRACE_NAMES):
    _native = _native_name_for_effect(_grace_id)
    if _native:
        _GRACE_NAMES[_grace_id] = _native

# Rarity-5 slot 6 is the verified canonical Grace-output context. Rarity-4
# slot 5 uses a separately measured stage-1 output map. Never attach final
# effect names to those stage-1 codes.
R5_GRACE_IDS: tuple[int, ...] = (
    0x6553, 0xCE68, 0xBABD, 0xEEEA, 0x16E2, 0x4192, 0x47EC,
    0x4FE4, 0xEB61, 0x7ECE, 0x71F6,
)
R4_SLOT5_OUTPUT_IDS: tuple[int, ...] = OBSERVED_GRACE_IDS

# Rarity-4 stage one writes one of these IDs into physical slot 5.  The
# completion finalizer may replace that slot with an ordinary completed effect,
# but when it completes an earlier slot (or finds no acceptable replacement),
# slot 5 survives as a real final Grace.  Keep the stage-one namespace separate
# from this final-record presentation list even though the raw IDs coincide.
R4_FINAL_GRACE_IDS: tuple[int, ...] = R4_SLOT5_OUTPUT_IDS

R5_GRACE_EFFECTS: tuple[EffectDefinition, ...] = tuple(
    EffectDefinition(
        effect_id,
        _native_name_for_effect(effect_id)
        or _GRACE_NAMES.get(effect_id, "未命名恩宠"),
    )
    for effect_id in R5_GRACE_IDS
)
# Backwards-compatible export.  It now intentionally means R5 Grace choices
# only; R4 slot-5 values are context tokens, not globally named Grace IDs.
GRACE_EFFECTS = R5_GRACE_EFFECTS

R4_FINAL_GRACE_EFFECTS: tuple[EffectDefinition, ...] = tuple(
    EffectDefinition(
        effect_id,
        _native_name_for_effect(effect_id)
        or _GRACE_NAMES.get(effect_id, "未命名恩宠"),
    )
    for effect_id in R4_FINAL_GRACE_IDS
)


R4_SLOT5_EFFECTS: tuple[EffectDefinition, ...] = tuple(
    EffectDefinition(
        effect_id,
        f"R4生成阶段结果码 0x{effect_id:04X}（非最终词条）",
    )
    for effect_id in R4_SLOT5_OUTPUT_IDS
)

# Special growth/state effects are displayed by name, but intentionally kept
# out of BETA_EFFECTS so they cannot be selected as ordinary generated effects.
SPECIAL_EFFECTS: tuple[EffectDefinition, ...] = (
    EffectDefinition(0x0001, "未完成的杰作（画龙点睛；最终效果实验预测中）"),
)

# Generic lookup is retained for legacy callers. Candidate/UI rendering must
# use contextual_effect_name() because raw IDs do not form one proven global
# namespace across rarity and slot roles.
EFFECT_BY_ID = {
    effect.effect_id: effect
    for effect in (
        *BETA_EFFECTS,
        *R4_FINAL_GRACE_EFFECTS,
        *R5_GRACE_EFFECTS,
        *SPECIAL_EFFECTS,
    )
}
ORDINARY_EFFECT_BY_ID = {effect.effect_id: effect for effect in BETA_EFFECTS}


def effect_name(effect_id: int) -> str:
    effect = EFFECT_BY_ID.get(effect_id)
    return effect.name if effect else "未知词条"


def native_effect_definitions() -> tuple[EffectDefinition, ...]:
    """Return the complete final-effect catalog in the preferred locale."""

    return tuple(
        EffectDefinition(effect_id, _native_name_for_effect(effect_id) or "未知词条")
        for effect_id in sorted(_NATIVE_NAMES_BY_EFFECT)
    )


@lru_cache(maxsize=32)
def searchable_scroll_effect_definitions(
    playthrough: int,
    rarity: int,
) -> tuple[EffectDefinition, ...]:
    """Return final ordinary effects reachable in one captured scroll context.

    This is intentionally derived from the native generation tables rather
    than the historical hand-maintained Beta list.  Stage-one tokens are not
    eligible because they fail the final candidate-context and weight gates.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if rarity not in (3, 4, 5):
        raise ValueError("scroll rarity must be 3, 4, or 5")

    from .effect_generation_tables import (
        SCROLL_RECORD_TYPES,
        load_default_effect_generation_tables,
    )

    tables = load_default_effect_generation_tables()
    record_type = SCROLL_RECORD_TYPES[playthrough - 1]
    capacities = tables.category_capacities(
        record_type=record_type,
        rarity=rarity,
    )
    reachable: list[EffectDefinition] = []
    for native_effect in tables.effects_by_id.values():
        if native_effect.row_index == 0:
            continue
        if not tables.candidate_context_allowed(
            native_effect.effect_id,
            record_type=record_type,
        ):
            continue
        category_key = tables.groups_by_key[native_effect.group_key].category_key
        if not 0 <= category_key < len(capacities) or capacities[category_key] == 0:
            continue
        if not tables.native_effect_weight(
            native_effect.effect_id,
            record_type=record_type,
            rarity=rarity,
            playthrough=playthrough,
            restricted_destination_slot=False,
        ):
            continue
        reachable.append(
            EffectDefinition(
                native_effect.effect_id,
                _player_ready_effect_name(native_effect.effect_id) or "未知词条",
            )
        )
    return tuple(sorted(reachable, key=lambda item: (item.name.casefold(), item.effect_id)))


def contextual_effect_name(
    effect_id: int,
    *,
    rarity: int,
    slot: int,
    native_stage_one: bool = False,
) -> str:
    """Resolve a raw slot ID using the rarity/slot namespace observed in game."""
    if rarity == 3 and slot == 5 and effect_id == 0x0001:
        return "未完成的杰作（画龙点睛）"
    if native_stage_one and rarity == 4 and slot == 5:
        if effect_id == 0xFFFFFFFF:
            return "空"
        return f"R4生成阶段结果码 0x{effect_id:04X}（非最终词条）"
    if rarity == 4 and slot == 5 and effect_id in R4_FINAL_GRACE_IDS:
        return _native_name_for_effect(effect_id) or _GRACE_NAMES.get(
            effect_id,
            f"R4恩宠 0x{effect_id:04X}（名称待验证）",
        )
    if rarity == 5 and slot == 6:
        known = _GRACE_NAMES.get(effect_id)
        if known:
            return known
        if effect_id == 0xFFFFFFFF:
            return "空"
        return f"R5恩宠 0x{effect_id:04X}（名称待验证）"
    ordinary = ORDINARY_EFFECT_BY_ID.get(effect_id)
    if ordinary:
        return ordinary.name
    # Direct local-display captures identified final 0xBABD as 月读的恩宠.
    # This is intentionally below the stage-1 branch because the same raw
    # number is also a transient R4 code that resolved to final 0xAE5A for
    # seed 183696634.
    if effect_id == 0xBABD:
        return _GRACE_NAMES.get(effect_id, "月读的恩宠")
    special = next((e for e in SPECIAL_EFFECTS if e.effect_id == effect_id), None)
    if special:
        return special.name
    if effect_id == 0xFFFFFFFF:
        return "空"
    native_name = _native_name_for_effect(effect_id)
    if native_name:
        return native_name
    return f"编号 0x{effect_id:08X}"


def target_effects_for_rarity(
    rarity: int,
    *,
    include_transient_stage_one: bool = False,
) -> tuple[EffectDefinition, ...]:
    """Return context-correct special-output choices.

    Rarity-4 slot 5 starts as a stage-one result code.  The finalizer may
    replace it with an ordinary effect or preserve it as a final Grace, so the
    product selector exposes the verified final Grace names while exact replay
    decides whether the selected Grace survived.  Rarity-5 slot 6 is always the
    canonical Grace context.
    """
    if rarity == 5:
        return R5_GRACE_EFFECTS
    if rarity == 4:
        return R4_FINAL_GRACE_EFFECTS
    if include_transient_stage_one and rarity == 3:
        return R4_SLOT5_EFFECTS
    return ()
