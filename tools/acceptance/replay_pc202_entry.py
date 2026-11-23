"""Replay saved game-thread entries with the pinned original PC202 MAIN interpreter."""

import argparse
import hashlib
import itertools
import json
from pathlib import Path
import struct
import sys


def immutable_cells(root):
    offline = root / "deliverables/martial-event-live-20261111/offline"

    def pinned(name, digest):
        raw = (offline / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError(f"Immutable resource proof changed: {name}")
        return json.loads(raw)

    proof = pinned(
        "IMMUTABLE_REPLAY.json",
        "906e22814a72611d05c1f33ebf4c8949791311ddc31cc1d5283075abfffac89d",
    )
    companion = pinned(
        "reaper-resources.request.json",
        "a0df7a72ce8b68fba5cb33b778dfb75a38168ce61f1f2b2fab6a8ba9dbae5243",
    )["scene"]
    # This proof contains file-verified resources and a replay-owned table
    # representation. Never import captured actor, mission, or target storage.
    allowed = {
        "owned_reconstruction_of_file_verified_immutable_resource",
        "owned_file_bound_static718",
        "owned_control_state",
    }
    cells = [item["cell"] for item in proof["fills"]]
    cells.extend(
        row
        for row in companion["memory_regions"]
        if row.get("provenance", {}).get("kind") in allowed
    )
    base = companion["module_base_u64"]
    ctor = root / "deliverables/optimizer-codex-takeover-20261114/tls-static-rerun"
    seeded = (ctor / "default_D_constructed.bin").read_bytes()
    mask = (ctor / "default_D_written_mask.bin").read_bytes()
    if (
        hashlib.sha256(seeded).hexdigest()
        != "92f873f75798a333f72325f353f68de79b7669a4e9e2bb1ff0e1a5b526792610"
        or hashlib.sha256(mask).hexdigest()
        != "559af51da34108697ffecf929d89793d15eaa91b2e0cabcaa493a765640768c0"
    ):
        raise ValueError("Original default D constructor evidence differs")
    for written, group in itertools.groupby(
        range(len(mask)), key=lambda i: mask[i] == 255
    ):
        offsets = list(group)
        if written:
            at = offsets[0]
            data = seeded[at : offsets[-1] + 1]
            cells.append(
                dict(
                    name=f"constructed_default_D_{at:x}",
                    address_u64=base + 0x4BA66B0 + at,
                    value_hex=data.hex(),
                    known_hex="ff" * len(data),
                    writable=False,
                    provenance={"kind": "original_constructor_known_writes"},
                )
            )
    return base, cells


def replay_entry(request, cache, replay, base, cells):
    scene = request["scene"]
    if scene["module_base_u64"] != base:
        raise ValueError("Relocated resource proof has another image base")
    used = []
    while True:
        result = replay._execute_request(request, cache)
        missing = result["last_missing_read"]
        if result["normal_MAIN_return"] or not missing:
            return result, used
        lo, width = missing["address_u64"], missing["width_bytes"]
        cell = next(
            (
                c
                for c in cells
                if c["address_u64"] <= lo
                and lo + width <= c["address_u64"] + len(c["value_hex"]) // 2
            ),
            None,
        )
        if cell is None or cell["name"] in used:
            return result, used
        start = cell["address_u64"]
        value = bytes.fromhex(cell["value_hex"])
        if cell["known_hex"].lower() != "ff" * len(value):
            raise ValueError("Incomplete static resource")
        for r in scene["memory_regions"]:
            origin = r["address_u64"]
            first, end = (
                max(origin, start),
                min(origin + len(r["value_hex"]) // 2, start + len(value)),
            )
            if end <= first:
                continue
            mask = bytes.fromhex(r["known_hex"])[first - origin : end - origin]
            old = bytes.fromhex(r["value_hex"])[first - origin : end - origin]
            if any(
                (a ^ b) & m
                for a, b, m in zip(
                    value[first - start : end - start], old, mask, strict=True
                )
            ):
                raise ValueError(
                    f"Static binding conflicts with captured bytes: {cell['name']}"
                )
        scene["memory_regions"].append(cell)
        used.append(cell["name"])


def empty_mission_map(scene):
    slot = scene["module_base_u64"] + 79330664
    pointers = set()
    for r in scene["memory_regions"]:
        offset = slot - r["address_u64"]
        if 0 <= offset and offset + 8 <= len(r["value_hex"]) // 2:
            if bytes.fromhex(r["known_hex"])[offset : offset + 8] == b"\xff" * 8:
                pointers.add(
                    struct.unpack_from("<Q", bytes.fromhex(r["value_hex"]), offset)[0]
                )
    if len(pointers) != 1:
        raise ValueError("The original mission owner was not captured unambiguously")
    header = pointers.pop() + 0x2820
    sentinel, buckets = 0x6004_0000_0000, 0x6004_0000_1000
    values = [
        (
            header + 8,
            struct.pack(
                "<QQQQQQQ", sentinel, 0, buckets, buckets + 128, buckets + 128, 7, 8
            ),
        ),
        (sentinel, struct.pack("<QQ", sentinel, sentinel)),
        (buckets, struct.pack("<16Q", *([sentinel] * 16))),
    ]
    return [
        dict(
            name=f"controlled_empty_mission_{i}",
            address_u64=a,
            value_hex=value.hex(),
            known_hex="ff" * len(value),
            writable=False,
            provenance={"kind": "controlled_empty_mission"},
        )
        for i, (a, value) in enumerate(values)
    ]


def run(root, capture, frames, output, empty_mission_control=False):
    sys.path.insert(
        0, str(root / "worktrees/research-damage/research/damage/evaluator/src")
    )
    from demand_fill_loop import REPLAY_DIR, install_getter_cut

    sys.path.insert(0, str(REPLAY_DIR))
    import replay
    import r12_bind

    binding = r12_bind.prepare(root / "cache")
    r12_bind.prepare = lambda _: binding
    install_getter_cut(replay)
    import main_forward
    from scene_from_capture import request_from_capture

    evidence = json.loads(capture.read_bytes())["raw_capture"]
    hits = {f["id"]: f for f in evidence["frames"]}
    shots = {s["sequence"]: s for s in evidence["snapshots"]}
    rows = []
    base, cells = immutable_cells(root)
    for frame in frames:
        request = request_from_capture(
            capture_path=capture,
            frame_id=frame,
            declared_machine_domain=main_forward.MACHINE,
        )
        bindings = cells + (
            empty_mission_map(request["scene"]) if empty_mission_control else []
        )
        result, used = replay_entry(request, root / "cache", replay, base, bindings)
        address = request["scene"]["main_address_u64"]
        outputs = next(
            r
            for r in shots[hits[frame]["final_sequence"]]["regions"]
            if r["role"] == "MAIN.final_outputs"
        )
        packet = bytes.fromhex(outputs["bytes_hex"])
        mask = bytes.fromhex(outputs["known_mask_hex"])
        observed, actual = [], []
        for off in (0x218, 0x224, 0x228):
            if mask[off : off + 4] != b"\xff" * 4:
                raise ValueError("Incomplete observed score")
            observed.append(struct.unpack_from("<f", packet, off)[0])
            writes = [
                v
                for v in result["original_store_journal"]
                if v["address_u64"] == address + off and v["width"] == 4
            ]
            after = writes[-1]["after"] if writes else None
            actual.append(
                struct.unpack("<f", bytes.fromhex(after["value_hex"]))[0]
                if after and after["known_hex"] == "ffffffff"
                else None
            )
        rows.append(
            dict(
                frame=frame,
                observed_components=observed,
                returned=result["normal_MAIN_return"],
                static_bindings=used,
                original_stores=actual,
                matched=bool(result["normal_MAIN_return"] and actual == observed),
                missing=result["last_missing_read"],
                dependency=result["native"].get("dependency"),
            )
        )
    report = dict(
        capture_sha256=hashlib.sha256(capture.read_bytes()).hexdigest(),
        process_reads=0,
        frames=rows,
        mission_map="controlled_empty" if empty_mission_control else "recorded_only",
    )
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("root", type=Path)
    p.add_argument("capture", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("frames", type=int, nargs="+")
    p.add_argument("--empty-mission-control", action="store_true")
    run(**vars(p.parse_args()))
