"""Flip the unresolved direct-actor packet bits in original PC202 MAIN."""

import argparse
import json
import sys
import tempfile
from pathlib import Path


def read(path):
    return json.loads(path.read_bytes())


def run(helper, bundle, cpu, output):
    sys.path.insert(0, str(helper))
    from original_reference import compare

    # The producer can set these for an ordinary tag-0 actor. Bit 28 controls
    # element override and is intentionally tested outside the neutral assumption.
    flags = (1 << 10, 1 << 11, 1 << 20, 1 << 25, 1 << 27)
    masks = (0, *flags, sum(flags), 1 << 28)
    packet = read(bundle / "event_plan.json")["main_address"] + 0x13C
    keys = ("total", "physical", "elemental")
    samples = {}
    for line in (cpu / "NATIVE_INPUTS.jsonl").read_bytes().splitlines():
        row = json.loads(line)
        signature = tuple(tuple(c[k] for k in keys) for c in row["details"]["contacts"])
        samples.setdefault(signature, row)
    hits, comparisons = [], []
    for expected, row in samples.items():
        for mask in masks:
            for pos, hit in enumerate(row["contacts"]):
                if sum(at == packet for at, _ in hit) != 1:
                    raise ValueError("Missing or ambiguous packet flags")
                modified = [[at, v ^ mask if at == packet else v] for at, v in hit]
                hits.append(row["outfit"] + modified)
                comparisons.append((mask, row["ordinal"], pos, expected[pos]))
    with tempfile.TemporaryDirectory(dir=output.parent) as work:
        path = Path(work)
        request = {
            "native_outfit_words": [],
            "native_hit_words": hits,
            "components": {"contacts": [{} for _ in hits]},
        }
        (path / "input.json").write_text(
            json.dumps({"top_case": request}), encoding="utf-8"
        )
        compare(bundle, path / "input.json", path / "native.json")
        actual = read(path / "native.json")["top_case"]["components"]["contacts"]

    changes = {mask: [] for mask in masks}
    for (mask, ordinal, index, expected), scored in zip(
        comparisons, actual, strict=True
    ):
        values = tuple(scored[k] for k in keys)
        if values != expected:
            changes[mask].append(
                {
                    "ordinal": ordinal,
                    "contact": index + 1,
                    "reference": expected,
                    "changed": values,
                }
            )
    receipt = {
        "schema": "pc202-direct-actor-flag-sensitivity/v1",
        "sample_rule": "First outfit with each distinct per-contact total, physical, and elemental sequence.",
        "sampled_outfits": len(samples),
        "native_contacts": len(actual),
        "masks": {
            hex(m): {"changed_contacts": len(changes[m]), "examples": changes[m][:2]}
            for m in masks
        },
        "unresolved_masks_preserved_scores_in_sample": not any(
            changes[m] for m in masks[:-1]
        ),
    }
    output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    if changes[0]:
        raise ValueError("Unmodified original MAIN did not reproduce the input scores")
    print(json.dumps(receipt))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("helper", "bundle", "cpu", "output"):
        parser.add_argument(name, type=Path)
    run(**vars(parser.parse_args()))
