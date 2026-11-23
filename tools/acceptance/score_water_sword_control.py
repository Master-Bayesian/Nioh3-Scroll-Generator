"""Score the current-role, uninterrupted full-hit control using the PC202 original MAIN oracle."""

import argparse
import json
from collections import defaultdict
import hashlib
import math
from pathlib import Path
import struct
import sys


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read(path):
    return json.loads(path.read_bytes())


def records(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()]


class NativeReads:
    """Reuse only when all potentially variable bytes read by a successful execution agree."""

    def __init__(self, oracle, variable):
        self.variable = variable
        self.seen = set()
        self.layouts = {}
        self.executions = self.reused = self.controls = 0
        self.oracle = oracle
        self.ki = oracle.roots["source_owner"] + 0xAC
        oracle.vm.uc.hook_add(oracle.vm.U.UC_HOOK_MEM_READ, self.observe)

    def observe(self, _machine, _access, addr, size, _value, _user):
        self.seen.update(i for i in range(addr, addr + size) if i in self.variable)

    def score(self, action, row, loadout, values, ki, flags):
        layouts = self.layouts.setdefault((action, row, flags), {})
        values.update(zip(range(self.ki, self.ki + 4), struct.pack("<f", ki)))
        for addresses, previous in layouts.items():
            key = tuple(values.get(i, -1) for i in addresses)
            if key in previous:
                self.reused += 1
                # Periodically compare a reused result against direct execution.
                if self.reused % 257:
                    return previous[key]
                self.controls += 1
                break
        else:
            previous = None
        self.seen.clear()
        answer = self.oracle.score(
            action,
            row,
            loadout["outfit"],
            loadout["contacts"][0],
            packet_flags=flags,
            ki=ki,
            stance=0,
            group104=(0.0, 0, 0.0),
        )
        self.executions += 1
        if "components" not in answer:
            raise ValueError(f"{loadout['ordinal']}:{action}:{row} {answer}")
        # The observed finisher has split debits. Its pre/post Ki must be immaterial
        # for these results regardless of which tick the benchmark contact would land on.
        if action == 25338 and any(i in self.seen for i in range(self.ki, self.ki + 4)):
            raise ValueError(
                "Finisher reads Ki; bind the balance at each contact first"
            )
        result = tuple(answer["components"])
        if previous is not None and result != previous[key]:
            raise ValueError("Witness reuse differs from direct original MAIN")
        layout = tuple(sorted(self.seen))
        layouts.setdefault(layout, {})[tuple(values.get(i, -1) for i in layout)] = (
            result
        )
        return result


def main(args):
    session = args.session
    sys.path.insert(0, str(session / "contact-window-review"))
    from water_window_oracle import WaterWindowOracle

    oracle = WaterWindowOracle(Path(__file__).resolve().parents[2], session)
    costs = {
        r["ordinal"]: r
        for r in records(session / "native-cost-review/OUTFIT_ACTIONS.jsonl")
    }
    loadouts = records(session / "dual-equipment-ki-inputs/NATIVE_INPUTS.jsonl")
    program = read(session / "contact-window-review/OWNED_SCHEDULES.json")
    route_ids = dict(program["ordinal_to_route"])
    if set(costs) != set(route_ids) or {r["ordinal"] for r in loadouts} != set(costs):
        raise ValueError("Different candidate domains")
    all_words = {a for row in loadouts for a, _ in row["outfit"] + row["contacts"][0]}
    all_words.add(oracle.roots["source_owner"] + 0xAC)
    cache = NativeReads(oracle, {a + b for a in all_words for b in range(4)})
    groups = defaultdict(list)
    for candidate in loadouts:
        index = candidate["ordinal"]
        values = {}
        for a, v in candidate["outfit"] + candidate["contacts"][0]:
            values.update(zip(range(a, a + 4), struct.pack("<I", v)))
        cost = costs[index]
        ki = float(cost["maximum_ki"])
        route = program["routes"][route_ids[index]]
        if route["actions"] != cost["actions"]:
            raise ValueError("Route lost its candidate owner")
        for step, action in enumerate(route["actions"]):
            ki = f32(ki - cost["cost_25329_25338"][action - 25329])
            for hit in program["actions"][str(action)]["rows"]:
                groups[action, hit["order"]].append(
                    (index, step, ki, candidate, values)
                )
    scores = defaultdict(dict)
    for (action, row), occurrences in groups.items():
        flags = int(oracle.data[str(action)]["class_bits"], 16) | 0x80
        for index, step, ki, candidate, values in occurrences:
            scores[index][step, row] = cache.score(
                action, row, candidate, values, ki, flags
            )
        print(f"Scored {action}/{row}: {len(occurrences)} windows", flush=True)
    ranked = []
    for idx, entries in scores.items():
        totals = [0.0, 0.0, 0.0]
        route = program["routes"][route_ids[idx]]
        for step, _action, row in route["windows"]:
            totals = [
                f32(a + b) for a, b in zip(totals, entries[step, row], strict=True)
            ]
        ranked.append(dict(ordinal=idx, items=costs[idx]["items"], components=totals))
    ranked.sort(key=lambda r: (-r["components"][0], r["ordinal"]))
    result = dict(
        scope="Original PC202 MAIN. High stance, benchmark frontal full hits, initial candidate source bank, grounded control, inactive packet 104. Diagnostic ordinal tie order.",
        windows=sum(len(x) for x in groups.values()),
        candidates=len(ranked),
        original_calls=cache.executions,
        reused=cache.reused,
        direct_rechecks=cache.controls,
        cpu_cuda_parity=False,
        ranked=ranked,
    )
    with args.output.open("x", encoding="utf8") as out:
        json.dump(result, out)
        out.write("\n")
    print(json.dumps({k: v for k, v in result.items() if k != "ranked"}))


def compare(session, original_result):
    host = read(original_result)
    gpu = read(session / "independent-water-kernel-check/WHOLE_CAST_CPU_CUDA.json")
    gear = read(session / "independent-water-kernel-check/OWNED_ASSIGNMENTS.json")
    files = {
        "input_sha256": session / "dual-equipment-ki-inputs/NATIVE_INPUTS.jsonl",
        "program_sha256": session / "contact-window-review/OWNED_SCHEDULES.json",
    }
    for key, file in files.items():
        if gpu[key] != hashlib.sha256(file.read_bytes()).hexdigest():
            raise ValueError(f"Original/device input identity differs: {key}")
    plan = read(files["program_sha256"])
    ranked = host["ranked"]
    ids = {n for n, _ in plan["ordinal_to_route"]}
    if (
        not gpu["cpu_cuda_per_window_agree"]
        or gpu["unanswered"]
        or gpu["answered"] != len(ids)
        or len(ranked) != len(ids)
        or {r["ordinal"] for r in ranked} != ids
        or sum(len(r["windows"]) * r["outfits"] for r in plan["routes"])
        != gpu["successful_windows"]
        or host["windows"] != gpu["successful_windows"]
        or host["original_calls"] + host["reused"] - host["direct_rechecks"]
        != host["windows"]
        or ranked != gpu["ranked"]
    ):
        raise ValueError("Incomplete or different original and device rankings")
    inventory = read(session / "samurai-high/read/inventory.json")
    pool = sorted(inventory["records"], key=lambda item: item["slot"])
    if gear["source_sha256"] != inventory["source_sha256"] or any(
        hashlib.sha256(bytes(item["record_bytes"])).hexdigest() != item["record_sha256"]
        for item in pool
    ):
        raise ValueError("Returned inventory identity differs from the role read")
    by_slot = {item["slot_index"]: item for item in pool}
    if len(by_slot) != len(pool):
        raise ValueError("Ambiguous save record identity")
    equipment = read(session / "samurai-high/read/bundle/context/context.json")[
        "character"
    ]["active"]
    if len(gear["conditional_water_rank"]) != len(ranked):
        raise ValueError("Missing equipment assignments")
    for index, mapped in enumerate(gear["conditional_water_rank"]):
        expected = ranked[index]
        returned = {a["slot"]: a for a in mapped["assignments"]}
        # A second accessory may be selected from the first accessory's source pool.
        selected = {
            (0 if position == 9 else position + 4): pool[i]
            for position, i in enumerate(expected["items"])
            if i is not None
        }
        if set(selected) != set(returned) or len(returned) != len(
            mapped["assignments"]
        ):
            raise ValueError("Returned slots differ from the scored outfit")
        for slot, assignment in returned.items():
            owned = by_slot[assignment["save_slot"]]
            if (
                selected[slot] is not owned
                or assignment["record_sha256"] != owned["record_sha256"]
            ):
                raise ValueError("Returned slot belongs to a different item")
        weapon_placement = None
        if 0 in selected:
            start = 4 * equipment
            bank_slot = int.from_bytes(
                bytes(selected[0]["assignment_bytes"])[start : start + 4], "little"
            )
            if bank_slot in (1, 2, 3):
                weapon_placement = bank_slot
        if mapped["move_previous_weapon_to_slot"] != weapon_placement:
            raise ValueError("Displaced weapon belongs to a different slot")
        if (
            expected["ordinal"] != mapped["ordinal"]
            or expected["components"] != mapped["components"]
            or not all(math.isfinite(x) for x in expected["components"])
        ):
            raise ValueError("Assignment does not belong to the scored candidate")
    best = ranked[0]["components"][0]
    return {
        "scope": gpu["scope"],
        "candidate_count": len(ids),
        "scheduled_windows": host["windows"],
        "cpu_cuda_per_window_equal": True,
        "original_rank_and_three_totals_equal": True,
        "original_calls": host["original_calls"],
        "original_exact_reuses_without_reexecution": host["reused"]
        - host["direct_rechecks"],
        "direct_rechecks": host["direct_rechecks"],
        "best": best,
        "top_ties": sum(r["components"][0] == best for r in ranked),
        "gear_identity": "independent-water-kernel-check/OWNED_ASSIGNMENTS.json",
        "independently_observed_game_damage": False,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--session", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify-original", type=Path)
    args = parser.parse_args()
    if args.verify_original:
        checked = compare(args.session, args.verify_original)
        with args.output.open("x", encoding="utf8") as out:
            json.dump(checked, out, indent=2)
            out.write("\n")
        print(json.dumps(checked))
    else:
        main(args)
