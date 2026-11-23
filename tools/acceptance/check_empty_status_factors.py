"""Execute the PC202 owner status reset and rebuild against the retained Ninja tree."""

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def memory_bytes(regions, address, width):
    data = {}
    for start, raw in regions:
        for pos in range(max(start, address), min(start + len(raw), address + width)):
            value = raw[pos - start]
            require(data.setdefault(pos, value) == value, "Conflicting role reads")
    return (
        bytes(data[i] for i in range(address, address + width))
        if len(data) == width
        else None
    )


def examine(role):
    names = ("snapshot.json", "ordinary-read-memory.json", "reference-target.json")
    files = {name: (role / name).read_bytes() for name in names}
    snap, memory, target = (json.loads(files[name]) for name in names)
    require(
        all(snap["process"][k] == memory[k] for k in ("pid", "creation_filetime"))
        and snap["module_base"] == memory["module_base"],
        "Process identity differs",
    )
    regions = [(r["address"], bytes.fromhex(r["bytes"])) for r in memory["regions"]]
    owner, player, d = (snap["roots"][k] for k in ("owner", "player", "materializer"))
    tree = snap["status_tree_raw"]
    header, sentinel = bytes(tree["header"]), bytes(tree["sentinel"])
    old, size = struct.unpack("<QQ", header)
    require(
        tree["nodes"] == tree["objects"] == snap["inputs"]["status_keys"] == []
        and size == 0
        and struct.unpack_from("<QQQ", sentinel) == (old, old, old)
        and sentinel[24:26] == b"\x01\x01"
        and memory_bytes(regions, old, len(sentinel)) == sentinel
        and memory_bytes(regions, owner + 0x3108, 16) == header
        and memory_bytes(regions, player + 0x3A0, 8) == struct.pack("<Q", owner),
        "Missing empty actor tree or actor ownership",
    )
    materializer = memory_bytes(regions, d, 0xB00)
    mission = target["actor_conditions"]
    require(
        materializer is not None
        and target["actor"] == player
        and memory_bytes(regions, snap["module_base"] + 79330664, 8)
        == struct.pack("<Q", mission["manager"])
        and memory_bytes(regions, mission["manager"] + 0x2820 + 16, 8)
        == struct.pack("<Q", len(mission["members"])),
        "Missing role D or mission membership",
    )
    observed = {}
    for offset, width in [
        (0x2E48, 4),
        (0x2E4C, 4),
        (0x3120, 8),
        (0x3128, 8),
        (0x32F4, 1),
    ]:
        value = memory_bytes(regions, owner + offset, width)
        observed[hex(offset)] = value.hex() if value is not None else None
    return sentinel, {
        "input_sha256": {k: digest(v) for k, v in files.items()},
        "process": snap["process"],
        "owner": hex(owner),
        "tree_members": size,
        "materializer_sha256": digest(materializer),
        "mission_members": mission["members"],
        "ordinary_owner_fields": observed,
    }


def run_probe(native, sections, sentinel, case):
    cpu = native.CPU(sections)
    agg = native.AGG
    if case in {"empty_queue", "queue_unobserved", "skip_reset"}:
        # Rebase the observed 3 self-links. Preserve every other sentinel byte.
        cpu.write(
            native.TREE, struct.pack("<QQQ", *([native.TREE] * 3)) + sentinel[24:]
        )
        cpu.q(agg + 0x348, native.TREE)
        cpu.q(agg + 0x350, 0)
        cpu.q(agg + 0x340, native.OUTER)
        cpu.q(native.OUTER + 0x3A0, native.H)
        if case != "queue_unobserved":
            # Controlled empty vector. Its equal pointers need not be null.
            cpu.q(agg + 0x360, native.TREE + 0x300)
            cpu.q(agg + 0x368, native.TREE + 0x300)
    else:
        vtable = {
            "type132": native.TYPE132,
            "type54_a": 0x3AA54C8,
            "type54_b": 0x3AA55D0,
        }[case]
        native.prepare_tree(cpu, [{"key": 0x500, "factor": 1.5, "vtable": vtable}])
    offsets = (0x88, 0x8C)
    for offset, dirty in zip(offsets, (5.5, 3.5), strict=True):
        cpu.f(agg + offset, dirty)

    def factors():
        return [struct.unpack("<f", cpu.raw(agg + off, 4))[0] for off in offsets]

    def step(rva):
        receipt = cpu.run(rva, rcx=agg)
        return {
            k: receipt[k] for k in ("completed", "steps", "missing", "trace_sha256")
        }

    reset = step(0xA66CC) if case != "skip_reset" else None
    after_reset = factors()
    if reset:
        require(
            reset["completed"] and after_reset == [1, 1],
            "Reset failed to restore both lanes",
        )
        require(
            {0xA67A3, 0xA67B1} <= {i for i, _ in cpu.current_trace},
            "Reset stores absent",
        )
    rebuild = step(0xA648C)
    expected = {"type132": [1.5, 1], "type54_a": [1.5, 1.5], "skip_reset": [5.5, 3.5]}
    if case == "queue_unobserved":
        require(
            not rebuild["completed"] and rebuild["missing"]["reader_rva"] == "0xa662f",
            "Missing queue did not stop at the native queue read",
        )
    else:
        require(
            rebuild["completed"] and factors() == expected.get(case, [1, 1]),
            "Wrong final lanes",
        )
    return {
        "case": case,
        "dirty": [5.5, 3.5],
        "reset": reset,
        "after_reset": after_reset,
        "rebuild": rebuild,
        "after_completed_rebuild": factors() if rebuild["completed"] else None,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("proofs", type=Path, help="PC202 deliverables directory")
    parser.add_argument("role", type=Path, help="verified Ninja read directory")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    vm = args.proofs / "r16-native-buffs-20261019/stacking"
    required = {
        "execute_stacking.py": "2f09f46a7bbf33f49948c921524c30ccad0e5da06d417333864a535ac4ac4177",
        "inspect_stack.py": "5ecea6fd25e89b3459cd52ac94a0c7b0ee2ffdb2fb39da8630d82fec35c03baf",
    }
    for name, expected in required.items():
        require(
            digest((vm / name).read_bytes()) == expected,
            "Unexpected replay tool: " + name,
        )
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(vm))
    import execute_stacking as native

    manifest, sections = native.load()
    text_rva, image = sections[".text"]
    code = {
        0x5FDEF9: "498d9dc02d0000",
        0x5FDF1B: "e850ebffff",
        0xA576A: "e85d0f0000",
        0xA5772: "e8150d0000",
        0xA67A3: "44898188000000",
        0xA67B1: "4489818c000000",
        0x78F1AE: "f30f108f482e0000",
        0x78F1BF: "f30f108f4c2e0000",
    }
    for rva, value in code.items():
        start = rva - text_rva
        require(
            image[start : start + len(value) // 2].hex() == value,
            f"Original instruction {rva:x}",
        )
    sentinel, role = examine(args.role)
    cases = (
        "empty_queue",
        "skip_reset",
        "queue_unobserved",
        "type132",
        "type54_a",
        "type54_b",
    )
    result = {
        "schema": "pc202-empty-status-factors/v1",
        "role": role,
        "native_sections": manifest["sections"],
        "native_instructions": {hex(rva): value for rva, value in code.items()},
        "cases": [run_probe(native, sections, sentinel, name) for name in cases],
        "scope": "Original reset/rebuild semantics. Empty queue and nonempty status objects are controls. "
        "The stored tree, D and mission membership do not measure the two final actor factors.",
    }
    output = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
