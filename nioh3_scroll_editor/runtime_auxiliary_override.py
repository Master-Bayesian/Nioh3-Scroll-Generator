"""Temporary runtime overrides for generated scroll auxiliary descriptors.

The game does not serialize enemy groups, terrain, and special rules as
independent fields in the canonical 0xE8 scroll record. This module installs a
version-gated trampoline at the verified descriptor-complete boundary and
rewrites one displayed Seed only while the game process remains alive.

The implementation deliberately reuses native vector storage. It never calls
an unverified game allocator, so a profile cannot request more enemy groups
than the selected Seed already generated.
"""

from __future__ import annotations

import ctypes
import struct
import sys
from ctypes import wintypes
from dataclasses import dataclass

from .native import find_module_base, find_nioh3_pid


MODULE_NAME = "Nioh3.exe"
MODULE_VERSION = "2.00.02"
DESCRIPTOR_COMPLETE_RVA = 0x20DD558
DESCRIPTOR_COMPLETE_BYTES = bytes.fromhex("48 8B 54 24 60")
REMOTE_ALLOCATION_SIZE = 0x1000

PROCESS_ACCESS = 0x0008 | 0x0010 | 0x0020 | 0x0400
MEM_COMMIT_RESERVE = 0x1000 | 0x2000
MEM_RELEASE = 0x8000
MEM_FREE = 0x10000
PAGE_EXECUTE_READWRITE = 0x40
ALLOCATION_GRANULARITY = 0x10000


@dataclass(frozen=True, slots=True)
class RuntimeAuxiliaryOverrideProfile:
    """One non-persistent descriptor override keyed by displayed Seed."""

    seed: int
    enemy_keys: tuple[int, ...] = ()
    special_rule_keys: tuple[int, int, int] | None = None
    terrain_value: int | None = None

    def __post_init__(self) -> None:
        if not 0 <= self.seed <= 0xFFFFFFFF:
            raise ValueError("seed must fit in uint32")
        if len(self.enemy_keys) > 8:
            raise ValueError("at most eight enemy groups are supported")
        if any(not 0 <= key <= 0xFFFFFFFF for key in self.enemy_keys):
            raise ValueError("enemy keys must fit in uint32")
        if self.special_rule_keys is not None:
            if len(self.special_rule_keys) != 3:
                raise ValueError("special_rule_keys must contain exactly three keys")
            if any(not 0 <= key <= 0xFFFF for key in self.special_rule_keys):
                raise ValueError("special-rule keys must fit in uint16")
        if self.terrain_value is not None and not 0 <= self.terrain_value <= 0xFF:
            raise ValueError("terrain_value must fit in uint8")
        if (
            not self.enemy_keys
            and self.special_rule_keys is None
            and self.terrain_value is None
        ):
            raise ValueError("an override profile must change at least one field")


class _CodeBuilder:
    def __init__(self) -> None:
        self.code = bytearray()
        self.labels: dict[str, int] = {}
        self.fixups: list[tuple[int, str]] = []

    def emit(self, data: bytes) -> None:
        self.code.extend(data)

    def branch(self, opcode: bytes, label: str) -> None:
        self.emit(opcode)
        displacement_offset = len(self.code)
        self.emit(b"\x00\x00\x00\x00")
        self.fixups.append((displacement_offset, label))

    def mark(self, label: str) -> None:
        if label in self.labels:
            raise ValueError(f"duplicate code label: {label}")
        self.labels[label] = len(self.code)

    def finish(self) -> bytes:
        for displacement_offset, label in self.fixups:
            target = self.labels[label]
            displacement = target - (displacement_offset + 4)
            struct.pack_into("<i", self.code, displacement_offset, displacement)
        return bytes(self.code)


def build_override_trampoline(
    profile: RuntimeAuxiliaryOverrideProfile,
    *,
    return_address: int,
    counter_address: int | None = None,
) -> bytes:
    """Build position-independent x64 code for one temporary profile."""

    if not 0 <= return_address <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("return_address must fit in uint64")
    if counter_address is not None and not 0 <= counter_address <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("counter_address must fit in uint64")

    builder = _CodeBuilder()
    builder.emit(bytes.fromhex("9C 50 51 52 41 50 41 51 41 52 41 53"))
    builder.emit(bytes.fromhex("41 81 FC") + struct.pack("<I", profile.seed))
    builder.branch(bytes.fromhex("0F 85"), "done")

    if profile.enemy_keys:
        required_bytes = len(profile.enemy_keys) * 0x28
        builder.emit(bytes.fromhex("48 8B 45 00"))  # mov rax,[rbp+0]
        builder.emit(bytes.fromhex("48 85 C0"))  # test rax,rax
        builder.branch(bytes.fromhex("0F 84"), "done")
        builder.emit(bytes.fromhex("48 8B 4D 08"))  # mov rcx,[rbp+8]
        builder.emit(bytes.fromhex("48 39 C1"))  # cmp rcx,rax
        builder.branch(bytes.fromhex("0F 82"), "done")
        builder.emit(bytes.fromhex("48 8B 55 10"))  # mov rdx,[rbp+10]
        builder.emit(bytes.fromhex("48 39 CA"))  # cmp rdx,rcx
        builder.branch(bytes.fromhex("0F 82"), "done")
        builder.emit(bytes.fromhex("49 89 C8 49 29 C0"))  # r8=end-begin
        builder.emit(bytes.fromhex("49 81 F8") + struct.pack("<I", required_bytes))
        builder.branch(bytes.fromhex("0F 82"), "done")

        # Validate every native inner vector before mutating any descriptor byte.
        for group_index in range(len(profile.enemy_keys)):
            group_offset = group_index * 0x28
            builder.emit(bytes.fromhex("4C 8B 80") + struct.pack("<I", group_offset))
            builder.emit(bytes.fromhex("4D 85 C0"))
            builder.branch(bytes.fromhex("0F 84"), "done")
            builder.emit(
                bytes.fromhex("4C 8B 88") + struct.pack("<I", group_offset + 0x08)
            )
            builder.emit(bytes.fromhex("4D 39 C1"))  # cmp r9,r8
            builder.branch(bytes.fromhex("0F 82"), "done")
            builder.emit(
                bytes.fromhex("4C 8B 90") + struct.pack("<I", group_offset + 0x10)
            )
            builder.emit(bytes.fromhex("4D 39 CA"))  # cmp r10,r9
            builder.branch(bytes.fromhex("0F 82"), "done")
            builder.emit(bytes.fromhex("4D 8D 58 14"))  # lea r11,[r8+0x14]
            builder.emit(bytes.fromhex("4D 39 D9"))  # cmp r9,r11
            builder.branch(bytes.fromhex("0F 82"), "done")

        for group_index, enemy_key in enumerate(profile.enemy_keys):
            group_offset = group_index * 0x28
            builder.emit(bytes.fromhex("4C 8B 80") + struct.pack("<I", group_offset))
            builder.emit(bytes.fromhex("41 C7 40 04") + struct.pack("<I", enemy_key))
            builder.emit(bytes.fromhex("4D 8D 48 14"))  # lea r9,[r8+0x14]
            builder.emit(
                bytes.fromhex("4C 89 88") + struct.pack("<I", group_offset + 0x08)
            )
        builder.emit(bytes.fromhex("4C 8D 80") + struct.pack("<I", required_bytes))
        builder.emit(bytes.fromhex("4C 89 45 08"))  # mov [rbp+8],r8

    if profile.special_rule_keys is not None:
        for offset, rule_key in zip(
            (0x18, 0x1A, 0x1C),
            profile.special_rule_keys,
            strict=True,
        ):
            builder.emit(bytes.fromhex("66 C7 45") + bytes((offset,)))
            builder.emit(struct.pack("<H", rule_key))

    if profile.terrain_value is not None:
        builder.emit(bytes.fromhex("C6 45 1F") + bytes((profile.terrain_value,)))

    if counter_address is not None:
        builder.emit(bytes.fromhex("49 BB") + struct.pack("<Q", counter_address))
        builder.emit(bytes.fromhex("F0 49 FF 03"))  # lock inc qword ptr [r11]

    builder.mark("done")
    builder.emit(bytes.fromhex("41 5B 41 5A 41 59 41 58 5A 59 58 9D"))
    builder.emit(DESCRIPTOR_COMPLETE_BYTES)
    builder.emit(bytes.fromhex("48 B8") + struct.pack("<Q", return_address))
    builder.emit(bytes.fromhex("FF E0"))
    return builder.finish()


def build_relative_jump(source_address: int, target_address: int) -> bytes:
    displacement = target_address - (source_address + 5)
    if not -(1 << 31) <= displacement < (1 << 31):
        raise ValueError("trampoline is outside the x64 rel32 jump range")
    return b"\xE9" + struct.pack("<i", displacement)


class MEMORY_BASIC_INFORMATION(ctypes.Structure):
    _fields_ = [
        ("BaseAddress", wintypes.LPVOID),
        ("AllocationBase", wintypes.LPVOID),
        ("AllocationProtect", wintypes.DWORD),
        ("PartitionId", wintypes.WORD),
        ("RegionSize", ctypes.c_size_t),
        ("State", wintypes.DWORD),
        ("Protect", wintypes.DWORD),
        ("Type", wintypes.DWORD),
    ]


def _require_windows() -> None:
    if sys.platform != "win32":
        raise RuntimeError("临时绘卷覆盖仅支持 Windows")


def _kernel32() -> ctypes.WinDLL:
    _require_windows()
    dll = ctypes.WinDLL("kernel32", use_last_error=True)
    dll.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    dll.OpenProcess.restype = wintypes.HANDLE
    dll.CloseHandle.argtypes = [wintypes.HANDLE]
    dll.CloseHandle.restype = wintypes.BOOL
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
    dll.VirtualProtectEx.argtypes = [
        wintypes.HANDLE,
        wintypes.LPVOID,
        ctypes.c_size_t,
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
    ]
    dll.VirtualProtectEx.restype = wintypes.BOOL
    dll.VirtualQueryEx.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCVOID,
        ctypes.POINTER(MEMORY_BASIC_INFORMATION),
        ctypes.c_size_t,
    ]
    dll.VirtualQueryEx.restype = ctypes.c_size_t
    dll.FlushInstructionCache.argtypes = [
        wintypes.HANDLE,
        wintypes.LPCVOID,
        ctypes.c_size_t,
    ]
    dll.FlushInstructionCache.restype = wintypes.BOOL
    return dll


def _last_error(operation: str) -> OSError:
    return ctypes.WinError(ctypes.get_last_error(), operation)


def _pointer_value(value: object) -> int:
    return int(ctypes.cast(value, ctypes.c_void_p).value or 0)


def _align_up(value: int, alignment: int) -> int:
    return (value + alignment - 1) & -alignment


class RuntimeAuxiliaryOverrideSession:
    """Own one installed temporary hook and restore it on stop."""

    def __init__(
        self,
        profile: RuntimeAuxiliaryOverrideProfile,
        *,
        pid: int | None = None,
    ) -> None:
        self.profile = profile
        self.pid = pid
        self.module_base = 0
        self.hook_address = 0
        self.process: int | None = None
        self.allocation: int | None = None
        self.patch: bytes | None = None
        self._installed_once = False
        self.counter_address: int | None = None

    @property
    def active(self) -> bool:
        return bool(self.process and self.allocation and self.patch)

    def _read(self, address: int, size: int) -> bytes:
        if not self.process:
            raise RuntimeError("override session is not open")
        dll = _kernel32()
        buffer = ctypes.create_string_buffer(size)
        transferred = ctypes.c_size_t()
        if not dll.ReadProcessMemory(
            self.process,
            address,
            buffer,
            size,
            ctypes.byref(transferred),
        ):
            raise _last_error("ReadProcessMemory")
        if transferred.value != size:
            raise RuntimeError(f"Short process read: {transferred.value} of {size}")
        return buffer.raw

    def _write(self, address: int, data: bytes) -> None:
        if not self.process:
            raise RuntimeError("override session is not open")
        dll = _kernel32()
        buffer = ctypes.create_string_buffer(data, len(data))
        transferred = ctypes.c_size_t()
        if not dll.WriteProcessMemory(
            self.process,
            address,
            buffer,
            len(data),
            ctypes.byref(transferred),
        ):
            raise _last_error("WriteProcessMemory")
        if transferred.value != len(data):
            raise RuntimeError(f"Short process write: {transferred.value} of {len(data)}")

    def _allocate_near(self, address: int) -> int:
        if not self.process:
            raise RuntimeError("override session is not open")
        dll = _kernel32()
        lower = max(ALLOCATION_GRANULARITY, address - 0x7FFF0000)
        upper = min(0x7FFFFFFFFFFF, address + 0x7FFF0000)
        cursor = lower
        information_size = ctypes.sizeof(MEMORY_BASIC_INFORMATION)
        while cursor < upper:
            information = MEMORY_BASIC_INFORMATION()
            queried = dll.VirtualQueryEx(
                self.process,
                cursor,
                ctypes.byref(information),
                information_size,
            )
            if not queried:
                cursor += ALLOCATION_GRANULARITY
                continue
            base = _pointer_value(information.BaseAddress)
            region_end = base + int(information.RegionSize)
            if information.State == MEM_FREE:
                candidate = _align_up(max(cursor, base), ALLOCATION_GRANULARITY)
                if candidate + REMOTE_ALLOCATION_SIZE <= min(region_end, upper):
                    allocation = dll.VirtualAllocEx(
                        self.process,
                        candidate,
                        REMOTE_ALLOCATION_SIZE,
                        MEM_COMMIT_RESERVE,
                        PAGE_EXECUTE_READWRITE,
                    )
                    if allocation:
                        result = _pointer_value(allocation)
                        build_relative_jump(address, result)
                        return result
            cursor = max(cursor + ALLOCATION_GRANULARITY, region_end)
        raise RuntimeError("无法在游戏函数附近分配安全的 rel32 跳板")

    def _write_hook(self, data: bytes) -> None:
        if not self.process or not self.hook_address:
            raise RuntimeError("override session is not open")
        dll = _kernel32()
        old_protection = wintypes.DWORD()
        if not dll.VirtualProtectEx(
            self.process,
            self.hook_address,
            len(data),
            PAGE_EXECUTE_READWRITE,
            ctypes.byref(old_protection),
        ):
            raise _last_error("VirtualProtectEx")
        try:
            self._write(self.hook_address, data)
            if not dll.FlushInstructionCache(
                self.process,
                self.hook_address,
                len(data),
            ):
                raise _last_error("FlushInstructionCache")
        finally:
            restored = wintypes.DWORD()
            dll.VirtualProtectEx(
                self.process,
                self.hook_address,
                len(data),
                old_protection.value,
                ctypes.byref(restored),
            )

    def start(self) -> None:
        if self.active:
            return
        dll = _kernel32()
        self.pid = find_nioh3_pid() if self.pid is None else self.pid
        self.module_base = find_module_base(self.pid)
        self.hook_address = self.module_base + DESCRIPTOR_COMPLETE_RVA
        process = dll.OpenProcess(PROCESS_ACCESS, False, self.pid)
        if not process:
            raise _last_error("OpenProcess")
        self.process = int(process)
        try:
            actual = self._read(self.hook_address, len(DESCRIPTOR_COMPLETE_BYTES))
            if actual != DESCRIPTOR_COMPLETE_BYTES:
                raise RuntimeError(
                    "绘卷辅助生成函数与《仁王3》PC v2.00.02 不匹配，已拒绝覆盖"
                )
            self.allocation = self._allocate_near(self.hook_address)
            self.counter_address = self.allocation + REMOTE_ALLOCATION_SIZE - 8
            code = build_override_trampoline(
                self.profile,
                return_address=self.hook_address + len(DESCRIPTOR_COMPLETE_BYTES),
                counter_address=self.counter_address,
            )
            if len(code) > REMOTE_ALLOCATION_SIZE - 8:
                raise RuntimeError("runtime override trampoline exceeds its allocation")
            self._write(self.allocation, code)
            self._write(self.counter_address, bytes(8))
            self.patch = build_relative_jump(self.hook_address, self.allocation)
            self._installed_once = True
            self._write_hook(self.patch)
        except Exception:
            self._rollback_start()
            raise

    def _rollback_start(self) -> None:
        if self.process and self.patch is not None:
            try:
                current = self._read(
                    self.hook_address,
                    len(DESCRIPTOR_COMPLETE_BYTES),
                )
                if current == self.patch:
                    self._write_hook(DESCRIPTOR_COMPLETE_BYTES)
                elif current != DESCRIPTOR_COMPLETE_BYTES:
                    # Keep the remote allocation alive if another writer changed
                    # the hook. Freeing it could leave a dangling executable jump.
                    return
            except Exception:
                return
        self._release_session(release_allocation=not self._installed_once)

    def _release_session(self, *, release_allocation: bool) -> None:
        dll = _kernel32()
        if release_allocation and self.process and self.allocation:
            dll.VirtualFreeEx(self.process, self.allocation, 0, MEM_RELEASE)
        if self.process:
            dll.CloseHandle(self.process)
        self.process = None
        self.allocation = None
        self.patch = None
        self._installed_once = False
        self.counter_address = None

    def hit_count(self) -> int:
        if not self.active or self.counter_address is None:
            return 0
        return struct.unpack("<Q", self._read(self.counter_address, 8))[0]

    def stop(self) -> None:
        if not self.process:
            return
        restored = False
        try:
            current = self._read(self.hook_address, len(DESCRIPTOR_COMPLETE_BYTES))
            if self.patch is not None and current == self.patch:
                self._write_hook(DESCRIPTOR_COMPLETE_BYTES)
                restored = True
            elif current == DESCRIPTOR_COMPLETE_BYTES:
                restored = True
            else:
                raise RuntimeError(
                    "游戏 Hook 已被其他程序改写；为避免跳转到已释放内存，"
                    "本程序不会强行覆盖或释放跳板"
                )
        except OSError:
            # A terminated game process releases the remote allocation itself.
            restored = True
        if not restored:
            raise RuntimeError("未能安全移除临时绘卷覆盖")
        # Do not free a trampoline that has ever been live. A game thread may
        # already have passed the restored hook and still be executing inside
        # it. The 4 KiB allocation is intentionally retired until game exit.
        self._release_session(release_allocation=False)

    def __enter__(self) -> "RuntimeAuxiliaryOverrideSession":
        self.start()
        return self

    def __exit__(self, *_args: object) -> None:
        self.stop()


__all__ = [
    "DESCRIPTOR_COMPLETE_BYTES",
    "DESCRIPTOR_COMPLETE_RVA",
    "MODULE_VERSION",
    "RuntimeAuxiliaryOverrideProfile",
    "RuntimeAuxiliaryOverrideSession",
    "build_override_trampoline",
    "build_relative_jump",
]
