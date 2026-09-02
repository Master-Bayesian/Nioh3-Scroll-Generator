"""Export the player-readable enemy combination guide for PC v2.00.02."""

from __future__ import annotations

import argparse
import csv
from collections import defaultdict
import json
from pathlib import Path
import sys
from typing import Any, Iterable


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
if str(REPOSITORY_ROOT) not in sys.path:
    sys.path.insert(0, str(REPOSITORY_ROOT))

from nioh3_scroll_editor.auxiliary_feasibility import (  # noqa: E402
    viable_enemy_branch_classes,
)
from tools.export_enemy_role_catalog import (  # noqa: E402
    DEFAULT_OUTPUT_ROOT as ROLE_CATALOG_OUTPUT_ROOT,
    build_role_catalog_payload,
)


DEFAULT_OUTPUT_ROOT = ROLE_CATALOG_OUTPUT_ROOT
SCHEMA = "nioh3-scroll-enemy-combination-catalog/v1"
LOCALES = ("zh-CN", "ja-JP", "en-US")
FAMILY_ORDER = ("O", "A", "B", "A/B")
FAMILY_TITLES = {
    "O": "Ordinary pool O (native roles 0-3)",
    "A": "Dedicated pool A (native role 4)",
    "B": "Dedicated pool B (native role 5)",
    "A/B": "Dedicated pool A/B alternative (native roles 4 and 5)",
}
FAMILY_NOTES = {
    "O": (
        "Structurally available to classes 1 and 2. It can coexist with other "
        "O requirements, or with at most one requirement assigned to B in class 1."
    ),
    "A": (
        "Structurally available only to class 0. It cannot coexist with an O "
        "requirement in the normal generator. Class 0 has at most three entries "
        "and reserves one entry for B, so at most two A-only requirements fit."
    ),
    "B": (
        "Available to class 0, or to the single highest-group B position in "
        "class 1. It can coexist with O only when at most one requested enemy "
        "is assigned to B."
    ),
    "A/B": (
        "This display name has distinct native candidates in both dedicated "
        "pools. Treat it as A in class 0 or as the one B requirement in class 1."
    ),
}


def _family_for_roles(roles: Iterable[int]) -> str:
    values = frozenset(int(role) for role in roles)
    has_ordinary = bool(values.difference((4, 5)))
    has_a = 4 in values
    has_b = 5 in values
    if has_ordinary and (has_a or has_b):
        return "mixed"
    if has_ordinary:
        return "O"
    if has_a and has_b:
        return "A/B"
    if has_a:
        return "A"
    if has_b:
        return "B"
    raise ValueError(f"empty or unsupported enemy role set: {sorted(values)}")


def _group_display_entries(role_payload: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in role_payload["rows"]:
        key = tuple(row["display_names"][locale] for locale in LOCALES)
        grouped[key].append(row)

    entries: list[dict[str, Any]] = []
    for names, rows in grouped.items():
        roles = sorted({int(row["role"]) for row in rows})
        family = _family_for_roles(roles)
        viable_classes = list(viable_enemy_branch_classes((roles,)))
        playthroughs = sorted(
            {
                playthrough
                for row in rows
                for playthrough in row["enabled_playthroughs"]
            }
        )
        entries.append(
            {
                "names": dict(zip(LOCALES, names, strict=True)),
                "player_family": family,
                "native_roles": roles,
                "viable_classes_for_single_requirement": viable_classes,
                "enabled_playthroughs_union": playthroughs,
                "candidate_keys": sorted({row["lookup_key"] for row in rows}),
                "candidate_rows": sorted(int(row["row_index"]) for row in rows),
                "costs": sorted({float(row["cost"]) for row in rows}),
                "candidate_count": len(rows),
                "family_rule": FAMILY_NOTES.get(
                    family,
                    "Mixed ordinary and dedicated roles require candidate-level analysis.",
                ),
            }
        )
    entries.sort(
        key=lambda entry: (
            FAMILY_ORDER.index(entry["player_family"])
            if entry["player_family"] in FAMILY_ORDER
            else len(FAMILY_ORDER),
            entry["names"]["zh-CN"],
            entry["names"]["en-US"],
        )
    )
    return entries


def _unavailable_display_entries(
    role_payload: dict[str, Any],
) -> list[dict[str, Any]]:
    from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog

    catalogs = {locale: load_auxiliary_name_catalog(locale) for locale in LOCALES}
    candidate_names = {
        tuple(row["names"][locale] for locale in LOCALES)
        for row in role_payload["rows"]
    }
    all_names: dict[tuple[str, str, str], set[str]] = defaultdict(set)
    for lookup_key in catalogs["zh-CN"].enemies:
        raw_key = int(lookup_key, 16)
        names = tuple(catalogs[locale].enemy_name(raw_key) for locale in LOCALES)
        all_names[names].add(f"0x{raw_key:08X}")

    unavailable = [
        {
            "names": dict(zip(LOCALES, names, strict=True)),
            "localization_lookup_keys": sorted(lookup_keys),
            "availability": "not_in_native_scroll_candidate_table",
        }
        for names, lookup_keys in all_names.items()
        if names not in candidate_names
    ]
    unavailable.sort(
        key=lambda entry: (
            entry["names"]["zh-CN"],
            entry["names"]["en-US"],
        )
    )
    return unavailable


def _resolve_roles(entries: list[dict[str, Any]], zh_name: str) -> frozenset[int]:
    matches = [
        entry for entry in entries if entry["names"]["zh-CN"] == zh_name
    ]
    if len(matches) != 1:
        raise RuntimeError(
            f"expected one player entry named {zh_name!r}, found {len(matches)}"
        )
    return frozenset(matches[0]["native_roles"])


def build_combination_payload() -> dict[str, Any]:
    role_payload = build_role_catalog_payload()
    entries = _group_display_entries(role_payload)
    unavailable_entries = _unavailable_display_entries(role_payload)
    native_candidate_names = {
        tuple(row["names"][locale] for locale in LOCALES)
        for row in role_payload["rows"]
    }
    target_names = ("一目连", "德川国松", "德川庆喜")
    target_roles = [_resolve_roles(entries, name) for name in target_names]
    target_classes = viable_enemy_branch_classes(target_roles)

    family_counts: dict[str, int] = defaultdict(int)
    for entry in entries:
        family_counts[entry["player_family"]] += 1

    return {
        "schema": SCHEMA,
        "game_version": role_payload["game_version"],
        "scope": {
            "caller_option": 0,
            "normal_ui_path": True,
            "recovered_branch_classes": [0, 1, 2],
            "mode_row_count": len(role_payload["class_profiles"]),
            "display_entry_count": len(entries),
            "unavailable_display_entry_count": len(unavailable_entries),
            "native_localization_display_entry_count": (
                len(native_candidate_names) + len(unavailable_entries)
            ),
            "candidate_row_count": len(role_payload["rows"]),
            "locales": list(LOCALES),
        },
        "meaning_of_legality": {
            "structurally_impossible": (
                "No branch class can assign a permitted native role to every "
                "requested enemy. No Seed can satisfy the combination on this path."
            ),
            "structurally_compatible": (
                "At least one branch class survives role-family checks. This does "
                "not prove that a Seed exists because playthrough, terrain, "
                "parameter gates, budgets, linked groups, and RNG still apply."
            ),
            "proven_legal": (
                "Exact forward replay produced a final enemy list containing all "
                "requested enemies under the selected playthrough and terrain."
            ),
        },
        "family_definitions": {
            family: {
                "title": FAMILY_TITLES[family],
                "display_entry_count": family_counts.get(family, 0),
                "rule": FAMILY_NOTES[family],
            }
            for family in FAMILY_ORDER
        },
        "branch_classes": {
            "0": {
                "approximate_natural_probability": 0.4,
                "allowed_roles": [4, 5],
                "modes": ["0x57", "0x6F"],
                "active_group_budgets": [[4, 4], [4, 4, 4]],
                "structural_test": (
                    "Every requirement has a role in {4, 5}, no more than three "
                    "requirements are selected, and no more than two are role-4-only."
                ),
                "notes": (
                    "All role-4/role-5 candidates cost 4, so each budget-4 group "
                    "contains exactly one entry. The highest active group uses role "
                    "5 when available; lower groups choose role 4 at about 20% or "
                    "role 5 at about 80% before later eligibility gates."
                ),
            },
            "1": {
                "approximate_natural_probability": 0.4,
                "ordinary_roles": [0, 1, 2, 3],
                "highest_group_role": 5,
                "maximum_requested_role5_assignments": 1,
                "modes": ["0x4C", "0x7D"],
                "active_group_budgets": [[3, 3, 4], [3, 3, 4, 4]],
                "structural_test": (
                    "Assign every requirement to an ordinary role or role 5, with "
                    "at most one requirement assigned to role 5."
                ),
            },
            "2": {
                "approximate_natural_probability": 0.2,
                "allowed_roles": [0, 1, 2, 3],
                "modes": ["0x48", "0x8E", "0x62"],
                "active_group_budgets": [[3, 3, 4], [3, 3, 4, 4], [3, 3, 4, 4, 5]],
                "structural_test": "Every requirement has at least one role in {0, 1, 2, 3}.",
            },
        },
        "quick_decision_table": [
            {
                "requirements": "O only",
                "structural_result": "compatible",
                "viable_classes": [1, 2],
            },
            {
                "requirements": "One to three A/B-family requirements, with at most two A-only",
                "structural_result": "compatible",
                "viable_classes": [0],
                "note": "A single B or A/B requirement may also fit class 1.",
            },
            {
                "requirements": "Three A-only requirements",
                "structural_result": "impossible",
                "viable_classes": [],
            },
            {
                "requirements": "Four or more A/B-family requirements",
                "structural_result": "impossible",
                "viable_classes": [],
            },
            {
                "requirements": "O plus exactly one B or A/B requirement",
                "structural_result": "compatible",
                "viable_classes": [1],
            },
            {
                "requirements": "O plus two or more B-assigned requirements",
                "structural_result": "impossible",
                "viable_classes": [],
            },
            {
                "requirements": "O plus any A-only requirement",
                "structural_result": "impossible",
                "viable_classes": [],
            },
        ],
        "worked_examples": [
            {
                "requested_names_zh_CN": list(target_names),
                "role_choices": [sorted(roles) for roles in target_roles],
                "viable_classes": list(target_classes),
                "result": "structurally_impossible",
                "reason": (
                    "Class 0 excludes Ichimokuren; class 1 permits only one role-5 "
                    "requirement; class 2 excludes both Tokugawa candidates."
                ),
            }
        ],
        "decision_algorithm": [
            "Resolve each selected display name to all candidate rows enabled for the chosen playthrough.",
            "Collect the possible native roles for each requirement; never collapse a display name to one global role.",
            "Evaluate class 0, class 1, and class 2 using the structural tests above.",
            "If no class survives, reject the request as structurally impossible before scanning Seeds.",
            "If a class survives, apply playthrough, terrain, descriptor, parameter, budget, linked-group, and duplicate gates.",
            "Only an exact full forward replay that contains every requested enemy proves a Seed legal.",
        ],
        "display_entries": entries,
        "unavailable_display_entries": unavailable_entries,
        "source_role_catalog": {
            "schema": role_payload["schema"],
            "candidate_table": role_payload["source"]["candidate_table"],
            "special_context_table": role_payload["source"]["special_context_table"],
        },
    }


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def _write_csv(path: Path, entries: list[dict[str, Any]]) -> None:
    fields = (
        "name_zh_CN",
        "name_ja_JP",
        "name_en_US",
        "player_family",
        "native_roles",
        "viable_classes_for_single_requirement",
        "enabled_playthroughs_union",
        "candidate_keys",
        "candidate_rows",
        "costs",
        "candidate_count",
        "family_rule",
    )
    with path.open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for entry in entries:
            writer.writerow(
                {
                    "name_zh_CN": entry["names"]["zh-CN"],
                    "name_ja_JP": entry["names"]["ja-JP"],
                    "name_en_US": entry["names"]["en-US"],
                    "player_family": entry["player_family"],
                    "native_roles": ";".join(map(str, entry["native_roles"])),
                    "viable_classes_for_single_requirement": ";".join(
                        map(str, entry["viable_classes_for_single_requirement"])
                    ),
                    "enabled_playthroughs_union": ";".join(
                        map(str, entry["enabled_playthroughs_union"])
                    ),
                    "candidate_keys": ";".join(entry["candidate_keys"]),
                    "candidate_rows": ";".join(map(str, entry["candidate_rows"])),
                    "costs": ";".join(f"{value:g}" for value in entry["costs"]),
                    "candidate_count": entry["candidate_count"],
                    "family_rule": entry["family_rule"],
                }
            )


def _write_unavailable_csv(path: Path, entries: list[dict[str, Any]]) -> None:
    fields = (
        "name_zh_CN",
        "name_ja_JP",
        "name_en_US",
        "availability",
        "localization_lookup_keys",
    )
    with path.open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        for entry in entries:
            writer.writerow(
                {
                    "name_zh_CN": entry["names"]["zh-CN"],
                    "name_ja_JP": entry["names"]["ja-JP"],
                    "name_en_US": entry["names"]["en-US"],
                    "availability": entry["availability"],
                    "localization_lookup_keys": ";".join(
                        entry["localization_lookup_keys"]
                    ),
                }
            )


def _write_markdown(path: Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Player enemy-combination guide — PC v2.00.02",
        "",
        "This file is generated by `tools/export_enemy_combination_guide.py`. Do not edit it by hand.",
        "",
        "It translates the native role table into player-facing combination rules for the normal scroll UI path (`caller_option = 0`). The application treats selected enemies as **must contain** requirements; a generated scroll may contain additional enemies.",
        "",
        "## How many classes exist?",
        "",
        "The recovered normal dispatcher has exactly three branch classes: 0, 1, and 2. The seven native mode rows are budget/group variants inside those three classes, not seven additional classes. Nonzero `caller_option` paths remain uncertified and are outside this guide.",
        "",
        "| Class | Natural share | Player-facing structure | Native roles | Modes and group budgets |",
        "| ---: | ---: | --- | --- | --- |",
        "| 0 | about 40% | Dedicated pools only. Every candidate costs 4, so it has exactly 2 or 3 entries. The highest entry is B; lower entries choose A or B. | 4/5 | `0x57`: 4,4; `0x6F`: 4,4,4 |",
        "| 1 | about 40% | Ordinary groups plus at most one B requirement in the highest group. | 0-3, then one 5 | `0x4C`: 3,3,4; `0x7D`: 3,3,4,4 |",
        "| 2 | about 20% | Ordinary pools only. | 0-3 | `0x48`: 3,3,4; `0x8E`: 3,3,4,4; `0x62`: 3,3,4,4,5 |",
        "",
        "A group budget is not the displayed enemy count. One group can accept more than one affordable candidate, and exact output still depends on native gates and RNG.",
        "Class 0 lower groups naturally choose A at about 20% and B at about 80% before eligibility filtering; the highest group uses B whenever that pool is available.",
        "",
        "## Player-facing families",
        "",
        "| Family | Meaning | Combination rule |",
        "| --- | --- | --- |",
        "| O | Ordinary pool, native roles 0-3 | Combine with O; optionally combine with one B-assigned requirement through class 1; never combine with A-only. |",
        "| A | Dedicated pool A, native role 4 | Combine only with A/B-family requirements through class 0; at most two A-only requirements fit. |",
        "| B | Dedicated pool B, native role 5 | Combine with A/B through class 0, or with O through class 1 only when the total B assignment is at most one. |",
        "| A/B | One display name with distinct role-4 and role-5 candidates | Use A or B in class 0; use B in class 1. |",
        "",
        "These letters are documentation labels, not official in-game enemy categories.",
        "",
        "## Pairwise compatibility matrix",
        "",
        "| Existing requirement | Add O | Add A | Add B |",
        "| --- | --- | --- | --- |",
        "| O | Compatible through class 1/2 | Impossible | Compatible through class 1 if this is the only B assignment |",
        "| A | Impossible | Compatible through class 0 | Compatible through class 0 |",
        "| B | Compatible through class 1 if no other B is required | Compatible through class 0 | Compatible through class 0 |",
        "",
        "Pairwise compatibility is not enough for three or more requirements. For example, each Tokugawa can pair with Ichimokuren separately, and the two Tokugawas can pair with each other, but all three together are impossible.",
        "",
        "## Quick compatibility table",
        "",
        "| Required family pattern | Structural result | Surviving class |",
        "| --- | --- | --- |",
        "| O only | Compatible | 1 or 2 |",
        "| One to three A/B-family requirements, with at most two A-only | Compatible | 0; a single B or A/B can also use 1 |",
        "| Three A-only requirements | Impossible | None; class 0 reserves one entry for B |",
        "| Four or more A/B-family requirements | Impossible | None; class 0 has at most three entries |",
        "| O + exactly one B or A/B | Compatible | 1 |",
        "| O + two or more B-assigned requirements | Impossible | None |",
        "| O + any A-only requirement | Impossible | None |",
        "",
        "## What 'legal' means",
        "",
        "1. Resolve every selected display name to all candidate keys enabled for the chosen playthrough.",
        "2. Try to assign every requirement to one of the three class structures above.",
        "3. If no class survives, the combination is structurally impossible and no Seed scan is needed.",
        "4. If a class survives, the combination is only structurally compatible. Terrain masks, descriptor and parameter gates, costs, group budgets, linked-group removal, and path-dependent RNG can still reject it.",
        "5. A combination is proven legal only when exact forward replay produces a final list containing all requested enemies. Failing to find a result in a partial batch is not proof of impossibility.",
        "",
        "## Worked example",
        "",
        "`一目连` is O (role 1), while `德川国松` and `德川庆喜` are B (role 5). Class 0 excludes the O requirement, class 1 cannot accept two B requirements, and class 2 excludes both B requirements. The combination is therefore structurally impossible.",
        "",
        "## Trilingual player catalog",
        "",
        f"The catalog contains {len(payload['display_entries'])} player-visible name entries backed by {payload['scope']['candidate_row_count']} native candidate rows. Raw lookup keys remain in the JSON and CSV siblings; this player guide shows only the variant count.",
        "",
    ]

    for family in FAMILY_ORDER:
        entries = [
            entry
            for entry in payload["display_entries"]
            if entry["player_family"] == family
        ]
        lines.extend(
            [
                f"### {FAMILY_TITLES[family]}",
                "",
                FAMILY_NOTES[family],
                "",
                "| Simplified Chinese | Japanese | English | Roles | Playthroughs | Cost(s) | Native variants |",
                "| --- | --- | --- | --- | --- | --- | ---: |",
            ]
        )
        for entry in entries:
            lines.append(
                "| {zh} | {ja} | {en} | {roles} | {playthroughs} | {costs} | {variants} |".format(
                    zh=entry["names"]["zh-CN"],
                    ja=entry["names"]["ja-JP"],
                    en=entry["names"]["en-US"],
                    roles="/".join(map(str, entry["native_roles"])),
                    playthroughs="/".join(
                        map(str, entry["enabled_playthroughs_union"])
                    ),
                    costs="/".join(f"{value:g}" for value in entry["costs"]),
                    variants=entry["candidate_count"],
                )
            )
        lines.append("")

    lines.extend(
        [
            "## Native names that are not legal scroll candidates",
            "",
            f"The native localization catalog contains {len(payload['unavailable_display_entries'])} additional display identities with no row in the current scroll enemy-candidate table. They should be shown as disabled or unavailable, not offered as legal Seed constraints.",
            "",
            "| Simplified Chinese | Japanese | English | Status |",
            "| --- | --- | --- | --- |",
        ]
    )
    for entry in payload["unavailable_display_entries"]:
        lines.append(
            "| {zh} | {ja} | {en} | Not in the scroll candidate table |".format(
                zh=entry["names"]["zh-CN"],
                ja=entry["names"]["ja-JP"],
                en=entry["names"]["en-US"],
            )
        )
    lines.append("")

    lines.extend(
        [
            "## Evidence boundary",
            "",
            "This guide is a structural preflight backed by the native candidate and special-context tables plus recovered class control flow. It does not replace exact Seed replay. A future executable version must receive a new versioned guide even if the localized names appear unchanged.",
        ]
    )
    path.write_text(
        "\n".join(lines) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def _write_chinese_markdown(path: Path, payload: dict[str, Any]) -> None:
    lines = [
        "# 玩家敌人组合指南 — PC v2.00.02",
        "",
        "本文件由 `tools/export_enemy_combination_guide.py` 自动生成，请勿手工修改。",
        "",
        "本指南把游戏原生敌人候选表转换成玩家可读的组合规则。软件中的敌人条件表示“必须至少出现这些敌人”；成品绘卷仍可能包含没有指定的其他敌人。",
        "",
        "## 先看结论",
        "",
        f"- 当前版本共有 {len(payload['display_entries'])} 个可以进入绘卷生成池的玩家可见敌人名称。",
        f"- 另有 {len(payload['unavailable_display_entries'])} 个名称只存在于游戏文本目录、没有绘卷候选行；软件不会把它们提供为合法搜索条件。",
        "- 普通生成路径只有 3 种结构 Class 0、1、2。原生表中的 7 个 mode 是这 3 种结构内部的预算变体，不是另外 7 种 Class。",
        "- 通过下表只能证明组合“结构上可能”；只有完整正向生成确实产出全部目标敌人，才能证明某个 Seed 合法。",
        "",
        "## 三种敌人结构",
        "",
        "| Class | 自然占比 | 玩家可理解的结构 |",
        "| ---: | ---: | --- |",
        "| 0 | 约 40% | 只使用专用池 A/B，共 2 或 3 名敌人；最高组必须能使用 B，纯 A 最多 2 项。 |",
        "| 1 | 约 40% | 普通池 O 为主，最高组最多加入 1 项 B。 |",
        "| 2 | 约 20% | 只使用普通池 O。 |",
        "",
        "## 玩家可读分组",
        "",
        "| 分组 | 含义 | 能否组合 |",
        "| --- | --- | --- |",
        "| O | 普通池，原生 role 0–3 | O 之间可组合；也可通过 Class 1 搭配最多 1 项 B；不能搭配纯 A。 |",
        "| A | 专用池 A，原生 role 4 | 只能通过 Class 0 与 A/B 组合；纯 A 最多 2 项。 |",
        "| B | 专用池 B，原生 role 5 | 可通过 Class 0 与 A/B 组合；与 O 组合时总共最多 1 项 B。 |",
        "| A/B | 同一显示名在 A、B 都有原生候选 | Class 0 可按 A 或 B 使用；Class 1 可按唯一 B 使用。 |",
        "",
        "这些字母只是本项目为了说明组合规则使用的标签，不是游戏官方分类。",
        "",
        "## 快速判断",
        "",
        "| 目标组合 | 结构结论 |",
        "| --- | --- |",
        "| 只有 O | 可能，走 Class 1 或 2。 |",
        "| O + 恰好 1 项 B 或 A/B | 可能，走 Class 1。 |",
        "| O + 任意纯 A | 不可能。 |",
        "| O + 需要 2 项或更多 B | 不可能。 |",
        "| 1–3 项 A/B，且纯 A 不超过 2 项 | 可能，走 Class 0。 |",
        "| 3 项纯 A | 不可能；Class 0 需要为 B 保留最高组。 |",
        "| 4 项或更多 A/B | 不可能；Class 0 最多只有 3 项。 |",
        "",
        "两两都能组合不代表三项一起合法。例如一目连分别可以与德川国松、德川庆喜组成二敌组合，两名德川也能组合，但三者同时要求时没有任何 Class 能容纳。",
        "",
        "## 软件如何判断",
        "",
        "1. 把每个显示名称展开为当前周目可用的全部原生候选行。",
        "2. 保留每个名称可能对应的全部 role，不把一个显示名错误压成单一 role。",
        "3. 依次检查 Class 0、1、2 是否能为每个必含条件分配合法位置。",
        "4. 没有 Class 存活时，直接提示结构无解，不浪费 Seed 计算时间。",
        "5. 有 Class 存活时，再由完整生成器检查周目、地形、参数门、预算、联动组和路径相关随机数。",
        "6. 只有完整正向回放实际包含全部目标敌人时，该 Seed 才进入结果。",
        "",
        "## 中日英敌人目录",
        "",
        "原始 lookup key 与候选行索引保存在同目录 JSON/CSV 中；这份玩家指南只显示名称、分组与可用周目。",
        "",
    ]
    chinese_family_titles = {
        "O": "普通池 O",
        "A": "专用池 A",
        "B": "专用池 B",
        "A/B": "A/B 双候选",
    }
    for family in FAMILY_ORDER:
        entries = [
            entry
            for entry in payload["display_entries"]
            if entry["player_family"] == family
        ]
        lines.extend(
            [
                f"### {chinese_family_titles[family]}",
                "",
                "| 简体中文 | 日文 | 英文 | 原生 role | 可用周目 | 原生变体数 |",
                "| --- | --- | --- | --- | --- | ---: |",
            ]
        )
        for entry in entries:
            lines.append(
                "| {zh} | {ja} | {en} | {roles} | {playthroughs} | {variants} |".format(
                    zh=entry["names"]["zh-CN"],
                    ja=entry["names"]["ja-JP"],
                    en=entry["names"]["en-US"],
                    roles="/".join(map(str, entry["native_roles"])),
                    playthroughs="/".join(
                        map(str, entry["enabled_playthroughs_union"])
                    ),
                    variants=entry["candidate_count"],
                )
            )
        lines.append("")

    lines.extend(
        [
            "## 不应提供给玩家选择的名称",
            "",
            "下列名称存在于游戏本地化目录，但当前绘卷敌人候选表中没有任何对应行。它们不能作为合法 Seed 条件，软件已从选择列表移除。",
            "",
            "| 简体中文 | 日文 | 英文 | 状态 |",
            "| --- | --- | --- | --- |",
        ]
    )
    for entry in payload["unavailable_display_entries"]:
        lines.append(
            "| {zh} | {ja} | {en} | 不在绘卷候选表 |".format(
                zh=entry["names"]["zh-CN"],
                ja=entry["names"]["ja-JP"],
                en=entry["names"]["en-US"],
            )
        )
    lines.extend(
        [
            "",
            "## 证据边界",
            "",
            "本指南的结构判断来自原生候选表、special-context 表和已恢复的 Class 控制流。它不是 Seed 存在性证明，也不能替代精确回放。游戏版本更新后，即使显示名称不变，也必须重新验证候选表与生成逻辑。",
        ]
    )
    path.write_text(
        "\n".join(lines) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def export(output_root: Path) -> dict[str, Any]:
    payload = build_combination_payload()
    output_root.mkdir(parents=True, exist_ok=True)
    _write_json(output_root / "enemy-combinations.json", payload)
    _write_csv(output_root / "enemy-combinations.csv", payload["display_entries"])
    _write_unavailable_csv(
        output_root / "enemy-unavailable.csv",
        payload["unavailable_display_entries"],
    )
    _write_markdown(output_root / "enemy-combinations.md", payload)
    _write_chinese_markdown(output_root / "enemy-combinations.zh-CN.md", payload)
    return payload


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    args = parser.parse_args()
    payload = export(args.output_root.resolve())
    print(
        f"Exported {len(payload['display_entries'])} player entries to "
        f"{args.output_root.resolve()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
