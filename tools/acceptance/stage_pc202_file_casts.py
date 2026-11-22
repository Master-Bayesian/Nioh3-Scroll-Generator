"""Stage resolved PC202 martial casts from exact installed action and damage rows."""

import argparse
import hashlib
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_witnessed_cast import (  # noqa: E402
    bind_actor_contacts,
    controller,
    digest,
    emit,
    read,
    require,
)
from verify_loadout_release import (  # noqa: E402
    POOLED_FILES,
    public_frame,
    seal,
    verify_scene_directory,
)


BASE = {"ninja": "kusarigama-strong", "samurai": "samurai-odachi-mid-quick"}
STANCE_WORD = {"none": None, "high": 0, "mid": 1, "low": 2}
IDENTITY = ("resource", "action", "parameter_sha256")


def resolve_cast(cast, route, items, selections):
    identity = {key: cast[key] for key in IDENTITY}
    (record,) = [
        item
        for item in items
        if item["namespace"] == identity["resource"]
        and all(item[key] == identity[key] for key in IDENTITY[1:])
    ]
    (selection,) = [
        item
        for item in selections
        if all(item[key] == identity[key] for key in IDENTITY)
    ]
    require(
        selection["school"] == route["school"] and selection["kind"] == "martial",
        "The native resource does not match the martial selector",
    )
    rows = controller(*(identity[key] for key in IDENTITY))
    active = [
        row
        for row in record["direct_rows"]
        if row["start"] >= 0 and (row["physical_motion"] or row["elemental_motion"])
    ]
    require(
        [item["row"] for item in active]
        == [contact["row"] for contact in cast["contacts"]],
        "The certified cast must account for each positive direct row in order",
    )
    if cast["basis"] == "isolated_resource_windows":
        require(
            cast["evidence"]["incoming_controller_references"]
            and not record["base_events"]
            and not record["extension_events"]
            and len(active) == len(record["direct_rows"]),
            "An isolated direct cast needs a controller entry and no extra events or zero-motion contacts",
        )
    else:
        require(cast["basis"] == "observed_complete_cast", "Unknown landing basis")

    contacts = []
    end_tick = -1
    for declared, contact in zip(active, cast["contacts"], strict=True):
        raw = rows[declared["row"]]
        require(
            digest(raw) == contact["row_sha256"] == declared["sha256"]
            and raw[0x94] == contact["row_kind"]
            and declared["repeat_interval"] == 0
            and declared["start"] > end_tick
            and raw[0x20] == 0xFF,
            f"Certified contact differs from the installed ordinary actor row: {cast['id']}",
        )
        end_tick = declared["end"]
        contacts.append(dict(contact, skill_key=record["skill_id"]))

    return identity, selection, rows, contacts


def stage_cast(args, cast, route, decoded, temporary, proof_digest):
    identity, selection, rows, contacts = decoded
    school, name = route["school"], cast["id"]
    base = args.data / "loadout" / BASE[school]
    verified = verify_scene_directory(base)
    original = read(base / "runtime-contract.json")
    generated = args.samurai_main if school == "samurai" else args.ninja_main
    require(
        original["kernel_layout"]["generated_main_sha256"]
        == digest(generated.read_bytes()),
        "Generated MAIN does not match the replay layout",
    )
    public = args.output / "loadout" / name
    public.mkdir(parents=True)
    scene = temporary / name
    scene.mkdir()
    for filename, item in verified["files"].items():
        source = base / filename
        if not source.is_file():
            source = args.data / "loadout-kernels" / item["sha256"] / filename
        if filename in POOLED_FILES:
            shared = args.output / "loadout-kernels" / item["sha256"] / filename
            if not shared.is_file():
                shared.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, shared)
        shutil.copyfile(source, scene / filename)

    plan = read(scene / "event_plan.json")
    require(
        plan["mode"] == {"kind": "fixed_contacts"}
        and plan["single_use"] is None
        and not plan.get("triggered_damage")
        and plan["initial_phase"] == "inactive",
        "A file cast needs an ordinary direct baseline",
    )
    (base_contact,) = plan["provenance"]["row_bindings"]
    plan.update(
        actor_damage_kind_from_row=True,
        direct_owner_element=any(
            rows[contact["row"]][4] & 0xC0 == 0 for contact in contacts
        ),
    )
    plan["source"]["incarnation"] = (
        int.from_bytes(
            hashlib.sha256((proof_digest + name).encode()).digest()[:8], "little"
        )
        or 1
    )
    hits = bind_actor_contacts(
        plan,
        name,
        identity,
        rows,
        contacts,
        dict(
            kind=cast["basis"],
            landing_basis=cast["landing_basis"],
            cast_proof_sha256=proof_digest,
            native_evidence=cast["evidence"],
        ),
        base_contact["aad_address"],
        STANCE_WORD[cast["stance"]],
    )
    descriptor = dict(
        verified["descriptor"],
        id=name,
        name=selection["name"],
        weapon=route["weapon"],
        weapon_class=route["weapon_class"],
        selection_weapon_class=route["weapon_class"],
        damage=cast["damage_label"],
        scene=dict(
            stance=cast["stance"], kind="martial", attack_id=name, buff="current"
        ),
        resource_action=dict(
            identity, row=contacts[0]["row"], row_sha256=contacts[0]["row_sha256"]
        ),
    )
    emit(scene / "bundle.json", descriptor)
    emit(scene / "event_plan.json", plan)
    (scene / "hits.txt").write_bytes(hits)
    frame = scene / "frame.bin"
    frame.write_bytes(public_frame(frame.read_bytes(), plan))
    seal(scene, generated, args.app_commit, original["native_source_commit"])
    for file in scene.iterdir():
        if file.name not in POOLED_FILES:
            shutil.copyfile(file, public / file.name)
    result = verify_scene_directory(public, args.app_commit)
    return dict(
        scene=name,
        school=school,
        contacts=len(contacts),
        contract_sha256=result["contract_sha256"],
    )


def stage(args):
    proof = read(args.casts)
    require(proof["schema"] == "nioh3-pc202-file-cast-proof/v1", "Unknown cast proof")
    inventory_digest = digest(args.inventory.read_bytes())
    require(
        proof["inventory_sha256"] == inventory_digest, "Wrong installed event inventory"
    )
    items = read(args.inventory)
    selections = read(args.catalog)["selections"]
    routes = {item["action"]["resource"]: item for item in read(args.routes)["routes"]}
    cast_inputs = [
        (
            cast,
            routes[cast["resource"]],
            resolve_cast(cast, routes[cast["resource"]], items, selections),
        )
        for cast in proof["casts"]
    ]
    args.output.mkdir(parents=True)
    for entry in args.data.iterdir():
        if entry.name in ("loadout", "loadout-kernels"):
            continue
        if entry.is_file():
            shutil.copyfile(entry, args.output / entry.name)
        elif entry.is_dir():
            shutil.copytree(entry, args.output / entry.name)
    proof_digest = digest(args.casts.read_bytes())
    with tempfile.TemporaryDirectory(dir=args.output.parent) as name:
        scenes = [
            stage_cast(args, cast, route, decoded, Path(name), proof_digest)
            for cast, route, decoded in cast_inputs
        ]
    return dict(
        cast_proof_sha256=proof_digest, inventory_sha256=inventory_digest, scenes=scenes
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in (
        "casts",
        "inventory",
        "catalog",
        "routes",
        "data",
        "samurai-main",
        "ninja-main",
        "output",
        "receipt",
    ):
        parser.add_argument("--" + key, required=True, type=Path)
    parser.add_argument("--app-commit", required=True)
    args = parser.parse_args()
    emit(args.receipt, stage(args))
