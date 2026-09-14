"""Compare repeated read-only possessed-enemy captures for deterministic fields."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


MT_CURSOR_AND_STATE_SIZE = 0x9C4


def load_probe(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    return document.get("probe", document)


def event_by_site(probe: dict[str, Any], site: str) -> dict[str, Any]:
    return next(event for event in probe["events"] if event["site"] == site)


def candidate_semantics(event: dict[str, Any]) -> list[dict[str, Any]]:
    items = []
    for entry in event["candidates"]["entries"]:
        record = entry["record"]
        raw = bytes.fromhex(record["record_raw_hex"])
        items.append(
            {
                "index": record.get("index"),
                "spawn_id": record.get("spawn_id"),
                "mission_key": record.get("mission_key"),
                "enemy_lookup_key": record.get("enemy_lookup_key"),
                "field_80": raw[0x80],
                "field_8e": raw[0x8E],
                "field_8f": raw[0x8F],
                "selector_class": raw[0x90],
                "assigned_index": raw[0x95],
                "field_df": raw[0xDF],
                "field_e9": raw[0xE9],
                "field_ea": raw[0xEA],
            }
        )
    return items


def mt_prefix(event: dict[str, Any]) -> bytes:
    hex_value = event.get("mt_cursor_and_state_raw_hex")
    if hex_value is None:
        hex_value = event["mt_state_raw_hex"][: MT_CURSOR_AND_STATE_SIZE * 2]
    return bytes.fromhex(hex_value)


def extended_mt(event: dict[str, Any]) -> bytes:
    hex_value = event.get("mt_extended_context_raw_hex", event.get("mt_state_raw_hex", ""))
    return bytes.fromhex(hex_value)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def compare(first_path: Path, second_path: Path) -> dict[str, Any]:
    first = load_probe(first_path)
    second = load_probe(second_path)
    sites = ["selector_pass_0", "selector_pass_1", "selector_final"]
    comparisons = []
    for site in sites:
        left = event_by_site(first, site)
        right = event_by_site(second, site)
        prefix_left = mt_prefix(left)
        prefix_right = mt_prefix(right)
        extended_left = extended_mt(left)
        extended_right = extended_mt(right)
        differences = [
            index
            for index, (a, b) in enumerate(zip(extended_left, extended_right, strict=True))
            if a != b
        ] if len(extended_left) == len(extended_right) else []
        comparisons.append(
            {
                "site": site,
                "candidate_semantics_equal": candidate_semantics(left) == candidate_semantics(right),
                "selection_mask_equal": left["candidates"]["selection_mask_hex"]
                == right["candidates"]["selection_mask_hex"],
                "mt_cursor_and_624_word_state_size": len(prefix_left),
                "mt_cursor_and_624_word_state_equal": prefix_left == prefix_right,
                "mt_cursor_and_624_word_state_sha256_a": sha256(prefix_left),
                "mt_cursor_and_624_word_state_sha256_b": sha256(prefix_right),
                "extended_context_size": len(extended_left),
                "extended_context_equal": extended_left == extended_right,
                "extended_context_first_difference": differences[0] if differences else None,
                "extended_context_last_difference": differences[-1] if differences else None,
                "extended_context_difference_count": len(differences),
            }
        )
    return {
        "schema": "nioh3-possessed-repeat-comparison/v1",
        "first_capture": first_path.name,
        "second_capture": second_path.name,
        "interpretation": (
            "The first 0x9C4 bytes are the requested cursor plus 624 uint32 MT state words. "
            "The older collectors also copied an extended 0x1388-byte region; differences "
            "after 0x9C4 are retained as raw evidence but are not treated as MT-state drift."
        ),
        "comparisons": comparisons,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = compare(args.first, args.second)
    rendered = json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
