"""Compare weapon attack and weapon imbue independently on the landed Water Tsubame projectile."""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile


def read(path):
    return json.loads(path.read_bytes())


def run(evidence, audit, output):
    helper = evidence / "optimizer-owned-projectiles-20261120"
    sys.path.insert(0, str(helper))
    from original_reference import compare

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from verify_loadout_release import sections

    def native(bundle, cases):
        with tempfile.TemporaryDirectory(dir=output.parent) as directory:
            temp = Path(directory)
            hits = [outfit + hit for outfit, contacts in cases for hit in contacts]
            request = dict(
                native_outfit_words=[],
                native_hit_words=hits,
                components={"contacts": [{} for _ in hits]},
            )
            (temp / "input.json").write_text(
                json.dumps({"top_case": request}), encoding="utf8"
            )
            compare(bundle, temp / "input.json", temp / "result.json")
            result = read(temp / "result.json")["top_case"]["components"]["contacts"]
            offsets = iter(result)
            return [[next(offsets) for _ in contacts] for _, contacts in cases]

    scene = "katana-mid-tsubame-gaeshi-water"
    bundle = helper / "ranked" / scene / "bundle"
    case = read(bundle.parent / "equipped.json")["top_case"]
    frame = sections((bundle / "context/frame.bin").read_bytes())
    main = struct.unpack_from("<Q", frame["addresses"], 16)[0]
    recipe = read(
        Path(__file__).resolve().parents[2]
        / "crates/nioh3-runtime/src/combat_context/water_projectile_pc202.json"
    )
    stat = recipe["roots"]["stat"]
    attack = dict(case["native_outfit_words"])[main + 8]
    primary = dict(case["native_outfit_words"]).get(
        stat + 0x24, struct.unpack_from("<I", frame["stat"], 0x24)[0]
    )
    cases, inputs = [], []
    for delta, selector, power in [
        (0, 0xFFFFFFFF, 100),
        (0, 22, 16),
        (200, 0xFFFFFFFF, 100),
        (200, 22, 16),
    ]:
        inputs.append(
            dict(
                packet_attack=attack + delta,
                weapon_element=selector,
                packet_power=power,
            )
        )
        imbue = 0 if selector == 0xFFFFFFFF else power
        patch = [
            [stat + 0x24, primary + delta],
            [stat + 0x3C, selector],
            [stat + 0x40, imbue],
            [main + 8, attack + delta],
            [main + 0x14, selector],
            [main + 0x18, power],
        ]
        cases.append((case["native_outfit_words"] + patch, case["native_hit_words"]))
    controls = [
        row | {"contacts": h}
        for row, h in zip(inputs, native(bundle, cases), strict=True)
    ]
    scores = [c["contacts"][2] for c in controls]
    if [hit["elemental"] for hit in scores] != [847, 847, 1126, 1126] or any(
        hit["physical"] for hit in scores
    ):
        raise ValueError(f"Unexpected water projectile response: {scores}")
    if controls[0]["contacts"][0] == controls[1]["contacts"][0]:
        raise ValueError("Imbue control did not discriminate on the direct hit")
    if [c["total"] for c in controls[0]["contacts"]] != [666, 0, 847]:
        raise ValueError("Control differs from the retained game observation")

    role = evidence / "optimizer-live-samurai-20261120/final-common-acceptance" / scene
    chosen = (21, 42)

    def selected(file):
        with (audit / file).open(encoding="utf8") as stream:
            return {
                r["ordinal"]: r
                for line in stream
                if (r := json.loads(line))["ordinal"] in chosen
            }

    rows, words = selected("AUDIT_ROWS.jsonl"), selected("NATIVE_INPUTS.jsonl")
    a, b = [rows[i] for i in chosen]
    if (
        a["items"][:-1] != b["items"][:-1]
        or a["attrs"] != b["attrs"]
        or a["items"][-1] == b["items"][-1]
    ):
        raise ValueError("Weapon-only comparison differs outside the weapon slot")
    # Backpack::from_slots uses a stable ordering by (slot, original ordinal).
    inventory = sorted(
        read(role / "inventory.json")["records"], key=lambda r: r["slot"]
    )
    outputs = native(
        role / "bundle", [(words[i]["outfit"], words[i]["contacts"]) for i in chosen]
    )
    outfits = []
    for i, hits in zip(chosen, outputs, strict=True):
        idx = rows[i]["items"][-1]
        item = inventory[idx]
        if item["slot"] != 0:
            raise ValueError("Compared item is not a primary weapon")
        record = bytes(item["record_bytes"])
        if hashlib.sha256(record).hexdigest() != item["record_sha256"]:
            raise ValueError("Inventory weapon digest differs")
        for channel in ("total", "physical", "elemental"):
            if (
                struct.unpack(
                    "<I", struct.pack("<f", sum(hit[channel] for hit in hits))
                )[0]
                != words[i]["expected"][channel]
            ):
                raise ValueError("Native candidate score differs from the App search")
        outfits.append(
            dict(
                ordinal=i,
                weapon_slot_index=item["slot_index"],
                item_key=hex(int.from_bytes(record[:2], "little")),
                weapon_sha256=item["record_sha256"],
                contacts=hits,
            )
        )
    if outputs[0][2]["elemental"] == outputs[1][2]["elemental"]:
        raise ValueError("The owned weapons did not discriminate on the projectile")
    receipt = dict(
        scene=scene,
        scope="PC202, fixed target and landed contacts; isolated numerical controls and two real owned weapons",
        controlled=controls,
        owned_weapons=outfits,
        other_slots=a["items"][:-1],
        attrs=a["attrs"],
    )
    output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf8")
    print(json.dumps(receipt))


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ("evidence", "audit", "output"):
        p.add_argument(arg, type=Path)
    run(**vars(p.parse_args()))
