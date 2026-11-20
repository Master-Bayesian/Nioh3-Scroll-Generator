#!/usr/bin/env python3
"""Independently bind recorded search rows to owned items and MAIN witnesses.

This checks archived evidence, never executes the game or either native DLL.
Run alongside handoff/checks/replay_parity.py: parity alone cannot establish
correct shared inputs or correct mappings from pool rows back to save records.
"""
from __future__ import annotations

import argparse
import copy
import gzip
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import struct
import tempfile

CASES = {
    "small": ("acceptance/audit-tier6-cpu", "acceptance/audit-tier6-cuda", "tier6", 480),
    "primary": ("acceptance/audit-tier5-cpu", "default-policy/default-audit-tier5", "tier5", 86160),
    "wide": ("acceptance/audit-broader-cpu", "acceptance/audit-broader-cuda", "broader", 970200),
}
SLOTS = [4, 5, 6, 7, 8, 9, 10, 11, 12, 0]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def word(value: float) -> int:
    require(math.isfinite(value), "non-finite damage")
    return struct.unpack("<I", struct.pack("<f", value))[0]


def number(bits: int) -> float:
    return struct.unpack("<f", struct.pack("<I", bits))[0]


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def inventory(root: Path):
    path = root / "replay-inputs/CURRENT_WHOLE_BACKPACK.json"
    doc = load(path)
    records = doc["records"]
    require(doc["searchable_items"] == len(records), "inventory row count differs")
    require(len({r["slot_index"] for r in records}) == len(records), "duplicate owned save identity")
    for row in records:
        require(len(row["record_bytes"]) == 0xE8, "owned record is not 232 bytes")
        require(sha(bytes(row["record_bytes"])) == row["record_sha256"], "owned record digest differs")
        require(row["slot"] in SLOTS, "owned item has unknown slot")
    # Independently reproduce the public Backpack::from_slots ordering contract.
    pool = sorted(enumerate(records), key=lambda r: (r[1]["slot"], r[0]))
    pool = [r for _, r in pool]
    context = load(root / "replay-inputs/bundle/context/context.json")
    active = context["character"]["active"]
    require(active in (0, 1), "unknown active equipment bank")
    swaps = {}
    for i, row in enumerate(pool):
        assignment = bytes(row["assignment_bytes"])
        require(len(assignment) == 8, "owned assignment extent differs")
        at = struct.unpack_from("<I", assignment, active * 4)[0]
        if row["slot"] == 0 and at in (1, 2, 3):
            swaps[i] = at
    counts = {slot: sum(r["slot"] == slot for r in records) for slot in SLOTS}
    accessories = counts[9] + counts[10]
    requested = math.prod(n for slot, n in counts.items() if slot not in (9, 10) and n)
    requested *= accessories * (accessories - 1) if accessories >= 2 else 1
    require(requested == int(doc["raw_cartesian_estimate_before_pruning_or_retained_options"]),
            "raw requested domain differs from owned Cartesian product")
    return doc, pool, context, swaps, requested, sha(path.read_bytes())


def placements(items, pool):
    require(len(items) == 10, "candidate must name ten physical positions")
    seen = set()
    result = []
    for slot, index in zip(SLOTS, items):
        if index is None:
            continue
        require(type(index) is int and 0 <= index < len(pool), "candidate pool row leaves owned inventory")
        row = pool[index]
        require(row["slot"] == slot or (slot in (9, 10) and row["slot"] in (9, 10)),
                "candidate places an owned item in the wrong physical slot")
        require(row["slot_index"] not in seen, "candidate wears one owned item twice")
        seen.add(row["slot_index"])
        result.append({"slot": slot, "save_slot": row["slot_index"], "record_sha256": row["record_sha256"]})
    return result


def verify_report(report, trace_rows, pool, context, swaps, requested, inventory_sha, witness):
    require(report["inventory_file_sha256"] == inventory_sha, "report belongs to another owned inventory")
    require(report["progress"]["requested"] == requested, "report changed requested Cartesian domain")
    require(report["owned_spare_weapons"] == len(swaps), "report changed spare weapon count")
    rank = report["progress"]["rank"]
    ordinal_rank = {r["ordinal"]: n for n, r in enumerate(rank)}
    retained = {}
    identities = set()
    count = 0
    for row in trace_rows:
        require(row["ordinal"] == count, "trace is not a contiguous ordinal prefix")
        require(row["attrs"] == context["attrs"], "fixed attributes differ from input context")
        assignment = placements(row["items"], pool)
        identity = tuple(row["items"])
        require(identity not in identities, "duplicate candidate assignment hides a missing outfit")
        identities.add(identity)
        n = ordinal_rank.get(count)
        if n is not None:
            actual = report["ranked"][n]
            require(actual["assignments"] == assignment, f"rank {n}: assignments do not name scored owned rows")
            expected_swap = swaps.get(row["items"][9])
            require(actual["move_previous_weapon_to_slot"] == expected_swap,
                    f"rank {n}: spare relocation does not match owned assignment")
            require(actual["score_bits"] == row["outcome"]["bits"], "rank score differs from its traced candidate")
            retained[n] = row
        count += 1
    require(count == report["progress"]["completed"], "trace candidate count differs")
    require(len(retained) == len(rank), "ranked candidate missing from trace")
    require(witness["passed"] and witness["complete"] and witness["unanswered"] == 0,
            "retained native witness is not accepted")
    require(witness["considered"] == count, "native witness domain count differs")
    require(witness["source_capture_sha256"] == context["provenance"]["source_sha256"],
            "native witness belongs to another source capture")
    require(witness["assignments"] == report["ranked"][0]["assignments"], "native witness names a different winner")
    details = report["top_case"]["components"]
    require(details["state"] == "available", "winner component verification unavailable")
    first = retained[0]["cpu_components"]
    for channel in ("total", "physical", "elemental"):
        expected = word(witness["native_damage"][channel])
        require(first[channel] == expected, f"native winner {channel} contradicts candidate trace")
        require(word(details[channel]) == expected, f"native winner {channel} contradicts displayed details")
        total = 0.0
        for contact in details["contacts"]:
            total = number(word(total + contact[channel]))
        require(word(total) == expected, f"ordered contact {channel} sum differs")
    require(report["ranked"][0]["score_bits"] == first["total"], "native winner score differs")
    return {"candidate_identities_checked": count, "ranked_owned_assignments_checked": len(rank),
            "native_witness_channels": witness["native_damage"], "owned_spare_count": len(swaps),
            "original_MAIN_executed_here": False,
            "native_witness_scope": "archived winner receipt consistency; no fresh original instruction execution"}


def rows(root, trace_ref):
    digest = hashlib.sha256()
    length = 0
    with gzip.open(root / trace_ref["path"], "rb") as stream:
        for line in stream:
            digest.update(line)
            length += len(line)
            yield json.loads(line)
    require(digest.hexdigest() == trace_ref["sha256"], "trace bytes do not match declared SHA-256")
    require(length == trace_ref["uncompressed_bytes"], "trace length differs from indexed bytes")


def fault_controls(root: Path, info):
    spec = importlib.util.spec_from_file_location("parity_comparator", root / "native/evidence/compare_search_results.py")
    comparator = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(comparator)
    index = load(root / "evidence/TRACE_INDEX.json")
    names = CASES["small"][:2]
    reports = [load(root / "evidence/host" / n / "SEARCH_RESULT.json") for n in names]
    traces = [list(rows(root, index[n + "/AUDIT_ROWS.jsonl"])) for n in names]
    witness = load(root / "evidence/host/original-winners/tier6.json")
    _, pool, context, swaps, requested, inventory_sha = info
    controls = []
    for fault in ("shared_wrong_owned_assignment", "shared_plus_one_damage"):
        changed = copy.deepcopy(reports)
        changed_rows = copy.deepcopy(traces)
        if fault == "shared_wrong_owned_assignment":
            for report in changed:
                assignment = report["ranked"][0]["assignments"]
                one = next(r for r in assignment if r["slot"] == 9)
                two = next(r for r in assignment if r["slot"] == 10)
                two.update(save_slot=one["save_slot"], record_sha256=one["record_sha256"])
        else:
            for report, trace in zip(changed, changed_rows):
                for row in trace:
                    row["outcome"]["bits"] = word(number(row["outcome"]["bits"]) + 1)
                    for components in (row["cpu_components"], row["cuda_components"]):
                        if components is not None:
                            for channel in ("total", "physical"):
                                components[channel] = word(number(components[channel]) + 1)
                for entry, ranked in zip(report["progress"]["rank"], report["ranked"]):
                    entry["score_bits"] = word(number(entry["score_bits"]) + 1)
                    ranked["score_bits"] = entry["score_bits"]
                    ranked["damage"] = number(entry["score_bits"])
                for channel in ("total", "physical"):
                    report["top_case"]["components"][channel] += 1
                    report["top_case"]["components"]["contacts"][0][channel] += 1
        with tempfile.TemporaryDirectory(prefix="nioh3-numeric-control-") as temp:
            temp = Path(temp)
            rp, tp = [], []
            for side, (report, trace) in enumerate(zip(changed, changed_rows)):
                payload = b"".join((json.dumps(r, separators=(",", ":")) + "\n").encode() for r in trace)
                report["audit"]["trace_sha256"] = sha(payload)
                rp.append(temp / f"{side}.json")
                tp.append(temp / f"{side}.jsonl")
                rp[-1].write_text(json.dumps(report))
                tp[-1].write_bytes(payload)
            old = comparator.compare_reports(*rp, audit=True, left_audit=tp[0], right_audit=tp[1])
            require(old["passed"], "fault control did not pass the original parity-only acceptance")
            try:
                verify_report(changed[0], changed_rows[0], pool, context, swaps, requested, inventory_sha, witness)
            except ValueError as error:
                controls.append({"fault": fault, "original_parity_passed": True,
                                 "new_binding_check_rejected": True, "rejection": str(error)})
            else:
                raise ValueError("new checker accepted the injected shared error")
    return controls


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("handoff", type=Path)
    parser.add_argument("--case", choices=[*CASES, "all"], default="all")
    parser.add_argument("--fault-controls", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.handoff.resolve()
    result = {"schema": "nioh3-independent-numeric-receipts/v1", "passed": False,
              "scope": "archived candidate identity and retained MAIN winner consistency",
              "original_MAIN_executed_here": False, "cuda_executed_here": False, "cases": {}}
    try:
        info = inventory(root)
        doc, pool, context, swaps, requested, inventory_sha = info
        index = load(root / "evidence/TRACE_INDEX.json")
        for case in CASES if args.case == "all" else [args.case]:
            left, right, native, count = CASES[case]
            case_result = []
            for name in (left, right):
                path = root / "evidence/host" / name / "SEARCH_RESULT.json"
                report = load(path)
                require(report["save_source_sha256"] == doc["source_sha256"], "report save source differs")
                summary = verify_report(report, rows(root, index[name + "/AUDIT_ROWS.jsonl"]), pool,
                                        context, swaps, requested, inventory_sha,
                                        load(root / "evidence/host/original-winners" / (native + ".json")))
                require(summary["candidate_identities_checked"] == count, "case count differs")
                summary.update(report=name, report_sha256=sha(path.read_bytes()), source_scope=report["source_scope"])
                case_result.append(summary)
            result["cases"][case] = case_result
        if args.fault_controls:
            result["fault_controls"] = fault_controls(root, info)
        result["passed"] = True
    except (ValueError, KeyError, OSError, TypeError, IndexError) as error:
        result["error"] = str(error)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": result["passed"], "cases": list(result["cases"]), "error": result.get("error")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
