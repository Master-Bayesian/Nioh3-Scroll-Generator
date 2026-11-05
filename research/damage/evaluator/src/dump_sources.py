"""Dump the non-equipment parameter tables read by the source compiler 556118, with texts and group names. argv: pid out"""
import ctypes, ctypes.wintypes as W, json, struct, sys
from pathlib import Path
P = Path("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021")
sys.path.insert(0, str(P / "src")); sys.path.insert(0, str(P))
from outfit_model import OutfitModel  # noqa: E402
from regress_fast import BASE as CAP, CACHE  # noqa: E402

pid, out = int(sys.argv[1]), Path(sys.argv[2])
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
prog = q(q(base + 0x4751860))
mgr = q(base + 0x45B9E30)

# Native text tree (5A4B04 -> 63D9C4 -> 63DDA0): left +0, right +0x10, nil byte +0x19, key u32 +0x20, utf16 ptr +0x28.
root = q(q(base + 0x45C8450) + 0x1CD0)
tree = q(root + 8)
cache = {}
def text(i):
    if i in cache:
        return cache[i]
    n, best = tree, None
    for _ in range(64):
        raw = read(n, 0x30)
        if raw[0x19]:
            break
        key = struct.unpack_from("<I", raw, 0x20)[0]
        if key >= i:
            best, nb = (key, struct.unpack_from("<Q", raw, 0x28)[0]), None
            n = struct.unpack_from("<Q", raw, 0)[0]
        else:
            n = struct.unpack_from("<Q", raw, 0x10)[0]
    r = None
    if best and best[0] == i and best[1]:
        s = read(best[1], 400)
        e = 0
        while e < 398 and s[e:e + 2] != b"\0\0":
            e += 2
        r = s[:e].decode("utf-16le", "replace")
    cache[i] = r
    return r

m = OutfitModel(CACHE, CAP / "capture.json", observe_only=True)
T = m.T
loc = json.loads(Path("F:/Nioh3_ScrollEditor/nioh3_scroll_editor/data/effect_names_multilingual.json").read_bytes())["effects"]
gname = {}
for eid in T.EFFECTS.indices:
    g = struct.unpack_from("<H", T.EFFECTS.row(eid), 2)[0]
    n = loc.get(f"0x{eid:08X}", {}).get("names", {}).get("zh-CN")
    if g and n:
        gname.setdefault(g, n)

def texts_in(row):
    found = {}
    for o in range(0, len(row) - 3, 4):
        v = struct.unpack_from("<I", row, o)[0]
        if 0x3000000 <= v < 0x3F00000 or 0x100000 <= v < 0x8000000:
            t = text(v)
            if t:
                found[hex(o)] = t
    return found

def table(off, stride):
    flat = q(q(mgr + off))
    count = u(flat + 4)
    return flat, count, read(flat + 8, count * stride)

result = {"base": hex(base), "progress": hex(prog)}
for off, stride in ((0x7E0, 0xC0), (0x278, 0xD8), (0x268, 0x38), (0x3F8, 0x5C), (0x400, 0x24), (0x118, 0x54)):
    try:
        flat, count, raw = table(off, stride)
    except RuntimeError as e:
        print(hex(off), "unreadable", e); continue
    rows = []
    for i in range(count):
        r = raw[i * stride:(i + 1) * stride]
        rows.append({"i": i, "hex": r.hex(), "texts": texts_in(r)})
    result[hex(off)] = {"count": count, "stride": stride, "rows": rows}
    print(hex(off), count, "first texts:", [x["texts"] for x in rows[:4]])

slots = []
for i in range(8):
    v = q(prog + 0x6340 + i * 16)
    slots.append({"i": i, "raw": hex(v), "group": hex(v & 0xFFFF), "value": v >> 32, "name": gname.get(v & 0xFFFF)})
result["slots6340"] = slots
print(slots)
result["slot60a0"] = [read(prog + 0x60A0 + i * 8, 8).hex() for i in range(0x18)]
result["gname"] = {hex(a): b for a, b in gname.items()}
out.write_text(json.dumps(result, ensure_ascii=False, indent=0), encoding="utf-8")
