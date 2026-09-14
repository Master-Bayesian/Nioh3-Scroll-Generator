"""Validate the semantic evidence contract of the possessed-enemy Pro handoff."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


EXE_SHA256 = "4047ECB623F3AE033F7D6F3637CD1C5408C72A89A038463268A7C9AC5C394159"
SECTION_HASHES = {
    "Nioh3_v2.0.1.0.text.bin": "F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023",
    "Nioh3_v2.0.1.0.rdata.bin": "DF245B4EF1478643845680DB183EA170F448B1EF8FF9710CBD040CB48F4E363D",
    "Nioh3_v2.0.1.0.pdata.bin": "928D0407FFCCFBD081559A2DF00D4995598B60B5C3F1D325D086CB531B09B904",
}
RUNS = {
    "156062997/20260912-sp-a": {
        "summary": "late-mask.summary.json",
        "field_8f": [("0xF40", "0x8BC34")],
    },
    "156062997/20260912-sp-b5": {
        "summary": "late-mask.summary.json",
        "field_8f": [("0xF40", "0x8BC34")],
    },
    "86872488/20260912-sp-negative-b": {
        "summary": "late-mask.summary.json",
        "field_8f": [],
    },
    "86872488/20260912-one-person-expedition-a": {
        "summary": "late-mask.summary.v2.json",
        "field_8f": [("0xF3F", "0xDCB98")],
        "field_e9_zero": ["0xF42", "0xF45"],
    },
    "86872488/20260913-one-person-expedition-b": {
        "summary": "late-mask.summary.v2.json",
        "field_8f": [("0xF3F", "0xDCB98")],
        "field_e9_zero": ["0xF3D", "0xF42"],
    },
}
FORBIDDEN_SUFFIXES = {".exe", ".dll", ".sav"}
FORBIDDEN_PARTS = {".codex_tmp", "project"}


class ValidationError(ValueError):
    """Raised when the handoff contradicts its evidence contract."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def load_json(path: Path) -> dict[str, object]:
    if not path.is_file():
        raise ValidationError(f"missing JSON evidence: {path}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValidationError(f"invalid JSON evidence: {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValidationError(f"JSON evidence is not an object: {path}")
    return value


def manifest_file_entries(manifest: dict[str, object]) -> list[tuple[str, str]]:
    entries: list[tuple[str, str]] = []
    files = manifest.get("files")
    if files is None:
        files = manifest.get("capture_files", {})
    if not isinstance(files, dict):
        raise ValidationError("run manifest files field is not an object")
    for value in files.values():
        if not isinstance(value, dict):
            continue
        filename = value.get("file")
        digest = value.get("sha256")
        if isinstance(filename, str) and isinstance(digest, str):
            entries.append((filename, digest.upper()))
    if not entries:
        raise ValidationError("run manifest has no file/hash entries")
    return entries


def validate_cleanup(run_root: Path) -> None:
    candidates = [
        run_root / "late-mask.cleanup-recheck.json",
        run_root / "late-mask.cleanup.json",
    ]
    cleanup_path = next((path for path in candidates if path.is_file()), None)
    if cleanup_path is None:
        raise ValidationError(f"missing final cleanup evidence: {run_root}")
    cleanup = load_json(cleanup_path)
    bridge = cleanup.get("bridge_result")
    if not isinstance(bridge, dict) or bridge.get("ok") is not True:
        raise ValidationError(f"cleanup bridge did not report success: {cleanup_path}")
    probe = bridge.get("probe")
    if not isinstance(probe, dict):
        raise ValidationError(f"cleanup probe state is missing: {cleanup_path}")
    if bridge.get("breakpoints") != []:
        raise ValidationError(f"global breakpoints remained: {cleanup_path}")
    if probe.get("owned_breakpoints") != []:
        raise ValidationError(f"owned breakpoints remained: {cleanup_path}")
    if probe.get("active") is not False or probe.get("cleanup_pending") is not False:
        raise ValidationError(f"probe cleanup was incomplete: {cleanup_path}")


def validate_run(root: Path, relative: str, contract: dict[str, object]) -> dict[str, object]:
    run_root = root / "evidence" / "controls" / relative
    manifest = load_json(run_root / "run_manifest.json")
    for filename, expected in manifest_file_entries(manifest):
        evidence_path = run_root / filename
        if not evidence_path.is_file():
            raise ValidationError(f"manifest evidence is missing: {evidence_path}")
        actual = sha256_file(evidence_path)
        if actual != expected:
            raise ValidationError(f"run-manifest hash mismatch: {evidence_path}")
    validate_cleanup(run_root)

    summary = load_json(run_root / str(contract["summary"]))
    actual_8f = sorted(
        (str(row["spawn_id"]), str(row["enemy_lookup_key"]))
        for row in summary.get("field_8f_nonzero_records", [])
    )
    expected_8f = sorted(contract["field_8f"])
    if actual_8f != expected_8f:
        raise ValidationError(f"unexpected +0x8F partition for {relative}: {actual_8f}")
    if summary.get("field_8f_nonzero_count") != len(expected_8f):
        raise ValidationError(f"unexpected +0x8F count for {relative}")
    if summary.get("field_ea_nonzero_count") != 0:
        raise ValidationError(f"unexpected +0xEA nonzero record for {relative}")
    if summary.get("selection_mask_hex") != "000000000000000000000000":
        raise ValidationError(f"unexpected late selection mask for {relative}")

    expected_e9 = contract.get("field_e9_zero")
    if expected_e9 is not None:
        if summary.get("field_e9_counts") != {"zero": 2, "one": 8, "other": 0}:
            raise ValidationError(f"unexpected +0xE9 counts for {relative}")
        actual_e9 = sorted(
            str(row["spawn_id"]) for row in summary.get("field_e9_zero_records", [])
        )
        if actual_e9 != sorted(expected_e9):
            raise ValidationError(f"unexpected +0xE9 zero partition for {relative}: {actual_e9}")
    return {
        "run": relative,
        "run_id": manifest.get("run_id"),
        "verified_manifest_files": len(manifest_file_entries(manifest)),
        "field_8f_nonzero": actual_8f,
    }


def validate_sections(root: Path) -> dict[str, object]:
    section_root = root / "evidence" / "runtime-sections"
    manifest = load_json(section_root / "manifest.json")
    executable = manifest.get("executable")
    if not isinstance(executable, dict) or executable.get("sha256") != EXE_SHA256:
        raise ValidationError("runtime-section manifest has the wrong executable identity")
    manifest_sections = {
        str(row["filename"]): str(row["sha256"]).upper()
        for row in manifest.get("sections", [])
        if isinstance(row, dict) and "filename" in row and "sha256" in row
    }
    for filename, expected in SECTION_HASHES.items():
        if manifest_sections.get(filename) != expected:
            raise ValidationError(f"section manifest identity mismatch: {filename}")
        path = section_root / filename
        if not path.is_file() or sha256_file(path) != expected:
            raise ValidationError(f"runtime section hash mismatch: {filename}")

    inventory = load_json(
        root / "evidence" / "native-static" / "v2.01-record-field-8f-accesses.json"
    )
    inputs = inventory.get("inputs")
    if not isinstance(inputs, dict):
        raise ValidationError("static field inventory has no input identities")
    if inputs.get("text_sha256") != SECTION_HASHES["Nioh3_v2.0.1.0.text.bin"]:
        raise ValidationError("static field inventory uses a different .text section")
    if inputs.get("pdata_sha256") != SECTION_HASHES["Nioh3_v2.0.1.0.pdata.bin"]:
        raise ValidationError("static field inventory uses a different .pdata section")
    return {"section_count": len(SECTION_HASHES), "section_hashes_verified": True}


def validate_forbidden_files(root: Path) -> None:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if any(part in FORBIDDEN_PARTS for part in relative.parts):
            raise ValidationError(f"forbidden package path: {relative.as_posix()}")
        if path.is_file() and path.suffix.lower() in FORBIDDEN_SUFFIXES:
            raise ValidationError(f"forbidden binary or save file: {relative.as_posix()}")


def validate_task(root: Path) -> None:
    task = (root / "TASK_FOR_PRO.md").read_text(encoding="utf-8")
    required = (
        "record+0x8F",
        "record+0xE9",
        "owner-observed",
        "one targeted live validation",
        "Do not build a product forward oracle or inverse solver",
    )
    missing = [phrase for phrase in required if phrase not in task]
    if missing:
        raise ValidationError(f"TASK_FOR_PRO.md is missing required boundaries: {missing}")
    forbidden = (
        "F3D is possessed ground truth",
        "complete the inverse solver",
        "repeat the selector experiment",
    )
    present = [phrase for phrase in forbidden if phrase in task]
    if present:
        raise ValidationError(f"TASK_FOR_PRO.md contains stale requirements: {present}")


def validate_handoff(root: Path) -> dict[str, object]:
    root = root.resolve()
    validate_forbidden_files(root)
    validate_task(root)
    runs = [validate_run(root, relative, contract) for relative, contract in RUNS.items()]
    sections = validate_sections(root)
    return {
        "schema": "nioh3-possessed-enemy-handoff-validation/v1",
        "ok": True,
        "package": str(root),
        "run_count": len(runs),
        "runs": runs,
        **sections,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    try:
        report = validate_handoff(args.package)
    except (OSError, ValidationError) as exc:
        parser.exit(1, f"possessed-enemy handoff validation failed: {exc}\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
