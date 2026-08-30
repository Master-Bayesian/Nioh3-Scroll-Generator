"""Build the deterministic PC v2.00.02 knowledge-catalog manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    REPOSITORY_ROOT
    / "docs"
    / "knowledge"
    / "versions"
    / "pc-v2.00.02"
    / "catalogs"
    / "catalog-manifest.json"
)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def _relative(path: Path) -> str:
    return path.relative_to(REPOSITORY_ROOT).as_posix()


def _file_entry(path: Path) -> dict[str, object]:
    return {
        "path": _relative(path),
        "size": path.stat().st_size,
        "sha256": _sha256(path),
    }


def build_manifest() -> dict[str, object]:
    data_root = REPOSITORY_ROOT / "nioh3_scroll_editor" / "data"
    knowledge_catalog_root = DEFAULT_OUTPUT.parent

    effect_path = data_root / "effect_names_multilingual.json"
    effect_payload = json.loads(effect_path.read_text(encoding="utf-8"))

    auxiliary: dict[str, object] = {}
    for locale in ("zh-CN", "ja-JP", "en-US"):
        path = data_root / "auxiliary_names" / f"{locale}.json"
        payload = json.loads(path.read_text(encoding="utf-8"))
        auxiliary[locale] = {
            **_file_entry(path),
            "schema": payload["schema"],
            "coverage": payload["coverage"],
            "resolved_names": {
                "terrain": sum(
                    bool(entry.get("name")) for entry in payload["terrain"].values()
                ),
                "special_rules": sum(
                    bool(entry.get("name") or entry.get("display_name"))
                    for entry in payload["special_rules"].values()
                ),
                "enemies": sum(
                    bool(entry.get("name")) for entry in payload["enemies"].values()
                ),
            },
            "provenance": "native_localization_pool",
        }

    enemy_role_path = knowledge_catalog_root / "enemy-roles.json"
    enemy_role_payload = json.loads(enemy_role_path.read_text(encoding="utf-8"))
    enemy_combination_path = knowledge_catalog_root / "enemy-combinations.json"
    enemy_combination_payload = json.loads(
        enemy_combination_path.read_text(encoding="utf-8")
    )

    resource_paths = {
        "recommended_level_curve": data_root / "recommended_level_curve.json",
        "effect_finalizer": (
            data_root
            / "r4_finalizer"
            / "pc_v2_00_02"
            / "resource_v1"
            / "manifest.json"
        ),
        "auxiliary_generation": (
            data_root
            / "auxiliary_generation"
            / "pc_v2_00_02"
            / "resource_v3"
            / "manifest.json"
        ),
        "grace_r4_stage_one": data_root / "grace_output_map_e604_r4_current.json",
        "grace_r5_research": data_root / "grace_output_map_e604_r5_current.json",
        "special_rule_item_names": data_root / "special_rule_item_names.json",
    }

    return {
        "schema": "nioh3-scroll-knowledge-catalog-manifest/v1",
        "game_version": "PC v2.00.02",
        "catalogs": {
            "final_effects": {
                **_file_entry(effect_path),
                "schema": effect_payload["schema"],
                "effect_count": effect_payload["effect_count"],
                "locales": effect_payload["locales"],
                "resolved_names": {
                    locale: sum(
                        bool(effect.get("names", {}).get(locale))
                        for effect in effect_payload["effects"].values()
                    )
                    for locale in effect_payload["locales"]
                },
                "context_rule": effect_payload.get("context_rule"),
            },
            "auxiliary_names": auxiliary,
            "enemy_roles": {
                "schema": enemy_role_payload["schema"],
                "candidate_rows": len(enemy_role_payload["rows"]),
                "role_counts": enemy_role_payload["role_counts"],
                "files": {
                    suffix: _file_entry(knowledge_catalog_root / f"enemy-roles.{suffix}")
                    for suffix in ("json", "csv", "md")
                },
                "source_table": enemy_role_payload["source"]["candidate_table"],
            },
            "enemy_combinations": {
                "schema": enemy_combination_payload["schema"],
                "display_entries": len(
                    enemy_combination_payload["display_entries"]
                ),
                "unavailable_display_entries": len(
                    enemy_combination_payload["unavailable_display_entries"]
                ),
                "recovered_branch_classes": enemy_combination_payload["scope"][
                    "recovered_branch_classes"
                ],
                "family_counts": {
                    family: details["display_entry_count"]
                    for family, details in enemy_combination_payload[
                        "family_definitions"
                    ].items()
                },
                "files": {
                    suffix: _file_entry(
                        knowledge_catalog_root / f"enemy-combinations.{suffix}"
                    )
                    for suffix in ("json", "csv", "md")
                },
                "player_guide_zh_CN": _file_entry(
                    knowledge_catalog_root / "enemy-combinations.zh-CN.md"
                ),
                "unavailable_file": _file_entry(
                    knowledge_catalog_root / "enemy-unavailable.csv"
                ),
                "legality_boundary": enemy_combination_payload[
                    "meaning_of_legality"
                ],
            },
        },
        "machine_resources": {
            name: {
                **_file_entry(path),
                **(
                    {
                        "status": "stage-one exact partition; final R4 Grace requires finalizer replay"
                    }
                    if name == "grace_r4_stage_one"
                    else {
                        "status": "technically parity-backed; product hidden; legacy current filename and incomplete map capture provenance"
                    }
                    if name == "grace_r5_research"
                    else {}
                ),
            }
            for name, path in resource_paths.items()
        },
        "rules": {
            "version_scoped": True,
            "unknown_names_remain_numeric": True,
            "stage_one_tokens_are_not_final_effect_names": True,
            "display_name_does_not_imply_single_enemy_role": True,
        },
    }


def export(output: Path) -> dict[str, object]:
    manifest = build_manifest()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    manifest = export(args.output.resolve())
    print(
        f"Exported {manifest['schema']} to {args.output.resolve()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
