"""Compare fixed-cast rankings and original PC202 damage for every outfit."""

import argparse
from itertools import islice
import json
from pathlib import Path
import struct
import sys
import tempfile


def read(path):
    return json.loads(path.read_bytes())


def run(helper, bundle, cpu, cuda, assumptions, output):
    sys.path.insert(0, str(helper))
    from original_reference import compare

    host, device = [read(p / "SEARCH_RESULT.json") for p in (cpu, cuda)]
    if (
        not host["complete"]
        or not device["complete"]
        or host["ranked"] != device["ranked"]
    ):
        raise ValueError("Search did not cover the whole domain or rankings differ")
    rows = [
        json.loads(line)
        for line in (cpu / "NATIVE_INPUTS.jsonl").read_bytes().splitlines()
    ]
    gpu = {
        row["ordinal"]: row["cuda_components"]
        for line in (cuda / "AUDIT_ROWS.jsonl").read_bytes().splitlines()
        if (row := json.loads(line))
    }
    planned = host["progress"]["planned"]
    if (
        {row["ordinal"] for row in rows} != set(gpu)
        or len(rows) != len(gpu)
        or len(gpu) != planned
    ):
        raise ValueError("Candidate identities differ")
    hits = []
    for row in rows:
        if not row["contacts"]:
            raise ValueError(f"Candidate {row['ordinal']} has no contacts")
        hits.extend(row["outfit"] + contact for contact in row["contacts"])
    with tempfile.TemporaryDirectory(dir=output.parent) as tmp:
        path = Path(tmp)
        case = dict(
            native_outfit_words=[],
            native_hit_words=hits,
            components={"contacts": [{} for _ in hits]},
        )
        (path / "input.json").write_text(
            json.dumps({"top_case": case}), encoding="utf8"
        )
        compare(bundle, path / "input.json", path / "native.json")
        native = read(path / "native.json")["top_case"]["components"]["contacts"]
    elemental = 0
    stream = iter(native)
    for row in rows:
        count = len(row["contacts"])
        contacts = list(islice(stream, count))
        if len(contacts) != count:
            raise ValueError("Missing original MAIN contact")
        details = row.get("details", {})
        if count > 1 or details:
            if details.get("state") != "available":
                raise ValueError("Missing translated per-contact results")
            for index, (replayed, scored) in enumerate(
                zip(contacts, details["contacts"], strict=True), 1
            ):
                if scored["contact"] != index or any(
                    struct.pack("<f", scored[k]) != struct.pack("<f", replayed[k])
                    for k in ("total", "physical", "elemental")
                ):
                    raise ValueError(
                        f"Candidate {row['ordinal']} contact {index} differs"
                    )
        for key in ("total", "physical", "elemental"):
            total = 0.0
            for hit in contacts:
                (total,) = struct.unpack("<f", struct.pack("<f", total + hit[key]))
            (bits,) = struct.unpack("<I", struct.pack("<f", total))
            if bits != row["expected"][key] or bits != gpu[row["ordinal"]][key]:
                raise ValueError(f"Candidate {row['ordinal']} differs in {key}")
        elemental += any(hit["elemental"] != 0 for hit in contacts)
    if next(stream, None) is not None:
        raise ValueError("Unexpected extra original MAIN contact")
    result = dict(
        candidates=planned,
        contacts=len(hits),
        with_elemental_damage=elemental,
        best=host["ranked"][0]["damage"],
        ranked_assignments=len(host["ranked"]),
        native_cpu_cuda_identical=True,
        context=host["context_sha256"],
        conditions=read(assumptions),
    )
    output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf8")
    print(json.dumps(result))


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ("helper", "bundle", "cpu", "cuda", "assumptions", "output"):
        p.add_argument(arg, type=Path)
    run(**vars(p.parse_args()))
