"""Read-only, no debugger: find attribute sources by the attribute pattern, dump them and their STAT. argv: pid out_dir"""
import ctypes, ctypes.wintypes as W, json, struct, sys
from pathlib import Path
pid, out = int(sys.argv[1]), Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
k = ctypes.WinDLL("kernel32", use_last_error=True)
k.OpenProcess.restype = W.HANDLE; k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_ulonglong), ("AllocationBase", ctypes.c_ulonglong), ("AllocationProtect", W.DWORD), ("PartitionId", W.WORD),
                ("RegionSize", ctypes.c_ulonglong), ("State", W.DWORD), ("Protect", W.DWORD), ("Type", W.DWORD)]
k.VirtualQueryEx.argtypes = [W.HANDLE, ctypes.c_ulonglong, ctypes.POINTER(MBI), ctypes.c_size_t]
k.VirtualQueryEx.restype = ctypes.c_size_t
h = k.OpenProcess(0x0410, False, pid)
buf = ctypes.create_string_buffer(0x100000); got = ctypes.c_size_t()
def read(a, s):
    b, n = ctypes.create_string_buffer(s), ctypes.c_size_t()
    if not k.ReadProcessMemory(h, ctypes.c_void_p(a), b, s, ctypes.byref(n)) or n.value != s:
        raise RuntimeError(hex(a))
    return b.raw
pat = struct.pack("<7I", 37, 150, 27, 26, 80, 15, 150)
hits = []
a = 0; m = MBI()
while k.VirtualQueryEx(h, a, ctypes.byref(m), ctypes.sizeof(m)):
    if m.State == 0x1000 and not (m.Protect & 0x101) and m.Type != 0x1000000:
        at, end = m.BaseAddress, m.BaseAddress + m.RegionSize
        while at < end:
            n = min(0x100000, end - at)
            k.ReadProcessMemory(h, ctypes.c_void_p(at), buf, n, ctypes.byref(got))
            size = got.value
            if size:
                d = buf.raw[:size]
                i = d.find(pat)
                while i >= 0:
                    hits.append(at + i)
                    i = d.find(pat, i + 1)
            at += size or 0x1000
    a = m.BaseAddress + m.RegionSize
srcs = []
for x in hits:
    try:
        if read(x + 0x20, 28) == pat:
            srcs.append(x - 0x14)
    except RuntimeError:
        pass
print("sources", [hex(s) for s in srcs])
keep = {}
for src in srcs:
    try:
        c = read(src, 0x6000)
    except RuntimeError as e:
        print("short", hex(src)); continue
    counts = [struct.unpack_from("<I", c, off + 0xC00)[0] for off in (0x35D0, 0x4728)]
    print(hex(src), "form", struct.unpack_from("<i", c, 0x2C)[0], "counts", counts, "guardians", [struct.unpack_from("<IHH", c, 0x58 + i * 8) for i in range(2)], "active", c[0x70], c[0x71], struct.unpack_from("<H", c, 0x72)[0],
          "cores", [hex(struct.unpack_from("<H", c, 0x80 + i * 0xE8)[0]) for i in range(8)])
    try:
        s = read(src - 0x810, 0x900)
        ptr = struct.unpack_from("<Q", s, 0x7D8)[0]
        print("   stat ok" if ptr == src else "   stat?", hex(ptr), "rec", [struct.unpack_from("<3I", s, 0x24 + 64 * i) for i in range(3)], "def", struct.unpack_from("<I", s, 0x1C8)[0], "weight", struct.unpack_from("<I", s, 0x7B8)[0], "12c", struct.unpack_from("<I", s, 0x12C)[0], "hp", struct.unpack_from("<I", s, 0x18)[0])
        if ptr == src:
            keep[src] = True
            (out / f"stat_{src:x}.bin").write_bytes(s)
            (out / f"attr_{src:x}.bin").write_bytes(c)
    except RuntimeError:
        print("   no stat")
