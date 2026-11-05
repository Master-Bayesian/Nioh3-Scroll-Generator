"""Title blessings: table manager+0x7E0 (stride 0x110) ranked by the map at progress+0x5EA8. argv: pid out.json"""
import ctypes, ctypes.wintypes as W, json, struct, sys
from pathlib import Path
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
mods = (ctypes.c_void_p * 1024)(); need = W.DWORD()
psapi.EnumProcessModulesEx(h, mods, ctypes.sizeof(mods), ctypes.byref(need), 3)
base = mods[0]
prog = q(q(base + 0x4751860)); mgr = q(base + 0x45B9E30)
texts = json.loads(Path("F:/Nioh3_r16_build/work/live-1104/ALL_TEXTS.json").read_bytes())
src = json.loads(Path("F:/Nioh3_r16_build/work/live-1104/SOURCES.json").read_bytes())
g = src["gname"]
sent, cnt = struct.unpack("<QQ", read(prog + 0x5EB0, 16))
print("sentinel check header", read(prog + 0x5EA8, 0x38).hex(), "sent", hex(sent), cnt)
ranks = {}
sent = q(prog + 0x5EB0); cnt = q(prog + 0x5EB8)
n = q(sent)
while n != sent and len(ranks) <= cnt + 1:
    raw = read(n, 0x20)
    ranks[struct.unpack_from("<H", raw, 0x10)[0]] = struct.unpack_from("<I", raw, 0x18)[0]
    n = q(n)
print("ranked", cnt, ranks)
flat = q(q(mgr + 0x7E0)); count = struct.unpack("<I", read(flat + 4, 4))[0]
rows = []
for i in range(count):
    r = read(flat + 8 + i * 0x110, 0x110)
    key = struct.unpack_from("<H", r, 0x80)[0]; group = struct.unpack_from("<H", r, 0x88)[0]
    vals = list(struct.unpack_from("<16I", r, 4))
    tx = {hex(o): texts[hex(v)] for o in range(0, 0x110, 4) for v in [struct.unpack_from("<I", r, o)[0]] if hex(v) in texts}
    rank = ranks.get(key, 0)
    rows.append(dict(i=i, key=hex(key), group=hex(group), gname=g.get(hex(group)), rank=rank, value=vals[rank] if rank < 16 else None, vals=vals, texts=tx, hex=r.hex()))
    print(i, hex(key), hex(group), g.get(hex(group)), "rank", rank, "val", rows[-1]["value"], vals[:11], {a: b[:30] for a, b in tx.items()})
out.write_text(json.dumps(dict(count=count, ranks={hex(a): b for a, b in ranks.items()}, rows=rows), ensure_ascii=False, indent=0), encoding="utf-8")
