"""Fail-closed compatibility checks for the bundled offline generation data."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from dataclasses import dataclass
import os
from pathlib import Path
import re


SUPPORTED_GAME_VERSION = "2.00.02"


@dataclass(frozen=True, slots=True)
class GameCompatibilityStatus:
    state: str
    detail: str
    executable: Path | None = None

    @property
    def supported(self) -> bool:
        return self.state == "supported"

    @property
    def known_mismatch(self) -> bool:
        return self.state == "unsupported"


def _steam_roots() -> tuple[Path, ...]:
    roots: list[Path] = []
    override = os.environ.get("NIOH3_GAME_EXE", "").strip()
    if override:
        override_path = Path(override)
        roots.append(
            override_path.parent.parent.parent
            if override_path.name.casefold() == "nioh3.exe"
            else override_path
        )

    program_files_x86 = os.environ.get("ProgramFiles(x86)")
    if program_files_x86:
        roots.append(Path(program_files_x86) / "Steam")
    roots.extend((Path(r"C:\Program Files (x86)\Steam"), Path(r"D:\Steam")))

    try:
        import winreg

        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam") as key:
            roots.append(Path(winreg.QueryValueEx(key, "SteamPath")[0]))
    except (ImportError, OSError):
        pass
    return tuple(dict.fromkeys(path.resolve() for path in roots))


def discover_game_executables() -> tuple[Path, ...]:
    candidates: list[Path] = []
    override = os.environ.get("NIOH3_GAME_EXE", "").strip()
    if override:
        candidates.append(Path(override))

    library_roots: list[Path] = []
    for steam_root in _steam_roots():
        library_roots.append(steam_root)
        library_file = steam_root / "steamapps" / "libraryfolders.vdf"
        try:
            text = library_file.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        for match in re.finditer(r'"path"\s*"([^"]+)"', text, flags=re.IGNORECASE):
            library_roots.append(Path(match.group(1).replace(r"\\", "\\")))

    for root in dict.fromkeys(path.resolve() for path in library_roots):
        candidates.append(root / "steamapps" / "common" / "Nioh3" / "Nioh3.exe")
    return tuple(
        dict.fromkeys(path.resolve() for path in candidates if path.is_file())
    )


class _FixedFileInfo(ctypes.Structure):
    _fields_ = [
        ("signature", wintypes.DWORD),
        ("structure_version", wintypes.DWORD),
        ("file_version_ms", wintypes.DWORD),
        ("file_version_ls", wintypes.DWORD),
        ("product_version_ms", wintypes.DWORD),
        ("product_version_ls", wintypes.DWORD),
        ("file_flags_mask", wintypes.DWORD),
        ("file_flags", wintypes.DWORD),
        ("file_os", wintypes.DWORD),
        ("file_type", wintypes.DWORD),
        ("file_subtype", wintypes.DWORD),
        ("file_date_ms", wintypes.DWORD),
        ("file_date_ls", wintypes.DWORD),
    ]


def _file_version(path: Path) -> tuple[int, int, int, int]:
    version = ctypes.WinDLL("version", use_last_error=True)
    get_size = version.GetFileVersionInfoSizeW
    get_size.argtypes = (wintypes.LPCWSTR, ctypes.POINTER(wintypes.DWORD))
    get_size.restype = wintypes.DWORD
    ignored = wintypes.DWORD()
    size = get_size(str(path), ctypes.byref(ignored))
    if not size:
        raise OSError(ctypes.get_last_error(), "GetFileVersionInfoSizeW failed")
    buffer = ctypes.create_string_buffer(size)
    get_info = version.GetFileVersionInfoW
    get_info.argtypes = (wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p)
    get_info.restype = wintypes.BOOL
    if not get_info(str(path), 0, size, buffer):
        raise OSError(ctypes.get_last_error(), "GetFileVersionInfoW failed")
    query = version.VerQueryValueW
    query.argtypes = (
        ctypes.c_void_p,
        wintypes.LPCWSTR,
        ctypes.POINTER(ctypes.c_void_p),
        ctypes.POINTER(wintypes.UINT),
    )
    query.restype = wintypes.BOOL
    value = ctypes.c_void_p()
    value_size = wintypes.UINT()
    if not query(buffer, "\\", ctypes.byref(value), ctypes.byref(value_size)):
        raise OSError(ctypes.get_last_error(), "VerQueryValueW failed")
    fixed = ctypes.cast(value, ctypes.POINTER(_FixedFileInfo)).contents
    return (
        fixed.file_version_ms >> 16,
        fixed.file_version_ms & 0xFFFF,
        fixed.file_version_ls >> 16,
        fixed.file_version_ls & 0xFFFF,
    )


def verify_game_executable(path: Path) -> GameCompatibilityStatus:
    try:
        version = _file_version(path)
    except (OSError, ValueError) as error:
        return GameCompatibilityStatus(
            "unreadable",
            f"已找到游戏，但无法验证生成代码：{error}",
            path,
        )
    if version != (2, 0, 0, 2):
        display_version = ".".join(str(part) for part in version)
        return GameCompatibilityStatus(
            "unsupported",
            (
                f"已安装游戏版本 {display_version} 超出 PC v{SUPPORTED_GAME_VERSION} "
                "验证范围。请先检查并更新绘卷生成器。"
            ),
            path,
        )
    return GameCompatibilityStatus(
        "supported",
        f"已安装游戏版本与 PC v{SUPPORTED_GAME_VERSION} 验证范围一致。",
        path,
    )


def detect_game_compatibility() -> GameCompatibilityStatus:
    executables = discover_game_executables()
    if not executables:
        return GameCompatibilityStatus(
            "not_found",
            (
                f"未自动找到游戏 EXE；离线算法仍按 PC v{SUPPORTED_GAME_VERSION} 运行。"
                "若游戏已更新，请先检查生成器更新。"
            ),
        )
    statuses = tuple(verify_game_executable(path) for path in executables)
    mismatch = next((status for status in statuses if status.known_mismatch), None)
    if mismatch is not None:
        return mismatch
    supported = next((status for status in statuses if status.supported), None)
    return supported or statuses[0]


__all__ = [
    "SUPPORTED_GAME_VERSION",
    "GameCompatibilityStatus",
    "detect_game_compatibility",
    "discover_game_executables",
    "verify_game_executable",
]
