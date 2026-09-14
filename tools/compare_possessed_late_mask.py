"""Compare two late-mask captures without assigning unproved visual semantics."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from summarize_possessed_late_mask import summarize


def load_capture(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"Capture root must be an object: {path}")
    return value


def final_event(capture: dict[str, Any]) -> dict[str, Any]:
    bridge = capture.get("bridge_result")
    if not isinstance(bridge, dict):
        raise ValueError("Capture does not contain bridge_result")
    nested = bridge.get("result")
    if isinstance(nested, dict):
        bridge = nested
    events = bridge.get("events")
    if not isinstance(events, list):
        raise ValueError("Capture does not contain events")
    finals = [event for event in events if isinstance(event, dict) and event.get("site") == "late_mask_final"]
    if len(finals) != 1:
        raise ValueError(f"Expected exactly one late_mask_final event, got {len(finals)}")
    return finals[0]


def candidate_addresses(capture: dict[str, Any]) -> list[str]:
    records = final_event(capture).get("records")
    if not isinstance(records, list):
        raise ValueError("Final event does not contain records")
    return [
        str(record.get("record_address"))
        for record in records
        if isinstance(record, dict)
        and record.get("mission_key") == "0xCC96"
        and record.get("candidate") == 1
    ]


def mission_addresses(capture: dict[str, Any]) -> list[str]:
    records = final_event(capture).get("records")
    if not isinstance(records, list):
        raise ValueError("Final event does not contain records")
    return [
        str(record.get("record_address"))
        for record in records
        if isinstance(record, dict) and record.get("mission_key") == "0xCC96"
    ]


def selected_fields(records: list[dict[str, Any]], fields: tuple[str, ...]) -> list[dict[str, Any]]:
    return [{field: record.get(field) for field in fields} for record in records]


def compare(first: dict[str, Any], second: dict[str, Any]) -> dict[str, Any]:
    first_summary = summarize(first)
    second_summary = summarize(second)
    first_positive = first_summary["field_8f_nonzero_records"]
    second_positive = second_summary["field_8f_nonzero_records"]
    first_mission = first_summary["mission_records"]
    second_mission = second_summary["mission_records"]
    identity_fields = ("index", "spawn_id", "mission_key", "enemy_lookup_key")
    state_fields = identity_fields + (
        "selector_class",
        "assigned_index",
        "field_8e",
        "field_8f",
        "field_e9",
        "field_ea",
    )
    partition_fields = ("index", "spawn_id", "enemy_lookup_key")
    mission_identity_equal = (
        selected_fields(first_mission, identity_fields)
        == selected_fields(second_mission, identity_fields)
    )
    mission_state_equal = (
        selected_fields(first_mission, state_fields)
        == selected_fields(second_mission, state_fields)
    )
    candidate_semantics_equal = first_summary["candidates"] == second_summary["candidates"]
    field_8f_positive_equal = first_positive == second_positive
    field_e9_counts_equal = first_summary["field_e9_counts"] == second_summary["field_e9_counts"]
    first_e9_zero = selected_fields(first_summary["field_e9_zero_records"], partition_fields)
    second_e9_zero = selected_fields(second_summary["field_e9_zero_records"], partition_fields)
    field_e9_zero_records_equal = first_e9_zero == second_e9_zero
    mask_equal = first_summary["selection_mask_hex"] == second_summary["selection_mask_hex"]
    address_lists_equal = candidate_addresses(first) == candidate_addresses(second)
    mission_address_lists_equal = mission_addresses(first) == mission_addresses(second)
    field_8f_native_partition_repeat_supported = (
        mission_identity_equal
        and field_8f_positive_equal
        and mask_equal
        and len(first_positive) == 1
        and first_positive[0].get("field_8f") == 1
    )
    field_e9_partition_repeat_supported = (
        mission_identity_equal and field_e9_counts_equal and field_e9_zero_records_equal
    )
    field_8f_nonzero_counts = [len(first_positive), len(second_positive)]
    first_positive_second_zero_split = field_8f_nonzero_counts == [1, 0]
    return {
        "schema": "nioh3-possessed-late-mask-comparison/v2",
        "first_run_id": first_summary["run_id"],
        "second_run_id": second_summary["run_id"],
        "mission_record_counts": [
            first_summary["mission_record_count"],
            second_summary["mission_record_count"],
        ],
        "mission_identity_equal": mission_identity_equal,
        "mission_state_equal": mission_state_equal,
        "candidate_semantics_equal": candidate_semantics_equal,
        "selection_mask_equal": mask_equal,
        "field_8f_positive_records_equal": field_8f_positive_equal,
        "field_8f_nonzero_counts": field_8f_nonzero_counts,
        "field_8f_native_partition_repeat_supported": field_8f_native_partition_repeat_supported,
        "first_positive_second_zero_split": first_positive_second_zero_split,
        "field_e9_counts_equal": field_e9_counts_equal,
        "first_field_e9_zero_records": first_e9_zero,
        "second_field_e9_zero_records": second_e9_zero,
        "field_e9_zero_records_equal": field_e9_zero_records_equal,
        "field_e9_partition_repeat_supported": field_e9_partition_repeat_supported,
        "field_ea_nonzero_counts": [
            first_summary["field_ea_nonzero_count"],
            second_summary["field_ea_nonzero_count"],
        ],
        "mission_record_addresses_equal": mission_address_lists_equal,
        "first_mission_record_addresses": mission_addresses(first),
        "second_mission_record_addresses": mission_addresses(second),
        "candidate_record_addresses_equal": address_lists_equal,
        "first_candidate_record_addresses": candidate_addresses(first),
        "second_candidate_record_addresses": candidate_addresses(second),
        "first_candidates": first_summary["candidates"],
        "second_candidates": second_summary["candidates"],
        "native_state_repeat_supported": field_8f_native_partition_repeat_supported,
        "interpretation": (
            "This comparison establishes only native record partitions across complete mission-record "
            "views. Stable counts do not establish a stable field partition. Interpretation requires "
            "separate visual labels in the run manifests before assigning possession or One Difficulty "
            "semantics."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Compare two possessed-enemy late-mask captures.")
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite existing comparison: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    result = compare(load_capture(args.first), load_capture(args.second))
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({
        "output": str(output),
        "native_state_repeat_supported": result["native_state_repeat_supported"],
        "first_positive_second_zero_split": result["first_positive_second_zero_split"],
    }))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
