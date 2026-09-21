"""Deterministic verification of Nioh 3 Studio one-file release artifacts.

The published release is exactly six assets. This tool verifies them in place
(local mode) and, on request, against the live GitHub release and a fresh public
re-download (public mode). It is read-only: it never builds, signs, renames, or
mutates a release, and it hard-codes no version, commit, or absolute path.

Local mode:
  python tools/verify_release_artifacts.py --directory <dir with the six assets>
      --version 0.8.0 --expected-sha <full candidate commit>
      --report <verification.json>

Public mode (adds the live release checks):
  ... --public [--public-download-dir <fresh, empty or absent dir>]

Only the standard library plus `cryptography` (Ed25519) are required.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import struct
import urllib.error
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

DEFAULT_REPOSITORY = "Master-Bayesian/Nioh3-Scroll-Generator"

# The installed production update key, exactly as the release tooling embeds it.
PUBLIC_KEY_RAW_B64 = "c6oPCnJE4B+7ZnDUkZRJzUo3PZQmlM/eMlFqRC1h3dU="

ONE_FILE_MAGIC = b"NIOH3_ONEFILE_V1"
FOOTER = struct.Struct("<16sQ32s")
LAUNCHER_PATH = "launcher/Nioh3Launcher.exe"
MANIFEST_SCHEMA = "nioh3-tauri-update/v1"
PORTABLE_SCHEMA = "nioh3-tauri-manifest/v1"
MAX_DOWNLOAD_BYTES = 60 * 1024 * 1024
RESERVED_DEVICE_NAMES = {"CON", "PRN", "AUX", "NUL"} | {
    f"{prefix}{index}" for prefix in ("COM", "LPT") for index in "123456789\u00b9\u00b2\u00b3"
}


def asset_names(version: str) -> list[str]:
    return [
        f"Nioh3Studio-{version}-win-x64.exe",
        f"Nioh3Studio-{version}-win-x64.exe.sha256",
        f"Nioh3Studio-{version}-win-x64.sha256",
        f"Nioh3Studio-{version}-win-x64.zip",
        "tauri-update.json",
        "test-inventory.json",
    ]


def release_base(repository: str) -> str:
    return f"https://github.com/{repository}/releases/download"


def tag_url(repository: str, version: str, name: str) -> str:
    return f"{release_base(repository)}/v{version}/{name}"


def safe_path(name: str) -> bool:
    path = PurePosixPath(name)
    return bool(name) and not path.is_absolute() and not any(c in name for c in '\\:<>"|?*') and all(
        part not in ("", ".", "..") and not part.endswith((" ", "."))
        and part.split(".")[0].upper() not in RESERVED_DEVICE_NAMES
        and not any(ord(c) < 32 for c in part)
        for part in name.split("/")
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def _auth_headers() -> dict[str, str]:
    headers = {"User-Agent": "nioh3-release-verify", "Accept": "application/vnd.github+json"}
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers=_auth_headers())
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def fetch_json(url: str) -> dict:
    return json.loads(fetch(url).decode("utf-8"))


def download(url: str, destination: Path) -> None:
    destination.write_bytes(fetch(url))


class Report:
    """Ordered check log with a single pass/fail roll-up."""

    def __init__(self) -> None:
        self.checks: list[dict] = []

    def record(self, name: str, ok: bool, detail: object) -> None:
        self.checks.append({"check": name, "ok": bool(ok), "detail": str(detail)})

    @property
    def passed(self) -> bool:
        return bool(self.checks) and all(entry["ok"] for entry in self.checks)


def verify_signature(manifest: dict) -> None:
    """Verify the production Ed25519 signature over the signed manifest body.

    The release tooling signs ``Buffer.from(JSON.stringify(manifest))``, so the
    payload keeps JS semantics: insertion order of the six signed keys, no
    whitespace, literal UTF-8.
    """
    payload = json.dumps(
        {
            "schema": manifest["schema"],
            "version": manifest["version"],
            "channel": manifest["channel"],
            "platform": manifest["platform"],
            "notes": manifest["notes"],
            "asset": manifest["asset"],
        },
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")
    key = Ed25519PublicKey.from_public_bytes(base64.b64decode(PUBLIC_KEY_RAW_B64))
    key.verify(base64.b64decode(manifest["signature"]), payload)


def require_x64_pe(raw: bytes) -> bool:
    if len(raw) < 0x40 or raw[:2] != b"MZ":
        return False
    offset = struct.unpack_from("<I", raw, 0x3C)[0]
    return (
        offset <= len(raw) - 24
        and raw[offset:offset + 4] == b"PE\0\0"
        and struct.unpack_from("<H", raw, offset + 4)[0] == 0x8664
    )


def inspect_zip(zip_path: Path, report: Report, expected_sha: str, version: str) -> dict:
    """Verify the internal update ZIP: coverage, safe unique paths, sizes, hashes, CRC."""
    summary: dict = {}
    try:
        archive = zipfile.ZipFile(zip_path)
    except zipfile.BadZipFile as error:
        report.record("zip_readable", False, str(error))
        return summary
    with archive:
        infos = archive.infolist()
        report.record("zip_readable", True, f"{len(infos)} members")

        unsafe = [entry.filename for entry in infos if not safe_path(entry.filename)]
        report.record("zip_paths_safe", not unsafe, "unsafe: " + ", ".join(unsafe[:5]) if unsafe else "all paths safe")

        folded = [entry.filename.casefold() for entry in infos]
        duplicates = sorted({name for name in folded if folded.count(name) > 1})
        report.record("zip_paths_unique", not duplicates, "duplicates: " + ", ".join(duplicates[:5]) if duplicates else "unique")

        directories = [entry.filename for entry in infos if entry.is_dir()]
        report.record("zip_no_directory_entries", not directories, str(len(directories)))

        try:
            manifest = json.loads(archive.read("build-manifest.json"))
        except KeyError:
            report.record("zip_manifest_present", False, "build-manifest.json missing")
            return summary
        except (json.JSONDecodeError, zipfile.BadZipFile) as error:
            report.record("zip_manifest_present", False, str(error))
            return summary
        report.record("zip_manifest_present", True, "build-manifest.json readable")

        report.record("zip_manifest_schema", manifest.get("schema") == PORTABLE_SCHEMA, str(manifest.get("schema")))
        report.record("zip_manifest_version", manifest.get("version") == version, str(manifest.get("version")))

        members = set(archive.namelist())
        try:
            declared = {entry["path"]: entry for entry in manifest.get("files", [])}
            body = members - {"build-manifest.json"}
            report.record("zip_coverage_exact", body == set(declared), f"members={len(members)} declared={len(declared)}")

            size_mismatch = [entry["path"] for entry in manifest.get("files", []) if archive.getinfo(entry["path"]).file_size != entry["size"]]
            report.record("zip_member_sizes", not size_mismatch, ", ".join(size_mismatch[:5]) if size_mismatch else "0 mismatch")

            hash_mismatch = [
                entry["path"]
                for entry in manifest.get("files", [])
                if hashlib.sha256(archive.read(entry["path"])).hexdigest() != entry["sha256"]
            ]
            report.record("zip_member_sha256", not hash_mismatch, ", ".join(hash_mismatch[:5]) if hash_mismatch else "0 mismatch")
        except (KeyError, TypeError, zipfile.BadZipFile) as error:
            report.record("zip_manifest_members", False, f"{type(error).__name__}: malformed build-manifest files list")
            return summary

        corrupt = archive.testzip()
        report.record("zip_member_crc", corrupt is None, str(corrupt))

        git = manifest.get("git", {})
        report.record("zip_source_sha", git.get("commit") == expected_sha, str(git.get("commit")))
        report.record("zip_source_clean", git.get("dirty") is False, str(git.get("dirty")))

        launcher = archive.read(LAUNCHER_PATH) if LAUNCHER_PATH in members else b""
        report.record("zip_launcher_present", bool(launcher) and require_x64_pe(launcher), f"{len(launcher)} bytes")

        summary.update(
            {
                "schema": manifest.get("schema"),
                "version": manifest.get("version"),
                "members": len(members),
                "declared": len(declared),
                "crc_corrupt_member": corrupt,
                "manifest_git": git,
                "launcher": launcher,
            }
        )
    return summary


def verify_outer(exe_path: Path, report: Report, zip_summary: dict) -> dict:
    """Verify the outer one-file EXE footer, embedded payload, and launcher stub."""
    raw = exe_path.read_bytes()
    report.record("outer_size_budget", len(raw) <= MAX_DOWNLOAD_BYTES, f"{len(raw)} bytes / {MAX_DOWNLOAD_BYTES} max")
    if len(raw) <= FOOTER.size:
        report.record("outer_footer_present", False, "executable is smaller than one footer")
        return {}
    footer = raw[-FOOTER.size:]
    magic, declared_size, footer_digest = FOOTER.unpack(footer)
    stub_len = len(raw) - FOOTER.size - declared_size
    embedded = raw[stub_len:len(raw) - FOOTER.size] if stub_len >= 0 else b""
    stub = raw[:stub_len] if stub_len >= 0 else b""

    report.record("outer_footer_magic", magic == ONE_FILE_MAGIC, magic.decode("ascii", "replace"))
    report.record("outer_payload_bytes_match_zip", declared_size == zip_summary.get("zip_bytes"), f"{declared_size} vs {zip_summary.get('zip_bytes')}")
    embedded_digest = hashlib.sha256(embedded).hexdigest()
    report.record("outer_payload_sha256_match_zip", embedded_digest == zip_summary.get("zip_sha256"), embedded_digest)
    report.record("outer_footer_hash_self_consistent", footer_digest.hex() == embedded_digest, footer_digest.hex())

    launcher = zip_summary.get("launcher") or b""
    report.record("outer_stub_matches_zip_launcher", bool(launcher) and stub == launcher, f"stub={len(stub)} launcher={len(launcher)}")
    report.record("outer_stub_is_x64_pe", require_x64_pe(stub), f"{len(stub)} bytes")

    return {
        "footer_magic_ok": magic == ONE_FILE_MAGIC,
        "declared_payload_bytes": declared_size,
        "embedded_payload_sha256": embedded_digest,
        "footer_payload_sha256": footer_digest.hex(),
        "stub_bytes": len(stub),
    }


def verify_local(directory: Path, version: str, expected_sha: str, repository: str, report: Report) -> dict:
    """Verify the six assets in ``directory``; returns the report body."""
    names = asset_names(version)
    present = sorted(path.name for path in directory.iterdir() if path.is_file())
    missing = [name for name in names if name not in present]
    unexpected = [name for name in present if name not in names]
    report.record("assets_present_exact", not missing, "missing: " + ", ".join(missing) if missing else "all six present")
    report.record("assets_no_unexpected", not unexpected, ", ".join(unexpected) if unexpected else "none")
    if missing:
        return {"assets": {}, "zip": {}, "outer": {}}

    exe_name, exe_sidecar_name, zip_sidecar_name, zip_name = names[0], names[1], names[2], names[3]
    assets = {
        name: {
            "name": name,
            "bytes": (directory / name).stat().st_size,
            "sha256": sha256_file(directory / name),
        }
        for name in names
    }
    exe_sha = assets[exe_name]["sha256"]
    zip_sha = assets[zip_name]["sha256"]

    exe_sidecar = (directory / exe_sidecar_name).read_text(encoding="utf-8").split()[0]
    zip_sidecar = (directory / zip_sidecar_name).read_text(encoding="utf-8").split()[0]
    report.record("exe_sidecar_matches", exe_sidecar == exe_sha, exe_sidecar)
    report.record("zip_sidecar_matches", zip_sidecar == zip_sha, zip_sidecar)

    try:
        manifest = json.loads((directory / "tauri-update.json").read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as error:
        report.record("manifest_json_valid", False, str(error))
        return {"assets": assets, "zip": {}, "outer": {}}
    report.record("manifest_json_valid", True, "parsed")
    asset = manifest.get("asset", {})
    report.record("manifest_schema", manifest.get("schema") == MANIFEST_SCHEMA, str(manifest.get("schema")))
    report.record("manifest_version", manifest.get("version") == version, str(manifest.get("version")))
    report.record("manifest_channel_stable", manifest.get("channel") == "stable", str(manifest.get("channel")))
    report.record("manifest_platform", manifest.get("platform") == "win32-x64", str(manifest.get("platform")))
    report.record("manifest_asset_name", asset.get("name") == zip_name, str(asset.get("name")))
    report.record("manifest_asset_size", asset.get("size") == assets[zip_name]["bytes"], str(asset.get("size")))
    report.record("manifest_asset_sha256", asset.get("sha256") == zip_sha, str(asset.get("sha256")))
    report.record("manifest_asset_url", asset.get("url") == tag_url(repository, version, zip_name), str(asset.get("url")))
    try:
        verify_signature(manifest)
        report.record("manifest_ed25519_production_key", True, "signature verified with the installed public key")
    except Exception as error:  # noqa: BLE001 - any signature failure is a failed check
        report.record("manifest_ed25519_production_key", False, f"{type(error).__name__}: {error}")

    report.record("zip_size_budget", assets[zip_name]["bytes"] <= MAX_DOWNLOAD_BYTES, f"{assets[zip_name]['bytes']} bytes / {MAX_DOWNLOAD_BYTES} max")
    zip_summary = inspect_zip(directory / zip_name, report, expected_sha, version)
    zip_summary.update({"zip_bytes": assets[zip_name]["bytes"], "zip_sha256": zip_sha})

    try:
        json.loads((directory / "test-inventory.json").read_text(encoding="utf-8"))
        report.record("test_inventory_json_valid", True, "parsed")
    except (json.JSONDecodeError, OSError) as error:
        report.record("test_inventory_json_valid", False, str(error))

    outer = verify_outer(directory / exe_name, report, zip_summary)
    return {"assets": assets, "zip": {k: v for k, v in zip_summary.items() if k != "launcher"}, "outer": outer}


def verify_public(directory: Path, version: str, expected_sha: str, repository: str, download_dir: Path | None, report: Report) -> dict:
    """Verify the live GitHub release, and optionally a fresh public re-download."""
    expected_names = asset_names(version)
    summary: dict = {}
    try:
        latest = fetch_json(f"https://api.github.com/repos/{repository}/releases/latest")
        report.record("latest_reachable", True, f"id={latest.get('id')}")
    except (urllib.error.URLError, urllib.error.HTTPError, json.JSONDecodeError) as error:
        report.record("latest_reachable", False, f"{type(error).__name__}: {error}")
        return summary

    report.record("latest_tag", latest.get("tag_name") == f"v{version}", str(latest.get("tag_name")))
    report.record("latest_not_draft", latest.get("draft") is False, str(latest.get("draft")))
    report.record("latest_not_prerelease", latest.get("prerelease") is False, str(latest.get("prerelease")))
    published = sorted(entry["name"] for entry in latest.get("assets", []))
    report.record("latest_asset_names", published == sorted(expected_names), ",".join(published))

    local_sha = {name: sha256_file(directory / name) for name in expected_names}
    digests = {entry["name"]: entry.get("digest") for entry in latest.get("assets", [])}
    comparable = {name: value for name, value in digests.items() if value}
    if comparable:
        mismatched = [name for name, digest in comparable.items() if digest != f"sha256:{local_sha[name]}"]
        report.record("public_asset_digests_match", not mismatched, ", ".join(mismatched) if mismatched else f"{len(comparable)} digests match")

    try:
        latest_manifest = fetch_json(f"https://github.com/{repository}/releases/latest/download/tauri-update.json")
        report.record("latest_download_manifest_version", latest_manifest.get("version") == version, str(latest_manifest.get("version")))
        report.record("latest_download_manifest_channel", latest_manifest.get("channel") == "stable", str(latest_manifest.get("channel")))
    except (urllib.error.URLError, urllib.error.HTTPError, json.JSONDecodeError) as error:
        report.record("latest_download_manifest_version", False, f"{type(error).__name__}: {error}")

    if download_dir is not None:
        summary["download_dir"] = str(download_dir)
        if download_dir.exists() and any(download_dir.iterdir()):
            report.record("public_redownload_dir_fresh", False, f"{download_dir} exists and is not empty")
        else:
            report.record("public_redownload_dir_fresh", True, str(download_dir))
            download_dir.mkdir(parents=True, exist_ok=True)
            matched = 0
            for name in expected_names:
                target = download_dir / name
                try:
                    download(tag_url(repository, version, name), target)
                except (urllib.error.URLError, urllib.error.HTTPError) as error:
                    report.record(f"public_redownload:{name}", False, f"{type(error).__name__}: {error}")
                    continue
                same = sha256_file(target) == local_sha[name]
                matched += int(same)
                report.record(f"public_redownload:{name}", same, sha256_file(target))
            report.record("public_redownload_byte_identity", matched == len(expected_names), f"{matched}/{len(expected_names)} assets match the verified copies")

    summary.update({"release_id": latest.get("id"), "release_url": f"https://github.com/{repository}/releases/tag/v{version}"})
    return summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--directory", type=Path, required=True, help="directory holding exactly the six release assets")
    parser.add_argument("--version", required=True, help="release version, without the leading v")
    parser.add_argument("--expected-sha", required=True, help="full candidate commit recorded by build-manifest.json")
    parser.add_argument("--report", type=Path, required=True, help="path for the JSON verification report")
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY, help="owner/repo of the release")
    parser.add_argument("--public", action="store_true", help="also verify the live GitHub release")
    parser.add_argument("--public-download-dir", type=Path, help="fresh dir for a public re-download (implies --public)")
    args = parser.parse_args(argv)

    public = args.public or args.public_download_dir is not None
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?", args.version):
        parser.error(f"--version is not a release version: {args.version!r}")
    if not re.fullmatch(r"[0-9a-f]{40}", args.expected_sha):
        parser.error(f"--expected-sha is not a full lowercase commit: {args.expected_sha!r}")
    if not args.directory.is_dir():
        parser.error(f"--directory is not a directory: {args.directory}")

    report = Report()
    body = verify_local(args.directory, args.version, args.expected_sha, args.repository, report)
    if public:
        if body.get("assets"):
            body["public"] = verify_public(args.directory, args.version, args.expected_sha, args.repository, args.public_download_dir, report)
        else:
            report.record("public_checks_skipped", False, "the local six assets must verify before their public identity is compared")
            body["public"] = {}

    result = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "repository": args.repository,
        "version": args.version,
        "candidate_sha": args.expected_sha,
        "mode": "public" if public else "local",
        "directory": str(args.directory),
        "assets": body.get("assets", {}),
        "zip": body.get("zip", {}),
        "outer": body.get("outer", {}),
        "checks": report.checks,
        "passed": report.passed,
    }
    if public:
        result["public"] = body.get("public", {})
    text = json.dumps(result, indent=2, sort_keys=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(text + "\n", encoding="utf-8")
    print(text)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
