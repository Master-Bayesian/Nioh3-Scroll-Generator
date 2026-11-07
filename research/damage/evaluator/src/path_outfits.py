"""How many compiled paths do different outfits need? Random real outfits from a backpack dump, each new path counted. argv: layers.json inventory_dir out.json count [export_dir [forced...]]

With export_dir, also writes the paths and every outfit with the VM's damage, for the Rust and GPU checks.
Forced files hold outfits the GPU reported, `bits slot:pool ...`: they run first at the starting attributes, so the VM checks the GPU's answer and any path it lacked gets compiled."""
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
folder = Path(sys.argv[2])
count = int(sys.argv[4])
catalog = json.loads((folder / "INVENTORY.json").read_bytes())
records = next(v for v in catalog.values() if isinstance(v, list) and len(v) > 100)
f0 = (folder / "INVENTORY_F0.bin").read_bytes()
pool = {}
for r in records:
    if r["kind"] == "armor" and r.get("physical_armor_slot") is not None:
        pool.setdefault(r["physical_armor_slot"], []).append(f0[r["F0_offset"]:r["F0_offset"] + ITEM])
pool = {s: pool[s] for s in sorted(pool)}
starts, flat = {}, []
for s, items in pool.items():
    starts[s] = len(flat)
    flat.extend(items)
print("slots", {s: len(v) for s, v in sorted(pool.items())}, flush=True)
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
real_pipeline, real_evaluate = p._pipeline, p._evaluate


def pipeline(m, groups):
    if groups is not None:
        held["ops"] = [(g, v) for totals in groups for g, v in totals.items()]
    return real_pipeline(m, groups)


def evaluate(t, s_t, d_t, source, guardian, cores):
    held.update(gain=guardian["attack"] + sum(x["attack"] for x in cores), defense=guardian["defense"] + sum(x["defense"] for x in cores))
    return real_evaluate(t, s_t, d_t, source, guardian, cores)


p._pipeline, p._evaluate = pipeline, evaluate
compiler = Compiler(p.ev, domain, [0xFBFE00, 0x2FA678])
rng = random.Random(20261108)
m = p.model("blind-1023/h1")
original = m.inv
start = [37, 150, 27, 26, 80, 15, 150]
rows, misses, wrong, refused = [], 0, 0, 0
vectors, cases, setups = [], [], set()
t0 = time.time()
forced = [line.split() for name in sys.argv[6:] for line in Path(name).read_text(encoding="ascii").strip().split("\n")]
gpu_checked = gpu_wrong = 0
for k in range(len(forced) + count):
    inv = bytearray(original)
    chosen = [-1] * 10
    claimed = None
    a = list(start)
    if k < len(forced):
        claimed = forced[k][0]
        for token in forced[k][1:]:
            if token.startswith("a:"):
                a = [int(x) for x in token[2:].split(",")]
                continue
            s, global_index = map(int, token.split(":"))
            at = m.active * 0xF70 + s * ITEM
            inv[at:at + ITEM] = flat[global_index]
            chosen[9 if s == 0 else s - 4] = global_index
    else:
        for s, items in pool.items():
            if rng.random() < .8:
                at = m.active * 0xF70 + s * ITEM
                pick = rng.randrange(len(items))
                inv[at:at + ITEM] = items[pick]
                chosen[9 if s == 0 else s - 4] = starts[s] + pick
        for _ in range(rng.randrange(0, 40)):
            i, j = rng.randrange(7), rng.randrange(7)
            if a[j] > 5:
                a[i] += 1
                a[j] -= 1
    m.inv = bytes(inv)
    try:
        r = p.predict("blind-1023/h1", a, None, worn)
    except Exception as error:
        refused += 1
        print(k, "refused", str(error)[:90], flush=True)
        continue
    finally:
        m.inv = original
    before = compiler.stats["traces"]
    try:
        got = compiler.evaluate(held["w"])
    except Exception as error:
        wrong += 1
        print(k, "compiler stopped", type(error).__name__, str(error)[:120], flush=True)
        continue
    new = compiler.stats["traces"] - before
    misses += new > 0
    same = got is not None and struct.pack("<f", got) == struct.pack("<f", r["damage"])
    wrong += not same
    if not same:
        print(k, "disagrees: compiled", got, "VM", r["damage"], {key: r.get(key) for key in ("status", "reason")}, flush=True)
    if claimed and claimed != "xxxxxxxx":
        gpu_checked += 1
        gpu_wrong += claimed != struct.pack("<f", r["damage"])[::-1].hex()
    rows.append({"damage": r["damage"], "new": new})
    if same:
        bits = struct.unpack("<I", struct.pack("<f", r["damage"]))[0]
        vectors.append((held["w"], bits))
        cases.append(f"{bits:08x} {' '.join(map(str, a))} {' '.join(map(str, chosen))}")
        setups.add((held["gain"], held["defense"], tuple(held["ops"])))
    if k % 20 == 0:
        print(k, f"{time.time() - t0:.0f}s", "mains", len(compiler.programs), "callees", sum(len(x) for x in compiler.subs), "misses", misses, "wrong", wrong, flush=True)
print("candidates", count, "misses", misses, "wrong", wrong, "refused", refused, "mains", len(compiler.programs), "sizes", sorted({q.size for q in compiler.programs}),
      "callees", sum(len(x) for x in compiler.subs), compiler.stats)
print("GPU claims checked", gpu_checked, "wrong", gpu_wrong, "forced", len(forced))
print("late misses", sum(1 for r in rows[len(rows) // 2:] if r["new"]), "of", len(rows) - len(rows) // 2)
Path(sys.argv[3]).write_text(json.dumps({"rows": rows, "stats": compiler.stats}, indent=1), encoding="utf-8")
if len(sys.argv) > 5:
    from path_export import export
    from pipeline_export import export_characters, export_frame, export_tables
    out = Path(sys.argv[5])
    print("setups", len(setups))
    assert len(setups) == 1
    export(compiler, vectors, out)
    export_tables(p, out)
    export_frame(p, out)
    export_characters(p, [m], out)
    (out / "pool.bin").write_bytes(b"".join(flat))
    (out / "gpu_cases.txt").write_text("\n".join(cases) + "\n", encoding="ascii", newline="\n")
    gain, defense, ops = next(iter(setups))
    lines = [f"{gain} {defense} {len(ops)}"] + [f"{g} {v}" for g, v in ops] + [str(len(pool))]
    lines += [f"{s} {starts[s]} {len(items)}" for s, items in pool.items()] + [" ".join(map(str, start))]
    (out / "gpu_setup.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
