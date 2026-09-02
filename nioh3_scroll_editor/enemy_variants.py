"""Versioned player-facing qualifiers for ambiguous enemy lookup keys."""

from __future__ import annotations

from typing import Mapping


LOCALES = ("zh-CN", "ja-JP", "en-US")


ENEMY_VARIANT_QUALIFIERS: dict[int, dict[str, str]] = {
    0x0004B03B: {
        "zh-CN": "人形",
        "ja-JP": "人型",
        "en-US": "Human form",
    },
    0x00092063: {
        "zh-CN": "妖怪形态",
        "ja-JP": "妖怪形態",
        "en-US": "Yokai form",
    },
    0x00071ED1: {
        "zh-CN": "人形",
        "ja-JP": "人型",
        "en-US": "Human form",
    },
    0x0000A5D2: {
        "zh-CN": "妖怪形态",
        "ja-JP": "妖怪形態",
        "en-US": "Yokai form",
    },
    0x000B605B: {
        "zh-CN": "人形",
        "ja-JP": "人型",
        "en-US": "Human form",
    },
    0x0004ACDF: {
        "zh-CN": "古代妖怪形态",
        "ja-JP": "古代妖怪形態",
        "en-US": "Ancient-yokai form",
    },
    0x00093F79: {
        "zh-CN": "江户妖怪形态",
        "ja-JP": "江戸妖怪形態",
        "en-US": "Edo-yokai form",
    },
    0x0000F9CA: {
        "zh-CN": "人形",
        "ja-JP": "人型",
        "en-US": "Human form",
    },
    0x000179F7: {
        "zh-CN": "妖怪形态",
        "ja-JP": "妖怪形態",
        "en-US": "Yokai form",
    },
    0x000D35E1: {
        "zh-CN": "现任／子",
        "ja-JP": "現任／息子",
        "en-US": "Current / son",
    },
    0x000202A7: {
        "zh-CN": "先代／父（鬼半藏）",
        "ja-JP": "先代／父（鬼半蔵）",
        "en-US": "Former / father (Demon Hanzo)",
    },
}


def enemy_variant_qualifier(lookup_key: int, locale: str = "zh-CN") -> str | None:
    """Return the configured form/identity qualifier for one native lookup key."""

    if locale not in LOCALES:
        raise ValueError(f"unsupported enemy-variant locale: {locale}")
    qualifiers = ENEMY_VARIANT_QUALIFIERS.get(int(lookup_key))
    return qualifiers.get(locale) if qualifiers is not None else None


def enemy_variant_display_name(
    name: str,
    lookup_key: int,
    locale: str = "zh-CN",
) -> str:
    """Add a configured form or identity qualifier to an ambiguous enemy name."""

    qualifier = enemy_variant_qualifier(lookup_key, locale)
    return f"{name}（{qualifier}）" if qualifier else name


def qualify_enemy_names(
    names: Mapping[str, str],
    lookup_key: int,
) -> dict[str, str]:
    """Apply one native lookup-key qualifier to a trilingual name mapping."""

    return {
        locale: enemy_variant_display_name(names[locale], lookup_key, locale)
        for locale in LOCALES
    }


def split_enemy_variant_display_groups(
    enemy_groups: Mapping[str, frozenset[int]],
    locale: str = "zh-CN",
) -> dict[str, frozenset[int]]:
    """Split configured same-name forms into exact player-selectable ID groups."""

    split_groups: dict[str, frozenset[int]] = {}
    for name, keys in enemy_groups.items():
        variant_keys = tuple(
            key for key in sorted(keys) if key in ENEMY_VARIANT_QUALIFIERS
        )
        remaining_keys = frozenset(keys).difference(variant_keys)
        if not variant_keys:
            split_groups[name] = frozenset(keys)
            continue
        for key in variant_keys:
            split_groups[
                enemy_variant_display_name(name, key, locale)
            ] = frozenset((key,))
        if remaining_keys:
            split_groups[name] = remaining_keys
    return dict(sorted(split_groups.items(), key=lambda item: item[0].casefold()))


__all__ = [
    "ENEMY_VARIANT_QUALIFIERS",
    "LOCALES",
    "enemy_variant_display_name",
    "enemy_variant_qualifier",
    "qualify_enemy_names",
    "split_enemy_variant_display_groups",
]
