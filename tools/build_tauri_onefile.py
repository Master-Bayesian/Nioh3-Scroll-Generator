"""Wrap a verified portable ZIP in its manifest-owned install-free launcher."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import struct
import zipfile

MAGIC = b"NIOH3_ONEFILE_V1"
FOOTER = struct.Struct("<16sQ32s")
LAUNCHER_PATH = "launcher/Nioh3Launcher.exe"
MAX_DOWNLOAD_BYTES = 60 * 1024 * 1024
MAX_EXPANDED_BYTES = 2 * 1024 * 1024 * 1024
REQUIRED_PATHS = {
    "Nioh3Studio.exe", LAUNCHER_PATH,
    "worker/nioh3-search-worker.exe", "worker/nioh3-protected-worker.exe",
    "packages/contracts/request.schema.json", "packages/contracts/response.schema.json",
    "packages/contracts/protected-request.schema.json", "packages/contracts/protected-response.schema.json",
}


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def safe_path(name: str) -> bool:
    path = PurePosixPath(name)
    reserved = {"CON", "PRN", "AUX", "NUL"} | {
        f"{prefix}{index}" for prefix in ("COM", "LPT") for index in "123456789¹²³"
    }
    return bool(name) and not path.is_absolute() and not any(c in name for c in '\\:<>"|?*') and all(
        part not in ("", ".", "..") and not part.endswith((" ", "."))
        and part.split(".")[0].upper() not in reserved
        and not any(ord(c) < 32 for c in part)
        for part in name.split("/")
    )


def require_x64_pe(raw: bytes) -> None:
    if len(raw) < 0x40 or raw[:2] != b"MZ":
        raise ValueError("Launcher is not a PE executable")
    offset = struct.unpack_from("<I", raw, 0x3C)[0]
    if offset > len(raw) - 24 or raw[offset:offset + 4] != b"PE\0\0" or struct.unpack_from("<H", raw, offset + 4)[0] != 0x8664:
        raise ValueError("Launcher must be a valid x64 PE executable")


def verify_archive(path: Path) -> tuple[dict, bytes]:
    if path.stat().st_size > MAX_DOWNLOAD_BYTES:
        raise ValueError("Portable ZIP exceeds the 60 MiB download budget")
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        if not infos or len(infos) > 10001:
            raise ValueError("Unexpected archive member count")
        seen = set()
        for entry in infos:
            key = entry.filename.casefold()
            if not safe_path(entry.filename) or key in seen or entry.is_dir() or stat.S_ISLNK(entry.external_attr >> 16) or entry.flag_bits & 1:
                raise ValueError("Unsafe or duplicate archive member")
            seen.add(key)
        if sum(entry.file_size for entry in infos) > MAX_EXPANDED_BYTES:
            raise ValueError("Expanded archive exceeds its size bound")
        manifest_info = archive.getinfo("build-manifest.json")
        if manifest_info.file_size > 4 * 1024 * 1024:
            raise ValueError("Package manifest exceeds its size bound")
        manifest = json.loads(archive.read(manifest_info))
        if manifest.get("schema") != "nioh3-tauri-manifest/v1" or not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?", str(manifest.get("version", ""))):
            raise ValueError("Unsupported portable manifest or version")
        git = manifest.get("git", {})
        if git.get("dirty") is not False or not re.fullmatch(r"[0-9a-f]{40}", str(git.get("commit", ""))):
            raise ValueError("One-file releases require a clean source commit")
        declared = {}
        for entry in manifest.get("files", []):
            name = entry.get("path", "")
            if not safe_path(name) or name.casefold() in declared or name.casefold() == "build-manifest.json":
                raise ValueError("Unsafe or duplicate manifest member")
            declared[name.casefold()] = name
            info = archive.getinfo(name)
            if info.file_size != entry.get("size"):
                raise ValueError("Archived file size differs: " + name)
            with archive.open(info) as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            if digest != entry.get("sha256"):
                raise ValueError("Archived file hash differs: " + name)
        if set(declared) | {"build-manifest.json"} != seen:
            raise ValueError("Archive contains unmanifested files")
        if not REQUIRED_PATHS.issubset(declared.values()):
            raise ValueError("Portable archive is missing required runtime files")
        if archive.getinfo(LAUNCHER_PATH).file_size > 16 * 1024 * 1024:
            raise ValueError("Launcher exceeds its size bound")
        launcher = archive.read(LAUNCHER_PATH)
        require_x64_pe(launcher)
        return manifest, launcher


def build_onefile(archive_path: Path, output: Path) -> dict:
    archive_path = archive_path.resolve(strict=True)
    output = output.resolve()
    if output.exists():
        raise FileExistsError("Choose a new one-file output path")
    archive_hash = bytes.fromhex(sha256(archive_path))
    manifest, launcher = verify_archive(archive_path)
    if bytes.fromhex(sha256(archive_path)) != archive_hash:
        raise ValueError("Archive changed while validating")
    archive_size = archive_path.stat().st_size
    expected_size = len(launcher) + archive_size + FOOTER.size
    if expected_size > MAX_DOWNLOAD_BYTES:
        raise ValueError("One-file executable exceeds the 60 MiB download budget")
    output.parent.mkdir(parents=True, exist_ok=True)
    created = False
    try:
        with output.open("xb") as stream:
            created = True
            stream.write(launcher)
            with archive_path.open("rb") as payload:
                shutil.copyfileobj(payload, stream)
            stream.write(FOOTER.pack(MAGIC, archive_size, archive_hash))
        # Re-read the emitted payload; a changing source archive cannot pass.
        with output.open("rb") as stream:
            stream.seek(len(launcher))
            digest = hashlib.sha256()
            remaining = archive_size
            while remaining:
                chunk = stream.read(min(1024 * 1024, remaining))
                if not chunk:
                    raise ValueError("Truncated embedded ZIP")
                digest.update(chunk)
                remaining -= len(chunk)
            if digest.digest() != archive_hash or stream.read() != FOOTER.pack(MAGIC, archive_size, archive_hash):
                raise ValueError("Embedded archive changed while building")
        report = {"path": str(output), "version": manifest["version"], "sourceCommit": manifest["git"]["commit"],
                  "bytes": output.stat().st_size, "sha256": sha256(output), "payloadBytes": archive_size,
                  "payloadSha256": archive_hash.hex(), "launcherSha256": hashlib.sha256(launcher).hexdigest(),
                  "format": "nioh3-onefile/v1", "footerBytes": FOOTER.size}
        output.with_suffix(output.suffix + ".sha256").write_text(f'{report["sha256"]}  {output.name}\n', encoding="ascii")
        return report
    except Exception:
        if created:
            output.unlink(missing_ok=True)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(json.dumps(build_onefile(args.archive, args.output)))


if __name__ == "__main__":
    main()
