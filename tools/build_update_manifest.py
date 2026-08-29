"""Build and sign one release manifest for the desktop update channel."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.updater import UpdateManifest  # noqa: E402


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest().upper()


def build_manifest(
    *,
    version: str,
    asset: Path,
    asset_url: str,
    notes: str,
    private_key_base64: str,
    published_at_utc: str,
) -> dict[str, object]:
    try:
        private_key_bytes = base64.b64decode(private_key_base64.strip(), validate=True)
    except ValueError as error:
        raise ValueError("update signing key is not valid base64") from error
    if len(private_key_bytes) != 32:
        raise ValueError("update signing key must be one raw Ed25519 private key")
    asset = asset.resolve()
    if not asset.is_file():
        raise FileNotFoundError(asset)
    unsigned = UpdateManifest(
        version=version,
        published_at_utc=published_at_utc,
        notes=notes,
        asset_name=asset.name,
        asset_url=asset_url,
        asset_size=asset.stat().st_size,
        asset_sha256=sha256_file(asset),
        signature_base64=base64.b64encode(bytes(64)).decode("ascii"),
    )
    unsigned.validate()
    signature = Ed25519PrivateKey.from_private_bytes(private_key_bytes).sign(
        unsigned.signed_payload()
    )
    value: dict[str, object] = {
        "schema": 1,
        "version": unsigned.version,
        "published_at_utc": unsigned.published_at_utc,
        "notes": unsigned.notes,
        "asset": {
            "name": unsigned.asset_name,
            "url": unsigned.asset_url,
            "size": unsigned.asset_size,
            "sha256": unsigned.asset_sha256,
        },
        "signature": base64.b64encode(signature).decode("ascii"),
    }
    UpdateManifest.from_mapping(value)
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--asset", type=Path, required=True)
    parser.add_argument("--asset-url", required=True)
    parser.add_argument("--notes-file", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--published-at-utc")
    args = parser.parse_args()

    private_key = os.environ.get("UPDATE_SIGNING_PRIVATE_KEY_BASE64", "")
    if not private_key:
        raise RuntimeError("UPDATE_SIGNING_PRIVATE_KEY_BASE64 is not set")
    published_at_utc = args.published_at_utc or datetime.now(timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )
    value = build_manifest(
        version=args.version,
        asset=args.asset,
        asset_url=args.asset_url,
        notes=args.notes_file.read_text(encoding="utf-8").strip(),
        private_key_base64=private_key,
        published_at_utc=published_at_utc,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
