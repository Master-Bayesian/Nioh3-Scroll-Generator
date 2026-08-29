"""Localized names for deterministic scroll auxiliary-generation outputs."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
import json
from pathlib import Path
from typing import Any, Mapping


AUXILIARY_NAME_SCHEMA = "nioh3-scroll-auxiliary-names/v1"
DEFAULT_AUXILIARY_NAME_ROOT = Path(__file__).resolve().parent / "data" / "auxiliary_names"


def _hex_key(value: int, width: int) -> str:
    return f"0X{value:0{width}X}"


@dataclass(frozen=True, slots=True)
class AuxiliaryNameCatalog:
    locale: str
    terrain: Mapping[str, Mapping[str, Any]]
    special_rules: Mapping[str, Mapping[str, Any]]
    enemies: Mapping[str, Mapping[str, Any]]

    def terrain_name(self, row_index: int) -> str:
        entry = self.terrain.get(str(row_index))
        if entry and entry.get("name"):
            return str(entry["name"])
        return f"Unknown terrain row {row_index}"

    def terrain_effect_name(self, key: int) -> str:
        wanted = _hex_key(key, 4).replace("0X", "0x")
        for entry in self.terrain.values():
            if wanted in entry.get("hash_keys", ()) and entry.get("name"):
                return str(entry["name"])
        return f"Unknown terrain effect 0x{key:04X}"

    def special_rule_name(self, key: int) -> str:
        if key == 0:
            return "None"
        entry = self.special_rules.get(_hex_key(key, 4))
        if entry:
            name = entry.get("display_name") or entry.get("name")
            if name:
                return str(name)
        return f"Unknown rule 0x{key:04X}"

    def enemy_name(self, lookup_key: int) -> str:
        entry = self.enemies.get(_hex_key(lookup_key, 8))
        if entry and entry.get("name"):
            return str(entry["name"])
        return f"Unknown enemy 0x{lookup_key:08X}"

    def enemy_keys_for_name(self, name: str) -> frozenset[int]:
        """Return every native lookup key sharing one localized enemy name."""

        wanted = name.strip().casefold()
        if not wanted:
            return frozenset()
        return frozenset(
            int(key, 16)
            for key, entry in self.enemies.items()
            if str(entry.get("name", "")).strip().casefold() == wanted
        )

    def special_rule_key_groups(self) -> Mapping[str, frozenset[int]]:
        """Group every native rule key by its localized displayed meaning."""

        grouped: dict[str, set[int]] = {}
        for key, entry in self.special_rules.items():
            name = str(entry.get("display_name") or entry.get("name") or "").strip()
            if name:
                grouped.setdefault(name, set()).add(int(key, 16))
        return {
            name: frozenset(keys)
            for name, keys in sorted(grouped.items(), key=lambda item: item[0].casefold())
        }

    def enemy_key_groups(self) -> Mapping[str, frozenset[int]]:
        """Group every native enemy lookup key by localized enemy name."""

        grouped: dict[str, set[int]] = {}
        for key, entry in self.enemies.items():
            name = str(entry.get("name", "")).strip()
            if name:
                grouped.setdefault(name, set()).add(int(key, 16))
        return {
            name: frozenset(keys)
            for name, keys in sorted(grouped.items(), key=lambda item: item[0].casefold())
        }


def _load_one(path: Path) -> AuxiliaryNameCatalog:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("schema") != AUXILIARY_NAME_SCHEMA:
        raise ValueError(f"unsupported auxiliary-name catalog schema: {path}")
    return AuxiliaryNameCatalog(
        locale=str(payload["locale"]),
        terrain=payload["terrain"],
        special_rules=payload["special_rules"],
        enemies=payload["enemies"],
    )


@lru_cache(maxsize=None)
def load_auxiliary_name_catalog(
    locale: str = "zh-CN",
    *,
    root: str | Path = DEFAULT_AUXILIARY_NAME_ROOT,
) -> AuxiliaryNameCatalog:
    """Load a bundled native catalog, falling back to Japanese if needed."""

    root_path = Path(root)
    requested = root_path / f"{locale}.json"
    if requested.is_file():
        return _load_one(requested)
    fallback = root_path / "ja-JP.json"
    if fallback.is_file():
        return _load_one(fallback)
    raise FileNotFoundError(f"no auxiliary-name catalog for {locale} or ja-JP")


__all__ = [
    "AUXILIARY_NAME_SCHEMA",
    "AuxiliaryNameCatalog",
    "DEFAULT_AUXILIARY_NAME_ROOT",
    "load_auxiliary_name_catalog",
]
