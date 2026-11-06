"""Compiled path against the VM on real candidates: every damage must match bit for bit. argv: layers.json out.json [count]"""
import json
import random
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from path_compile import Compiler  # noqa: E402
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
count = int(sys.argv[3]) if len(sys.argv) > 3 else 60
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
worn = [p.core(live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]) for s in range(2)]
base = p.base
domain = [(base.stat_addr, base.stat_addr + 8), (base.stat_addr + 0x10, base.stat_addr + 0x7C0), (base.d_addr, base.d_addr + 0xB00),
          (base.main_addr + 8, base.main_addr + 0x18), (base.R["current_player.external_STAT"][0] + 0x7B8, base.R["current_player.external_STAT"][0] + 0x7BC),
          (base.stat_addr + 0x844, base.stat_addr + 0x860)]
held = {}
vm_eval = p.ev.evaluate


def capture(pieces, overrides=None):
    held["w"] = overrides
    return vm_eval(pieces, overrides)


p.ev.evaluate = capture
compiler = Compiler(p.ev, domain, [0xFBFE00, 0x2FA678])
start = [37, 150, 27, 26, 80, 15, 150]
rng = random.Random(20261106)
sets = [start, [5, 200, 15, 75, 5, 35, 150], [5, 45, 15, 200, 5, 60, 155], [5, 5, 5, 36, 6, 5, 140]]
while len(sets) < count:
    a = [5] * 7
    for _ in range(sum(start) - 35):
        k = rng.randrange(7)
        if a[k] < 250:
            a[k] += 1
    if rng.random() < .5:  # also walk near the optimum, where the search spends its time
        a = list(sets[rng.randrange(3)])
        for _ in range(rng.randrange(1, 12)):
            i, j = rng.randrange(7), rng.randrange(7)
            if a[j] > 5:
                a[i] += 1
                a[j] -= 1
    sets.append(a)


def fresh(n):
    out = []
    while len(out) < n:
        a = list(sets[rng.randrange(4)])
        for _ in range(rng.randrange(1, 60)):
            i, j = rng.randrange(7), rng.randrange(7)
            if a[j] > 5 and a[i] < 250:
                a[i] += 1
                a[j] -= 1
        out.append(a)
    return out


rows, wrong = [], 0
vectors = []
vm_time = compiled_time = 0.0
for a in sets:
    t = time.perf_counter()
    r = p.predict("blind-1023/h1", a, None, worn)
    vm_time += time.perf_counter() - t
    t = time.perf_counter()
    before = dict(compiler.stats)
    got = compiler.evaluate(held["w"])
    compiled_time += time.perf_counter() - t
    same = got is not None and r["damage"] is not None and struct.pack("<f", got) == struct.pack("<f", r["damage"])
    wrong += not same
    vectors.append((held["w"], struct.unpack("<I", struct.pack("<f", r["damage"]))[0]))
    rows.append({"attributes": a, "vm": r["damage"], "compiled": got, "same": same, "new_path": compiler.stats["traces"] != before["traces"]})
    print(a, r["damage"], got, "ok" if same else "DIFF", "new path" if rows[-1]["new_path"] else "", flush=True)
fast = int(sys.argv[4]) if len(sys.argv) > 4 else 0
spent = {"pipeline and writes": 0.0, "compiled": 0.0}
misses = checked = disagreed = 0
stopped = {"damage": 0.0, "status": "skipped"}
for k, a in enumerate(fresh(fast)):
    t = time.perf_counter()
    p.ev.evaluate = lambda pieces, overrides=None: (held.update(w=overrides), stopped)[1]
    p.predict("blind-1023/h1", a, None, worn)
    spent["pipeline and writes"] += time.perf_counter() - t
    t = time.perf_counter()
    traces = compiler.stats["traces"]
    got = compiler.evaluate(held["w"])
    grew = compiler.stats["traces"] != traces
    misses += grew
    if not grew:
        spent["compiled"] += time.perf_counter() - t
    vectors.append((held["w"], struct.unpack("<I", struct.pack("<f", got))[0]))
    if k % 25 == 0:
        p.ev.evaluate = capture
        r = p.predict("blind-1023/h1", a, None, worn)
        checked += 1
        disagreed += r["damage"] != got
print("fast pass", fast, "new paths", misses, "VM checks", checked, "disagreed", disagreed, {k: f"{v / max(fast, 1) * 1000:.2f} ms" for k, v in spent.items()})
print("wrong", wrong, "of", len(sets), compiler.stats, "paths", [q.size for q in compiler.programs])
Path(sys.argv[2]).write_text(json.dumps({"wrong": wrong, "stats": compiler.stats, "paths": [q.size for q in compiler.programs], "rows": rows}, indent=1), encoding="utf-8")
print(sorted(compiler.reasons.items(), key=lambda x: -x[1])[:30])
print("inline", [hex(x) for x in compiler.inline])
print("sites", len(compiler.subs), "sub paths", [len(x) for x in compiler.subs], "sizes", [q.size for x in compiler.subs for q in x][:40])
if len(sys.argv) > 5:
    from path_export import export
    export(compiler, vectors, sys.argv[5])
