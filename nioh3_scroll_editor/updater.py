"""Signed, fail-closed update checks for the managed Windows installation."""

from __future__ import annotations

import base64
import ctypes
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Callable
from urllib.parse import urlparse

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

from .app_settings import UPDATE_CHANNEL_BETA, UPDATE_CHANNEL_STABLE, UPDATE_CHANNELS
from .version import APP_ID


MAX_MANIFEST_BYTES = 128 * 1024
MAX_RELEASE_INDEX_BYTES = 512 * 1024
MAX_UPDATE_BYTES = 128 * 1024 * 1024
MANAGED_INSTALL_MARKER = ".nioh3-scroll-generator-managed-install.json"
MANAGED_INSTALL_SCHEMA = 1
MANAGED_INSTALL_CHANNEL = "stable"
_RELEASE_VERSION = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-(beta|rc)\.(0|[1-9]\d*))?$"
)
_SHA256 = re.compile(r"^[0-9A-Fa-f]{64}$")
APPLY_UPDATE_SWITCH = "--apply-managed-update"
POST_UPDATE_CLEANUP_SWITCH = "--post-update-cleanup"
UPDATE_REPLACE_TIMEOUT_SECONDS = 45.0


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
    channel: str = UPDATE_CHANNEL_STABLE


@dataclass(frozen=True, slots=True)
class DownloadedUpdate:
    manifest: UpdateManifest
    path: Path


def release_version_tuple(value: str) -> tuple[int, int, int, int, int]:
    match = _RELEASE_VERSION.fullmatch(value)
    if match is None:
        raise ValueError(
            "release version must use major.minor.patch or "
            "major.minor.patch-beta.N/rc.N"
        )
    major, minor, patch, prerelease_kind, prerelease_number = match.groups()
    stage = {
        "beta": 0,
        "rc": 1,
        None: 2,
    }[prerelease_kind]
    return (
        int(major),
        int(minor),
        int(patch),
        stage,
        int(prerelease_number or 0),
    )


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


def fetch_latest_prerelease_manifest(
    releases_api_url: str,
    public_key_base64: str,
    *,
    timeout: float = 10.0,
    open_url: Callable[[str, float], BinaryIO] = _default_open_url,
) -> UpdateManifest | None:
    """Return the newest signed manifest from a GitHub prerelease.

    GitHub's ``releases/latest`` route intentionally excludes prereleases. The
    beta channel therefore resolves one manifest through the Releases API, but
    still trusts only the Ed25519-signed manifest and its exact asset hash.
    """

    parsed_url = urlparse(releases_api_url)
    if parsed_url.scheme != "https" or parsed_url.netloc != "api.github.com":
        raise ValueError("prerelease discovery must use the GitHub HTTPS API")
    with open_url(releases_api_url, timeout) as response:
        raw = response.read(MAX_RELEASE_INDEX_BYTES + 1)
    if len(raw) > MAX_RELEASE_INDEX_BYTES:
        raise ValueError("GitHub release index is too large")
    try:
        releases = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("GitHub release index is not valid UTF-8 JSON") from error
    if not isinstance(releases, list):
        raise ValueError("GitHub release index is not a list")

    candidates: list[tuple[tuple[int, int, int, int, int], str, str]] = []
    for release in releases:
        if (
            not isinstance(release, dict)
            or release.get("draft") is True
            or release.get("prerelease") is not True
        ):
            continue
        tag_name = str(release.get("tag_name", ""))
        version = tag_name[1:] if tag_name.startswith("v") else tag_name
        try:
            version_key = release_version_tuple(version)
        except ValueError:
            continue
        assets = release.get("assets")
        if not isinstance(assets, list):
            continue
        manifest_url = ""
        for preferred_name in ("beta.json", "latest.json"):
            matching = next(
                (
                    asset
                    for asset in assets
                    if isinstance(asset, dict)
                    and asset.get("name") == preferred_name
                ),
                None,
            )
            if matching is not None:
                manifest_url = str(matching.get("browser_download_url", ""))
                break
        if manifest_url:
            candidates.append((version_key, version, manifest_url))
    if not candidates:
        return None

    _version_key, tagged_version, manifest_url = max(candidates)
    manifest = fetch_update_manifest(
        manifest_url,
        public_key_base64,
        timeout=timeout,
        open_url=open_url,
    )
    if manifest.version != tagged_version:
        raise ValueError("signed beta manifest version does not match its release tag")
    return manifest


def check_for_update(
    current_version: str,
    manifest_url: str,
    public_key_base64: str,
    *,
    channel: str = UPDATE_CHANNEL_STABLE,
    releases_api_url: str | None = None,
    **fetch_options: object,
) -> UpdateCheckResult:
    current = release_version_tuple(current_version)
    if channel not in UPDATE_CHANNELS:
        raise ValueError("unsupported update channel")
    stable_manifest = fetch_update_manifest(
        manifest_url,
        public_key_base64,
        **fetch_options,
    )
    manifest = stable_manifest
    if channel == UPDATE_CHANNEL_BETA:
        if not releases_api_url:
            raise ValueError("beta update channel has no GitHub Releases API URL")
        prerelease_manifest = fetch_latest_prerelease_manifest(
            releases_api_url,
            public_key_base64,
            **fetch_options,
        )
        if (
            prerelease_manifest is not None
            and release_version_tuple(prerelease_manifest.version)
            > release_version_tuple(stable_manifest.version)
        ):
            manifest = prerelease_manifest
    return UpdateCheckResult(
        current_version=current_version,
        manifest=manifest,
        update_available=release_version_tuple(manifest.version) > current,
        channel=channel,
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


def ensure_managed_install(executable: Path) -> Path:
    """Enroll one portable executable as the only managed update target."""

    supplied = executable.absolute()
    if supplied.is_symlink() or not supplied.is_file():
        raise RuntimeError("automatic installation requires a regular executable file")
    target = supplied.resolve()
    marker = target.parent / MANAGED_INSTALL_MARKER
    if marker.exists():
        validate_managed_install(target)
        return marker
    value = {
        "schema": MANAGED_INSTALL_SCHEMA,
        "app_id": APP_ID,
        "channel": MANAGED_INSTALL_CHANNEL,
        "executable": target.name,
    }
    temporary = marker.with_name(f"{marker.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("x", encoding="utf-8", newline="\n") as stream:
            json.dump(value, stream, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
            stream.write("\n")
        os.replace(temporary, marker)
    finally:
        temporary.unlink(missing_ok=True)
    validate_managed_install(target)
    return marker


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
    if value.get("schema") != MANAGED_INSTALL_SCHEMA:
        raise RuntimeError("managed installation marker uses an unsupported schema")
    if value.get("channel") != MANAGED_INSTALL_CHANNEL:
        raise RuntimeError("managed installation marker uses an unsupported channel")
    if value.get("executable") != target.name:
        raise RuntimeError("managed installation marker does not match this executable")
    return target


def validate_downloaded_update(
    downloaded: DownloadedUpdate,
    *,
    current_executable: Path,
    state_root: Path,
) -> tuple[Path, Path]:
    """Validate the managed target and the signed executable cached for it."""

    target = validate_managed_install(current_executable)
    update_root = (state_root.resolve() / "updates").resolve()
    source = downloaded.path.resolve()
    if update_root not in source.parents or not source.is_file():
        raise ValueError("downloaded update is outside the managed update cache")
    if source.name != downloaded.manifest.asset_name:
        raise ValueError("downloaded update file name does not match the signed manifest")
    if source.stat().st_size != downloaded.manifest.asset_size:
        raise RuntimeError("downloaded update size changed after verification")
    if _sha256_file(source) != downloaded.manifest.asset_sha256:
        raise RuntimeError("downloaded update changed after verification")
    if source == target:
        raise ValueError("downloaded update cannot be the running managed executable")
    return source, target


def _wait_for_process_exit(process_id: int, timeout: float) -> None:
    """Wait for one process without requiring optional process libraries."""

    if process_id <= 0 or process_id == os.getpid():
        raise ValueError("old process ID is invalid")
    if os.name == "nt":
        synchronize = 0x00100000
        wait_timeout = 0x00000102
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
        kernel32.OpenProcess.restype = ctypes.c_void_p
        kernel32.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel32.WaitForSingleObject.restype = ctypes.c_uint32
        kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel32.OpenProcess(synchronize, False, process_id)
        if not handle:
            return
        try:
            result = kernel32.WaitForSingleObject(handle, max(1, int(timeout * 1000)))
        finally:
            kernel32.CloseHandle(handle)
        if result == wait_timeout:
            raise TimeoutError("the previous application process did not exit")
        return

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            os.kill(process_id, 0)
        except ProcessLookupError:
            return
        time.sleep(0.05)
    raise TimeoutError("the previous application process did not exit")


def apply_managed_update(
    *,
    source: Path,
    target: Path,
    state_root: Path,
    expected_sha256: str,
    expected_size: int,
    old_process_id: int,
    timeout: float = UPDATE_REPLACE_TIMEOUT_SECONDS,
    wait_for_exit: Callable[[int, float], None] = _wait_for_process_exit,
    replace_file: Callable[[Path, Path], None] = os.replace,
    start_process: Callable[..., object] = subprocess.Popen,
) -> None:
    """Apply a verified update from the new executable's helper mode."""

    source = source.resolve()
    target = validate_managed_install(target)
    update_root = (state_root.resolve() / "updates").resolve()
    if update_root not in source.parents or not source.is_file():
        raise ValueError("update helper source is outside the managed update cache")
    if source == target:
        raise ValueError("update helper cannot replace its own running file")
    if not _SHA256.fullmatch(expected_sha256):
        raise ValueError("update helper SHA-256 is invalid")
    if source.stat().st_size != expected_size:
        raise RuntimeError("update helper source size does not match the manifest")
    if _sha256_file(source) != expected_sha256.upper():
        raise RuntimeError("update helper source hash does not match the manifest")

    wait_for_exit(old_process_id, timeout)
    staged = target.with_name(f"{target.name}.{os.getpid()}.new")
    staged.unlink(missing_ok=True)
    try:
        shutil.copyfile(source, staged)
        if staged.stat().st_size != expected_size:
            raise RuntimeError("staged update size does not match the manifest")
        if _sha256_file(staged) != expected_sha256.upper():
            raise RuntimeError("staged update hash does not match the manifest")

        deadline = time.monotonic() + timeout
        while True:
            try:
                replace_file(staged, target)
                break
            except (OSError, PermissionError):
                if time.monotonic() >= deadline:
                    raise TimeoutError(
                        "the previous executable remained locked during replacement"
                    )
                time.sleep(0.10)
        if _sha256_file(target) != expected_sha256.upper():
            raise RuntimeError("installed update hash does not match the manifest")
        start_process(
            [
                str(target),
                POST_UPDATE_CLEANUP_SWITCH,
                "--source",
                str(source),
                "--state-root",
                str(state_root.resolve()),
            ],
            close_fds=True,
        )
    finally:
        staged.unlink(missing_ok=True)


def cleanup_downloaded_update(
    source: Path,
    state_root: Path,
    *,
    timeout: float = UPDATE_REPLACE_TIMEOUT_SECONDS,
) -> None:
    """Remove the helper executable after its PyInstaller parent releases it."""

    source = source.resolve()
    update_root = (state_root.resolve() / "updates").resolve()
    if update_root not in source.parents:
        raise ValueError("post-update cleanup source is outside the update cache")
    deadline = time.monotonic() + timeout
    while source.exists():
        try:
            source.unlink()
        except OSError:
            if time.monotonic() >= deadline:
                return
            time.sleep(0.10)
    for directory in (source.parent, update_root):
        try:
            directory.rmdir()
        except OSError:
            pass


def _show_update_helper_error(message: str) -> None:
    if os.name == "nt":
        ctypes.windll.user32.MessageBoxW(
            None,
            f"自动更新失败，旧版本仍可继续使用。\n\n{message}",
            "仁王3绘卷生成器更新失败",
            0x10,
        )


def handle_update_command_line(arguments: list[str] | None = None) -> int | None:
    """Handle updater-only startup modes before the desktop UI is imported."""

    args = list(sys.argv[1:] if arguments is None else arguments)
    if not args:
        return None
    mode = args[0]
    if mode not in (APPLY_UPDATE_SWITCH, POST_UPDATE_CLEANUP_SWITCH):
        return None

    def option(name: str) -> str:
        try:
            index = args.index(name)
            value = args[index + 1]
        except (ValueError, IndexError) as error:
            raise ValueError(f"missing updater option {name}") from error
        return value

    if mode == POST_UPDATE_CLEANUP_SWITCH:
        source = Path(option("--source"))
        state_root = Path(option("--state-root"))
        threading.Thread(
            target=cleanup_downloaded_update,
            args=(source, state_root),
            daemon=True,
        ).start()
        return None

    target = Path(option("--target"))
    try:
        apply_managed_update(
            source=Path(sys.executable),
            target=target,
            state_root=Path(option("--state-root")),
            expected_sha256=option("--sha256"),
            expected_size=int(option("--size"), 10),
            old_process_id=int(option("--old-process-id"), 10),
        )
    except Exception as error:
        _show_update_helper_error(str(error))
        try:
            if target.is_file():
                subprocess.Popen([str(target)], close_fds=True)
        except OSError:
            pass
        return 1
    return 0


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
    downloaded: DownloadedUpdate,
    *,
    current_executable: Path,
    state_root: Path,
    process_id: int | None = None,
) -> None:
    source, target = validate_downloaded_update(
        downloaded,
        current_executable=current_executable,
        state_root=state_root,
    )
    creation_flags = (
        getattr(subprocess, "DETACHED_PROCESS", 0)
        | getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        | getattr(subprocess, "CREATE_NO_WINDOW", 0)
    )
    subprocess.Popen(
        [
            str(source),
            APPLY_UPDATE_SWITCH,
            "--old-process-id",
            str(process_id or os.getpid()),
            "--target",
            str(target),
            "--state-root",
            str(state_root.resolve()),
            "--sha256",
            downloaded.manifest.asset_sha256,
            "--size",
            str(downloaded.manifest.asset_size),
        ],
        close_fds=True,
        creationflags=creation_flags,
    )
