from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DELIVERY_NAME = "Nioh3_Generation_Inversion_Pro_Handoff_20260830"
OUTPUT_ROOT = ROOT / "deliverables" / DELIVERY_NAME
ZIP_PATH = ROOT / "deliverables" / f"{DELIVERY_NAME}.zip"


FILE_PATHS = [
    "nioh3_seed_math.py",
    "emaki_exchange.py",
    "research/EFFECT_GENERATION_TABLE_INDEX.md",
    "research/EFFECT_SEED_SOLVER.md",
    "research/RARITY4_FINAL_GRACE_PREDICTION.md",
    "research/native_seed_accelerator.cu",
    "research/native_enemy_matcher.cuh",
    "research/solve_effect_seed.py",
    "audit/parity/ng3-r3-live-parity-20260829.json",
    "audit/parity/ng3-r4-live-parity-20260829.json",
    "audit/ng3-r5-native-parity-live-10000-20260829.json",
    "audit/seed-solver-cuda-benchmark-20260829.json",
    "audit/enemy-combination-feasibility-ichimokuren-tokugawa-20260829.json",
    "audit/p1_dynamic/native_auxiliary_mode_corpus_20260828_234502.json",
    "audit/p1_static/COMPLETE_AUXILIARY_PARITY_20260829.md",
    "audit/p1_static/P1_FREEZE_AND_NEXT_BOUNDARY_20260829.md",
    "audit/p1_static/SPECIAL_RULE_VALUE_ANALYSIS_20260829.md",
    "audit/p1_static/CLASS1_ENEMY_PARITY_20260828.md",
    "audit/p1_static/auxiliary_generator_subsystems_20260828.json",
    "audit/p1_static/auxiliary_enemy_generator_body2_20260828.json",
    "audit/p1_static/auxiliary_rule_helpers_20260828.json",
    "audit/p1_static/r4_per_effect_finalizer_1109270_20260829.json",
    "audit/p1_static/r4_finalizer_helper_functions_20260829.json",
    "audit/p1_static/r5_effect_main_and_helpers_20260829.json",
    "audit/p1_static/r5_effect_generator_helpers_20260829.json",
    "audit/p1_static/enemy_classes_0_2_102AA90_102BEC5.asm",
    "audit/p1_static/enemy_1029990_102A260.asm",
    "audit/p1_static/enemy_1029440_1029C00.asm",
    "audit/runtime_sections/v2.00.02_20260827_title/manifest.json",
    "audit/runtime_sections/v2.00.02_20260827_title/Nioh3_v2.00.02.text.bin",
    "audit/runtime_sections/v2.00.02_20260827_title/Nioh3_v2.00.02.rdata.bin",
    "audit/runtime_sections/v2.00.02_20260827_title/Nioh3_v2.00.02.pdata.bin",
]


TREE_PATHS = [
    "research/pro_handoff",
    "docs/knowledge",
    "test_fixtures/r4_native_corpus",
    "audit/r4_finalizer_capture/seed_183696634_20260827/completion_loop",
    "audit/p1_dynamic/auxiliary_generation_tables_20260828_150605",
    "audit/p1_dynamic/enemy_parameter_gate_20260828_231828",
    "nioh3_scroll_editor/data/r4_finalizer/pc_v2_00_02/resource_v1",
    "nioh3_scroll_editor/data/auxiliary_generation/pc_v2_00_02/resource_v3",
]


MODULE_PATHS = [
    "nioh3_scroll_editor/__init__.py",
    "nioh3_scroll_editor/models.py",
    "nioh3_scroll_editor/native.py",
    "nioh3_scroll_editor/catalog.py",
    "nioh3_scroll_editor/effect_catalog.py",
    "nioh3_scroll_editor/auxiliary_catalog.py",
    "nioh3_scroll_editor/effect_sequence.py",
    "nioh3_scroll_editor/effect_generation_tables.py",
    "nioh3_scroll_editor/effect_seed_solver.py",
    "nioh3_scroll_editor/auxiliary_generation.py",
    "nioh3_scroll_editor/auxiliary_feasibility.py",
    "nioh3_scroll_editor/seed_accelerator.py",
    "nioh3_scroll_editor/r4_finalizer_engine.py",
    "nioh3_scroll_editor/r4_finalizer_reference.py",
    "nioh3_scroll_editor/r4_finalizer_resource.py",
    "nioh3_scroll_editor/r4_table_bundle.py",
    "nioh3_scroll_editor/grace_map.py",
    "nioh3_scroll_editor/primary_map.py",
    "nioh3_scroll_editor/recommended_level.py",
    "nioh3_scroll_editor/data/recommended_level_curve.json",
    "nioh3_scroll_editor/data/effect_names_multilingual.json",
    "nioh3_scroll_editor/data/auxiliary_names/zh-CN.json",
    "nioh3_scroll_editor/data/auxiliary_names/ja-JP.json",
    "nioh3_scroll_editor/data/auxiliary_names/en-US.json",
    "nioh3_scroll_editor/data/grace_output_map_e604_r4_current.json",
    "nioh3_scroll_editor/data/grace_output_map_e604_r5_current.json",
    "nioh3_scroll_editor/data/grace_names_zh_cn.json",
    "nioh3_scroll_editor/data/special_rule_item_names.json",
]


TEST_PATHS = [
    "test_auxiliary_catalog.py",
    "test_auxiliary_generation.py",
    "test_effect_catalog.py",
    "test_effect_generation_tables.py",
    "test_effect_seed_solver.py",
    "test_effect_sequence.py",
    "test_effect_solver_cli.py",
    "test_enemy_role_catalog.py",
    "test_grace_accelerated_scanner.py",
    "test_grace_map.py",
    "test_joint_solver.py",
    "test_r4_batch_wrapper.py",
    "test_r4_finalizer_engine.py",
    "test_r4_finalizer_reference.py",
    "test_r4_finalizer_resource.py",
    "test_r4_table_bundle.py",
    "test_recommended_level.py",
]


def _git(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, encoding="utf-8"
    ).strip()


def _copy_file(relative_path: str) -> None:
    source = ROOT / relative_path
    if not source.is_file():
        raise FileNotFoundError(relative_path)
    destination = OUTPUT_ROOT / relative_path
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def _copy_tree(relative_path: str) -> None:
    source = ROOT / relative_path
    if not source.is_dir():
        raise FileNotFoundError(relative_path)
    destination = OUTPUT_ROOT / relative_path
    shutil.copytree(source, destination, dirs_exist_ok=True)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def main() -> int:
    if OUTPUT_ROOT.exists():
        shutil.rmtree(OUTPUT_ROOT)
    if ZIP_PATH.exists():
        ZIP_PATH.unlink()
    OUTPUT_ROOT.mkdir(parents=True)

    for relative_path in FILE_PATHS + MODULE_PATHS + TEST_PATHS:
        _copy_file(relative_path)
    for relative_path in TREE_PATHS:
        _copy_tree(relative_path)

    entries = []
    for path in sorted(OUTPUT_ROOT.rglob("*")):
        if not path.is_file() or path.name == "MANIFEST.json":
            continue
        entries.append(
            {
                "path": path.relative_to(OUTPUT_ROOT).as_posix(),
                "bytes": path.stat().st_size,
                "sha256": _sha256(path),
            }
        )

    manifest = {
        "schema": "nioh3-generation-inversion-pro-handoff/v1",
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "game_version": "PC v2.00.02",
        "source_commit": _git("rev-parse", "HEAD"),
        "source_branch": _git("branch", "--show-current"),
        "private_research_only": True,
        "raw_user_saves_included": False,
        "account_identifiers_included": False,
        "file_count": len(entries),
        "total_bytes": sum(entry["bytes"] for entry in entries),
        "files": entries,
    }
    manifest_path = OUTPUT_ROOT / "MANIFEST.json"
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    with zipfile.ZipFile(ZIP_PATH, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(OUTPUT_ROOT.rglob("*")):
            if path.is_file():
                archive.write(
                    path,
                    Path(DELIVERY_NAME) / path.relative_to(OUTPUT_ROOT),
                )

    summary = {
        "directory": str(OUTPUT_ROOT),
        "zip": str(ZIP_PATH),
        "zip_bytes": ZIP_PATH.stat().st_size,
        "zip_sha256": _sha256(ZIP_PATH),
        "manifest_files": manifest["file_count"],
        "manifest_bytes": manifest["total_bytes"],
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
