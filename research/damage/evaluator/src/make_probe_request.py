"""Write a filled request with the effect probe applied (every zero f32 D effect field set to 0.01).

The probed path can read memory the unprobed fill never demanded; resume demand_fill_loop on the result
(--resume-request) while the game is still open.
argv: <capture.json> <ENTRY_REQUEST.json> <out.json> [--exclude 0x104 ...]
"""
import argparse
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from outfit_model import OutfitModel  # noqa: E402


def main():
    p = argparse.ArgumentParser()
    p.add_argument("capture", type=Path)
    p.add_argument("request", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("--exclude", nargs="*", default=["0x104", "0x530", "0x714"])
    args = p.parse_args()
    from regress_fast import CACHE
    m = OutfitModel(CACHE, args.capture, observe_only=True)
    request = json.loads(args.request.read_bytes())
    region = next(r for r in request["scene"]["memory_regions"]
                  if r["address_u64"] <= m.d_addr < r["address_u64"] + len(r["value_hex"]) // 2)
    value = bytearray.fromhex(region["value_hex"])
    skip = {int(x, 16) for x in args.exclude}
    probed = []
    for row in m.F.catalog()["recipes"].values():
        for op in row.get("operations", []):
            if str(op.get("equation", "")).startswith("D_f32") and op.get("width") == 4:
                off = int(op["destination"], 16)
                at = m.d_addr + off - region["address_u64"]
                if off not in skip and 0 <= at <= len(value) - 4 and value[at:at + 4] == b"\0\0\0\0":
                    value[at:at + 4] = struct.pack("<f", 0.01)
                    probed.append(hex(off))
    region["value_hex"] = value.hex()
    args.out.write_text(json.dumps(request) + "\n")
    print(json.dumps({"probed_fields": len(probed)}))


if __name__ == "__main__":
    main()
