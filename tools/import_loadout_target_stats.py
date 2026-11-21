"""Import fixed opponent numeric STAT fields from original images with complete byte masks."""

import argparse
import json
import re
import struct
from pathlib import Path


def build(schemas, replay_root, reset, output, opponent=()):
    block = (
        reset.read_text(encoding="utf8").split("const STAT:", 1)[1].split("];", 1)[0]
    )
    spans = [
        (int(a, 16), int(b, 16))
        for a, b in re.findall(r"\(0x([0-9a-f]+), 0x([0-9a-f]+), 0x[0-9a-f]+\)", block)
    ]
    if not spans:
        raise ValueError("Missing native STAT reset spans")
    targets = {}
    pairs = [
        (schemas / (f.name + ".json"), f / "context")
        for f in sorted(replay_root.iterdir())
        if (schemas / (f.name + ".json")).is_file()
        and (f / "context/image_known.bin").is_file()
    ]
    for schema, ctx in [*pairs, *opponent]:
        roots = json.loads(schema.read_bytes())["roots"]
        address = roots["target_stat"]
        image = (ctx / "image.bin").read_bytes()
        known = (ctx / "image_known.bin").read_bytes()
        pages = {
            struct.unpack_from("<Q", image, i)[0]: i // 65544
            for i in range(0, len(image), 65544)
        }
        payload = bytearray(0x900)
        for a, b in spans:
            for off in range(a, b):
                absolute = address + off
                i = absolute % 65536
                idx = pages[absolute & ~65535]
                if not known[idx * 8200 + 8 + i // 8] & (1 << (i % 8)):
                    raise ValueError(
                        f"Unknown original opponent field in {ctx} at {off:#x}"
                    )
                payload[off] = image[idx * 65544 + 8 + i]
        # A14F78 and A18F4C index the fixed opponent's condition arrays.
        owner_words = bytearray(32)
        present = 0
        for index, offset in enumerate(
            (0x2EBC, 0x2EC0, 0x2EC4, 0x2F14, 0x2FA4, 0x2FA8, 0x2FAC, 0x2FFC)
        ):
            at = roots["target_owner"] + offset
            page = pages.get(at & ~65535)
            within = at % 65536
            if page is not None and all(
                known[page * 8200 + 8 + i // 8] & (1 << (i % 8))
                for i in range(within, within + 4)
            ):
                start = page * 65544 + 8 + within
                owner_words[index * 4 : index * 4 + 4] = image[start : start + 4]
                present |= 1 << index
        payload.extend(bytes([present]) + owner_words)
        if targets.setdefault(address, payload) != payload:
            raise ValueError(
                f"Distinct opponents share address {address:#x}; qualify the benchmark identity"
            )
    if not targets:
        raise ValueError("No fixed opponents")
    output.write_bytes(
        b"".join(struct.pack("<Q", a) + v for a, v in sorted(targets.items()))
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("schemas", "replay_root", "reset", "output"):
        parser.add_argument(key, type=Path)
    parser.add_argument(
        "--opponent",
        nargs=2,
        type=Path,
        action="append",
        default=[],
        metavar=("SCHEMA", "CAPTURE"),
    )
    build(**vars(parser.parse_args()))
