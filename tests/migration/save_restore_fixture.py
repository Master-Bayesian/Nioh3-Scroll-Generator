"""Synthetic multi-role generations for protected restore regressions.

The helpers in this module never discover or open a user save.  Callers supply
an explicit task-local ``SAVEDATA.BIN`` path and already-synthetic bytes.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROLE_FILES = {
    "main_save": "SAVEDATA.BIN",
    "game_backup": "BACKUP.BIN",
    "system_save": "SYSTEMSAVEDATA.BIN",
}


def generation_bytes(main: bytes, label: str) -> dict[str, bytes]:
    """Return one generation whose three roles are independently identifiable."""

    return {
        "main_save": main,
        "game_backup": f"{label}:game-backup".encode("ascii"),
        "system_save": f"{label}:system-save".encode("ascii"),
    }


def target_paths(save_path: Path) -> dict[str, Path]:
    account = save_path.parent.parent
    return {
        "main_save": save_path,
        "game_backup": save_path.parent / "BACKUP.BIN",
        "system_save": account / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
    }


def write_generation(save_path: Path, generation: dict[str, bytes]) -> None:
    for role, path in target_paths(save_path).items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(generation[role])


def read_generation(save_path: Path) -> dict[str, bytes]:
    return {role: path.read_bytes() for role, path in target_paths(save_path).items()}


def assert_generations_distinct(*generations: dict[str, bytes]) -> None:
    """Fail before exercising restore if any role reuses a generation's bytes."""

    for role in ROLE_FILES:
        values = [generation[role] for generation in generations]
        if len(set(values)) != len(values):
            raise AssertionError(f"restore fixture role {role} is not generation-distinct")


def generation_digests(generation: dict[str, bytes]) -> dict[str, str]:
    return {
        role: hashlib.sha256(generation[role]).hexdigest()
        for role in ROLE_FILES
    }


def write_backup_bundle(
    state_root: Path,
    backup_id: str,
    generation: dict[str, bytes],
    *,
    account_id: int,
    save_slot_index: int = 0,
    file_overrides: dict[str, str] | None = None,
    omitted_roles: set[str] | None = None,
) -> Path:
    """Write a v2 manifest and its synthetic role files under ``backups``."""

    omitted = omitted_roles or set()
    overrides = file_overrides or {}
    bundle = state_root / "backups" / backup_id
    bundle.mkdir(parents=True, exist_ok=True)
    entries: list[dict[str, object]] = []
    written: set[str] = set()
    for role, default_name in ROLE_FILES.items():
        if role in omitted:
            continue
        backup_file = overrides.get(role, default_name)
        payload = generation[role]
        if backup_file not in written:
            (bundle / backup_file).write_bytes(payload)
            written.add(backup_file)
        entries.append(
            {
                "source_role": role,
                "source_path": f"synthetic/{role}",
                "backup_file": backup_file,
                "size": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest().upper(),
            }
        )
    manifest = {
        "backup_manifest_schema": "nioh3-scroll-backup/v2",
        "save_schema_profile": "nioh3-pc-v2.00.02-v2.01/save-layout-v1",
        "operation_id": "a" * 32,
        "created_at_utc": "2026-09-20T00:00:00+00:00",
        "action": "synthetic-restore-source",
        "steam_account_id": account_id,
        "save_slot_index": save_slot_index,
        "backup_files": entries,
    }
    (bundle / "backup-manifest.json").write_text(
        json.dumps(manifest, indent=2), encoding="utf-8"
    )
    return bundle
