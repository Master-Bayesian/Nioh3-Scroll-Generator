from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import Protocol, Sequence


class EffectRole(StrEnum):
    PRIMARY = "primary"
    SECONDARY = "secondary"
    SPECIAL = "special"
    GRACE = "grace"
    UNKNOWN = "unknown"


class CatalogProvenance(StrEnum):
    NATIVE_RESOLVER = "native_resolver"
    MANUAL_CAPTURE = "manual_capture"
    UNVERIFIED_HINT = "unverified_hint"


class CatalogGenerationStage(StrEnum):
    FINAL_RECORD = "final_record"
    NATIVE_STAGE_ONE = "native_stage_one"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class EffectSlotPayload:
    prefix: int
    effect_id: int
    value: int
    metadata: int
    tail_0: int
    tail_1: int

    def as_tuple(self) -> tuple[int, int, int, int, int, int]:
        return (
            self.prefix,
            self.effect_id,
            self.value,
            self.metadata,
            self.tail_0,
            self.tail_1,
        )


@dataclass(frozen=True, slots=True)
class EffectContextKey:
    game_version: str
    record_type: int
    playthrough: int
    rarity: int
    generation_stage: CatalogGenerationStage
    slot_role: EffectRole
    slot_index: int
    raw_slot: EffectSlotPayload


@dataclass(frozen=True, slots=True)
class LocalizedEffectName:
    locale: str
    display_name: str
    text_id: int | None
    provenance: CatalogProvenance
    evidence_id: str


@dataclass(frozen=True, slots=True)
class EffectCatalogEntry:
    key: EffectContextKey
    sample_seed: int
    names: tuple[LocalizedEffectName, ...]

    def name_for_locale(self, locale: str) -> LocalizedEffectName | None:
        normalized = normalize_locale(locale)
        return next((name for name in self.names if name.locale == normalized), None)


class LocalizedTextOracle(Protocol):
    def available_locales(self) -> Sequence[str]: ...

    def resolve_text(self, text_id: int, locale: str) -> str | None: ...


def normalize_locale(locale: str) -> str:
    normalized = locale.strip().replace("_", "-")
    if not normalized:
        raise ValueError("locale cannot be empty")
    parts = normalized.split("-")
    language = parts[0].lower()
    if len(parts) == 1:
        return language
    region = parts[1].upper()
    remainder = parts[2:]
    return "-".join((language, region, *remainder))


def capture_all_localized_names(
    oracle: LocalizedTextOracle,
    *,
    text_id: int,
    evidence_id: str,
) -> tuple[LocalizedEffectName, ...]:
    captures: list[LocalizedEffectName] = []
    seen: set[str] = set()
    for raw_locale in oracle.available_locales():
        locale = normalize_locale(raw_locale)
        if locale in seen:
            raise ValueError(f"duplicate locale returned by text oracle: {locale}")
        seen.add(locale)
        display_name = oracle.resolve_text(text_id, locale)
        if display_name is None:
            continue
        normalized_name = display_name.strip()
        if not normalized_name:
            continue
        captures.append(
            LocalizedEffectName(
                locale=locale,
                display_name=normalized_name,
                text_id=text_id,
                provenance=CatalogProvenance.NATIVE_RESOLVER,
                evidence_id=evidence_id,
            )
        )
    return tuple(sorted(captures, key=lambda item: item.locale))


def catalog_entry_to_dict(entry: EffectCatalogEntry) -> dict[str, object]:
    key = entry.key
    return {
        "context": {
            "game_version": key.game_version,
            "record_type": f"0x{key.record_type:04X}",
            "playthrough": key.playthrough,
            "rarity": key.rarity,
            "generation_stage": key.generation_stage.value,
            "slot_role": key.slot_role.value,
            "slot_index": key.slot_index,
        },
        "raw_slot": {
            "prefix": key.raw_slot.prefix,
            "effect_id": f"0x{key.raw_slot.effect_id:08X}",
            "value": key.raw_slot.value,
            "metadata": key.raw_slot.metadata,
            "tail_0": key.raw_slot.tail_0,
            "tail_1": key.raw_slot.tail_1,
        },
        "sample_seed": entry.sample_seed,
        "names": [
            {
                "locale": name.locale,
                "display_name": name.display_name,
                "text_id": name.text_id,
                "provenance": name.provenance.value,
                "evidence_id": name.evidence_id,
            }
            for name in entry.names
        ],
    }
