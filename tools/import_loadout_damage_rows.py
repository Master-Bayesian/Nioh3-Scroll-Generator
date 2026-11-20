"""Build the address-independent PC202 damage row index from the reviewed action inventory."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def build(inventory, selections):
    def identity(row, namespace):
        return row[namespace], row["action"], row["parameter_sha256"]

    expected = {identity(row, "resource") for row in selections}
    seen_actions = set()
    entries = {}
    bodies = {}
    for action in inventory:
        action_id = identity(action, "namespace")
        if action_id in seen_actions:
            raise ValueError("Duplicate action identity")
        seen_actions.add(action_id)
        namespace, number, parameter = action_id
        for row in action["direct_rows"]:
            body = bytes.fromhex(row["raw_hex"])
            checksum = hashlib.sha256(body).digest()
            if len(body) != 192 or checksum.hex() != row["sha256"]:
                raise ValueError(f"Damage row differs from catalog: {action_id}, {row['row']}")
            if struct.unpack_from("<hh", body, 16) != (row["physical_motion"], row["elemental_motion"]):
                raise ValueError("Decoded motion differs from the original row")
            key = hashlib.sha256(namespace.encode() + b"\0" + struct.pack("<HI", number, row["row"]) + bytes.fromhex(parameter)).digest()
            if key in entries or bodies.setdefault(checksum, body) != body:
                raise ValueError("Conflicting damage row identity")
            entries[key] = checksum
    if seen_actions != expected:
        raise ValueError("Damage row source and action catalog cover different identities")
    ordered = sorted(bodies)
    slots = {checksum: i for i, checksum in enumerate(ordered)}
    return (struct.pack("<8sII", b"N3AAD001", len(entries), len(ordered))
            + b"".join(key + struct.pack("<I", slots[entries[key]]) for key in sorted(entries))
            + b"".join(bodies[checksum] for checksum in ordered))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inventory", type=Path)
    parser.add_argument("catalog", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_bytes(build(json.loads(args.inventory.read_bytes()),
                                  json.loads(args.catalog.read_bytes())["selections"]))
