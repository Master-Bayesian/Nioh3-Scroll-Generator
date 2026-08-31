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
UPDATE_CHANNEL_STABLE = "stable"
UPDATE_CHANNEL_BETA = "beta"
UPDATE_CHANNELS = frozenset((UPDATE_CHANNEL_STABLE, UPDATE_CHANNEL_BETA))


@dataclass(frozen=True, slots=True)
class AppSettings:
    data_root: Path
    settings_path: Path
    update_channel: str = UPDATE_CHANNEL_STABLE


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


def _validated_update_channel(value: object) -> str:
    channel = str(value or UPDATE_CHANNEL_STABLE).strip().lower()
    if channel not in UPDATE_CHANNELS:
        raise ValueError("unsupported update channel")
    return channel


def _read_pointer_payload(pointer_path: Path) -> dict[str, object]:
    payload = json.loads(pointer_path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict) or payload.get("schema") != SETTINGS_SCHEMA:
        raise ValueError("unsupported settings schema")
    return payload


def _write_settings(
    *,
    data_root: Path,
    update_channel: str,
    pointer_path: Path,
) -> AppSettings:
    resolved = _validated_data_root(str(data_root))
    channel = _validated_update_channel(update_channel)
    resolved.mkdir(parents=True, exist_ok=True)
    pointer_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema": SETTINGS_SCHEMA,
        "data_root": str(resolved),
        "update_channel": channel,
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
    return AppSettings(
        data_root=resolved,
        settings_path=pointer_path,
        update_channel=channel,
    )


def load_app_settings(*, fallback_root: Path) -> AppSettings:
    pointer_path = settings_path(fallback_root=fallback_root)
    payload: dict[str, object] = {}
    try:
        payload = _read_pointer_payload(pointer_path)
        update_channel = _validated_update_channel(payload.get("update_channel"))
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        update_channel = UPDATE_CHANNEL_STABLE
    environment_override = os.environ.get(ENV_DATA_ROOT, "").strip()
    if environment_override:
        data_root = _validated_data_root(environment_override)
        return AppSettings(
            data_root=data_root,
            settings_path=pointer_path,
            update_channel=update_channel,
        )

    default_root = default_state_root(fallback_root=fallback_root)
    try:
        data_root = _validated_data_root(payload.get("data_root"))
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        data_root = default_root
    return AppSettings(
        data_root=data_root,
        settings_path=pointer_path,
        update_channel=update_channel,
    )


def save_data_root(data_root: Path, *, fallback_root: Path) -> AppSettings:
    current = load_app_settings(fallback_root=fallback_root)
    pointer_path = settings_path(fallback_root=fallback_root)
    return _write_settings(
        data_root=data_root,
        update_channel=current.update_channel,
        pointer_path=pointer_path,
    )


def save_update_channel(channel: str, *, fallback_root: Path) -> AppSettings:
    current = load_app_settings(fallback_root=fallback_root)
    return _write_settings(
        data_root=current.data_root,
        update_channel=channel,
        pointer_path=current.settings_path,
    )


__all__ = [
    "AppSettings",
    "ENV_DATA_ROOT",
    "SETTINGS_FILENAME",
    "SETTINGS_SCHEMA",
    "UPDATE_CHANNEL_BETA",
    "UPDATE_CHANNEL_STABLE",
    "UPDATE_CHANNELS",
    "default_state_root",
    "load_app_settings",
    "save_data_root",
    "save_update_channel",
    "settings_path",
]
