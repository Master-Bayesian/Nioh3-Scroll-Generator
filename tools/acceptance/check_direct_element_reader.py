"""Replay the normal first reader with missing, inactive and active PC202 owner elements."""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def read(path):
    return json.loads(path.read_bytes())


def word(image, mask, address):
    for at in range(0, len(image), 65544):
        page = struct.unpack_from("<Q", image, at)[0]
        offset = address - page
        if 0 <= offset <= 65532:
            masks = at // 65544 * 8200
            if struct.unpack_from("<Q", mask, masks)[0] != page:
                raise ValueError("Page and known-byte identities differ")
            if all(
                mask[masks + 8 + i // 8] & (1 << (i % 8))
                for i in range(offset, offset + 4)
            ):
                return struct.unpack_from("<I", image, at + 8 + offset)[0]
    return None


def run(reader, data, scene, save, role, base, output):
    snapshot, memory = read(role / "snapshot.json"), read(base)
    if any(memory[k] != v for k, v in snapshot["process"].items()):
        raise ValueError(
            "Snapshot and controlled base have different process identities"
        )
    owner = snapshot["roots"]["owner"]
    addresses = [owner + x for x in (0x2E9C, 0x2EA4)]
    normalized = read(data / "loadout" / scene / "event_plan.json")
    if not normalized["direct_owner_element"]:
        raise ValueError("The scene does not request current owner element acquisition")
    for region in memory["regions"]:
        start, raw = region["address"], region["bytes"]
        if any(start < end + 4 and end < start + len(raw) // 2 for end in addresses):
            raise ValueError(
                "Reference already contains the value that this control varies"
            )
    output.mkdir(parents=True, exist_ok=False)
    results = []
    for name, selected, power, ready, failed_read in (
        ("unknown", None, None, False, 0),
        ("inactive", 0xFFFFFFFF, None, True, None),
        ("missing-power", 0, None, False, 1),
        ("element-zero", 0, 101, True, None),
    ):
        case = output / name
        case.mkdir()
        extra = [
            {"address": addr, "bytes": struct.pack("<I", value).hex()}
            for addr, value in zip(addresses, (selected, power), strict=True)
            if value is not None
        ]
        fixture = case / "memory.json"
        fixture.write_text(
            json.dumps(
                dict(
                    memory,
                    regions=sorted(
                        [*memory["regions"], *extra], key=lambda r: r["address"]
                    ),
                ),
                separators=(",", ":"),
            ),
            encoding="utf-8",
        )
        dest = case / "observed"
        proc = subprocess.run(
            [reader, data, save, dest, "--scene", scene, "--fixture", fixture],
            capture_output=True,
            text=True,
        )
        (case / "reader.log").write_text(proc.stdout + proc.stderr, encoding="utf-8")
        result = read(dest / "receipt.json")
        if result["ready"] != ready or result["game_writes"] or result["save_writes"]:
            raise ValueError(f"{name}: {result}")
        if ready:
            if proc.returncode or not result["replay_saved"]:
                raise ValueError(f"{name}: ordinary replay failed")
            ctx = dest / "bundle/context"
            image, mask = (
                (ctx / f).read_bytes() for f in ("image.bin", "image_known.bin")
            )
            private = normalized["source"]["owner"]
            actual = [word(image, mask, private + off) for off in (0x2E9C, 0x2EA4)]
            if actual[0] != selected or (power is not None and actual[1] != power):
                raise ValueError(f"{name}: wrong owner input {actual!r}")
        else:
            actual = None
            message = " ".join(
                (str(result.get("error")), proc.stdout, proc.stderr)
            ).lower()
            if f"{addresses[failed_read]:x}" not in message:
                raise ValueError(f"{name}: unexpected refusal: {message}")
        results.append(
            {
                "name": name,
                "selected": selected,
                "power": power,
                "ready": ready,
                "known_private_words": actual,
                "reader_receipt": result,
            }
        )
    summary = {
        "schema": "pc202-direct-owner-element-first-read/v1",
        "scene": scene,
        "base_sha256": hashlib.sha256(base.read_bytes()).hexdigest(),
        "cases": results,
        "scope": "Ordinary production reader over the retained role with explicit synthetic input variants.",
    }
    (output / "READBACK.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(summary))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("reader", "data", "scene", "save", "role", "base", "output"):
        parser.add_argument(arg, type=str if arg == "scene" else Path)
    run(**vars(parser.parse_args()))
