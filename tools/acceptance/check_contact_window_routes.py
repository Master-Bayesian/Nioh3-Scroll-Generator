"""Find PC202 contact windows with matching timing and motion but different row flags."""

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import struct
import sys


def native_route(text, flags, component):
    from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
    from unicorn.x86_const import UC_X86_REG_R13, UC_X86_REG_RBX

    start, passed, rejected = 0x117AB05, 0x117AB45, 0x117C6ED
    code = text[start - 0x1000 : passed - 0x1000]
    if (
        hashlib.sha256(code).hexdigest()
        != "8a7dd44e29cf83f10f4d282d99d04f29d31b415f36271c4bae12a349c703c0ef"
    ):
        raise ValueError("Collision predicate differs from PC202")
    vm = Uc(UC_ARCH_X86, UC_MODE_64)
    vm.mem_map(0x117A000, 0x3000)
    vm.mem_write(start, code)
    vm.mem_map(0x10000, 0x3000)
    vm.reg_write(UC_X86_REG_R13, 0x10000)
    vm.reg_write(UC_X86_REG_RBX, 0x11000)
    vm.mem_write(0x10048, struct.pack("<I", flags))
    if component is not None:
        vm.mem_write(0x110E8, struct.pack("<Q", 0x12000))
        vm.mem_write(0x12080, struct.pack("<i", component))
    exits = []

    def stop(machine, pc, _size, _data):
        if pc in (passed, rejected):
            exits.append(pc)
            machine.emu_stop()

    vm.hook_add(UC_HOOK_CODE, stop)
    vm.emu_start(start, 0, count=20)
    (exit_pc,) = exits
    return exit_pc == passed


def benchmark_component(directory):
    target = json.loads((directory / "reference-target.json").read_bytes())
    memory = json.loads((directory / "ordinary-read-memory.json").read_bytes())
    if any(target["process"][k] != memory[k] for k in ("pid", "creation_filetime")):
        raise ValueError("Opponent and memory belong to different process instances")

    def value(at, size):
        candidates = {
            bytes.fromhex(r["bytes"])[at - r["address"] : at - r["address"] + size]
            for r in memory["regions"]
            if r["address"] <= at and at + size <= r["address"] + len(r["bytes"]) // 2
        }
        (data,) = candidates
        return int.from_bytes(data, "little")

    actor = target["opponent"]["actor"]
    component = value(actor + 0xE8, 8)
    return dict(
        actor=actor,
        component=component,
        value_at_80=value(component + 0x80, 4) if component else None,
        scenario="The recorded training warrior is the effective recipient; no recipient redirect.",
    )


def run(hits, native_reader, output, text, benchmark):
    sys.path.insert(0, str(native_reader))
    from hit_history import history_expiry
    from hit_identity import collision_group

    raw = hits.read_bytes()
    action_hashes = {
        r["key"]: r["parameter_sha256"]
        for line in hits.with_name("actions.jsonl").read_bytes().splitlines()
        if (r := json.loads(line))
    }
    groups = defaultdict(list)
    scanned = 0
    for line in raw.splitlines():
        hit = json.loads(line)
        row = bytes.fromhex(hit["raw_hex"])
        scanned += 1
        if len(row) != 192 or hashlib.sha256(row).hexdigest() != hit["row_sha256"]:
            raise ValueError("Original damage row identity differs")
        physical, elemental = struct.unpack_from("<hh", row, 0x10)
        if not hit["resource_key"] or not (physical or elemental):
            continue
        history = history_expiry(row)
        key = (
            hit["action_key"],
            hit["start_time"],
            hit["end_time_helper"],
            hit["repeat_interval"],
            collision_group(row),
            physical,
            elemental,
            row[0x0E],
            bool(row[0x47] & 4),
        )
        (flags,) = struct.unpack_from("<H", row, 0x48)
        groups[key].append(
            dict(
                resource=hit["resource_key"],
                action=hit["action_id"],
                row=hit["row_order"],
                flags=flags,
                weapon=hit["weapon_label"],
                style=hit["style"],
                history=history.kind.value,
                digest=hit["row_sha256"],
            )
        )
    pairs = []
    for key, rows in groups.items():
        if {r["flags"] for r in rows} >= {0x0102, 0x0082}:
            pairs.append(
                dict(
                    action_key=key[0],
                    parameter_sha256=action_hashes[key[0]],
                    start=key[1],
                    end=key[2],
                    interval=key[3],
                    group=key[4],
                    physical_motion=key[5],
                    elemental_motion=key[6],
                    rows=rows,
                )
            )
    if not pairs:
        raise ValueError("Expected paired native contact windows")
    by_type = Counter((p["rows"][0]["style"], p["rows"][0]["weapon"]) for p in pairs)
    machine = text.read_bytes()
    target = benchmark_component(benchmark)
    flags_set = sorted({r["flags"] for p in pairs for r in p["rows"]})
    controls = [
        dict(flags=flags, component=c, continues=native_route(machine, flags, c))
        for flags in flags_set
        for c in (0, 1, 2, -1, None)
    ]
    summary = dict(
        scanned_rows=scanned,
        paired_windows=len(pairs),
        actions=len({p["action_key"] for p in pairs}),
        form_weapon_pairs=[
            dict(style=s, weapon=w, windows=n) for (s, w), n in sorted(by_type.items())
        ],
    )
    output.write_text(
        json.dumps(
            dict(
                summary=summary,
                source_sha256=hashlib.sha256(raw).hexdigest(),
                interpretation="Original 117AB05..117AB45 filters effective recipients. Geometry and target hit history still determine landings.",
                native_controls=controls,
                benchmark=target,
                windows=pairs,
            ),
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf8",
    )
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("hits", "native_reader", "output", "text", "benchmark"):
        parser.add_argument(arg, type=Path)
    run(**vars(parser.parse_args()))
