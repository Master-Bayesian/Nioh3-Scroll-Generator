"""Signed, fail-closed update checks for the managed Windows installation."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Callable
from urllib.parse import urlparse

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

from .version import APP_ID


MAX_MANIFEST_BYTES = 128 * 1024
MAX_UPDATE_BYTES = 128 * 1024 * 1024
MANAGED_INSTALL_MARKER = ".nioh3-scroll-generator-managed-install.json"
_RELEASE_VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
_SHA256 = re.compile(r"^[0-9A-Fa-f]{64}$")


@dataclass(frozen=True, slots=True)
class UpdateManifest:
    version: str
    published_at_utc: str
    notes: str
    asset_name: str
    asset_url: str
    asset_size: int
    asset_sha256: str
    signature_base64: str

    @classmethod
    def from_mapping(cls, value: object) -> "UpdateManifest":
        if not isinstance(value, dict) or value.get("schema") != 1:
            raise ValueError("unsupported update manifest schema")
        asset = value.get("asset")
        if not isinstance(asset, dict):
            raise ValueError("update manifest is missing its asset")
        manifest = cls(
            version=str(value.get("version", "")),
            published_at_utc=str(value.get("published_at_utc", "")),
            notes=str(value.get("notes", "")),
            asset_name=str(asset.get("name", "")),
            asset_url=str(asset.get("url", "")),
            asset_size=int(asset.get("size", 0)),
            asset_sha256=str(asset.get("sha256", "")).upper(),
            signature_base64=str(value.get("signature", "")),
        )
        manifest.validate()
        return manifest

    def validate(self) -> None:
        release_version_tuple(self.version)
        if not self.published_at_utc:
            raise ValueError("update manifest has no publication timestamp")
        if Path(self.asset_name).name != self.asset_name or not self.asset_name.lower().endswith(".exe"):
            raise ValueError("update asset name must be one executable file name")
        parsed_url = urlparse(self.asset_url)
        if parsed_url.scheme != "https" or not parsed_url.netloc:
            raise ValueError("update asset URL must use HTTPS")
        if not 0 < self.asset_size <= MAX_UPDATE_BYTES:
            raise ValueError("update asset size is outside the accepted range")
        if not _SHA256.fullmatch(self.asset_sha256):
            raise ValueError("update asset SHA-256 is invalid")
        try:
            signature = base64.b64decode(self.signature_base64, validate=True)
        except ValueError as error:
            raise ValueError("update signature is not valid base64") from error
        if len(signature) != 64:
            raise ValueError("update signature is not an Ed25519 signature")

    def signed_payload(self) -> bytes:
        payload = {
            "asset_name": self.asset_name,
            "asset_sha256": self.asset_sha256,
            "asset_size": self.asset_size,
            "asset_url": self.asset_url,
            "notes": self.notes,
            "published_at_utc": self.published_at_utc,
            "schema": 1,
            "version": self.version,
        }
        return json.dumps(
            payload,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")


@dataclass(frozen=True, slots=True)
class UpdateCheckResult:
    current_version: str
    manifest: UpdateManifest
    update_available: bool


@dataclass(frozen=True, slots=True)
class DownloadedUpdate:
    manifest: UpdateManifest
    path: Path


def release_version_tuple(value: str) -> tuple[int, int, int]:
    match = _RELEASE_VERSION.fullmatch(value)
    if match is None:
        raise ValueError("release version must use major.minor.patch")
    return tuple(int(part) for part in match.groups())


def verify_manifest_signature(manifest: UpdateManifest, public_key_base64: str) -> None:
    try:
        public_key_bytes = base64.b64decode(public_key_base64, validate=True)
    except ValueError as error:
        raise ValueError("embedded update public key is not valid base64") from error
    if len(public_key_bytes) != 32:
        raise ValueError("embedded update public key is not an Ed25519 key")
    signature = base64.b64decode(manifest.signature_base64, validate=True)
    try:
        Ed25519PublicKey.from_public_bytes(public_key_bytes).verify(
            signature,
            manifest.signed_payload(),
        )
    except InvalidSignature as error:
        raise ValueError("update manifest signature verification failed") from error


def _default_open_url(url: str, timeout: float) -> BinaryIO:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": f"{APP_ID}-updater/1"},
    )
    return urllib.request.urlopen(request, timeout=timeout)


def fetch_update_manifest(
    manifest_url: str,
    public_key_base64: str,
    *,
    timeout: float = 10.0,
    open_url: Callable[[str, float], BinaryIO] = _default_open_url,
) -> UpdateManifest:
    parsed_url = urlparse(manifest_url)
    if parsed_url.scheme != "https" or not parsed_url.netloc:
        raise ValueError("update manifest URL must use HTTPS")
    with open_url(manifest_url, timeout) as response:
        raw = response.read(MAX_MANIFEST_BYTES + 1)
    if len(raw) > MAX_MANIFEST_BYTES:
        raise ValueError("update manifest is too large")
    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("update manifest is not valid UTF-8 JSON") from error
    manifest = UpdateManifest.from_mapping(value)
    verify_manifest_signature(manifest, public_key_base64)
    return manifest


def check_for_update(
    current_version: str,
    manifest_url: str,
    public_key_base64: str,
    **fetch_options: object,
) -> UpdateCheckResult:
    current = release_version_tuple(current_version)
    manifest = fetch_update_manifest(
        manifest_url,
        public_key_base64,
        **fetch_options,
    )
    return UpdateCheckResult(
        current_version=current_version,
        manifest=manifest,
        update_available=release_version_tuple(manifest.version) > current,
    )


def download_update(
    manifest: UpdateManifest,
    state_root: Path,
    *,
    timeout: float = 30.0,
    open_url: Callable[[str, float], BinaryIO] = _default_open_url,
) -> DownloadedUpdate:
    manifest.validate()
    update_root = state_root.resolve() / "updates" / manifest.version
    update_root.mkdir(parents=True, exist_ok=True)
    destination = update_root / manifest.asset_name
    temporary = update_root / (manifest.asset_name + ".part")
    if destination.is_file():
        if _sha256_file(destination) == manifest.asset_sha256 and destination.stat().st_size == manifest.asset_size:
            return DownloadedUpdate(manifest=manifest, path=destination)
        raise FileExistsError("an existing downloaded update does not match the signed manifest")
    if temporary.exists():
        temporary.unlink()
    digest = hashlib.sha256()
    total = 0
    try:
        with open_url(manifest.asset_url, timeout) as response, temporary.open("xb") as output:
            while True:
                block = response.read(1024 * 1024)
                if not block:
                    break
                total += len(block)
                if total > manifest.asset_size or total > MAX_UPDATE_BYTES:
                    raise ValueError("downloaded update exceeds the signed size")
                digest.update(block)
                output.write(block)
        if total != manifest.asset_size:
            raise ValueError("downloaded update size does not match the signed manifest")
        if digest.hexdigest().upper() != manifest.asset_sha256:
            raise ValueError("downloaded update SHA-256 does not match the signed manifest")
        os.replace(temporary, destination)
    finally:
        if temporary.exists():
            temporary.unlink()
    return DownloadedUpdate(manifest=manifest, path=destination)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest().upper()


def validate_managed_install(executable: Path) -> Path:
    target = executable.resolve()
    marker = target.parent / MANAGED_INSTALL_MARKER
    if not target.is_file() or not marker.is_file():
        raise RuntimeError("automatic installation requires the managed installer build")
    try:
        value = json.loads(marker.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError("managed installation marker is invalid") from error
    if not isinstance(value, dict) or value.get("app_id") != APP_ID:
        raise RuntimeError("managed installation marker belongs to another application")
    if value.get("executable") != target.name:
        raise RuntimeError("managed installation marker does not match this executable")
    return target


def prepare_managed_update_script(
    downloaded: DownloadedUpdate,
    *,
    current_executable: Path,
    state_root: Path,
    process_id: int | None = None,
) -> Path:
    target = validate_managed_install(current_executable)
    update_root = (state_root.resolve() / "updates").resolve()
    source = downloaded.path.resolve()
    if update_root not in source.parents or not source.is_file():
        raise ValueError("downloaded update is outside the managed update cache")
    if _sha256_file(source) != downloaded.manifest.asset_sha256:
        raise RuntimeError("downloaded update changed after verification")
    script = update_root / f"install-{downloaded.manifest.version}.ps1"
    if script.exists():
        raise FileExistsError(script)
    script.write_text(
        """param(
    [Parameter(Mandatory=$true)][int]$ProcessId,
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$Target,
    [Parameter(Mandatory=$true)][string]$Sha256
)
$ErrorActionPreference = 'Stop'
Wait-Process -Id $ProcessId -ErrorAction SilentlyContinue
if ((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -ne $Sha256) {
    throw 'Downloaded update changed after verification.'
}
$stagedTarget = $Target + '.new'
Copy-Item -LiteralPath $Source -Destination $stagedTarget
Move-Item -LiteralPath $stagedTarget -Destination $Target -Force
Start-Process -FilePath $Target
Remove-Item -LiteralPath $Source -Force
Remove-Item -LiteralPath $PSCommandPath -Force
""",
        encoding="utf-8-sig",
    )
    return script


def launch_managed_update(
    script: Path,
    downloaded: DownloadedUpdate,
    *,
    current_executable: Path | None = None,
    process_id: int | None = None,
) -> None:
    target = validate_managed_install(current_executable or Path(sys.executable))
    creation_flags = (
        getattr(subprocess, "DETACHED_PROCESS", 0)
        | getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        | getattr(subprocess, "CREATE_NO_WINDOW", 0)
    )
    subprocess.Popen(
        [
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(script.resolve()),
            "-ProcessId",
            str(process_id or os.getpid()),
            "-Source",
            str(downloaded.path.resolve()),
            "-Target",
            str(target),
            "-Sha256",
            downloaded.manifest.asset_sha256,
        ],
        close_fds=True,
        creationflags=creation_flags,
    )
