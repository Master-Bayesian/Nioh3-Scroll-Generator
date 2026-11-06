"""Test save only, owner approved 11-06: replace every copy of the seven attributes and its level word. Verifies each read-back.
argv: pid old7(comma) new7(comma)"""
import ctypes, ctypes.wintypes as W, struct, sys
pid = int(sys.argv[1]); old = [int(x) for x in sys.argv[2].split(",")]; new = [int(x) for x in sys.argv[3].split(",")]
old_level, new_level = sum(old) - 35, sum(new) - 35
k = ctypes.WinDLL("kernel32", use_last_error=True)
k.OpenProcess.restype = W.HANDLE; k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k.WriteProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_ulonglong), ("AllocationBase", ctypes.c_ulonglong), ("AllocationProtect", W.DWORD), ("PartitionId", W.WORD),
                ("RegionSize", ctypes.c_ulonglong), ("State", W.DWORD), ("Protect", W.DWORD), ("Type", W.DWORD)]
k.VirtualQueryEx.argtypes = [W.HANDLE, ctypes.c_ulonglong, ctypes.POINTER(MBI), ctypes.c_size_t]
k.VirtualQueryEx.restype = ctypes.c_size_t
h = k.OpenProcess(0x0438, False, pid)
buf = ctypes.create_string_buffer(0x100000); got = ctypes.c_size_t()
pat = struct.pack("<7I", *old); hits = []
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
                    hits.append((at + i, i >= 4 and struct.unpack_from("<I", d, i - 4)[0] == old_level))
                    i = d.find(pat, i + 1)
            at += size or 0x1000
    a = m.BaseAddress + m.RegionSize
print("copies", len(hits), "with level", sum(x[1] for x in hits))
def write(addr, data):
    n = ctypes.c_size_t()
    if not k.WriteProcessMemory(h, ctypes.c_void_p(addr), data, len(data), ctypes.byref(n)) or n.value != len(data):
        raise RuntimeError(hex(addr))
    b = ctypes.create_string_buffer(len(data))
    k.ReadProcessMemory(h, ctypes.c_void_p(addr), b, len(data), ctypes.byref(n))
    if b.raw != data:
        raise RuntimeError("read-back " + hex(addr))
for addr, level in hits:
    write(addr, struct.pack("<7I", *new))
    if level:
        write(addr - 4, struct.pack("<I", new_level))
print("written", new, "level", new_level)
