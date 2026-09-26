from __future__ import annotations

import argparse
import csv
import hashlib
import json
import struct
import sys
from pathlib import Path
from typing import Any, Iterable

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.catalog import searchable_scroll_effect_definitions
from nioh3_scroll_editor.effect_generation_tables import (
    EffectGenerationTableIndex,
    load_default_effect_generation_tables,
)


GAME_VERSION = "PC v2.00.02"
RARITIES = (3, 4, 5)
PLAYTHROUGHS = range(1, 6)
LEVEL_MIN = 1
LEVEL_MAX = 180
LOCALES = ("zh-CN", "ja-JP", "en-US")


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_csv(path: Path, fieldnames: list[str], rows: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fieldnames, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def name_quality(names: dict[str, str]) -> str:
    if any(not names.get(locale, "").strip() for locale in LOCALES):
        return "missing_locale"
    markers = ("{}", "~BUFF~", "~DEBUFF~", "^09", "\ufffd", "��")
    if any(marker in value for value in names.values() for marker in markers):
        return "template_or_unresolved"
    return "complete"


def raw_values(
    tables: EffectGenerationTableIndex,
    effect_id: int,
    rarity: int,
    level: int,
) -> tuple[int, ...]:
    rarity_row = tables.rarity_generation[rarity]
    return tuple(
        sorted(
            {
                tables.resolved_effect_value(
                    effect_id,
                    roll_percent=roll_percent,
                    level=level,
                )
                & 0xFFFFFFFF
                for roll_percent in range(
                    rarity_row.minimum_roll_percent,
                    rarity_row.maximum_roll_percent + 1,
                )
            }
        )
    )


def range_row(
    *,
    effect_id: int,
    rarity: int,
    level: int,
    values: tuple[int, ...] | None,
    error: str = "",
) -> dict[str, Any]:
    if values is None:
        return {
            "effect_id_hex": f"0x{effect_id:08X}",
            "effect_id": effect_id,
            "rarity": rarity,
            "level": level,
            "status": "not_resolved_by_base_formula",
            "minimum_raw": "",
            "maximum_raw": "",
            "minimum_raw_hex": "",
            "maximum_raw_hex": "",
            "distinct_value_count": "",
            "exact_raw_values": "",
            "error": error,
        }
    return {
        "effect_id_hex": f"0x{effect_id:08X}",
        "effect_id": effect_id,
        "rarity": rarity,
        "level": level,
        "status": "verified_base_formula",
        "minimum_raw": values[0],
        "maximum_raw": values[-1],
        "minimum_raw_hex": f"0x{values[0]:08X}",
        "maximum_raw_hex": f"0x{values[-1]:08X}",
        "distinct_value_count": len(values),
        "exact_raw_values": " | ".join(str(value) for value in values),
        "error": "",
    }


def export(output_dir: Path) -> None:
    output_dir.mkdir(parents=True, exist_ok=True)
    data_root = PROJECT_ROOT / "nioh3_scroll_editor" / "data"
    names_payload = load_json(data_root / "effect_names_multilingual.json")
    special_item_payload = load_json(data_root / "special_rule_item_names.json")
    tables = load_default_effect_generation_tables()

    reachable_contexts: dict[int, list[str]] = {}
    for playthrough in PLAYTHROUGHS:
        for rarity in RARITIES:
            context = f"P{playthrough}R{rarity}"
            for effect in searchable_scroll_effect_definitions(playthrough, rarity):
                reachable_contexts.setdefault(effect.effect_id, []).append(context)
    scroll_effect_ids = frozenset(reachable_contexts)

    effects_rows: list[dict[str, Any]] = []
    catalog_json: list[dict[str, Any]] = []
    for effect_id in sorted(tables.effects_by_id):
        definition = tables.effect(effect_id)
        group = tables.group_for_effect(effect_id)
        raw_names = names_payload["effects"].get(f"0x{effect_id:08X}", {})
        names = {
            locale: str(raw_names.get("names", {}).get(locale, "")).strip()
            for locale in LOCALES
        }
        contexts = tuple(reachable_contexts.get(effect_id, ()))
        row = {
            "effect_id_hex": f"0x{effect_id:08X}",
            "effect_id": effect_id,
            "row_index": definition.row_index,
            "text_id": raw_names.get("text_id", ""),
            "name_zh_CN": names["zh-CN"],
            "name_ja_JP": names["ja-JP"],
            "name_en_US": names["en-US"],
            "name_quality": name_quality(names),
            "group_key_hex": f"0x{definition.group_key:04X}",
            "group_key": definition.group_key,
            "category_key_hex": f"0x{group.category_key:02X}",
            "category_key": group.category_key,
            "flags_hex": f"0x{definition.flags:08X}",
            "normalization_flags_hex": f"0x{definition.normalization_flags:08X}",
            "progress_threshold": definition.progress_threshold,
            "alternate_threshold": definition.alternate_threshold,
            "scroll_reachable": bool(contexts),
            "scroll_contexts": " | ".join(contexts),
        }
        effects_rows.append(row)
        catalog_json.append(
            {
                "effect_id": effect_id,
                "effect_id_hex": row["effect_id_hex"],
                "row_index": definition.row_index,
                "text_id": row["text_id"],
                "names": names,
                "name_quality": row["name_quality"],
                "group_key": definition.group_key,
                "category_key": group.category_key,
                "flags": definition.flags,
                "normalization_flags": definition.normalization_flags,
                "progress_threshold": definition.progress_threshold,
                "alternate_threshold": definition.alternate_threshold,
                "scroll_contexts": list(contexts),
            }
        )

    effect_fields = list(effects_rows[0])
    write_csv(output_dir / "effects_full_trilingual.csv", effect_fields, effects_rows)
    (output_dir / "effects_full_trilingual.json").write_text(
        json.dumps(
            {
                "schema": "nioh3-effect-catalog-handoff/v1",
                "game_version": GAME_VERSION,
                "locales": list(LOCALES),
                "effect_count": len(catalog_json),
                "scroll_reachable_effect_count": len(scroll_effect_ids),
                "effects": catalog_json,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    level_180_rows: list[dict[str, Any]] = []
    for effect_id in sorted(tables.effects_by_id):
        for rarity in RARITIES:
            try:
                values = raw_values(tables, effect_id, rarity, LEVEL_MAX)
                row = range_row(
                    effect_id=effect_id,
                    rarity=rarity,
                    level=LEVEL_MAX,
                    values=values,
                )
            except (KeyError, ValueError) as error:
                row = range_row(
                    effect_id=effect_id,
                    rarity=rarity,
                    level=LEVEL_MAX,
                    values=None,
                    error=str(error),
                )
            names = names_payload["effects"].get(f"0x{effect_id:08X}", {}).get(
                "names", {}
            )
            row.update(
                {
                    "name_zh_CN": names.get("zh-CN", ""),
                    "name_ja_JP": names.get("ja-JP", ""),
                    "name_en_US": names.get("en-US", ""),
                    "scroll_reachable": effect_id in scroll_effect_ids,
                    "scroll_contexts": " | ".join(
                        reachable_contexts.get(effect_id, ())
                    ),
                }
            )
            level_180_rows.append(row)
    level_180_fields = list(level_180_rows[0])
    write_csv(
        output_dir / "effect_value_ranges_level_180.csv",
        level_180_fields,
        level_180_rows,
    )

    scroll_range_rows: list[dict[str, Any]] = []
    for effect_id in sorted(scroll_effect_ids):
        names = names_payload["effects"][f"0x{effect_id:08X}"]["names"]
        for rarity in RARITIES:
            rarity_row = tables.rarity_generation[rarity]
            for level in range(LEVEL_MIN, LEVEL_MAX + 1):
                values = raw_values(tables, effect_id, rarity, level)
                row = range_row(
                    effect_id=effect_id,
                    rarity=rarity,
                    level=level,
                    values=values,
                )
                row.update(
                    {
                        "name_zh_CN": names.get("zh-CN", ""),
                        "name_ja_JP": names.get("ja-JP", ""),
                        "name_en_US": names.get("en-US", ""),
                        "minimum_roll_percent": rarity_row.minimum_roll_percent,
                        "maximum_roll_percent": rarity_row.maximum_roll_percent,
                        "scroll_contexts": " | ".join(reachable_contexts[effect_id]),
                    }
                )
                scroll_range_rows.append(row)
    scroll_range_fields = list(scroll_range_rows[0])
    write_csv(
        output_dir / "scroll_effect_value_ranges_levels_1_180.csv",
        scroll_range_fields,
        scroll_range_rows,
    )

    special_item_rows = []
    for raw_key, names in sorted(
        special_item_payload["items"].items(), key=lambda item: int(item[0], 0)
    ):
        key = int(raw_key, 0)
        special_item_rows.append(
            {
                "item_key_hex": f"0x{key:04X}",
                "item_key": key,
                "name_zh_CN": names.get("zh-CN", ""),
                "name_ja_JP": names.get("ja-JP", ""),
                "name_en_US": names.get("en-US", ""),
                "coverage": "zh-CN+en-US" if not names.get("ja-JP") else "trilingual",
                "scope": "special-rule qualifier item only",
            }
        )
    write_csv(
        output_dir / "special_rule_items_partial.csv",
        list(special_item_rows[0]),
        special_item_rows,
    )

    item_table = tables.resource.table("item")
    native_item_rows = []
    for row_index, raw_row in enumerate(item_table.rows()):
        native_item_rows.append(
            {
                "row_index": row_index,
                "row_sha256": hashlib.sha256(raw_row).hexdigest().upper(),
                "record_type_hex_at_0x152": f"0x{struct.unpack_from('<H', raw_row, 0x152)[0]:04X}",
                "field_0x154": struct.unpack_from("<I", raw_row, 0x154)[0],
                "field_0x15C": struct.unpack_from("<I", raw_row, 0x15C)[0],
                "candidate_item_flags_hex_at_0xB0": f"0x{struct.unpack_from('<I', raw_row, 0xB0)[0]:08X}",
                "mode_at_0x182": raw_row[0x182],
                "name_zh_CN": "",
                "name_ja_JP": "",
                "name_en_US": "",
                "name_status": "not_present_in_current_capture",
            }
        )
    write_csv(
        output_dir / "native_item_rows_raw_no_names.csv",
        list(native_item_rows[0]),
        native_item_rows,
    )

    summary = {
        "schema": "nioh3-effect-range-handoff-summary/v1",
        "game_version": GAME_VERSION,
        "effect_count": len(effects_rows),
        "scroll_reachable_effect_count": len(scroll_effect_ids),
        "level_180_range_row_count": len(level_180_rows),
        "scroll_level_1_180_range_row_count": len(scroll_range_rows),
        "special_rule_item_partial_count": len(special_item_rows),
        "native_item_raw_row_count": len(native_item_rows),
        "limitations": [
            "The 1-180 exhaustive range sheet covers the 51 effects reachable by captured scroll-generation contexts.",
            "All 3609 native effects have level-180 range attempts; 182 non-scroll/contextual definitions are not resolved by the verified base formula and are marked instead of guessed.",
            "The current item-table capture contains 3362 raw rows but no complete trilingual item-name localization pool.",
            "Only 32 special-rule qualifier items currently have verified Simplified Chinese and English names; Japanese remains blank.",
        ],
    }
    (output_dir / "SUMMARY.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    readme = f"""# Nioh 3 PC v2.00.02 effect catalog and raw-value ranges

This handoff is generated from the verified tables bundled with the project.

## Contents

- `effects_full_trilingual.csv/json`: all {len(effects_rows)} native effect IDs and three-locale names, native group/category metadata, and scroll reachability contexts.
- `effect_value_ranges_level_180.csv`: rarity 3/4/5 raw-value ranges at level 180 for all native effect IDs. Unsupported contextual definitions are marked rather than guessed.
- `scroll_effect_value_ranges_levels_1_180.csv`: exhaustive level 1-180 raw ranges and exact discrete values for all {len(scroll_effect_ids)} effects reachable by the captured scroll-generation contexts.
- `special_rule_items_partial.csv`: {len(special_item_rows)} automatic-activation qualifier items; currently Simplified Chinese and English only.
- `native_item_rows_raw_no_names.csv`: {len(native_item_rows)} raw native item rows with only fields whose offsets are already documented. It is not a localized item-name catalog.
- `SUMMARY.json`: machine-readable coverage and limitations.

## Interpretation

`verified_base_formula` means the bundled PC v2.00.02 native normalization tables can reproduce the listed raw values for the stated level, rarity, and percentile interval. It does not mean the effect is legal on a scroll. Use `scroll_reachable` and `scroll_contexts` for that distinction.

The app accepts any uint32 raw value for local-only edits. Values outside the listed native set are deliberate modded values and should not be presented as naturally generated.

## Known gap

The project does not currently contain a complete three-language equipment/item localization pool. The 3362-row item parameter capture is therefore exported separately with blank name columns. A future locale capture can join names onto `row_index` or a subsequently recovered stable item key without changing the effect catalog.
"""
    (output_dir / "README.md").write_text(readme, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args()
    export(args.output_dir.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
