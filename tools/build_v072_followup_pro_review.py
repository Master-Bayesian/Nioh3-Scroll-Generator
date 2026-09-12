from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DELIVERABLES = ROOT / "deliverables"
DELIVERY_NAME = "Nioh3_v0.7.2_Followup_Code_Review_20260911"
OUTPUT_ROOT = DELIVERABLES / DELIVERY_NAME
ZIP_PATH = DELIVERABLES / f"{DELIVERY_NAME}.zip"

PROJECT_TREES = (
    ".github/workflows",
    "apps/desktop",
    "apps/tauri",
    "apps/workshop",
    "docs/knowledge",
    "nioh3_scroll_editor",
    "packages",
    "packaging",
    "tests",
    "test_fixtures",
    "tools",
)

PROJECT_FILES = (
    ".gitignore",
    "AGENTS.md",
    "README.md",
    "README.en.md",
    "THIRD_PARTY_NOTICES.md",
    "package.json",
    "package-lock.json",
    "requirements.txt",
    "requirements-dev.txt",
    "tsconfig.json",
    "emaki_exchange.py",
    "launch_editor.py",
    "launch_protected_worker.py",
    "launch_search_worker.py",
    "nioh3_seed_math.py",
    "scroll_lab.py",
)

EXCLUDED_DIRECTORY_NAMES = {
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
}

README_TEXT = """# Nioh 3 Studio v0.7.2 follow-up code review

This package contains the current dirty worktree needed to review three related
product fixes: intermittent live insertion, title-screen save insertion, and
support logging. It also records the already completed cart prominence and
standalone installer. Earlier NG1/NG2 evidence covers inserted scroll payloads,
not the running game's actual progression.

Reading order:

1. `TASK_FOR_PRO.md`
2. `ENVIRONMENT.json`
3. `validation/RESULTS.json`
4. `git/STATUS_SHORT.txt` and `git/WORKTREE.diff`
5. `project-source/docs/knowledge/V072_FOLLOWUP_FIXES_20260911.md`
6. `evidence/v0.7.1-ui-only-log.png`

The source snapshot includes tracked and relevant untracked files as they exist
in the worktree. Build output, private saves, game binaries, memory dumps,
signing material, caches, and unrelated root scratch scripts are excluded.

The possessed-enemy inverse problem and product telemetry are explicitly out of
scope. The former is frozen because seed 86872488 is not an offline positive;
the latter remains a separate pending product decision.
"""

TASK_TEXT = """# Pro task: review v0.7.2 follow-up fixes

Review the supplied current worktree and return prioritized findings plus
concrete patches and regression tests. Do not rely on chat history.

## P0: title-screen save insertion

Review `SaveApplication`, `SaveInstaller`, and the new quiescence, related-file
fingerprint, backup, journal, durable replacement, readback, and rollback code.
The product requirement is to support adding generated scrolls while Nioh 3 is
running at the title screen. A user reported that adding at the title screen and
then exiting without entering the game can sometimes corrupt the save, possibly
on exit or on the next launch.

Determine whether the current implementation prevents the real delayed
overwrite or stale in-memory-save failure, rather than only detecting concurrent
file writes. Identify the exact remaining race or game ownership issue and
propose the smallest safe implementation that preserves title-screen insertion.
Do not solve this by requiring the game to be completely closed. Preserve
automatic backups and append-only generated-scroll scope. Add fault tests for
every mechanism you identify. Clearly separate synthetic confidence from live
acceptance that still requires the game.

## P0: intermittent native live insertion

Review the full renderer -> Tauri broker -> protected Python worker ->
`LiveAddApplication` -> `LiveAddAdapter` -> native transport path. Some users
reported three failures followed by one success, and others reported that
restarting the application made live insertion work again.

The current patch fixes one concrete cause: `LiveAddAdapter` used to retain a
false `pending` owner when the default native transport rejected a request and
could prove that no in-memory or durable receipt existed. Validate that `_submit`
releases ownership only after this proof and preserves no-replay behavior for
timeout, disconnect, receipt-creation failure, late receipt, and uncertain native
ownership. Find any other lifecycle holes across
preview retries, process-ID reuse, batch children, worker restart, native hook
cleanup, and receipt recovery. Explain each user-visible failure mode and supply
focused patches/tests.

## P1: support diagnostics

Review bounded logging and automatic clipboard replacement. A v0.7.1 user log
contained only UI status lines and was insufficient to diagnose live insertion.
Current code traces operation inputs/results, broker worker payloads/errors,
candidate raw records, save paths, receipts, and worker stderr. An operation
failure automatically replaces the clipboard with diagnostics. Copy Log now
includes the newest 128,000 bytes across rotated files plus version, data/log
directories, package verification, and worker state. Disk use is capped at the
current 4 MiB segment plus four archives.

Confirm that the first actionable failure is retained, rotation order and UTF-8
tail behavior are correct, clipboard capture cannot mask the original error,
and the output contains enough source path, candidate bytes, operation identity,
native receipt, and worker error to reproduce both save and live failures.
Patch any omission without adding unbounded logging.

## Already covered; verify for regressions only

- Add to cart has a high-contrast gold action and selected state.
- v0.7.2 has a standalone setup EXE; users do not need the portable ZIP.
- Earlier live-game acceptance covered NG1-NG3 scroll payloads x R3/R4/R5,
  followed by save/reload. It did not vary or record the running character's
  actual progression and does not prove NG1/NG2 runtime compatibility.

## Pending live matrix

Review whether the native live-insertion implementation has any dependency on
the running character's currently selected progression, unlocked content,
inventory initialization, scheduler state, or native database state. Current
static inspection exposes no explicit runtime-progression gate. One controlled
test inserted R3 seed 10032001 while the game was actually running in NG1:
inventory 41 -> 42, serial 2446282, slot 29, prior records and native index
preserved, source save unchanged, and cleanup verified. Normal save/reload was
not performed. The user deferred actual-NG2 testing until a matching user report.
Treat NG1 persistence and actual-NG2 compatibility as unaccepted, and keep them
separate from the candidate scroll's own playthrough/category field.

## Out of scope

- local or cross-installation usage statistics;
- web edition work;
- possessed Crucible enemy selection or inversion;
- rewriting verified RNG, R4 finalization, or generation semantics;
- publishing a release.

## Required response

1. Findings ordered by severity, with exact file/line references.
2. Root-cause assessment for each P0 issue and any remaining unknowns.
3. Minimal code patches and meaningful regression tests.
4. A short live acceptance sequence for the title-screen and intermittent-live
   paths after the code review is integrated.
5. A release recommendation: block, conditional, or ready, with explicit gates.
"""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def git_text(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    return result.stdout


def ignore_tree(_directory: str, names: list[str]) -> set[str]:
    return {name for name in names if name in EXCLUDED_DIRECTORY_NAMES}


def copy_project(destination: Path) -> None:
    for relative in PROJECT_TREES:
        source = ROOT / relative
        if source.is_dir():
            shutil.copytree(
                source,
                destination / relative,
                dirs_exist_ok=True,
                ignore=ignore_tree,
            )
    for relative in PROJECT_FILES:
        source = ROOT / relative
        if source.is_file():
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)


def write_git_context(destination: Path) -> dict[str, object]:
    target = destination / "git"
    target.mkdir(parents=True, exist_ok=True)
    commit = git_text("rev-parse", "HEAD").strip()
    branch = git_text("branch", "--show-current").strip()
    status = git_text("status", "--short")
    files = {
        "COMMIT.txt": commit + "\n",
        "BRANCH.txt": branch + "\n",
        "STATUS_SHORT.txt": status,
        "WORKTREE.diff": git_text("diff", "--binary", "HEAD"),
        "STAGED.diff": git_text("diff", "--binary", "--cached"),
        "UNTRACKED_FILES.txt": git_text(
            "ls-files", "--others", "--exclude-standard"
        ),
    }
    for name, content in files.items():
        (target / name).write_text(content, encoding="utf-8", newline="\n")
    return {
        "commit": commit,
        "branch": branch,
        "dirty": bool(status.strip()),
        "status_short": status.splitlines(),
    }


def copy_optional_evidence(destination: Path) -> list[dict[str, object]]:
    evidence = destination / "evidence"
    evidence.mkdir(parents=True, exist_ok=True)
    copied: list[dict[str, object]] = []
    sources = (
        (
            Path(os.environ.get("LOCALAPPDATA", ""))
            / "Temp"
            / "codex-clipboard-b7ccfe63-ed32-495c-b0a2-17090bd45237.png",
            "v0.7.1-ui-only-log.png",
        ),
        (
            Path(os.environ.get("APPDATA", ""))
            / "io.github.master-bayesian.nioh3-studio"
            / "logs"
            / "desktop.log",
            "current-desktop.log",
        ),
        (
            ROOT
            / ".codex_tmp"
            / "runtime-progression-live-add"
            / "ng1-20260911-r2"
            / "prepared.json",
            "actual-ng1-live-add-prepared.json",
        ),
        (
            ROOT
            / ".codex_tmp"
            / "runtime-progression-live-add"
            / "ng1-20260911-r2"
            / "verification.json",
            "actual-ng1-live-add-verification.json",
        ),
        (
            ROOT
            / ".codex_tmp"
            / "runtime-progression-live-add"
            / "ng1-20260911-r2"
            / "runtime-context.json",
            "actual-ng1-runtime-context.json",
        ),
    )
    for source, name in sources:
        if source.is_file():
            target = evidence / name
            shutil.copy2(source, target)
            copied.append(
                {"path": f"evidence/{name}", "bytes": target.stat().st_size, "sha256": sha256(target)}
            )
    return copied


def release_inventory() -> list[dict[str, object]]:
    release = ROOT / "deliverables" / "releases" / "v0.7.2"
    if not release.is_dir():
        return []
    return [
        {
            "name": path.name,
            "bytes": path.stat().st_size,
            "sha256": sha256(path),
        }
        for path in sorted(release.iterdir())
        if path.is_file()
    ]


def write_hash_manifest(root: Path) -> None:
    rows = []
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.name != "SHA256SUMS.txt":
            rows.append(f"{sha256(path)}  {path.relative_to(root).as_posix()}")
    (root / "SHA256SUMS.txt").write_text("\n".join(rows) + "\n", encoding="utf-8")


def build(validation_path: Path) -> dict[str, object]:
    DELIVERABLES.mkdir(parents=True, exist_ok=True)
    if OUTPUT_ROOT.parent.resolve() != DELIVERABLES.resolve():
        raise RuntimeError("Output path escaped deliverables")
    if OUTPUT_ROOT.exists():
        shutil.rmtree(OUTPUT_ROOT)
    if ZIP_PATH.exists():
        ZIP_PATH.unlink()
    OUTPUT_ROOT.mkdir()

    git = write_git_context(OUTPUT_ROOT)
    project = OUTPUT_ROOT / "project-source"
    project.mkdir()
    copy_project(project)
    evidence = copy_optional_evidence(OUTPUT_ROOT)

    validation_target = OUTPUT_ROOT / "validation" / "RESULTS.json"
    validation_target.parent.mkdir(parents=True)
    shutil.copy2(validation_path, validation_target)

    environment = {
        "schema": "nioh3-v072-followup-review/v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "platform": os.name,
        "application_version": "0.7.2",
        "game_version": "PC v2.01",
        "git": git,
        "evidence": evidence,
        "release_artifacts": release_inventory(),
        "live_acceptance": {
            "previous_matrix": "NG1-NG3 scroll payloads x R3-R5 inserted and save/reload verified",
            "running_game_ng1": "current-memory insertion passed for R3 seed 10032001; persistence not tested",
            "running_game_ng2": "deferred until a matching user report; not accepted",
            "current_title_exit_restart": "pending controlled acceptance",
            "current_live_add_rejection_fix": "synthetic regression only",
        },
    }
    (OUTPUT_ROOT / "README.md").write_text(README_TEXT, encoding="utf-8")
    (OUTPUT_ROOT / "TASK_FOR_PRO.md").write_text(TASK_TEXT, encoding="utf-8")
    (OUTPUT_ROOT / "ENVIRONMENT.json").write_text(
        json.dumps(environment, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    write_hash_manifest(OUTPUT_ROOT)

    with zipfile.ZipFile(ZIP_PATH, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(OUTPUT_ROOT.rglob("*")):
            if path.is_file():
                archive.write(path, f"{DELIVERY_NAME}/{path.relative_to(OUTPUT_ROOT).as_posix()}")
    with zipfile.ZipFile(ZIP_PATH) as archive:
        bad = archive.testzip()
        names = archive.namelist()
    if bad:
        raise RuntimeError(f"ZIP CRC failure: {bad}")
    required = {
        f"{DELIVERY_NAME}/README.md",
        f"{DELIVERY_NAME}/TASK_FOR_PRO.md",
        f"{DELIVERY_NAME}/ENVIRONMENT.json",
        f"{DELIVERY_NAME}/SHA256SUMS.txt",
        f"{DELIVERY_NAME}/validation/RESULTS.json",
        f"{DELIVERY_NAME}/project-source/nioh3_scroll_editor/live_add_adapter.py",
        f"{DELIVERY_NAME}/project-source/nioh3_scroll_editor/savegame.py",
    }
    missing = sorted(required.difference(names))
    if missing:
        raise RuntimeError(f"ZIP is missing required files: {missing}")
    return {
        "directory": str(OUTPUT_ROOT),
        "zip": str(ZIP_PATH),
        "zip_bytes": ZIP_PATH.stat().st_size,
        "zip_sha256": sha256(ZIP_PATH),
        "file_count": sum(1 for path in OUTPUT_ROOT.rglob("*") if path.is_file()),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validation", type=Path, required=True)
    args = parser.parse_args()
    validation = args.validation.resolve(strict=True)
    print(json.dumps(build(validation), indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
