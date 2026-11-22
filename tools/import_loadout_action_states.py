"""Build the PC202 action-controller inputs from the resource directory and existing damage-row bank."""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


def build(resource_root, native_reader, catalog, damage_bank, output, receipt):
    sys.path.insert(0, str(native_reader))
    from action_extensions import action_file_records
    from action_resource import ActionResource

    sha = hashlib.sha256
    bank = damage_bank.read_bytes()
    magic, num_keys, num_rows = struct.unpack_from("<8sII", bank)
    if magic != b"N3AAD001" or len(bank) != 16 + num_keys * 36 + num_rows * 192:
        raise ValueError("Invalid damage-row bank")
    row_keys = dict(struct.iter_unpack("<32sI", bank[16 : 16 + num_keys * 36]))
    bodies = memoryview(bank)[16 + num_keys * 36 :]
    selections = json.loads(catalog.read_bytes())["selections"]
    expected = {(r["resource"], r["action"], r["parameter_sha256"]) for r in selections}
    manifest = json.loads(
        (resource_root / "WEAPON_RESOURCE_MANIFEST.json").read_bytes()
    )["resources"]
    bindings = [(resource, records[0]) for resource, records in manifest.items()]
    common = json.loads((resource_root / "COMMON_RESOURCE_BINDING.json").read_bytes())
    bindings.extend((f"common:{r['category']}:{r['sha256']}", r) for r in common)
    states, params, references = {}, {}, []
    seen, by_resource = set(), {}
    for key, binding in bindings:
        raw = (
            resource_root / "matched-resources" / Path(binding["path"]).name
        ).read_bytes()
        if sha(raw).hexdigest() != binding["sha256"]:
            raise ValueError(f"Resource digest differs for {key}")
        resource = ActionResource.load(key, raw)
        rows = resource.records(2, 192)
        with_rows = 0
        first_index = 0  # PC202 49BD28 indexes the resource's concatenated row-reference lists.
        for parameter, action in zip(
            resource.records(1, 336), action_file_records(resource), strict=True
        ):
            number = struct.unpack_from("<H", parameter, 12)[0]
            digest = sha(parameter).digest()
            identity = (key, number, digest.hex())
            if identity in seen or identity not in expected:
                raise ValueError(f"Ambiguous or unexpected action {identity}")
            seen.add(identity)
            prefix = key.encode() + b"\0" + struct.pack("<H", number)
            uid = sha(prefix + digest).digest()
            start = len(references)
            for local, native in enumerate(action.damage_indices):
                row_key = sha(prefix + struct.pack("<I", local) + digest).digest()
                slot = row_keys[row_key]
                if rows[native] != bodies[slot * 192 : (slot + 1) * 192]:
                    raise ValueError(
                        f"Controller and damage-row bank disagree for {identity} row {local}"
                    )
                references.append(slot)
            indices = action.damage_indices
            with_rows += bool(indices)
            states[uid] = (
                params.setdefault(parameter, len(params)),
                start,
                len(indices),
                first_index,
            )
            first_index += len(indices)
        by_resource[key] = {
            "parameters": len(resource.records(1, 336)),
            "with_direct_rows": with_rows,
        }
    if seen != expected or len(states) != len(expected) or len(references) != num_keys:
        raise ValueError("Catalog coverage or damage-row reference count differs")
    out = bytearray(
        struct.pack("<8sIII", b"N3ACT001", len(states), len(params), len(references))
    )
    for key, vals in sorted(states.items()):
        out.extend(struct.pack("<32sIIHH", key, *vals))
    out.extend(b"".join(params))
    out.extend(b"".join(struct.pack("<I", ref) for ref in references))
    output.write_bytes(out)
    receipt.write_text(
        json.dumps(
            dict(
                actions=len(states),
                parameters=len(params),
                row_references=len(references),
                distinct_rows=num_rows,
                bytes=len(out),
                sha256=sha(out).hexdigest(),
                resources=by_resource,
                scope="Original controller parameter and local row order. Contact production and landed multiplicity are separate.",
            ),
            indent=2,
        ),
        encoding="utf8",
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in (
        "resource_root",
        "native_reader",
        "catalog",
        "damage_bank",
        "output",
        "receipt",
    ):
        parser.add_argument(arg, type=Path)
    build(**vars(parser.parse_args()))
