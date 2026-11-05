"""Walk the whole native text tree and save {id: text}. argv: pid out.json"""
import ctypes, ctypes.wintypes as W, json, struct, sys
from pathlib import Path
pid, out = int(sys.argv[1]), Path(sys.argv[2])
k = ctypes.WinDLL("kernel32", use_last_error=True)
k.OpenProcess.restype = W.HANDLE; k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
psapi = ctypes.WinDLL("psapi")
psapi.EnumProcessModulesEx.argtypes = [W.HANDLE, ctypes.c_void_p, W.DWORD, ctypes.POINTER(W.DWORD), W.DWORD]
h = k.OpenProcess(0x0410, False, pid)
buf = ctypes.create_string_buffer(0x1000); n_ = ctypes.c_size_t()
def read(a, s):
    if not k.ReadProcessMemory(h, ctypes.c_void_p(a), buf, s, ctypes.byref(n_)) or n_.value != s:
        raise RuntimeError(hex(a))
    return buf.raw[:s]
q = lambda a: struct.unpack("<Q", read(a, 8))[0]
mods = (ctypes.c_void_p * 1024)(); need = W.DWORD()
psapi.EnumProcessModulesEx(h, mods, ctypes.sizeof(mods), ctypes.byref(need), 3)
base = mods[0]
root = q(q(base + 0x45C8450) + 0x1CD0)
stack, res, bad = [q(root + 8)], {}, 0
while stack:
    n = stack.pop()
    raw = read(n, 0x30)
    if raw[0x19]:
        continue
    l, r = struct.unpack_from("<Q", raw, 0)[0], struct.unpack_from("<Q", raw, 0x10)[0]
    key, p = struct.unpack_from("<I", raw, 0x20)[0], struct.unpack_from("<Q", raw, 0x28)[0]
    stack += [l, r]
    if p:
        try:
            s = read(p, 0x600)
        except RuntimeError:
            try:
                s = read(p, 0x100)
            except RuntimeError:
                bad += 1; continue
        e = 0
        while e < len(s) - 1 and s[e:e + 2] != b"\0\0":
            e += 2
        res[hex(key)] = s[:e].decode("utf-16le", "replace")
out.write_text(json.dumps(res, ensure_ascii=False, indent=0), encoding="utf-8")
print("texts", len(res), "bad", bad)
