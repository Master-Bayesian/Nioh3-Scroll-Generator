"""Build immediate-hit damage-dealt scenarios from sealed scenes and replayed owners."""

import argparse
import json
from pathlib import Path
import shutil
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_loadout_release import REQUIRED, seal, verify_scene_directory


def read(path):
    return json.loads(path.read_bytes())


def emit(path, value):
    path.write_bytes((json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode())


def stage(base, reference, kernel, catalog, commit, directory, diagnostic):
    original = verify_scene_directory(base)
    desc = original["descriptor"].copy()
    plan = read(reference / "event_plan.json")
    if len(plan["templates"]) != 1 or plan["mode"] != {"kind": "fixed_contacts"}:
        raise ValueError("Immediate evade scoring requires a single contact")
    for name in REQUIRED - {"bundle.json", "event_plan.json"}:
        if (base / name).read_bytes() != (reference / name).read_bytes():
            raise ValueError(f"Reference changed an unbound input: {name}")
    recorded = read(reference / "bundle.json")
    action = recorded["resource_action"]
    (row,) = (
        r
        for r in read(catalog)["selections"]
        if all(r[k] == action[k] for k in ("resource", "action", "parameter_sha256"))
    )
    if row["school"] != recorded["school"] or not row["weapon"]:
        raise ValueError("Action owner differs from the catalog")
    ninja = row["school"] == "ninja"
    trigger = "ninja_evade" if ninja else "samurai_deflect"
    scene_id = recorded["id"] + ("-evade" if ninja else "-deflect-damage")
    desc.update(
        id=scene_id,
        scene=dict(recorded["scene"], buff=trigger + "_damage"),
        resource_action=action,
        weapon_class=recorded["weapon_class"],
        selection_weapon_class=recorded.get("selection_weapon_class", recorded["weapon_class"]),
        weapon=row["weapon"].split(" · ")[-1],
        diagnostic=diagnostic,
    )
    if diagnostic:
        desc.update(
            name=f"{desc['weapon']} · {action['action']}",
            damage=f"Action {action['action']}, row {action['row']}",
        )
    plan["triggered_damage"] = trigger
    recipes = (
        Path(__file__).resolve().parents[2] / "crates/nioh3-runtime/src/combat_context"
    )
    options = (
        [read(recipes / "kusarigama_strong_pc202.json")]
        if ninja
        else read(recipes / "samurai_direct_pc202.json")
    )
    (owner,) = {
        r["roots"]["source_owner"]
        for r in options
        if r["roots"]["packet"] == plan["main_address"]
    }
    plan["triggered_damage_owner"] = owner
    with tempfile.TemporaryDirectory(dir=directory) as tmp:
        target = Path(tmp) / scene_id
        shutil.copytree(base, target)
        for name in ("main_cpu.dll", "main_device.dll"):
            shutil.copyfile(kernel / name, target / name)
        emit(target / "bundle.json", desc)
        emit(target / "event_plan.json", plan)
        seal(
            target,
            kernel / "main_gen.inc",
            commit,
            read(base / "runtime-contract.json")["native_source_commit"],
        )
        result = verify_scene_directory(target, commit)
        target.rename(directory / scene_id)
    return result["contract_sha256"]


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for key in ("base", "reference", "kernel", "catalog", "directory"):
        p.add_argument(key, type=Path)
    p.add_argument("commit")
    p.add_argument("--diagnostic", action="store_true")
    print(stage(**vars(p.parse_args())))
