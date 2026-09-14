from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def probe_from_document(document: dict[str, Any]) -> dict[str, Any]:
    bridge = document.get("bridge_result", document)
    if not isinstance(bridge, dict):
        raise ValueError("Capture bridge result is not an object")
    probe = bridge.get("result", bridge.get("value", bridge))
    if not isinstance(probe, dict):
        raise ValueError("Capture probe is not an object")
    return probe


def raw_byte(record: dict[str, Any], offset: int) -> int:
    raw = bytes.fromhex(str(record.get("record_raw_hex", "")))
    if len(raw) <= offset:
        raise ValueError(f"Record {record.get('record_address')} is too short for offset 0x{offset:X}")
    return raw[offset]


def summarize(document: dict[str, Any]) -> dict[str, Any]:
    probe = probe_from_document(document)
    final_events = [event for event in probe.get("events", []) if event.get("site") == "late_mask_final"]
    if not final_events:
        return {
            "schema": "nioh3-possessed-late-mask-summary/v2",
            "run_id": probe.get("run_id"),
            "capture_schema": probe.get("schema"),
            "event_count": len(probe.get("events", [])),
            "final_event_present": False,
            "classification": "zero-event or incomplete capture; not assignment evidence",
        }
    final = final_events[-1]
    mission_records: list[dict[str, Any]] = []
    candidates: list[dict[str, Any]] = []
    for record in final.get("records", []):
        if record.get("mission_key") != "0xCC96":
            continue
        candidate = int(record.get("candidate", raw_byte(record, 0xE9)))
        field_8f = raw_byte(record, 0x8F)
        field_ea = raw_byte(record, 0xEA)
        if "field_8f" in record and int(record["field_8f"]) != field_8f:
            raise ValueError("Explicit field_8f disagrees with raw record bytes")
        explicit_ea = record.get("field_ea", record.get("possessed"))
        if explicit_ea is not None and int(explicit_ea) != field_ea:
            raise ValueError("Explicit field_ea disagrees with raw record bytes")
        mission_record = {
            "index": record.get("index"),
            "record_address": record.get("record_address"),
            "spawn_id": record.get("spawn_id"),
            "mission_key": record.get("mission_key"),
            "enemy_lookup_key": record.get("enemy_lookup_key"),
            "selector_class": record.get("selector_class", raw_byte(record, 0x90)),
            "assigned_index": record.get("assigned_index"),
            "field_8e": raw_byte(record, 0x8E),
            "field_8f": field_8f,
            "field_e9": candidate,
            "field_ea": field_ea,
        }
        mission_records.append(mission_record)
        if candidate != 0:
            candidates.append({
                key: value
                for key, value in mission_record.items()
                if key not in {"record_address", "mission_key", "selector_class"}
            })
    nonzero_8f = [record for record in candidates if record["field_8f"] != 0]
    nonzero_ea = [record for record in candidates if record["field_ea"] != 0]
    field_e9_zero_records = [record for record in mission_records if record["field_e9"] == 0]
    field_e9_one_records = [record for record in mission_records if record["field_e9"] == 1]
    return {
        "schema": "nioh3-possessed-late-mask-summary/v2",
        "run_id": probe.get("run_id"),
        "capture_schema": probe.get("schema"),
        "event_count": len(probe.get("events", [])),
        "final_event_present": True,
        "selection_mask_hex": final.get("selection_mask_hex"),
        "mission_record_count": len(mission_records),
        "candidate_count": len(candidates),
        "field_e9_counts": {
            "zero": len(field_e9_zero_records),
            "one": len(field_e9_one_records),
            "other": len(mission_records) - len(field_e9_zero_records) - len(field_e9_one_records),
        },
        "field_e9_zero_records": field_e9_zero_records,
        "field_8f_nonzero_count": len(nonzero_8f),
        "field_8f_nonzero_records": nonzero_8f,
        "field_ea_nonzero_count": len(nonzero_ea),
        "field_ea_nonzero_records": nonzero_ea,
        "mission_records": mission_records,
        "candidates": candidates,
        "classification": "native record-state observation; visual semantics require the run manifest",
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Summarize late-mask candidate fields from raw records.")
    parser.add_argument("capture", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = summarize(json.loads(args.capture.read_text(encoding="utf-8")))
    rendered = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output is None:
        print(rendered, end="")
        return
    if args.output.exists():
        raise FileExistsError(f"Refusing to overwrite derived evidence: {args.output}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")


if __name__ == "__main__":
    main()
