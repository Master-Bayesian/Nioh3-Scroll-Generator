"""Build a public, all-landed PC202 high-stance held attack from native action records."""

import argparse
from hashlib import sha256
from pathlib import Path
import shutil
import struct
import tempfile

from compile_witnessed_cast import controller, emit, read, require
from verify_loadout_release import public_frame, seal, verify_scene_directory


def publish(
    base,
    kernel,
    schedules,
    packets,
    output,
    app_commit,
    current,
    opening=False,
    activate=False,
):
    require(
        not activate or opening,
        "Fresh Water Sword Arts activation has no Ki-depletion endpoint",
    )
    require(not (current and activate), "Choose a read or an assumed activation")
    buff = "current" if current else "untriggered"
    if activate:
        buff = "arts"
    original = verify_scene_directory(base)
    descriptor = original["descriptor"]
    resource = "0x60000019f"
    require(
        descriptor["weapon_class"] == 1
        and descriptor["resource_action"]["resource"] == resource,
        "expected the Samurai dual-swords encounter",
    )
    plan = read(base / "event_plan.json")
    packet = plan["main_address"]
    actions = read(schedules)["actions"]
    flags = read(packets)["actions"]
    addresses = [packet + 0x13C, packet + 0x148]
    templates, values = [], []
    for action in range(25329, 25339):
        phase = actions[str(action)]
        digest = phase["parameter_sha256"]
        rows = controller(resource, action, digest)
        selected = [row for row in phase["rows"] if row["recipient_0"]]
        require(
            [r["order"] for r in selected] == [0, 1],
            "unexpected benchmark recipient rows",
        )
        identity = dict(resource=resource, action=action, parameter_sha256=digest)
        for row in selected:
            i = row["order"]
            require(
                sha256(rows[i]).hexdigest() == row["row_sha256"],
                "damage row hash changed",
            )
            templates.append(
                dict(
                    id=f"{action}-{i}",
                    identity=identity,
                    landed=True,
                    guard=True,
                    packet=dict(kind="prepared_actor"),
                    scored=True,
                )
            )
            values.append([int(flags[str(action)]["class_bits"], 16) | 0x80, 0])
            if action == 25329 and i == 0:
                first = dict(identity, row=i, row_sha256=row["row_sha256"])
    lines = [
        str(len(values)),
        *(" ".join(f"{word:x}" for word in row) for row in [addresses, *values]),
    ]
    hits = ("\n".join(lines) + "\n").encode("ascii")
    plan.update(
        mode=dict(
            kind="held_water_sword",
            arts="full_gauge_activation" if activate else buff,
            objective="opening_three_actions" if opening else "until_depleted",
        ),
        actor_skill_key=0x98CF,
        templates=templates,
        provenance=dict(
            landing_basis="All eligible direct windows land on the grounded frontal benchmark."
        ),
        template_layout_sha256=sha256(struct.pack("<2Q", *addresses)).hexdigest(),
        hit_values_sha256=sha256(hits).hexdigest(),
    )
    plan["source"]["incarnation"] = int.from_bytes(
        sha256(output.name.encode()).digest()[:8], "little"
    )
    shown = dict(
        descriptor,
        id=output.name,
        name="水形剑",
        selection_weapon_class=1,
        current_action_read=True,
        damage=(
            "水形剑（起手三段，最多六次命中）"
            if opening
            else "水形剑（满精力持续至耗尽，正面全中）"
        ),
        resource_action=first,
        scene=dict(
            stance="high",
            kind="martial",
            attack_id=(
                "samurai-dual-swords-water-sword-opening"
                if opening
                else "samurai-dual-swords-water-sword"
            ),
            buff=buff,
        ),
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as work:
        scene = Path(work) / output.name
        shutil.copytree(base, scene)
        emit(scene / "bundle.json", shown)
        emit(scene / "event_plan.json", plan)
        (scene / "hits.txt").write_bytes(hits)
        (scene / "frame.bin").write_bytes(
            public_frame((base / "frame.bin").read_bytes(), plan)
        )
        for name in ("main_cpu.dll", "main_device.dll"):
            shutil.copyfile(kernel / name, scene / name)
        seal(
            scene,
            kernel / "main_gen.inc",
            app_commit,
            read(base / "runtime-contract.json")["native_source_commit"],
        )
        verify_scene_directory(scene, app_commit)
        scene.rename(output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("base", "kernel", "schedules", "packets", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--app-commit", required=True)
    parser.add_argument("--current", action="store_true")
    parser.add_argument(
        "--opening", action="store_true", help="Score at most the first three actions."
    )
    parser.add_argument(
        "--activate",
        action="store_true",
        help="Start the attack by activating Arts Proficiency from a full gauge.",
    )
    publish(**vars(parser.parse_args()))
