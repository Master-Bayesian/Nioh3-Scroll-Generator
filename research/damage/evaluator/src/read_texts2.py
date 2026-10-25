"""Resolve text ids from a window of the live text pool (read-only). argv: <pid> <pool_address_hex> <ids.json> <out.json>"""
import ctypes, ctypes.wintypes as W, json, struct, sys
K = ctypes.WinDLL("kernel32", use_last_error=True)
K.OpenProcess.restype = W.HANDLE
K.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
K.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
h = K.OpenProcess(0x0410, False, int(sys.argv[1]))
center, ids = int(sys.argv[2], 16), json.load(open(sys.argv[3]))
lo, size = center - (256 << 20), 512 << 20
parts = []
for off in range(0, size, 1 << 20):
    buf = ctypes.create_string_buffer(1 << 20); n = ctypes.c_size_t()
    ok = K.ReadProcessMemory(h, ctypes.c_void_p(lo + off), buf, 1 << 20, ctypes.byref(n))
    parts.append(buf.raw[:n.value].ljust(1 << 20, b"\0") if ok else bytes(1 << 20))
data = b"".join(parts)
out = {}
for tid in ids:
    key = struct.pack("<I", int(tid, 16))
    j = data.find(key)
    while j >= 0:
        units = struct.unpack_from("<I", data, j + 4)[0]
        if 1 <= units <= 400 and data[j + 6 + 2 * units:j + 8 + 2 * units] == b"\0\0":
            try:
                out[tid] = data[j + 8:j + 6 + 2 * units].decode("utf-16-le")
                break
            except UnicodeDecodeError:
                pass
        j = data.find(key, j + 1)
json.dump(out, open(sys.argv[4], "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(json.dumps({"wanted": len(ids), "resolved": len(out)}))
