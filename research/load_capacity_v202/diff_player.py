"""Compare two read-only player snapshots and rank capacity-like fields."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
from pathlib import Path


def load(path: Path) -> tuple[dict[str, object], bytes]:
    metadata = json.loads(path.read_text(encoding="utf-8"))
    if metadata.get("schema") != "nioh3-load-capacity-player-capture/v1":
        raise ValueError(f"Unexpected capture schema: {path}")
    raw = path.with_name(str(metadata["snapshot_file"])).read_bytes()
    if hashlib.sha256(raw).hexdigest() != metadata["snapshot_sha256"]:
        raise ValueError(f"Snapshot hash mismatch: {path}")
    return metadata, raw


def comparable(a: dict[str, object], b: dict[str, object], allow_stamina_change: bool) -> None:
    keys = ["process_birth_filetime", "executable_sha256", "player_address"]
    if not allow_stamina_change:
        keys.append("stamina")
    elif a["style"] != b["style"]:
        raise ValueError("Stamina-change comparison requires the same style")
    for key in keys:
        if a[key] != b[key]:
            raise ValueError(f"Snapshots differ in {key}; capture a controlled same-process style switch")


def candidates(a: bytes, b: bytes, ui_a: float, ui_b: float) -> list[dict[str, object]]:
    hits = []
    for offset in range(0, min(len(a), len(b)) - 3, 4):
        if a[offset:offset + 4] == b[offset:offset + 4]:
            continue
        value_a = struct.unpack_from("<f", a, offset)[0]
        value_b = struct.unpack_from("<f", b, offset)[0]
        integer_a = struct.unpack_from("<I", a, offset)[0]
        integer_b = struct.unpack_from("<I", b, offset)[0]
        for kind, first, second in (
            ("f32", value_a, value_b),
            ("u32/10", integer_a / 10, integer_b / 10),
            ("u32/100", integer_a / 100, integer_b / 100),
        ):
            if math.isfinite(first) and math.isfinite(second) and abs(first - ui_a) <= 0.051 and abs(second - ui_b) <= 0.051:
                hits.append({"offset": f"0x{offset:X}", "type": kind, "first": first, "second": second})
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("first", type=Path)
    parser.add_argument("second", type=Path)
    parser.add_argument("--allow-stamina-change", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    first, first_raw = load(args.first)
    second, second_raw = load(args.second)
    comparable(first, second, args.allow_stamina_change)
    changed = [f"0x{offset:X}" for offset in range(min(len(first_raw), len(second_raw))) if first_raw[offset] != second_raw[offset]]
    result = {
        "first_style": first["style"],
        "second_style": second["style"],
        "first_stamina": first["stamina"],
        "second_stamina": second["stamina"],
        "changed_byte_count": len(changed),
        "changed_byte_offsets": changed[:256],
        "changed_offsets_truncated": len(changed) > 256,
        "capacity_candidates": candidates(first_raw, second_raw, float(first["ui_capacity"]), float(second["ui_capacity"])),
    }
    if args.output:
        if args.output.exists():
            raise FileExistsError(f"Refusing to replace output: {args.output}")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
