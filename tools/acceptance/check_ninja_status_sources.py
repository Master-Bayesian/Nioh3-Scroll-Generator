"""Verify owned equipment, status competition and the first hit after Ninja Evade.

Run against the retained PC202 Ninja backpack. The capture starts with an empty
status tree. The successful command and immediate hit are explicit hypotheses.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys
import tempfile


ROLE = (
    "optimizer-live-samurai-20261120/"
    "ninja-reference-game-read-cddd918b04054ddd8f2375d674c66479"
)
# (selected D byte, original comparison RVA, bypass RVA, creation key)
GATES = (
    (0x4EF, 0x84385A, 0x8438D9, 0x1F6),
    (0xA9F, 0x843BD4, 0x843C0D, 0xCB),
    (0x9ED, 0x843DB0, 0x843E2F, 0x31),
)


def read(path):
    return json.loads(path.read_bytes())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def u16(data, at):
    return struct.unpack_from("<H", data, at)[0]


def u32(data, at):
    return struct.unpack_from("<I", data, at)[0]


def packed_rows(tables, name, stride):
    data = tables[name + ".rows"]
    return {
        key: data[8 + index * stride : 8 + (index + 1) * stride]
        for key, index in struct.iter_unpack("<II", tables[name + ".keys"])
    }


def qualify(raw, item, attrs):
    grade = struct.unpack_from("<b", raw, 0x30)[0]
    if grade >= 3 and 1 in raw[0x31:0x33]:
        raise ValueError("This input requires a remodel requirement increment")
    column = int(grade >= 5 and u32(item, 0x60) != 0x2D32)
    needed = list(item[0x18C + column : 0x19A : 2])
    return needed, all(
        available >= required for available, required in zip(attrs, needed, strict=True)
    )


def game_bytes(regions, address, size):
    (value,) = {
        bytes.fromhex(r["bytes"])[
            address - r["address"] : address - r["address"] + size
        ]
        for r in regions
        if r["address"] <= address
        and address + size <= r["address"] + len(r["bytes"]) // 2
    }
    return value


def field_in_outfit(writes, base, address, offset):
    word = writes.get(address + (offset & ~3))
    return base[offset] if word is None else (word >> (8 * (offset & 3))) & 255


def captured_d(bundle, address):
    raw = (bundle / "context/image.bin").read_bytes()
    masks = (bundle / "context/image_known.bin").read_bytes()
    pages = {
        struct.unpack_from("<Q", raw, p)[0]: raw[p + 8 : p + 65544]
        for p in range(0, len(raw), 65544)
    }
    known = {
        struct.unpack_from("<Q", masks, p)[0]: masks[p + 8 : p + 8200]
        for p in range(0, len(masks), 8200)
    }
    result = {}
    for offset, *_ in GATES:
        at = address + offset
        page, local = at & ~0xFFFF, at & 0xFFFF
        if page not in known or not known[page][local // 8] & (1 << (local % 8)):
            raise ValueError(f"Unknown D+{offset:X} in the scored role")
        result[offset] = pages[page][local]
    return result


def candidate_inputs(evidence, sections):
    root = evidence / ROLE
    verified = root / "verified"
    standalone = read(verified / "snapshot.json")
    mem = read(verified / "ordinary-read-memory.json")
    roots, attrs = standalone["roots"], standalone["inputs"]["raw_attrs"]
    if (
        any(standalone["process"][k] != mem[k] for k in ("pid", "creation_filetime"))
        or standalone["inputs"]["status_keys"]
    ):
        raise ValueError("Expected the original Ninja role with an empty active tree")
    worn = [
        game_bytes(mem["regions"], roots["materializer"] + g[0], 1)[0] for g in GATES
    ]
    if worn != [0, 0, 1]:
        raise ValueError(f"Unexpected worn gate bytes {worn}")
    skill = [r for r in standalone["inputs"]["configured_skills"] if r["key"] == 0xD18D]
    if len(skill) != 1 or skill[0]["rank"] != 1 or skill[0]["enabled"] != 1:
        raise ValueError("The role no longer has its recorded Ninja skill enabled")
    component = int.from_bytes(
        game_bytes(mem["regions"], roots["player"] + 0x3B0, 8), "little"
    )
    if (
        not component
        or int.from_bytes(
            game_bytes(mem["regions"], roots["player"] + 0x3A0, 8), "little"
        )
        != roots["owner"]
    ):
        raise ValueError("Recorded Ninja component or owner binding differs")
    marker_at = component + 0x1C0
    marker_captured = any(
        r["address"] <= marker_at
        and marker_at + 4 <= r["address"] + len(r["bytes"]) // 2
        for r in mem["regions"]
    )
    recipe = read(
        Path(__file__).resolve().parents[2]
        / "crates/nioh3-runtime/src/combat_context/kusarigama_strong_pc202.json"
    )
    d_address = recipe["roots"]["d"]
    counts, provenance, selected, examples = {}, {}, {}, {}
    for case in sorted((root / "all-ninja").glob("ninja-type-*-contact")):
        bundle = case / "target-bundle"
        audit = case / "native-inputs"
        captured = captured_d(bundle, d_address)
        tables = sections((bundle / "tables.bin").read_bytes())
        items = packed_rows(tables, "item", 0x1A0)
        effects = packed_rows(tables, "effect", 0xD8)
        inventory_file = case / "export/inventory.json"
        inventory = sorted(
            read(inventory_file)["records"], key=lambda item: item["slot"]
        )
        receipt = read(audit / "SEARCH_RESULT.json")
        if receipt["inventory_file_sha256"] != sha(inventory_file.read_bytes()):
            raise ValueError(
                "Candidate inventory identity differs from its scored file"
            )
        equipment = []
        for instance in inventory:
            raw = bytes(instance["record_bytes"])
            if sha(raw) != instance["record_sha256"]:
                raise ValueError("Owned item bytes differ")
            key = u16(raw, 0)
            sources = []
            for line in range(7):
                effect = u32(raw, 0x38 + 24 * line)
                row = effects.get(effect)
                if row and u16(row, 2) == 0x6B5B:
                    sources.append([effect, u32(raw, 0x3C + 24 * line)])
            qualified = False
            if sources or key == 0xE61A:
                required, qualified = qualify(raw, items[key], attrs)
                witness = dict(
                    item=key,
                    slot=instance["slot"],
                    save_slot=instance["slot_index"],
                    digest=instance["record_sha256"],
                    group_6b5b=sources,
                    attributes_required=required,
                    qualifies=qualified,
                )
                examples[instance["slot_index"]] = witness
            equipment.append((instance, bool(sources and qualified)))
        distribution = Counter()
        with (
            (audit / "NATIVE_INPUTS.jsonl").open() as native,
            (audit / "AUDIT_ROWS.jsonl").open() as placements,
        ):
            for line, placement in zip(native, placements, strict=True):
                raw, plan = json.loads(line), json.loads(placement)
                if raw["ordinal"] != plan["ordinal"] or plan["attrs"] != attrs:
                    raise ValueError("Retained candidate inputs and assignments differ")
                writes = dict(raw["outfit"])
                gates = tuple(
                    field_in_outfit(writes, captured, d_address, gate[0])
                    for gate in GATES
                )
                owned_source = any(
                    equipment[i][1] for i in plan["items"] if i is not None
                )
                if gates != (int(owned_source), 0, 1):
                    raise ValueError(
                        f"{case.name} {raw['ordinal']}: gear-derived rule disagrees with materialized {gates}"
                    )
                distribution[gates] += 1
                if case.name == "ninja-type-6-contact" and raw["ordinal"] in (1, 17):
                    selected[raw["ordinal"]] = (
                        raw,
                        plan,
                        equipment[plan["items"][-1]][0],
                        bundle,
                    )
        if (
            sum(distribution.values())
            != read(case / "NATIVE_PARITY.json")["candidates"]
        ):
            raise ValueError("Incomplete retained candidate coverage")
        counts[case.name] = {
            "".join(map(str, bits)): n for bits, n in sorted(distribution.items())
        }
        provenance[case.name] = {
            "inventory": receipt["inventory_file_sha256"],
            "native_inputs": sha((audit / "NATIVE_INPUTS.jsonl").read_bytes()),
            "assignments": sha((audit / "AUDIT_ROWS.jsonl").read_bytes()),
        }
    if (
        set(selected) != {1, 17}
        or selected[1][1]["items"][:-1] != selected[17][1]["items"][:-1]
    ):
        raise ValueError("The paired equipment witness differs outside the held weapon")
    if (
        sum(v.get("001", 0) for v in counts.values()) != 4480
        or sum(v.get("101", 0) for v in counts.values()) != 40
    ):
        raise ValueError("The retained gear pair no longer discriminates")
    return (
        dict(
            roles=counts,
            gate_order=[
                {"selected_d": hex(g[0]), "source_key": hex(g[3])} for g in GATES
            ],
            worn_gate_bytes=worn,
            current_status_keys=[],
            enabled_skill=skill[0],
            outer_command={
                "opcode": "0x140",
                "component": component,
                "requires_nonzero_component_1c0": True,
                "component_1c0_in_ordinary_read": int.from_bytes(
                    game_bytes(mem["regions"], marker_at, 4), "little"
                )
                if marker_captured
                else None,
            },
            attrs=attrs,
            source_records=examples,
            inputs=provenance,
        ),
        selected,
        recipe["roots"]["source_owner"],
    )


def native_trigger(text, offset, start, bypass, expected):
    """Run each gate up to the real source creation call on the original positive path."""
    from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
    from unicorn import x86_const as reg

    page = start & ~0xFFF
    code = text[page - 0x1000 : page]
    result = []
    for enabled in (0, 1):
        vm = Uc(UC_ARCH_X86, UC_MODE_64)
        vm.mem_map(page, 0x1000)
        vm.mem_write(page, code)
        vm.mem_map(0x10000, 0x4000)
        vm.mem_write(0x10000 + offset, bytes([enabled]))
        vm.mem_write(0x110E0, struct.pack("<Q", 0x12345678))
        for which, val in [
            (reg.UC_X86_REG_RAX, 0x10000),
            (reg.UC_X86_REG_R14, 0),
            (reg.UC_X86_REG_RSI, 0x11000),
            (reg.UC_X86_REG_RSP, 0x12900),
            (reg.UC_X86_REG_RBP, 0x12800),
        ]:
            vm.reg_write(which, val)
        reached = []

        def stop(machine, pc, size, _):
            if pc == bypass:
                reached.append(None)
                machine.emu_stop()
            elif size == 5 and code[pc - page] == 0xE8:
                target = pc + 5 + struct.unpack_from("<i", code, pc - page + 1)[0]
                if target == 0x3EFB90:
                    if machine.reg_read(reg.UC_X86_REG_RCX) != 0x12345678:
                        raise ValueError("Status recipient changed")
                    reached.append(machine.reg_read(reg.UC_X86_REG_EDX))
                    machine.emu_stop()

        vm.hook_add(UC_HOOK_CODE, stop)
        vm.emu_start(start, 0, count=96)
        if reached != [expected if enabled else None]:
            raise ValueError(f"Original source gate {start:X} disagrees: {reached}")
        result.append(reached[0])
    return result


def duration_controls(batch, definition):
    """Vary timing inputs in fresh RAM and observe both original factory arguments."""
    from prepare_creation_batch import value32

    key = definition["native_lookup_key_u16"]
    stat = int(batch.stats["selected"]["STAT"], 16)
    damage = int(batch.stats["selected"]["D"], 16)
    outputs = []
    for label, stat_multiplier, hp_multiplier, add, subtract in (
        ("shortened", 1.0, 0.5, 0.0, 0.25),
        ("longer", 1.25, 2.0, 0.5, 0.0),
    ):
        vm, memory = batch.setup(key)
        for at, number in (
            (stat + 0x1AC, stat_multiplier),
            (batch.hp + 0x2DDC, hp_multiplier),
            (damage + 0x6F8, add),
            (damage + 0x770, subtract),
        ):
            word = int.from_bytes(struct.pack("<f", number), "little")
            memory.write(at, batch.W.val(word, 32), initial=True)
        result = vm.run(0x3EFB90, stop_before={0x207CEB0})
        if result["status"] != "completed_slice" or vm.pc != 0x207CEB0:
            raise ValueError(f"Unresolved source {key:X}: {result}")
        args = [value32(vm, name) for name in ("xmm1", "xmm2", "xmm3")]
        if any(a["known_mask_hex"] != "ffffffff" for a in args):
            raise ValueError("Unknown class-0 factory input")
        outputs.append(
            dict(
                case=label,
                duration=args[0]["f32"],
                a8=args[1]["bits_hex"],
                ac=args[2]["bits_hex"],
            )
        )
    if outputs[0]["duration"] >= outputs[1]["duration"]:
        raise ValueError("Duration controls were not discriminating")
    return outputs


def native_aggregation(evidence):
    stacking = evidence / "r16-native-buffs-20261019/stacking"
    dispatch = evidence / "r16-native-buffs-continue-20261019/dispatch"
    sys.path[:0] = [str(stacking), str(dispatch)]
    import execute_stacking as native
    from prepare_creation_batch import Batch

    _, sections = native.load()
    text = sections[".text"][1]
    branches = [native_trigger(text, *gate) for gate in GATES]
    batch = Batch()
    states, table = {}, {}
    for key in (0x1F6, 0xCB, 0x31, 0x1AE, 0x37D):
        definition = next(
            row for row in batch.definitions if row["native_lookup_key_u16"] == key
        )
        prepared = batch.run(definition)
        if (
            prepared["factory_rva"] != "0x207ceb0"
            or prepared["runtime_status_key_u16"] != 0x312
        ):
            raise ValueError("Original creation did not produce a class-0 source")
        regs = prepared["factory_float_registers"]
        states[key] = {
            "key": 0x312,
            "factor": regs["xmm2"]["f32"],
            "strength_factor": regs["xmm3"]["f32"],
        }
        table[hex(key)] = dict(
            factor=regs["xmm2"]["bits_hex"],
            strength=regs["xmm3"]["bits_hex"],
            raw_duration=prepared["raw_duration_parameter_u32"],
        )
        controls = duration_controls(batch, definition)
        if any(
            (r["a8"], r["ac"]) != (regs["xmm2"]["bits_hex"], regs["xmm3"]["bits_hex"])
            for r in controls
        ):
            raise ValueError(
                f"Candidate duration operands changed source {key:X} strength"
            )
        table[hex(key)]["duration_controls"] = controls
    cases = {
        "no_trigger": [],
        "skill_only": [0x31],
        "gear_and_skill": [0x1F6, 0x31],
        "reverse_order": [0x31, 0x1F6],
        "shared_and_skill": [0xCB, 0x31],
        "after_gear_removed_skill_remains": [0x31],
        "after_membership_removed_and_rebuilt": [],
    }
    factors = {}
    for name, keys in cases.items():
        initial = states[0x1F6]["factor"] if name.startswith("after_") else 1.0
        receipt, _ = native.rebuild_case(
            sections, name, [states[k] for k in keys], initial=initial
        )
        factors[name] = int(receipt["final_factor"]["bits"], 16)
    if list(factors.values()) != [
        0x3F800000,
        0x3F8CCCCD,
        0x3F933333,
        0x3F933333,
        0x3F8CCCCD,
        0x3F8CCCCD,
        0x3F800000,
    ]:
        raise ValueError(f"Class-0 winner or reset changed: {factors}")
    return dict(creation=table, gate_branches=branches, factor_bits=factors), factors


def original_damage(evidence, selected, owner, factors, temp_root):
    sys.path.insert(0, str(evidence / "optimizer-owned-projectiles-20261120"))
    from original_reference import compare

    expected, words = [], []
    with tempfile.TemporaryDirectory(dir=temp_root) as temp:
        temp = Path(temp)
        for ordinal in (1, 17):
            row, _, item, bundle = selected[ordinal]
            (hit,) = row["contacts"]
            for state in [
                "no_trigger",
                "skill_only",
                "gear_and_skill",
                "after_membership_removed_and_rebuilt",
            ]:
                words.append(row["outfit"] + hit + [[owner + 0x2E3C, factors[state]]])
                expected.append(
                    dict(
                        ordinal=ordinal, save_slot=item["slot_index"], assumption=state
                    )
                )
        request = {
            "native_outfit_words": [],
            "native_hit_words": words,
            "components": {"contacts": [{} for _ in words]},
        }
        (temp / "in.json").write_text(
            json.dumps({"top_case": request}), encoding="utf8"
        )
        # OriginalMain deliberately ignores every input word outside the declared
        # mutable ABI. Supply exactly the new aggregate operand in the oracle copy.
        (temp / "context").mkdir()
        for name in ("scene_entry.bin", "image.bin", "image_known.bin"):
            shutil.copyfile(bundle / "context" / name, temp / "context" / name)
        contract = read(bundle / "runtime-contract.json")
        contract["kernel_layout"]["domain"].append([owner + 0x2E3C, owner + 0x2E40])
        (temp / "runtime-contract.json").write_text(
            json.dumps(contract), encoding="utf8"
        )
        compare(temp, temp / "in.json", temp / "out.json")
        contacts = read(temp / "out.json")["top_case"]["components"]["contacts"]
    for control, damage in zip(expected, contacts, strict=True):
        control["damage"] = damage
        admitted = "gear_and_skill" if control["ordinal"] == 17 else "skill_only"
        control["eligible_after_trigger"] = control["assumption"] == admitted
    for idx, (ordinal, golden) in enumerate(
        ((1, (844, 929, 971)), (17, (532, 586, 612)))
    ):
        row = selected[ordinal][0]
        baseline, skill, gear, reset = expected[4 * idx : 4 * idx + 4]
        bits = struct.unpack("<I", struct.pack("<f", baseline["damage"]["total"]))[0]
        if (
            bits != row["expected"]["total"]
            or reset["damage"] != baseline["damage"]
            or tuple(r["damage"]["physical"] for r in (baseline, skill, gear)) != golden
            or any(r["damage"]["elemental"] for r in (baseline, skill, gear))
        ):
            raise ValueError(
                f"Original MAIN control {ordinal}: {row['expected']}, {expected[4 * idx : 4 * idx + 4]}"
            )
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from verify_loadout_release import sections

    report, selected, owner = candidate_inputs(args.evidence, sections)
    report["status"], factors = native_aggregation(args.evidence)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report["original_hits"] = original_damage(
        args.evidence, selected, owner, factors, args.output.parent
    )
    report["scope"] = (
        "Recorded fixed-attribute, required-set candidate inputs. One admitted Ninja evade immediately before "
        "one landed attack, empty initial transient state, an ordinary rebuild, and no intervening status expiry, "
        "consumption or refresh. Source preparation is controlled original execution; creation at a real hit and "
        "its wall-clock duration were not observed. Expiry control starts after removal and rebuild."
    )
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    print(
        json.dumps(
            dict(distribution=report["roles"], original_hits=report["original_hits"])
        )
    )


if __name__ == "__main__":
    main()
