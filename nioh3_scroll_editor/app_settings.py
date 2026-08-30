"""Persistent application data-directory settings.

The small settings pointer stays in the default per-user location so the
application can find a user-selected data directory after upgrades. Backups,
update downloads, caches, and reports are stored under the selected directory.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import tempfile


SETTINGS_SCHEMA = "nioh3-scroll-generator-settings/v1"
SETTINGS_FILENAME = "settings.json"
ENV_DATA_ROOT = "NIOH3_SCROLL_DATA_ROOT"


@dataclass(frozen=True, slots=True)
class AppSettings:
    data_root: Path
    settings_path: Path


def default_state_root(*, fallback_root: Path) -> Path:
    local_app_data = os.environ.get("LOCALAPPDATA", "").strip()
    base = Path(local_app_data) if local_app_data else fallback_root
    return (base / "Nioh3ScrollGenerator").resolve()


def settings_path(*, fallback_root: Path) -> Path:
    return default_state_root(fallback_root=fallback_root) / SETTINGS_FILENAME


def _validated_data_root(value: object) -> Path:
    if not isinstance(value, str) or not value.strip():
        raise ValueError("data_root must be a non-empty path string")
    path = Path(value).expanduser()
    if not path.is_absolute():
        raise ValueError("data_root must be an absolute path")
    resolved = path.resolve()
    if resolved.exists() and not resolved.is_dir():
        raise ValueError("data_root points to a file")
    return resolved


def load_app_settings(*, fallback_root: Path) -> AppSettings:
    pointer_path = settings_path(fallback_root=fallback_root)
    environment_override = os.environ.get(ENV_DATA_ROOT, "").strip()
    if environment_override:
        data_root = _validated_data_root(environment_override)
        return AppSettings(data_root=data_root, settings_path=pointer_path)

    default_root = default_state_root(fallback_root=fallback_root)
    try:
        payload = json.loads(pointer_path.read_text(encoding="utf-8"))
        if payload.get("schema") != SETTINGS_SCHEMA:
            raise ValueError("unsupported settings schema")
        data_root = _validated_data_root(payload.get("data_root"))
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        data_root = default_root
    return AppSettings(data_root=data_root, settings_path=pointer_path)


def save_data_root(data_root: Path, *, fallback_root: Path) -> AppSettings:
    resolved = _validated_data_root(str(data_root))
    resolved.mkdir(parents=True, exist_ok=True)
    pointer_path = settings_path(fallback_root=fallback_root)
    pointer_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema": SETTINGS_SCHEMA,
        "data_root": str(resolved),
    }
    encoded = (json.dumps(payload, ensure_ascii=False, indent=2) + "\n").encode(
        "utf-8"
    )
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            dir=pointer_path.parent,
            prefix=f".{pointer_path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary.write(encoded)
            temporary.flush()
            os.fsync(temporary.fileno())
            temporary_path = Path(temporary.name)
        os.replace(temporary_path, pointer_path)
    finally:
        if temporary_path is not None and temporary_path.exists():
            temporary_path.unlink()
    return AppSettings(data_root=resolved, settings_path=pointer_path)


__all__ = [
    "AppSettings",
    "ENV_DATA_ROOT",
    "SETTINGS_FILENAME",
    "SETTINGS_SCHEMA",
    "default_state_root",
    "load_app_settings",
    "save_data_root",
    "settings_path",
]
