"""How many compiled paths do different outfits need? Random real outfits from a backpack dump, each new path counted. argv: layers.json inventory_dir out.json count [export_dir [forced...]]

With export_dir, also writes the paths and every outfit with the VM's damage, for the Rust and GPU checks.
Forced files hold outfits the GPU reported, `bits slot:pool ...`: they run first at the starting attributes, so the VM checks the GPU's answer and any path it lacked gets compiled."""
import json
import os
import random
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from moves import MODIFIER_ROLE, MoveTable, hit_writes  # noqa: E402
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
WEAPON_TYPE_FIELD = 0x58
weapon_type, move_id, stance = map(int, os.environ.get("NIOH3_MOVE", "6409:25036:1").split(":"))
hits = MoveTable().hits(weapon_type, move_id, stance)
worn_weapon = None
weapons_file = os.environ.get("NIOH3_WEAPON_POOL")
if weapons_file:  # the player picks the weapon first: it stays worn, slot 0 leaves the search
    raw = Path(weapons_file).read_bytes()
    wanted = int(os.environ.get("NIOH3_WEAPON_KEY", "0"), 16)
    for i in range(0, len(raw) - ITEM + 1, 0xF0):
        row = raw[i:i + ITEM]
        key = struct.unpack_from("<H", row)[0]
        if key and key in p.PS.T.ITEMS.indices and struct.unpack_from("<I", p.PS.T.ITEMS.row(key), WEAPON_TYPE_FIELD)[0] == weapon_type and wanted in (0, key):
            worn_weapon = row
            break
    if worn_weapon is None:
        raise SystemExit(f"no weapon of type {weapon_type} in {weapons_file}")
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
          (base.stat_addr + 0x844, base.stat_addr + 0x860),
          (base.R[MODIFIER_ROLE][0] + 0x10, base.R[MODIFIER_ROLE][0] + 0x14), (base.main_addr + 0x13C, base.main_addr + 0x140),
          (base.main_addr + 0x144, base.main_addr + 0x14C)]
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
if worn_weapon:
    inventory = bytearray(m.inv)
    at = m.active * 0xF70
    inventory[at:at + ITEM] = worn_weapon
    m.inv = bytes(inventory)
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
            if rng.random() < .8 and not (s == 0 and worn_weapon):
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
    outfit_writes = held["w"]
    total, same, stopped = 0.0, True, False
    hit_vectors = []
    for hit in hits:
        writes = {**outfit_writes, **hit_writes(base, hit)}
        damage = vm_eval([], writes)
        try:
            got = compiler.evaluate(writes)
        except Exception as error:
            print(k, "compiler stopped", type(error).__name__, str(error)[:120], flush=True)
            stopped = True
            break
        agrees = got is not None and damage["damage"] is not None and struct.pack("<f", got) == struct.pack("<f", damage["damage"])
        same = same and agrees
        if not agrees:
            print(k, "disagrees: compiled", got, "VM", damage["damage"], {key: damage.get(key) for key in ("status", "reason")}, flush=True)
        else:
            total = struct.unpack("<f", struct.pack("<f", total + damage["damage"]))[0]
            hit_vectors.append((writes, struct.unpack("<I", struct.pack("<f", damage["damage"]))[0]))
    if stopped:
        wrong += 1
        continue
    new = compiler.stats["traces"] - before
    misses += new > 0
    wrong += not same
    if claimed and claimed != "xxxxxxxx":
        gpu_checked += 1
        gpu_wrong += claimed != struct.pack("<f", total)[::-1].hex()
    rows.append({"damage": total, "new": new})
    if same:
        bits = struct.unpack("<I", struct.pack("<f", total))[0]
        vectors.extend(hit_vectors)
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
    words = [sorted(hit_writes(base, h).items()) for h in hits]
    lines = [f"{len(hits)}", " ".join(f"{a:x}" for a, _ in words[0])]
    lines += [" ".join(f"{struct.unpack('<I', v)[0]:x}" for _, v in row) for row in words]
    (out / "hits.txt").write_text(chr(10).join(lines) + chr(10), encoding="ascii", newline=chr(10))
    (out / "gpu_cases.txt").write_text(chr(10).join(cases) + chr(10), encoding="ascii", newline=chr(10))
    gain, defense, ops = next(iter(setups))
    lines = [f"{gain} {defense} {len(ops)}"] + [f"{g} {v}" for g, v in ops] + [str(len(pool))]
    lines += [f"{s} {starts[s]} {len(items)}" for s, items in pool.items()] + [" ".join(map(str, start))]
    (out / "gpu_setup.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
