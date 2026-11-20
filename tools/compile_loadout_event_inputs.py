"""Materialize the attack-owned part of an explicit PC202 event sequence."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def compile_descriptors(plan, resolver, address):
    """Use the descriptor address read from the scene's actual MAIN packet."""
    bindings = plan["provenance"]["row_bindings"]
    if len(bindings) != len(plan["templates"]):
        raise ValueError("Missing template row binding")
    refs = []
    for i, (binding, template) in enumerate(zip(bindings, plan["templates"])):
        candidates = binding["exact_action_matches"]
        if binding["template"] != i or binding["aad_address"] != address or not candidates:
            raise ValueError("Damage descriptor does not belong to this contact")
        named = template.get("identity")
        if named and not any(all(r[k] == named[k] for k in ("resource", "action", "parameter_sha256"))
                             for r in candidates):
            raise ValueError("Template action differs from its source identity")
        refs.extend(dict(r, row_sha256=binding["row_sha256"]) for r in candidates)
    # Resolve every ambiguous identity. Agreement on bytes authorizes their
    # common damage descriptor, without attributing the event to one of them.
    result = subprocess.run([str(resolver)], input=json.dumps(refs).encode(),
                            capture_output=True, check=True)
    compiled = json.loads(result.stdout)
    seed = bytes(compiled["initial_bytes"])
    offsets = compiled["word_offsets"]
    rows = []
    cursor = 0
    for binding in bindings:
        for _ in binding["exact_action_matches"]:
            row = bytearray(seed)
            for at, word in zip(offsets, compiled["template_words"][cursor], strict=True):
                struct.pack_into("<I", row, at, word)
            if hashlib.sha256(row).hexdigest() != binding["row_sha256"]:
                raise ValueError("Resolved parameter does not match the event")
            cursor += 1
        rows.append(bytes(row))
    return rows


def bind_hit_inputs(raw, address, descriptors):
    """Preserve the evaluator's write domain, order and actor/phase patches."""
    lines = raw.decode("ascii").splitlines()
    addresses = [int(value, 16) for value in lines[1].split()]
    rows = [[int(value, 16) for value in line.split()] for line in lines[2:] if line.strip()]
    if int(lines[0]) != len(rows) or len(rows) != len(descriptors):
        raise ValueError("Contact count differs from the event plan")
    for i in range(0, 192, 4):
        if address + i not in addresses and any(row[i:i+4] != descriptors[0][i:i+4] for row in descriptors):
            raise ValueError(f"Changing damage field {i:#x} has no write in this kernel's input layout")
    for row, descriptor in zip(rows, descriptors):
        for j, (a, _) in enumerate(zip(addresses, row, strict=True)):
            if address <= a < address + 192:
                if (a - address) % 4:
                    raise ValueError("Unaligned descriptor patch")
                row[j], = struct.unpack_from("<I", descriptor, a - address)
    return ("\n".join([str(len(rows)), lines[1], *(" ".join(f"{word:x}" for word in row)
                                                  for row in rows)]) + "\n").encode("ascii")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scene", type=Path)
    parser.add_argument("resolver", type=Path, help="Built compile_action_rows executable")
    parser.add_argument("address", type=lambda x: int(x, 0), help="Canonical descriptor address for this kernel")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    plan = json.loads((args.scene / "event_plan.json").read_bytes())
    descriptors = compile_descriptors(plan, args.resolver, args.address)
    hits = bind_hit_inputs((args.scene / "hits.txt").read_bytes(), args.address, descriptors)
    plan["hit_values_sha256"] = hashlib.sha256(hits).hexdigest()
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "damage_descriptor.bin").write_bytes(descriptors[0])
    (args.output / "hits.txt").write_bytes(hits)
    (args.output / "event_plan.json").write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
