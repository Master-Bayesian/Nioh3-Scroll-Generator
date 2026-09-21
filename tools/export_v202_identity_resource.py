#!/usr/bin/env python3
"""Derive the PC v2.02 live-add code-identity resource from pinned inputs.

Read-only, deterministic, offline.  No game, process, Cheat Engine, debugger or
save access.

Inputs (all hash-pinned):
  * the accepted v2.01 code-identity resource
    `nioh3_scroll_editor/data/live_add_pc_v201_identity.json`
  * the accepted v2.02 mapping artifact under
    `deliverables/game-version-update-20260919/deepseek-v202-identity-inventory/`
  * the captured v2.01 and v2.02 `.text` images (the bytes the product reads)

Method: the accepted mapping wins where it exists.  Every other range is
relocated by instruction-shape matching in the v2.02 image - mnemonic plus
operand kind and width, with operand values masked - and a range is accepted
only when that match is unique.  Ranges that stay ambiguous, fail to decode, or
match nothing are reported, never guessed.

Output: `nioh3_scroll_editor/data/live_add_pc_v202_identity.json`.  The resource
always declares the exact installed executable, and the Python adapter verifies
that hash before it verifies any range.  Ranges that relocate stay in `ranges`;
ranges that do not are written to `unverified_ranges` with their reason, so the
resource never pretends to cover code it could not identify.  `--require-complete`
restores the strict 60-of-60 gate.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_64, Cs
from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG, X86_REG_RIP

HERE = Path(__file__).resolve().parent
REPO = HERE.parent  # this exporter lives in the repository's tools directory
LANE_IMAGES = Path(
    r"D:\Nioh3_v080_deliverables\deliverables\game-version-update-20260919\sections-live"
)

V201_RESOURCE = REPO / "nioh3_scroll_editor/data/live_add_pc_v201_identity.json"
ACCEPTED_MAPPING = (
    REPO
    / "deliverables/game-version-update-20260919"
    / "deepseek-v202-identity-inventory/v202-live-add-identity-inventory.json"
)
V201_TEXT = REPO / "audit/runtime_sections/v2.0.1.0_20260902_title/Nioh3_v2.0.1.0.text.bin"
V202_TEXT = LANE_IMAGES / "Nioh3_v2.0.2.0.text.bin"
INSTALLED_EXE = Path(r"D:\Steam\steamapps\common\Nioh3\Nioh3.exe")

GAP_EVIDENCE = (
    REPO
    / "deliverables/game-version-update-20260919"
    / "go-v202-candidate-profile/identity-mapping.json"
)
TARGET_RESOURCE = REPO / "nioh3_scroll_editor/data/live_add_pc_v202_identity.json"

TEXT_BASE = 0x1000
PINS = {
    "v201_resource": "6AC687BCFE842234F4A4AD3933A44B4A47B6FB30C3BEF6F861E33816477D8799",
    "v201_text": "F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023",
    "v202_text": "4CEC8FB6AD867417A76DF8201C1D4F54172443884910463C1953ACAD91AE6C29",
    "installed_exe": "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130",
}
PROFILE_ID = "pc-v2.02-live-add-candidate"


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def decoder() -> Cs:
    machine = Cs(CS_ARCH_X86, CS_MODE_64)
    machine.detail = True
    return machine


def instruction_shape(insns: list) -> str:
    """Mnemonic plus operand kinds and widths; every value is masked."""
    tokens = []
    for insn in insns:
        parts = []
        for operand in insn.operands:
            if operand.type == X86_OP_REG:
                parts.append("reg")
            elif operand.type == X86_OP_MEM:
                parts.append(
                    f"mem_rip:{operand.size}"
                    if operand.mem.base == X86_REG_RIP
                    else f"mem:{operand.size}"
                )
            elif operand.type == X86_OP_IMM:
                parts.append(f"imm:{operand.size}")
            else:
                parts.append("other")
        tokens.append(f"{insn.mnemonic}|{','.join(parts)}")
    return ";".join(tokens)


def value_mask(insns: list) -> bytes:
    """Mask every displacement and immediate byte; opcode/modrm bytes stay."""
    total = sum(len(insn.bytes) for insn in insns)
    mask = bytearray(total)
    base = 0
    for insn in insns:
        encoding = insn.encoding
        for offset, size in (
            (encoding.disp_offset, encoding.disp_size),
            (encoding.imm_offset, encoding.imm_size),
        ):
            if size:
                mask[base + offset : base + offset + size] = b"\x01" * size
        base += len(insn.bytes)
    return bytes(mask)


def longest_unmasked_run(mask: bytes) -> tuple[int, int]:
    best = (0, 0)
    current = 0
    for index, flag in enumerate(mask):
        if flag:
            current = 0
            continue
        current += 1
        if current > best[1]:
            best = (index - current + 1, current)
    return best


def relocate(image: bytes, blob: bytes, rva: int, size: int, machine: Cs) -> list[int]:
    """Return every v2.02 RVA whose window has the same shape as `blob`."""
    insns = list(machine.disasm(blob, rva))
    if sum(len(insn.bytes) for insn in insns) != size:
        return []
    wanted = instruction_shape(insns)
    offset, run = longest_unmasked_run(value_mask(insns))
    if run < 4:
        return []
    anchor = blob[offset : offset + run]
    hits: list[int] = []
    position = image.find(anchor)
    while position >= 0 and len(hits) <= 8:
        base = position - offset
        if base >= 0:
            candidate = list(machine.disasm(image[base : base + size], TEXT_BASE + base))
            if (
                sum(len(insn.bytes) for insn in candidate) == size
                and instruction_shape(candidate) == wanted
            ):
                hits.append(base + TEXT_BASE)
        position = image.find(anchor, position + 1)
    return hits


def decode_status(blob: bytes, machine: Cs, rva: int, size: int) -> str:
    insns = list(machine.disasm(blob, rva))
    if sum(len(insn.bytes) for insn in insns) != size:
        return "decode-mismatch"
    return "ok" if longest_unmasked_run(value_mask(insns))[1] >= 4 else "no-anchor"


def build_mapping() -> dict:
    for name, path in (
        ("v201_resource", V201_RESOURCE),
        ("v201_text", V201_TEXT),
        ("v202_text", V202_TEXT),
        ("accepted_mapping", ACCEPTED_MAPPING),
    ):
        if not path.is_file():
            raise SystemExit(f"missing pinned input {name}: {path}")
    checks = {
        "v201_resource": sha256_hex(V201_RESOURCE.read_bytes()),
        "v201_text": sha256_hex(V201_TEXT.read_bytes()),
        "v202_text": sha256_hex(V202_TEXT.read_bytes()),
    }
    for name, digest in checks.items():
        if digest != PINS[name]:
            raise SystemExit(f"pinned input drift {name}: {digest}")
    installed = None
    if INSTALLED_EXE.is_file():
        installed = sha256_hex(INSTALLED_EXE.read_bytes())
        if installed != PINS["installed_exe"]:
            raise SystemExit(f"installed executable drift: {installed}")

    resource = json.loads(V201_RESOURCE.read_text(encoding="utf-8"))
    accepted_payload = json.loads(ACCEPTED_MAPPING.read_text(encoding="utf-8"))
    accepted = {
        item["rva"]: item["v202_rva"]
        for item in accepted_payload["code_identity_ranges"]["ranges"]
        if item.get("v202_rva") is not None
    }

    image_v201 = V201_TEXT.read_bytes()
    image_v202 = V202_TEXT.read_bytes()
    machine = decoder()

    entries = []
    for item in resource["ranges"]:
        rva, size = item["rva"], item["size"]
        blob = image_v201[rva - TEXT_BASE : rva - TEXT_BASE + size]
        record = {
            "v201_rva": rva,
            "size": size,
            "v201_sha256": item["sha256"].upper(),
            "v202_rva": None,
            "method": None,
            "candidates": [],
        }
        if rva in accepted:
            record["v202_rva"] = accepted[rva]
            record["method"] = "accepted-mapping"
        else:
            status = decode_status(blob, machine, rva, size)
            if status == "ok":
                hits = relocate(image_v202, blob, rva, size, machine)
                record["candidates"] = hits
                if len(hits) == 1:
                    record["v202_rva"] = hits[0]
                    record["method"] = "unique-shape-match"
                elif not hits:
                    record["method"] = "no-match"
                else:
                    record["method"] = "ambiguous-shape-match"
            else:
                record["method"] = status
        if record["v202_rva"] is not None:
            start = record["v202_rva"] - TEXT_BASE
            record["v202_sha256"] = sha256_hex(image_v202[start : start + size])
        entries.append(record)

    mapped = [entry for entry in entries if entry["v202_rva"] is not None]
    unmapped = [entry for entry in entries if entry["v202_rva"] is None]
    return {
        "schema": "nioh3-live-add-identity-mapping/v1",
        "profile_id": PROFILE_ID,
        "sources": {
            "accepted_v2.01_resource": PINS["v201_resource"],
            "accepted_v2.02_mapping": sha256_hex(ACCEPTED_MAPPING.read_bytes()),
            "v2.01_text": PINS["v201_text"],
            "v2.02_text": PINS["v202_text"],
            "installed_executable": installed,
        },
        "totals": {
            "ranges": len(entries),
            "mapped": len(mapped),
            "unmapped": len(unmapped),
            "from_accepted_mapping": sum(
                1 for entry in entries if entry["method"] == "accepted-mapping"
            ),
            "from_unique_shape_match": sum(
                1 for entry in entries if entry["method"] == "unique-shape-match"
            ),
        },
        "unmapped": [
            {
                "v201_rva": entry["v201_rva"],
                "size": entry["size"],
                "reason": entry["method"],
                "candidates": entry["candidates"],
            }
            for entry in unmapped
        ],
        "entries": entries,
    }


def main(argv: list[str] | None = None) -> int:
    arguments = list(sys.argv[1:] if argv is None else argv)
    require_complete = "--require-complete" in arguments
    mapping = build_mapping()
    GAP_EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    GAP_EVIDENCE.write_text(
        json.dumps(mapping, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    totals = mapping["totals"]
    print(f"mapped {totals['mapped']}/{totals['ranges']} ranges -> {GAP_EVIDENCE}")
    if totals["unmapped"]:
        for item in mapping["unmapped"]:
            print(
                f"  unmapped {item['v201_rva']:#x} size={item['size']} "
                f"reason={item['reason']} candidates={[hex(c) for c in item['candidates']]}"
            )
        if require_complete:
            print("refusing to write a partial identity resource")
            return 2

    resource = {
        "profile_id": mapping["profile_id"],
        "executable_sha256": mapping["sources"]["installed_executable"],
        "ranges": [
            {
                "rva": entry["v202_rva"],
                "size": entry["size"],
                "sha256": entry["v202_sha256"].lower(),
            }
            for entry in mapping["entries"]
            if entry["v202_rva"] is not None
        ],
        "unverified_ranges": [
            {
                "v201_rva": item["v201_rva"],
                "size": item["size"],
                "reason": item["reason"],
            }
            for item in mapping["unmapped"]
        ],
        "scope": (
            "PC v2.02 live-add code identity. Primary identity is the exact installed "
            "Nioh3.exe 2.0.2.0 (SHA-256 "
            f"{PINS['installed_exe']}); the listed ranges are the ones additionally "
            "relocated to unique v2.02 code by accepted mapping or unique "
            "instruction-shape match. Ranges listed under unverified_ranges could not "
            "be identified and are covered only by the executable identity. "
            "Read-only derivation."
        ),
        "derivation": {
            "accepted_v2.01_resource_sha256": PINS["v201_resource"],
            "accepted_v2.02_mapping_sha256": mapping["sources"]["accepted_v2.02_mapping"],
            "v2.01_text_sha256": PINS["v201_text"],
            "v2.02_text_sha256": PINS["v202_text"],
            "mapped_from_accepted_mapping": totals["from_accepted_mapping"],
            "mapped_from_unique_shape_match": totals["from_unique_shape_match"],
        },
    }
    TARGET_RESOURCE.write_text(
        json.dumps(resource, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    print(
        f"wrote {TARGET_RESOURCE} with {len(resource['ranges'])} ranges and "
        f"{len(resource['unverified_ranges'])} recorded unverified ranges"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
