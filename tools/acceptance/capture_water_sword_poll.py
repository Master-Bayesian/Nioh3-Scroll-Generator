"""Observe the PC202 Water Sword action, controller debit and owner Ki via ordinary reads."""

import argparse
from contextlib import closing
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import time

BUILD_ROOT = Path("F:/Nioh3_r16_build")
PLAYER_GLOBAL = 0x4745348
RESOURCE_DIGEST = "fff9383f4d499a229d7e66cd000261ceb953dbd680830c4e3b57fd75dcf34f09"
ENTRY = 25329
II = 25332
FINISH = 25338
# Local Water Sword destinations in section 3. Other destinations end the profile.
EDGES = {
    25329: {25330, FINISH},
    25330: {25331, FINISH},
    25331: {25330, II, FINISH},
    25332: {25333, FINISH},
    25333: {25332, 25334, FINISH},
    25334: {25335, FINISH},
    25335: {25334, 25336, FINISH},
    25336: {25337, FINISH},
    25337: {25336, FINISH},
    FINISH: set(),
}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def scalar(memory, address, fmt):
    return struct.unpack("<" + fmt, memory.read(address, struct.calcsize("<" + fmt)))[0]


def as_float(bits):
    return struct.unpack("<f", struct.pack("<I", bits))[0]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load_actions(file):
    data = file.read_bytes()
    pos, length = struct.unpack_from("<II", data, 12)
    if length % 336 or pos + length > len(data):
        raise ValueError("Invalid PC202 action-parameter section")
    return [data[i : i + 336] for i in range(pos, pos + length, 336)]


def water_actions(root):
    path = (
        root
        / "deliverables/event-graph-20261111/matched-resources/archive-01-entry-00091.bin"
    )
    if digest(path.read_bytes()) != RESOURCE_DIGEST:
        raise ValueError("Water Sword resource identity differs")
    result = {}
    for row in load_actions(path):
        action = struct.unpack_from("<H", row, 12)[0]
        if action in EDGES:
            if action in result:
                raise ValueError("Ambiguous Water Sword action parameter")
            result[action] = row
    if set(result) != set(EDGES):
        raise ValueError("Water Sword rows missing")
    return result


def stable_read(memory, base, hint):
    """No callback or thread pause. Accept two identical independently anchored reads."""

    def sweep():
        player_root = memory.q(base + PLAYER_GLOBAL)
        actor = memory.q(player_root + 0x1338)
        owner = memory.q(actor + 0x3A0)
        if (actor, owner) != hint or memory.q(owner) != actor:
            raise ValueError("Player/owner identity changed")
        state = memory.q(memory.q(actor + 0x3B0) + 8)
        if memory.q(state + 0xE0) != actor:
            raise ValueError("Action state belongs to another actor")
        record = memory.q(state + 0xE8)
        row = memory.q(record + 0x20)
        raw = memory.read(row, 336)
        return dict(
            state=state,
            record=record,
            parameter=row,
            action=struct.unpack_from("<H", raw, 12)[0],
            digest=digest(raw),
            cost_bits=scalar(memory, state + 0x100C, "I"),
            ki_bits=scalar(memory, owner + 0xAC, "I"),
        )

    first = sweep()
    if first == sweep():
        return first
    return None


def preflight(memory, snapshot):
    roots = snapshot["roots"]
    actor, owner = roots["player"], roots["owner"]
    basis = stable_read(memory, snapshot["module_base"], (actor, owner))
    if not basis:
        raise ValueError("Initial read crossed a game update; retry preflight")
    mode = scalar(memory, owner + 0x2C, "I")
    stat = memory.q(owner + 0x2D90) or owner + 0x25A8
    if stat != roots["stat"] or memory.q(stat + 8) != owner or mode != 0:
        raise ValueError(
            "Expected the freshly observed Samurai role and selected stats"
        )
    maximum = scalar(memory, stat + 0x1C, "I")
    descriptor = memory.q(actor + 0xE8)
    controller = memory.q(actor + 0x3B0)
    mask = memory.q(controller + 0x40) if controller else 0
    factors = struct.unpack("<3f", memory.read(owner + 0x2E80, 12))
    materializer = memory.q(stat + 0x7C0)
    return dict(
        first=basis,
        maximum_ki=maximum,
        process=snapshot["process"],
        actor=actor,
        owner=owner,
        stance=snapshot["inputs"]["style"],
        observed_mode=mode,
        descriptor_bit31=bool(scalar(memory, descriptor + 0x68, "I") & (1 << 31))
        if descriptor
        else None,
        suppress_general_reduction=bool(scalar(memory, mask, "B") & 4)
        if mask
        else False,
        owner_cost_factors=factors,
        materializer_sha256=digest(memory.read(materializer, 0x900)),
        stat_agility=scalar(memory, stat + 0x79C, "I"),
        initial_ki=as_float(basis["ki_bits"]),
    )


def summarize(
    samples, hashes, maximum, entry=ENTRY, edges=None, upgrade=II, finish=FINISH
):
    """Compare each observed decrease against the reader's own original cost field."""
    edges = EDGES if edges is None else edges
    segments, problems, spans = [], set(), []
    previous = None
    had_cast, outside = False, False
    gaps, discarded = [], 0
    for sample in samples:
        if "action" not in sample:
            if had_cast and not outside:
                discarded += 1
            continue
        action = sample["action"]
        balance = as_float(sample["ki_bits"])
        cost = as_float(sample["cost_bits"])
        if not math.isfinite(balance) or not math.isfinite(cost):
            problems.add("nonfinite_game_field")
            continue
        recognized = action in hashes and hashes[action] == sample["digest"]
        if previous is not None and (recognized or (had_cast and not outside)):
            gaps.append(sample["ms"] - previous["ms"])
        if not recognized and action in hashes:
            problems.add("action_number_with_other_parameter_bytes")
        if not recognized:
            if had_cast:
                outside = True
            previous = sample
            continue
        if outside:
            problems.add("second_or_interrupted_cast")
        had_cast = True
        if not segments or segments[-1]["action"] != action or outside:
            if not segments:
                if action != entry:
                    problems.add("no_entry")
                if previous is None or as_float(previous["ki_bits"]) != maximum:
                    problems.add("no_full_ki_sample_immediately_before_entry")
            elif action not in edges.get(segments[-1]["action"], set()):
                problems.add("skipped_or_unexpected_transition")
            segments.append(
                dict(
                    action=action,
                    from_ms=sample["ms"],
                    to_ms=sample["ms"],
                    samples=0,
                    cost_bits=[],
                    decreases=[],
                    ki_before=as_float(previous["ki_bits"]) if previous else None,
                    balance_increased=False,
                    ki_min=balance,
                    ki_max=balance,
                )
            )
        block = segments[-1]
        block["samples"] += 1
        block["to_ms"] = sample["ms"]
        block["ki_min"] = min(block["ki_min"], balance)
        block["ki_max"] = max(block["ki_max"], balance)
        if sample["cost_bits"] not in block["cost_bits"]:
            block["cost_bits"].append(sample["cost_bits"])
        if previous is not None:
            before = as_float(previous["ki_bits"])
            if balance > before:
                block["balance_increased"] = True
            if balance < before:
                block["decreases"].append(
                    dict(
                        before=before,
                        after=balance,
                        cost=cost,
                        matches_original_cost=f32(before - cost) == balance,
                    )
                )
        previous = sample

    seen_depletion = False
    for block in segments:
        changes = block["decreases"]
        block["adjusted_cost_samples"] = [as_float(x) for x in block.pop("cost_bits")]
        if block["action"] == finish and not changes:
            block["debit_evidence"] = "finish_seen_without_an_observed_decrease"
        elif len(changes) == 1 and changes[0]["matches_original_cost"]:
            block["debit_evidence"] = "balance_change_matches_single_f32_charge"
        elif (
            block["action"] == finish
            and len(changes) > 1
            and len(block["adjusted_cost_samples"]) == 1
            and not block["balance_increased"]
            and changes[0]["before"] == block["ki_before"]
            and f32(changes[0]["before"] - changes[0]["cost"]) == changes[-1]["after"]
        ):
            # This establishes the net debit only. A poll does not see every game tick.
            block["debit_evidence"] = "split_finish_balance_matches_f32_net_charge"
        else:
            block["debit_evidence"] = "charge_not_resolved_by_polling"
            problems.add("missing_or_nonmatching_charge")
        if any(
            x["before"] > 0 >= x["after"] and x["matches_original_cost"]
            for x in changes
        ):
            seen_depletion = True
        spans.append(block["to_ms"] - block["from_ms"])
    if not segments:
        problems.add("no_resource_matched_actions")
    if had_cast and not outside:
        problems.add("no_return_to_an_outside_action")
    if not seen_depletion:
        problems.add("no_observed_exhausting_charge")
    if seen_depletion:
        first_depleted = next(
            i
            for i, block in enumerate(segments)
            if any(
                x["before"] > 0 >= x["after"] and x["matches_original_cost"]
                for x in block["decreases"]
            )
        )
        if any(block["action"] != finish for block in segments[first_depleted + 1 :]):
            problems.add("new_sustained_action_after_exhaustion")
    return dict(
        actions=[b["action"] for b in segments],
        observed_ii_entry=any(b["action"] == upgrade for b in segments),
        segments=segments,
        discarded_reads_during_cast=discarded,
        max_stable_sample_gap_ms=max(gaps, default=None),
        shortest_observed_action_span_ms=min(spans, default=None),
        returned_outside=outside,
        observed_exhausting_charge=seen_depletion,
        observations_reconcile=not problems,
        unresolved=sorted(problems),
        scope="Polled observations only. No caller, target contact or intervening unobserved game tick is established.",
    )


def reader_class(root):
    sys.path.insert(
        0, str(root / "worktrees/research-damage/research/damage/evaluator/src")
    )
    from read_player_context import Reader

    return Reader


def write(path, obj):
    path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf8")


def replay(args):
    arm = json.loads((args.capture / "ARMED.json").read_bytes())
    raw = (args.capture / "SAMPLES.json").read_bytes()
    samples = json.loads(raw)
    if not samples or samples[0] != dict(ms=0, **arm["initial"]["first"]):
        raise ValueError("Sample stream does not start with the armed actor")
    parameters = {i: digest(row) for i, row in water_actions(args.root).items()}
    result = summarize(samples, parameters, arm["initial"]["maximum_ki"])
    result.update(
        process=arm["initial"]["process"],
        snapshot_sha256=arm["observation_sha256"],
        samples_sha256=digest(raw),
    )
    with args.output.open("x", encoding="utf8") as out:
        json.dump(result, out, indent=2)
        out.write("\n")
    print(
        json.dumps(
            {"actions": len(result["actions"]), "unresolved": result["unresolved"]}
        )
    )
    return result["observations_reconcile"]


def live(args):
    snapshot = json.loads(args.snapshot.read_bytes())
    process = snapshot["process"]
    parameters = {key: digest(raw) for key, raw in water_actions(args.root).items()}
    identity = dict(
        process_id=process["pid"], creation_filetime=str(process["creation_filetime"])
    )
    if args.mode == "capture" and not (
        0 < args.seconds <= 120 and 2 <= args.interval_ms <= 50
    ):
        raise ValueError(
            "Expected a short positive polling interval and a capture of at most 120 seconds"
        )
    with closing(reader_class(args.root)(identity)) as reader:
        initial = preflight(reader, snapshot)
        if args.mode == "preflight":
            print(json.dumps(initial))
            return
        if (
            initial["initial_ki"] != initial["maximum_ki"]
            or initial["first"]["action"] in EDGES
        ):
            raise ValueError("Wait for full Ki and an idle actor before arming")
        args.output.mkdir(parents=True, exist_ok=False)
        stat = snapshot["roots"]["stat"]
        materializer = reader.q(stat + 0x7C0)
        bank = reader.read(materializer, 0x900)
        if digest(bank) != initial["materializer_sha256"]:
            raise ValueError("Selected damage bank changed during arming")
        write(
            args.output / "COST_INPUTS.json",
            dict(
                process=process,
                actor=initial["actor"],
                owner=initial["owner"],
                selected_stat_address=stat,
                materializer_address=materializer,
                materializer_0_to_8ff_hex=bank.hex(),
                owner_3670=scalar(reader, initial["owner"] + 0x3670, "B"),
                actor_flags={
                    key: initial[key]
                    for key in (
                        "descriptor_bit31",
                        "suppress_general_reduction",
                        "owner_cost_factors",
                        "maximum_ki",
                        "stat_agility",
                    )
                },
                source="Single pre-cast ordinary read. Per-action cost and balance are recorded separately.",
            ),
        )
        arm = dict(
            schema="pc202-water-sword-rpm/v1",
            armed_utc=datetime.now(timezone.utc).isoformat(),
            duration_seconds=args.seconds,
            period_ms=args.interval_ms,
            observation_sha256=digest(args.snapshot.read_bytes()),
            access="PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ",
            initial=initial,
        )
        samples = [dict(ms=0.0, **initial["first"])]
        start = time.perf_counter()
        write(args.output / "arming.tmp", arm)
        (args.output / "arming.tmp").replace(args.output / "ARMED.json")
        print("ARMED", str(args.output / "ARMED.json"), flush=True)
        inside = False
        exit_since = None
        while time.perf_counter() - start < args.seconds:
            tick = time.perf_counter()
            try:
                sample = stable_read(
                    reader,
                    snapshot["module_base"],
                    (initial["actor"], initial["owner"]),
                )
            except (OSError, ValueError) as err:
                sample = dict(error=str(err))
            sample = dict(
                ms=(time.perf_counter() - start) * 1000,
                **(sample or {"unstable": True}),
            )
            samples.append(sample)
            if (
                sample.get("action") in parameters
                and sample.get("digest") == parameters[sample["action"]]
            ):
                inside = True
                exit_since = None
            elif inside and "action" in sample:
                exit_since = tick if exit_since is None else exit_since
                if tick - exit_since >= 1:
                    break
            time.sleep(max(0, args.interval_ms / 1000 - (time.perf_counter() - tick)))
        write(args.output / "SAMPLES.json", samples)
        result = summarize(samples, parameters, initial["maximum_ki"])
        result.update(
            process=process,
            snapshot_sha256=arm["observation_sha256"],
            bytes_read=reader.bytes_read,
        )
        reader.check_birth()
        write(args.output / "RESULT.json", result)
        print(
            json.dumps(
                {
                    k: result[k]
                    for k in (
                        "actions",
                        "observed_ii_entry",
                        "observations_reconcile",
                        "unresolved",
                    )
                }
            )
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=BUILD_ROOT)
    sub = parser.add_subparsers(dest="mode", required=True)
    pre = sub.add_parser("preflight")
    pre.add_argument("--snapshot", type=Path, required=True)
    cap = sub.add_parser("capture")
    cap.add_argument("--snapshot", type=Path, required=True)
    cap.add_argument("--output", type=Path, required=True)
    cap.add_argument("--seconds", type=float, default=90)
    cap.add_argument("--interval-ms", type=float, default=8)
    saved = sub.add_parser("replay")
    saved.add_argument("--capture", type=Path, required=True)
    saved.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.mode == "replay":
        sys.exit(0 if replay(args) else 1)
    live(args)
