"""Freeze the bounded r4 repair source identity for independent review."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
RELEVANT_PATHS = (
    "AGENTS.md",
    "crates/nioh3-save/Cargo.toml",
    "crates/nioh3-save/Cargo.lock",
    "crates/nioh3-save/src/error.rs",
    "crates/nioh3-save/src/transaction.rs",
    "crates/nioh3-protected/Cargo.toml",
    "crates/nioh3-protected/Cargo.lock",
    "crates/nioh3-protected/src/jobs.rs",
    "crates/nioh3-protected/src/main.rs",
    "crates/nioh3-protected/src/save_app.rs",
    "crates/nioh3-runtime/Cargo.toml",
    "crates/nioh3-runtime/Cargo.lock",
    "crates/nioh3-runtime/src/bin/runtime_mutation_helper.rs",
    "crates/nioh3-runtime/src/mutation/win_session.rs",
    "crates/nioh3-runtime/tests/native_helper_api.rs",
    "crates/nioh3-runtime/tests/observation_failure_helper.rs",
    "nioh3_scroll_editor/effect_batch_filter.py",
    "nioh3_scroll_editor/effect_generation_tables.py",
    "nioh3_scroll_editor/effect_path_inverse.py",
    "nioh3_scroll_editor/effect_preimage_search.py",
    "nioh3_scroll_editor/effect_seed_solver.py",
    "nioh3_scroll_editor/effect_sequence.py",
    "nioh3_scroll_editor/search_application.py",
    "nioh3_scroll_editor/search_jobs.py",
    "nioh3_scroll_editor/search_worker.py",
    "packages/contracts/generated.ts",
    "packages/contracts/protected-requests.ts",
    "packages/contracts/protected-response.schema.json",
    "packages/contracts/protected-responses.ts",
    "packages/contracts/response.schema.json",
    "packages/contracts/responses.ts",
    "tests/migration/cargo_target.py",
    "tests/migration/restore_fault_harness/Cargo.toml",
    "tests/migration/restore_fault_harness/Cargo.lock",
    "tests/migration/restore_fault_harness/src/main.rs",
    "tests/migration/save_restore_fixture.py",
    "tests/migration/test_build_root_policy.py",
    "tests/migration/test_packaged_host_resolver.py",
    "tests/migration/test_protected_save_acceptance.py",
    "tests/migration/test_save_transaction_parity.py",
    "tests/test_effect_path_inverse.py",
    "tests/test_python_r5_preimage_identity.py",
    "tests/test_python_r5_table_selection.py",
    "tools/export_r4_closure_evidence.py",
    "tools/export_r4_closure_identity.py",
    "tools/run_python_tests.ps1",
)


def run_git(*arguments: str, binary: bool = False) -> bytes | str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=not binary,
        encoding=None if binary else "utf-8",
    )
    return completed.stdout


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise SystemExit(f"refusing to overwrite identity evidence: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)

    missing = [relative for relative in RELEVANT_PATHS if not (ROOT / relative).is_file()]
    if missing:
        raise SystemExit(f"relevant source files are missing: {missing}")

    path_arguments = ("--", *RELEVANT_PATHS)
    diff = run_git("diff", "--binary", *path_arguments, binary=True)
    assert isinstance(diff, bytes)
    relevant_status = run_git("status", "--porcelain=v1", "-uall", *path_arguments)
    full_status = run_git("status", "--porcelain=v1", "-uall")
    assert isinstance(relevant_status, str)
    assert isinstance(full_status, str)

    report = {
        "schema": "nioh3-r4-closure-source-identity/v1",
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "repository": {
            "root": str(ROOT),
            "branch": str(run_git("branch", "--show-current")).strip(),
            "head": str(run_git("rev-parse", "HEAD")).strip(),
            "head_tree": str(run_git("rev-parse", "HEAD^{tree}")).strip(),
            "dirty": bool(full_status.strip()),
            "full_status_line_count_uall": len(full_status.splitlines()),
            "relevant_status_lines": relevant_status.splitlines(),
            "relevant_diff_sha256": hashlib.sha256(diff).hexdigest(),
            "relevant_diff_bytes": len(diff),
        },
        "files": [
            {
                "path": relative,
                "bytes": (ROOT / relative).stat().st_size,
                "sha256": sha256_file(ROOT / relative),
            }
            for relative in RELEVANT_PATHS
        ],
    }
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"ok": True, "files": len(RELEVANT_PATHS), "output": str(output)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
