"""Stage public per-action inputs using catalog-bound rows and canonical Windows scorers."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct

from compile_loadout_event_inputs import compile_descriptors
from verify_loadout_release import public_frame, seal, verify_scene_directory


def bind_candidate_sources(plan, recipe):
    field, = (f for f in recipe["fields"] if f["root"] == "source_actor" and f["path"] == [944, 8, 2664])
    plan.update(candidate_source_address=recipe["roots"]["source"], candidate_category_field=field["at"])
    held = recipe["roots"]["source_owner"] + 0x590
    if all(any(f["at"] == held + offset and f["n"] == 4 and f["root"] == "source_owner"
               for f in recipe["fields"]) for offset in (0, 0x14, 0x18)):
        plan["candidate_inventory_record"] = held


def stage(source, base, generated, groups, resolver, output, source_commit, native_commit):
    names = (
        "katana-light", "katana-mid-strong-current", "katana-mid-sword-ki-current",
        "samurai-odachi-mid-quick", "samurai-odachi-mid-strong", "samurai-dual-swords-mid-quick",
        "katana-mid-tsubame-gaeshi-water",
    )
    verify_scene_directory(base, source_commit, scene_id="kusarigama-strong")
    output.mkdir(parents=True, exist_ok=False)
    recipes = Path(__file__).resolve().parents[1] / "crates/nioh3-runtime/src/combat_context"
    samurai = json.loads((recipes / "samurai_direct_pc202.json").read_bytes())
    samurai.append(json.loads((recipes / "water_projectile_pc202.json").read_bytes()))
    by_key = {bytes(r["action_row_key"]): r for r in samurai}
    shutil.copytree(base, output / "kusarigama-strong")
    families = json.loads(groups.read_bytes())["groups"]
    for name in names:
        src = source / name
        dest = output / name
        dest.mkdir()
        family, = [row for row in families if name in row["scenes"]]
        kernel = generated / family["id"]
        plan = json.loads((src / "event_plan.json").read_bytes())
        descriptor = json.loads((src / "bundle.json").read_bytes())
        if plan["mode"] != {"kind": "fixed_contacts"} or plan.get("single_use") is not None:
            raise ValueError(f"{name} requires additional state inputs")
        bindings = plan["provenance"]["row_bindings"]
        root = bindings[0]
        match, = root["exact_action_matches"]
        key = match["resource"].encode() + b"\0" + struct.pack("<HI", match["action"], match["row"]) + bytes.fromhex(match["parameter_sha256"])
        recipe = by_key[hashlib.sha256(key).digest()]
        bind_candidate_sources(plan, recipe)
        if "candidate_inventory_record" in plan:
            kernel = generated / "held-record" / family["id"]
        descriptor["resource_action"] = {**match, "row_sha256": root["row_sha256"]}
        raw = (src / "hits.txt").read_bytes()
        address = root["aad_address"]
        main = plan["main_address"]
        incarnation = int.from_bytes(hashlib.sha256(name.encode()).digest()[:8], "little") or 1
        projectile_scene = any(t["packet"]["kind"] != "prepared_actor" for t in plan["templates"])
        offsets = [0x13c, 0x144, 0x148]
        if projectile_scene:
            offsets += [0x198, 0x19c, 0x280, 0x284, 0x288, 0x28c, 0x290, 0x29c]
        allowed = {main + offset for offset in offsets}
        if any(not (address <= word < address + 192 or word in allowed)
               for word in (int(v, 16) for v in raw.splitlines()[1].split())):
            raise ValueError(f"{name} contains a hit input with undeclared ownership")
        rows = compile_descriptors(plan, resolver, address)
        lines = raw.decode("ascii").splitlines()
        addresses = [int(value, 16) for value in lines[1].split()]
        columns = [i for i, a in enumerate(addresses) if a in allowed]
        writes = [addresses[i] for i in columns] + [plan["main_address"] + 0x190, plan["main_address"] + 0x194]
        contacts = []
        spawned = 0
        for line, binding, body, template in zip(lines[2:], bindings, rows, plan["templates"], strict=True):
            selected, = binding["exact_action_matches"]
            if hashlib.sha256(body).hexdigest() != binding["row_sha256"]:
                raise ValueError("Attack parameter differs from the selected row")
            old = [int(v, 16) for v in line.split()]
            packet = template["packet"]
            if packet["kind"] == "prepared_actor":
                if any(selected[key] != match[key] for key in ("resource", "action", "parameter_sha256")):
                    raise ValueError(f"{name} has another actor action")
                pointer = address if selected["row"] == match["row"] else 0x600000100000 + 192 * selected["row"]
                if projectile_scene:
                    actor = recipe["roots"]["source_actor"]
                    if (old[addresses.index(main+0x198)] | old[addresses.index(main+0x19c)] << 32) != actor:
                        raise ValueError("Direct contact has a different actor")
            elif packet["kind"] == "unimbued_owner_projectile":
                obj = recipe["projectiles"][spawned]
                if (selected["action"], selected["row"], body.hex()) != (obj["action"], obj["selector"], obj["damage_row"]):
                    raise ValueError("Spawn descriptor selects a different child action row")
                if packet["source_tag"] != (obj["descriptor_head"][0] << 4 | 4) or packet["owner"] != plan["source"]["owner"]:
                    raise ValueError("Projectile is not bound to this actor and resource")
                a, b = addresses.index(main+0x198), addresses.index(main+0x19c)
                if (old[a] | old[b] << 32) != packet["object"]:
                    raise ValueError("Projectile source and packet disagree")
                actor = obj["object"]
                spawned += 1
                pointer = obj["row_address"]
                old[a], old[b] = actor & 0xffffffff, actor >> 32
                packet.update(object=actor, incarnation=(incarnation - 1 + spawned) % ((1 << 64) - 1) + 1)
            else:
                raise ValueError(f"Unsupported packet kind in {name}")
            contacts.append([old[i] for i in columns] + [pointer & 0xffffffff, pointer >> 32])
        if projectile_scene and spawned != len(recipe["projectiles"]):
            raise ValueError("Event plan and constructor count differ")
        def words(items):
            return " ".join(format(v, "x") for v in items)
        hits = ("\n".join([str(len(contacts)), words(writes), *(words(row) for row in contacts)]) + "\n").encode()
        plan["hit_values_sha256"] = hashlib.sha256(hits).hexdigest()
        plan["template_layout_sha256"] = hashlib.sha256(b"".join(struct.pack("<Q", at) for at in writes)).hexdigest()
        plan["provenance"] = {"kind": "resource_bound_training_encounter", "row_bindings": bindings}
        # The normalized owner is part of the evaluator layout. Use an action
        # namespace for this immutable plan's event source incarnation.
        plan["source"]["incarnation"] = incarnation
        for file in ("arena.bin", "mapped.bin", "programs.txt", "tables.bin", "source_numeric.bin",
                     "live_groups.txt", "survival_groups.txt", "gpu_setup.txt"):
            shutil.copyfile(base / file, dest / file)
        for file in ("main_cpu.dll", "main_device.dll"):
            shutil.copyfile(kernel / file, dest / file)
        (dest / "event_plan.json").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        (dest / "bundle.json").write_text(json.dumps(descriptor, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        (dest / "frame.bin").write_bytes(public_frame((src / "frame.bin").read_bytes(), plan))
        (dest / "hits.txt").write_bytes(hits)
        seal(dest, kernel / "main_gen.inc", source_commit, native_commit)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("source", "base", "generated", "groups", "resolver", "output"):
        parser.add_argument(arg, type=Path)
    parser.add_argument("source_commit")
    parser.add_argument("native_commit")
    stage(**vars(parser.parse_args()))
