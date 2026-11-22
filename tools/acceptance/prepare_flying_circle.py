"""Build the two captured fixed-contact Flying Circle scenes for the ordinary reader."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_loadout_release import public_frame, seal, sections


def read(path):
    return json.loads(path.read_bytes())


def write(path, value):
    path.write_bytes((json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode())


def known(region, offset, size):
    sl = slice(offset * 2, (offset + size) * 2)
    if region["known_mask_hex"][sl].lower() != "ff" * size:
        raise ValueError(f"Unknown {region['role']} field at {offset:#x}")
    return bytes.fromhex(region["bytes_hex"][sl])


def words(seq):
    return " ".join(f"{w:x}" for w in seq)


def build(data, base, kernel, capture, bindings, catalog, app_commit):
    witness = read(capture)
    raw = witness["raw_capture"]
    if (
        hashlib.sha256(capture.read_bytes()).hexdigest()
        != "3082956033c0626b52fafb361eb20682f20b168abe7983769f46cbe05b8aacdb"
    ):
        raise ValueError("Unexpected Flying Circle capture")
    shots = {s["sequence"]: s for s in raw["snapshots"]}
    actions = read(catalog)
    rows = read(bindings)["summary"]["flying_circle"]
    evidence = []
    for hit, binding, variant in zip(
        raw["frames"], rows, ("early", "full"), strict=True
    ):
        owner = binding["owner_action"]
        if (
            hit["action_id"] != owner
            or not hit["source_owner_is_player"]
            or hit["spawn_association"] != "observed_birth"
        ):
            raise ValueError("Witness lacks the selected owner's projectile birth")
        regions = {r["role"]: r for r in shots[hit["entry_sequence"]]["regions"]}
        source = regions["source.projectile"]
        packet = regions["MAIN.entry_prestate"]
        damage = known(regions["MAIN.damage_row"], 0, 192)
        parameter = known(regions["MAIN.selected_action_parameter"], 0, 336)
        head = struct.unpack("<4I", known(source, 0x5D0, 16))
        if not (
            binding["row_sha256"] == hashlib.sha256(damage).hexdigest()
            and binding["parameter_sha256"] == hashlib.sha256(parameter).hexdigest()
            and binding["target_action"]
            == int.from_bytes(known(source, 0x758, 4), "little")
            and binding["row"] == known(source, 0x75C, 1)[0]
            and binding["carrier"] == head[0]
            and head[1] == 0
            and known(packet, 0x14, 8) == struct.pack("<II", 0xFFFFFFFF, 100)
            and known(packet, 0x298, 4) == bytes(4)
            and known(source, 0x260, 8) == int(hit["player"], 16).to_bytes(8, "little")
            and known(packet, 0x28, 8)
            == known(packet, 0x198, 8)
            == int(hit["source"], 16).to_bytes(8, "little")
        ):
            raise ValueError(
                "Recorded controller, projectile or row differs from the static selection"
            )
        plan = read(base / "event_plan.json")
        descriptor = read(base / "bundle.json")
        scene_id = "ninja-shield-spear-flying-circle-" + variant
        scene = data / "loadout" / scene_id
        shutil.copytree(base, scene)
        template = plan["templates"][0]
        obj = 0x600002000000
        tag = int.from_bytes(known(source, 0, 4), "little")
        skill = int.from_bytes(known(packet, 0x136, 2), "little")
        if (
            skill != int.from_bytes(parameter[0xDC:0xE0], "little")
            or tag != head[0] << 4 | 4
        ):
            raise ValueError("Recorded skill or source tag differs from its producer")
        owner_key = {k: binding[k] for k in ("resource", "parameter_sha256")} | {
            "action": owner
        }
        target_key = dict(
            resource=binding["resource"],
            parameter_sha256=binding["target_parameter_sha256"],
            action=binding["target_action"],
        )
        template.update(
            id=scene_id,
            identity=target_key,
            packet=dict(
                kind="owner_weapon_projectile",
                object=obj,
                incarnation=1,
                owner=plan["source"]["owner"],
                descriptor_kind=0,
                source_tag=tag,
            ),
        )
        template.pop("settlement", None)
        plan["actor_skill_key"] = skill
        element_policy, element_flags = struct.unpack("<iI", known(source, 0x5F8, 8))
        (packet_flags,) = struct.unpack("<I", known(packet, 0x13C, 4))
        if (
            element_policy == -1
            and not element_flags & 0x40000
            or known(packet, 0x288, 2)
            != bytes((0, int(bool(packet_flags & 0x200) and head[3] != 2)))
            or known(packet, 0x0C, 4) != struct.pack("<I", damage[0x94])
        ):
            raise ValueError(
                "Native projectile packet differs from its descriptor policy"
            )
        plan["initial_projectile"] = dict(
            object=obj,
            descriptor_head=list(head),
            descriptor_5f8=element_policy,
            descriptor_5fc=element_flags,
            packet_flags=packet_flags,
            secondary_element_selector=bool(damage[5] & 0x20),
        )
        main = plan["main_address"]
        offsets = (0x13C, 0x198, 0x19C)
        addrs = [main + off for off in offsets]
        values = [
            int.from_bytes(known(packet, 0x13C, 4), "little"),
            obj & 0xFFFFFFFF,
            obj >> 32,
        ]
        hits = f"1\n{words(addrs)}\n{words(values)}\n".encode("ascii")
        plan["template_layout_sha256"] = hashlib.sha256(
            struct.pack("<3Q", *addrs)
        ).hexdigest()
        plan["hit_values_sha256"] = hashlib.sha256(hits).hexdigest()
        plan["provenance"] = dict(
            owner=owner_key,
            charge=variant,
            capture_frame=hit["id"],
            row_bindings=[
                dict(
                    template=0,
                    aad_address=read_recipe_aad(),
                    row_sha256=binding["row_sha256"],
                    exact_action_matches=[target_key | {"row": binding["row"]}],
                    identity_determined=True,
                )
            ],
        )
        (selected,) = [
            x
            for x in actions["selections"]
            if all(x[k] == v for k, v in owner_key.items())
        ]
        descriptor.update(
            id=scene_id,
            name=selected["name"],
            school="ninja",
            weapon=selected["weapon"].split(" · ")[-1],
            weapon_class=17,
            selection_weapon_class=17,
            damage=selected["name"]
            + ("（早放）" if variant == "early" else "（蓄满）"),
            resource_action=owner_key | {"projectile_slot": binding["slot"]},
            scene=dict(
                kind="martial", stance="none", attack_id=scene_id, buff="current"
            ),
        )
        selected.update(
            scene_ids=list(dict.fromkeys([*selected["scene_ids"], scene_id])),
            plan_ready=True,
            weapon_class=17,
            attack_id=scene_id,
            stance="none",
        )
        write(scene / "event_plan.json", plan)
        write(scene / "bundle.json", descriptor)
        (scene / "hits.txt").write_bytes(hits)
        frame = sections((base / "frame.bin").read_bytes())
        frame.pop("packet_kind", None)
        frame["aad_packet_kind"] = damage[0x94:0x95]
        frame["weapon_element_packet"] = bytes([1])
        frame["hit_packets"] = bytes(12)
        frame_bytes = b"".join(
            struct.pack("<I", len(k)) + k.encode() + struct.pack("<I", len(v)) + v
            for k, v in frame.items()
        )
        (scene / "frame.bin").write_bytes(public_frame(frame_bytes, plan))
        original = read(base / "runtime-contract.json")
        if (
            original["kernel_layout"]["generated_main_sha256"]
            != hashlib.sha256(kernel.read_bytes()).hexdigest()
        ):
            raise ValueError("Base scene and generated kernel differ")
        contract = seal(scene, kernel, app_commit, original["native_source_commit"])
        evidence.append(
            dict(
                scene=scene_id,
                owner=owner_key,
                target=target_key,
                row=binding["row"],
                observed_hp_loss=binding["observed_hp_loss"],
                files=contract["files"]["bundle.json"],
            )
        )
    write(data / "loadout/catalog.json", actions)
    return evidence


def read_recipe_aad():
    recipe = (
        Path(__file__).resolve().parents[2]
        / "crates/nioh3-runtime/src/combat_context/kusarigama_strong_pc202.json"
    )
    return read(recipe)["roots"]["attack_data"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("data", "base", "kernel", "capture", "bindings", "catalog"):
        parser.add_argument(arg, type=Path)
    parser.add_argument("app_commit")
    args = parser.parse_args()
    result = build(**vars(args))
    write(args.data.parent / "SCENES.json", result)
