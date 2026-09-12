from __future__ import annotations

"""Capture read-only title-save lifecycle fingerprints for native research."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable


SCHEMA = "nioh3-title-save-lifecycle/v1"
SAFE_LABEL = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$")
SAVE_SLOT = re.compile(r"^SAVEDATA(?P<index>[0-9]{2})$", re.IGNORECASE)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def discover_save_paths() -> list[Path]:
    root = (
        Path(os.environ.get("LOCALAPPDATA", ""))
        / "KoeiTecmo"
        / "NIOH3"
        / "Savedata"
    )
    if not root.is_dir():
        return []
    candidates = []
    for path in root.glob("*/SAVEDATA??/SAVEDATA.BIN"):
        if path.is_file() and SAVE_SLOT.fullmatch(path.parent.name):
            candidates.append(path.resolve())
    return sorted(candidates, key=lambda value: str(value).casefold())


def related_save_paths(save_path: Path) -> tuple[tuple[str, Path], ...]:
    resolved = save_path.resolve()
    return (
        ("main_save", resolved),
        ("game_backup", resolved.parent / "BACKUP.BIN"),
        (
            "system_save",
            resolved.parent.parent / "SYSTEMSAVEDATA00" / "SAVEDATA.BIN",
        ),
    )


def stable_fingerprint(role: str, path: Path) -> dict[str, object]:
    try:
        before = path.stat()
    except FileNotFoundError:
        return {
            "role": role,
            "path": str(path),
            "exists": False,
            "size": None,
            "mtime_ns": None,
            "sha256": None,
        }
    digest = sha256_file(path)
    after = path.stat()
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise RuntimeError(f"{role} changed while its fingerprint was captured")
    return {
        "role": role,
        "path": str(path),
        "exists": True,
        "size": after.st_size,
        "mtime_ns": after.st_mtime_ns,
        "sha256": digest,
    }


def process_snapshot() -> list[dict[str, object]]:
    if os.name != "nt":
        return []
    # Match CE observer receipts: FILETIME UTC as an integer string, not a
    # locale-dependent CIM DateTime CSV field. No code executes in the target.
    command = (
        "$ErrorActionPreference='Stop';@([Diagnostics.Process]::GetProcessesByName('Nioh3') | "
        "ForEach-Object { [pscustomobject]@{pid=$_.Id;"
        "creation_filetime=$_.StartTime.ToUniversalTime().ToFileTimeUtc().ToString();"
        "executable_path=$_.MainModule.FileName} }) | ConvertTo-Json -Compress"
    )
    try:
        result = subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command", command],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            encoding="utf-8-sig", timeout=15, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return [{"capture_error": f"Process identity capture failed: {type(exc).__name__}"}]

    if result.returncode != 0:
        return [{"capture_error": result.stderr.strip()[:512] or f"exit {result.returncode}"}]
    if len(result.stdout) > 16384:
        raise RuntimeError("Process identity output exceeded its bound")
    rows = json.loads(result.stdout or "[]")
    if isinstance(rows, dict):
        rows = [rows]
    if not isinstance(rows, list) or len(rows) > 16:
        raise RuntimeError("Invalid process identity inventory")
    for row in rows:
        if (not isinstance(row, dict) or type(row.get("pid")) is not int or row["pid"] <= 0
                or not isinstance(row.get("creation_filetime"), str)
                or not row["creation_filetime"].isdigit()
                or len(row["creation_filetime"]) > 20
                or not isinstance(row.get("executable_path"), str)
                or len(row["executable_path"]) > 2048):
            raise RuntimeError("Incomplete process instance identity")
    return rows


def reserve_capture_directory(run_root: Path, stage: str) -> tuple[int, Path]:
    """Reserve a monotonically numbered directory, including incomplete runs.

    Older code counted *.json directly under run_root, but snapshots actually
    live in numbered subdirectories. That reset the sequence to one each time.
    Atomic mkdir also prevents two collectors from selecting the same number.
    """
    require_label(stage, "stage")
    run_root.mkdir(parents=True, exist_ok=True)
    for _ in range(1000):
        numbers = [int(m.group(1)) for p in run_root.iterdir()
                   if (m := re.match(r"^(\d{3})_", p.name))]
        sequence = max(numbers, default=0) + 1
        if sequence > 999:
            raise RuntimeError("Capture stage limit reached; start a new run ID")
        target = run_root / f"{sequence:03d}_{stage}"
        try:
            target.mkdir(exist_ok=False)
            return sequence, target
        except FileExistsError:
            continue
    raise RuntimeError("Could not reserve a capture directory")


def save_identity(path: Path) -> dict[str, object]:
    match = SAVE_SLOT.fullmatch(path.parent.name)
    if match is None:
        raise ValueError(f"Unrecognized character-save directory: {path.parent}")
    account_folder = path.parents[1].name
    account_key = hashlib.sha256(account_folder.encode("utf-8")).hexdigest()[:16]
    return {
        "account_key": account_key,
        "save_slot": int(match.group("index")),
        "save_path": str(path),
    }


def capture_files(
    output: Path,
    fingerprints: Iterable[dict[str, object]],
) -> list[dict[str, object]]:
    private = output / "private-files"
    private.mkdir(parents=True, exist_ok=False)
    captures = []
    for item in fingerprints:
        if not item["exists"]:
            continue
        source = Path(str(item["path"]))
        target = private / f"{item['role']}.bin"
        shutil.copy2(source, target)
        copied_hash = sha256_file(target)
        after = stable_fingerprint(str(item["role"]), source)
        if after["sha256"] != item["sha256"] or copied_hash != item["sha256"]:
            raise RuntimeError(f"{item['role']} changed while its private copy was captured")
        captures.append(
            {
                "role": item["role"],
                "relative_path": target.relative_to(output).as_posix(),
                "size": target.stat().st_size,
                "sha256": copied_hash,
                "private_do_not_package": True,
            }
        )
    return captures


def require_label(value: str, field: str) -> str:
    if not SAFE_LABEL.fullmatch(value):
        raise ValueError(f"{field} must match {SAFE_LABEL.pattern}")
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="list detected character saves")
    parser.add_argument("--save-path", type=Path, help="target character SAVEDATA.BIN")
    parser.add_argument("--run-id", help="controlled run ID, for example C1")
    parser.add_argument("--stage", help="stage label, for example after_editor_commit")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path(".codex_tmp/title-save-ownership-captures"),
    )
    parser.add_argument("--note", default="")
    parser.add_argument(
        "--capture-private-files",
        action="store_true",
        help="copy encrypted save files locally; never include these in a handoff",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    detected = discover_save_paths()
    if args.list:
        print(json.dumps([save_identity(path) for path in detected], indent=2))
        return 0

    if not args.run_id or not args.stage:
        raise ValueError("--run-id and --stage are required for a capture")
    run_id = require_label(args.run_id, "run ID")
    stage = require_label(args.stage, "stage")
    if args.save_path is None:
        if len(detected) != 1:
            raise ValueError(
                f"Detected {len(detected)} character saves; pass --save-path from --list"
            )
        save_path = detected[0]
    else:
        save_path = args.save_path.resolve(strict=True)
    if save_path.name.upper() != "SAVEDATA.BIN":
        raise ValueError("--save-path must be a character SAVEDATA.BIN")

    run_root = args.output_root.resolve() / run_id
    run_root.mkdir(parents=True, exist_ok=True)
    sequence, capture_root = reserve_capture_directory(run_root, stage)

    fingerprints = [
        stable_fingerprint(role, path) for role, path in related_save_paths(save_path)
    ]
    private_files = (
        capture_files(capture_root, fingerprints)
        if args.capture_private_files
        else []
    )
    result = {
        "schema": SCHEMA,
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "read_only": True,
        "capture_is_atomic": False,
        "native_save_ownership": "not_acquired",
        "run_id": run_id,
        "sequence": sequence,
        "stage": stage,
        "note": args.note,
        "save": save_identity(save_path),
        "game_processes": process_snapshot(),
        "files": fingerprints,
        "private_files": private_files,
    }
    output = capture_root / "snapshot.json"
    output.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(output)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError) as error:
        print(f"capture failed: {error}", file=sys.stderr)
        raise SystemExit(2)
