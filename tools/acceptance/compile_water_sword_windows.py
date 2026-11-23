"""Join a projected PC202 Water Sword sustain route to its benchmark contact rows."""

import argparse
import hashlib
import json
from pathlib import Path
import struct

from capture_water_sword_poll import water_actions


def read(path):
    return json.loads(path.read_bytes())


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def compile_schedules(root, session):
    review = session / "contact-window-review"
    resource = read(review / "CONTACT_WINDOWS.json")
    packet = read(review / "SAMURAI_PACKET.json")
    base = session / "native-cost-review"
    cost = read(base / "OUTFIT_COST_PARITY.json")
    observed = read(session / "water-sword-high-02/REPLAY.json")
    rows_file = (base / "OUTFIT_ACTIONS.jsonl").read_bytes()
    if cost["sha256"]["OUTFIT_ACTIONS.jsonl"] != digest(rows_file):
        raise ValueError("Projection differs from verified equipment costs")
    if packet["process"] != observed["process"]:
        raise ValueError(
            "Observed packet and resource inputs belong to different roles"
        )
    parameters = water_actions(root)
    actions = {}
    for action, parameter in parameters.items():
        windows = resource["actions"][str(action)]
        fields = packet["actions"][str(action)]
        selected = [r for r in windows["direct_rows"] if r["recipient_0"]]
        indices = [r["order"] for r in selected]
        ticks = [r["tick"] for r in selected]
        if (
            windows["parameter_sha256"] != digest(parameter)
            or indices != fields["recipient_0_indices"]
            or len(windows["direct_rows"]) != fields["bank_rows"]
            or not fields["skill_write_predicate"]
            or fields["skill_key_accessor"] != int(fields["actor_skill_link"], 16)
            or ticks != sorted(set(ticks))
            or windows["file_declared_projectile_keys"]
            or any(r["attached_row_definition_events"] for r in windows["direct_rows"])
        ):
            raise ValueError(f"Inconsistent direct row schedule for {action}")
        actions[action] = {
            "parameter_sha256": digest(parameter),
            "skill_key": fields["skill_key_accessor"],
            "base_ki": struct.unpack_from("<h", parameter, 0x16)[0],
            "rows": selected,
        }
    routes = {}
    owned = []
    for line in rows_file.splitlines():
        row = json.loads(line)
        if row["unknown"] is not None:
            raise ValueError(f"Unknown cost for candidate {row['ordinal']}")
        path = tuple(row["actions"])
        if path not in routes:
            windows = [
                [index, action, hit["order"]]
                for index, action in enumerate(path)
                for hit in actions[action]["rows"]
            ]
            routes[path] = {
                "id": len(routes),
                "actions": path,
                "windows": windows,
                "outfits": 0,
            }
        route = routes[path]
        route["outfits"] += 1
        owned.append([row["ordinal"], route["id"]])
    if len(owned) != cost["covered"] or len({n for n, _ in owned}) != len(owned):
        raise ValueError("Projected schedules differ from certified candidate coverage")
    observed_count = sum(len(actions[action]["rows"]) for action in observed["actions"])
    if observed_count != resource["declared_direct_windows_for_recipient_zero"]:
        raise ValueError("Observed route disagrees with direct row census")
    return {
        "schema": "pc202-owned-water-sword-contact-schedules/v1",
        "target": "Full hits on the benchmark training warrior, recipient component 0",
        "scope": "Ordered eligible windows. Per-contact damage is not evaluated.",
        "outfit_actions_sha256": digest(rows_file),
        "raw_resource_sha256": resource["source_action_resource_sha256"],
        "recipient_predicate_sha256": resource["recipient_branch_sha256"],
        "actions": actions,
        "routes": list(routes.values()),
        "ordinal_to_route": owned,
        "observed_full_hit_windows": observed_count,
        "actual_landed_contacts": resource["actual_landed_contacts_in_empty_area"],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("F:/Nioh3_r16_build"))
    parser.add_argument("--session", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    schedule = compile_schedules(args.root, args.session)
    with args.output.open("x", encoding="utf8") as out:
        json.dump(schedule, out, indent=2)
        out.write("\n")
    print({len(r["windows"]): r["outfits"] for r in schedule["routes"]})
