"""Verify that the original actor element override reached every scored cast contact."""

import argparse
import hashlib
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_bytes())


def audit(path):
    rows = [json.loads(line) for line in path.read_bytes().splitlines()]
    indexed = {r["ordinal"]: r for r in rows}
    if len(indexed) != len(rows):
        raise ValueError(f"Duplicate candidate in {path}")
    return indexed


def compare(directory, previous):
    scores, results, identities = {}, {}, {}
    for name in ("inactive", "element-zero"):
        case = directory / name
        parity = read(case / "PARITY.json")
        cpu = read(case / "cpu/SEARCH_RESULT.json")
        gpu_root = case / (
            "device-profile" if (case / "device-profile").exists() else "cuda"
        )
        gpu = read(gpu_root / "SEARCH_RESULT.json")
        device = gpu["performance"]["device"]["native"]
        if (
            not parity["native_cpu_cuda_identical"]
            or not gpu["complete"]
            or gpu["scorer"] != "cuda"
            or gpu["ranked"] != cpu["ranked"]
            or device["contacts"] != parity["contacts"]
            or device["kernel_ms"] <= 0
            or device["d2h_bytes"] <= 0
        ):
            raise ValueError(f"{name}: incomplete native or device ranking")
        plan = read(case / "observed/bundle/event_plan.json")
        main = plan["main_address"]
        inputs = audit(case / "cpu/NATIVE_INPUTS.jsonl")
        ids = audit(case / "cpu/AUDIT_ROWS.jsonl")
        if inputs.keys() != ids.keys() or len(ids) != parity["candidates"]:
            raise ValueError(f"{name}: candidate sets differ")
        checked = 0
        for row in inputs.values():
            for contact in row["contacts"]:
                words = dict(row["outfit"] + contact)
                if bool(words[main + 0x13C] & (1 << 28)) != (name == "element-zero"):
                    raise ValueError(f"{name}: inaccurate packet flag")
                if name == "element-zero" and (
                    words[main + 0x14],
                    words[main + 0x18],
                ) != (0, 101):
                    raise ValueError("Weapon preparation overwrote the actor element")
                checked += 1
        if checked != parity["contacts"]:
            raise ValueError(f"{name}: native contact count differs")
        scores[name] = {i: x["expected"] for i, x in inputs.items()}
        identities[name] = {i: x["items"] for i, x in ids.items()}
        results[name] = {
            "native_parity": parity,
            "native_device_contacts": device["contacts"],
            "kernel_ms": device["kernel_ms"],
            "d2h_bytes": device["d2h_bytes"],
            "first_assignments": cpu["ranked"][0]["assignments"],
        }
        if name == "inactive" and previous:
            old = read(previous / "SEARCH_RESULT.json")
            old_rows = audit(previous / "AUDIT_ROWS.jsonl")
            if (
                old["ranked"] != cpu["ranked"]
                or old_rows.keys() != ids.keys()
                or any(
                    (x["items"], x["cpu_components"])
                    != (ids[i]["items"], ids[i]["cpu_components"])
                    for i, x in old_rows.items()
                )
            ):
                raise ValueError("Inactive branch changed the prior ranking")
            results[name]["identical_previous_candidates"] = len(ids)
            results[name]["previous_report_sha256"] = hashlib.sha256(
                (previous / "SEARCH_RESULT.json").read_bytes()
            ).hexdigest()
    if identities["inactive"] != identities["element-zero"]:
        raise ValueError("The two controls searched different outfits")
    before, after = scores["inactive"], scores["element-zero"]
    return {
        "schema": "pc202-direct-element-ranks/v1",
        "cases": results,
        "changes": {
            k: sum(before[i][k] != after[i][k] for i in before)
            for k in ("physical", "elemental", "total")
        },
        "winner_assignment_changed": results["inactive"]["first_assignments"]
        != results["element-zero"]["first_assignments"],
        "scope": "Synthetic role inputs compared within the same saved inventory and fixed cast.",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--previous", type=Path)
    args = parser.parse_args()
    summary = compare(args.directory, args.previous)
    args.output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary["changes"]))
