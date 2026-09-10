"""Read-only Windows x64 thread-stack ownership for captured stack pointers."""
import argparse
import ctypes as C
from ctypes import wintypes as W
import json
from pathlib import Path
import struct


class ThreadEntry(C.Structure):
    _fields_ = [('size', W.DWORD), ('usage', W.DWORD), ('tid', W.DWORD), ('pid', W.DWORD),
                ('priority', W.LONG), ('delta', W.LONG), ('flags', W.DWORD)]


def stack_owners(pid, pointers):
    kernel = C.WinDLL('kernel32', use_last_error=True)
    nt = C.WinDLL('ntdll')
    for name, args, result in (
        ('CreateToolhelp32Snapshot', [W.DWORD, W.DWORD], W.HANDLE),
        ('Thread32First', [W.HANDLE, C.POINTER(ThreadEntry)], W.BOOL),
        ('Thread32Next', [W.HANDLE, C.POINTER(ThreadEntry)], W.BOOL),
        ('OpenThread', [W.DWORD, W.BOOL, W.DWORD], W.HANDLE),
        ('OpenProcess', [W.DWORD, W.BOOL, W.DWORD], W.HANDLE),
        ('ReadProcessMemory', [W.HANDLE, C.c_void_p, C.c_void_p, C.c_size_t, C.POINTER(C.c_size_t)], W.BOOL),
        ('CloseHandle', [W.HANDLE], W.BOOL),
    ):
        function = getattr(kernel, name); function.argtypes = args; function.restype = result
    nt.NtQueryInformationThread.argtypes = [W.HANDLE, W.ULONG, C.c_void_p, W.ULONG, C.POINTER(W.ULONG)]
    nt.NtQueryInformationThread.restype = W.LONG
    process = kernel.OpenProcess(0x410, False, pid)
    if not process: raise C.WinError(C.get_last_error())
    snapshot = kernel.CreateToolhelp32Snapshot(4, 0)
    if snapshot in (None, C.c_void_p(-1).value):
        kernel.CloseHandle(process); raise C.WinError(C.get_last_error())
    rows = []
    try:
        entry = ThreadEntry(); entry.size = C.sizeof(entry)
        more = kernel.Thread32First(snapshot, C.byref(entry))
        while more:
            if entry.pid == pid:
                thread = kernel.OpenThread(0x40, False, entry.tid)
                if thread:
                    try:
                        # Validate the native x64 basic-information layout against
                        # the requested PID/TID and NT_TIB.Self before using it.
                        info = C.create_string_buffer(48); length = W.ULONG()
                        status = nt.NtQueryInformationThread(thread, 0, info, 48, C.byref(length))
                        if status == 0 and length.value == 48:
                            teb, owner_pid, owner_tid = struct.unpack_from('<QQQ', info.raw, 8)
                            if owner_pid == pid and owner_tid == entry.tid and teb:
                                tib = C.create_string_buffer(0x38); read = C.c_size_t()
                                if kernel.ReadProcessMemory(process, teb, tib, 0x38, C.byref(read)) and read.value == 0x38:
                                    base, limit = struct.unpack_from('<QQ', tib.raw, 8)
                                    self_pointer = struct.unpack_from('<Q', tib.raw, 0x30)[0]
                                    if self_pointer == teb and 0 < limit < base:
                                        rows.append({'thread_id': entry.tid, 'teb': hex(teb), 'stack_base': hex(base), 'stack_limit': hex(limit),
                                                     'matched_pointers': [hex(value) for value in pointers if limit <= value < base]})
                    finally: kernel.CloseHandle(thread)
            more = kernel.Thread32Next(snapshot, C.byref(entry))
    finally:
        kernel.CloseHandle(snapshot); kernel.CloseHandle(process)
    return {'schema': 'nioh3-observed-stack-owners/v1', 'pid': pid, 'read_only': True,
            'validated_thread_count': len(rows), 'matches': [row for row in rows if row['matched_pointers']],
            'unresolved': [hex(value) for value in pointers if not any(hex(value) in row['matched_pointers'] for row in rows)],
            'scope': 'TEB stack bounds at capture time; not proof that remote-thread invocation is safe'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--pid', type=int, required=True)
    parser.add_argument('--stack', type=lambda value:int(value,0), action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = stack_owners(args.pid, args.stack)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report))
