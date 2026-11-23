"""Build a fixed martial cast with the shared PC202 peak conditions."""

import argparse
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_coexisting_loadout import publish  # noqa: E402
from compile_witnessed_cast import ROOT, digest, emit, read, require, resolve_controller  # noqa: E402
from verify_loadout_release import verify_scene_directory  # noqa: E402


def skill_rows(data):
    count = struct.unpack_from("<I", data, 4)[0]
    rows = data[8:211400]
    require(len(rows) == count * 192, "Unexpected PC202 skill table extent")
    return {struct.unpack_from("<H", row, 0x18)[0]: row for row in
            (rows[offset:offset + 192] for offset in range(0, len(rows), 192))}


def skill_key(parameter):
    key = struct.unpack_from("<i", parameter, 0xDC)[0]
    return key & 0xFFFF if key >= 0 else None


def suppresses_follow_up(parameter, skills):
    if parameter is None:
        return False
    flags = struct.unpack_from("<Q", parameter, 0x110)[0]
    key = skill_key(parameter)
    return bool(flags & (1 << 39)) or bool(key and key in skills and skills[key][0xB9] == 2)


def actor_route(parameter, secondary, skills):
    """35A220, 377F0C and the FC condition through 377E5C."""
    key = skill_key(parameter)
    skill = skills.get(key)
    arts = key in (0x457F, 0x8677) or (
        skill is not None and skill[0xB9] == 0 and bool(struct.unpack_from("<I", skill, 0x1C)[0] & 0x400)
    )
    flags = struct.unpack_from("<Q", parameter, 0xF8)[0]
    follow_up = (bool(flags & 0x40700000)
                 and not suppresses_follow_up(parameter, skills)
                 and not suppresses_follow_up(secondary, skills))
    return dict(arts_eligible=arts, follow_up_eligible=follow_up)


def stage(args):
    base = verify_scene_directory(args.base)
    descriptor = base["descriptor"]
    plan = read(args.base / "event_plan.json")
    require(plan["mode"] == {"kind": "fixed_contacts"} and descriptor["scene"]["kind"] == "martial",
            "Expected a verified fixed martial cast")
    school = descriptor["school"]
    skills = skill_rows((ROOT / "crates/nioh3-runtime/src/combat_context/pc202_skill_lookup.bin").read_bytes())
    contacts, arts = [], []
    bindings = plan["provenance"]["row_bindings"]
    for template, binding in zip(plan["templates"], bindings, strict=True):
        require(template["landed"] is True and template["guard"] is True and template["scored"] is True,
                "The fixed comparison assumes each declared contact damages the benchmark")
        if template["packet"]["kind"] != "prepared_actor":
            require(len(plan["templates"]) == 1 and school == "ninja",
                    "Use original settlement evidence for a mixed or repeated projectile cast")
            continue
        (identity,) = binding["exact_action_matches"]
        parameter, rows = resolve_controller(identity["resource"], identity["action"], identity["parameter_sha256"])
        row = rows[identity["row"]]
        require(digest(row) == binding["row_sha256"], "Certified contact row changed")
        # 4893B8 selects F1 (active action +F8 bit 38 or 55). A landed ordinary
        # actor HP contact with neutral settlement flags can consume the grant.
        flags = struct.unpack_from("<Q", parameter, 0xF8)[0]
        motion = any(struct.unpack_from("<hh", row, 0x10))
        contacts.append(motion and not flags & ((1 << 38) | (1 << 55)))
        if school == "samurai":
            require(args.secondary_empty and skill_key(parameter) is not None,
                    "The Samurai peak needs a declared empty secondary controller and a martial action")
            arts.append(actor_route(parameter, None, skills))
    owner = plan["candidate_source_address"] - 0xB180
    publish(args.base, args.kernel, "theoretical_peak", args.output, owner,
            peak_contacts=contacts or None, peak_supplies=True, peak_target_ailments=True,
            actor_arts_routes=arts or None)
    return dict(
        scene=args.output.name, ordinary=args.base.name, base_contract=base["contract_sha256"],
        peak_contact_settlement=contacts, actor_arts_routes=arts,
        secondary_controller="empty" if school == "samurai" else None,
        contract_sha256=verify_scene_directory(args.output)["contract_sha256"],
    )


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    for option in ("base", "kernel", "output", "receipt"):
        cli.add_argument("--" + option, required=True, type=Path)
    cli.add_argument("--secondary-empty", action="store_true", help="Declare the fixed cast's empty secondary controller")
    args = cli.parse_args()
    emit(args.receipt, stage(args))
