"""Export durable synthetic evidence for the r4 restore-journal closure.

The ordinary parity tests delete their temporary saves after a green run. This
collector drives the same public Rust harness but keeps the raw pending receipt,
restore journal, checkpoint manifest, role-byte SHA-256 identities, and restart
classification for independent review. It never discovers or opens a user save
and never attaches to the game.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration.save_restore_fixture import (  # noqa: E402
    assert_generations_distinct,
    generation_bytes,
    generation_digests,
    read_generation,
    write_backup_bundle,
    write_generation,
)
from tests.migration.test_save_transaction_parity import (  # noqa: E402
    run_restore_harness,
)
from tests.migration.test_save_read_parity import (  # noqa: E402
    build_fixture_bytes,
    native_transform_short,
)


ACCOUNT_ID = 76_561_198_000_000_000
JOURNAL_CUTS = (
    "journal-create",
    "journal-write",
    "journal-flush",
    "journal-replace",
)
SCROLL_VALUE_OFFSET = 0x176CCE + 0x20


def build_restore_generations(case_root: Path) -> tuple[dict[str, bytes], ...]:
    """Build three valid, distinct encrypted save generations for one case."""

    fixture_root = case_root / "fixture-generations"
    fixture_root.mkdir(parents=True, exist_ok=True)
    generations: list[dict[str, bytes]] = []
    for label, marker in (
        ("A", 0xA0A0A0A0),
        ("B", 0xB0B0B0B0),
        ("C", 0xC0C0C0C0),
    ):
        plain_bytes = bytearray(build_fixture_bytes())
        plain_bytes[SCROLL_VALUE_OFFSET : SCROLL_VALUE_OFFSET + 4] = marker.to_bytes(
            4, "little"
        )
        plain = fixture_root / f"generation-{label.lower()}-plain.bin"
        container = fixture_root / f"generation-{label.lower()}-container.bin"
        plain.write_bytes(plain_bytes)
        native_transform_short(plain, container)
        generations.append(
            generation_bytes(container.read_bytes(), f"generation-{label.lower()}")
        )
    assert_generations_distinct(*generations)
    return tuple(generations)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_key_values(text: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, value = line.partition("=")
        if separator:
            values[key] = value
    return values


def unique_path(paths: list[Path], label: str) -> Path:
    if len(paths) != 1:
        raise RuntimeError(f"expected one {label}, found {len(paths)}")
    return paths[0]


def run_case(output: Path, cargo_target: Path, cut: str, *, external_c: bool) -> dict:
    slug = "abc-mix" if external_c else cut
    case_root = output / ".work" / slug
    save_path = case_root / "target" / str(ACCOUNT_ID) / "SAVEDATA00" / "SAVEDATA.BIN"
    state_root = case_root / "state"
    source_a, checkpoint_b, external_generation_c = build_restore_generations(case_root)
    write_generation(save_path, checkpoint_b)
    backup_id = f"source-a-{slug}"
    write_backup_bundle(
        state_root,
        backup_id,
        source_a,
        account_id=ACCOUNT_ID,
        save_slot_index=0,
    )

    prepare_arguments = [
        "prepare",
        "--state-root",
        str(state_root),
        "--save-path",
        str(save_path),
        "--backup-id",
        backup_id,
    ]
    prepared = run_restore_harness(str(cargo_target), prepare_arguments)
    if prepared.returncode != 0:
        raise RuntimeError(f"{slug}: prepare failed: {prepared.stderr}")
    plan_id = prepared.stdout.strip().splitlines()[0]

    commit_cut = "after-replace:system_save" if external_c else cut
    commit_arguments = [
        "commit",
        "--state-root",
        str(state_root),
        "--save-path",
        str(save_path),
        "--plan-id",
        plan_id,
        "--crash-cut",
        commit_cut,
    ]
    committed = run_restore_harness(str(cargo_target), commit_arguments)
    if committed.returncode != 9:
        raise RuntimeError(
            f"{slug}: crash cut exited {committed.returncode}, expected 9: "
            f"{committed.stdout}{committed.stderr}"
        )

    if external_c:
        save_path.write_bytes(external_generation_c["main_save"])

    classify_arguments = [
        "classify",
        "--state-root",
        str(state_root),
        "--save-path",
        str(save_path),
        "--plan-id",
        plan_id,
    ]
    classified = run_restore_harness(str(cargo_target), classify_arguments)
    if classified.returncode != 0:
        raise RuntimeError(f"{slug}: classify failed: {classified.stderr}")

    receipt = state_root / "v2-operations" / f"{plan_id}.json"
    journal = unique_path(
        list((state_root / "backups").glob("*/restore-journal.json")),
        "restore journal",
    )
    checkpoint_candidates: list[Path] = []
    for manifest in (state_root / "backups").glob("*/backup-manifest.json"):
        content = json.loads(manifest.read_text(encoding="utf-8"))
        if content.get("action") == "pre-restore-checkpoint":
            checkpoint_candidates.append(manifest)
    checkpoint = unique_path(checkpoint_candidates, "pre-restore checkpoint manifest")

    artifact_root = output / "artifacts" / slug
    artifact_root.mkdir(parents=True, exist_ok=True)
    retained = {
        "receipt": artifact_root / "pending-receipt.json",
        "journal": artifact_root / "restore-journal.json",
        "checkpoint_manifest": artifact_root / "checkpoint-manifest.json",
    }
    shutil.copy2(receipt, retained["receipt"])
    shutil.copy2(journal, retained["journal"])
    shutil.copy2(checkpoint, retained["checkpoint_manifest"])

    current = read_generation(save_path)
    return {
        "case": slug,
        "cut": commit_cut,
        "external_c_injected_after_crash": external_c,
        "plan_id": plan_id,
        "commands": {
            "prepare": {
                "arguments": prepare_arguments,
                "exit_code": prepared.returncode,
                "stdout": prepared.stdout,
                "stderr": prepared.stderr,
            },
            "commit_crash_cut": {
                "arguments": commit_arguments,
                "exit_code": committed.returncode,
                "stdout": committed.stdout,
                "stderr": committed.stderr,
            },
            "classify_restart": {
                "arguments": classify_arguments,
                "exit_code": classified.returncode,
                "stdout": classified.stdout,
                "stderr": classified.stderr,
            },
        },
        "classification": parse_key_values(classified.stdout),
        "generation_sha256": {
            "A_source": generation_digests(source_a),
            "B_checkpoint": generation_digests(checkpoint_b),
            "C_external": generation_digests(external_generation_c),
            "current": generation_digests(current),
        },
        "raw_artifacts": {
            "receipt": retained["receipt"].relative_to(output).as_posix(),
            "receipt_sha256": sha256_file(retained["receipt"]),
            "journal": retained["journal"].relative_to(output).as_posix(),
            "journal_sha256": sha256_file(retained["journal"]),
            "checkpoint_manifest": retained["checkpoint_manifest"]
            .relative_to(output)
            .as_posix(),
            "checkpoint_manifest_sha256": sha256_file(
                retained["checkpoint_manifest"]
            ),
            "checkpoint_id": checkpoint.parent.name,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo-target", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        if not output.is_dir() or any(output.iterdir()):
            raise SystemExit(f"refusing to overwrite non-empty evidence directory: {output}")
    output.mkdir(parents=True, exist_ok=True)
    cargo_target = args.cargo_target.resolve()
    cargo_target.mkdir(parents=True, exist_ok=True)

    head = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout.strip()

    cases = [
        run_case(output, cargo_target, cut, external_c=False)
        for cut in JOURNAL_CUTS
    ]
    cases.append(
        run_case(output, cargo_target, "after-replace:system_save", external_c=True)
    )
    shutil.rmtree(output / ".work")
    report = {
        "schema": "nioh3-r4-restore-closure-evidence/v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "repository_head": head,
        "evidence_grade": "synthetic public-Rust-harness crash-cut and restart classification",
        "target": {
            "game_process_opened": False,
            "target_memory_read": False,
            "target_memory_written": False,
            "user_save_opened": False,
            "synthetic_files_written": True,
        },
        "cargo_target": str(cargo_target),
        "cases": cases,
    }
    (output / "summary.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps({"ok": True, "cases": len(cases), "output": str(output)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
