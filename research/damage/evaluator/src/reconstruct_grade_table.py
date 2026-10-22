"""Rebuild the heap copy of the native718 grade-curve table (2FA678 lookup, D24926 binary search) from static bytes.

Observed layout in the PID 53660 capture (every retained same-PID cell is checked against it):
  rows:     base + 84*i for i in BINDING.native_resource_row_order index, bytes = resource01119.bin row order[i]
  pointers: ptr_base + 8*i = base + 84*i (662 entries), header Q = [ptr_base, ptr_end] at header_addr
argv: <retained ENTRY_REQUEST.json or FILLED_CELLS.json> <grade-curves dir> <out cells.json>
      [--base 0x1c627f0b240 --ptr-base 0x1c627f18ba0]
"""
import argparse
import json
import struct
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("retained", type=Path)
p.add_argument("grade_dir", type=Path)
p.add_argument("out", type=Path)
p.add_argument("--base", type=lambda v: int(v, 0), default=0x1C627F0B240)
p.add_argument("--ptr-base", type=lambda v: int(v, 0), default=0x1C627F18BA0)
args = p.parse_args()
ROW = 84
blob = (args.grade_dir / "resource01119.bin").read_bytes()
order = json.loads((args.grade_dir / "BINDING.json").read_bytes())["native_resource_row_order"]
n = (len(blob) - 8) // ROW
assert len(order) == n == struct.unpack_from("<I", blob, 4)[0]
image = {}
for i, j in enumerate(order):
    row = blob[8 + ROW * j:8 + ROW * (j + 1)]
    for k, b in enumerate(row):
        image[args.base + ROW * i + k] = b
    for k, b in enumerate(struct.pack("<Q", args.base + ROW * i)):
        image[args.ptr_base + 8 * i + k] = b
lo, hi = min(image), max(image) + 1
doc = json.loads(args.retained.read_bytes())
cells = doc["scene"]["memory_regions"] if isinstance(doc, dict) else doc
checked = mismatched = 0
for r in cells:
    a, data = r["address_u64"], bytes.fromhex(r["value_hex"])
    for i, b in enumerate(data):
        if a + i in image:
            checked += 1
            mismatched += image[a + i] != b
print(json.dumps({"rows": n, "range": [hex(lo), hex(hi)], "retained_bytes_checked": checked, "mismatched": mismatched}))
assert checked and not mismatched, "reconstruction disagrees with retained same-PID cells"
out = []
for a in range(lo - (lo % 8), hi, 8):
    chunk = bytes(image.get(a + i, 0) for i in range(8))
    if all((a + i) in image for i in range(8)):
        out.append({"name": "static_resource_" + hex(a), "address_u64": a, "value_hex": chunk.hex(), "known_hex": "ff" * 8,
                    "writable": True, "provenance": {"grade": "static resource01119 rebuilt in native row order; layout checked "
                                                              "against retained same-PID cells", "module_image": False,
                                                     "demand_reader_rva": "static_resource:grade_curve_table", "pid": None}})
args.out.write_text(json.dumps(out) + "\n")
print(json.dumps({"cells": len(out)}))
