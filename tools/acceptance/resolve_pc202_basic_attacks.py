"""Resolve standing quick/strong attack references in the retained PC202 assets."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import sys


ENTRY_ACTIONS = {"quick": 636, "strong": 639}
STANCES = {"high": 0x2DA, "mid": 0x2DB, "low": 0x2DC}
UNSET = 0xFFFF
NATIVE_TEXT_SHA256 = "4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def eligible(reference, guard, input_code=-1, input_mode=0xFF, timing_mode=0):
    # REA 2B2D88 checks the five guards, inversion and OR fields, and skill gates.
    # 20DFC0 uses a separate crossing branch for the shield-spear continuation.
    row = reference.raw
    return (
        reference.group == 0
        and struct.unpack_from("<2h", row, 0x0C) == (-32768, 32767)
        and struct.unpack_from("<3h", row, 0x10) == (input_code, -1, -1)
        and row[0x16] == input_mode
        and row[0x19] == timing_mode
        and struct.unpack_from("<5H", row, 0x20) == (guard, UNSET, UNSET, UNSET, UNSET)
        and row[0x2A] == row[0x38] == 0
        and row[0x3A:0x40] == b"\xff" * 6
    )


def resolve(root, matrix):
    sys.path.insert(
        0, str(root / "project-source/research/research/damage/evaluator/src")
    )
    from action_references import action_references
    from catalog_production import resource_bindings
    from packet_classification import action_packet_flags

    manifest = (
        root / "evidence/research/event-graph-20261111/WEAPON_RESOURCE_MANIFEST.json"
    )
    inventory = (
        root
        / "evidence/research/martial-event-catalog-20261111/ACTION_EVENT_INVENTORY_EXTENDED.json"
    )
    catalog_bytes = inventory.read_bytes()
    matrix_bytes = matrix.read_bytes()
    weapons = json.loads(matrix_bytes)
    if Counter(w["school"] for w in weapons) != {"samurai": 8, "ninja": 8}:
        raise ValueError("Expected the sixteen native form and weapon identities")
    if len({w["action"]["resource"] for w in weapons}) != len(weapons):
        raise ValueError("Repeated weapon resource identity")
    resources, parameters = resource_bindings(root)
    actions = {}
    for record in json.loads(catalog_bytes):
        actions.setdefault((record["namespace"], record["action"]), []).append(record)

    def lookup(resource, action):
        (record,) = actions[resource, action]
        ordinal, parameter = parameters[resource, action, record["parameter_sha256"]]
        return record, parameter, action_references(resources[resource], ordinal)

    def select(references, guard, input_code=-1, input_mode=0xFF, timing_mode=0):
        rows = [
            ref
            for ref in references
            if eligible(ref, guard, input_code, input_mode, timing_mode)
        ]
        (target,) = {ref.target_actions[0] for ref in rows}
        if target is None:
            raise ValueError("Standing reference has no first target")
        return target, [
            {"record": ref.record_index, "sha256": digest(ref.raw)} for ref in rows
        ]

    routes = []
    for weapon in weapons:
        resource = weapon["action"]["resource"]
        school = weapon["school"]
        for kind, entry in ENTRY_ACTIONS.items():
            _, _, references = lookup(resource, entry)
            stances = STANCES if school == "samurai" else {"none": UNSET}
            for stance, guard in stances.items():
                # Hatchet quick submits the ordinary input through mode 1.
                mode_one = (
                    school == "ninja" and weapon["type"] == 14 and kind == "quick"
                )
                selected, refs = select(
                    references, guard, 0 if mode_one else -1, 1 if mode_one else 0xFF
                )
                chain = [{"from": entry, "to": selected, "references": refs}]
                action, parameter, next_refs = lookup(resource, selected)
                if school == "ninja" and weapon["type"] == 17 and kind == "strong":
                    follow, refs = select(next_refs, UNSET, timing_mode=1)
                    chain.append({"from": selected, "to": follow, "references": refs})
                    action, parameter, _ = lookup(resource, follow)
                rows = [
                    {
                        key: row[key]
                        for key in (
                            "row",
                            "sha256",
                            "physical_motion",
                            "elemental_motion",
                            "start",
                            "end",
                            "repeat_interval",
                            "aad_collision_group",
                            "history_expiry",
                        )
                    }
                    for row in action["direct_rows"]
                    if row["physical_motion"] > 0 or row["elemental_motion"] > 0
                ]
                bits = action_packet_flags(parameter)
                if not rows or bits != (0x04000001 if kind == "quick" else 0x04000002):
                    raise ValueError(
                        f"Unexpected scoring action for {resource} {stance} {kind}"
                    )
                routes.append(
                    {
                        "school": school,
                        "weapon": weapon["weapon"],
                        "weapon_class": weapon["type"],
                        "stance": stance,
                        "attack": kind,
                        "action": {
                            "resource": resource,
                            "action": action["action"],
                            "parameter_sha256": action["parameter_sha256"],
                        },
                        "static_packet_flags": bits,
                        "references": chain,
                        "positive_direct_rows": rows,
                        "direct_row_count": len(action["direct_rows"]),
                        "extension_event_count": len(action["extension_events"]),
                    }
                )
    return {
        "schema": "nioh3-pc202-standing-attack-routes/v1",
        "scope": "Static action selection and declared direct rows. Landing and score verification are separate.",
        "native_text_sha256": NATIVE_TEXT_SHA256,
        "asset_manifest_sha256": digest(manifest.read_bytes()),
        "event_inventory_sha256": digest(catalog_bytes),
        "weapon_identity_sha256": digest(matrix_bytes),
        "routes": routes,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--weapon-matrix", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = resolve(args.root, args.weapon_matrix)
    with args.output.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
        output.write("\n")
    print(
        json.dumps(
            {
                "routes": len(report["routes"]),
                "one_positive_row": sum(
                    len(route["positive_direct_rows"]) == 1
                    for route in report["routes"]
                ),
                "output": str(args.output),
            }
        )
    )
