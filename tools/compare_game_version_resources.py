from __future__ import annotations

"""Compare deterministic generation resources across Nioh 3 versions."""

import argparse
import json
import struct
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, Sequence


REPORT_SCHEMA = "nioh3-game-version-resource-comparison/v1"
TRANSIENT_R4_FILES = {"globals/playthrough_progress.bin"}


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return payload


def indexed_files(manifest: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        str(item["filename"]): dict(item)
        for item in manifest.get("files", ())
    }


def compare_r4_resources(
    baseline_manifest_path: Path,
    target_manifest_path: Path,
    baseline_manifest: Mapping[str, Any],
    target_manifest: Mapping[str, Any],
) -> dict[str, Any]:
    baseline_files = indexed_files(baseline_manifest)
    target_files = indexed_files(target_manifest)
    names = sorted(set(baseline_files).union(target_files))
    files = []
    for name in names:
        baseline = baseline_files.get(name)
        target = target_files.get(name)
        equal = bool(
            baseline
            and target
            and int(baseline["size"]) == int(target["size"])
            and str(baseline["sha256"]).upper()
            == str(target["sha256"]).upper()
        )
        files.append(
            {
                "filename": name,
                "kind": "transient_progress" if name in TRANSIENT_R4_FILES else "static",
                "baseline": baseline,
                "target": target,
                "equal": equal,
            }
        )
    static_files = [item for item in files if item["kind"] == "static"]
    transient_files = [item for item in files if item["kind"] == "transient_progress"]
    baseline_constants = baseline_manifest.get("float_constants", {})
    target_constants = target_manifest.get("float_constants", {})
    constant_names = sorted(set(baseline_constants).union(target_constants))
    constants = [
        {
            "name": name,
            "baseline_bits": baseline_constants.get(name, {}).get("bits"),
            "target_bits": target_constants.get(name, {}).get("bits"),
            "equal": (
                baseline_constants.get(name, {}).get("bits")
                == target_constants.get(name, {}).get("bits")
            ),
        }
        for name in constant_names
    ]
    baseline_mode = list(baseline_manifest.get("mode_gate_bytes", ()))
    target_mode = list(target_manifest.get("mode_gate_bytes", ()))
    progress_item = next(
        (item for item in files if item["filename"] in TRANSIENT_R4_FILES),
        None,
    )
    progress_vectors: list[dict[str, Any]] = []
    if progress_item and progress_item["baseline"] and progress_item["target"]:
        baseline_progress = (
            baseline_manifest_path.parent / progress_item["filename"]
        ).read_bytes()
        target_progress = (
            target_manifest_path.parent / progress_item["filename"]
        ).read_bytes()
        if len(baseline_progress) == len(target_progress) == 80:
            baseline_values = struct.unpack("<20I", baseline_progress)
            target_values = struct.unpack("<20I", target_progress)
            for selector in range(1, 6):
                start = (selector - 1) * 4
                baseline_vector = list(baseline_values[start : start + 4])
                target_vector = list(target_values[start : start + 4])
                progress_vectors.append(
                    {
                        "selector": selector,
                        "baseline": baseline_vector,
                        "target": target_vector,
                        "equal": baseline_vector == target_vector,
                    }
                )
    target_selector = int(
        target_manifest.get("source", {}).get("effective_playthrough", 0)
    )
    active_vector = next(
        (item for item in progress_vectors if item["selector"] == target_selector),
        None,
    )
    return {
        "files": files,
        "float_constants": constants,
        "mode_gate_bytes": {
            "baseline": baseline_mode,
            "target": target_mode,
            "equal": baseline_mode == target_mode,
        },
        "static_payloads_equal": bool(static_files)
        and all(item["equal"] for item in static_files)
        and bool(constants)
        and all(item["equal"] for item in constants)
        and baseline_mode == target_mode,
        "playthrough_progress": {
            "target_effective_selector": target_selector,
            "vectors": progress_vectors,
            "all_equal": bool(transient_files)
            and all(item["equal"] for item in transient_files),
            "effective_selector_equal": bool(active_vector and active_vector["equal"]),
        },
        "playthrough_progress_equal": bool(active_vector and active_vector["equal"]),
        "all_captured_playthrough_progress_equal": bool(transient_files)
        and all(item["equal"] for item in transient_files),
    }


def auxiliary_resource_tables(
    manifest: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    raw_tables = manifest.get("tables", {})
    if not isinstance(raw_tables, dict):
        raise ValueError("auxiliary resource tables must be an object")
    return {str(name): dict(value) for name, value in raw_tables.items()}


def capture_tables(manifest: Mapping[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        str(item["name"]): dict(item)
        for item in manifest.get("tables", ())
    }


def compare_auxiliary_tables(
    baseline_manifest: Mapping[str, Any],
    target_capture_manifest: Mapping[str, Any],
) -> dict[str, Any]:
    baseline_tables = auxiliary_resource_tables(baseline_manifest)
    target_tables = capture_tables(target_capture_manifest)
    tables = []
    for name in sorted(baseline_tables):
        baseline = baseline_tables[name]
        target = target_tables.get(name)
        baseline_file = baseline.get("file", {})
        target_file = target.get("rows_blob", {}) if target else {}
        equal = bool(
            target
            and int(baseline["row_size"]) == int(target["row_size"])
            and int(baseline["row_count"]) == int(target["row_count"])
            and int(baseline_file["size"]) == int(target_file["size"])
            and str(baseline_file["sha256"]).upper()
            == str(target_file["sha256"]).upper()
        )
        tables.append(
            {
                "name": name,
                "baseline_row_size": baseline.get("row_size"),
                "target_row_size": target.get("row_size") if target else None,
                "baseline_row_count": baseline.get("row_count"),
                "target_row_count": target.get("row_count") if target else None,
                "baseline_sha256": baseline_file.get("sha256"),
                "target_sha256": target_file.get("sha256"),
                "equal": equal,
            }
        )
    return {
        "tables": tables,
        "all_equal": bool(tables) and all(item["equal"] for item in tables),
    }


def build_report(
    baseline_r4_path: Path,
    target_r4_path: Path,
    baseline_auxiliary_path: Path,
    target_capture_path: Path,
) -> dict[str, Any]:
    baseline_r4 = load_json(baseline_r4_path)
    target_r4 = load_json(target_r4_path)
    baseline_auxiliary = load_json(baseline_auxiliary_path)
    target_capture = load_json(target_capture_path)
    r4 = compare_r4_resources(
        baseline_r4_path,
        target_r4_path,
        baseline_r4,
        target_r4,
    )
    auxiliary = compare_auxiliary_tables(baseline_auxiliary, target_capture)
    return {
        "schema": REPORT_SCHEMA,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "inputs": {
            "baseline_r4": str(baseline_r4_path.resolve()),
            "target_r4": str(target_r4_path.resolve()),
            "baseline_auxiliary": str(baseline_auxiliary_path.resolve()),
            "target_capture": str(target_capture_path.resolve()),
        },
        "r4": r4,
        "auxiliary": auxiliary,
        "gates": {
            "static_generation_resources_equal": (
                r4["static_payloads_equal"] and auxiliary["all_equal"]
            ),
            "playthrough_context_equal": r4["playthrough_progress_equal"],
            "product_enablement_allowed": False,
        },
        "limitations": [
            "Equal tables do not prove that native control flow is unchanged.",
            "Native parity and save-layout validation remain separate gates.",
            "The playthrough context depends on the save loaded by the game process.",
        ],
    }


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare target generation captures with baseline resources"
    )
    parser.add_argument("--baseline-r4", type=Path, required=True)
    parser.add_argument("--target-r4", type=Path, required=True)
    parser.add_argument("--baseline-auxiliary", type=Path, required=True)
    parser.add_argument("--target-capture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.output.exists():
        raise FileExistsError(f"refusing to overwrite existing report: {args.output}")
    report = build_report(
        args.baseline_r4,
        args.target_r4,
        args.baseline_auxiliary,
        args.target_capture,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output.resolve()), **report["gates"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
