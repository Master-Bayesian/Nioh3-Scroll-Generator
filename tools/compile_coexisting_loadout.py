"""Bind a verified PC202 action and its expanded native kernel to a combined buff condition."""

import argparse
import json
from pathlib import Path
import shutil
import tempfile

from compile_witnessed_cast import emit, require
from verify_loadout_release import CONTRACT, copy_verified_scene, seal, verify_scene_directory


def publish(base, kernel, selection, output, damage_owner, *, peak_contacts=None,
            peak_supplies=False, peak_saturated_target=False, peak_target_ailments=False,
            coating=None, coating_routes=None, actor_arts_routes=None):
    checked = verify_scene_directory(base)
    descriptor = checked["descriptor"]
    plan = json.loads((base / "event_plan.json").read_bytes())
    contract = json.loads((base / CONTRACT).read_bytes())
    action = descriptor["scene"]
    if selection == "deflect_arts_opening":
        require(
            action["buff"] == "arts"
            and plan["mode"]
            == {
                "kind": "held_water_sword",
                "arts": "full_gauge_activation",
                "objective": "opening_three_actions",
            },
            "Expected the sealed Water Sword fresh Arts opening",
        )
        event = "samurai_deflect"
    elif selection == "style_shift_absorption_damage":
        require(
            action["buff"] == "amrita_absorption_damage"
            and plan["triggered_damage"] == "amrita_absorption"
            and plan.get("triggered_damage_owner", plan["source"]["owner"])
            == damage_owner,
            "Expected a matching first-hit absorption action and actor",
        )
        event = "style_shift_then_absorption"
    elif selection == "theoretical_peak":
        if plan["mode"]["kind"] == "held_water_sword":
            require(
                action["buff"] in {"arts", "untriggered", "theoretical_peak"},
                "Expected a sealed baseline held action",
            )
            plan["mode"].pop("current_effect", None)
            plan["mode"]["arts"] = "theoretical_peak"
        else:
            require(
                plan["mode"]["kind"] == "fixed_contacts"
                and action["buff"] in {"current", "untriggered", "amrita_absorption_damage", "theoretical_peak"}
                and plan.get("triggered_damage_owner", damage_owner) == damage_owner
                and plan.get("candidate_source_address") == damage_owner + 0xB180,
                "Expected a fixed-contact plan with the same candidate actor",
            )
            if peak_contacts is not None:
                for template, consumes in zip(plan["templates"], peak_contacts, strict=True):
                    require(type(consumes) is bool, "Expected a native settlement decision")
                    template["peak_consumes_follow_up"] = consumes
            if actor_arts_routes is not None:
                for template, arts_route in zip(plan["templates"], actor_arts_routes, strict=True):
                    require(template["packet"]["kind"] == "prepared_actor" and descriptor["school"] == "samurai",
                            "Arts route requires the Samurai direct actor producer")
                    template["peak_actor_arts"] = arts_route
            require(
                len(plan["templates"]) == 1
                or all("peak_consumes_follow_up" in row for row in plan["templates"]),
                "A multihit peak needs a settlement decision for each contact",
            )
        if peak_supplies:
            plan["peak_held_supplies"] = True
        if peak_saturated_target:
            require(peak_supplies, "Saturated target needs owned supply evidence")
            plan["peak_saturated_target"] = True
        if peak_target_ailments:
            require(peak_supplies and not peak_saturated_target,
                    "Multiple ailments need one policy for owned supply evidence")
            plan["peak_target_ailments"] = True
        if coating is not None:
            require(peak_supplies and coating in {"fire", "water", "lightning", "wind"}
                    and coating_routes is not None and any(coating_routes),
                    "Weapon coating requires supplies and an eligible packet route")
            plan["peak_coating"] = coating
            for template, eligible in zip(plan["templates"], coating_routes, strict=True):
                require(type(eligible) is bool, "Expected a native coating route decision")
                template["peak_allows_coating"] = eligible
        event = "theoretical_peak"
    else:
        raise ValueError(f"Unknown combined condition: {selection}")
    descriptor["id"] = output.name
    action["buff"] = selection if coating is None else selection + "_" + coating
    plan.update(triggered_damage=event, triggered_damage_owner=damage_owner)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        scene = Path(temp) / output.name
        copy_verified_scene(base, scene, checked)
        emit(scene / "bundle.json", descriptor)
        emit(scene / "event_plan.json", plan)
        for name in ("main_cpu.dll", "main_device.dll"):
            shutil.copyfile(kernel / name, scene / name)
        seal(
            scene,
            kernel / "main_gen.inc",
            contract["app_source_commit"],
            contract["native_source_commit"],
        )
        scene.rename(output)
    return descriptor


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("base", type=Path)
    cli.add_argument("kernel", type=Path)
    cli.add_argument(
        "selection",
        choices=[
            "deflect_arts_opening",
            "style_shift_absorption_damage",
            "theoretical_peak",
        ],
    )
    cli.add_argument("output", type=Path)
    cli.add_argument("--damage-owner", type=lambda value: int(value, 0), required=True)
    publish(**vars(cli.parse_args()))
