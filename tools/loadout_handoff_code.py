"""Validate bounded native code evidence without distributing full game sections."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path

def verify_reviewed_code(directory: Path, executable: str, signatures: dict[int, str]) -> str:
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    proof = json.loads((directory / "reviewed-code-windows.json").read_text(encoding="utf-8"))
    section = next(row for row in manifest["sections"] if row["name"] == ".text")
    if (manifest["executable"]["sha256"].lower() != executable.lower()
            or proof["executable_sha256"].lower() != executable.lower()
            or proof["source_text_sha256"].lower() != section["sha256"].lower()):
        raise ValueError("Code-window source identity mismatch")
    windows = {int(row["rva"], 0): row for row in proof["windows"]}
    for rva, expected in signatures.items():
        row = windows[rva]
        raw = bytes.fromhex(row["bytes_hex"])
        if raw.hex() != expected or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Native signature mismatch at {rva:#x}")
    # This is the recorded source hash, not a receiver-side full-section replay.
    return proof["source_text_sha256"].lower()
