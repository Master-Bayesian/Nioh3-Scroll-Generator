"""Drop items no outfit can need: within a slot, an item another item beats on every damage-relevant number, with the same set membership.

Appends the survivors to pool.bin and points gpu_setup.txt at them, so the same GPU binary searches the pruned space. argv: in_dir out_dir
The rule is a heuristic until the pruned search returns the same best as the full one; it is checked that way, never assumed.

With a required set (argv: in_dir out_dir set_key pieces) the space becomes one disjoint subspace per choice of which slots hold set pieces, at
least `pieces` of them, written to gpu_spaces.txt. A slot chosen for the set lists only that set's items, every other slot only the rest, so no
outfit is counted twice. With set_key "list" it prints the sets in the pool and stops.
"""
import itertools
import os
import shutil
import struct
import sys
from pathlib import Path

ITEM = 0xE8


def sections(path):
    d, at, out = Path(path).read_bytes(), 0, {}
    while at < len(d):
        n = struct.unpack_from("<I", d, at)[0]
        name = d[at + 4:at + 4 + n].decode()
        at += 4 + n
        size = struct.unpack_from("<I", d, at)[0]
        out[name] = d[at + 4:at + 4 + size]
        at += 4 + size
    return out


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


class Keyed:
    def __init__(self, s, name, stride):
        self.rows, self.stride = s[f"{name}.rows"], stride
        k = s[f"{name}.keys"]
        self.index = dict(struct.iter_unpack("<II", k))

    def row(self, key):
        i = self.index.get(key)
        return None if i is None else self.rows[8 + i * self.stride:8 + (i + 1) * self.stride]


source, target = Path(sys.argv[1]), Path(sys.argv[2])
required = sys.argv[3] if len(sys.argv) > 3 else None
pieces = int(sys.argv[4]) if len(sys.argv) > 4 else 0
# NIOH3_LIMITS "max_load min_defense min_health". A limit only adds what can decide it: load adds weight if some outfit could break it, minimum defense
# adds defense, and either survival minimum adds the groups that write those numbers. Anything else about an item stays out of the comparison.
LIMITS = [float(w) for w in (os.environ.get("NIOH3_LIMITS") or "").split()] or [float("inf"), 0.0, 0.0]
target.mkdir(parents=True, exist_ok=True)
s = sections(source / "tables.bin")
items, effects, groups = Keyed(s, "item", 0x1A0), Keyed(s, "effect", 0xD8), Keyed(s, "group", 0x70)
base70 = s["base70"]
half = struct.unpack_from("<f", s["consts"], 12)[0]
char = sections(source / "characters.bin")["char0"]
cap = u32(char, 8)
live = {int(w) for w in (source / "live_groups.txt").read_text(encoding="ascii").split()}
if LIMITS[1] > 0 or LIMITS[2] > 0:
    live |= {int(w) for w in (source / "survival_groups.txt").read_text(encoding="ascii").split()}
lines = (source / "gpu_setup.txt").read_text(encoding="ascii").strip().split("\n")
op_n = int(lines[0].split()[2])
slot_n = int(lines[1 + op_n])
ranges = [tuple(map(int, x.split())) for x in lines[2 + op_n:2 + op_n + slot_n]]
attrs = list(map(int, lines[2 + op_n + slot_n].split()))
pool = (source / "pool.bin").read_bytes()
WATCH_WEIGHT = False


def field70(index, off):
    return u16(base70, min(index, len(base70) // 0x94 - 1) * 0x94 + off)


def qualified(item, original):
    rarity = struct.unpack_from("<b", item, 0x30)[0]
    delta = 0
    if rarity >= 3:
        delta = int(field70(min(u16(item, 0xA), cap), 0x62) * (((item[0x31] == 1) + (item[0x32] == 1)) * half))
    column = 1 if rarity >= 5 and u32(original, 0x60) != 0x2D32 else 0
    for i in range(7):
        b = original[0x18C + 2 * i + column]
        if (b + delta if b else 0) > attrs[i]:
            return False
    return True


def profile(item):
    original = items.row(u16(item, 0))
    rarity = struct.unpack_from("<b", item, 0x30)[0]
    selected = original
    if rarity >= 4 and u16(original, 0x160):
        selected = items.row(u16(original, 0x160)) or original
    weight = u32(selected, 0x98) + int(field70(min(u16(item, 0xA), cap), 0x6A) * ((((item[0x31] == 2) + (item[0x32] == 2)) * half) if rarity >= 3 else 0))
    values, sides = {}, set()
    if qualified(item, original):
        for i in range(7):
            effect = u32(item, 0x38 + 24 * i)
            row = None if effect == 0xFFFFFFFF else effects.row(effect)
            if row is None:
                continue
            group = u16(row, 2)
            g = groups.row(group)
            if g is None:
                continue
            if u32(g, 8):
                sides.add(u32(g, 8))
            elif group in live:
                values[group] = values.get(group, 0) + u32(item, 0x3C + 24 * i)
    level = 0 if u32(item, 0x18) & 0x200000 else min(u16(item, 6), 180)
    defense = u32(selected, 0xF8) * max(level, 1) + u32(selected, 0x50) * 1000 + values.get(0xDA05, 0) * 1000
    return values, frozenset(sides), weight, level, u32(original, 0xB0) & 0x100, defense


def heaviest_load():
    """Load of the heaviest outfit at the smallest capacity of either stance; at or under the limit, weight cannot decide anything."""
    power = struct.unpack_from("<HH", s["power90"], attrs[2] * 32 + 8)
    total = 0
    for slot, start, count in ranges:
        if slot >= 4:
            total += max((profile(pool[(start + i) * ITEM:(start + i + 1) * ITEM])[2] for i in range(count)), default=0)
    return total / min(power)


WATCH_WEIGHT = heaviest_load() > LIMITS[0]


def beats(a, b):
    """a is at least as good as b everywhere, and can stand in for b's set membership."""
    if a[1] != b[1] and b[1]:
        return False
    if WATCH_WEIGHT and a[2] > b[2]:
        return False
    if LIMITS[1] > 0 and a[5] < b[5]:
        return False
    if a[1] == b[1] and b[1] and a[3] < b[3]:
        return False
    if a[4] < b[4]:
        return False
    return all(a[0].get(k, 0) >= v for k, v in b[0].items())


def kept(rows, slot=None):
    out = []
    if os.environ.get("NIOH3_NO_PRUNE") or slot == 0:  # a weapon's worth is its attack, which no effect list shows
        return [i for i, _ in rows]
    for i, p in rows:
        strictly = any(beats(q, p) and (not beats(p, q) or j < i) for j, q in rows if j != i)
        if not strictly:
            out.append(i)
    return out


links = []


def finish(appended, spaces):
    for name in ("path_kernel.exe", "arena.bin", "mapped.bin", "tables.bin", "frame.bin", "characters.bin", "dom_base.bin", "dom_known.bin"):
        shutil.copyfile(source / name, target / name)
    shutil.copyfile(source / "gpu_cases.txt", target / "gpu_cases.txt")
    (target / "pool.bin").write_bytes(appended)
    first = {}
    for i in range(len(pool) // ITEM):
        first.setdefault(bytes(pool[i * ITEM:(i + 1) * ITEM]), i)
    origin = [first[bytes(appended[i * ITEM:(i + 1) * ITEM])] for i in range(len(appended) // ITEM)]
    (target / "gpu_origin.txt").write_text(" ".join(map(str, origin)) + "\n", encoding="ascii", newline="\n")  # pool row each appended row copies
    lines[2 + op_n:2 + op_n + slot_n] = [f"{a} {b} {c}" for a, b, c in spaces[0]]
    (target / "gpu_setup.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
    text = [str(len(spaces))] + [" ".join(f"{a} {b} {c}" for a, b, c in one) for one in spaces]
    (target / "gpu_spaces.txt").write_text("\n".join(text) + "\n", encoding="ascii", newline="\n")
    (target / "gpu_full.txt").write_text("\n".join(" ".join(map(str, link)) for link in links) + "\n", encoding="ascii", newline="\n")


def split_by_set():
    tally = {}
    rows_of = {}
    for slot, start, count in ranges:
        rows_of[slot] = [(start + i, profile(pool[(start + i) * ITEM:(start + i + 1) * ITEM])) for i in range(count)]
        for _, p in rows_of[slot]:
            for key in p[1]:
                tally.setdefault(key, {}).setdefault(slot, 0)
                tally[key][slot] += 1
    if required == "list":
        for key, by in sorted(tally.items(), key=lambda kv: -sum(kv[1].values())):
            print(hex(key), [by.get(slot, 0) for slot, _, _ in ranges])
        return
    key = int(required, 16)
    appended, sections_of = bytearray(pool), {}
    for slot, start, count in ranges:
        inside = [(i, p) for i, p in rows_of[slot] if key in p[1] and slot != 0]
        outside = [(i, p) for i, p in rows_of[slot] if key not in p[1] or slot == 0]
        sections_of[slot] = []
        for group in (outside, inside):
            at = len(appended) // ITEM
            chosen = kept(group, slot)
            for i in chosen:
                appended += pool[i * ITEM:(i + 1) * ITEM]
            full = len(appended) // ITEM
            for i, _ in group:
                appended += pool[i * ITEM:(i + 1) * ITEM]
            links.append((at, len(chosen), full, len(group)))
            sections_of[slot].append((at, len(chosen)))
        print("slot", slot, "outside", len(outside), "->", sections_of[slot][0][1], "inside", len(inside), "->", sections_of[slot][1][1])
    spaces, total = [], 0
    slots = [slot for slot, _, _ in ranges]
    can_hold = [slot for slot in slots if slot != 0 and sections_of[slot][1][1]]
    for k in range(pieces, len(can_hold) + 1):
        for chosen in itertools.combinations(can_hold, k):
            one = [(slot, *sections_of[slot][1 if slot in chosen else 0]) for slot in slots]
            size = 1
            for _, _, n in one:
                size *= n
            if size:
                spaces.append(one)
                total += size
    print("spaces", len(spaces), "combinations", total)
    finish(appended, spaces)


if required:
    split_by_set()
    sys.exit(0)
appended, new_ranges, total, kept_total = bytearray(pool), [], 1, 1
for slot, start, count in ranges:
    rows = [(i, profile(pool[(start + i) * ITEM:(start + i + 1) * ITEM])) for i in range(count)]
    survivors = kept(rows, slot)
    print("slot", slot, count, "->", len(survivors))
    new_ranges.append((slot, len(appended) // ITEM, len(survivors)))
    links.append((len(appended) // ITEM, len(survivors), start, count))
    for i in survivors:
        appended += pool[(start + i) * ITEM:(start + i + 1) * ITEM]
    total *= count
    kept_total *= len(survivors)
print("combinations", total, "->", kept_total)
finish(appended, [new_ranges])
