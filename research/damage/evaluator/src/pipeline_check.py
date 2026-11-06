"""Write mutated characters and the Python model's answers, for the Rust pipeline to match bit for bit. argv: layers.json out_dir count"""
import random
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from pipeline_export import export_characters, export_frame, export_tables  # noqa: E402
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
count = int(sys.argv[3])
CAPTURES = ["blind-1023/base", "blind-1023/h1", "blind-1023/h2", "blind-1023/h3", "blind-1023/h4", "blind-1023/h5"]
names = [c for c in CAPTURES if Path(W + c + "/capture.json").exists()]
models = [p.model(c) for c in names]
export_tables(p, out)
export_frame(p, out)
export_characters(p, models, out)
print("exported", names, flush=True)
T = p.base.T
effects = sorted(T.EFFECTS.indices)
BANK = 0xF70
pool = {s: [] for s in range(17)}
for m in models:
    for b in (0, 1):
        for s in range(17):
            item = bytes(m.inv[b * BANK + s * ITEM:b * BANK + (s + 1) * ITEM])
            if struct.unpack_from("<H", item)[0]:
                pool[s].append(item)
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
worn = [p.core(live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]) for s in range(2)]
rng = random.Random(20261107)


def attributes():
    if rng.random() < .1:
        return [rng.randrange(1, 300) for _ in range(7)]
    a = [5] * 7
    for _ in range(rng.randrange(100, 460)):
        a[rng.randrange(7)] += 1
    return a


def mutate(inv):
    inv = bytearray(inv)
    level = rng.random()
    if level < .3:
        return inv
    for s in range(17):
        if rng.random() > .4:
            continue
        for b in (0, 1):
            if rng.random() > .7:
                continue
            at = b * BANK + s * ITEM
            if pool[s] and rng.random() < .5:
                inv[at:at + ITEM] = rng.choice(pool[s])
            if rng.random() < .15:
                inv[at:at + ITEM] = bytes(ITEM)
                continue
            if not struct.unpack_from("<H", inv, at)[0] or level < .55:
                continue
            for field in rng.sample(range(8), rng.randrange(1, 6)):
                if field == 0:
                    struct.pack_into("<H", inv, at + 0xA, rng.choice([0, 3, 15, 30, 45, 90, 120, 200]))
                elif field == 1:
                    inv[at + 0x30] = rng.randrange(0, 7)
                elif field == 2:
                    inv[at + 0x31], inv[at + 0x32] = rng.randrange(0, 3), rng.randrange(0, 3)
                elif field == 3:
                    struct.pack_into("<H", inv, at + 6, rng.choice([1, 50, 120, 180, 250]))
                elif field == 4:
                    struct.pack_into("<I", inv, at + 0x14, rng.choice([0, 500, 5000, 40000, 200000]))
                elif field == 5:
                    inv[at + 0x33] = rng.choice([0, 0, 1, 2, 4, 8, 16, 127])
                elif field == 6:
                    struct.pack_into("<I", inv, at + 0x18, struct.unpack_from("<I", inv, at + 0x18)[0] ^ rng.choice([0x200000, 0x80000, 0x40000]))
                else:
                    for k in range(7):
                        if rng.random() < .6:
                            struct.pack_into("<I", inv, at + 0x38 + 24 * k, rng.choice(effects))
                            struct.pack_into("<I", inv, at + 0x3C + 24 * k, rng.choice([0, 1, 5, 12, 40, 100, 300, 1000]))
    return inv


seen = {}
real_pipeline, real_evaluate = p._pipeline, p._evaluate


def pipeline(m, groups):
    r = real_pipeline(m, groups)
    if groups is not None:
        seen.update(s=r[0], d=r[1], ops=[(g, v) for totals in groups for g, v in totals.items()])
    return r


def evaluate(t, s_t, d_t, source, guardian, cores):
    seen.update(gain=guardian["attack"] + sum(x["attack"] for x in cores), defense=guardian["defense"] + sum(x["defense"] for x in cores))
    return real_evaluate(t, s_t, d_t, source, guardian, cores)


p._pipeline, p._evaluate = pipeline, evaluate
p.ev.evaluate = lambda pieces, overrides=None: (seen.update(w=overrides), {"damage": 0.0, "status": "skipped"})[1]
lines, failed = [], 0
t0 = time.time()
with (out / "expect.bin").open("wb") as f:
    for k in range(count):
        c = rng.randrange(len(models))
        m = models[c]
        a = attributes()
        original = m.inv
        m.inv = bytes(mutate(original))
        seen.clear()
        status = 0
        try:
            p.predict(names[c], a, None, worn if rng.random() < .5 else None)
        except Exception as error:
            status, failed = 1, failed + 1
            if failed <= 8:
                print(k, type(error).__name__, str(error)[:120], flush=True)
        f.write(bytes([status]) + m.inv)
        m.inv = original
        if status == 0:
            ws = sorted(seen["w"].items())
            f.write(seen["s"] + seen["d"] + struct.pack("<I", len(ws)))
            for address, data in ws:
                f.write(struct.pack("<Q", address) + data)
            ops = " ".join(f"{g}:{v}" for g, v in seen["ops"])
            lines.append(f"{c} {' '.join(map(str, a))} {seen['gain']} {seen['defense']} {ops}")
        else:
            lines.append(f"{c} {' '.join(map(str, a))} 0 0 ")
        if k % 200 == 0:
            print(k, f"{time.time() - t0:.0f}s", flush=True)
(out / "vectors.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
print("vectors", count, "python refused", failed, f"{(time.time() - t0) / count * 1000:.1f} ms each")
