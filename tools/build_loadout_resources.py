"""Project verified PC v2.02 HP/Ki research into the read-only planner."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import sys


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--research-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--validation-output", type=Path, required=True)
    args = parser.parse_args()
    if args.validation_output.exists():
        raise FileExistsError(args.validation_output)
    root = args.research_root.resolve()
    sys.path.insert(0, str(root))
    from research.native_bonus_curve import NativeBonusCurve
    from nioh3_scroll_editor.r4_finalizer_reference import f32, f32_mul, f32_div
    from nioh3_scroll_editor.r4_finalizer_resource import load_r4_finalizer_resource_for_version

    reference = module("resource_reference", root / "deliverables/player-hp-ki-research-20261014/reference.py")
    life = module("life_reference", root / "deliverables/life-reflection-stamina-research-20261014/analyze_effect.py")
    rows, metadata = reference.load_table()
    research = root / "deliverables/life-reflection-stamina-research-20261014"
    qualified = life.analyze(research / "native-tables-01")
    positive = life.replay_live(qualified, research / "live-bplus-01")
    if (positive["base_hp"], positive["extra_hp"], positive["maximum_hp"]) != (880, 135, 1015):
        raise ValueError("Qualified positive control no longer reproduces")
    resource = load_r4_finalizer_resource_for_version((2, 0, 2, 0))
    curve = NativeBonusCurve(resource.bonus_curve_entry(i).row for i in range(resource.manifest["bonus_curve"]["entry_count"]))
    modifiers_path = root / "deliverables/player-resource-modifiers-research-20261014/analysis.json"
    modifiers = json.loads(modifiers_path.read_text(encoding="utf-8"))
    if modifiers["identity"]["executable_sha256"] != reference.EXE_SHA256:
        raise ValueError("Modifier and base versions differ")
    if not all(modifiers["checks"][key] for key in ("all_native_curve_rows_match_resource", "all_consumed_blob_hashes_match", "all_instruction_bytes_match")):
        raise ValueError("Unqualified modifier report")
    for entry in modifiers["amrita_ki_curve"]["rows"]:
        row = curve.rows[entry["index"]]
        if row.key != 0xCBE1 or row.point != entry["point"] or list(row.samples) != entry["samples"]:
            raise ValueError("Amrita report/resource row mismatch")
    life_percent = [curve.evaluate(0xFA3E, 1, grade) for grade in range(18)]
    gauge_start = [curve.evaluate(0xCBE1, 0, grade) for grade in range(18)]
    gauge_end = [curve.evaluate(0xCBE1, 1000, grade) for grade in range(18)]
    if [row.point for row in curve.rows if row.key == 0xCBE1] != [0, 1000]:
        raise ValueError("Amrita curve acquired unsupported intermediate knots")
    base_report_path = root / "deliverables/player-hp-ki-research-20261014/base-curves.json"
    base_report = json.loads(base_report_path.read_text(encoding="utf-8"))
    if base_report["source_table_sha256"] != metadata["table_sha256"]:
        raise ValueError("Base report/raw table mismatch")
    for row in base_report["curves"]:
        raw = rows[row["index"]]
        if [row[key] for key in ("level_hp", "constitution_hp", "heart_ki", "stamina_hp")] != [raw[i] for i in (0, 1, 2, 6)]:
            raise ValueError("Base report semantic columns differ from raw rows")
    files = [base_report_path, research / "effect-analysis-with-live.json", modifiers_path]
    output = dict(schema="nioh3-loadout-resources/v1", executable_sha256=reference.EXE_SHA256,
        attribute_table_sha256=metadata["table_sha256"],
        evidence={path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in files},
        level_hp=[row[0] for row in rows[1:451]],
        constitution_hp=[row[1] for row in rows[1:151]],
        heart_ki=[row[2] for row in rows[1:151]],
        stamina_hp=[row[6] for row in rows[1:151]],
        life_reflection_percent=life_percent, amrita_ki_start=gauge_start, amrita_ki_full=gauge_end)
    base_vectors = []
    for level in range(1, 451):
        c, h, s = 5 + level % 146, 5 + level * 7 % 146, 5 + level * 13 % 146
        value = reference.calculate(rows, level=level, constitution=c, heart=h, stamina=s)
        base_vectors.append([level, c, h, s, value["base_hp"], value["base_ki"]])
    life_vectors = [[s, grade, math.trunc(f32_mul(f32_mul(life_percent[grade], f32(.01)), rows[s][6]))]
                    for s in range(5, 151) for grade in range(18)]
    gauge_vectors = []
    for percent in [*range(101), .1, 33.3, 66.6, 99.9]:
        point = math.trunc(f32_mul(f32_div(percent, 100), 1000))
        for grade in range(18):
            gauge_vectors.append([percent, grade, math.trunc(curve.evaluate(0xCBE1, point, grade))])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    args.validation_output.parent.mkdir(parents=True, exist_ok=True)
    args.validation_output.write_text(json.dumps(dict(base_vectors=base_vectors, life_vectors=life_vectors,
        gauge_vectors=gauge_vectors, positive_control=positive, product_sha256=hashlib.sha256(args.output.read_bytes()).hexdigest()),
        ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(json.dumps(dict(base_vectors=len(base_vectors), life_vectors=len(life_vectors), gauge_vectors=len(gauge_vectors))))


if __name__ == "__main__":
    main()
