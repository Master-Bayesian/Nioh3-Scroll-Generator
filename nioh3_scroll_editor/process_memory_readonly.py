"""Read-only Windows process access shared by runtime inspection."""
import ctypes
import struct
import sys
from .native import find_module_base, find_nioh3_pid
from .runtime_catalog_probe import _kernel32, _read_process_memory, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ

class ProcessReader:
    def __init__(self) -> None:
        if sys.platform != "win32":
            raise RuntimeError("Process inspection requires Windows")
        self.pid = find_nioh3_pid()
        self.module_base = find_module_base(self.pid)
        self.dll = _kernel32()
        self.handle = self.dll.OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            False,
            self.pid,
        )
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())

    def close(self) -> None:
        if self.handle:
            self.dll.CloseHandle(self.handle)
            self.handle = None

    def __enter__(self) -> "ProcessReader":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def read(self, address: int, size: int, *, exact: bool = True) -> bytes:
        data = _read_process_memory(self.dll, self.handle, address, size)
        if exact and len(data) != size:
            raise RuntimeError(
                f"ReadProcessMemory({address:#x}, {size:#x}) returned {len(data):#x} bytes"
            )
        return data

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.read(address, 4))[0]

    def u64(self, address: int) -> int:
        return struct.unpack("<Q", self.read(address, 8))[0]

    def creation_time(self) -> str:
        """Creation FILETIME disambiguates a reused Windows process ID."""
        from ctypes import wintypes
        values = [wintypes.FILETIME() for _ in range(4)]
        self.dll.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
        self.dll.GetProcessTimes.restype = wintypes.BOOL
        if not self.dll.GetProcessTimes(self.handle, *(ctypes.byref(value) for value in values)):
            raise ctypes.WinError(ctypes.get_last_error())
        return str((values[0].dwHighDateTime << 32) | values[0].dwLowDateTime)
