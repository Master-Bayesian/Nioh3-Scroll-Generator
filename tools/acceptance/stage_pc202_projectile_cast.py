"""Close a fixed primary projectile objective against its installed spawn graph."""

import argparse
import struct
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_witnessed_cast import digest, emit, read, require  # noqa: E402
from verify_loadout_release import copy_verified_scene, seal, verify_scene_directory  # noqa: E402


# PC202 factory methods: terrain/pose (5), attached effects (7), model
# motion (16), and model mask selection (21). Type 3 has two spawn keys.
# Other kinds, including independently damaging type 33, need their own rules.
NON_DAMAGE_MODULES = {5: "terrain_pose", 7: "attached_effect", 16: "model_motion", 21: "model_mask"}
LIFECYCLE_REASONS = ("expiry", "contact", "alternate_environment", "environment")


def spawn_record(spawns, key):
    slot = key - 20000
    require(0 <= slot < len(spawns), f"Missing same-resource spawn {key}")
    raw = spawns[slot]
    require(any(raw[:4]), f"Missing constructor identity for spawn {key}")
    return raw


def terminal_graph(spawns, modules, key):
    """Prove that any descendant of the declared primary has no damage packet."""
    root = key
    pending = [root]
    records = {}
    while pending:
        key = pending.pop()
        require(key not in records, f"Repeated or cyclic descendant spawn {key}")
        raw = spawn_record(spawns, key)
        action = struct.unpack_from("<H", raw, 0x12)[0]
        selector = struct.unpack_from("<b", raw, 0x15)[0]
        global_index = struct.unpack_from("<H", raw, 0x128)[0]
        has_row = (action != 0xFFFF and selector >= 0) or global_index < 0x335
        require(key == root or not has_row, f"Descendant {key} selects a damage row")
        keys = struct.unpack_from("<4H", raw, 0x100)
        children = {reason: child for reason, child in zip(LIFECYCLE_REASONS, keys, strict=True) if child}
        bindings = []
        for slot, index in enumerate(struct.unpack_from("<10H", raw, 0x110)):
            if index == 0xFFFF:
                continue
            require(index < len(modules), f"Missing spawn module {index} for {key}")
            module = modules[index]
            kind = struct.unpack_from("<i", module, 0x4C)[0]
            if kind == 3:
                effects, projectile = struct.unpack_from("<ii", module)
                if projectile > 0:
                    children[f"module_{slot}"] = projectile
                role = "spawn_keys"
            else:
                require(kind in NON_DAMAGE_MODULES, f"Unclassified damage behavior for module {kind}")
                effects = None
                role = NON_DAMAGE_MODULES[kind]
            bindings.append(dict(slot=slot, index=index, kind=kind, role=role,
                                 effects_key=effects, sha256=digest(module)))
        records[key] = dict(
            sha256=digest(raw), action=action, row=selector,
            global_selector=global_index if global_index < 0x335 else None,
            has_primary=has_row, children=children, modules=bindings,
        )
        pending.extend(children.values())
    return records


def installed_records(args):
    sys.path.insert(0, str(args.resource_reader))
    from action_resource import ActionResource

    entry = read(args.manifest)["resources"][args.resource][0]
    raw = (args.manifest.parent / "matched-resources" / Path(entry["path"]).name).read_bytes()
    require(digest(raw) == entry["sha256"], "Installed action resource differs")
    resource = ActionResource.load(args.resource, raw)
    return entry["sha256"], resource.records(7, 336), resource.records(8, 80)


def stage(args, receipt):
    base = args.primary_scene
    checked = verify_scene_directory(base)
    contract = read(base / "runtime-contract.json")
    descriptor = checked["descriptor"]
    plan = read(base / "event_plan.json")
    (contact,) = plan["templates"]
    bound = plan["provenance"]["scheduled_spawn"]
    require(
        bound["resource"] == args.resource and bound["event_key"] == args.spawn
        and bound["spawn_sha256"] == receipt["graph"][args.spawn]["sha256"]
        and receipt["graph"][args.spawn]["has_primary"]
        and plan["mode"] == {"kind": "fixed_contacts"}
        and contact["packet"]["kind"] == "owner_weapon_projectile"
        and contact["landed"] is True and contact["guard"] is True and contact["scored"] is True,
        "The cast needs the certified fixed unguarded primary contact for this resource",
    )
    name = args.output.name
    descriptor.update(id=name, damage=args.damage_label)
    descriptor["scene"]["attack_id"] = name
    contact["id"] = name
    plan["source"]["incarnation"] = int(digest(name.encode())[:16], 16) or 1
    plan["provenance"].update(
        landing_basis="One unguarded primary contact on the fixed benchmark opponent.",
        scope="Full damage of the declared cast. Optional environmental descendants have no damage packet.",
        projectile_closure=receipt,
    )
    require(digest(args.generated_main.read_bytes()) == contract["kernel_layout"]["generated_main_sha256"],
            "Generated MAIN differs from the certified primary")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=args.output.parent) as staging:
        scene = Path(staging) / name
        copy_verified_scene(base, scene, checked)
        emit(scene / "bundle.json", descriptor)
        emit(scene / "event_plan.json", plan)
        seal(scene, args.generated_main, args.app_commit, contract["native_source_commit"])
        scene.rename(args.output)
    return verify_scene_directory(args.output, args.app_commit)["contract_sha256"]


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    for option in ("manifest", "resource-reader", "receipt"):
        cli.add_argument("--" + option, required=True, type=Path)
    cli.add_argument("--resource", required=True)
    cli.add_argument("--spawn", required=True, type=int)
    for option in ("primary-scene", "generated-main", "output"):
        cli.add_argument("--" + option, type=Path)
    cli.add_argument("--app-commit")
    cli.add_argument("--damage-label")
    args = cli.parse_args()
    source_sha, spawns, modules = installed_records(args)
    receipt = dict(
        schema="nioh3-pc202-projectile-damage-closure/v1", resource=args.resource,
        source_sha256=source_sha, primary=args.spawn,
        graph=terminal_graph(spawns, modules, args.spawn),
        methods=["65E908", "5C9BC4", "5B50F0", "5B5280", "49D610",
                 "28E2330", "9A67DC", "28E27F8", "28E1998", "28E3100"],
    )
    if args.primary_scene is not None:
        require(all((args.generated_main, args.app_commit, args.damage_label, args.output)),
                "Staging requires generated MAIN, app commit, label and output")
        receipt = dict(receipt, scene=args.output.name, contract_sha256=stage(args, receipt))
    emit(args.receipt, receipt)


if __name__ == "__main__":
    main()
