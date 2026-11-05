"""Which effect fields does the target's debuff scale? Zero one D word at a time, with and without the status.

A word whose removal changes the with/without-debuff ratio is gated by the debuff.
argv: capture.json request.json empty_retained.json out.json
"""
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fast_eval import Evaluator  # noqa: E402
from situations import status_tree  # noqa: E402

CACHE = "F:/Nioh3_r16_build/cache"
capture, request, retained, out = sys.argv[1:5]
ev = Evaluator(CACHE, capture, request, retained, [], observe_only=True)
m = ev.model
clear = status_tree(ev, m.R["target.status.tree.head"][0], [])
with_ = ev.evaluate([])["damage"]
without = ev.evaluate([], clear)["damage"]
print("with status", with_, "emptied", without, "ratio", with_ / without)
rows = []
for at in range(0, 0xB00, 4):
    word = m.d[at:at + 4]
    if word == m.d0[at:at + 4] or word == b"\0\0\0\0":
        continue
    f = struct.unpack("<f", word)[0]
    if not 0 < abs(f) < 100000:
        continue
    zero = {m.d_addr + at: struct.pack("<f", m.d0[at:at + 4] and struct.unpack("<f", m.d0[at:at + 4])[0])}
    a = ev.evaluate([], zero)["damage"]
    b = ev.evaluate([], {**clear, **zero})["damage"]
    if a is None or b is None:
        print(hex(at), "unknown")
        continue
    row = {"d": hex(at), "value": round(f, 4), "with": a, "emptied": b, "gap": a - b}
    rows.append(row)
    if a != with_ or b != without:
        print(row, flush=True)
Path(out).write_text(json.dumps({"with": with_, "emptied": without, "rows": rows}, ensure_ascii=False, indent=0), encoding="utf-8")
