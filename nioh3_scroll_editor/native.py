from __future__ import annotations

import ctypes
import struct
import sys
import threading
from ctypes import wintypes
from dataclasses import dataclass, replace
from itertools import islice
from typing import Callable, Iterator

from emaki_exchange import CATEGORY_TO_TYPE, EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE
from .auxiliary_generation import (
    AuxiliarySearchCriteria,
    CompleteAuxiliaryResult,
    generate_complete_auxiliary,
)
from .grace_map import GraceOutputMap, iter_natural_seeds_for_grace, load_grace_output_map
from .grace_map import first_u16_ranges_for_grace
from .joint_solver import DrawConstraint, U16Runs, iter_constraint_intersection

from .models import (
    CandidateRecordStage,
    ScrollCandidate,
    candidate_has_expected_effect_count,
    effective_required_secondary_ids,
)
from .primary_map import PrimaryFirstDrawOutputMap, PrimaryOutputMap


PROCESS_ACCESS = 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0400
MEM_COMMIT_RESERVE = 0x1000 | 0x2000
MEM_RELEASE = 0x8000
PAGE_EXECUTE_READWRITE = 0x40
WAIT_OBJECT_0 = 0
TH32CS_SNAPPROCESS = 0x00000002
TH32CS_SNAPMODULE = 0x00000008
TH32CS_SNAPMODULE32 = 0x00000010
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value
CANONICALIZE_RVA = 0x20DD6EC
FINALIZE_EFFECT_RVA = 0x22799A8
CANONICALIZE_SIGNATURE = bytes.fromhex(
    "48 89 5C 24 18 48 89 7C 24 20 55 48 8D 6C 24 C0"
)
FINALIZE_EFFECT_SIGNATURE = bytes.fromhex(
    "40 53 55 56 57 41 54 41 55 41 56 41 57 48 81 EC 78 01 00 00"
)
INIT_COMPACT_RVA = 0x1B6F650
RESET_COMPACT_RVA = 0x1BBADBC
EFFECTIVE_LEVEL_RVA = 0x3DB834
INIT_GENERATION_CONTEXT_RVA = 0x570DF8
INCOMPLETE_RECORD_RVA = 0x110BF30
GENERATE_EFFECTS_RVA = 0x577964
ASSEMBLE_SCROLL_RVA = 0x2277FE8
PLAYTHROUGH_VECTOR_RVA = 0x578CD4
PLAYTHROUGH_MANAGER_POINTER_RVA = 0x47494A0
REMOTE_CODE_SIZE = 0x400

NATIVE_SIGNATURES = {
    INIT_COMPACT_RVA: bytes.fromhex("40 53 48 83 EC 20 33 C0"),
    RESET_COMPACT_RVA: bytes.fromhex("48 83 EC 28 33 C0 C6 41"),
    EFFECTIVE_LEVEL_RVA: bytes.fromhex("F7 41 18 00 00 20 00 75"),
    INIT_GENERATION_CONTEXT_RVA: bytes.fromhex("48 83 EC 28 4C 8B D9 66"),
    INCOMPLETE_RECORD_RVA: bytes.fromhex("33 D2 48 8D 41 42 80 38"),
    GENERATE_EFFECTS_RVA: bytes.fromhex("48 8B C4 48 89 58 20 55"),
    ASSEMBLE_SCROLL_RVA: bytes.fromhex("48 89 5C 24 08 48 89 6C"),
    PLAYTHROUGH_VECTOR_RVA: bytes.fromhex("48 83 EC 48 48 8B 05 51"),
}


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", wintypes.DWORD),
        ("cntUsage", wintypes.DWORD),
        ("th32ProcessID", wintypes.DWORD),
        ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", wintypes.DWORD),
        ("cntThreads", wintypes.DWORD),
        ("th32ParentProcessID", wintypes.DWORD),
        ("pcPriClassBase", wintypes.LONG),
        ("dwFlags", wintypes.DWORD),
        ("szExeFile", wintypes.WCHAR * 260),
    ]


class MODULEENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", wintypes.DWORD),
        ("th32ModuleID", wintypes.DWORD),
        ("th32ProcessID", wintypes.DWORD),
        ("GlblcntUsage", wintypes.DWORD),
        ("ProccntUsage", wintypes.DWORD),
        ("modBaseAddr", ctypes.POINTER(ctypes.c_byte)),
        ("modBaseSize", wintypes.DWORD),
        ("hModule", wintypes.HMODULE),
        ("szModule", wintypes.WCHAR * 256),
        ("szExePath", wintypes.WCHAR * 260),
    ]


def _require_windows() -> None:
    if sys.platform != "win32":
        raise RuntimeError("原生种子扫描器仅支持 Windows")


def _kernel32() -> ctypes.WinDLL:
    _require_windows()
    dll = ctypes.WinDLL("kernel32", use_last_error=True)
    dll.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    dll.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    dll.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
    dll.Process32FirstW.restype = wintypes.BOOL
    dll.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
    dll.Process32NextW.restype = wintypes.BOOL
    dll.Module32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(MODULEENTRY32W)]
    dll.Module32FirstW.restype = wintypes.BOOL
    dll.Module32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(MODULEENTRY32W)]
    dll.Module32NextW.restype = wintypes.BOOL
    dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    dll.OpenProcess.restype = wintypes.HANDLE
    dll.CloseHandle.argtypes = [wintypes.HANDLE]
    dll.CloseHandle.restype = wintypes.BOOL
    dll.VirtualAllocEx.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        ctypes.c_size_t,
        wintypes.DWORD,
        wintypes.DWORD,
    ]
    dll.VirtualAllocEx.restype = wintypes.LPVOID
    dll.VirtualFreeEx.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        ctypes.c_size_t,
        wintypes.DWORD,
    ]
    dll.VirtualFreeEx.restype = wintypes.BOOL
    dll.ReadProcessMemory.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCVOID,
        wintypes.LPVOID,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    dll.ReadProcessMemory.restype = wintypes.BOOL
    dll.WriteProcessMemory.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        wintypes.LPCVOID,
        ctypes.c_size_t,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    dll.WriteProcessMemory.restype = wintypes.BOOL
    dll.CreateRemoteThread.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        ctypes.c_size_t,
        wintypes.LPVOID,
        wintypes.LPVOID,
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
    ]
    dll.CreateRemoteThread.restype = wintypes.HANDLE
    dll.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    dll.WaitForSingleObject.restype = wintypes.DWORD
    dll.GetExitCodeThread.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    dll.GetExitCodeThread.restype = wintypes.BOOL
    return dll


def _last_error(operation: str) -> OSError:
    return ctypes.WinError(ctypes.get_last_error(), operation)


def find_nioh3_pid() -> int:
    dll = _kernel32()
    snapshot = dll.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if not snapshot or snapshot == INVALID_HANDLE_VALUE:
        raise _last_error("CreateToolhelp32Snapshot(processes)")
    matches: list[int] = []
    try:
        entry = PROCESSENTRY32W()
        entry.dwSize = ctypes.sizeof(entry)
        ok = dll.Process32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            if entry.szExeFile.casefold() == "nioh3.exe":
                matches.append(entry.th32ProcessID)
            ok = dll.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        dll.CloseHandle(snapshot)
    if len(matches) != 1:
        raise RuntimeError(f"应当只运行一个 Nioh3.exe，当前找到 {len(matches)} 个")
    return matches[0]


def find_module_base(pid: int, module_name: str = "Nioh3.exe") -> int:
    dll = _kernel32()
    flags = TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32
    snapshot = dll.CreateToolhelp32Snapshot(flags, pid)
    if not snapshot or snapshot == INVALID_HANDLE_VALUE:
        raise _last_error("CreateToolhelp32Snapshot(modules)")
    try:
        entry = MODULEENTRY32W()
        entry.dwSize = ctypes.sizeof(entry)
        ok = dll.Module32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            if entry.szModule.casefold() == module_name.casefold():
                return ctypes.cast(entry.modBaseAddr, ctypes.c_void_p).value or 0
            ok = dll.Module32NextW(snapshot, ctypes.byref(entry))
    finally:
        dll.CloseHandle(snapshot)
    raise RuntimeError("无法在游戏进程中找到 Nioh3.exe 模块")


def build_batch_wrapper(source: int, destination: int, function: int, count: int) -> bytes:
    if not 1 <= count <= 0xFFFFFFFF:
        raise ValueError("count must fit in uint32 and be nonzero")
    prefix = b"".join(
        (
            bytes.fromhex("53 56 57 41 54 48 83 EC 28"),
            b"\x48\xBB" + struct.pack("<Q", source),
            b"\x48\xBE" + struct.pack("<Q", destination),
            b"\xBF" + struct.pack("<I", count),
            b"\x49\xBC" + struct.pack("<Q", function),
        )
    )
    loop = bytes.fromhex(
        "48 89 F1 "       # mov rcx,rsi
        "48 89 DA "       # mov rdx,rbx
        "41 FF D4 "       # call r12
        "48 81 C3 E8 00 00 00 "
        "48 81 C6 E8 00 00 00 "
        "FF CF"           # dec edi
    )
    jump_back = -(len(loop) + 2)
    if not -128 <= jump_back <= 127:
        raise AssertionError("batch wrapper loop no longer fits a short jump")
    suffix = b"\x75" + struct.pack("<b", jump_back) + bytes.fromhex(
        "31 C0 48 83 C4 28 41 5C 5F 5E 5B C3"
    )
    return prefix + loop + suffix


def build_seed_range_wrapper(
    source: int,
    destination: int,
    function: int,
    start_seed: int,
    seed_step: int,
    count: int,
) -> bytes:
    """Build a remote loop that reuses one source record and increments its seed."""
    if not 0 <= start_seed <= 0xFFFFFFFF:
        raise ValueError("start_seed must fit in uint32")
    if not 1 <= seed_step <= 0xFFFFFFFF:
        raise ValueError("seed_step must be between 1 and 0xFFFFFFFF")
    if not 1 <= count <= 0xFFFFFFFF:
        raise ValueError("count must fit in uint32 and be nonzero")
    prefix = b"".join(
        (
            bytes.fromhex("53 56 57 41 54 41 55 48 83 EC 20"),
            b"\x48\xBB" + struct.pack("<Q", source),
            b"\x48\xBE" + struct.pack("<Q", destination),
            b"\xBF" + struct.pack("<I", count),
            b"\x41\xBD" + struct.pack("<I", start_seed),
            b"\x49\xBC" + struct.pack("<Q", function),
        )
    )
    loop = bytes.fromhex(
        "44 89 6B 20 "    # mov [rbx+0x20],r13d
        "48 89 F1 "       # mov rcx,rsi
        "48 89 DA "       # mov rdx,rbx
        "41 FF D4 "       # call r12
        "48 81 C6 E8 00 00 00 "
    ) + b"\x41\x81\xC5" + struct.pack("<I", seed_step) + bytes.fromhex(
        "FF CF"           # dec edi
    )
    jump_back = -(len(loop) + 2)
    if not -128 <= jump_back <= 127:
        raise AssertionError("seed range wrapper loop no longer fits a short jump")
    suffix = b"\x75" + struct.pack("<b", jump_back) + bytes.fromhex(
        "31 C0 48 83 C4 20 41 5D 41 5C 5F 5E 5B C3"
    )
    return prefix + loop + suffix


def build_effect_finalizer_wrapper(
    source: int,
    destination: int,
    function: int,
    effect_index: int,
    reveal: bool,
) -> bytes:
    """Call the native per-effect finalizer on isolated remote buffers."""
    if not 0 <= effect_index < 7:
        raise ValueError("effect_index must be between 0 and 6")
    return b"".join(
        (
            bytes.fromhex("48 83 EC 28"),
            b"\x48\xB9" + struct.pack("<Q", destination),
            b"\x48\xBA" + struct.pack("<Q", source),
            b"\x41\xB8" + struct.pack("<I", effect_index),
            b"\x41\xB9" + struct.pack("<I", int(reveal)),
            b"\x48\xB8" + struct.pack("<Q", function),
            bytes.fromhex("FF D0 31 C0 48 83 C4 28 C3"),
        )
    )


def build_effect_finalizer_batch_wrapper(
    source: int,
    destination: int,
    function: int,
    count: int,
    effect_index: int,
    reveal: bool,
) -> bytes:
    """Call the same native finalizer index for a contiguous record batch."""
    if not 1 <= count <= 0xFFFFFFFF:
        raise ValueError("count must fit in uint32 and be nonzero")
    if not 0 <= effect_index < 7:
        raise ValueError("effect_index must be between 0 and 6")
    prefix = b"".join(
        (
            bytes.fromhex("53 56 57 41 54 48 83 EC 28"),
            b"\x48\xBB" + struct.pack("<Q", source),
            b"\x48\xBE" + struct.pack("<Q", destination),
            b"\xBF" + struct.pack("<I", count),
            b"\x49\xBC" + struct.pack("<Q", function),
        )
    )
    loop = b"".join(
        (
            bytes.fromhex("48 89 F1 48 89 DA"),
            b"\x41\xB8" + struct.pack("<I", effect_index),
            b"\x41\xB9" + struct.pack("<I", int(reveal)),
            bytes.fromhex(
                "41 FF D4 "
                "48 81 C3 E8 00 00 00 "
                "48 81 C6 E8 00 00 00 "
                "FF CF"
            ),
        )
    )
    jump_back = -(len(loop) + 2)
    if not -128 <= jump_back <= 127:
        raise AssertionError("effect finalizer batch loop no longer fits a short jump")
    suffix = b"\x75" + struct.pack("<b", jump_back) + bytes.fromhex(
        "31 C0 48 83 C4 28 41 5C 5F 5E 5B C3"
    )
    return prefix + loop + suffix


class _MachineCodeBuilder:
    def __init__(self) -> None:
        self.code = bytearray()
        self.labels: dict[str, int] = {}
        self.relative_fixups: list[tuple[int, str]] = []

    def emit(self, data: bytes) -> None:
        self.code.extend(data)

    def mark(self, label: str) -> None:
        if label in self.labels:
            raise ValueError(f"duplicate machine-code label: {label}")
        self.labels[label] = len(self.code)

    def jump32(self, opcode: bytes, label: str) -> None:
        self.emit(opcode)
        displacement_offset = len(self.code)
        self.emit(b"\x00\x00\x00\x00")
        self.relative_fixups.append((displacement_offset, label))

    def finish(self) -> bytes:
        for displacement_offset, label in self.relative_fixups:
            if label not in self.labels:
                raise ValueError(f"missing machine-code label: {label}")
            displacement = self.labels[label] - (displacement_offset + 4)
            struct.pack_into("<i", self.code, displacement_offset, displacement)
        return bytes(self.code)


def _emit_absolute_call(builder: _MachineCodeBuilder, address: int) -> None:
    builder.emit(b"\x48\xB8" + struct.pack("<Q", address) + b"\xFF\xD0")


def build_explicit_playthrough_seed_range_wrapper(
    source: int,
    destination: int,
    module_base: int,
    start_seed: int,
    seed_step: int,
    count: int,
    playthrough: int,
    generation_mode: int = 0,
) -> bytes:
    """Generate a seed range with a temporary 1-5 playthrough context."""
    if not 0 <= start_seed <= 0xFFFFFFFF:
        raise ValueError("start_seed must fit in uint32")
    if not 1 <= seed_step <= 0xFFFFFFFF:
        raise ValueError("seed_step must be between 1 and 0xFFFFFFFF")
    if not 1 <= count <= 0xFFFFFFFF:
        raise ValueError("count must fit in uint32 and be nonzero")
    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be between 1 and 5")
    if generation_mode not in (0, 1):
        raise ValueError("generation_mode must be 0 or 1")

    builder = _MachineCodeBuilder()
    builder.emit(bytes.fromhex("53 56 57 41 54 41 55 41 56 41 57"))
    builder.emit(bytes.fromhex("48 81 EC A0 01 00 00"))
    builder.emit(b"\x48\xBB" + struct.pack("<Q", source))
    builder.emit(b"\x48\xBE" + struct.pack("<Q", destination))
    builder.emit(b"\x41\xBC" + struct.pack("<I", count))
    builder.emit(b"\x41\xBD" + struct.pack("<I", start_seed))
    builder.emit(b"\x41\xBE" + struct.pack("<I", seed_step))
    builder.emit(b"\x41\xBF" + struct.pack("<I", playthrough))

    builder.mark("loop")
    builder.emit(bytes.fromhex("44 89 6B 20"))  # mov [rbx+0x20],r13d

    builder.emit(bytes.fromhex("48 8D 4C 24 60"))
    _emit_absolute_call(builder, module_base + INIT_COMPACT_RVA)
    builder.emit(bytes.fromhex("48 8D 4C 24 60"))
    _emit_absolute_call(builder, module_base + RESET_COMPACT_RVA)

    builder.emit(bytes.fromhex("0F B7 03"))
    builder.emit(bytes.fromhex("66 89 44 24 60"))
    builder.emit(bytes.fromhex("48 89 D9"))
    _emit_absolute_call(builder, module_base + EFFECTIVE_LEVEL_RVA)
    builder.emit(bytes.fromhex("0F B7 C0"))
    builder.emit(bytes.fromhex("89 44 24 64"))
    builder.emit(bytes.fromhex("0F B7 43 10"))
    builder.emit(bytes.fromhex("89 44 24 68"))

    builder.emit(bytes.fromhex("44 0F B6 43 31"))
    builder.emit(bytes.fromhex("41 80 F8 03"))
    builder.jump32(bytes.fromhex("0F 83"), "rarity_ready")
    builder.emit(bytes.fromhex("41 B8 03 00 00 00"))
    builder.mark("rarity_ready")
    builder.emit(bytes.fromhex("44 88 44 24 6C"))

    builder.emit(bytes.fromhex("44 8B 4B 20"))
    builder.emit(bytes.fromhex("44 89 4C 24 70"))
    builder.emit(bytes.fromhex("8B 83 DC 00 00 00"))
    builder.emit(bytes.fromhex("89 44 24 74"))

    builder.emit(bytes.fromhex("44 0F B7 53 02"))
    builder.emit(bytes.fromhex("0F B7 43 04"))
    builder.emit(bytes.fromhex("49 C1 E2 10"))
    builder.emit(bytes.fromhex("49 09 C2"))
    builder.emit(bytes.fromhex("8B 43 14"))
    builder.emit(bytes.fromhex("49 C1 E2 20"))
    builder.emit(bytes.fromhex("49 09 C2"))
    builder.emit(bytes.fromhex("4C 89 54 24 78"))
    builder.emit(bytes.fromhex("8A 43 0F"))
    builder.emit(bytes.fromhex("88 84 24 82 00 00 00"))

    builder.emit(bytes.fromhex("48 8D 4C 24 20"))
    builder.emit(bytes.fromhex("0F B7 13"))
    _emit_absolute_call(builder, module_base + INIT_GENERATION_CONTEXT_RVA)

    builder.emit(bytes.fromhex("31 C0"))
    builder.emit(bytes.fromhex("48 89 84 24 40 01 00 00"))
    builder.emit(bytes.fromhex("48 89 84 24 48 01 00 00"))
    builder.emit(bytes.fromhex("44 88 BC 24 40 01 00 00"))
    builder.emit(
        b"\x48\xB8"
        + struct.pack("<Q", module_base + PLAYTHROUGH_MANAGER_POINTER_RVA)
    )
    builder.emit(bytes.fromhex("48 8B 00"))
    builder.emit(bytes.fromhex("48 8B 40 08"))
    builder.emit(bytes.fromhex("48 89 84 24 48 01 00 00"))
    builder.emit(bytes.fromhex("48 8D 8C 24 40 01 00 00"))
    builder.emit(bytes.fromhex("48 8D 94 24 60 01 00 00"))
    _emit_absolute_call(builder, module_base + PLAYTHROUGH_VECTOR_RVA)
    builder.emit(bytes.fromhex("0F 10 00"))
    builder.emit(bytes.fromhex("0F 11 44 24 44"))
    builder.emit(bytes.fromhex("44 88 7C 24 40"))

    builder.emit(bytes.fromhex("48 89 D9"))
    _emit_absolute_call(builder, module_base + INCOMPLETE_RECORD_RVA)
    builder.emit(bytes.fromhex("C7 44 24 3C 00 00 00 00"))
    builder.emit(bytes.fromhex("84 C0"))
    builder.jump32(bytes.fromhex("0F 84"), "incomplete_ready")
    builder.emit(bytes.fromhex("C7 44 24 3C 00 00 80 3F"))
    builder.mark("incomplete_ready")

    builder.emit(bytes.fromhex("48 8D 4C 24 70"))
    builder.emit(bytes.fromhex("48 8D 94 24 84 00 00 00"))
    builder.emit(bytes.fromhex("4C 8D 44 24 20"))
    if generation_mode == 0:
        builder.emit(bytes.fromhex("45 31 C9"))
    else:
        builder.emit(bytes.fromhex("41 B1 01"))
    _emit_absolute_call(builder, module_base + GENERATE_EFFECTS_RVA)

    builder.emit(bytes.fromhex("48 89 F1"))
    builder.emit(bytes.fromhex("48 8D 54 24 60"))
    _emit_absolute_call(builder, module_base + ASSEMBLE_SCROLL_RVA)

    builder.emit(bytes.fromhex("48 81 C6 E8 00 00 00"))
    builder.emit(bytes.fromhex("45 01 F5"))
    builder.emit(bytes.fromhex("41 FF CC"))
    builder.jump32(bytes.fromhex("0F 85"), "loop")

    builder.emit(bytes.fromhex("31 C0"))
    builder.emit(bytes.fromhex("48 81 C4 A0 01 00 00"))
    builder.emit(bytes.fromhex("41 5F 41 5E 41 5D 41 5C 5F 5E 5B C3"))
    wrapper = builder.finish()
    if len(wrapper) > REMOTE_CODE_SIZE:
        raise AssertionError("explicit playthrough wrapper exceeds the code region")
    return wrapper


def build_source_record(
    template: bytes,
    *,
    seed: int,
    rarity: int,
    level: int,
    recommended_level: int,
    transfer_count: int = 0,
) -> bytes:
    if len(template) != SCROLL_RECORD_SIZE:
        raise ValueError("template must be exactly 0xE8 bytes")
    for name, value, maximum in (
        ("seed", seed, 0xFFFFFFFF),
        ("rarity", rarity, 0x0F),
        ("level", level, 0xFFFF),
        ("recommended_level", recommended_level, 0xFFFF),
        ("transfer_count", transfer_count, 0xFFFFFFFF),
    ):
        if not 0 <= value <= maximum:
            raise ValueError(f"{name} is outside its supported range")
    record = bytearray(template)
    struct.pack_into("<H", record, 0x06, level)
    struct.pack_into("<H", record, 0x08, level)
    struct.pack_into("<H", record, 0x10, recommended_level)
    struct.pack_into("<H", record, 0x12, recommended_level)
    struct.pack_into("<I", record, 0x20, seed)
    record[0x30] = rarity
    record[0x31] = rarity
    struct.pack_into("<I", record, 0xDC, transfer_count)
    return bytes(record)


class NativeBatchOracle:
    def __init__(self, *, pid: int | None = None, max_batch_size: int = 2048) -> None:
        if not 1 <= max_batch_size <= 4096:
            raise ValueError("max_batch_size must be between 1 and 4096")
        self.dll = _kernel32()
        self.pid = find_nioh3_pid() if pid is None else pid
        self.module_base = find_module_base(self.pid)
        self.max_batch_size = max_batch_size
        self.process: int | None = None
        self.allocation: int | None = None
        self.source_address = 0
        self.destination_address = 0

    def __enter__(self) -> "NativeBatchOracle":
        self.open()
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def open(self) -> None:
        if self.process:
            return
        process = self.dll.OpenProcess(PROCESS_ACCESS, False, self.pid)
        if not process:
            raise _last_error("OpenProcess")
        self.process = process
        try:
            function = self.module_base + CANONICALIZE_RVA
            signature = self.read(function, len(CANONICALIZE_SIGNATURE))
            if signature != CANONICALIZE_SIGNATURE:
                raise RuntimeError(
                    "游戏生成器特征与《仁王3》v2.00.02 不匹配，已拒绝调用"
                )
            finalizer = self.module_base + FINALIZE_EFFECT_RVA
            finalizer_signature = self.read(finalizer, len(FINALIZE_EFFECT_SIGNATURE))
            if finalizer_signature != FINALIZE_EFFECT_SIGNATURE:
                raise RuntimeError(
                    "游戏最终化函数特征与《仁王3》v2.00.02 不匹配，已拒绝调用"
                )
            for rva, expected in NATIVE_SIGNATURES.items():
                actual = self.read(self.module_base + rva, len(expected))
                if actual != expected:
                    raise RuntimeError(
                        f"周目生成链特征 {rva:#x} 与《仁王3》v2.00.02 不匹配，已拒绝调用"
                    )
            source_size = self.max_batch_size * SCROLL_RECORD_SIZE
            destination_size = source_size
            allocation_size = REMOTE_CODE_SIZE + source_size + destination_size
            allocation = self.dll.VirtualAllocEx(
                process,
                None,
                allocation_size,
                MEM_COMMIT_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
            if not allocation:
                raise _last_error("VirtualAllocEx")
            self.allocation = int(allocation)
            self.source_address = self.allocation + REMOTE_CODE_SIZE
            self.destination_address = self.source_address + source_size
        except Exception:
            self.close()
            raise

    def close(self) -> None:
        if self.process and self.allocation:
            self.dll.VirtualFreeEx(self.process, self.allocation, 0, MEM_RELEASE)
        if self.process:
            self.dll.CloseHandle(self.process)
        self.process = None
        self.allocation = None

    def read(self, address: int, size: int) -> bytes:
        if not self.process:
            raise RuntimeError("oracle session is not open")
        buffer = ctypes.create_string_buffer(size)
        transferred = ctypes.c_size_t()
        if not self.dll.ReadProcessMemory(
            self.process, address, buffer, size, ctypes.byref(transferred)
        ):
            raise _last_error("ReadProcessMemory")
        if transferred.value != size:
            raise RuntimeError(f"Short process read: {transferred.value} of {size}")
        return buffer.raw

    def write(self, address: int, data: bytes) -> None:
        if not self.process:
            raise RuntimeError("oracle session is not open")
        buffer = ctypes.create_string_buffer(data)
        transferred = ctypes.c_size_t()
        if not self.dll.WriteProcessMemory(
            self.process, address, buffer, len(data), ctypes.byref(transferred)
        ):
            raise _last_error("WriteProcessMemory")
        if transferred.value != len(data):
            raise RuntimeError(f"Short process write: {transferred.value} of {len(data)}")

    def generate(self, source_records: list[bytes], *, timeout_ms: int = 60_000) -> list[bytes]:
        if not self.process or not self.allocation:
            raise RuntimeError("oracle session is not open")
        if not source_records or len(source_records) > self.max_batch_size:
            raise ValueError("source_records must fit in the configured batch")
        if any(len(record) != SCROLL_RECORD_SIZE for record in source_records):
            raise ValueError("every source record must be exactly 0xE8 bytes")
        count = len(source_records)
        payload = b"".join(source_records)
        output_size = count * SCROLL_RECORD_SIZE
        wrapper = build_batch_wrapper(
            self.source_address,
            self.destination_address,
            self.module_base + CANONICALIZE_RVA,
            count,
        )
        self.write(self.source_address, payload)
        self.write(self.destination_address, bytes(output_size))
        self.write(self.allocation, wrapper)

        thread_id = wintypes.DWORD()
        thread = self.dll.CreateRemoteThread(
            self.process,
            None,
            0,
            self.allocation,
            None,
            0,
            ctypes.byref(thread_id),
        )
        if not thread:
            raise _last_error("CreateRemoteThread")
        try:
            wait_result = self.dll.WaitForSingleObject(thread, timeout_ms)
            if wait_result != WAIT_OBJECT_0:
                raise RuntimeError(f"等待游戏原生生成器失败：{wait_result:#x}")
            exit_code = wintypes.DWORD()
            if not self.dll.GetExitCodeThread(thread, ctypes.byref(exit_code)):
                raise _last_error("GetExitCodeThread")
            if exit_code.value != 0:
                raise RuntimeError(f"游戏原生生成器线程返回异常：{exit_code.value:#x}")
        finally:
            self.dll.CloseHandle(thread)
        output = self.read(self.destination_address, output_size)
        records = [
            output[index:index + SCROLL_RECORD_SIZE]
            for index in range(0, output_size, SCROLL_RECORD_SIZE)
        ]
        if any(struct.unpack_from("<H", record, 0)[0] == 0 for record in records):
            raise RuntimeError("游戏原生生成器返回了空记录")
        return records

    def finalize_effect_stage(
        self,
        source_record: bytes,
        *,
        effect_index: int,
        reveal: bool = True,
        timeout_ms: int = 60_000,
    ) -> bytes:
        """Run the native completion-time effect finalizer without touching the save."""
        if not self.process or not self.allocation:
            raise RuntimeError("oracle session is not open")
        if len(source_record) != SCROLL_RECORD_SIZE:
            raise ValueError("source_record must be exactly 0xE8 bytes")
        if not 0 <= effect_index < 7:
            raise ValueError("effect_index must be between 0 and 6")

        wrapper = build_effect_finalizer_wrapper(
            self.source_address,
            self.destination_address,
            self.module_base + FINALIZE_EFFECT_RVA,
            effect_index,
            reveal,
        )
        self.write(self.source_address, source_record)
        self.write(self.destination_address, bytes(SCROLL_RECORD_SIZE))
        self.write(self.allocation, wrapper)

        thread_id = wintypes.DWORD()
        thread = self.dll.CreateRemoteThread(
            self.process,
            None,
            0,
            self.allocation,
            None,
            0,
            ctypes.byref(thread_id),
        )
        if not thread:
            raise _last_error("CreateRemoteThread")
        try:
            wait_result = self.dll.WaitForSingleObject(thread, timeout_ms)
            if wait_result != WAIT_OBJECT_0:
                raise RuntimeError(f"等待游戏原生最终化函数失败：{wait_result:#x}")
            exit_code = wintypes.DWORD()
            if not self.dll.GetExitCodeThread(thread, ctypes.byref(exit_code)):
                raise _last_error("GetExitCodeThread")
            if exit_code.value != 0:
                raise RuntimeError(f"游戏原生最终化线程返回异常：{exit_code.value:#x}")
        finally:
            self.dll.CloseHandle(thread)

        record = self.read(self.destination_address, SCROLL_RECORD_SIZE)
        if struct.unpack_from("<H", record, 0)[0] == 0:
            raise RuntimeError("游戏原生最终化函数返回了空记录")
        return record

    def finalize_effect_stage_batch(
        self,
        source_records: list[bytes],
        *,
        effect_index: int,
        reveal: bool = True,
        timeout_ms: int = 60_000,
    ) -> list[bytes]:
        """Run one completion-time effect index for a contiguous record batch."""
        if not self.process or not self.allocation:
            raise RuntimeError("oracle session is not open")
        if not source_records or len(source_records) > self.max_batch_size:
            raise ValueError("source_records must fit in the configured batch")
        if any(len(record) != SCROLL_RECORD_SIZE for record in source_records):
            raise ValueError("every source record must be exactly 0xE8 bytes")
        if not 0 <= effect_index < 7:
            raise ValueError("effect_index must be between 0 and 6")

        count = len(source_records)
        payload = b"".join(source_records)
        output_size = count * SCROLL_RECORD_SIZE
        wrapper = build_effect_finalizer_batch_wrapper(
            self.source_address,
            self.destination_address,
            self.module_base + FINALIZE_EFFECT_RVA,
            count,
            effect_index,
            reveal,
        )
        self.write(self.source_address, payload)
        self.write(self.destination_address, bytes(output_size))
        self.write(self.allocation, wrapper)

        thread_id = wintypes.DWORD()
        thread = self.dll.CreateRemoteThread(
            self.process,
            None,
            0,
            self.allocation,
            None,
            0,
            ctypes.byref(thread_id),
        )
        if not thread:
            raise _last_error("CreateRemoteThread")
        try:
            wait_result = self.dll.WaitForSingleObject(thread, timeout_ms)
            if wait_result != WAIT_OBJECT_0:
                raise RuntimeError(f"等待游戏原生批量最终化函数失败：{wait_result:#x}")
            exit_code = wintypes.DWORD()
            if not self.dll.GetExitCodeThread(thread, ctypes.byref(exit_code)):
                raise _last_error("GetExitCodeThread")
            if exit_code.value != 0:
                raise RuntimeError(f"游戏原生批量最终化线程返回异常：{exit_code.value:#x}")
        finally:
            self.dll.CloseHandle(thread)

        output = self.read(self.destination_address, output_size)
        records = [
            output[index : index + SCROLL_RECORD_SIZE]
            for index in range(0, output_size, SCROLL_RECORD_SIZE)
        ]
        if any(struct.unpack_from("<H", record, 0)[0] == 0 for record in records):
            raise RuntimeError("游戏原生批量最终化函数返回了空记录")
        return records

    def finalize_stage_record(
        self,
        source_record: bytes,
        *,
        reveal: bool = True,
        timeout_ms: int = 60_000,
    ) -> tuple[bytes, int | None]:
        """Mirror the game's completion loop and return the first final record."""
        if len(source_record) != SCROLL_RECORD_SIZE:
            raise ValueError("source_record must be exactly 0xE8 bytes")

        for effect_index in range(7):
            effect_offset = EFFECT_START + effect_index * EFFECT_STRIDE
            if struct.unpack_from("<H", source_record, effect_offset)[0] == 0:
                continue
            if source_record[effect_offset + 0x0D] & 0x40:
                continue
            if source_record[effect_offset + 0x0E] & 0x04:
                continue
            candidate = self.finalize_effect_stage(
                source_record,
                effect_index=effect_index,
                reveal=reveal,
                timeout_ms=timeout_ms,
            )
            if candidate[effect_offset + 0x0E] & 0x04:
                return candidate, effect_index
        return source_record, None

    def generate_seed_range(
        self,
        template: bytes,
        *,
        start_seed: int,
        seed_step: int,
        count: int,
        playthrough: int | None = None,
        generation_mode: int = 0,
        timeout_ms: int = 60_000,
    ) -> list[bytes]:
        if not self.process or not self.allocation:
            raise RuntimeError("oracle session is not open")
        if len(template) != SCROLL_RECORD_SIZE:
            raise ValueError("template must be exactly 0xE8 bytes")
        if not 1 <= count <= self.max_batch_size:
            raise ValueError("count must fit in the configured batch")
        if not 0 <= start_seed <= 0xFFFFFFFF:
            raise ValueError("start_seed must fit in uint32")
        if not 1 <= seed_step <= 0xFFFFFFFF:
            raise ValueError("seed_step must be between 1 and 0xFFFFFFFF")
        if playthrough is not None and not 1 <= playthrough <= 5:
            raise ValueError("playthrough must be between 1 and 5, or None")
        if generation_mode not in (0, 1):
            raise ValueError("generation_mode must be 0 or 1")
        if playthrough is None and generation_mode != 0:
            raise ValueError("generation_mode 1 requires an explicit playthrough context")

        output_size = count * SCROLL_RECORD_SIZE
        if playthrough is None:
            wrapper = build_seed_range_wrapper(
                self.source_address,
                self.destination_address,
                self.module_base + CANONICALIZE_RVA,
                start_seed,
                seed_step,
                count,
            )
        else:
            manager_pointer = struct.unpack(
                "<Q", self.read(self.module_base + PLAYTHROUGH_MANAGER_POINTER_RVA, 8)
            )[0]
            if manager_pointer == 0:
                raise RuntimeError("游戏周目管理器尚未初始化")
            table_pointer = struct.unpack("<Q", self.read(manager_pointer + 8, 8))[0]
            if table_pointer == 0:
                raise RuntimeError("游戏周目参数表尚未初始化")
            wrapper = build_explicit_playthrough_seed_range_wrapper(
                self.source_address,
                self.destination_address,
                self.module_base,
                start_seed,
                seed_step,
                count,
                playthrough,
                generation_mode,
            )
        self.write(self.source_address, template)
        self.write(self.destination_address, bytes(output_size))
        self.write(self.allocation, wrapper)

        thread_id = wintypes.DWORD()
        thread = self.dll.CreateRemoteThread(
            self.process,
            None,
            0,
            self.allocation,
            None,
            0,
            ctypes.byref(thread_id),
        )
        if not thread:
            raise _last_error("CreateRemoteThread")
        try:
            wait_result = self.dll.WaitForSingleObject(thread, timeout_ms)
            if wait_result != WAIT_OBJECT_0:
                raise RuntimeError(f"等待游戏原生生成器失败：{wait_result:#x}")
            exit_code = wintypes.DWORD()
            if not self.dll.GetExitCodeThread(thread, ctypes.byref(exit_code)):
                raise _last_error("GetExitCodeThread")
            if exit_code.value != 0:
                raise RuntimeError(f"游戏原生生成器线程返回异常：{exit_code.value:#x}")
        finally:
            self.dll.CloseHandle(thread)

        output = self.read(self.destination_address, output_size)
        records = [
            output[index:index + SCROLL_RECORD_SIZE]
            for index in range(0, output_size, SCROLL_RECORD_SIZE)
        ]
        if any(struct.unpack_from("<H", record, 0)[0] == 0 for record in records):
            raise RuntimeError("游戏原生生成器返回了空记录")
        return records


@dataclass(frozen=True, slots=True)
class ScanProgress:
    scanned: int
    current_seed: int


def _require_grace_acceleration_context(
    *,
    template: bytes,
    rarity: int,
    playthrough: int | None,
    grace_output_map: GraceOutputMap | None = None,
) -> GraceOutputMap:
    """Load the measured first-draw Grace map for the requested search.

    Rarity 4 and 5 use their own measured maps.  Rarity 3 has no direct Grace
    slot, so the experimental growing-effect accelerator intentionally reuses
    the rarity-4 map as a prediction for what slot-5 ``0x0001`` may resolve to.
    All three paths remain restricted to the E604/current-loaded-state context.
    """
    map_rarity = 4 if rarity == 3 else rarity
    if map_rarity not in (4, 5):
        raise ValueError("Grace-accelerated scanning currently supports rarity 3, 4, or 5")
    mapping = grace_output_map or load_grace_output_map(rarity=map_rarity)
    record_type = struct.unpack_from("<H", template, 0)[0]
    if record_type != mapping.record_type:
        raise ValueError(
            "Grace-accelerated scanning is verified only for "
            f"record type {mapping.record_type:#06X}; template is {record_type:#06X}"
        )
    if mapping.rarity != map_rarity:
        raise ValueError("特殊结果逆向映射的稀有度与当前生成条件不匹配")
    expected_slot = 5 if map_rarity == 4 else 6
    if mapping.effect_slot != expected_slot:
        raise ValueError("特殊结果逆向映射的槽位与当前生成条件不匹配")
    if grace_output_map is None and playthrough not in (None, 3):
        raise ValueError(
            "Grace inverse maps are available only for the category-3/E604 context"
        )
    if playthrough is not None and CATEGORY_TO_TYPE[playthrough] != record_type:
        raise ValueError("特殊结果逆向映射的绘卷类型与所选周目不匹配")
    if grace_output_map is not None and playthrough not in (3, 4, 5):
        raise ValueError("实时特殊结果映射仅适用于三至五周目")
    return mapping


def _require_experimental_slot5_grace_context(
    *, template: bytes, rarity: int, playthrough: int | None
) -> None:
    """Guard exact-seed/raw-range rarity-3/4 growing-effect experiments."""
    record_type = struct.unpack_from("<H", template, 0)[0]
    if record_type != 0xE604:
        raise ValueError("Experimental slot-5 grace filtering requires an E604 template")
    if rarity not in (3, 4):
        raise ValueError("Experimental slot-5 grace filtering supports raw rarity 3 or 4")
    if playthrough not in (None, 3):
        raise ValueError(
            "Experimental slot-5 Grace filtering requires category 3/E604"
        )


def _candidate_matches_scan_filters(
    record: bytes,
    *,
    rarity: int,
    playthrough: int | None,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
    required_secondary_id_groups: tuple[frozenset[int], ...] = (),
    grace_effect_id: int | None = None,
    grace_effect_slot: int = 6,
    required_slot5_effect_id: int | None = None,
    auxiliary_criteria: AuxiliarySearchCriteria | None = None,
) -> ScrollCandidate | None:
    candidate = ScrollCandidate.from_record(
        record,
        playthrough=playthrough,
        record_stage=(
            CandidateRecordStage.FINAL_RECORD
            if rarity == 5
            else CandidateRecordStage.NATIVE_STAGE_ONE
        ),
    )
    if not candidate_has_expected_effect_count(candidate, rarity):
        return None
    primary_id = struct.unpack_from("<I", record, EFFECT_START + 4)[0]
    if primary_effect_ids and primary_id not in primary_effect_ids:
        return None
    if grace_effect_id is not None:
        if not 1 <= grace_effect_slot <= 7:
            raise ValueError("grace_effect_slot must be between 1 and 7")
        actual_grace_id = struct.unpack_from(
            "<I", record, EFFECT_START + (grace_effect_slot - 1) * EFFECT_STRIDE + 4
        )[0]
        # Accelerated-map contradictions fail closed instead of silently being
        # treated as ordinary misses.  This catches stale or wrong-context maps.
        if actual_grace_id != grace_effect_id:
            raise RuntimeError(
                "Native grace output contradicts the accelerated seed prediction: "
                f"expected {grace_effect_id:#x}, got {actual_grace_id:#x} "
                f"in slot {grace_effect_slot}"
            )
    if required_slot5_effect_id is not None:
        actual_slot5_id = struct.unpack_from(
            "<I", record, EFFECT_START + 4 * EFFECT_STRIDE + 4
        )[0]
        if actual_slot5_id != required_slot5_effect_id:
            return None
    effective_required_secondary = effective_required_secondary_ids(
        primary_id=primary_id,
        primary_effect_ids=primary_effect_ids,
        required_secondary_ids=required_secondary_ids,
    )
    if effective_required_secondary or required_secondary_id_groups:
        # When slot 5 is reserved for the growing effect / rarity-4 Grace, only
        # slots 2-4 are ordinary secondary slots.  Otherwise preserve the
        # established rarity-derived ordinary-slot bounds.
        secondary_stop = (
            4
            if required_slot5_effect_id is not None or grace_effect_slot == 5 and grace_effect_id is not None
            else min(max(rarity + 1, 1), 5)
        )
        secondary_ids = {
            struct.unpack_from("<I", record, EFFECT_START + index * EFFECT_STRIDE + 4)[0]
            for index in range(1, secondary_stop)
        }
        if not effective_required_secondary.issubset(secondary_ids):
            return None
        ordinary_match_ids = set(secondary_ids)
        if not primary_effect_ids:
            ordinary_match_ids.add(primary_id)
        if any(
            not group.intersection(ordinary_match_ids)
            for group in required_secondary_id_groups
        ):
            return None
    if auxiliary_criteria is not None and not auxiliary_criteria.is_empty:
        if playthrough is None:
            raise ValueError("auxiliary constraints require an explicit playthrough")
        auxiliary = generate_complete_auxiliary(candidate.seed, playthrough)
        if not auxiliary_criteria.matches(auxiliary):
            return None
        candidate = replace(candidate, auxiliary=auxiliary)
    return candidate


def _prefilter_auxiliary_items(
    items: list[object],
    *,
    seed_of: Callable[[object], int],
    playthrough: int | None,
    criteria: AuxiliarySearchCriteria | None,
) -> tuple[list[object], dict[int, CompleteAuxiliaryResult]]:
    """Apply the game-closed auxiliary generator before native effect calls."""

    if criteria is None or criteria.is_empty:
        return items, {}
    if playthrough is None:
        raise ValueError("auxiliary constraints require an explicit playthrough")
    accepted: list[object] = []
    results: dict[int, CompleteAuxiliaryResult] = {}
    for item in items:
        seed = seed_of(item)
        auxiliary = generate_complete_auxiliary(seed, playthrough)
        if criteria.matches(auxiliary):
            accepted.append(item)
            results[seed] = auxiliary
    return accepted, results


def _shadow_r4_grace_for_seed(
    oracle: NativeBatchOracle,
    template: bytes,
    *,
    seed: int,
    level: int,
    recommended_level: int,
) -> int:
    source = build_source_record(
        template,
        seed=seed,
        rarity=4,
        level=level,
        recommended_level=recommended_level,
    )
    shadow = oracle.generate([source])[0]
    actual_seed = struct.unpack_from("<I", shadow, 0x20)[0]
    if actual_seed != seed:
        raise RuntimeError("Native batch oracle changed an experimental shadow seed")
    return struct.unpack_from("<I", shadow, EFFECT_START + 4 * EFFECT_STRIDE + 4)[0]


def scan_next_candidate(
    oracle: NativeBatchOracle,
    *,
    template: bytes,
    start_seed: int,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
    required_secondary_id_groups: tuple[frozenset[int], ...] = (),
    grace_effect_id: int | None = None,
    rarity: int = 5,
    level: int = 180,
    recommended_level: int = 183,
    playthrough: int | None = None,
    seed_step: int = 1,
    max_seeds: int = 1_000_000,
    grace_start_after_seed: int | None = None,
    primary_output_map: PrimaryOutputMap | None = None,
    primary_first_output_map: PrimaryFirstDrawOutputMap | None = None,
    grace_output_map: GraceOutputMap | None = None,
    joint_start_after_trial: int = 0,
    accelerate_grace: bool = True,
    cancel_event: threading.Event | None = None,
    progress: Callable[[ScanProgress], None] | None = None,
    auxiliary_criteria: AuxiliarySearchCriteria | None = None,
) -> ScrollCandidate | None:
    if not 0 <= start_seed <= 0xFFFFFFFF:
        raise ValueError("start_seed must fit in uint32")
    if not 1 <= seed_step <= 0xFFFFFFFF:
        raise ValueError("seed_step must be between 1 and 0xFFFFFFFF")
    if max_seeds <= 0:
        raise ValueError("max_seeds must be positive")
    if playthrough is not None and not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be between 1 and 5, or None")
    if grace_effect_id is not None and not 0 <= grace_effect_id <= 0xFFFFFFFF:
        raise ValueError("grace_effect_id must fit in uint32, or be None")
    if grace_effect_id is None and grace_start_after_seed is not None:
        raise ValueError("grace_start_after_seed is only valid when a grace target is selected")
    if grace_start_after_seed is not None and not accelerate_grace:
        raise ValueError("grace_start_after_seed requires accelerated Grace scanning")
    if joint_start_after_trial < 0:
        raise ValueError("joint_start_after_trial cannot be negative")
    if (
        joint_start_after_trial
        and primary_output_map is None
        and primary_first_output_map is None
    ):
        raise ValueError("joint_start_after_trial requires a primary output map")
    if primary_output_map is not None and grace_start_after_seed is not None:
        raise ValueError("joint search resumes by pivot trial, not Grace seed cursor")
    if primary_first_output_map is not None and grace_start_after_seed is not None:
        raise ValueError("primary-only solving resumes by pivot trial")
    if primary_output_map is not None and primary_first_output_map is not None:
        raise ValueError("select exactly one primary output map")
    if grace_output_map is not None and grace_effect_id is None:
        raise ValueError("a live special-result map requires a selected special result")
    if primary_output_map is not None and (
        grace_effect_id is None or not primary_effect_ids or rarity != 5 or not accelerate_grace
    ):
        raise ValueError(
            "primary output maps require accelerated rarity-5 Grace and a primary target"
        )
    if primary_first_output_map is not None and (
        grace_effect_id is not None or not primary_effect_ids
    ):
        raise ValueError("first-draw primary maps require a primary target without Grace")

    if primary_first_output_map is not None:
        record_type = struct.unpack_from("<H", template, 0)[0]
        if (
            primary_first_output_map.game_version != "2.00.02"
            or primary_first_output_map.record_type != record_type
            or primary_first_output_map.rarity != rarity
            or primary_first_output_map.category != playthrough
            or primary_first_output_map.draw_index != 1
        ):
            raise ValueError("主词条 draw-1 映射与所选周目/稀有度不匹配")
        primary_runs = primary_first_output_map.runs_for_effects(primary_effect_ids)
        solutions = iter_constraint_intersection(
            (DrawConstraint("primary", 1, primary_runs),),
            natural_only=True,
            start_after_trial=joint_start_after_trial,
        )
        checked = 0
        while checked < max_seeds:
            if cancel_event and cancel_event.is_set():
                return None
            raw_batch = list(
                islice(solutions, min(oracle.max_batch_size, max_seeds - checked))
            )
            if not raw_batch:
                return None
            checked += len(raw_batch)
            batch, auxiliary_by_seed = _prefilter_auxiliary_items(
                raw_batch,
                seed_of=lambda item: item.seed,
                playthrough=playthrough,
                criteria=auxiliary_criteria,
            )
            if not batch:
                if progress:
                    progress(ScanProgress(scanned=checked, current_seed=raw_batch[-1].seed))
                continue
            sources = [
                build_source_record(
                    template,
                    seed=solution.seed,
                    rarity=rarity,
                    level=level,
                    recommended_level=recommended_level,
                )
                for solution in batch
            ]
            generated = oracle.generate(sources)
            if len(generated) != len(batch):
                raise RuntimeError("游戏原生生成器返回了错误数量的联立候选")
            for solution, record in zip(batch, generated, strict=True):
                actual_seed = struct.unpack_from("<I", record, 0x20)[0]
                if actual_seed != solution.seed:
                    raise RuntimeError("游戏原生生成器改变了联立求解 Seed")
                candidate = _candidate_matches_scan_filters(
                    record,
                    rarity=rarity,
                    playthrough=playthrough,
                    primary_effect_ids=primary_effect_ids,
                    required_secondary_ids=required_secondary_ids,
                    required_secondary_id_groups=required_secondary_id_groups,
                    grace_effect_id=None,
                    auxiliary_criteria=None,
                )
                if candidate is not None:
                    return replace(
                        candidate,
                        joint_search_trial=solution.pivot_trial,
                        auxiliary=auxiliary_by_seed.get(solution.seed),
                    )
            if progress:
                progress(ScanProgress(scanned=checked, current_seed=raw_batch[-1].seed))
        return None

    # Complete fixed-prefix solver for the verified NG3 rarity-5 path:
    # draw 1 = Grace and draw 2 = primary.  Only their exact intersection is
    # sent to the game; path-dependent secondary effects remain native-verified.
    if primary_output_map is not None:
        mapping = _require_grace_acceleration_context(
            template=template,
            rarity=rarity,
            playthrough=playthrough,
            grace_output_map=grace_output_map,
        )
        if (
            primary_output_map.game_version != "2.00.02"
            or primary_output_map.record_type != mapping.record_type
            or primary_output_map.rarity != rarity
            or primary_output_map.playthrough != mapping.playthrough
            or primary_output_map.grace_effect_id != grace_effect_id
            or primary_output_map.grace_effect_slot != mapping.effect_slot
            or primary_output_map.draw_index != 2
        ):
            raise ValueError("主词条映射与当前游戏/恩宠生成上下文不匹配")
        grace_runs = U16Runs.from_ranges(
            (entry.start, entry.end)
            for entry in first_u16_ranges_for_grace(grace_effect_id, mapping)
        )
        primary_runs = primary_output_map.runs_for_effects(primary_effect_ids)
        solutions = iter_constraint_intersection(
            (
                DrawConstraint("grace", 1, grace_runs),
                DrawConstraint("primary", 2, primary_runs),
            ),
            natural_only=True,
            start_after_trial=joint_start_after_trial,
        )
        scanned = 0
        while scanned < max_seeds:
            if cancel_event and cancel_event.is_set():
                return None
            raw_batch = list(
                islice(solutions, min(oracle.max_batch_size, max_seeds - scanned))
            )
            if not raw_batch:
                return None
            scanned += len(raw_batch)
            batch, auxiliary_by_seed = _prefilter_auxiliary_items(
                raw_batch,
                seed_of=lambda item: item.seed,
                playthrough=playthrough,
                criteria=auxiliary_criteria,
            )
            if not batch:
                if progress:
                    progress(ScanProgress(scanned=scanned, current_seed=raw_batch[-1].seed))
                continue
            sources = [
                build_source_record(
                    template,
                    seed=solution.seed,
                    rarity=rarity,
                    level=level,
                    recommended_level=recommended_level,
                )
                for solution in batch
            ]
            generated = oracle.generate(sources)
            if len(generated) != len(batch):
                raise RuntimeError("游戏原生生成器返回了错误数量的联立候选")
            for solution, record in zip(batch, generated, strict=True):
                actual_seed = struct.unpack_from("<I", record, 0x20)[0]
                if actual_seed != solution.seed:
                    raise RuntimeError("游戏原生生成器改变了联立搜索 Seed")
                candidate = _candidate_matches_scan_filters(
                    record,
                    rarity=rarity,
                    playthrough=playthrough,
                    primary_effect_ids=primary_effect_ids,
                    required_secondary_ids=required_secondary_ids,
                    required_secondary_id_groups=required_secondary_id_groups,
                    grace_effect_id=grace_effect_id,
                    grace_effect_slot=mapping.effect_slot,
                    auxiliary_criteria=None,
                )
                if candidate is not None:
                    return replace(
                        candidate,
                        joint_search_trial=solution.pivot_trial,
                        auxiliary=auxiliary_by_seed.get(solution.seed),
                    )
            if progress:
                progress(ScanProgress(scanned=scanned, current_seed=raw_batch[-1].seed))
        return None

    # Accelerated Grace path.  R4 and R5 use their own first-u16 maps.  R3
    # enumerates seeds from the R4 map, generates only the real R3 candidates,
    # then performs one R4 shadow validation only after the ordinary filters and
    # slot-5 0x0001 have already matched.
    if grace_effect_id is not None and accelerate_grace and rarity in (3, 4, 5):
        mapping = _require_grace_acceleration_context(
            template=template,
            rarity=rarity,
            playthrough=playthrough,
            grace_output_map=grace_output_map,
        )
        grace_seeds = iter_natural_seeds_for_grace(
            grace_effect_id,
            mapping,
            start_after_seed=grace_start_after_seed,
        )
        scanned = 0
        while scanned < max_seeds:
            if cancel_event and cancel_event.is_set():
                return None
            seed_batch = list(
                islice(grace_seeds, min(oracle.max_batch_size, max_seeds - scanned))
            )
            if not seed_batch:
                return None
            source_records = [
                build_source_record(
                    template,
                    seed=first_draw.seed,
                    rarity=rarity,
                    level=level,
                    recommended_level=recommended_level,
                )
                for first_draw in seed_batch
            ]
            generated = oracle.generate(source_records)
            if len(generated) != len(seed_batch):
                raise RuntimeError("Native batch oracle returned an unexpected record count")

            if rarity in (4, 5):
                for first_draw, record in zip(seed_batch, generated, strict=True):
                    actual_seed = struct.unpack_from("<I", record, 0x20)[0]
                    if actual_seed != first_draw.seed:
                        raise RuntimeError(
                            "Native batch oracle changed an accelerated source seed: "
                            f"expected {first_draw.seed:#x}, got {actual_seed:#x}"
                        )
                    candidate = _candidate_matches_scan_filters(
                        record,
                        rarity=rarity,
                        playthrough=playthrough,
                        primary_effect_ids=primary_effect_ids,
                        required_secondary_ids=required_secondary_ids,
                        required_secondary_id_groups=required_secondary_id_groups,
                        grace_effect_id=grace_effect_id,
                        grace_effect_slot=mapping.effect_slot,
                        auxiliary_criteria=auxiliary_criteria,
                    )
                    if candidate is not None:
                        return candidate
            else:
                # R3 stores 0x0001 in slot 5, not the predicted Grace itself.
                # Only after an R3 record passes every real filter do we spend a
                # native call validating its same-seed R4 shadow.
                for first_draw, record in zip(seed_batch, generated, strict=True):
                    actual_seed = struct.unpack_from("<I", record, 0x20)[0]
                    if actual_seed != first_draw.seed:
                        raise RuntimeError(
                            "Native batch oracle changed an accelerated source seed: "
                            f"expected {first_draw.seed:#x}, got {actual_seed:#x}"
                        )
                    candidate = _candidate_matches_scan_filters(
                        record,
                        rarity=3,
                        playthrough=playthrough,
                        primary_effect_ids=primary_effect_ids,
                        required_secondary_ids=required_secondary_ids,
                        required_secondary_id_groups=required_secondary_id_groups,
                        grace_effect_id=None,
                        required_slot5_effect_id=0x0001,
                        auxiliary_criteria=auxiliary_criteria,
                    )
                    if candidate is None:
                        continue
                    shadow_grace = _shadow_r4_grace_for_seed(
                        oracle,
                        template,
                        seed=candidate.seed,
                        level=level,
                        recommended_level=recommended_level,
                    )
                    if shadow_grace != grace_effect_id:
                        raise RuntimeError(
                            "Rarity-3 growing-effect prediction contradicts the same-seed "
                            f"rarity-4 native shadow: expected {grace_effect_id:#x}, "
                            f"got {shadow_grace:#x}"
                        )
                    return replace(candidate, predicted_growth_grace_id=shadow_grace)

            scanned += len(seed_batch)
            if progress:
                progress(ScanProgress(scanned=scanned, current_seed=seed_batch[-1].seed))
        return None

    # Non-accelerated Grace path is retained for exact-seed experiments from
    # the UI.  It never interprets the numeric start seed as an inverted-map
    # cursor.  R3 still uses a same-seed R4 shadow; R4 checks slot 5 directly.
    if grace_effect_id is not None:
        if rarity in (3, 4):
            _require_experimental_slot5_grace_context(
                template=template, rarity=rarity, playthrough=playthrough
            )
        elif rarity == 5:
            _require_grace_acceleration_context(
                template=template,
                rarity=rarity,
                playthrough=playthrough,
                grace_output_map=grace_output_map,
            )
        else:
            raise ValueError("Grace filtering currently supports rarity 3, 4, or 5")

        scanned = 0
        while scanned < max_seeds:
            if cancel_event and cancel_event.is_set():
                return None
            count = min(oracle.max_batch_size, max_seeds - scanned)
            batch_start = (start_seed + scanned * seed_step) & 0xFFFFFFFF
            source_template = build_source_record(
                template,
                seed=batch_start,
                rarity=rarity,
                level=level,
                recommended_level=recommended_level,
            )
            generated = oracle.generate_seed_range(
                source_template,
                start_seed=batch_start,
                seed_step=seed_step,
                count=count,
                playthrough=playthrough,
            )
            if rarity in (4, 5):
                grace_slot = 5 if rarity == 4 else 6
                for record in generated:
                    candidate = _candidate_matches_scan_filters(
                        record,
                        rarity=rarity,
                        playthrough=playthrough,
                        primary_effect_ids=primary_effect_ids,
                        required_secondary_ids=required_secondary_ids,
                        required_secondary_id_groups=required_secondary_id_groups,
                        grace_effect_id=grace_effect_id,
                        grace_effect_slot=grace_slot,
                        auxiliary_criteria=auxiliary_criteria,
                    )
                    if candidate is not None:
                        return candidate
            else:
                for record in generated:
                    candidate = _candidate_matches_scan_filters(
                        record,
                        rarity=3,
                        playthrough=playthrough,
                        primary_effect_ids=primary_effect_ids,
                        required_secondary_ids=required_secondary_ids,
                        required_secondary_id_groups=required_secondary_id_groups,
                        grace_effect_id=None,
                        required_slot5_effect_id=0x0001,
                        auxiliary_criteria=auxiliary_criteria,
                    )
                    if candidate is None:
                        continue
                    shadow_grace = _shadow_r4_grace_for_seed(
                        oracle,
                        template,
                        seed=candidate.seed,
                        level=level,
                        recommended_level=recommended_level,
                    )
                    if shadow_grace == grace_effect_id:
                        return replace(candidate, predicted_growth_grace_id=shadow_grace)
            scanned += count
            if progress:
                progress(
                    ScanProgress(
                        scanned=scanned,
                        current_seed=(batch_start + (count - 1) * seed_step) & 0xFFFFFFFF,
                    )
                )
        return None

    # Ordinary unfiltered seed scan.
    scanned = 0
    while scanned < max_seeds:
        if cancel_event and cancel_event.is_set():
            return None
        count = min(oracle.max_batch_size, max_seeds - scanned)
        batch_start = (start_seed + scanned * seed_step) & 0xFFFFFFFF
        source_template = build_source_record(
            template,
            seed=batch_start,
            rarity=rarity,
            level=level,
            recommended_level=recommended_level,
        )
        generated = oracle.generate_seed_range(
            source_template,
            start_seed=batch_start,
            seed_step=seed_step,
            count=count,
            playthrough=playthrough,
        )
        for record in generated:
            candidate = _candidate_matches_scan_filters(
                record,
                rarity=rarity,
                playthrough=playthrough,
                primary_effect_ids=primary_effect_ids,
                required_secondary_ids=required_secondary_ids,
                required_secondary_id_groups=required_secondary_id_groups,
                grace_effect_id=None,
                auxiliary_criteria=auxiliary_criteria,
            )
            if candidate is not None:
                return candidate
        scanned += count
        if progress:
            progress(
                ScanProgress(
                    scanned=scanned,
                    current_seed=(batch_start + (count - 1) * seed_step) & 0xFFFFFFFF,
                )
            )
    return None
