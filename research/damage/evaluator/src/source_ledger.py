"""Reconcile the live effect-group aggregate against each known source. argv: pid stat_hex out.json"""
import collections, ctypes, ctypes.wintypes as W, json, struct, sys
from pathlib import Path
pid, stat_arg, out = int(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
k = ctypes.WinDLL("kernel32", use_last_error=True)
k.OpenProcess.restype = W.HANDLE; k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
psapi = ctypes.WinDLL("psapi")
psapi.EnumProcessModulesEx.argtypes = [W.HANDLE, ctypes.c_void_p, W.DWORD, ctypes.POINTER(W.DWORD), W.DWORD]
h = k.OpenProcess(0x0410, False, pid)
def read(a, s):
    b, n = ctypes.create_string_buffer(s), ctypes.c_size_t()
    if not k.ReadProcessMemory(h, ctypes.c_void_p(a), b, s, ctypes.byref(n)) or n.value != s:
        raise RuntimeError(hex(a))
    return b.raw
q = lambda a: struct.unpack("<Q", read(a, 8))[0]
u = lambda a: struct.unpack("<I", read(a, 4))[0]
mods = (ctypes.c_void_p * 1024)(); need = W.DWORD()
psapi.EnumProcessModulesEx(h, mods, ctypes.sizeof(mods), ctypes.byref(need), 3)
base = mods[0]
prog = q(q(base + 0x4751860)); mgr = q(base + 0x45B9E30)
W_ = Path("F:/Nioh3_r16_build/work/live-1104")
gname = json.loads((W_ / "SOURCES.json").read_bytes())["gname"]
texts = json.loads((W_ / "ALL_TEXTS.json").read_bytes())

stat = int(stat_arg, 16)
src = q(stat + 0x7D8)
def aggregate(off):
    count = u(src + off + 0xC00)
    raw = read(src + off, 0xC00)
    return {struct.unpack_from("<H", raw, i * 12)[0]: (struct.unpack_from("<I", raw, i * 12 + 4)[0], struct.unpack_from("<H", raw, i * 12 + 8)[0]) for i in range(count)}
form0, form1 = aggregate(0x35D0), aggregate(0x4728)
print("source", hex(src), "form", u(src + 0x2C), "entries", len(form0), len(form1))

# Known sources.
led = collections.defaultdict(list)
def add(form, group, value, label):
    if group:
        led[(form, group)].append((value, label))

# Blessings (form = [rsp+0x30] = the source's current form for all table sources).
for r in json.loads((W_ / "BLESSINGS.json").read_bytes())["rows"]:
    if r["value"]:
        add("all", int(r["group"], 16), r["value"], "恩惠")
# Six-rank tables decoded earlier.
for s in range(2):
    for f in range(2):
        v = q(prog + 0x6340 + (s + 2 * f) * 16)
        add(f, v & 0xFFFF, v >> 32, "slot6340")
# Passive skills.
flat130 = q(q(mgr + 0x130)); n130 = u(flat130 + 4); rows130 = read(flat130 + 8, n130 * 0xC0)
flat598 = q(q(mgr + 0x598)); n598 = u(flat598 + 4); rows598 = read(flat598 + 8, n598 * 0x94)
by598 = {}
for i in range(n598):
    r = rows598[i * 0x94:(i + 1) * 0x94]
    key, rank = struct.unpack_from("<II", r)
    by598[(key & 0xFFFF, rank)] = r
sent = q(prog + 0x6028); cnt = q(prog + 0x6030)
nodes = {}
n = q(sent)
while n != sent and len(nodes) <= cnt + 1:
    raw = read(n, 0x20)
    nodes[struct.unpack_from("<H", raw, 0x10)[0]] = (struct.unpack_from("<H", raw, 0x1A)[0], struct.unpack_from("<H", raw, 0x1C)[0])
    n = q(n)
skills = []
for i in range(n130):
    r = rows130[i * 0xC0:(i + 1) * 0xC0]
    key = struct.unpack_from("<H", r, 0x18)[0]
    kind, cat = r[0xB9], struct.unpack_from("<b", r, 0xAC)[0]
    cost = struct.unpack_from("<f", r, 0x84)[0]
    node = nodes.get(key)
    if kind != 1 or not node or not node[1]:
        continue
    r598 = by598.get((key, node[0] or 1))
    if r598 is None:
        continue
    recs = []
    for o in (0x40, 0x4C, 0x58):
        g, v = struct.unpack_from("<H", r598, o)[0], struct.unpack_from("<I", r598, o + 8)[0]
        if g:
            recs.append((g, v))
    forms = {0x15: (0,), 0x16: (1,), 0x17: (0, 1)}.get(cat, ())
    skills.append(dict(key=hex(key), cat=cat, cost=cost, rank=node[0], flags=hex(struct.unpack_from("<I", r, 0x1C)[0]), recs=[(hex(a), gname.get(hex(a)), b) for a, b in recs]))
    for g, v in recs:
        for f in forms:
            add(f, g, v, f"技能{key:04X}")

rows = []
for form, agg in ((0, form0), (1, form1)):
    for g, (v, extra) in sorted(agg.items()):
        known = led.get(("all", g), []) + led.get((form, g), [])
        total = sum(x for x, _ in known)
        rows.append(dict(form=form, group=hex(g), name=gname.get(hex(g)), live=v, extra=extra, known=total, rest=v - total, parts=known))
out.write_text(json.dumps(dict(source=hex(src), rows=rows, skills=skills, nodes=len(nodes)), ensure_ascii=False, indent=0), encoding="utf-8")
print("skills", len(skills))
for r in rows:
    if r["form"] == 0:
        print(r["group"], r["name"], "live", r["live"], "x", r["extra"], "known", r["known"], "rest", r["rest"], [p[1] + "=" + str(p[0]) for p in r["parts"]])
