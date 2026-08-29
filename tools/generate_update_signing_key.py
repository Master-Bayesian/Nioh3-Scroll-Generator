"""Generate one Ed25519 key pair for the signed desktop update channel."""

from __future__ import annotations

import argparse
import base64
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    NoEncryption,
    PrivateFormat,
    PublicFormat,
)


def write_exclusive(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="ascii", newline="\n") as stream:
        stream.write(value)
        stream.write("\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--private-output", type=Path, required=True)
    parser.add_argument("--public-output", type=Path, required=True)
    args = parser.parse_args()

    key = Ed25519PrivateKey.generate()
    private_value = base64.b64encode(
        key.private_bytes(Encoding.Raw, PrivateFormat.Raw, NoEncryption())
    ).decode("ascii")
    public_value = base64.b64encode(
        key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)
    ).decode("ascii")
    write_exclusive(args.private_output, private_value)
    try:
        write_exclusive(args.public_output, public_value)
    except Exception:
        args.private_output.unlink(missing_ok=True)
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
