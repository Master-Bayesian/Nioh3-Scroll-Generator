"""Export the versioned native enemy-role catalog used by scroll generation."""

from __future__ import annotations

import argparse
import csv
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import struct
import sys
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
if str(REPOSITORY_ROOT) not in sys.path:
    sys.path.insert(0, str(REPOSITORY_ROOT))
DEFAULT_OUTPUT_ROOT = (
    REPOSITORY_ROOT
    / "docs"
    / "knowledge"
    / "versions"
    / "pc-v2.00.02"
    / "catalogs"
)
GAME_VERSION = "PC v2.00.02"
LOCALES = ("zh-CN", "ja-JP", "en-US")
ROLE_CLASS_PATHS = {
    0: ("class1:ordinary", "class2:ordinary"),
    1: ("class1:ordinary", "class2:ordinary"),
    2: ("class1:ordinary", "class2:ordinary"),
    3: ("class1:ordinary", "class2:ordinary"),
    4: ("class0:role4",),
    5: ("class0:role5", "class1:highest-only"),
}


CANONICAL_TEXT_SUFFIXES = frozenset((".csv", ".json", ".md", ".py", ".yaml", ".yml"))


def _canonical_file_bytes(path: Path) -> bytes:
    data = path.read_bytes()
    if path.suffix.casefold() in CANONICAL_TEXT_SUFFIXES:
        data = data.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    return data


def _sha256(path: Path) -> str:
    return hashlib.sha256(_canonical_file_bytes(path)).hexdigest().upper()


def _hex(value: int, width: int) -> str:
    return f"0x{value:0{width}X}"


def _enabled_playthroughs(mask: int) -> list[int]:
    return [playthrough for playthrough in range(1, 6) if mask & (1 << (playthrough - 1))]


def _load_rows() -> tuple[list[dict[str, Any]], dict[str, Any]]:
    from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
    from nioh3_scroll_editor.auxiliary_generation import (
        DEFAULT_AUXILIARY_RESOURCE_ROOT,
        load_default_auxiliary_generation_tables,
    )

    tables = load_default_auxiliary_generation_tables()
    if tables.enemy_candidates is None or tables.special_context is None:
        raise RuntimeError("the bundled native enemy tables are incomplete")

    catalogs = {locale: load_auxiliary_name_catalog(locale) for locale in LOCALES}
    rows: list[dict[str, Any]] = []
    for row_index, raw in enumerate(tables.enemy_candidates.rows()):
        lookup_key = struct.unpack_from("<I", raw, 0x04)[0]
        role = raw[0x1A]
        catalog_key = f"0X{lookup_key:08X}"
        localization = {
            locale: {
                "name": catalogs[locale].enemy_name(lookup_key),
                "native_markup": catalogs[locale].enemies[catalog_key].get(
                    "native_markup"
                ),
                "text_id": catalogs[locale].enemies[catalog_key].get("text_id"),
                "parameter_row_index": catalogs[locale].enemies[catalog_key].get(
                    "row_index"
                ),
                "provenance": catalogs[locale].enemies[catalog_key].get(
                    "provenance"
                ),
            }
            for locale in LOCALES
        }
        names = {
            locale: localization[locale]["name"]
            for locale in LOCALES
        }
        if any(name.startswith("Unknown enemy ") for name in names.values()):
            raise RuntimeError(
                f"enemy candidate row {row_index} is missing a localized name"
            )
        playthrough_mask = raw[0x16]
        rows.append(
            {
                "row_index": row_index,
                "lookup_key": _hex(lookup_key, 8),
                "names": names,
                "localization": localization,
                "role": role,
                "class_paths": list(ROLE_CLASS_PATHS[role]),
                "cost": struct.unpack_from("<f", raw, 0x0C)[0],
                "cost_float32_bits": _hex(struct.unpack_from("<I", raw, 0x0C)[0], 8),
                "scratch_rule_key": _hex(struct.unpack_from("<H", raw, 0x12)[0], 4),
                "terrain_mask": _hex(struct.unpack_from("<H", raw, 0x14)[0], 4),
                "playthrough_mask": _hex(playthrough_mask, 2),
                "enabled_playthroughs": _enabled_playthroughs(playthrough_mask),
                "linked_group": raw[0x18],
                "selector": raw[0x19],
                "terrain_discriminator": raw[0x1B],
                "raw_row_hex": raw.hex().upper(),
            }
        )

    resource_root = DEFAULT_AUXILIARY_RESOURCE_ROOT
    candidate_path = resource_root / "tables" / "auxiliary_enemy_candidate.bin"
    context_path = resource_root / "tables" / "special_context.bin"
    metadata = {
        "candidate_table": {
            "path": candidate_path.relative_to(REPOSITORY_ROOT).as_posix(),
            "sha256": _sha256(candidate_path),
            "row_size": 0x1C,
            "row_count": len(rows),
            "parameter_manager_offset": "0xA80",
        },
        "special_context_table": {
            "path": context_path.relative_to(REPOSITORY_ROOT).as_posix(),
            "sha256": _sha256(context_path),
            "row_size": 0x30,
            "row_count": tables.special_context.row_count,
        },
        "localization_catalogs": {
            locale: {
                "path": (
                    Path("nioh3_scroll_editor")
                    / "data"
                    / "auxiliary_names"
                    / f"{locale}.json"
                ).as_posix(),
                "sha256": _sha256(
                    REPOSITORY_ROOT
                    / "nioh3_scroll_editor"
                    / "data"
                    / "auxiliary_names"
                    / f"{locale}.json"
                ),
            }
            for locale in LOCALES
        },
    }
    return rows, metadata


def _load_class_profiles() -> list[dict[str, Any]]:
    from nioh3_scroll_editor.auxiliary_generation import (
        load_default_auxiliary_generation_tables,
    )

    table = load_default_auxiliary_generation_tables().special_context
    if table is None:
        raise RuntimeError("the bundled special-context table is unavailable")
    profiles: list[dict[str, Any]] = []
    for row_index, raw in enumerate(table.rows()):
        budgets = [struct.unpack_from("<f", raw, 4 + index * 4)[0] for index in range(5)]
        profiles.append(
            {
                "row_index": row_index,
                "mode": _hex(raw[0x28], 2),
                "branch_class": raw[0x29],
                "active_budgets": [value for value in budgets if value > 0.0],
                "raw_row_hex": raw.hex().upper(),
            }
        )
    return profiles


def build_role_catalog_payload() -> dict[str, Any]:
    rows, source = _load_rows()
    role_counts = Counter(row["role"] for row in rows)
    by_role: dict[str, list[int]] = defaultdict(list)
    by_lookup_key: dict[str, int] = {}
    by_localized_name: dict[str, dict[str, list[int]]] = {
        locale: defaultdict(list) for locale in LOCALES
    }
    for row in rows:
        by_role[str(row["role"])].append(row["row_index"])
        by_lookup_key[row["lookup_key"]] = row["row_index"]
        for locale in LOCALES:
            by_localized_name[locale][row["names"][locale]].append(row["row_index"])
    return {
        "schema": "nioh3-scroll-enemy-role-catalog/v1",
        "game_version": GAME_VERSION,
        "scope": {
            "caller_option": 0,
            "locales": list(LOCALES),
            "candidate_rows": len(rows),
            "distinct_lookup_keys": len({row["lookup_key"] for row in rows}),
        },
        "source": source,
        "candidate_row_layout": {
            "+0x04": "uint32 enemy lookup key",
            "+0x0C": "float32 budget cost",
            "+0x12": "uint16 scratch special-rule key",
            "+0x14": "uint16 terrain exclusion mask",
            "+0x16": "uint8 playthrough bit mask",
            "+0x18": "uint8 linked-group identifier",
            "+0x19": "uint8 descriptor selector",
            "+0x1A": "uint8 native role",
            "+0x1B": "uint8 terrain discriminator",
        },
        "role_counts": {str(role): role_counts[role] for role in sorted(role_counts)},
        "class_constraints": {
            "0": {
                "roles": [4, 5],
                "notes": "The highest active group uses role 5 when available; lower groups choose role 4 or role 5 through threshold key 0xCEFC.",
            },
            "1": {
                "ordinary_roles": [0, 1, 2, 3],
                "highest_group_roles": [5],
                "maximum_role5_entries": 1,
            },
            "2": {
                "ordinary_roles": [0, 1, 2, 3],
                "excluded_roles": [4, 5],
            },
        },
        "class_selection": {
            "generator_rva": "0x10291F0",
            "threshold_key": "0x1E7D",
            "threshold_out_of_10000": 2000,
            "class_2_when_first_roll_below_threshold": True,
            "remaining_branch_binary_split": [1, 0],
            "approximate_natural_distribution": {
                "0": 0.4,
                "1": 0.4,
                "2": 0.2,
            },
            "note": "Exact counts follow the discrete 16-bit RNG stream.",
        },
        "class_0_lower_group_role_selection": {
            "threshold_key": "0xCEFC",
            "threshold_out_of_10000": 2000,
            "role_5_when_roll_at_least_threshold": True,
        },
        "native_entry_points": {
            "class_0": "0x102AB7A",
            "class_1": "0x102A259",
            "class_2": "0x1029A70",
            "budget_helper": "0x1026FD0",
            "lcg": "0x30DCD4",
        },
        "class_profiles": _load_class_profiles(),
        "indexes": {
            "by_role": {key: value for key, value in sorted(by_role.items())},
            "by_lookup_key": {
                key: value for key, value in sorted(by_lookup_key.items())
            },
            "by_localized_name": {
                locale: {
                    name: row_indices
                    for name, row_indices in sorted(
                        by_localized_name[locale].items(), key=lambda item: item[0]
                    )
                }
                for locale in LOCALES
            },
        },
        "rows": rows,
    }


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def _write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    fields = (
        "row_index",
        "lookup_key",
        "name_zh_CN",
        "name_ja_JP",
        "name_en_US",
        "role",
        "class_paths",
        "cost",
        "cost_float32_bits",
        "scratch_rule_key",
        "terrain_mask",
        "playthrough_mask",
        "enabled_playthroughs",
        "linked_group",
        "selector",
        "terrain_discriminator",
        "raw_row_hex",
    )
    with path.open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    "row_index": row["row_index"],
                    "lookup_key": row["lookup_key"],
                    "name_zh_CN": row["names"]["zh-CN"],
                    "name_ja_JP": row["names"]["ja-JP"],
                    "name_en_US": row["names"]["en-US"],
                    "role": row["role"],
                    "class_paths": ";".join(row["class_paths"]),
                    "cost": row["cost"],
                    "cost_float32_bits": row["cost_float32_bits"],
                    "scratch_rule_key": row["scratch_rule_key"],
                    "terrain_mask": row["terrain_mask"],
                    "playthrough_mask": row["playthrough_mask"],
                    "enabled_playthroughs": ";".join(
                        str(value) for value in row["enabled_playthroughs"]
                    ),
                    "linked_group": row["linked_group"],
                    "selector": row["selector"],
                    "terrain_discriminator": row["terrain_discriminator"],
                    "raw_row_hex": row["raw_row_hex"],
                }
            )


def _write_markdown(path: Path, payload: dict[str, Any]) -> None:
    grouped: dict[tuple[int, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in payload["rows"]:
        key = (
            row["role"],
            row["names"]["zh-CN"],
            row["names"]["ja-JP"],
            row["names"]["en-US"],
        )
        grouped[key].append(row)

    lines = [
        "# Native enemy role catalog — PC v2.00.02",
        "",
        "This file is generated by `tools/export_enemy_role_catalog.py`. Do not edit it by hand.",
        "",
        "The native role is byte `+0x1A` of each `0x1C`-byte row in the bundled enemy-candidate table. It is a selection-family field, not a localized enemy species. The same displayed name can have multiple lookup keys and multiple roles.",
        "",
        "## Structural class rules",
        "",
        "- Class 0 selects only roles 4 and 5.",
        "- Class 1 selects roles 0–3 in ordinary groups and at most one role-5 entry in the highest group.",
        "- Class 2 selects only roles 0–3.",
        "- These rules cover the normal scroll UI path (`caller_option = 0`). Other caller-option paths remain uncertified.",
        "",
        "## Candidate counts",
        "",
        "| Role | Candidate rows | Structurally reachable class paths |",
        "| ---: | ---: | --- |",
    ]
    for role, count in payload["role_counts"].items():
        paths = ", ".join(ROLE_CLASS_PATHS[int(role)])
        lines.append(f"| {role} | {count} | {paths} |")

    lines.extend(
        [
            "",
            "## Grouped trilingual role table",
            "",
            "Rows are grouped only when role and all three native names match. The JSON and CSV siblings preserve every candidate row and raw byte.",
            "",
            "| Role | 简体中文 | 日本語 | English | Lookup key @ candidate row |",
            "| ---: | --- | --- | --- | --- |",
        ]
    )
    for (role, zh_name, ja_name, en_name), rows in sorted(
        grouped.items(), key=lambda item: (item[0][0], item[0][1], item[0][3])
    ):
        references = ", ".join(
            f"`{row['lookup_key']}@{row['row_index']}`"
            for row in sorted(rows, key=lambda item: item["row_index"])
        )
        lines.append(
            f"| {role} | {zh_name} | {ja_name} | {en_name} | {references} |"
        )
    path.write_text(
        "\n".join(lines) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def export(output_root: Path) -> dict[str, Any]:
    payload = build_role_catalog_payload()
    output_root.mkdir(parents=True, exist_ok=True)
    _write_json(output_root / "enemy-roles.json", payload)
    _write_csv(output_root / "enemy-roles.csv", payload["rows"])
    _write_markdown(output_root / "enemy-roles.md", payload)
    return payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    args = parser.parse_args()
    payload = export(args.output_root.resolve())
    print(
        f"Exported {len(payload['rows'])} candidate rows to {args.output_root.resolve()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
