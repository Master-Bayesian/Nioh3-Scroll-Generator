"""Search attributes, soul cores and guardian for one hit, outfit fixed. Exact model calls, hill climb from several starts. argv: layers.json out.json"""
import json
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
items = [live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM] for s in range(8)]
owned = [c for c in (p.core(x) for x in items) if c]
H1 = "blind-1023/h1"
NOW = [37, 150, 27, 26, 80, 15, 150]
TOTAL, LOW = sum(NOW), 5
worn = owned[:2]
live_groups = {}
for i in range(struct.unpack_from("<I", live, 0x35D0 + 0xC00)[0]):
    e = live[0x35D0 + i * 12:0x35D0 + i * 12 + 12]
    live_groups[struct.unpack_from("<H", e)[0]] = struct.unpack_from("<I", e, 4)[0]
got = p.predict(H1, NOW, None, worn)
print("aggregate model vs live", {hex(g): (live_groups.get(g), got["aggregate"].get(g)) for g in set(live_groups) | set(got["aggregate"]) if live_groups.get(g) != got["aggregate"].get(g)}, flush=True)
calls = 0


def score(attrs, guardian=None, cores=worn):
    global calls
    calls += 1
    return p.predict(H1, attrs, guardian, cores)["damage"]


def climb(start, guardian=None, cores=worn):
    best, value = list(start), score(start, guardian, cores)
    for step in (40, 20, 10, 5, 1):
        better = True
        while better:
            better = False
            for a in range(7):
                for b in range(7):
                    if a == b or best[a] - step < LOW:
                        continue
                    c = list(best)
                    c[a] -= step
                    c[b] += step
                    v = score(c, guardian, cores)
                    if v > value:
                        best, value, better = c, v, True
    return value, best


t0 = time.time()
starts = [NOW, [69, 69, 69, 69, 69, 70, 70], [5, 150, 5, 5, 155, 5, 160], [5, 5, 5, 5, 5, 5, 455]]
runs = [climb(s) for s in starts]
runs.sort(reverse=True)
for r in runs:
    print("attributes", r, f"calls {calls} {time.time() - t0:.0f}s", flush=True)
best_attrs = runs[0][1]
one_axis = {}
for i in range(7):
    row = []
    for n in range(-30, 31, 10):
        a = list(NOW)
        a[i] += n
        a[6 if i != 6 else 1] -= n
        row.append((n, score(a) if min(a) >= LOW else None))
    one_axis[i] = row
print("axes, moved against 7th (2nd for 7th)", one_axis, flush=True)
guards = {"kusanagi": 0xDB, "tora": 0x15E, "phoenix": 0x15D, "nekomata": 0xC5}
for n, k in guards.items():
    g = p.guardian(k)
    print("guardian", n, g["attack"], g["defense"], "now", score(NOW, g), "best attrs", score(best_attrs, g), flush=True)
pairs = []
for i, a in enumerate(owned):
    for b in owned[i + 1:]:
        pairs.append((score(NOW, None, [a, b]), hex(a["key"]), hex(b["key"])))
pairs.sort(reverse=True)
print("core pairs at current attributes", pairs[:8], flush=True)
Path(sys.argv[2]).write_text(json.dumps({"attributes": runs, "axes": one_axis, "core_pairs": pairs}, indent=1), encoding="utf-8")
