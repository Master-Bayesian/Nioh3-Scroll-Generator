"""Exercise a controlled active effect through ordinary reads and through original MAIN."""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


def load(path):
    return json.loads(path.read_bytes())


def fixture(case, root):
    memory = load(case / "next-ordinary-reader/synthetic-empty-lists.json")
    snap = load(case / "samurai-high/read/snapshot.json")
    owner, actor = (snap["roots"][key] for key in ("owner", "player"))
    regions = memory["regions"]

    def install(address, raw):
        matches = [
            row
            for row in regions
            if row["address"] < address + len(raw)
            and row["address"] + len(row["bytes"]) // 2 > address
        ]
        if not matches:
            regions.append(dict(address=address, bytes=raw.hex()))
        for row in matches:
            offset = address - row["address"]
            data = bytearray.fromhex(row["bytes"])
            assert 0 <= offset and offset + len(raw) <= len(data)
            data[offset : offset + len(raw)] = raw
            row["bytes"] = data.hex()

    (head,) = [
        struct.unpack_from(
            "<Q", bytes.fromhex(r["bytes"]), owner + 0x3328 - r["address"]
        )[0]
        for r in regions
        if r["address"] <= owner + 0x3328
        and r["address"] + len(r["bytes"]) // 2 >= owner + 0x3330
    ]
    base = 0x722000000000
    assert all(not (base <= r["address"] < base + 0x1000) for r in regions)
    node, obj, vt = base, base + 0x100, base + 0x300
    install(head, struct.pack("<Q", node))
    data = bytearray(0x320)
    for off, val in [
        (0, head),
        (0x10, obj),
        (0x100, vt),
        (0x108, actor),
        (0x1A0, 2),
        (0x310, memory["module_base"] + 0x3FAA7C),
    ]:
        struct.pack_into("<Q", data, off, val)
    data[0x114] = 0x20
    struct.pack_into("<I", data, 0x11C, 0x7011)
    struct.pack_into("<f", data, 0x138, -1)
    install(base, data)
    install(owner + 0x2DF4, struct.pack("<f", 0.5))
    regions.sort(key=lambda x: x["address"])
    (root / "active-control-memory.json").write_text(
        json.dumps(memory, separators=(",", ":")), encoding="utf-8"
    )


CHANNELS = ("total", "physical", "elemental")


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def float_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def from_bits(value):
    return struct.unpack("<f", struct.pack("<I", value))[0]


def by_ordinal(path):
    result = {}
    with path.open("rb") as source:
        for line in source:
            row = json.loads(line)
            ordinal = row["ordinal"]
            assert ordinal not in result, (path, ordinal)
            result[ordinal] = row
    return result


def host_pair(root, mode, equipped_root, domain, slots):
    cpu_dir, gpu_dir = (root / f"{mode}-{backend}" for backend in ("cpu", "cuda"))
    cpu = load(cpu_dir / "SEARCH_RESULT.json")
    gpu = load(gpu_dir / "SEARCH_RESULT.json")
    equipped_dir = equipped_root / f"{mode}-equipped"
    equipped_job = load(equipped_dir / "SEARCH_RESULT.json")
    keys = (
        "save_source_sha256",
        "bundle_input_sha256",
        "context_sha256",
        "inventory_file_sha256",
        "required_sets",
        "source_scope",
    )
    assert all(cpu[k] == gpu[k] == equipped_job[k] for k in keys), mode
    assert cpu["save_source_sha256"] == slots["source_sha256"]
    assert cpu["scorer"] == "cpu" and gpu["scorer"] == "cuda"
    assert cpu["complete"] and gpu["complete"]
    assert cpu["ranked"] == gpu["ranked"] and len(cpu["ranked"]) == 400
    cp, gp = cpu["progress"], gpu["progress"]
    for key in (
        "domain_sha256",
        "requested",
        "planned",
        "completed",
        "answered",
        "excluded_by_set_bound",
        "no_path",
        "outside_limits",
        "failures",
        "rank",
    ):
        assert cp[key] == gp[key], (mode, key)
    assert cp["answered"] == cp["planned"] == cp["completed"] == len(domain)
    assert not (cp["no_path"] or cp["outside_limits"] or cp["failures"])
    host = by_ordinal(cpu_dir / "AUDIT_ROWS.jsonl")
    device = by_ordinal(gpu_dir / "AUDIT_ROWS.jsonl")
    assert host.keys() == device.keys() == domain.keys()
    for ordinal, candidate in host.items():
        other = device[ordinal]
        assert candidate["items"] == other["items"] == domain[ordinal]["items"]
        assert candidate["attrs"] == other["attrs"]
        assert candidate["outcome"] == other["outcome"]
        assert candidate["outcome"]["state"] == "damage"
        assert candidate["outcome"]["bits"] == candidate["cpu_components"]["total"]
        assert (
            candidate["cpu_components"]
            == other["cuda_components"]
            == other["cpu_components"]
        )
    if mode == "neutral":
        for ordinal, candidate in host.items():
            assert [candidate["cpu_components"][k] for k in CHANNELS] == [
                float_bits(x) for x in domain[ordinal]["components"]
            ]
    ranked = [(r["ordinal"], r["score_bits"]) for r in cp["rank"]]
    sorted_rows = sorted(
        host.values(),
        key=lambda r: (
            -from_bits(r["cpu_components"]["total"]),
            tuple(-1 if v is None else v for v in r["items"]),
        ),
    )
    slot_map = {r["ordinal"]: r for r in slots["conditional_water_rank"]}
    assert slot_map.keys() == domain.keys()
    for (ordinal, bits), rank, row in zip(
        ranked, cpu["ranked"], sorted_rows[:400], strict=True
    ):
        assert row["ordinal"] == ordinal and row["outcome"]["bits"] == bits, (
            mode,
            ordinal,
        )
        assert all(
            rank[k] == slot_map[ordinal][k]
            for k in ("assignments", "move_previous_weapon_to_slot")
        )
        assert rank["score_bits"] == bits == float_bits(rank["damage"])
    return (
        cpu,
        gpu,
        load(equipped_dir / "EQUIPPED.json")["top_case"]["components"],
        host,
    )


def condition_words(control, mode):
    if mode == "neutral":
        return (0.0, 0, 0.0), 0.0
    count = 0 if mode == "fresh" else 2
    (entry,) = [
        row
        for row in control["stat10_cases"]
        if row["role"] == 0 and row["martial"] and row["count"] == count
    ]
    return (
        from_bits(int(entry["packet_174_bits"], 16)),
        entry["packet_178"] & 0xFFFFFFFF,
        from_bits(int(entry["packet_17c_bits"], 16)),
    ), (0.0 if mode == "fresh" else 0.5)


def action_schedule(mode, neutral, paid, is_equipped):
    ki = neutral["maximum_ki"]
    if mode in ("fresh", "ready-open"):
        actions = neutral["actions"][:3]
        assert actions == [25329, 25330, 25331]
        return [(a, float(ki)) for a in actions]
    if mode == "neutral":
        actions, debits = neutral["actions"], neutral["cost_25329_25338"]
    elif is_equipped:
        # Original per-action costs on the equipped actor; use the established
        # resource prefix and repeat only the final sustain pair until depletion.
        debits = [
            paid["adjusted_equipped"]["true"][str(a)] for a in range(25329, 25339)
        ]
        if len(neutral["actions"]) < 12:
            raise ValueError("Expected the independently verified upgraded route")
        prefix = neutral["actions"][:9]
        assert neutral["actions"][9:-1] == [
            25336 + (i % 2) for i in range(len(neutral["actions"]) - 10)
        ]
        actions = []
        balance = ki
        while balance > 0:
            index = len(actions)
            a = (
                prefix[index]
                if index < len(prefix)
                else 25336 + ((index - len(prefix)) % 2)
            )
            actions.append(a)
            assert debits[a - 25329] > 0
            balance = f32(balance - debits[a - 25329])
        actions.append(25338)
    else:
        assert ki == paid["maximum_ki"]
        actions, debits = paid["paid_actions"], paid["active_adjusted"]
    scheduled = []
    for a in actions:
        ki = f32(ki - debits[a - 25329])
        scheduled.append((a, ki))
    if mode == "paid" and not is_equipped:
        assert ki == paid["final_balance"]
    return scheduled


def compare(case, root, routes, equipped_root, output):
    review = case / "contact-window-review"
    sys.path.insert(0, str(review))
    from water_window_oracle import WaterWindowOracle

    oracle = WaterWindowOracle(Path(__file__).resolve().parents[2], case)
    control = load(review / "rea-36d8/getter-verification/ACTIVE_PACKET.json")
    assert control["status"] == "passed" and control["unknown_reads"] == 0
    info = load(review / "SAMURAI_PACKET.json")
    old = case / "independent-water-kernel-check"
    assert load(review / "WHOLE_CAST_PARITY.json")[
        "original_rank_and_three_totals_equal"
    ]
    original_domain = load(old / "WHOLE_CAST_CPU_CUDA.json")["ranked"]
    slots = load(old / "OWNED_ASSIGNMENTS.json")
    domain = {r["ordinal"]: r for r in original_domain}
    assert len(domain) == len(original_domain) == 1120
    equipped = load(review / "EQUIPPED_WORDS.json")
    old_candidates = by_ordinal(case / "dual-equipment-ki-inputs/NATIVE_INPUTS.jsonl")
    neutral_routes = by_ordinal(case / "native-cost-review/OUTFIT_ACTIONS.jsonl")
    paid_routes = by_ordinal(routes / "PAID_OUTFITS.jsonl")
    actual_neutral = load(case / "native-cost-review/NATIVE_COST_PARITY.json")
    actual_paid = load(routes / "NATIVE_ARTS_COST.json")
    assert (
        set(domain)
        == old_candidates.keys()
        == neutral_routes.keys()
        == paid_routes.keys()
    )
    start_memory = load(case / "samurai-high/read/ordinary-read-memory.json")
    snapshot = load(case / "samurai-high/read/snapshot.json")
    coordinate = snapshot["roots"]["stat"] + 0x1BC
    (original_stat,) = {
        struct.unpack_from(
            "<I", bytes.fromhex(row["bytes"]), coordinate - row["address"]
        )[0]
        for row in start_memory["regions"]
        if row["address"] <= coordinate
        and coordinate + 4 <= row["address"] + len(row["bytes"]) // 2
    }
    worn_neutral = {
        "maximum_ki": actual_neutral["observed_role_flags"]["maximum_ki"],
        "cost_25329_25338": [
            actual_neutral["rows"][str(a)]["adjusted_cost"] for a in range(25329, 25339)
        ],
        "actions": actual_paid["observed_neutral_action_sequence"],
    }
    results = {}
    for mode in ("fresh", "ready-open", "paid", "neutral"):
        cpu, gpu, worn, host = host_pair(root, mode, equipped_root, domain, slots)
        winner = cpu["progress"]["rank"][0]["ordinal"]
        words, additive = condition_words(control, mode)
        oracle.persist(
            oracle.roots["source_owner"] + 0x2DF4, struct.pack("<f", additive)
        )
        results[mode] = {
            "context": cpu["context_sha256"],
            "inventory": cpu["inventory_file_sha256"],
            "backend_assignments_compared": len(host),
            "backend_ranks_compared": len(cpu["ranked"]),
            "winner_ordinal": winner,
            "distinct_keys": None if mode == "neutral" else 0 if mode == "fresh" else 2,
            "source_owner_additive": additive,
            "native_packet_174_bits": hex(float_bits(words[0])),
            "source_artifacts_sha256": {
                f"{backend}/{file}": hashlib.sha256(
                    (root / f"{mode}-{backend}" / file).read_bytes()
                ).hexdigest()
                for backend in ("cpu", "cuda")
                for file in ("SEARCH_RESULT.json", "AUDIT_ROWS.jsonl")
            },
        }
        for name, inputs, neutral, paid, observed in [
            ("equipped", equipped, worn_neutral, actual_paid, worn),
            (
                "winner",
                old_candidates[winner],
                neutral_routes[winner],
                paid_routes[winner],
                cpu["top_case"]["components"],
            ),
        ]:
            stat_at = oracle.roots["stat"] + 0x1BC
            assert dict(inputs["outfit"]).get(stat_at, original_stat) == 10
            schedule = action_schedule(mode, neutral, paid, name == "equipped")
            native = []
            for action, ki in schedule:
                flags = int(info["actions"][str(action)]["class_bits"], 16)
                for row in (0, 1):
                    hit = oracle.score(
                        action,
                        row,
                        inputs["outfit"],
                        inputs["contacts"][0],
                        packet_flags=flags | (0 if mode == "neutral" else 0x80),
                        ki=ki,
                        stance=0,
                        group104=words,
                    )
                    if "components" not in hit:
                        raise ValueError(
                            f"Original MAIN could not score {mode}/{name}/{action}/{row}: {hit}"
                        )
                    native.append(tuple(hit["components"]))
            details = observed["contacts"]
            assert [h["contact"] for h in details] == list(range(1, len(details) + 1))
            assert [h["template"] for h in details] == [
                2 * (a - 25329) + r + 1 for a, _ in schedule for r in (0, 1)
            ]
            assert len(native) == len(details), (mode, name, len(native), len(details))
            measured = [tuple(h[c] for c in CHANNELS) for h in details]
            if native != measured:
                i = next(
                    i
                    for i, (n, m) in enumerate(zip(native, measured, strict=True))
                    if n != m
                )
                raise ValueError(
                    f"Contact mismatch {mode}/{name}/{i + 1}: native={native[i]}, App={measured[i]}"
                )
            totals = list(map(sum, zip(*native, strict=True)))
            assert totals == [observed[c] for c in CHANNELS]
            if name == "winner":
                assert observed == gpu["top_case"]["components"]
                assert tuple(float_bits(n) for n in totals) == tuple(
                    host[winner]["cpu_components"][c] for c in CHANNELS
                )
            results[mode][name] = dict(
                actions=len(schedule),
                totals=totals,
                schedule=schedule,
                original_contacts=native,
            )
            print(
                f"{mode}/{name}: {len(native)} original contacts {totals}", flush=True
            )
    output.parent.mkdir(exist_ok=True, parents=True)
    output.write_text(
        json.dumps(
            dict(
                scope="Retained original PC202 MAIN, independent action routes, candidate words bound to the same owned inputs. "
                "Equipped and winner compared per contact; all host/device candidate totals, physical/elemental channels, "
                "coverage, retained ties and placement compared. All published ranks independently reconstructed from the candidate scores and owned inventory slots. Neutral also matches the previous native domain for every candidate. "
                "Active objects and fresh grant are offline controlled starting conditions.",
                modes=results,
            ),
            indent=2,
        )
        + "\n",
        encoding="utf8",
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True, dest="command")
    fixture_command = commands.add_parser("fixture")
    fixture_command.add_argument("case", type=Path)
    fixture_command.add_argument("root", type=Path)
    checker = commands.add_parser("compare")
    for key in ("case", "root"):
        checker.add_argument(key, type=Path)
    repo = Path(__file__).resolve().parents[2]
    receipt_root = repo / "deliverables/arts-and-sources-20261121/native-comparison"
    checker.add_argument(
        "routes",
        type=Path,
        nargs="?",
        default=repo / "deliverables/arts-ki-native-20261121",
    )
    checker.add_argument("equipped_root", type=Path, nargs="?", default=receipt_root)
    checker.add_argument(
        "output", type=Path, nargs="?", default=receipt_root / "NATIVE_RANK_PARITY.json"
    )
    args = parser.parse_args()
    if args.command == "fixture":
        fixture(args.case, args.root)
    else:
        compare(args.case, args.root, args.routes, args.equipped_root, args.output)
