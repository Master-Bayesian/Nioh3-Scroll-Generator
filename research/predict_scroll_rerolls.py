from __future__ import annotations

import argparse
from dataclasses import asdict
import json
from pathlib import Path
import sys
from typing import Sequence


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor.catalog import effect_name, native_effect_name
from nioh3_scroll_editor.reroll import (
    advance_reroll_counter,
    predict_reroll_candidates,
)


def _parse_int(value: str) -> int:
    return int(value, 0)


def build_report(
    record: bytes,
    *,
    slot_index: int,
    cycles: int,
    initial_counter_advance: int,
    dynamic_gate_group_keys: Sequence[int] | None,
) -> dict[str, object]:
    if cycles < 1:
        raise ValueError("cycles must be positive")
    current = advance_reroll_counter(record, initial_counter_advance)
    predictions: list[dict[str, object]] = []
    for cycle in range(cycles):
        prediction = predict_reroll_candidates(
            current,
            slot_index,
            dynamic_gate_group_keys=dynamic_gate_group_keys,
        )
        item = asdict(prediction)
        item["selected_slot"] = slot_index + 1
        item["candidates"] = [
            {
                **asdict(candidate),
                "effect_id_hex": f"0x{candidate.effect_id:04X}",
                "group_key_hex": f"0x{candidate.group_key:04X}",
                "name_zh_CN": (
                    native_effect_name(candidate.effect_id, "zh-CN")
                    or effect_name(candidate.effect_id)
                ),
            }
            for candidate in prediction.candidates
        ]
        predictions.append(item)
        current = advance_reroll_counter(current)
    return {
        "schema": "nioh3-scroll-reroll-prediction/v1",
        "game_version": "PC v2.00.02",
        "status": "native-static-candidate-awaiting-live-parity",
        "counter_semantics": {
            "current_pool": "uses the counter already stored at record +0x0C",
            "manual_refresh": "increments +0x0C before rebuilding the pool",
            "accepted_candidate": "writes the candidate and increments +0x0C",
            "completion_path": "increments +0x0C after completion/rebuild",
        },
        "predictions": predictions,
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Predict PC v2.00.02 scroll reroll pools from a raw 0xE8 record"
    )
    parser.add_argument("record", type=Path, help="raw 0xE8 final scroll record")
    parser.add_argument("--slot", type=int, required=True, help="one-based effect slot")
    parser.add_argument("--cycles", type=int, default=1)
    parser.add_argument(
        "--advance-before-first",
        type=int,
        default=0,
        help="counter increments before the first displayed pool",
    )
    parser.add_argument(
        "--dynamic-group",
        type=_parse_int,
        action="append",
        help="save-scoped group key allowed by RVA 0x2167804; repeat as needed",
    )
    parser.add_argument(
        "--assume-no-dynamic-groups",
        action="store_true",
        help="mark an explicitly empty dynamic group context",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)

    if not 1 <= args.slot <= 7:
        parser.error("--slot must be in 1..7")
    if args.dynamic_group and args.assume_no_dynamic_groups:
        parser.error("do not combine --dynamic-group and --assume-no-dynamic-groups")
    if args.dynamic_group:
        dynamic_groups: Sequence[int] | None = args.dynamic_group
    elif args.assume_no_dynamic_groups:
        dynamic_groups = ()
    else:
        dynamic_groups = None

    report = build_report(
        args.record.read_bytes(),
        slot_index=args.slot - 1,
        cycles=args.cycles,
        initial_counter_advance=args.advance_before_first,
        dynamic_gate_group_keys=dynamic_groups,
    )
    rendered = json.dumps(report, ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered + "\n", encoding="utf-8")
        print(args.output)
    else:
        print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
