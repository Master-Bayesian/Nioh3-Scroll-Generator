"""Compare a production CPU/device search to an independent whole-cast and slot oracle."""

import argparse
import hashlib
import json
from pathlib import Path
import struct


def read(path):
    return json.loads(path.read_bytes())


def compare(session, root):
    kernel = session / "independent-water-kernel-check"
    accepted = read(session / "contact-window-review/WHOLE_CAST_PARITY.json")
    assert accepted["original_rank_and_three_totals_equal"]
    native = read(kernel / "WHOLE_CAST_CPU_CUDA.json")["ranked"]
    slots = read(kernel / "OWNED_ASSIGNMENTS.json")
    by_id = {r["ordinal"]: r for r in slots["conditional_water_rank"]}
    ids = {r["ordinal"]: r for r in native}
    assert (
        len(native) == len(ids) == accepted["candidate_count"]
        and ids.keys() == by_id.keys()
    )
    ordered = sorted(
        native,
        key=lambda r: (
            -r["components"][0],
            tuple(-1 if i is None else i for i in r["items"]),
        ),
    )
    expected = [
        dict(
            damage=r["components"][0],
            assignments=by_id[r["ordinal"]]["assignments"],
            move_previous_weapon_to_slot=by_id[r["ordinal"]][
                "move_previous_weapon_to_slot"
            ],
        )
        for r in ordered
    ]
    runs = {}
    for backend in ("cpu", "cuda"):
        directory = root / ("search-" + backend)
        result = read(directory / "SEARCH_RESULT.json")
        progress = result["progress"]
        assert (
            result["complete"]
            and result["save_source_sha256"] == slots["source_sha256"]
        )
        assert (
            progress["answered"]
            == progress["completed"]
            == progress["planned"]
            == len(ids)
        )
        assert not any(
            progress.get(k, 0)
            for k in ("no_path", "refused", "source_uncomputed", "outside_limits")
        )
        seen = set()
        for line in (directory / "AUDIT_ROWS.jsonl").read_bytes().splitlines():
            row = json.loads(line)
            ordinal = row["ordinal"]
            assert ordinal not in seen and row["items"] == ids[ordinal]["items"]
            seen.add(ordinal)
            bits = [
                struct.unpack("<I", struct.pack("<f", v))[0]
                for v in ids[ordinal]["components"]
            ]
            fields = (
                ["cpu_components"]
                if backend == "cpu"
                else ["cpu_components", "cuda_components"]
            )
            for field in fields:
                assert [
                    row[field][k] for k in ("total", "physical", "elemental")
                ] == bits, (backend, ordinal, field)
            assert row["outcome"] == {"state": "damage", "bits": bits[0]}
        assert seen == ids.keys()
        ranked = result["ranked"]
        assert len(ranked) == min(400, len(expected))
        for actual, reference in zip(ranked, expected):
            assert all(actual[k] == v for k, v in reference.items()), (
                backend,
                actual,
                reference,
            )
        runs[backend] = dict(
            answered=len(seen),
            ranks=len(ranked),
            backend=result["scorer"],
            search_sha256=hashlib.sha256(
                (directory / "SEARCH_RESULT.json").read_bytes()
            ).hexdigest(),
        )
    reference = dict(
        progress={
            k: progress[k]
            for k in (
                "requested",
                "planned",
                "completed",
                "answered",
                "excluded_by_set_bound",
            )
        },
        ranked=expected[:400],
    )
    (root / "NATIVE_APP_REFERENCE.json").write_text(
        json.dumps(reference), encoding="utf-8"
    )
    schedules = read(session / "contact-window-review/OWNED_SCHEDULES.json")
    counts = [None] * len(ids)
    for ordinal, route in schedules["ordinal_to_route"]:
        assert counts[ordinal] is None
        counts[ordinal] = len(schedules["routes"][route]["windows"])
    assert (
        all(type(n) is int and n > 0 for n in counts)
        and sum(counts) == accepted["scheduled_windows"]
    )
    schedule = dict(
        scene_id=read(root / "read/bundle/bundle.json")["id"],
        domain_sha256=progress["domain_sha256"],
        save_source_sha256=slots["source_sha256"],
        counts=counts,
    )
    (root / "NATIVE_CONTACT_COUNTS.json").write_text(
        json.dumps(schedule), encoding="utf-8"
    )
    return dict(
        original_parity=accepted["original_rank_and_three_totals_equal"],
        runs=runs,
        conditions="Full Ki, frontal all-hit, no additional martial effect.",
        windows=accepted["scheduled_windows"],
        best=expected[0]["damage"],
        winning_ordinals=[
            r["ordinal"] for r in ordered if r["components"][0] == expected[0]["damage"]
        ],
    )


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("session", type=Path)
    p.add_argument("root", type=Path)
    a = p.parse_args()
    receipt = compare(a.session, a.root)
    (a.root / "RANK_PARITY.json").write_text(
        json.dumps(receipt, indent=2), encoding="utf-8"
    )
    print(json.dumps(receipt))
