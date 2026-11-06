"""Search a player's backpack from a decrypted save, on this character and attack: real item counts without touching the game.

Item records in the save are the live pool's rows (stride 0xF0, E8 payload), so the same GPU binary runs them. Worn items are not in the backpack
rows, and the character stays the one the bundle was captured on, so this measures search size and speed, not that player's real damage.
argv: gpu_dir out_dir decrypted_save names.json class("武士防具"|"忍者防具")
"""
import collections
import json
import shutil
import struct
import sys
from pathlib import Path

ITEM, ROW = 0xE8, 0xF0
PART = {"头部": 4, "身体": 5, "手臂": 6, "腿部": 7, "足部": 8}
source, target, save, names_path, kind = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), sys.argv[5]
target.mkdir(parents=True, exist_ok=True)
names = json.loads(names_path.read_bytes())
d = save.read_bytes()


def is_item(at):
    key = struct.unpack_from("<H", d, at)[0]
    return str(key) in names and struct.unpack_from("<H", d, at + 2)[0] == key and d[at + 0x30] <= 5


# The pool's alignment differs between saves only if the layout does; take it from the data, and refuse if it is not clear.
alignment = collections.Counter(at % ROW for at in range(0x200000, 0x320000) if is_item(at) and names[str(struct.unpack_from("<H", d, at)[0])][1] == "防具")
(phase, votes), *rest = alignment.most_common(3)
if rest and rest[0][1] * 2 > votes and (rest[0][0] - phase) % ROW != 2:
    raise SystemExit(f"no clear record alignment: {alignment.most_common(3)}")
pool = bytearray((source / "pool.bin").read_bytes())
by_slot = {slot: [] for slot in PART.values()}
for at in range(0x200000 + (phase - 0x200000) % ROW, 0x320000, ROW):
    if is_item(at):
        n = names[str(struct.unpack_from("<H", d, at)[0])]
        if n[1] == "防具" and n[2] == kind and n[3] in PART:
            by_slot[PART[n[3]]].append(d[at:at + ITEM])
lines = (source / "gpu_setup.txt").read_text(encoding="ascii").strip().split("\n")
op_n = int(lines[0].split()[2])
slot_n = int(lines[1 + op_n])
ranges = []
total = 1
for slot, rows in sorted(by_slot.items()):
    ranges.append(f"{slot} {len(pool) // ITEM} {len(rows)}")
    for row in rows:
        pool += row
    total *= max(len(rows), 1)
lines[2 + op_n:2 + op_n + slot_n] = ranges
print("armor per slot", [len(by_slot[s]) for s in sorted(by_slot)], "combinations", total)
for name in ("path_kernel.exe", "arena.bin", "mapped.bin", "tables.bin", "frame.bin", "characters.bin", "dom_base.bin", "dom_known.bin", "gpu_cases.txt", "live_groups.txt"):
    shutil.copyfile(source / name, target / name)
(target / "pool.bin").write_bytes(pool)
(target / "gpu_setup.txt").write_text("\n".join(lines) + "\n", encoding="ascii", newline="\n")
(target / "gpu_spaces.txt").unlink(missing_ok=True)
