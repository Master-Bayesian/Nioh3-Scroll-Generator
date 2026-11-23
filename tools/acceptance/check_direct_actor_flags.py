"""Compare PC202 direct-actor packet classification for two fixed Ninja casts."""

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

from check_empty_status_factors import memory_bytes

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compile_witnessed_cast import ROOT, controller, extract, read, require


CASES = [
    ("0x6000000fd", 25047, [0, 1, 2, 3]),
    ("0x600000158", 25299, [1, 3, 5, 7]),
]
NATIVE_BITS = {
    0x2D: 1,
    0x2E: 2,
    0x33: 4,
    0x32: 8,
    0x2F: 0x40,
    0x30: 0x10,
    0x31: 0x20,
    0xEC: 0x400000,
    0xF5: 0x4000000,
}


def controller_vm(native, sections, parameter, rows, role_class):
    cpu = native.CPU(sections)
    state, layer, param = native.H + 0x800, native.OUTER, native.OUTER + 0x800
    table, row_base = native.OUTER + 0x1800, native.OBJECTS
    component, actor, owner = native.H + 0x2500, native.H + 0x3000, native.H + 0x4500
    cpu.q(component + 8, state)
    cpu.q(state + 0xE8, layer)
    cpu.q(layer + 0x20, param)
    cpu.write(param, parameter)
    cpu.q(layer + 0x40, table)
    cpu.write(layer + 0x48, struct.pack("<HH", 0, len(rows)))
    for i, row in enumerate(rows):
        cpu.q(table + 8 * i, row_base + 192 * i)
        cpu.write(row_base + 192 * i, row)
    cpu.q(state + 0xE0, actor)
    cpu.q(actor + 0x3A0, owner)
    cpu.write(owner + 0x2C, role_class)
    return cpu, state, layer, component


def original_flags(native, sections, parameter, rows, role_class, state_1060):
    cpu, state, layer, component = controller_vm(
        native, sections, parameter, rows, role_class
    )
    if state_1060 is not None:
        cpu.write(state + 0x1060, state_1060)

    def query(rva, rcx, *, predicate=None, mask):
        cpu.uc.reg_write(native.X.UC_X86_REG_R8, layer)
        run = cpu.run(rva, rcx=rcx, rdx=predicate)
        if run["completed"]:
            return {"value": cpu.uc.reg_read(native.X.UC_X86_REG_RAX) & mask}
        return {"value": None, "first_missing_rva": run["missing"]["reader_rva"]}

    result = {
        f"{key:#x}": query(0x2E9948, state, predicate=key, mask=255)
        for key in (*NATIVE_BITS, 0x75, 0xFC, 0x104)
    }
    require(query(0x2B2D3C, component, mask=255)["value"] == 1, "Missing martial link")
    skill = query(0x9CB920, component, mask=65535)["value"]
    # FC checks the previous/secondary action through state+2E8. That link
    # is deliberately left unknown in this retained-role reconstruction.
    return skill, result


def element_branch(native, sections, row, selected, expect_rva):
    cpu = native.CPU(sections)
    owner, packet, row_address = native.H, native.OUTER, native.OBJECTS
    if selected is not None:
        cpu.d(owner + 0x2E9C, selected)
    cpu.d(owner + 0x2EA4, 101)
    cpu.q(owner + 0x2DC0 + 0x378, 0)  # Controlled null optional object for this test.
    cpu.d(packet + 0x13C, 0)
    cpu.write(row_address, row)
    for register, value in (
        (native.X.UC_X86_REG_RBX, owner),
        (native.X.UC_X86_REG_RDI, row_address),
        (native.X.UC_X86_REG_R15, packet),
    ):
        cpu.uc.reg_write(register, value)
    run = cpu.run(0x2B5649, rcx=owner + 0x2DC0, stop_rva=expect_rva)
    if selected is None:
        require(
            not run["completed"] and run["missing"]["reader_rva"] == "0x2b5649",
            "Unobserved selected element was treated as known",
        )
        return {"selected_element": None, "first_missing_rva": "0x2b5649"}
    flags = int.from_bytes(cpu.raw(packet + 0x13C, 4), "little")
    require(
        run["completed"] and flags == (0 if selected == 0xFFFFFFFF else 1 << 28),
        "Unexpected element override",
    )
    stored = None
    if selected != 0xFFFFFFFF:
        # Ordinary actor, no descriptor or late field override. The 2B5988
        # epilogue packs the exact local words produced by 2B567F..2B5698.
        save = cpu.run(0x2B5988, stop_rva=0x2B59B7)
        stored = list(struct.unpack("<II", cpu.raw(packet + 0x14, 8)))
        require(
            save["completed"] and stored == [selected, 101],
            "Original final element stores differ",
        )
    return {
        "controlled_selected_element": selected,
        "flags": hex(flags),
        "original_packet_element_and_power": stored,
        "trace_sha256": run["trace_sha256"],
    }


def verify(proofs, role, evaluator):
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(proofs / "r16-native-buffs-20261019/stacking"))
    sys.path.insert(0, str(evaluator))
    import execute_stacking as native
    from packet_classification import action_packet_flags

    _, sections = native.load()
    raw_actions = read(
        proofs / "event-graph-20261111/NATIVE_CLASSIFICATION_INPUTS.json"
    )
    expected = read(ROOT / "data/loadout/catalog.json")["selections"]
    snapshot, memory = (
        read(role / name) for name in ("snapshot.json", "ordinary-read-memory.json")
    )
    require(
        all(memory[k] == v for k, v in snapshot["process"].items()),
        "Role generation differs",
    )
    regions = [(r["address"], bytes.fromhex(r["bytes"])) for r in memory["regions"]]

    def operand(base, at, size=4):
        return memory_bytes(regions, base + at, size)

    player, owner = (snapshot["roots"][k] for k in ("player", "owner"))
    component = int.from_bytes(operand(player, 0x3B0, 8), "little")
    state = int.from_bytes(operand(component, 8, 8), "little")
    require(operand(player, 0x3A0, 8) == struct.pack("<Q", owner), "Wrong role owner")
    role_class = operand(owner, 0x2C)
    require(role_class == struct.pack("<I", 1), "Expected a Ninja role")
    player_flag = int.from_bytes(operand(player, 0xAB84), "little")
    role_flags = int.from_bytes(operand(owner, 0x188), "little")
    require(
        not role_flags & 2 and player_flag != 0xFFFFFFFF, "Reference actor flags differ"
    )
    held_element = operand(owner, 0x2E9C)
    previous = operand(state, 0x2E8, 8)
    require(previous is None, "Use the observed previous controller in the replay")
    actions, spiral = [], None
    for resource, action, landed in CASES:
        (item,) = [
            r for r in expected if (r["resource"], r["action"]) == (resource, action)
        ]
        (raw,) = [
            r
            for r in raw_actions
            if (r["resource"], r["action"]) == (resource, action)
            and hashlib.sha256(bytes.fromhex(r["raw"])).hexdigest()
            == item["parameter_sha256"]
        ]
        parameter = bytes.fromhex(raw["raw"])
        rows = controller(resource, action, item["parameter_sha256"])
        skill, preds = original_flags(
            native, sections, parameter, rows, role_class, operand(state, 0x1060)
        )
        require(
            all(preds[hex(k)]["value"] in (0, 1) for k in NATIVE_BITS),
            "Unresolved static predicate",
        )
        flags = sum(bit for k, bit in NATIVE_BITS.items() if preds[hex(k)]["value"])
        require(
            flags == action_packet_flags(parameter) == 0x04000042
            and skill == int.from_bytes(parameter[0xDC:0xDE], "little")
            and preds["0x104"]["value"] == 0,
            "Native action classification differs",
        )
        selectors = []
        for index in landed:
            row = rows[index]
            row_flags = int.from_bytes(row[:8], "little")
            require(
                not row_flags & ((1 << 38) | (1 << 39)) and row[32] >= 29,
                "This direct-contact row uses another element override rule",
            )
            selectors.append(
                {"row": index, "row_flags": hex(row_flags), "row_type": row[32]}
            )
        checks = [
            element_branch(native, sections, rows[landed[0]], v, exit_rva)
            for v, exit_rva in [(None, 0x2B56A1), (0xFFFFFFFF, 0x2B56A1), (0, 0x2B574D)]
        ]
        actions.append(
            {
                "resource": resource,
                "action": action,
                "parameter_sha256": item["parameter_sha256"],
                "skill_key": skill,
                "static_packet_bits": hex(flags),
                "native_queries": preds,
                "landed_rows": selectors,
                "bit28_controls": checks,
            }
        )
        if action == 25047:
            witness = (
                proofs
                / "martial-event-live-20261111/phase01-crescent/flying-circle-clean.json"
            )
            contacts = extract(
                read(witness),
                566,
                action,
                item["parameter_sha256"],
                rows,
                read(
                    ROOT
                    / "crates/nioh3-runtime/src/combat_context/kusarigama_strong_pc202.json"
                ),
            )
            require(
                [r["row"] for r in contacts] == landed
                and all(
                    r["skill_key"] == skill and r["packet_flags"] == 0x060000C2
                    for r in contacts
                ),
                "Spiral Thrust four-contact game control differs",
            )
            spiral = contacts
    return {
        "schema": "pc202-direct-actor-flags/v1",
        "actions": actions,
        "spiral_observation": spiral,
        "role": {
            "process": snapshot["process"],
            "owner_flags": hex(role_flags),
            "actor_ab84": player_flag,
            "owner_2e9c": None if held_element is None else held_element.hex(),
        },
        "main_direct_read": {
            "rva": "0x78d545",
            "bit24": False,
            "bit28_rule": "owner+2E9C is not -1 for all eight listed rows",
        },
        "scope": "Action-defined bits, positive skill links, and the current Ninja branch. "
        "The full Wild Wolves II packet and its game damage were not observed.",
    }


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("proofs", "role", "evaluator"):
        p.add_argument(name, type=Path)
    p.add_argument("--output", type=Path)
    args = p.parse_args()
    result = json.dumps(verify(args.proofs, args.role, args.evaluator), indent=2) + "\n"
    if args.output:
        args.output.write_text(result, encoding="utf-8")
    else:
        print(result, end="")
