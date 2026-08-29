from __future__ import annotations

"""Exact fixed-draw Seed solver with native verification for path-dependent effects.

The solver never scans the 32-bit Seed space sequentially. It builds or reuses
complete 65,536-bucket draw maps, intersects their inverse images, and sends
only the resulting Seeds to the game's native generator for final validation.

This command is read-only with respect to the save. It decrypts a temporary
copy to obtain a canonical record template and never installs a candidate.
"""

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Iterable


PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.app import primary_map_cache_path
from nioh3_scroll_editor.auxiliary_catalog import (
    AuxiliaryNameCatalog,
    load_auxiliary_name_catalog,
)
from nioh3_scroll_editor.auxiliary_feasibility import (
    EnemyKeyRequirement,
    analyze_enemy_feasibility,
)
from nioh3_scroll_editor.auxiliary_generation import AuxiliarySearchCriteria
from nioh3_scroll_editor.catalog import contextual_effect_name
from nioh3_scroll_editor.effect_seed_solver import (
    EffectSeedIntersectionReport,
    EffectSeedRequest,
    collect_effect_seed_page,
)
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity5_effect_sequence,
    generate_ng3_rarity5_primary_effect,
    generate_ng3_rarity5_primary_effect_id,
    generate_ng3_rarity5_primary_effect_ids,
    serialize_ng3_rarity5_effect_slots,
)
from nioh3_scroll_editor.grace_map import (
    GraceMapProgress,
    GraceOutputMap,
    build_live_grace_output_map,
    first_u16_ranges_for_grace,
    load_grace_output_map,
)
from nioh3_scroll_editor.native import NativeBatchOracle, ScanProgress, scan_next_candidate
from nioh3_scroll_editor.primary_map import (
    PrimaryFirstDrawOutputMap,
    PrimaryMapProgress,
    PrimaryOutputMap,
    build_primary_first_draw_output_map,
    build_primary_output_map,
    load_primary_map,
    save_primary_map,
)
from nioh3_scroll_editor.savegame import (
    SaveCrypto,
    SaveInstaller,
    default_crypto_tool,
    discover_save_paths,
)


CATALOG_PATH = PROJECT_ROOT / "nioh3_scroll_editor" / "data" / "effect_names_multilingual.json"
STATE_ROOT = Path(os.environ.get("LOCALAPPDATA", PROJECT_ROOT)) / "Nioh3ScrollGenerator"


def parse_int(value: str) -> int:
    return int(value, 0)


def _normalized_name(value: str) -> str:
    return "".join(value.casefold().split())


def load_effect_name_index() -> dict[str, frozenset[int]]:
    payload = json.loads(CATALOG_PATH.read_text(encoding="utf-8"))
    if payload.get("schema") != "nioh3-effect-localization-catalog/v1":
        raise ValueError("unsupported effect catalog schema")
    grouped: dict[str, set[int]] = {}
    for item in payload.get("effects", {}).values():
        if not isinstance(item, dict):
            continue
        effect_id = int(item["effect_id"])
        for name in item.get("names", {}).values():
            normalized = _normalized_name(str(name))
            if normalized:
                grouped.setdefault(normalized, set()).add(effect_id)
    return {name: frozenset(effect_ids) for name, effect_ids in grouped.items()}


def resolve_effect(value: str, name_index: dict[str, frozenset[int]]) -> int:
    try:
        effect_id = int(value, 0)
    except ValueError:
        matches = name_index.get(_normalized_name(value), frozenset())
        if not matches:
            raise ValueError(f"unknown effect name: {value!r}") from None
        if len(matches) != 1:
            choices = ", ".join(f"0x{effect_id:08X}" for effect_id in sorted(matches))
            raise ValueError(
                f"effect name {value!r} is ambiguous; use one raw ID: {choices}"
            )
        effect_id = next(iter(matches))
    if not 0 <= effect_id <= 0xFFFFFFFF:
        raise ValueError(f"effect ID is outside uint32: {value!r}")
    return effect_id


def resolve_effects(
    values: Iterable[str], name_index: dict[str, frozenset[int]]
) -> frozenset[int]:
    return frozenset(resolve_effect(value, name_index) for value in values)


@dataclass(frozen=True, slots=True)
class AuxiliaryNameIndexes:
    terrain: dict[str, frozenset[int]]
    rules: dict[str, frozenset[int]]
    enemies: dict[str, frozenset[int]]


def _freeze_name_groups(grouped: dict[str, set[int]]) -> dict[str, frozenset[int]]:
    return {name: frozenset(values) for name, values in grouped.items()}


def load_auxiliary_name_indexes() -> AuxiliaryNameIndexes:
    terrain: dict[str, set[int]] = {}
    rules: dict[str, set[int]] = {}
    enemies: dict[str, set[int]] = {}
    for locale in ("zh-CN", "en-US", "ja-JP"):
        catalog = load_auxiliary_name_catalog(locale)
        for entry in catalog.terrain.values():
            name = _normalized_name(str(entry.get("name", "")))
            if not name:
                continue
            for key in entry.get("hash_keys", ()):
                terrain.setdefault(name, set()).add(int(str(key), 16))
        for key, entry in catalog.special_rules.items():
            value = int(key, 16)
            for field in ("display_name", "name"):
                name = _normalized_name(str(entry.get(field, "")))
                if name:
                    rules.setdefault(name, set()).add(value)
        for key, entry in catalog.enemies.items():
            name = _normalized_name(str(entry.get("name", "")))
            if name:
                enemies.setdefault(name, set()).add(int(key, 16))
    return AuxiliaryNameIndexes(
        terrain=_freeze_name_groups(terrain),
        rules=_freeze_name_groups(rules),
        enemies=_freeze_name_groups(enemies),
    )


def resolve_auxiliary_group(
    value: str,
    index: dict[str, frozenset[int]],
    *,
    kind: str,
    max_value: int,
) -> frozenset[int]:
    try:
        raw = int(value, 0)
    except ValueError:
        matches = index.get(_normalized_name(value), frozenset())
        if not matches:
            raise ValueError(f"unknown {kind} name: {value!r}") from None
        return matches
    if not 0 <= raw <= max_value:
        raise ValueError(f"{kind} key is outside its native range: {value!r}")
    return frozenset((raw,))


def resolve_auxiliary_groups(
    values: Iterable[str],
    index: dict[str, frozenset[int]],
    *,
    kind: str,
    max_value: int,
) -> tuple[frozenset[int], ...]:
    return tuple(
        resolve_auxiliary_group(value, index, kind=kind, max_value=max_value)
        for value in values
    )


def choose_save_path(explicit: Path | None) -> Path:
    if explicit is not None:
        path = explicit.resolve()
        if not path.is_file():
            raise FileNotFoundError(path)
        return path
    discovered = discover_save_paths()
    if not discovered:
        raise FileNotFoundError("no Nioh 3 save was found")
    if len(discovered) != 1:
        choices = "\n".join(f"  {path}" for path in discovered)
        raise ValueError(f"multiple saves were found; select one with --save:\n{choices}")
    return discovered[0]


def _progress_line(kind: str, completed: int, total: int) -> None:
    percent = completed * 100.0 / total
    print(f"{kind}: {completed}/{total} ({percent:.1f}%)", file=sys.stderr)


def _load_or_build_grace_map(
    oracle: NativeBatchOracle,
    *,
    template: bytes,
    playthrough: int,
    rarity: int,
    level: int,
    recommended_level: int,
) -> GraceOutputMap:
    if playthrough == 3:
        map_rarity = 4 if rarity == 3 else rarity
        return load_grace_output_map(rarity=map_rarity)
    if playthrough not in (4, 5) or rarity != 5:
        raise ValueError("Grace inversion supports NG3 rarity 3-5 and NG4/NG5 rarity 5")
    return build_live_grace_output_map(
        oracle,
        template=template,
        category=playthrough,
        rarity=rarity,
        level=level,
        recommended_level=recommended_level,
        progress=lambda update: _progress_line(
            "special-result map", update.mapped_buckets, update.total_buckets
        ),
    )


def _load_or_build_primary_map(
    oracle: NativeBatchOracle,
    *,
    template: bytes,
    save_fingerprint: str,
    playthrough: int,
    rarity: int,
    grace_effect_id: int | None,
    grace_mapping: GraceOutputMap | None,
    level: int,
    recommended_level: int,
) -> PrimaryOutputMap | PrimaryFirstDrawOutputMap:
    cache_path = primary_map_cache_path(
        STATE_ROOT,
        save_fingerprint=save_fingerprint,
        playthrough=playthrough,
        rarity=rarity,
        grace_effect_id=grace_effect_id,
    )
    if cache_path.is_file():
        return load_primary_map(
            cache_path,
            expected_context_fingerprint=save_fingerprint,
        )

    progress = lambda update: _progress_line(
        "primary map", update.mapped_buckets, update.total_buckets
    )
    if grace_effect_id is None:
        if playthrough not in (1, 2):
            raise ValueError("primary draw-1 inversion applies only to NG1 and NG2")
        mapping = build_primary_first_draw_output_map(
            oracle,
            template=template,
            category=playthrough,
            rarity=rarity,
            level=level,
            recommended_level=recommended_level,
            progress=progress,
        )
    else:
        if grace_mapping is None:
            raise ValueError("Grace-conditioned primary inversion requires a Grace map")
        mapping = build_primary_output_map(
            oracle,
            template=template,
            grace_effect_id=grace_effect_id,
            mapping=grace_mapping,
            rarity=rarity,
            level=level,
            recommended_level=recommended_level,
            progress=progress,
        )
    save_primary_map(
        cache_path,
        mapping,
        context_fingerprint=save_fingerprint,
    )
    print(f"cached primary map: {cache_path}", file=sys.stderr)
    return mapping


def auxiliary_payload(auxiliary: object, names: AuxiliaryNameCatalog) -> dict[str, object]:
    return {
        "terrain": {
            "row_index": auxiliary.terrain.selected_row_index,
            "value": auxiliary.terrain.value,
            "effect_keys": [
                {"key": f"0x{key:04X}", "name": names.terrain_effect_name(key)}
                for key in auxiliary.terrain.display_effect_keys
            ],
        },
        "enemies": [
            {
                "group": group_index,
                "position": position,
                "lookup_key": f"0x{entry.lookup_key:08X}",
                "name": names.enemy_name(entry.lookup_key),
                "role": entry.role,
            }
            for group_index, group in enumerate(auxiliary.enemies.groups)
            for position, entry in enumerate(group.entries)
        ],
        "special_rules": [
            {
                "position": index,
                "key": f"0x{entry.key:04X}",
                "name": names.special_rule_name(entry.key),
                "display_value": entry.display_value,
                "display_unit": entry.display_unit,
                "display_grade": entry.display_grade,
            }
            for index, entry in enumerate(auxiliary.special_rules.entries)
        ],
    }


def candidate_payload(
    candidate: object,
    playthrough: int,
    auxiliary_names: AuxiliaryNameCatalog,
) -> dict[str, object]:
    effects = [
        {
            "slot": effect.slot,
            "effect_id": f"0x{effect.effect_id:08X}",
            "name": contextual_effect_name(
                effect.effect_id,
                rarity=candidate.rarity,
                slot=effect.slot,
                native_stage_one=candidate.record_stage.value == "native_stage_one",
            ),
            "value": effect.value,
        }
        for effect in candidate.effects
    ]
    payload = {
        "schema": "nioh3-effect-seed-solver-result/v1",
        "game_version": "2.00.02",
        "playthrough": playthrough,
        "seed": candidate.seed,
        "seed_hex": f"0x{candidate.seed:08X}",
        "rarity": candidate.rarity,
        "record_stage": candidate.record_stage.value,
        "joint_search_trial": candidate.joint_search_trial,
        "effects": effects,
        "record_hex": candidate.record.hex(),
    }
    if candidate.auxiliary is not None:
        payload["auxiliary"] = auxiliary_payload(candidate.auxiliary, auxiliary_names)
    return payload


def fixed_candidate_payload(
    candidate: object,
    request: EffectSeedRequest,
    auxiliary_names: AuxiliaryNameCatalog,
) -> dict[str, object]:
    payload: dict[str, object] = {
        "schema": "nioh3-effect-seed-solver-result/v1",
        "game_version": "2.00.02",
        "found": True,
        "verification": (
            "game-closed-ng3-r5-effect-sequence-and-auxiliary"
            if candidate.effect_sequence is not None
            else "fixed-draw-and-auxiliary-only"
        ),
        "playthrough": request.playthrough,
        "rarity": request.rarity,
        "seed": candidate.seed,
        "seed_hex": f"0x{candidate.seed:08X}",
        "joint_search_trial": candidate.pivot_trial,
        "fixed_draws": [
            {"name": name, "draw_index": draw_index}
            for name, draw_index in candidate.fixed_draws
        ],
        "requested": {
            "grace": (
                f"0x{request.grace_effect_id:08X}"
                if request.grace_effect_id is not None
                else None
            ),
            "primary": [f"0x{value:08X}" for value in sorted(request.primary_effect_ids)],
            "required_secondary": [
                f"0x{value:08X}"
                for value in sorted(request.required_secondary_ids)
            ],
        },
    }
    if candidate.auxiliary is not None:
        payload["auxiliary"] = auxiliary_payload(candidate.auxiliary, auxiliary_names)
    if candidate.effect_sequence is not None:
        payload["effect_slots_hex"] = serialize_ng3_rarity5_effect_slots(
            candidate.effect_sequence
        ).hex()
        payload["effects"] = [
            {
                "slot": effect.slot,
                "effect_id": f"0x{effect.effect_id:08X}",
                "name": contextual_effect_name(
                    effect.effect_id,
                    rarity=request.rarity,
                    slot=effect.slot,
                    native_stage_one=False,
                ),
                "roll_percent": effect.roll_percent,
                "resolved_value": effect.resolved_value,
                "category_and_flags": f"0x{effect.category_and_flags:02X}",
                "effect_flags": f"0x{effect.effect_flags:02X}",
            }
            for effect in candidate.effect_sequence.effects
        ]
    return payload


def candidate_batch_payload(
    results: list[dict[str, object]],
    *,
    verification: str,
    requested_results: int,
    resume_trial: int,
) -> dict[str, object]:
    next_resume_trial = resume_trial
    if results:
        trial = results[-1].get("joint_search_trial")
        if isinstance(trial, int):
            next_resume_trial = trial
    return {
        "schema": "nioh3-effect-seed-solver-batch/v1",
        "game_version": "2.00.02",
        "found": bool(results),
        "verification": verification,
        "requested_results": requested_results,
        "result_count": len(results),
        "next_resume_trial": next_resume_trial,
        "results": results,
    }


def intersection_report_payload(
    report: EffectSeedIntersectionReport | None,
) -> dict[str, object] | None:
    if report is None:
        return None
    return {
        "scope": (
            "global_exact_total"
            if report.is_global_total
            else "inspected_range_exact_count"
        ),
        "start_after_trial": report.start_after_trial,
        "inspected_through_trial": report.inspected_through_trial,
        "family_size": report.family_size,
        "fixed_seed_count": report.fixed_seed_count,
        "complete_match_count": report.complete_match_count,
        "exhausted_family": report.exhausted_family,
        "stages": [
            {
                "kind": stage.kind,
                "values": [f"0x{value:08X}" for value in stage.values],
                "cumulative_count": stage.count,
            }
            for stage in report.stages
        ],
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Invert fixed effect draws and native-verify path-dependent Nioh 3 "
            "scroll effects without sequential 32-bit Seed scanning."
        )
    )
    parser.add_argument("--playthrough", type=int, choices=range(1, 6), required=True)
    parser.add_argument("--rarity", type=int, choices=range(0, 6), default=5)
    parser.add_argument("--level", type=int, default=180)
    parser.add_argument("--recommended-level", type=int, default=183)
    parser.add_argument("--grace", help="Grace raw ID or exact zh-CN/en-US/ja-JP name")
    parser.add_argument(
        "--primary",
        action="append",
        default=[],
        help="allowed primary raw ID or exact localized name; repeat for alternatives",
    )
    parser.add_argument(
        "--secondary",
        action="append",
        default=[],
        help="required secondary raw ID or exact localized name; repeat for conjunction",
    )
    parser.add_argument(
        "--terrain",
        action="append",
        default=[],
        help="required terrain effect key or exact localized name; repeat for conjunction",
    )
    parser.add_argument(
        "--rule",
        action="append",
        default=[],
        help="required special-rule key or exact localized name; repeat for conjunction",
    )
    parser.add_argument(
        "--enemy",
        action="append",
        default=[],
        help="required enemy lookup key or exact localized name; repeat for conjunction",
    )
    parser.add_argument(
        "--output-locale",
        choices=("zh-CN", "en-US", "ja-JP"),
        default="zh-CN",
    )
    parser.add_argument("--max-native-candidates", type=int, default=1_000_000)
    parser.add_argument(
        "--max-results",
        type=int,
        default=1,
        help=(
            "return this many matching Seeds; exact NG3 rarity-5 results include "
            "the complete effect sequence and auxiliary preview"
        ),
    )
    parser.add_argument("--resume-trial", type=int, default=0)
    parser.add_argument(
        "--fixed-only",
        action="store_true",
        help=(
            "do not connect to the game; NG3 rarity-5 replays exact effects and "
            "auxiliary outputs without a save or cached primary map"
        ),
    )
    parser.add_argument("--save", type=Path)
    parser.add_argument("--output", type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if not 0 <= args.level <= 0xFFFF or not 0 <= args.recommended_level <= 0xFFFF:
        raise ValueError("level and recommended level must fit in uint16")
    if args.max_native_candidates <= 0:
        raise ValueError("--max-native-candidates must be positive")
    if args.max_results <= 0:
        raise ValueError("--max-results must be positive")
    if args.resume_trial < 0:
        raise ValueError("--resume-trial cannot be negative")

    name_index = load_effect_name_index()
    primary_ids = resolve_effects(args.primary, name_index)
    secondary_ids = resolve_effects(args.secondary, name_index)
    grace_effect_id = (
        resolve_effect(args.grace, name_index) if args.grace is not None else None
    )
    auxiliary_indexes = load_auxiliary_name_indexes()
    terrain_groups = resolve_auxiliary_groups(
        args.terrain,
        auxiliary_indexes.terrain,
        kind="terrain",
        max_value=0xFFFF,
    )
    rule_groups = resolve_auxiliary_groups(
        args.rule,
        auxiliary_indexes.rules,
        kind="special-rule",
        max_value=0xFFFF,
    )
    enemy_groups = resolve_auxiliary_groups(
        args.enemy,
        auxiliary_indexes.enemies,
        kind="enemy",
        max_value=0xFFFFFFFF,
    )
    auxiliary_criteria = AuxiliarySearchCriteria(
        required_terrain_effect_key_groups=terrain_groups,
        required_special_rule_key_groups=rule_groups,
        required_enemy_lookup_key_groups=enemy_groups,
    )
    if enemy_groups:
        feasibility = analyze_enemy_feasibility(
            (
                EnemyKeyRequirement(label, group)
                for label, group in zip(args.enemy, enemy_groups, strict=True)
            ),
            playthrough=args.playthrough,
        )
        if not feasibility.possible:
            raise ValueError(
                "enemy constraints are structurally impossible: "
                + "; ".join(feasibility.reasons)
            )
    if args.playthrough in (1, 2):
        if not primary_ids or grace_effect_id is not None:
            raise ValueError("NG1/NG2 requires a primary constraint and has no Grace constraint")
    elif grace_effect_id is None:
        raise ValueError("NG3-NG5 requires a Grace/special-result constraint")

    exact_game_closed = args.fixed_only and (args.playthrough, args.rarity) == (3, 5)
    inventory = None
    template = None
    save_fingerprint = None
    if not exact_game_closed:
        save_path = choose_save_path(args.save)
        crypto = SaveCrypto(default_crypto_tool(PROJECT_ROOT))
        installer = SaveInstaller(save_path=save_path, crypto=crypto, state_root=STATE_ROOT)
        inventory = installer.capture_inventory()
        template = inventory.template_record_for_playthrough(args.playthrough)
        save_fingerprint = hashlib.sha256(inventory.decrypted).hexdigest()

    if args.fixed_only:
        if secondary_ids and (args.playthrough, args.rarity) != (3, 5):
            raise ValueError(
                "--fixed-only secondary verification currently supports only NG3 rarity 5"
            )
        if args.playthrough > 3:
            raise ValueError("--fixed-only currently has no persisted NG4/NG5 Grace map")
        grace_mapping = None
        if grace_effect_id is not None:
            map_rarity = 4 if args.rarity == 3 else args.rarity
            grace_mapping = load_grace_output_map(rarity=map_rarity)
        primary_mapping = None
        uses_exact_effect_replay = (args.playthrough, args.rarity) == (3, 5)
        if primary_ids and not uses_exact_effect_replay:
            assert save_fingerprint is not None
            cache_path = primary_map_cache_path(
                STATE_ROOT,
                save_fingerprint=save_fingerprint,
                playthrough=args.playthrough,
                rarity=args.rarity,
                grace_effect_id=grace_effect_id,
            )
            if not cache_path.is_file():
                raise FileNotFoundError(
                    "the certified primary map is not cached; run once without "
                    "--fixed-only while the game is at the title screen: "
                    f"{cache_path}"
                )
            primary_mapping = load_primary_map(
                cache_path,
                expected_context_fingerprint=save_fingerprint,
            )
        request = EffectSeedRequest(
            playthrough=args.playthrough,
            rarity=args.rarity,
            primary_effect_ids=primary_ids,
            required_secondary_ids=secondary_ids,
            grace_effect_id=grace_effect_id,
            auxiliary_criteria=auxiliary_criteria,
        )
        fixed_page = collect_effect_seed_page(
            request,
            page_size=args.max_results,
            grace_mapping=grace_mapping,
            primary_mapping=(
                primary_mapping if isinstance(primary_mapping, PrimaryOutputMap) else None
            ),
            primary_first_mapping=(
                primary_mapping
                if isinstance(primary_mapping, PrimaryFirstDrawOutputMap)
                else None
            ),
            effect_sequence_generator=(
                (
                    lambda seed: generate_ng3_rarity5_effect_sequence(
                        seed,
                        level=args.level,
                        grace_mapping=grace_mapping,
                    )
                )
                if uses_exact_effect_replay
                else None
            ),
            primary_effect_generator=(
                (
                    lambda seed: generate_ng3_rarity5_primary_effect(
                        seed,
                        level=args.level,
                        grace_mapping=grace_mapping,
                    )
                )
                if uses_exact_effect_replay
                else None
            ),
            primary_effect_id_generator=(
                (
                    lambda seed: generate_ng3_rarity5_primary_effect_id(
                        seed,
                        grace_mapping=grace_mapping,
                    )
                )
                if uses_exact_effect_replay
                else None
            ),
            primary_effect_id_batch_generator=(
                (
                    lambda seeds: generate_ng3_rarity5_primary_effect_ids(
                        seeds,
                        grace_id=grace_effect_id,
                        grace_mapping=grace_mapping,
                    )
                )
                if uses_exact_effect_replay and grace_effect_id is not None
                else None
            ),
            start_after_trial=args.resume_trial,
            max_trials=args.max_native_candidates,
        )
        fixed_results = list(fixed_page.candidates)
        if not fixed_results:
            payload = {
                "schema": "nioh3-effect-seed-solver-result/v1",
                "found": False,
                "verification": (
                    "game-closed-ng3-r5-effect-sequence-and-auxiliary"
                    if uses_exact_effect_replay
                    else "fixed-draw-and-auxiliary-only"
                ),
                "resume_trial": args.resume_trial,
                "next_resume_trial": fixed_page.next_start_after_trial,
                "checked_trials": args.max_native_candidates,
                "intersection": intersection_report_payload(
                    fixed_page.intersection_report
                ),
            }
            exit_code = 2
        else:
            auxiliary_names = load_auxiliary_name_catalog(args.output_locale)
            result_payloads = [
                fixed_candidate_payload(candidate, request, auxiliary_names)
                for candidate in fixed_results
            ]
            payload = (
                result_payloads[0]
                if args.max_results == 1
                else candidate_batch_payload(
                    result_payloads,
                    verification=(
                        "game-closed-ng3-r5-effect-sequence-and-auxiliary"
                        if uses_exact_effect_replay
                        else "fixed-draw-and-auxiliary-only"
                    ),
                    requested_results=args.max_results,
                    resume_trial=args.resume_trial,
                )
            )
            if args.max_results > 1:
                payload["next_resume_trial"] = fixed_page.next_start_after_trial
            payload["intersection"] = intersection_report_payload(
                fixed_page.intersection_report
            )
            exit_code = 0
        serialized = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
        if args.output is not None:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(serialized, encoding="utf-8")
        print(serialized, end="")
        return exit_code

    assert template is not None
    assert save_fingerprint is not None
    with NativeBatchOracle(max_batch_size=2048) as oracle:
        grace_mapping = None
        if grace_effect_id is not None:
            grace_mapping = _load_or_build_grace_map(
                oracle,
                template=template,
                playthrough=args.playthrough,
                rarity=args.rarity,
                level=args.level,
                recommended_level=args.recommended_level,
            )
            first_u16_ranges_for_grace(grace_effect_id, grace_mapping)

        primary_mapping = None
        if primary_ids:
            primary_mapping = _load_or_build_primary_map(
                oracle,
                template=template,
                save_fingerprint=save_fingerprint,
                playthrough=args.playthrough,
                rarity=args.rarity,
                grace_effect_id=grace_effect_id,
                grace_mapping=grace_mapping,
                level=args.level,
                recommended_level=args.recommended_level,
            )

        scan_kwargs: dict[str, object] = {
            "oracle": oracle,
            "template": template,
            "start_seed": 0,
            "primary_effect_ids": primary_ids,
            "required_secondary_ids": secondary_ids,
            "grace_effect_id": grace_effect_id,
            "rarity": args.rarity,
            "level": args.level,
            "recommended_level": args.recommended_level,
            "playthrough": args.playthrough,
            "max_seeds": args.max_native_candidates,
            "auxiliary_criteria": auxiliary_criteria,
            "progress": lambda update: _progress_line(
                "native verification", update.scanned, args.max_native_candidates
            ),
        }
        if grace_mapping is not None:
            scan_kwargs["grace_output_map"] = grace_mapping
        if isinstance(primary_mapping, PrimaryOutputMap):
            scan_kwargs["primary_output_map"] = primary_mapping
        elif isinstance(primary_mapping, PrimaryFirstDrawOutputMap):
            scan_kwargs["primary_first_output_map"] = primary_mapping

        candidates = []
        joint_cursor = args.resume_trial
        grace_cursor = None
        uses_primary_map = isinstance(
            primary_mapping, (PrimaryOutputMap, PrimaryFirstDrawOutputMap)
        )
        if args.resume_trial and not uses_primary_map:
            raise ValueError("--resume-trial requires a primary constraint")
        for _ in range(args.max_results):
            scan_kwargs["joint_start_after_trial"] = joint_cursor if uses_primary_map else 0
            scan_kwargs["grace_start_after_seed"] = (
                grace_cursor if not uses_primary_map else None
            )
            candidate = scan_next_candidate(**scan_kwargs)
            if candidate is None:
                break
            candidates.append(candidate)
            if uses_primary_map:
                if (
                    not isinstance(candidate.joint_search_trial, int)
                    or candidate.joint_search_trial <= joint_cursor
                ):
                    raise RuntimeError("native solver returned a non-advancing trial cursor")
                joint_cursor = candidate.joint_search_trial
            else:
                grace_cursor = candidate.seed

    if not candidates:
        payload: dict[str, object] = {
            "schema": "nioh3-effect-seed-solver-result/v1",
            "found": False,
            "resume_trial": args.resume_trial,
            "checked_native_candidates": args.max_native_candidates,
        }
    else:
        auxiliary_names = load_auxiliary_name_catalog(args.output_locale)
        result_payloads = [
            candidate_payload(candidate, args.playthrough, auxiliary_names)
            for candidate in candidates
        ]
        for result in result_payloads:
            result["found"] = True
        payload = (
            result_payloads[0]
            if args.max_results == 1
            else candidate_batch_payload(
                result_payloads,
                verification="native-full-record",
                requested_results=args.max_results,
                resume_trial=args.resume_trial,
            )
        )

    serialized = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized, encoding="utf-8")
    print(serialized, end="")
    return 0 if candidates else 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error
