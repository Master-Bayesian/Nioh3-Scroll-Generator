"""Windows process lifetime identity; access denial is never evidence of exit."""
from __future__ import annotations

import ctypes
import os
from ctypes import wintypes


def creation_time_from_handle(dll, handle) -> str:
    values = [wintypes.FILETIME() for _ in range(4)]
    dll.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    dll.GetProcessTimes.restype = wintypes.BOOL
    if not dll.GetProcessTimes(handle, *(ctypes.byref(value) for value in values)):
        raise ctypes.WinError(ctypes.get_last_error())
    return str((values[0].dwHighDateTime << 32) | values[0].dwLowDateTime)


def process_creation_time(pid: int) -> str | None:
    if os.name != 'nt':
        raise RuntimeError('Process lifetime inspection requires Windows')
    dll = ctypes.WinDLL('kernel32', use_last_error=True)
    dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    dll.OpenProcess.restype = wintypes.HANDLE
    dll.CloseHandle.argtypes = [wintypes.HANDLE]
    dll.CloseHandle.restype = wintypes.BOOL
    dll.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    dll.GetExitCodeProcess.restype = wintypes.BOOL
    handle = dll.OpenProcess(0x1000, False, pid)
    if not handle:
        error = ctypes.get_last_error()
        if error == 87:
            return None
        raise ctypes.WinError(error)
    try:
        code = wintypes.DWORD()
        if not dll.GetExitCodeProcess(handle, ctypes.byref(code)):
            raise ctypes.WinError(ctypes.get_last_error())
        if code.value != 259:
            return None
        return creation_time_from_handle(dll, handle)
    finally:
        dll.CloseHandle(handle)


def original_process_exited(pid: int, creation_time: str | None) -> bool:
    try:
        current = process_creation_time(pid)
        return current is None or (creation_time is not None and current != creation_time)
    except Exception:
        return False
