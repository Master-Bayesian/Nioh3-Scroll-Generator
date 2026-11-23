"""Join PC202 file-declared projectile references to the App's damage rows.

The output describes selectable rows. A spawned object's hit and its owner's
resource selection are separate runtime facts. Flying Circle supplies two
independently recorded examples of both in this audit.
"""

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import struct
import sys


def read(path):
    return json.loads(path.read_bytes())


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def resolve(resources, reader, bank_file, global_binding):
    sys.path.insert(0, str(reader))
    from action_extensions import action_file_records
    from action_resource import ActionResource
    from move_events import scheduled_references
    from projectile_row_selection import (
        PrimaryRowKind,
        record_global_key,
        spawn_row_intent,
    )

    bank = bank_file.read_bytes()
    tag, key_count, row_count = struct.unpack_from("<8sII", bank)
    body_start = 16 + key_count * 36
    if tag != b"N3AAD001" or len(bank) != body_start + row_count * 192:
        raise ValueError("Invalid App damage-row bank")
    keys = dict(struct.iter_unpack("<32sI", bank[16:body_start]))
    global_info = read(global_binding)["binding"]
    global_rows = (global_binding.parent / Path(global_info["path"]).name).read_bytes()
    if (
        len(global_rows) != global_info["rows"] * 192
        or digest(global_rows) != global_info["sha256"]
    ):
        raise ValueError(
            "Global damage-row bank differs from the installed resource binding"
        )
    manifests = read(resources / "WEAPON_RESOURCE_MANIFEST.json")["resources"]
    entries = [(key, variants[0]) for key, variants in manifests.items()]
    for entry in read(resources / "COMMON_RESOURCE_BINDING.json"):
        entries.append((f"common:{entry['category']}:{entry['sha256']}", entry))

    bindings, counts = [], Counter()
    for space, entry in entries:
        raw = (resources / "matched-resources" / Path(entry["path"]).name).read_bytes()
        if digest(raw) != entry["sha256"]:
            raise ValueError(f"Resource identity changed: {space}")
        resource = ActionResource.load(space, raw)
        records = list(
            zip(resource.records(1, 336), action_file_records(resource), strict=True)
        )
        targets = defaultdict(list)
        for parameter, record in records:
            action = struct.unpack_from("<H", parameter, 12)[0]
            targets[action].append((parameter, record.damage_indices))
        rows = resource.records(2, 192)
        spawns = resource.records(7, 336)
        for parameter, record in records:
            owner = struct.unpack_from("<H", parameter, 12)[0]
            base = scheduled_references(
                {"raw_parameter_hex": parameter.hex(), "resource_key": space}
            )
            events = [(e.slot, e.command, e.tick) for e in base]
            for slot, ext in enumerate(record.extension_rows, 16):
                key, tick = struct.unpack_from("<HH", ext)
                events.append((slot, key, tick))
            for slot, key, tick in events:
                if not 20000 <= key < 0x8000:
                    continue
                spawn = spawns[key - 20000]
                imbue_selector, imbue_flags = struct.unpack_from("<iI", spawn, 0xC0)
                intent = spawn_row_intent(spawn, global_info["rows"])
                occurrence = dict(
                    resource=space,
                    owner_action=owner,
                    parameter_sha256=digest(parameter),
                    owner_direct_rows=len(record.damage_indices),
                    slot=slot,
                    tick=tick,
                    event_key=key,
                    carrier=struct.unpack_from("<I", spawn)[0],
                    descriptor_fields=list(struct.unpack_from("<3I", spawn, 0x90)),
                    requested_action=struct.unpack_from("<H", spawn, 0x12)[0],
                    requested_row=struct.unpack_from("<b", spawn, 0x15)[0],
                    record_global_key=record_global_key(spawn),
                    imbue_selector=imbue_selector,
                    imbue_flags=imbue_flags,
                    checks_owner_imbue=imbue_selector != -1
                    or bool(imbue_flags & 0x40000),
                    spawn_sha256=digest(spawn),
                    primary_row=intent.kind.value,
                )
                if intent.kind == PrimaryRowKind.GLOBAL_ROW:
                    start = 192 * intent.global_index
                    occurrence.update(
                        global_index=intent.global_index,
                        row_sha256=digest(global_rows[start : start + 192]),
                    )
                if intent.kind != PrimaryRowKind.ACTION_REQUEST:
                    counts[intent.kind.value] += 1
                    bindings.append(occurrence)
                    continue
                matches = targets[intent.action]
                if len(matches) != 1 or intent.row >= len(matches[0][1]):
                    counts["requires_other_owner_lookup"] += 1
                    occurrence.update(
                        target_action=intent.action,
                        row=intent.row,
                        requires_owner_lookup=True,
                    )
                    bindings.append(occurrence)
                    continue
                target_parameter, target_rows = matches[0]
                target_digest = hashlib.sha256(target_parameter).digest()
                prefix = (
                    space.encode()
                    + b"\0"
                    + struct.pack("<HI", intent.action, intent.row)
                )
                row_key = hashlib.sha256(prefix + target_digest).digest()
                bank_slot = keys[row_key]
                start = body_start + bank_slot * 192
                native = rows[target_rows[intent.row]]
                if bank_slot >= row_count or bank[start : start + 192] != native:
                    raise ValueError(
                        f"Original and App damage row disagree: {space}/{owner}/{key}"
                    )
                counts["extension_row" if slot >= 16 else "base_row"] += 1
                occurrence.update(
                    target_action=intent.action,
                    target_parameter_sha256=target_digest.hex(),
                    row=intent.row,
                    row_key=row_key.hex(),
                    row_sha256=digest(native),
                )
                bindings.append(occurrence)
    return bindings, dict(counts), global_rows


def compare_flying_circle(bindings, observation):
    births = {s["id"]: s for s in observation["spawn_records"]}
    results = []
    for hit in observation["observations"]:
        birth = births[hit["spawn_id"]]
        if not (
            hit["complete"]
            and birth["complete"]
            and hit["source_kind"] == "owned_projectile"
            and hit["spawn_association"] == "observed_birth"
            and birth["source"] == hit["source"]
            and birth["owner_action"] == hit["action_id"]
            and birth["outcome"] == "returned_source"
        ):
            raise ValueError("Missing projectile birth or owner binding")
        # Predict from the file for this owner before comparing any observed
        # spawn key or row. Both charge branches have one explicit primary row.
        (row,) = [
            b
            for b in bindings
            if b["resource"] == "0x6000000fd"
            and b["owner_action"] == hit["action_id"]
            and "row_key" in b
        ]
        expected = (row["event_key"], row["target_action"], row["row"])
        observed = (birth["event_key"], birth["source_action"], birth["source_row"])
        damage = hit["hp_before"] - hit["hp_after"]
        if (
            expected != observed
            or damage != hit["main_damage"]
            or damage != hit["hp_loss"]
        ):
            raise ValueError(
                f"Projectile binding or observed damage differs: {row['owner_action']}"
            )
        results.append(dict(**row, observed_hp_loss=damage))
    if [r["owner_action"] for r in results] != [25203, 25204]:
        raise ValueError("Expected both recorded Flying Circle charge branches")
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in (
        "resources",
        "native_reader",
        "bank",
        "global_binding",
        "observation",
        "output",
    ):
        parser.add_argument(name, type=Path)
    parser.add_argument(
        "--action-bank",
        type=Path,
        help="Write the resource-scoped spawn occurrence index and PC202 global damage table",
    )
    parser.add_argument(
        "--catalog",
        type=Path,
        help="Report row-selection coverage by form and weapon type",
    )
    args = parser.parse_args()
    bindings, counts, global_rows = resolve(
        args.resources, args.native_reader, args.bank, args.global_binding
    )
    examples = compare_flying_circle(bindings, read(args.observation))
    if args.action_bank:
        entries = {}
        for b in bindings:
            owner = (
                b["resource"].encode() + b"\0" + struct.pack("<H", b["owner_action"])
            )
            key = (
                hashlib.sha256(owner + bytes.fromhex(b["parameter_sha256"])).digest(),
                b["slot"],
            )
            if key in entries:
                raise ValueError("Duplicate scheduled projectile occurrence")
            entries[key] = struct.pack(
                "<HHHIHbH32siI3I",
                b["slot"],
                b["tick"],
                b["event_key"],
                b["carrier"],
                b["requested_action"],
                b["requested_row"],
                b["record_global_key"] & 0xFFFF,
                bytes.fromhex(b["row_key"]) if "row_key" in b else bytes(32),
                b["imbue_selector"],
                b["imbue_flags"],
                *b["descriptor_fields"],
            )
        args.action_bank.write_bytes(
            struct.pack("<8sII", b"N3PRJ003", len(entries), len(global_rows) // 192)
            + b"".join(k[0] + entries[k] for k in sorted(entries))
            + global_rows
        )
    explicit_owners = {
        (r["resource"], r["owner_action"], r["parameter_sha256"]): r[
            "owner_direct_rows"
        ]
        for r in bindings
        if "row_key" in r
    }
    summary = dict(
        scope="File-declared projectile references and their primary rows on PC202. Only the two Flying Circle branches are compared to observed hits.",
        counts=counts,
        owner_imbue_lookup_by_primary={
            kind: dict(
                Counter(
                    "checks_owner" if b["checks_owner_imbue"] else "descriptor_branch"
                    for b in bindings
                    if b["primary_row"] == kind
                )
            )
            for kind in sorted({b["primary_row"] for b in bindings})
        },
        selected_global_rows=dict(
            sorted(
                Counter(
                    b["global_index"] for b in bindings if "global_index" in b
                ).items()
            )
        ),
        matched_explicit_owner_actions=len(explicit_owners),
        explicit_owners_without_direct_rows=sum(
            count == 0 for count in explicit_owners.values()
        ),
        bank_sha256=digest(args.bank.read_bytes()),
        weapon_manifest_sha256=digest(
            (args.resources / "WEAPON_RESOURCE_MANIFEST.json").read_bytes()
        ),
        common_manifest_sha256=digest(
            (args.resources / "COMMON_RESOURCE_BINDING.json").read_bytes()
        ),
        global_binding_sha256=digest(args.global_binding.read_bytes()),
        observation_sha256=digest(args.observation.read_bytes()),
        flying_circle=examples,
    )
    if args.catalog:
        kinds = {
            (r["resource"], r["action"], r["parameter_sha256"]): r.get("weapon")
            or "common"
            for r in read(args.catalog)["selections"]
        }
        by_weapon = defaultdict(Counter)
        for b in bindings:
            weapon = kinds[b["resource"], b["owner_action"], b["parameter_sha256"]]
            by_weapon[weapon][b["primary_row"]] += 1
        summary["row_selection_by_weapon"] = dict(sorted(by_weapon.items()))
    args.output.write_text(
        json.dumps(dict(summary=summary, bindings=bindings), indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
