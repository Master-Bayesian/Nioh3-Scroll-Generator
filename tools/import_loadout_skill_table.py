"""Bind the PC202 skill rows and immutable native lookup index used by MAIN."""

import argparse
import hashlib
import json
from pathlib import Path


def import_table(proof, archive, output):
    raw = proof.read_bytes()
    if (
        hashlib.sha256(raw).hexdigest()
        != "97c5850f1be55850397de8a99de0dc10226fb8975a2f9a9b8a4547da53fde57f"
    ):
        raise ValueError("PC202 static skill table evidence differs")
    regions = {
        r["name"]: bytes.fromhex(r["value_hex"]) for r in json.loads(raw)["regions"]
    }
    rows = (archive / "00294.bin").read_bytes()
    if rows != regions["complete_skill_rows"] or len(rows) != 211400:
        raise ValueError("Native skill table differs from resource 00294")
    index = regions["complete_skill_hash"]
    if len(index) != 17624:
        raise ValueError("Invalid native skill index size")
    variants = (archive / "00346.bin").read_bytes()
    if variants != regions["complete_variant_rows"] or len(variants) != 3068:
        raise ValueError("Native skill variant table differs from resource 00346")
    indexed = proof.with_name("CURRENT_STATIC_PARAMS.json").read_bytes()
    if (
        hashlib.sha256(indexed).hexdigest()
        != "6ac12862c40092b2335426bf2fae397d2de595abfb899d4536a7d524973e6e0b"
    ):
        raise ValueError("PC202 variant index evidence differs")
    regions = {
        r["name"]: bytes.fromhex(r["value_hex"]) for r in json.loads(indexed)["regions"]
    }
    variant_index = regions["complete_variant_hash"]
    if variants != regions["complete_variant_rows"] or len(variant_index) != 728:
        raise ValueError("Variant index does not belong to the verified rows")
    output.write_bytes(rows + index + variants + variant_index)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("proof", "archive", "output"):
        parser.add_argument(arg, type=Path)
    import_table(**vars(parser.parse_args()))
