"""Drop items no outfit can need: within a slot, an item another item beats on every damage-relevant number, with the same set membership.

Appends the survivors to pool.bin and points gpu_setup.txt at them, so the same GPU binary searches the pruned space. argv: in_dir out_dir
The rule is a heuristic until the pruned search returns the same best as the full one; it is checked that way, never assumed.
"""
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
target.mkdir(parents=True, exist_ok=True)
s = sections(source / "tables.bin")
items, effects, groups = Keyed(s, "item", 0x1A0), Keyed(s, "effect", 0xD8), Keyed(s, "group", 0x70)
base70 = s["base70"]
half = struct.unpack_from("<f", s["consts"], 12)[0]
char = sections(source / "characters.bin")["char0"]
cap = u32(char, 8)
live = {int(w) for w in (source / "live_groups.txt").read_text(encoding="ascii").split()}
lines = (source / "gpu_setup.txt").read_text(encoding="ascii").strip().split("\n")
op_n = int(lines[0].split()[2])
slot_n = int(lines[1 + op_n])
ranges = [tuple(map(int, x.split())) for x in lines[2 + op_n:2 + op_n + slot_n]]
attrs = list(map(int, lines[2 + op_n + slot_n].split()))
pool = (source / "pool.bin").read_bytes()


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
    return values, frozenset(sides), weight, level, u32(original, 0xB0) & 0x100


def beats(a, b):
    """a is at least as good as b everywhere, and can stand in for b's set membership."""
    if a[1] != b[1] and b[1]:
        return False
    if a[1] == b[1] and b[1] and a[3] < b[3]:
        return False
    if a[2] > b[2] or a[4] < b[4]:
        return False
    return all(a[0].get(k, 0) >= v for k, v in b[0].items())


appended, new_ranges, total, kept_total = bytearray(pool), [], 1, 1
for slot, start, count in ranges:
    rows = [(i, profile(pool[(start + i) * ITEM:(start + i + 1) * ITEM])) for i in range(count)]
    survivors = []
    for i, p in rows:
        strictly = any(beats(q, p) and (not beats(p, q) or j < i) for j, q in rows if j != i)
        if not strictly:
            survivors.append(i)
    print("slot", slot, count, "->", len(survivors))
    new_ranges.append((slot, len(appended) // ITEM, len(survivors)))
    for i in survivors:
        appended += pool[(start + i) * ITEM:(start + i + 1) * ITEM]
    total *= count
    kept_total *= len(survivors)
print("combinations", total, "->", kept_total)
for name in ("path_kernel.exe", "arena.bin", "mapped.bin", "tables.bin", "frame.bin", "characters.bin", "dom_base.bin", "dom_known.bin"):
    shutil.copyfile(source / name, target / name)
shutil.copyfile(source / "gpu_cases.txt", target / "gpu_cases.txt")
(target / "pool.bin").write_bytes(appended)
lines[2 + op_n:2 + op_n + slot_n] = [f"{a} {b} {c}" for a, b, c in new_ranges]
(target / "gpu_setup.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
