"""Build the immutable PC v2.01 possessed-enemy assignment Pro handoff."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import uuid
import zipfile
from datetime import datetime, timezone
from pathlib import Path

try:
    from .validate_possessed_enemy_handoff import validate_handoff
    from .validate_research_handoff import validate_directory, validate_zip
except ImportError:  # Direct script execution keeps tools/ on sys.path.
    from validate_possessed_enemy_handoff import validate_handoff
    from validate_research_handoff import validate_directory, validate_zip


ROOT = Path(__file__).resolve().parents[1]
SPEC_PATH = (
    ROOT
    / "research"
    / "possessed_enemy_capture"
    / "handoff_specs"
    / "pc_v2.01_20260913_v1.json"
)
DELIVERABLES = ROOT / "deliverables"


README = """# PC v2.01 possessed-enemy assignment research handoff

This is a private, self-contained evidence package for recovering the native
writer and upstream selection path for possessed-enemy assignment in Nioh 3 PC
v2.01. It contains five controlled runtime captures, bounded static triage,
the matching loaded module sections, and the exact collectors used to produce
the evidence.

Read in this order:

1. `TASK_FOR_PRO.md`
2. `evidence/EVIDENCE_REGISTER.md`
3. `evidence/CONTROL_MATRIX.json`
4. the five `evidence/controls/*/*/run_manifest.json` files
5. `evidence/native-static/STATIC_TRIAGE.md`
6. `evidence/native-static/v2.01-record-field-8f-accesses.json`
7. `evidence/runtime-sections/manifest.json`
8. the relevant collectors under `project-source/`

The owner observations are accurate textual observations recorded during the
runs. There is no retained video or screenshot evidence, and none is required
to interpret the native control matrix. The run manifests identify which
statements are direct native facts, owner-observed visual labels, or mappings
inferred from unique wave/count partitions.

Raw captures and final cleanup records are authoritative for native bytes and
observer safety. Summaries and comparisons are reproducible derived views.
Historical absolute addresses, PIDs, CE session IDs, and local game paths are
process evidence, not stable signatures.

The `.text`, `.rdata`, and `.pdata` files are loaded-section dumps from the
matching executable. They contain copyrighted game code. This entire package
is for private analysis only and must not be published or redistributed.
"""


TASK = """# Pro task: recover the PC v2.01 possessed-enemy assignment path

Work only from this package. Prior chat history is not part of the evidence.
The immediate task is native causal recovery, not another broad live survey.

## Exact research question

Recover the earliest causal source and native writer that determines mission
task `record+0x8F`, which repeatedly correlates with the visually possessed
enemy. Prove the base object's identity, follow its caller/data-flow chain to
the actual assignment inputs, and recover any PRNG or selection algorithm used.
Separately explain the writer and selection path for `record+0xE9`, which in
the two expedition captures follows the owner-observed One Difficulty
partition. Determine whether `+0xE9` is a prerequisite, an independent state,
or merely correlated with `+0x8F`.

## Grounded controls

- Seed `156062997`, two fresh normal single-player entries: the owner observed
  one first-wave possessed Koroka. In both captures only mission record
  `spawn 0xF40 / lookup 0x8BC34` has `+0x8F=1`.
- Seed `86872488`, corrected fresh normal single-player negative: the owner
  observed no possessed enemy and all six captured mission records have
  `+0x8F=0`.
- Seed `86872488`, two fresh one-person expedition entries: the owner observed
  one possessed member of the two second-wave Nuppeppo. In both captures only
  `spawn 0xF3F / lookup 0xDCB98` has `+0x8F=1`.
- Expedition A has `+0xE9=0` for `F42/F45`; expedition B has `+0xE9=0` for
  `F3D/F42`. Those moving partitions exactly follow the owner-observed ordinary
  rather than One Difficulty counts. Both second-wave Nuppeppo have `+0xE9=1`,
  while only the possessed partition has `+0x8F=1`.
- Every selected capture has `+0xEA=0` and the late 12-byte mask all zero.

The visual-to-record assignment for duplicate enemies is inferred from the
unique wave/count partitions; it is not a direct actor-pointer join. The
external invincibility trainer was a shared condition of the captures and is a
potential confounder, although it does not explain the positive/negative split.

## Required static recovery

1. Classify every exact-displacement `+0x8F` write candidate as initialization,
   clear, causal assignment, copy, consumer, or unrelated object. A matching
   displacement alone is not object proof.
2. Identify the earliest causal writer with its exact RVA, function boundary,
   key instructions, caller chain, and field evidence proving that its base is
   the captured mission task record or a source copied into it.
3. Trace every branch and early return from mission/session inputs to the
   assignment. Identify whether normal solo versus one-person expedition is an
   explicit input or changes an upstream candidate/state object.
4. Recover the exact PRNG path if present: state origin, seed material, draw
   order, range reduction/rejection, candidate pool mutation, and final choice.
   Supply executable pseudocode and show how each packaged control constrains it.
5. Trace `record+0xE9` independently from the known selector consumer around
   `0xE3ADF0`, and establish its temporal and causal relationship to `+0x8F`.
6. Provide version-gated signatures/relocation masks for all proposed RVAs.

The static inventory has 1,257 raw `0x8F` displacement occurrences, 209
validated accesses, and 34 writes. The large candidate at
`0x2A3BD0..0x2A4C4E` is strongly disfavored because its `+0x20` member is
dereferenced as a pointer while the captured task record stores packed mission
identity there. The leaf setters at `0x2228FA4` and `0x2934EE8` have no validated
direct E8/E9 callers; this does not exclude indirect/vtable callers. Treat the
rdata pointer-candidate report as an untyped heuristic with false positives,
not a validated xref list.

## Required response

Return:

1. findings separated into confirmed, owner-observed, inferred, and unknown;
2. a writer-candidate disposition table with package-relative citations;
3. the proven call/data-flow graph and exact data layout;
4. PRNG/selection pseudocode and worked explanations for all five controls;
5. stable signatures and a fail-closed PC v2.01 version gate;
6. a minimal read-only collector patch only if dynamic proof is still needed;
7. explicit falsification criteria and remaining unknowns.

If one live check is indispensable, request at most one targeted live validation
against a specific RVA/object provenance. Give no more than four signature-
gated, owner-scoped breakpoints, exact registers/read ranges/event schema, a
bounded stop condition, and outcomes that can falsify the hypothesis. Do not
request another generic selector repeat or open-ended gameplay matrix.

Do not build a product forward oracle or inverse solver until the causal path is
proved and the resulting algorithm passes an independent matching-version live
validation. Do not write game memory, call game functions, use historical
absolute addresses as signatures, or treat the absence of video as missing
ground truth.
"""


EVIDENCE_REGISTER = """# Evidence register

## Authority order

1. `late-mask.json`, matching run manifest hashes, and final cleanup evidence:
   authoritative native capture and observer-safety evidence.
2. Owner-observed labels in each run manifest: accurate textual visual
   observations made during the corresponding run. No recording survives.
3. `late-mask.summary.v2.json`, comparisons, and control matrix: reproducible
   derived views and explicit visual-to-record inferences.
4. Static inventories and disassembly: exact bytes/RVAs, but semantic object
   classifications remain hypotheses until provenance is proved.

## Selected controls

- `controls/156062997/20260912-sp-a`: first valid normal-solo positive.
- `controls/156062997/20260912-sp-b5`: independent positive repeat.
- `controls/86872488/20260912-sp-negative-b`: corrected normal-solo negative.
- `controls/86872488/20260912-one-person-expedition-a`: expedition positive A.
- `controls/86872488/20260913-one-person-expedition-b`: new-process expedition
  positive B with a changed One Difficulty partition.

For expedition A, `late-mask.summary.json` is an older candidate-only view.
Use `late-mask.summary.v2.json` or the raw capture for complete task records.

## Important limitations

- `record+0x8F` is a repeated state-marker correlation, not yet a proven cause.
- `record+0xE9` follows the two owner-observed One Difficulty partitions, but
  its exact universal semantic and causal relationship to `+0x8F` are open.
- Duplicate-enemy identity uses a unique count/wave inference, not actor pointer
  identity.
- All selected runs used the same invincibility trainer condition.
- Run A's exact observer source revision is not currently recovered; the raw
  output and signatures remain usable, and this provenance gap is disclosed.
- The rdata pointer-candidate file is a heuristic scan and contains false
  positives. It must not be promoted to typed vtable/RTTI evidence.
"""


STATIC_TRIAGE = """# Static `record+0x8F` triage

The field-access inventory is a bounded exact-displacement census, not an
object-aware xref graph. It found 1,257 raw literal occurrences, 1,009 bounded
candidate functions, 209 validated accesses, 34 writes, and 178 reads.

Current dispositions:

- `0x2A3BD0..0x2A4C4E`: strongly disfavored. Its object has pointer/container
  behavior at `+0x20` and a family of latch fields around `+0x8F`; this is
  incompatible with the captured mission record layout. Its sole validated
  direct caller is in `0xAF910..0xB05DC` and passes an enclosing `+0x590` object.
- `0x703468`, `0x7051D0`, `0x707278`, `0x21A5398`: generic copy/move functions
  for a string/container-bearing object, not the captured task-record layout.
- `0xE47F50..0xE481DE`: initializes a float-heavy global/config object and writes
  zero at `+0x8F`; layout is incompatible.
- `0x345DBC..0x347152`: unpacks a signed six-bit packed field into `+0x8F`, not
  a simple possession boolean. Mission linkage is unknown.
- `0x2228FA4` and `0x2934EE8`: eight-byte leaf setters containing
  `[RCX+0x8F]=1; ret`. They have no validated direct E8/E9 caller, but indirect
  callers and object provenance remain unknown.
- Additional byte-copy/remap candidates around `0xFF2347` and `0x30AF2AA` have
  no proven mission-record base.

Use the matching `.text/.pdata/.rdata` dumps to recover indirect references,
constructor/copy provenance, RTTI/vtable evidence, and the real upstream path.
All large-section analysis must use raw byte prefilters plus bounded function
decoding; the included incident report explains why whole-section Capstone
decoding is prohibited.
"""


CONTROL_MATRIX = {
    "schema": "nioh3-possessed-enemy-control-matrix/v1",
    "evidence_boundary": {
        "visual_basis": "owner-observed textual descriptions during each run",
        "recording_available": False,
        "native_authority": "raw late-mask capture plus run-manifest hashes",
        "record_mapping": "wave/count partition inference where duplicate enemies exist",
    },
    "runs": [
        {
            "path": "controls/156062997/20260912-sp-a",
            "seed": 156062997,
            "mode": "normal single-player",
            "visual": "first-wave possessed Koroka",
            "field_8f_nonzero": ["F40/8BC34"],
        },
        {
            "path": "controls/156062997/20260912-sp-b5",
            "seed": 156062997,
            "mode": "normal single-player",
            "visual": "first-wave possessed Koroka repeat",
            "field_8f_nonzero": ["F40/8BC34"],
        },
        {
            "path": "controls/86872488/20260912-sp-negative-b",
            "seed": 86872488,
            "mode": "normal single-player",
            "visual": "no possessed enemy",
            "field_8f_nonzero": [],
        },
        {
            "path": "controls/86872488/20260912-one-person-expedition-a",
            "seed": 86872488,
            "mode": "one-person expedition",
            "visual": "one possessed member of the two wave-2 Nuppeppo",
            "field_8f_nonzero": ["F3F/DCB98"],
            "field_e9_zero_spawn_ids": ["F42", "F45"],
        },
        {
            "path": "controls/86872488/20260913-one-person-expedition-b",
            "seed": 86872488,
            "mode": "one-person expedition",
            "visual": "one possessed member of the two wave-2 Nuppeppo; wave-level One Difficulty counts recorded",
            "field_8f_nonzero": ["F3F/DCB98"],
            "field_e9_zero_spawn_ids": ["F3D", "F42"],
        },
    ],
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def load_spec() -> dict[str, object]:
    value = json.loads(SPEC_PATH.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or value.get("schema") != "nioh3-possessed-enemy-handoff-spec/v1":
        raise ValueError("invalid possessed-enemy handoff spec")
    return value


def git_text(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=True,
        text=True,
        encoding="utf-8",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout


def copy_file(relative: str, destination_root: Path) -> None:
    source = ROOT / relative
    if not source.is_file():
        raise FileNotFoundError(source)
    destination = destination_root / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def ignore_tree(_directory: str, names: list[str]) -> set[str]:
    ignored = {"__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache"}
    return set(names).intersection(ignored)


def write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8", newline="\n")


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def copy_inputs(staging: Path, spec: dict[str, object]) -> list[dict[str, object]]:
    controls_root = staging / "evidence" / "controls"
    prefix = Path("audit/possessed_enemy_capture")
    for relative_value in spec["control_runs"]:
        relative = Path(str(relative_value))
        source = ROOT / relative
        destination = controls_root / relative.relative_to(prefix)
        shutil.copytree(source, destination, ignore=ignore_tree)

    static_root = staging / "evidence" / "native-static"
    for relative_value in spec["static_evidence"]:
        source = ROOT / str(relative_value)
        destination = static_root / source.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    project_root = staging / "project-source"
    for relative_value in spec["project_files"]:
        copy_file(str(relative_value), project_root)
    for relative_value in spec["project_trees"]:
        relative = Path(str(relative_value))
        shutil.copytree(
            ROOT / relative,
            project_root / relative,
            dirs_exist_ok=True,
            ignore=ignore_tree,
        )

    section_root = ROOT / str(spec["runtime_section_root"])
    section_destination = staging / "evidence" / "runtime-sections"
    section_destination.mkdir(parents=True)
    shutil.copy2(section_root / "manifest.json", section_destination / "manifest.json")
    copied_sections: list[dict[str, object]] = []
    for filename, expected in spec["runtime_section_hashes"].items():
        source = section_root / filename
        actual = sha256_file(source)
        if actual != expected:
            raise ValueError(f"runtime section hash mismatch: {filename}")
        shutil.copy2(source, section_destination / filename)
        copied_sections.append(
            {"filename": filename, "bytes": source.stat().st_size, "sha256": actual}
        )
    return copied_sections


def write_checksums(root: Path, metadata: dict[str, object]) -> None:
    entries = []
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.name not in {"MANIFEST.json", "SHA256SUMS.txt"}:
            entries.append(
                {
                    "path": path.relative_to(root).as_posix(),
                    "bytes": path.stat().st_size,
                    "sha256": sha256_file(path),
                }
            )
    write_json(
        root / "MANIFEST.json",
        {
            "schema": "nioh3-possessed-enemy-assignment-pro-handoff/v1",
            "created_utc": datetime.now(timezone.utc).isoformat(),
            **metadata,
            "file_count_excluding_manifest_and_checksums": len(entries),
            "total_bytes_excluding_manifest_and_checksums": sum(row["bytes"] for row in entries),
            "files": entries,
        },
    )
    entries.append(
        {
            "path": "MANIFEST.json",
            "bytes": (root / "MANIFEST.json").stat().st_size,
            "sha256": sha256_file(root / "MANIFEST.json"),
        }
    )
    write_text(
        root / "SHA256SUMS.txt",
        "".join(f"{row['sha256'].lower()}  {row['path']}\n" for row in entries),
    )


def write_zip(root: Path, archive_path: Path) -> None:
    temporary = archive_path.with_suffix(f".{uuid.uuid4().hex}.tmp.zip")
    try:
        with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(root.rglob("*")):
                if path.is_file():
                    archive.write(path, Path(root.name) / path.relative_to(root))
        temporary.replace(archive_path)
    finally:
        if temporary.exists():
            temporary.unlink()


def build(_python_executable: str) -> dict[str, object]:
    spec = load_spec()
    delivery_name = str(spec["delivery_name"])
    output_root = DELIVERABLES / delivery_name
    zip_path = DELIVERABLES / f"{delivery_name}.zip"
    DELIVERABLES.mkdir(parents=True, exist_ok=True)
    if output_root.exists() or zip_path.exists():
        raise FileExistsError(f"refusing to replace immutable delivery: {output_root} or {zip_path}")
    if output_root.parent.resolve() != DELIVERABLES.resolve():
        raise RuntimeError("delivery escaped deliverables")

    staging = DELIVERABLES / f".{delivery_name}.{uuid.uuid4().hex}.staging"
    staging.mkdir()
    try:
        sections = copy_inputs(staging, spec)
        write_text(staging / "README.md", README)
        write_text(staging / "TASK_FOR_PRO.md", TASK)
        write_text(staging / "evidence" / "EVIDENCE_REGISTER.md", EVIDENCE_REGISTER)
        write_json(staging / "evidence" / "CONTROL_MATRIX.json", CONTROL_MATRIX)
        write_text(staging / "evidence" / "native-static" / "STATIC_TRIAGE.md", STATIC_TRIAGE)

        git_context = {
            "commit": git_text("rev-parse", "HEAD").strip(),
            "branch": git_text("branch", "--show-current").strip(),
            "dirty": bool(git_text("status", "--short").strip()),
        }
        environment = {
            "schema": "nioh3-possessed-enemy-assignment-environment/v1",
            "game_version": spec["game_version"],
            "executable_sha256": spec["executable_sha256"],
            "private_analysis_only": True,
            "recording_available": False,
            "owner_observation_format": "text recorded in run manifests",
            "git": git_context,
            "runtime_sections": sections,
        }
        write_json(staging / "ENVIRONMENT.json", environment)

        validation = validate_handoff(staging)
        packaged_validation = {**validation, "package": "."}
        write_json(
            staging / "evidence" / "verification" / "EVIDENCE_VALIDATION.json",
            packaged_validation,
        )
        write_checksums(staging, {"git": git_context, "runtime_sections": sections})
        validate_directory(staging)

        staging.replace(output_root)
        write_zip(output_root, zip_path)
        validate_zip(output_root, zip_path)
        return {
            "directory": str(output_root),
            "zip": str(zip_path),
            "zip_bytes": zip_path.stat().st_size,
            "zip_sha256": sha256_file(zip_path),
            "file_count": sum(path.is_file() for path in output_root.rglob("*")),
            "semantic_validation": validation,
        }
    finally:
        if staging.exists():
            if staging.parent.resolve() != DELIVERABLES.resolve() or not staging.name.startswith(
                f".{delivery_name}."
            ):
                raise RuntimeError(f"refusing to remove unexpected staging path: {staging}")
            shutil.rmtree(staging)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", default=sys.executable)
    args = parser.parse_args()
    try:
        report = build(args.python)
    except (OSError, ValueError, RuntimeError, zipfile.BadZipFile) as exc:
        parser.exit(1, f"possessed-enemy handoff build failed: {exc}\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
