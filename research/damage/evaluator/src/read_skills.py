"""List the player's learned skills from the running game (read-only ReadProcessMemory).

Progress object = [[Nioh3.exe+0x4751860]] (9D2B74 reads the level at +0x69DC from it). 9C12E0 looks a skill up in
the MSVC unordered_map at progress+0x5D98 and returns the rank at node+0x14; source_talents.py reads the second map
at +0x5DE8 the same way. Each map header holds the list sentinel pointer at +8 and the element count at +0x10; list
nodes hold next (+0), prev (+8), the key (+0x10, u32) and the rank (+0x14, u32). Names: skill_key_u16 in
SKILL_SOURCE_CATALOG.json. argv: <pid> <out.json>
"""
import ctypes
import ctypes.wintypes as W
import json
import struct
import sys
from pathlib import Path

CATALOG = Path("F:/Nioh3_r16_build/deliverables/r16-native-buffs-20261019/catalog/skill-batch-01/SKILL_SOURCE_CATALOG.json")
PROGRESS_GLOBAL_RVA = 0x4751860
MAPS = {"map5d98": 0x5D98, "map5de8": 0x5DE8}


def main():
    pid, out = int(sys.argv[1]), Path(sys.argv[2])
    k = ctypes.WinDLL("kernel32", use_last_error=True)
    k.OpenProcess.restype = W.HANDLE
    k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
    k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
    psapi = ctypes.WinDLL("psapi")
    h = k.OpenProcess(0x0410, False, pid)
    if not h:
        raise SystemExit("cannot open the game process")

    def read(address, size):
        buf, n = ctypes.create_string_buffer(size), ctypes.c_size_t()
        if not k.ReadProcessMemory(h, ctypes.c_void_p(address), buf, size, ctypes.byref(n)) or n.value != size:
            raise RuntimeError(f"read failed at {address:#x}")
        return buf.raw

    q = lambda a: struct.unpack("<Q", read(a, 8))[0]
    modules = (ctypes.c_void_p * 1024)()
    needed = W.DWORD()
    psapi.EnumProcessModulesEx.argtypes = [W.HANDLE, ctypes.c_void_p, W.DWORD, ctypes.POINTER(W.DWORD), W.DWORD]
    psapi.EnumProcessModulesEx(h, modules, ctypes.sizeof(modules), ctypes.byref(needed), 3)
    base = modules[0]
    progress = q(q(base + PROGRESS_GLOBAL_RVA))
    names = {s["skill_key_u16"]: s.get("native_name_zh") for s in json.loads(CATALOG.read_bytes())["skills"]}
    result = {"module_base": hex(base), "progress": hex(progress), "level": struct.unpack("<I", read(progress + 0x69DC, 4))[0]}
    for label, offset in MAPS.items():
        header = read(progress + offset, 0x40)
        sentinel, count = struct.unpack_from("<QQ", header, 8)
        rows, node = [], q(sentinel)
        while node != sentinel and len(rows) <= count + 1:
            key, rank = struct.unpack("<II", read(node + 0x10, 8))
            rows.append({"key": key, "key_hex": hex(key), "rank": rank, "name": names.get(key & 0xFFFF)})
            node = q(node)
        result[label] = {"count": count, "walked": len(rows), "skills": rows}
    out.write_text(json.dumps(result, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps({label: (result[label]["count"], result[label]["walked"]) for label in MAPS} | {"level": result["level"]}))


if __name__ == "__main__":
    main()
