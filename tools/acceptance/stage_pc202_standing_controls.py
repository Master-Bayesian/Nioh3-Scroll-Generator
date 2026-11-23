"""Build engineering first-contact controls for resolved PC202 standing attacks."""

import argparse
import json
from pathlib import Path
import shutil
import struct
import sys
import tempfile


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from compile_witnessed_cast import controller, digest, emit, read, require  # noqa: E402
from verify_loadout_release import POOLED_FILES, seal, verify_scene_directory  # noqa: E402


BASE_SCENES = {
    # These templates include the complete held-item layout used by every
    # Samurai class. Narrow katana templates omit fields used by some items.
    ("samurai", "quick"): "samurai-odachi-mid-quick",
    ("samurai", "strong"): "samurai-odachi-mid-strong",
    ("ninja", "quick"): "kusarigama-strong",
    ("ninja", "strong"): "kusarigama-strong",
}
STANCE_WORD = {"high": 0, "mid": 1, "low": 2}


def hit_words(plan, raw, school, kind, stance, action_flags):
    count, address_line, value_line = raw.decode("ascii").splitlines()
    require(count == "1", "Expected one actor contact")
    addresses = [int(value, 16) for value in address_line.split()]
    values = dict(
        zip(addresses, (int(value, 16) for value in value_line.split()), strict=True)
    )
    main = plan["main_address"]
    flags_at = main + 0x13C
    base_flags = 0x06000081 if school == "samurai" and kind == "quick" else 0x06000082
    require(
        values.get(flags_at, base_flags) == base_flags, "Standing packet flags differ"
    )
    # Preserve the admitted neutral actor bits and install the action-defined bits.
    values[flags_at] = (base_flags & ~0x0400007F) | action_flags
    if school == "samurai":
        require(main + 0x148 in values, "Samurai template must bind its stance word")
        values[main + 0x148] = STANCE_WORD[stance]
    plan["template_layout_sha256"] = digest(
        b"".join(struct.pack("<Q", address) for address in values)
    )
    addresses = " ".join(f"{address:x}" for address in values)
    data = " ".join(f"{value:x}" for value in values.values())
    return f"1\n{addresses}\n{data}\n".encode("ascii")


def stage_control(route, data, output, temporary, generated, app_commit, route_sha):
    school, kind, stance = (route[key] for key in ("school", "attack", "stance"))
    weapon_class, weapon = route["weapon_class"], route["weapon"]
    name = f"{school}-type-{weapon_class}-{stance}-{kind}"
    (row,) = route["positive_direct_rows"]
    identity = route["action"]
    rows = controller(
        identity["resource"], identity["action"], identity["parameter_sha256"]
    )
    require(
        row["row"] == 0 and row["repeat_interval"] == 0, "Expected a fixed root contact"
    )
    bound = rows[0]
    require(
        digest(bound) == row["sha256"] and bound[0x20] == 0xFF and bound[4] & 0xC0 == 0,
        "Damage row differs from the catalog or ordinary owner element layout",
    )

    base = data / "loadout" / BASE_SCENES[school, kind]
    report = verify_scene_directory(base)
    contract = read(base / "runtime-contract.json")
    require(
        contract["kernel_layout"]["generated_main_sha256"]
        == digest(generated.read_bytes()),
        "Generated MAIN differs from the verified base scene",
    )
    public = output / "loadout" / name
    public.mkdir(parents=True)
    scene = temporary / name
    scene.mkdir()
    for filename, item in report["files"].items():
        source = base / filename
        if not source.is_file():
            source = data / "loadout-kernels" / item["sha256"] / filename
        if filename in POOLED_FILES:
            shared = output / "loadout-kernels" / item["sha256"] / filename
            if not shared.is_file():
                shared.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, shared)
        shutil.copyfile(source, scene / filename)

    descriptor = read(scene / "bundle.json")
    label = "速攻击" if kind == "quick" else "力攻击"
    stage = {"none": "", "high": "上段", "mid": "中段", "low": "下段"}[stance]
    descriptor.update(
        id=name,
        name=f"{weapon} · {stage}{label}",
        weapon=weapon,
        weapon_class=weapon_class,
        selection_weapon_class=weapon_class,
        scene=dict(stance=stance, kind=kind, attack_id=name, buff="current"),
        damage=f"{label}（首击）",
        resource_action=dict(identity, row=0, row_sha256=row["sha256"]),
    )

    plan = read(scene / "event_plan.json")
    (template,) = plan["templates"]
    (binding,) = plan["provenance"]["row_bindings"]
    require(
        template["packet"] == {"kind": "prepared_actor"}, "Expected an actor packet"
    )
    template.update(id=name + ":0", identity=identity)
    template["settlement"].update(
        physical_motion=row["physical_motion"], elemental_motion=row["elemental_motion"]
    )
    binding.update(
        row_sha256=row["sha256"], exact_action_matches=[dict(identity, row=0)]
    )
    plan["provenance"] = dict(
        kind="pc202_standing_contact_control",
        standing_route_sha256=route_sha,
        row_bindings=[binding],
        landing_basis="One declared unguarded contact under the isolated benchmark.",
    )
    plan["source"]["incarnation"] = (
        int.from_bytes(bytes.fromhex(digest(name.encode()))[:8], "little") or 1
    )
    hits = hit_words(
        plan,
        (scene / "hits.txt").read_bytes(),
        school,
        kind,
        stance,
        route["static_packet_flags"],
    )
    plan["hit_values_sha256"] = digest(hits)
    emit(scene / "bundle.json", descriptor)
    emit(scene / "event_plan.json", plan)
    (scene / "hits.txt").write_bytes(hits)
    seal(scene, generated, app_commit, contract["native_source_commit"])
    for file in scene.iterdir():
        if file.name not in POOLED_FILES:
            shutil.copyfile(file, public / file.name)
    result = verify_scene_directory(public, app_commit)
    return {
        "scene": name,
        "school": school,
        "action": identity,
        "contract_sha256": result["contract_sha256"],
        "extension_event_count": route["extension_event_count"],
    }


def stage(args):
    raw = args.routes.read_bytes()
    report = json.loads(raw)
    require(
        report["schema"] == "nioh3-pc202-standing-attack-routes/v1",
        "Unknown route report",
    )
    args.output.mkdir(parents=True)
    for entry in args.data.iterdir():
        if entry.name in ("loadout", "loadout-kernels"):
            continue
        if entry.is_file():
            shutil.copyfile(entry, args.output / entry.name)
        elif entry.is_dir():
            shutil.copytree(entry, args.output / entry.name)
    with tempfile.TemporaryDirectory(dir=args.output.parent) as name:
        receipts = [
            stage_control(
                route,
                args.data,
                args.output,
                Path(name),
                args.samurai_main if route["school"] == "samurai" else args.ninja_main,
                args.app_commit,
                digest(raw),
            )
            for route in report["routes"]
            if len(route["positive_direct_rows"]) == 1
        ]
    return {
        "scope": "Engineering controls for one declared contact. Any action events outside this contact still need separate admission.",
        "routes_sha256": digest(raw),
        "scenes": receipts,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("routes", "data", "samurai-main", "ninja-main", "output", "receipt"):
        parser.add_argument("--" + key, required=True, type=Path)
    parser.add_argument("--app-commit", required=True)
    args = parser.parse_args()
    result = stage(args)
    emit(args.receipt, result)
    print(json.dumps({"staged_controls": len(result["scenes"])}))
