"""Compile a witnessed PC202 direct-actor cast for a fixed landed-contact scenario."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import tempfile

from verify_loadout_release import public_frame, seal, verify_scene_directory


ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / "crates/nioh3-loadout/src"
ROWS_BASE = 0x600000100000  # ControllerInput::install's private row arena.


def read(path):
    return json.loads(path.read_bytes())


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(test, message):
    if not test:
        raise ValueError(message)


def emit(path, document):
    path.write_bytes(
        (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode()
    )


def resolve_controller(resource, action, parameter_hash):
    states = (NATIVE / "pc202_action_states.bin").read_bytes()
    bank = (NATIVE / "pc202_damage_rows.bin").read_bytes()
    magic, records, params, refs = struct.unpack_from("<8sIII", states)
    tag, keys, rows = struct.unpack_from("<8sII", bank)
    start = 20 + 44 * records
    indices = start + 336 * params
    rowstart = 16 + keys * 36
    require(
        magic == b"N3ACT001" and indices + 4 * refs == len(states),
        "Invalid controller bank",
    )
    require(
        tag == b"N3AAD001" and rowstart + 192 * rows == len(bank), "Invalid damage bank"
    )
    prefix = resource.encode() + b"\0" + struct.pack("<H", action)
    key = hashlib.sha256(prefix + bytes.fromhex(parameter_hash)).digest()
    (match,) = [
        r for r in struct.iter_unpack("<32sIIHH", states[20:start]) if r[0] == key
    ]
    _, parameter, first, count, _ = match
    require(
        parameter < params and first + count <= refs, "Invalid controller references"
    )
    raw_parameter = states[start + 336 * parameter : start + 336 * (parameter + 1)]
    require(digest(raw_parameter) == parameter_hash, "Controller parameter differs")
    slots = struct.unpack_from(f"<{count}I", states, indices + 4 * first)
    require(slots and max(slots) < rows, "Missing controller damage rows")
    return raw_parameter, [bank[rowstart + 192 * slot : rowstart + 192 * (slot + 1)] for slot in slots]


def controller(resource, action, parameter_hash):
    return resolve_controller(resource, action, parameter_hash)[1]


def body(region):
    data = bytes.fromhex(region["bytes_hex"])
    require(
        len(data) == region["requested_bytes"]
        and bytes.fromhex(region["known_mask_hex"]) == b"\xff" * len(data)
        and digest(data) == region["sha256"],
        f"Incomplete bytes for {region['role']}",
    )
    return data


def extract(witness, generation, action, parameter_hash, native_rows, recipe):
    require(
        witness["mode"] == "live"
        and witness["actual_live_evidence"]
        and not witness["identity"]["mock_only"]
        and witness["identity"]["loaded_text_sha256"]
        == "4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29",
        "Expected original PC202 capture",
    )
    raw = witness["raw_capture"]
    frames = [f for f in raw["frames"] if f.get("action_generation") == generation]
    require(
        frames and not raw["errors"] and not raw["cleanup_errors"], "Incomplete capture"
    )
    require(
        len({(f["source"], f["target"], f["thread_id"]) for f in frames}) == 1,
        "Cast changes actors or thread",
    )
    require(
        [f["id"] for f in frames]
        == list(range(frames[0]["id"], frames[0]["id"] + len(frames))),
        "Interrupted cast",
    )
    shots = {s["sequence"]: s for s in raw["snapshots"]}

    def regions(seq, kind):
        shot = shots[seq]
        require(shot["kind"] == kind, "Incorrect snapshot sequence")
        return {r["role"]: r for r in shot["regions"]}

    contacts = []
    for f in frames:
        require(
            f["action_id"] == action
            and f["complete"]
            and f["source_is_player"]
            and f["source"] == f["player"]
            and f["normal_tag0_binding"]
            and f["spawn_association"] == "actor_source"
            and f["lifecycle"] == "hp_applied"
            and not f["prior_rebuild_candidates"],
            "Unsupported contact or rebuild",
        )
        entry = regions(f["entry_sequence"], "MAIN_entry_prestate")
        packet = body(entry["MAIN.entry_prestate"])
        require(
            entry["MAIN.entry_prestate"]["address_hex"] == f["hit"]
            and digest(body(entry["MAIN.selected_action_parameter"])) == parameter_hash
            and digest(body(entry["MAIN.layers.active0.parameter"])) == parameter_hash,
            "Active parameter changed",
        )
        active = f["action_layers"][0]
        require(
            active["slot"] == 0
            and active["kind"] == "active"
            and active["action_id"] == action
            and active["damage_row_count"] == len(native_rows),
            "Active controller has different rows",
        )
        row_pointer = struct.unpack_from("<Q", packet, 0x190)[0]
        matches = []
        for i, expected in enumerate(native_rows):
            region = entry[f"MAIN.layers.active0.damage{i}"]
            require(
                body(region) == expected,
                "Live layer differs from the catalog-qualified controller",
            )
            if int(region["address_hex"], 16) == row_pointer:
                matches.append(i)
        (index,) = matches
        require(
            int(entry["MAIN.damage_row"]["address_hex"], 16) == row_pointer
            and body(entry["MAIN.damage_row"]) == native_rows[index],
            "Packet points to another row",
        )
        # Weapon-owned fields are projected for each candidate. Other fixed
        # packet operands must match this ABI unless explicitly compiled here.
        for field in recipe["fields"]:
            if (
                field["root"] == "packet"
                and len(field["path"]) == 1
                and field["kind"] == "benchmark"
            ):
                (offset,) = field["path"]
                if (offset, field["n"]) not in ((0x136, 2), (0x13C, 4)):
                    require(
                        packet[offset : offset + field["n"]].hex() == field["bits"],
                        f"Unbound fixed packet operand at {offset:#x}",
                    )
        before = regions(f["final_sequence"], "MAIN_final")["target.HP_before_apply"]
        after = regions(f["HP_after_sequence"], "HP_after_store")[
            "actual_HP_after_R11_plus20"
        ]
        hp_at = int(f["target_hp_address"], 16)
        require(
            int(before["address_hex"], 16) + 8
            == hp_at
            == int(after["address_hex"], 16),
            "HP subject differs",
        )
        pre = struct.unpack_from("<Q", body(before), 8)[0]
        post = struct.unpack_from("<Q", body(after))[0]
        require(
            pre > post and (not contacts or contacts[-1]["hp_after"] == pre),
            "Unsettled HP history",
        )
        contacts.append(
            dict(
                frame=f["id"],
                row=index,
                hp_before=pre,
                hp_after=post,
                skill_key=struct.unpack_from("<H", packet, 0x136)[0],
                packet_flags=struct.unpack_from("<I", packet, 0x13C)[0],
            )
        )
    require(
        len({c["skill_key"] for c in contacts}) == 1, "Cast changes its actor skill key"
    )
    return contacts


def controlled_direct_cast(path, identity, rows):
    raw = path.read_bytes()
    proof = json.loads(raw)
    require(proof["schema"] == "pc202-direct-actor-flags/v1", "Unknown packet proof")
    (action,) = [
        r for r in proof["actions"] if all(r.get(k) == v for k, v in identity.items())
    ]
    require(
        proof["main_direct_read"]["bit24"] is False
        and proof["role"]["actor_ab84"] != 0xFFFFFFFF,
        "The normal Ninja actor branch was not checked",
    )
    # This scenario chooses the normal, untriggered branch. Unobserved helper
    # outputs are explicit controls. No bits are taken from another move.
    flags = int(action["static_packet_bits"], 16) | 0x80
    contacts = []
    for item in action["landed_rows"]:
        index = item["row"]
        require(0 <= index < len(rows), "Direct window refers to a missing row")
        row = rows[index]
        (start,) = struct.unpack_from("<h", row, 8)
        require(
            start >= 0
            and any(struct.unpack_from("<hh", row, 0x10))
            and struct.unpack_from("<Q", row)[0] == int(item["row_flags"], 16)
            and row[0x20] == item["row_type"]
            and (not contacts or start > contacts[-1]["tick"]),
            "Damage row does not match the ordered direct windows",
        )
        contacts.append(
            dict(
                row=index, tick=start, skill_key=action["skill_key"], packet_flags=flags
            )
        )
    return contacts, {
        "kind": "controlled_direct_cast",
        "action_flag_evidence_sha256": digest(raw),
        "landing_basis": "One unguarded contact in each named damage window against the benchmark.",
        "conditions": {
            "unobserved_packet_bits": "clear",
            "owner_selected_element": -1,
            "owner_physical_factor": 1.0,
            "owner_elemental_factor": 1.0,
            "current_role_values_observed": False,
        },
    }


def bind_actor_contacts(
    plan, name, identity, rows, contacts, provenance, aad, stance=None
):
    """Use resolved actor contacts with the selected form's native packet layout."""
    require(contacts and rows, "A direct cast needs a damage row and a contact")
    (skill_key,) = {contact["skill_key"] for contact in contacts}
    root_row = contacts[0]["row"]
    offsets = (
        (0x13C, 0x148, 0x190, 0x194) if stance is not None else (0x13C, 0x190, 0x194)
    )
    addresses = [plan["main_address"] + offset for offset in offsets]

    def words(values):
        return " ".join(f"{n:x}" for n in values)

    lines = [str(len(contacts)), words(addresses)]
    plan["templates"] = []
    bindings = []
    for ordinal, contact in enumerate(contacts):
        index = contact["row"]
        require(0 <= index < len(rows), "Contact refers to a missing controller row")
        address = aad if index == root_row else ROWS_BASE + 192 * index
        values = [contact["packet_flags"]]
        if stance is not None:
            values.append(stance)
        values.extend((address & 0xFFFFFFFF, address >> 32))
        lines.append(words(values))
        plan["templates"].append(
            dict(
                id=f"{name}:{ordinal}",
                identity=identity,
                packet={"kind": "prepared_actor"},
                landed=True,
                guard=True,
                scored=True,
            )
        )
        bindings.append(
            dict(
                template=ordinal,
                row_sha256=digest(rows[index]),
                aad_address=address,
                exact_action_matches=[dict(identity, row=index)],
                identity_determined=True,
            )
        )
    hits = ("\n".join(lines) + "\n").encode("ascii")
    plan.update(
        actor_skill_key=skill_key,
        hit_values_sha256=digest(hits),
        template_layout_sha256=digest(
            b"".join(struct.pack("<Q", at) for at in addresses)
        ),
        provenance=dict(
            {k: v for k, v in provenance.items() if k != "landing_basis"},
            row_bindings=bindings,
            landing_basis=provenance["landing_basis"],
        ),
    )
    return hits


def compile_scene(args):
    public = verify_scene_directory(args.base)
    base = public["descriptor"]
    weapon = read(args.weapon_reference)
    require(
        weapon["resource_action"]["resource"] == args.resource
        and weapon["school"] == base["school"] == "ninja"
        and weapon["selection_weapon_class"] == weapon["weapon_class"],
        "Resource is not bound to the requested weapon and direct actor layout",
    )
    identity = dict(
        resource=args.resource, action=args.action, parameter_sha256=args.parameter
    )
    (selected,) = [
        r
        for r in read(args.catalog)["selections"]
        if all(r[k] == v for k, v in identity.items())
    ]
    require(
        selected["school"] == weapon["school"] and selected["kind"] == "martial",
        "Unsupported catalog selection",
    )
    plan = read(args.base / "event_plan.json")
    require(
        plan["mode"] == {"kind": "fixed_contacts"}
        and plan.get("single_use") is None
        and len(plan["templates"]) == 1
        and plan["templates"][0]["packet"] == {"kind": "prepared_actor"}
        and not plan.get("candidate_buff")
        and plan["initial_phase"] == "inactive",
        "Unsupported base event policy",
    )
    recipe = read(
        ROOT / "crates/nioh3-runtime/src/combat_context/kusarigama_strong_pc202.json"
    )
    require(recipe["roots"]["packet"] == plan["main_address"], "Wrong Ninja layout")
    rows = controller(args.resource, args.action, args.parameter)
    if getattr(args, "controlled_packet", None):
        require(
            args.generation is None and not args.witness, "Use one cast input origin"
        )
        contacts, provenance = controlled_direct_cast(
            args.controlled_packet, identity, rows
        )
    else:
        require(
            args.witness and args.generation is not None,
            "A witnessed cast needs its generation",
        )
        raw = args.witness.read_bytes()
        contacts = extract(
            json.loads(raw), args.generation, args.action, args.parameter, rows, recipe
        )
        provenance = dict(
            kind="witnessed_direct_cast",
            generation=args.generation,
            witness_sha256=digest(raw),
            frames=[c["frame"] for c in contacts],
            landing_basis="Fixed landed contacts from one observed controller generation.",
        )
    require(
        [c["row"] for c in contacts] == args.rows,
        "Landed row sequence differs from the declared cast",
    )
    root_row = contacts[0]["row"]
    if getattr(args, "direct_owner_element", False):
        require(
            all(row[0x20] == 0xFF and row[4] & 0xC0 == 0 for row in rows),
            "Live owner element requires compatible ordinary direct rows",
        )
        plan["direct_owner_element"] = True
        provenance["conditions"] = {
            "other_packet_bits": "Retain the recorded per-contact packet flags."
            if args.witness
            else "Use the declared neutral controls for unobserved per-contact flags.",
            "owner_element_source": "Read current owner+2E9C; read owner+2EA4 for an active override.",
            "owner_status_factors": "Use the ordinary encounter reader's current actor operands.",
        }
    hits = bind_actor_contacts(
        plan,
        args.output.name,
        identity,
        rows,
        contacts,
        provenance,
        recipe["roots"]["attack_data"],
    )
    # A private evaluator owner and a deterministic identity for this immutable plan.
    plan["source"]["incarnation"] = (
        int.from_bytes(
            hashlib.sha256(
                (
                    args.output.name
                    + (
                        provenance["witness_sha256"] + str(args.generation)
                        if args.witness
                        else provenance["action_flag_evidence_sha256"]
                    )
                ).encode()
            ).digest()[:8],
            "little",
        )
        or 1
    )
    descriptor = dict(
        base,
        id=args.output.name,
        name=selected["name"],
        weapon=weapon["weapon"],
        selection_weapon_class=weapon["selection_weapon_class"],
        weapon_class=weapon["weapon_class"],
        scene=dict(
            stance="none",
            kind=selected["kind"],
            attack_id=args.output.name,
            buff="current",
        ),
        damage=args.damage_label,
        resource_action=dict(identity, row=root_row, row_sha256=digest(rows[root_row])),
    )
    require(
        base.get("benchmark_target_sha256")
        and base["benchmark_target_sha256"] == weapon.get("benchmark_target_sha256"),
        "Weapon reference and scene do not share the fixed observed opponent",
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.output.parent) as temp:
        scene = Path(temp) / args.output.name
        shutil.copytree(args.base, scene)
        for dll in ("main_cpu.dll", "main_device.dll"):
            require(
                (scene / dll).read_bytes() == (args.kernel / dll).read_bytes(),
                "Kernel differs from sealed base",
            )
        emit(scene / "bundle.json", descriptor)
        emit(scene / "event_plan.json", plan)
        (scene / "hits.txt").write_bytes(hits)
        (scene / "frame.bin").write_bytes(
            public_frame((scene / "frame.bin").read_bytes(), plan)
        )
        contract = read(args.base / "runtime-contract.json")
        require(
            digest((args.kernel / "main_gen.inc").read_bytes())
            == contract["kernel_layout"]["generated_main_sha256"],
            "Kernel source differs from sealed base",
        )
        seal(
            scene,
            args.kernel / "main_gen.inc",
            args.app_commit,
            contract["native_source_commit"],
        )
        final = verify_scene_directory(scene, args.app_commit)
        require(
            final["layout"]["fixed_scored_contacts"] == len(contacts)
            and final["layout"]["empty_actor_buffers_and_seeds"],
            "Invalid public cast layout",
        )
        scene.rename(args.output)
    return dict(
        scene=args.output.name,
        contract_sha256=final["contract_sha256"],
        contacts=contacts,
        historical_hp_losses=(
            [c["hp_before"] - c["hp_after"] for c in contacts] if args.witness else None
        ),
        input_origin=provenance["kind"],
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("base", "weapon-reference", "kernel", "catalog", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--witness", type=Path)
    source.add_argument(
        "--controlled-packet",
        type=Path,
        help="Native direct-actor flags proof for a controlled development scene",
    )
    parser.add_argument(
        "--direct-owner-element",
        action="store_true",
        help="Read and apply the actor's element override for each eligible row",
    )
    for name in ("resource", "parameter", "app-commit", "damage-label"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--action", type=int, required=True)
    parser.add_argument("--generation", type=int)
    parser.add_argument(
        "--rows",
        nargs="+",
        type=int,
        required=True,
        help="Expected ordered landed row selectors",
    )
    print(json.dumps(compile_scene(parser.parse_args()), indent=2))
