"""Add the PC202 held-item branch to a translated MAIN generation input."""

import argparse
import hashlib
import json
from pathlib import Path
import struct

from capstone import CS_ARCH_X86, CS_MODE_64, Cs
from capstone.x86 import X86_OP_IMM


def extend(listing, image, scene, text, record, output):
    code = text.read_bytes()
    if (
        hashlib.sha256(code).hexdigest()
        != "4cec8fb6ad867417a76df8201c1d4f54172443884910463c1953acad91ae6c29"
    ):
        raise ValueError("Unexpected PC202 text identity")
    output.mkdir(parents=True, exist_ok=True)
    doc = json.loads(listing.read_bytes())
    raw, known = (
        bytearray(image.read_bytes()),
        bytearray(image.with_name("image_known.bin").read_bytes()),
    )
    slots = {
        struct.unpack_from("<Q", raw, i)[0]: i // 65544
        for i in range(0, len(raw), 65544)
    }
    rows = {row["rva"]: row for row in doc["instructions"]}
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    md.detail = True
    # Full basic blocks and both returns of the item flag helper. The curve evaluator
    # and float accumulation callees are already in the canonical listing.
    for lo, hi in [(0x552644, 0x552654), (0x2081C50, 0x2081C8A), (0x791A6D, 0x791AA0)]:
        end = lo
        for ins in md.disasm(code[lo - 4096 : hi - 4096], lo):
            rva = ins.address
            end = rva + ins.size
            for i, value in enumerate(ins.bytes):
                address = doc["module"] + rva + i
                page, offset = address & ~65535, address & 65535
                if page not in slots:
                    slots[page] = len(slots)
                    raw.extend(struct.pack("<Q", page) + bytes(65536))
                    known.extend(struct.pack("<Q", page) + bytes(8192))
                index = slots[page]
                pos, mask = index * 65544 + 8 + offset, index * 8200 + 8 + offset // 8
                if known[mask] & (1 << (offset % 8)) and raw[pos] != value:
                    raise ValueError(f"Original code mismatch at {rva + i:#x}")
                raw[pos] = value
                known[mask] |= 1 << (offset % 8)
            if rva not in rows:
                rows[rva] = dict(
                    rva=rva,
                    size=ins.size,
                    bytes=ins.bytes.hex(),
                    mnemonic=ins.mnemonic,
                    op_str=ins.op_str,
                    executed=0,
                    targets=[],
                )
            if ins.mnemonic == "ret":
                continue
            targets = [end]
            if ins.mnemonic == "call" or ins.mnemonic.startswith("j"):
                assert ins.operands[0].type == X86_OP_IMM
                targets = [ins.operands[0].imm] + (
                    [] if ins.mnemonic in ("call", "jmp") else [end]
                )
            rows[rva]["targets"] = sorted(set(targets) | set(rows[rva]["targets"]))
        if end != hi:
            raise ValueError("Incomplete decode")
    doc["instructions"] = [rows[k] for k in sorted(rows)]
    doc["bails"] = sorted(set(doc["bails"]) - {0x791A70, 0x791A79})
    doc["knowledge_sha256"] = hashlib.sha256(known).hexdigest()
    (output / "image.bin").write_bytes(raw)
    (output / "image_known.bin").write_bytes(known)
    (output / "listing.json").write_text(
        json.dumps(doc, separators=(",", ":")), encoding="utf8"
    )
    (output / "edges.txt").write_bytes(listing.with_name("edges.txt").read_bytes())
    lines = scene.read_text().splitlines()
    for offset in (0, 0x14, 0x18):
        lines.append(f"domain {record + offset:x} {record + offset + 4:x}")
    (output / "scene.txt").write_text("\n".join(lines) + "\n", encoding="utf8")
    (output / "inputs.json").write_text(
        json.dumps(
            dict(listing=str(output / "listing.json"), image=str(output / "image.bin"))
        ),
        encoding="utf8",
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("listing", "image", "scene", "text", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("record", type=lambda value: int(value, 0))
    extend(**vars(parser.parse_args()))
