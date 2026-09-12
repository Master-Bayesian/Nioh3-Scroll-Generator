from __future__ import annotations

"""Build the PC v2.01 title-save ownership reverse-engineering handoff."""

import hashlib
import json
import shutil
import subprocess
import zipfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DELIVERABLES = ROOT / "deliverables"
DELIVERY_NAME = "Nioh3_Title_Save_Ownership_Pro_Handoff_PC_v2.01_20260911_r2"
OUTPUT_ROOT = DELIVERABLES / DELIVERY_NAME
ZIP_PATH = DELIVERABLES / f"{DELIVERY_NAME}.zip"
SECTIONS = ROOT / "audit" / "runtime_sections" / "v2.0.1.0_20260902_title"
PREP_SNAPSHOT = (
    ROOT
    / ".codex_tmp"
    / "title-save-ownership-captures"
    / "PREP"
    / "001_game_closed_baseline"
    / "snapshot.json"
)


PROJECT_FILES = (
    "AGENTS.md",
    "docs/knowledge/CURRENT_HANDOFF.md",
    "docs/knowledge/NATIVE_LIVE_ADD_EXECUTOR_20260909.md",
    "docs/knowledge/RESEARCH_HANDOFF_WORKFLOW.md",
    "docs/knowledge/TITLE_SAVE_OWNERSHIP_RESEARCH_PLAN_20260911.md",
    "docs/knowledge/V072_PRO_REVIEW_INTEGRATION_20260911.md",
    "nioh3_scroll_editor/live_add_adapter.py",
    "nioh3_scroll_editor/live_add_application.py",
    "nioh3_scroll_editor/live_add_native_transport.py",
    "nioh3_scroll_editor/live_add_operations.py",
    "nioh3_scroll_editor/native_submission_guard.py",
    "nioh3_scroll_editor/process_instance.py",
    "nioh3_scroll_editor/protected_jobs.py",
    "nioh3_scroll_editor/save_application.py",
    "nioh3_scroll_editor/savegame.py",
    "tests/test_live_add_adapter.py",
    "tests/test_live_add_application.py",
    "tests/test_save_commit_guard.py",
    "tests/test_single_native_insertion.py",
    "tests/test_v072_live_lifecycle.py",
    "tests/test_v072_save_races.py",
    "tools/capture_title_save_lifecycle.py",
)


EVIDENCE_FILES = (
    "research/ILLEGAL_AUXILIARY_OVERRIDE_PROOF.md",
    "research/map_live_stack_owners.py",
    "research/owned_breakpoint_lifecycle_ce.lua",
    "research/probe_save_record_serialization_ce.lua",
    "research/probe_save_write_origin_ce.lua",
)


README = """# Nioh 3 PC v2.01 title-save ownership handoff

This private research package contains the current application code and native
binary evidence needed to solve the remaining v0.7.2 publication blocker.
Title-screen disk insertion can pass every external transaction check and still
be replaced later by the game's older in-memory generation during exit.

Reading order:

1. `TASK_FOR_PRO.md`
2. `project-source/docs/knowledge/TITLE_SAVE_OWNERSHIP_RESEARCH_PLAN_20260911.md`
3. `ENVIRONMENT.json`
4. `evidence/runtime-sections/manifest.json`
5. `evidence/PC_V2_00_02_STALE_LEADS.md`
6. `project-source/nioh3_scroll_editor/save_application.py`
7. `project-source/nioh3_scroll_editor/savegame.py`
8. `git/STATUS_SHORT.txt` and `git/WORKTREE.diff`

The raw `.text`, `.rdata` and `.pdata` files are memory section dumps from the
matching PC v2.01 executable identity. They are included for private analysis
and must not be redistributed.

The old Cheat Engine observers and the RVAs in the v2.00.02 evidence are stale
leads. They are supplied to recover function shapes and capture requirements;
they must not be attached to v2.01 unchanged. In particular, the old
`probe_save_write_origin_ce.lua` clears every breakpoint in the session and is
not acceptable as the new observer.

No live C0-C3 title lifecycle run is included. The supplied PREP snapshot was
taken while the game was closed and proves only that the read-only fingerprint
tool works on the current three-file save layout. Do not infer native ownership
or release readiness from it.
"""


TASK = """# Pro task: recover the PC v2.01 title-save ownership protocol

Work only from this package. Do not rely on prior chat history and do not claim
live acceptance without new game evidence.

## Product requirement

Keep title-screen generated-scroll insertion. It must remain safe if the player
does not load a character and exits the game immediately. On the next cold
start the append must still exist, all previous records must be intact and the
main, backup and system save files must form a game-accepted generation.

The external writer already supplies automatic three-file backup, source
fingerprints, atomic replacement, encrypted readback, structural verification,
durable operation receipts and conservative unknown outcomes. The unresolved
problem is that the running game may still own an older plaintext generation
and flush it after the application reports committed.

## Static reverse-engineering work

Use the supplied PC v2.01 `.text`, `.rdata` and `.pdata` section dumps to recover
the current equivalents of the v2.00.02 save-state dispatcher, save wrapper,
buffer/write coordinator, temporary-file writer and serializer. Start from the
old function shapes, file API call sites, `.tmp` path references and unwind
boundaries. Follow the control flow upward into the save manager and title
state.

Identify, with evidence:

- the native object that owns the selected character save at title;
- the queue, lock, dirty and generation fields used by exit-time saving;
- the native completion callback and its owning thread;
- the load, reload or cache-invalidation path for the selected account/slot;
- whether the canonical `0xE8` scroll record can be appended to the game-owned
  plaintext inventory before requesting a native save;
- every version-specific constant and signature needed to locate these paths.

Separate confirmed native control flow, strong static inference and unknowns.
Do not assign semantic names to fields solely because they change near a save.

## Required observer patch

Provide a new PC v2.01 `research/probe_title_save_ownership_ce.lua` or an exact
patch implementing it. It must:

- load `research/owned_breakpoint_lifecycle_ce.lua`;
- own and remove only its own breakpoints;
- validate executable identity and byte signatures before arming;
- record stable RVAs separately from process addresses;
- bound every event vector and string read;
- capture both successful and rejected/early-return save paths;
- expose status, clear-captures and stop/cleanup functions;
- verify cleanup and report any owned breakpoint that remains;
- make no target-memory or save-data writes.

The observer should collect the process/thread/event identity, file-write
chronology, serializer inputs, manager pointers, candidate state fields and
completion path required by the C0-C3 matrix in the supplied research plan.
If static evidence cannot justify a narrow internal breakpoint, begin with the
file API boundary and state exactly what additional dynamic capture will choose
the next breakpoint.

## Native protocol design

Return the smallest defensible product protocol. Prefer appending to the
game-owned plaintext inventory and invoking its native save. If that cannot be
proved safe, specify an external commit followed by native reload/cache
invalidation under shared save ownership. Every native call must run on the
game's actual owning thread; do not propose an arbitrary remote thread.

Define prepare, acceptance, completion, verification, cleanup and recovery.
Bind receipts to process creation identity, account, slot, operation ID and save
generation. A lost response after native acceptance must remain unknown and
must never replay the append.

## Required response

1. Findings ordered by confidence and impact, with RVA ranges and evidence.
2. A v2.01 call graph and data-flow description from title state to file write.
3. Stable signatures and relocation masks for every proposed runtime locator.
4. The bounded read-only CE observer patch and its output schema.
5. The proposed native protocol and exact unresolved risks.
6. The minimum C0-C3 live actions needed after the observer is integrated.
7. A release decision. It must remain BLOCK unless the live gates have already
   been satisfied by matching evidence.

## Forbidden substitutes

Do not solve the requirement with a sleep, process suspension, repeated
external overwrite, exit hook, forced process termination or a requirement that
the player loads the character once. Do not use the loaded-character live-add
hook as a title save API without independent control-flow proof.
"""


STALE_LEADS = """# PC v2.00.02 save-path leads; stale for v2.01

A controlled v2.00.02 save opened both character and system
`SAVEDATA.BIN.tmp`, then wrote complete temporary files. Every observed
`WriteFile` returned through game RVA `0x61E5EF`.

Recovered v2.00.02 ranges:

| Role | RVA range |
| --- | --- |
| save-state dispatcher | `0x61D044..0x61D0F7` |
| save wrapper | `0x61D600..0x61D613` |
| buffer/write coordinator | `0x61D6CC..0x61D942` |
| temporary-file writer | `0x61E514..0x61E71C` |
| serialization transform | `0x61E8AC..0x61EF48` |

At the old serialization-transform entry, `[RCX+0xE0]` was the plaintext
payload pointer, `[RCX+0xE8]` was its byte count and `RDX` was the final output
buffer. The transform built a `0x158`-byte header. The character plaintext was
`0x900058` bytes, and a known `0xE8` scroll record occurred at the expected
canonical record offset.

These values are shape anchors only. Do not use any old RVA in PC v2.01 until a
current byte signature, unwind boundary and live call event prove the mapping.
"""


def sha256_file(path: Path) -> str:
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


def copy_relative(relative: str, destination: Path) -> None:
    source = ROOT / relative
    if not source.is_file():
        raise FileNotFoundError(source)
    target = destination / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)


def sanitized_prep_snapshot() -> dict[str, object] | None:
    if not PREP_SNAPSHOT.is_file():
        return None
    data = json.loads(PREP_SNAPSHOT.read_text(encoding="utf-8"))
    data["save"].pop("save_path", None)
    for item in data.get("files", []):
        item.pop("path", None)
    for process in data.get("game_processes", []):
        process.pop("executable_path", None)
    return data


def write_manifest(root: Path) -> None:
    rows = []
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.name != "SHA256SUMS.txt":
            rows.append(f"{sha256_file(path)}  {path.relative_to(root).as_posix()}")
    (root / "SHA256SUMS.txt").write_text(
        "\n".join(rows) + "\n", encoding="utf-8", newline="\n"
    )


def main() -> None:
    DELIVERABLES.mkdir(parents=True, exist_ok=True)
    if OUTPUT_ROOT.exists() or ZIP_PATH.exists():
        raise FileExistsError(
            f"Refusing to replace an existing delivery: {OUTPUT_ROOT} or {ZIP_PATH}"
        )
    if OUTPUT_ROOT.parent.resolve() != DELIVERABLES.resolve():
        raise RuntimeError("Output path escaped deliverables")
    OUTPUT_ROOT.mkdir()

    (OUTPUT_ROOT / "README.md").write_text(README, encoding="utf-8", newline="\n")
    (OUTPUT_ROOT / "TASK_FOR_PRO.md").write_text(
        TASK, encoding="utf-8", newline="\n"
    )

    git_root = OUTPUT_ROOT / "git"
    git_root.mkdir()
    commit = git_text("rev-parse", "HEAD").strip()
    branch = git_text("branch", "--show-current").strip()
    status = git_text("status", "--short")
    (git_root / "COMMIT.txt").write_text(commit + "\n", encoding="utf-8")
    (git_root / "BRANCH.txt").write_text(branch + "\n", encoding="utf-8")
    (git_root / "STATUS_SHORT.txt").write_text(status, encoding="utf-8")
    (git_root / "WORKTREE.diff").write_text(
        git_text("diff", "--binary", "HEAD"), encoding="utf-8"
    )
    (git_root / "UNTRACKED_FILES.txt").write_text(
        git_text("ls-files", "--others", "--exclude-standard"), encoding="utf-8"
    )

    project = OUTPUT_ROOT / "project-source"
    for relative in PROJECT_FILES:
        copy_relative(relative, project)

    evidence = OUTPUT_ROOT / "evidence"
    for relative in EVIDENCE_FILES:
        copy_relative(relative, evidence / "prior-research")
    (evidence / "PC_V2_00_02_STALE_LEADS.md").write_text(
        STALE_LEADS, encoding="utf-8", newline="\n"
    )

    runtime = evidence / "runtime-sections"
    runtime.mkdir(parents=True)
    for source in sorted(SECTIONS.iterdir()):
        if source.is_file():
            shutil.copy2(source, runtime / source.name)

    prep = sanitized_prep_snapshot()
    if prep is not None:
        (evidence / "PREP_GAME_CLOSED_BASELINE.json").write_text(
            json.dumps(prep, indent=2) + "\n", encoding="utf-8", newline="\n"
        )

    section_manifest = json.loads((SECTIONS / "manifest.json").read_text(encoding="utf-8"))
    environment = {
        "schema": "nioh3-title-save-ownership-handoff/v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "platform": "Windows",
        "game_version": "PC v2.01",
        "application_target": "post-v0.7.2 blocked working tree",
        "git": {
            "commit": commit,
            "branch": branch,
            "dirty": bool(status.strip()),
            "status_short": status.splitlines(),
        },
        "executable": section_manifest["executable"],
        "runtime_sections": section_manifest["sections"],
        "live_capture_status": "not_started",
        "known_controls": ["C0", "C1", "C2", "C3"],
        "privacy": {
            "private_saves_included": False,
            "account_identifiers_included": False,
            "full_game_executable_included": False,
            "raw_sections_private_analysis_only": True,
        },
    }
    (OUTPUT_ROOT / "ENVIRONMENT.json").write_text(
        json.dumps(environment, indent=2) + "\n", encoding="utf-8", newline="\n"
    )

    write_manifest(OUTPUT_ROOT)
    with zipfile.ZipFile(
        ZIP_PATH, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6
    ) as archive:
        for path in sorted(OUTPUT_ROOT.rglob("*")):
            if path.is_file():
                archive.write(path, f"{DELIVERY_NAME}/{path.relative_to(OUTPUT_ROOT).as_posix()}")

    with zipfile.ZipFile(ZIP_PATH) as archive:
        bad = archive.testzip()
        if bad is not None:
            raise RuntimeError(f"ZIP CRC failed at {bad}")
        names = archive.namelist()
        required = {
            f"{DELIVERY_NAME}/README.md",
            f"{DELIVERY_NAME}/TASK_FOR_PRO.md",
            f"{DELIVERY_NAME}/ENVIRONMENT.json",
            f"{DELIVERY_NAME}/SHA256SUMS.txt",
        }
        if not required.issubset(names):
            raise RuntimeError("ZIP is missing one or more required root files")

    print(
        json.dumps(
            {
                "directory": str(OUTPUT_ROOT),
                "zip": str(ZIP_PATH),
                "zip_size": ZIP_PATH.stat().st_size,
                "zip_sha256": sha256_file(ZIP_PATH),
                "files": sum(1 for path in OUTPUT_ROOT.rglob("*") if path.is_file()),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
