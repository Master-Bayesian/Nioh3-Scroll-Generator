"""Validate the structure, hashes, and optional ZIP of a Pro research handoff."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile


REQUIRED_FILES = ("README.md", "TASK_FOR_PRO.md", "ENVIRONMENT.json", "SHA256SUMS.txt")
REQUIRED_DIRECTORIES = ("evidence", "project-source")
HASH_LINE = re.compile(r"^([0-9a-fA-F]{64})  (.+)$")


class HandoffValidationError(ValueError):
    """Raised when a research handoff violates its structural contract."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def package_files(root: Path) -> dict[str, Path]:
    files: dict[str, Path] = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise HandoffValidationError(f"symbolic links are not allowed: {path.relative_to(root)}")
        if path.is_file() and path.name != "SHA256SUMS.txt":
            files[path.relative_to(root).as_posix()] = path
    return files


def safe_relative_path(value: str) -> bool:
    if "\\" in value or "\x00" in value:
        return False
    path = PurePosixPath(value)
    return (
        bool(value)
        and value == path.as_posix()
        and not path.is_absolute()
        and all(part not in {"", ".", ".."} for part in path.parts)
    )


def parse_hashes(path: Path) -> dict[str, str]:
    entries: dict[str, str] = {}
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        match = HASH_LINE.fullmatch(raw_line)
        if match is None:
            raise HandoffValidationError(f"invalid SHA256SUMS.txt line {line_number}")
        digest, relative = match.groups()
        if not safe_relative_path(relative) or relative == "SHA256SUMS.txt":
            raise HandoffValidationError(f"unsafe or self-referential checksum path: {relative}")
        if relative in entries:
            raise HandoffValidationError(f"duplicate checksum path: {relative}")
        entries[relative] = digest.lower()
    if not entries:
        raise HandoffValidationError("SHA256SUMS.txt is empty")
    return entries


def validate_directory(root: Path) -> dict[str, object]:
    root = root.resolve()
    if not root.is_dir():
        raise HandoffValidationError(f"package directory does not exist: {root}")

    for name in REQUIRED_FILES:
        path = root / name
        if not path.is_file() or path.stat().st_size == 0:
            raise HandoffValidationError(f"required file is missing or empty: {name}")
    for name in REQUIRED_DIRECTORIES:
        path = root / name
        if not path.is_dir() or not any(item.is_file() for item in path.rglob("*")):
            raise HandoffValidationError(f"required directory has no evidence files: {name}")

    try:
        environment = json.loads((root / "ENVIRONMENT.json").read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise HandoffValidationError(f"ENVIRONMENT.json is invalid: {exc}") from exc
    if not isinstance(environment, dict) or not environment:
        raise HandoffValidationError("ENVIRONMENT.json must contain a non-empty object")

    files = package_files(root)
    expected_hashes = parse_hashes(root / "SHA256SUMS.txt")
    if set(expected_hashes) != set(files):
        missing = sorted(set(files) - set(expected_hashes))
        extra = sorted(set(expected_hashes) - set(files))
        raise HandoffValidationError(f"checksum coverage differs; missing={missing}, extra={extra}")
    for relative, path in files.items():
        actual = sha256_file(path)
        if actual != expected_hashes[relative]:
            raise HandoffValidationError(f"checksum mismatch: {relative}")

    return {"package": str(root), "file_count": len(files) + 1, "hashes_verified": len(files)}


def validate_zip(root: Path, archive_path: Path) -> dict[str, object]:
    root = root.resolve()
    archive_path = archive_path.resolve()
    if not archive_path.is_file():
        raise HandoffValidationError(f"ZIP does not exist: {archive_path}")
    disk_files = {
        "SHA256SUMS.txt": root / "SHA256SUMS.txt",
        **package_files(root),
    }
    prefix = root.name + "/"
    with zipfile.ZipFile(archive_path) as archive:
        bad_member = archive.testzip()
        if bad_member is not None:
            raise HandoffValidationError(f"ZIP CRC failure: {bad_member}")
        members: dict[str, zipfile.ZipInfo] = {}
        for info in archive.infolist():
            name = info.filename
            if not name.startswith(prefix):
                raise HandoffValidationError(f"ZIP member is outside the package root: {name}")
            relative = name[len(prefix):].rstrip("/")
            if info.is_dir():
                if relative and not safe_relative_path(relative):
                    raise HandoffValidationError(f"unsafe ZIP directory: {name}")
                continue
            if not safe_relative_path(relative) or relative in members:
                raise HandoffValidationError(f"unsafe or duplicate ZIP member: {name}")
            if (info.external_attr >> 16) & 0o170000 == 0o120000:
                raise HandoffValidationError(f"symbolic links are not allowed in ZIP: {name}")
            members[relative] = info
        if set(members) != set(disk_files):
            missing = sorted(set(disk_files) - set(members))
            extra = sorted(set(members) - set(disk_files))
            raise HandoffValidationError(f"ZIP contents differ; missing={missing}, extra={extra}")
        for relative, path in disk_files.items():
            archived_digest = hashlib.sha256(archive.read(members[relative])).hexdigest()
            if archived_digest != sha256_file(path):
                raise HandoffValidationError(f"ZIP content differs: {relative}")

    return {
        "zip": str(archive_path),
        "zip_size": archive_path.stat().st_size,
        "zip_sha256": sha256_file(archive_path),
        "zip_files_verified": len(disk_files),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--zip", dest="archive", type=Path)
    args = parser.parse_args()

    try:
        report = validate_directory(args.package)
        if args.archive is not None:
            report.update(validate_zip(args.package, args.archive))
    except (HandoffValidationError, OSError, zipfile.BadZipFile) as exc:
        parser.exit(1, f"research handoff validation failed: {exc}\n")

    print(json.dumps({"ok": True, **report}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
