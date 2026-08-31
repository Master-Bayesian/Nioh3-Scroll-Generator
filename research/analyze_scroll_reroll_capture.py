from __future__ import annotations

import argparse
from itertools import combinations
import json
from pathlib import Path
import struct
import sys
from typing import Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor.reroll import predict_reroll_candidates


MANIFEST_COLUMNS = (
    "event",
    "rva",
    "record",
    "seed",
    "counter",
    "slot",
    "output",
    "count",
    "candidates",
    "dump",
)


def _subsets(values: Sequence[int]) -> Iterable[tuple[int, ...]]:
    for count in range(len(values) + 1):
        yield from combinations(values, count)


def _parse_manifest(path: Path) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        if not raw_line or raw_line.startswith("#"):
            continue
        fields = raw_line.split("\t")
        if len(fields) != len(MANIFEST_COLUMNS):
            raise ValueError(
                f"{path}: expected {len(MANIFEST_COLUMNS)} fields, got {len(fields)}"
            )
        rows.append(dict(zip(MANIFEST_COLUMNS, fields, strict=True)))
    return rows


def _parse_native_candidates(path: Path, count: int) -> tuple[tuple[int, int], ...]:
    data = path.read_bytes()
    if len(data) != count * 0x18:
        raise ValueError(
            f"{path}: expected {count * 0x18:#x} bytes, got {len(data):#x}"
        )
    return tuple(
        (
            struct.unpack_from("<I", data, index * 0x18 + 0x04)[0],
            data[index * 0x18 + 0x0C],
        )
        for index in range(count)
    )


def analyze_capture(root: str | Path) -> dict[str, object]:
    capture_root = Path(root)
    rows = _parse_manifest(capture_root / "manifest.tsv")
    latest_entry: dict[str, str] | None = None
    comparisons: list[dict[str, object]] = []
    for row in rows:
        if row["event"] == "candidate_entry":
            latest_entry = row
            continue
        if row["event"] != "candidate_return" or not row["dump"]:
            continue
        if latest_entry is None or not latest_entry["dump"]:
            raise ValueError("candidate_return has no preceding candidate_entry dump")
        record = (capture_root / latest_entry["dump"]).read_bytes()
        slot_index = int(latest_entry["slot"], 0)
        native_count = int(row["count"], 0)
        native = _parse_native_candidates(capture_root / row["dump"], native_count)

        incomplete = predict_reroll_candidates(record, slot_index)
        conditional = incomplete.dynamic_gate_group_keys
        matching_contexts: list[tuple[int, ...]] = []
        predicted_by_context: list[dict[str, object]] = []
        for enabled in _subsets(conditional):
            prediction = predict_reroll_candidates(
                record,
                slot_index,
                dynamic_gate_group_keys=enabled,
            )
            vector = tuple(
                (candidate.effect_id, candidate.roll_percent)
                for candidate in prediction.candidates
            )
            matches = vector == native
            if matches:
                matching_contexts.append(enabled)
            predicted_by_context.append(
                {
                    "enabled_dynamic_group_keys": [
                        f"0x{value:04X}" for value in enabled
                    ],
                    "candidate_vector": [
                        {"effect_id": f"0x{effect_id:04X}", "roll_percent": roll}
                        for effect_id, roll in vector
                    ],
                    "matches_native": matches,
                }
            )

        comparisons.append(
            {
                "displayed_seed": struct.unpack_from("<I", record, 0x20)[0],
                "reroll_counter": struct.unpack_from("<H", record, 0x0C)[0],
                "selected_slot": slot_index + 1,
                "native_candidate_vector": [
                    {"effect_id": f"0x{effect_id:04X}", "roll_percent": roll}
                    for effect_id, roll in native
                ],
                "conditional_dynamic_group_keys": [
                    f"0x{value:04X}" for value in conditional
                ],
                "matching_dynamic_contexts": [
                    [f"0x{value:04X}" for value in enabled]
                    for enabled in matching_contexts
                ],
                "parity": bool(matching_contexts),
                "context_uniquely_identified": len(matching_contexts) == 1,
                "context_trials": predicted_by_context,
            }
        )
        latest_entry = None

    return {
        "schema": "nioh3-scroll-reroll-capture-analysis/v1",
        "game_version": "PC v2.00.02",
        "comparison_count": len(comparisons),
        "all_candidate_vectors_match": bool(comparisons)
        and all(item["parity"] for item in comparisons),
        "comparisons": comparisons,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compare a CE reroll capture with the offline PC v2.00.02 predictor"
    )
    parser.add_argument("capture", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    report = analyze_capture(args.capture)
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    output = args.output or (args.capture / "analysis.json")
    output.write_text(rendered, encoding="utf-8")
    print(output)
    print(f"all_candidate_vectors_match={report['all_candidate_vectors_match']}")
    return 0 if report["all_candidate_vectors_match"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
