"""Resolve known PC202 source requests against each outfit and a declared event."""

import argparse
from collections import Counter
import csv
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import struct

from check_ninja_status_sources import ROLE


# PC202 845970 reads the new form twice, on opposite sides of 376C6C / 846514.
# The early bank is the effective D after the form store, before general rebuild.
STYLE_SHIFT_RULES = (
    ("before_rebuild", 0x865, None, 0x1A1, 0x16B),
    ("after_migration", 0x72A, 1, 0x278, 0x396),
    ("after_migration", 0x72A, 0, 0x1DA, 0x37B),
)


@dataclass
class CandidateBank:
    """One effective materializer bank; missing template bytes stay unknown."""

    base: int
    captured: bytes
    known: bytes
    words: dict[int, int]

    def value(self, offset, condition):
        size = 1 if condition == "byte_nonzero" else 4
        raw = bytearray()
        for at in range(offset, offset + size):
            word = self.words.get(self.base + (at & ~3))
            if word is not None:
                raw.append((word >> (8 * (at % 4))) & 255)
            elif at < len(self.known) and self.known[at]:
                raw.append(self.captured[at])
            else:
                return None
        return raw[0] if size == 1 else struct.unpack("<f", raw)[0]

    @classmethod
    def complete(cls, data):
        if len(data) != 0xB00:
            raise ValueError("Expected one complete 0xB00-byte PC202 materialized D")
        return cls(0, data, bytes([1]) * len(data), {})


def style_shift_requests(previous_form, requested_form, pending, early, late):
    """Conditional requests, in execution order. None means a missing stage field."""
    if previous_form not in (0, 1) or requested_form not in (0, 1, 2):
        raise ValueError("Unexpected PC202 form or pending-target sentinel")
    changed = pending and requested_form != 2 and previous_form != requested_form
    results = {}
    for phase, offset, destination, source, _ in STYLE_SHIFT_RULES:
        if not changed or (destination is not None and requested_form != destination):
            result = False
        else:
            bank = early if phase == "before_rebuild" else late
            result = admitted(
                bank.value(offset, "byte_nonzero") if bank else None, "byte_nonzero"
            )
        results[f"{source:#x}/self"] = result
    return results


def read(path):
    return json.loads(path.read_bytes())


def rules_for_sources(path):
    rules = []
    with path.open(encoding="utf-8-sig", newline="") as source:
        for row in csv.DictReader(source):
            if row["source_gate"] == "unknown_in_reviewed_handlers":
                continue
            for gate in row["source_gate"].split("|"):
                handler, offset, condition, recipient = gate.split(":")
                if condition not in {"byte_nonzero", "f32_positive"}:
                    raise ValueError(f"Unrecognized source comparison: {condition}")
                rules.append(
                    (
                        handler,
                        int(offset.removeprefix("D+"), 16),
                        condition,
                        recipient,
                        int(row["source"], 16),
                        int(row["status_key"], 16),
                    )
                )
    return rules


def admitted(value, condition):
    if value is None:
        return None
    return value != 0 if condition == "byte_nonzero" else value > 0.0


def snapshot_values(regions, base, offset, condition):
    size = 1 if condition == "byte_nonzero" else 4
    address = base + offset
    values = set()
    for row in regions:
        i = address - row["address"]
        if i >= 0 and i + size <= len(row["bytes"]) // 2:
            values.add(bytes.fromhex(row["bytes"][2 * i : 2 * (i + size)]))
    if len(values) > 1:
        raise ValueError("Conflicting retained role bytes")
    if not values:
        return None
    return struct.unpack("<B" if size == 1 else "<f", values.pop())[0]


def original_snapshots(area, rules):
    references = {}
    with (area / "rea-source-rules/WORN_GATE_EVIDENCE.csv").open(
        encoding="utf-8-sig", newline=""
    ) as f:
        for row in csv.DictReader(f):
            key = (
                row["case"],
                row["handler"],
                int(row["source"], 16),
                row["recipient"],
            )
            references[key] = {"True": True, "False": False, "": None}[row["gate_open"]]
    seen = set()
    for export in (area / "cases").glob("*/export"):
        snap = read(export / "snapshot.json")
        mem = read(export / "ordinary-read-memory.json")
        for handler, offset, condition, recipient, src, _ in rules:
            key = (export.parent.name, handler, src, recipient)
            value = snapshot_values(
                mem["regions"], snap["roots"]["materializer"], offset, condition
            )
            if key not in references or admitted(value, condition) != references[key]:
                raise ValueError(f"Role comparison differs: {key}")
            seen.add(key)
    if not seen or seen != references.keys():
        raise ValueError("Incomplete retained role or event rule coverage")
    return len(seen)


def page_slice(path, base, size, mask):
    page = (base & ~65535).to_bytes(8, "little")
    stride = 8200 if mask else 65544
    raw = path.read_bytes()
    if len(raw) % stride:
        raise ValueError(f"Invalid page records in {path}")
    (start,) = [i + 8 for i in range(0, len(raw), stride) if raw[i : i + 8] == page]
    offset = base & 65535
    if offset + size > 65536:
        raise ValueError("Expected gate fields in a single retained page")
    if mask:
        return bytes(
            bool(raw[start + (j // 8)] & (1 << (j % 8)))
            for j in range(offset, offset + size)
        )
    return raw[start + offset : start + offset + size]


def fields_for_candidate(words, base, captured, known, rules):
    bank = CandidateBank(base, captured, known, words)
    return {
        f"{source:#x}/{recipient}": admitted(bank.value(offset, comparison), comparison)
        for _, offset, comparison, recipient, source, _ in rules
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("names", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument(
        "--shift-banks",
        type=Path,
        nargs=2,
        metavar=("EARLY_D", "LATE_D"),
        help="Two complete PC202 D images explicitly bound to before rebuild and after migration.",
    )
    parser.add_argument(
        "--shift-to",
        choices=("samurai", "ninja"),
        help="Destination of the explicit two-stage shift.",
    )
    args = parser.parse_args()
    if bool(args.shift_banks) != bool(args.shift_to):
        parser.error("--shift-banks and --shift-to must be supplied together")
    area = args.evidence / "optimizer-action-buff-20261121"
    rules = rules_for_sources(area / "rea-source-rules/SOURCE_STATUS_RULES.csv")
    name_data = read(args.names)
    by_status = {row["status_key"]: row for row in name_data["joined"]}
    catalog = []
    for handler, offset, comparison, recipient, source, status in rules:
        label = by_status[status]
        if source not in label["sources"]:
            raise ValueError(
                "Displayed status differs from the native source definition"
            )
        catalog.append(
            dict(
                handler=handler,
                source=hex(source),
                status=hex(status),
                recipient=recipient,
                offset=hex(offset),
                comparison=comparison,
                name=label["name"],
            )
        )
    checked = original_snapshots(area, rules)
    style_catalog = []
    for stage, offset, destination, source, status in STYLE_SHIFT_RULES:
        label = by_status[status]
        if source not in label["sources"]:
            raise ValueError("Style shift source differs from its displayed status")
        style_catalog.append(
            dict(
                handler="845970",
                stage=stage,
                source=hex(source),
                status=hex(status),
                recipient="self",
                offset=hex(offset),
                destination=destination,
                name=label["name"],
            )
        )
    ninja = [r for r in rules if r[0] == "8437a0"]
    area_role = args.evidence / ROLE / "all-ninja"
    ref = read(area / "ninja-evade/CANDIDATE_STATUS_SOURCES.json")
    recipe = (
        Path(__file__).resolve().parents[2]
        / "crates/nioh3-runtime/src/combat_context/kusarigama_strong_pc202.json"
    )
    base = read(recipe)["roots"]["d"]
    results, shift_results, pair = {}, {}, {}
    for name, profile in ref["inputs"].items():
        case = area_role / name
        ctx = case / "target-bundle/context"
        captured = page_slice(ctx / "image.bin", base, 0xB00, False)
        mask = page_slice(ctx / "image_known.bin", base, 0xB00, True)
        raw = (case / "native-inputs/NATIVE_INPUTS.jsonl").read_bytes()
        if hashlib.sha256(raw).hexdigest() != profile["native_inputs"]:
            raise ValueError(f"Changed candidate inputs for {name}")
        signatures, shifts, prior, seen = Counter(), Counter(), Counter(), set()
        for line in raw.splitlines():
            row = json.loads(line)
            ordinal = row["ordinal"]
            if ordinal in seen:
                raise ValueError("Repeated candidate identity")
            seen.add(ordinal)
            words = dict(row["outfit"])
            gates = fields_for_candidate(words, base, captured, mask, ninja)
            signatures[json.dumps(gates, sort_keys=True)] += 1
            # This candidate is an already rebuilt Ninja outfit. Its early stage
            # was never recorded, so the early-only source remains unanswered.
            requests = style_shift_requests(
                0, 1, True, None, CandidateBank(base, captured, mask, words)
            )
            shifts[json.dumps(requests)] += 1
            bits = [
                gates[f"{int(g['source_key'], 16):#x}/self"] for g in ref["gate_order"]
            ]
            if None in bits:
                raise ValueError("Missing independently validated equipment field")
            prior["".join(str(int(x)) for x in bits)] += 1
            if name == "ninja-type-6-contact" and ordinal in (1, 17):
                pair[ordinal] = gates
        if dict(prior) != ref["roles"][name]:
            raise ValueError(
                f"Independent gear and skill derivation differs for {name}"
            )
        results[name] = [
            dict(candidate_count=n, inner_gate_open=json.loads(k))
            for k, n in sorted(signatures.items())
        ]
        shift_results[name] = [
            dict(candidate_count=n, source_requests=json.loads(k))
            for k, n in sorted(shifts.items())
        ]
    if set(pair) != {1, 17} or pair[1] == pair[17]:
        raise ValueError("The same-type held-weapon change must discriminate")
    output = dict(
        scope="Open inner gates conditional on the stated combat event. Null = missing bytes. "
        "Registration, current activity, duration and simultaneous winners are separate.",
        rules=catalog,
        retained_role_comparisons=checked,
        ninja_evade_candidates=results,
        same_type_weapon_pair=pair,
        style_shift=dict(
            rules=style_catalog,
            scope="Conditional on a successful shift into each rebuilt Ninja outfit. "
            "The before-rebuild field is unknown. Results concern grant requests only.",
            ninja_destination_candidates=shift_results,
        ),
    )
    if args.shift_banks:
        early_data, late_data = (path.read_bytes() for path in args.shift_banks)
        destination = int(args.shift_to == "ninja")
        output["style_shift"]["explicit_pair"] = dict(
            previous_form=1 - destination,
            requested_form=destination,
            early_sha256=hashlib.sha256(early_data).hexdigest(),
            late_sha256=hashlib.sha256(late_data).hexdigest(),
            source_requests=style_shift_requests(
                1 - destination,
                destination,
                True,
                CandidateBank.complete(early_data),
                CandidateBank.complete(late_data),
            ),
        )
    args.output.write_text(
        json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(
        f"{len(rules)} single-stage rules, {len(style_catalog)} Style Shift rules, "
        f"{checked} retained role checks, {len(results)} owned type domains."
    )


if __name__ == "__main__":
    main()
