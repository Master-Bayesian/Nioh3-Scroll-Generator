from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import struct
import subprocess
import xml.etree.ElementTree as ET
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path


EXPERIMENT_STAGES = (
    "source_before_obtain",
    "source_after_obtain",
    "source_after_reroll",
    "recipient_before_receive",
    "recipient_after_receive",
)

EXPERIMENT_SCHEMA = "nioh3-scroll-propagation/v1"
SCROLL_RECORD_SIZE = 0xE8
SCROLL_EFFECT_COUNT = 7
SCROLL_EFFECT_START = 0x34
SCROLL_EFFECT_STRIDE = 0x18
USER_CHECKSUM_BODY_START = 0x190
USER_CHECKSUM_BODY_END = 0x900190
USER_CHECKSUM_SEED_OFFSET = 0x900190
USER_CHECKSUM_VALUE_OFFSET = 0x900194
BODY_CRYPTO_MATERIAL_START = 0x49
BODY_CRYPTO_MATERIAL_END = 0x89


@dataclass(frozen=True)
class ByteRange:
    start: int
    end: int

    @property
    def length(self) -> int:
        return self.end - self.start


@dataclass(frozen=True)
class SpecialEffect:
    effect_id: int
    name: str
    raw_key: str


def load_special_effect_catalog(path: Path) -> dict[int, list[SpecialEffect]]:
    """Extract the user-supplied Cheat Engine effect dictionary as data only."""
    root = ET.parse(path).getroot()
    dropdown_text: str | None = None
    for entry in root.iter("CheatEntry"):
        description = entry.findtext("Description", default="").strip('"')
        if description == "SPECIAL_EFFECT_DROPDOWN":
            dropdown_text = entry.findtext("DropDownList")
            break
    if dropdown_text is None:
        raise ValueError("SPECIAL_EFFECT_DROPDOWN was not found in the cheat table")

    catalog: dict[int, list[SpecialEffect]] = {}
    for line in dropdown_text.splitlines():
        key, separator, name = line.partition(":")
        if not separator:
            continue
        byte_tokens = key.strip().split()
        if len(byte_tokens) != 4:
            continue
        try:
            raw = bytes(int(token, 16) for token in byte_tokens)
        except ValueError:
            continue
        effect_id = int.from_bytes(raw, "little")
        catalog.setdefault(effect_id, []).append(
            SpecialEffect(effect_id=effect_id, name=name.strip(), raw_key=key.strip())
        )
    return catalog


def parse_scroll_record(
    record: bytes, catalog: dict[int, list[SpecialEffect]] | None = None
) -> dict:
    if len(record) != SCROLL_RECORD_SIZE:
        raise ValueError(
            f"A scroll record must be exactly {SCROLL_RECORD_SIZE:#x} bytes, got {len(record):#x}"
        )

    effects = []
    for index in range(SCROLL_EFFECT_COUNT):
        start = SCROLL_EFFECT_START + index * SCROLL_EFFECT_STRIDE
        prefix, effect_id, value, metadata, tail_0, tail_1 = struct.unpack_from(
            "<6I", record, start
        )
        names = [] if catalog is None else [item.name for item in catalog.get(effect_id, [])]
        effects.append(
            {
                "slot": index + 1,
                "offset": start,
                "offset_hex": hex(start),
                "prefix": prefix,
                "prefix_hex": hex(prefix),
                "effect_id": effect_id,
                "effect_id_hex": hex(effect_id),
                "catalog_names": names,
                "value": value,
                "value_hex": hex(value),
                "metadata": metadata,
                "metadata_hex": hex(metadata),
                "tail": [tail_0, tail_1],
            }
        )

    return {
        "record_size": len(record),
        "record_type": struct.unpack_from("<I", record, 0)[0],
        "level_primary": struct.unpack_from("<H", record, 6)[0],
        "level_secondary": struct.unpack_from("<H", record, 8)[0],
        "scroll_id": struct.unpack_from("<I", record, 0x20)[0],
        "transfer_count": struct.unpack_from("<I", record, 0xDC)[0],
        "effects": effects,
    }


def compute_user_checksum(body: bytes, seed: int) -> int:
    if len(body) != 0x900000:
        raise ValueError(f"Checksum body must be 0x900000 bytes, got {len(body):#x}")
    total = 0
    for base in range(0, len(body), 0x400):
        block_sum = 0
        for offset in range(base, base + 0x400, 8):
            block_sum += struct.unpack_from("<q", body, offset)[0]
        total = ((total + block_sum) ^ seed) & 0xFFFFFFFFFFFFFFFF
    return ((total // 0xFFFFFFFF) + (total & 0xFFFFFFFF)) & 0xFFFFFFFF


def patch_user_checksum(data: bytearray) -> tuple[int, int]:
    if len(data) < USER_CHECKSUM_VALUE_OFFSET + 4:
        raise ValueError("The save is too small to contain the Nioh 3 user checksum")
    seed = struct.unpack_from("<I", data, USER_CHECKSUM_SEED_OFFSET)[0]
    old_checksum = struct.unpack_from("<I", data, USER_CHECKSUM_VALUE_OFFSET)[0]
    new_checksum = compute_user_checksum(
        bytes(data[USER_CHECKSUM_BODY_START:USER_CHECKSUM_BODY_END]), seed
    )
    struct.pack_into("<I", data, USER_CHECKSUM_VALUE_OFFSET, new_checksum)
    return old_checksum, new_checksum


def prepare_decrypted_save_for_encryption(save_data: bytes) -> bytes:
    if not save_data.startswith(b"RNNUSR"):
        raise ValueError("Encryption preparation requires a decrypted RNNUSR user save")
    prepared = bytearray(save_data)
    prepared[BODY_CRYPTO_MATERIAL_START:BODY_CRYPTO_MATERIAL_END] = bytes(
        BODY_CRYPTO_MATERIAL_END - BODY_CRYPTO_MATERIAL_START
    )
    return bytes(prepared)


def transplant_effect_slot(
    save_data: bytes,
    destination_record_offset: int,
    donor_record_offset: int,
    destination_slot: int,
    donor_slot: int,
) -> tuple[bytes, dict]:
    if not save_data.startswith(b"RNNUSR"):
        raise ValueError("Effect transplantation requires a decrypted RNNUSR user save")
    if destination_slot not in range(1, SCROLL_EFFECT_COUNT + 1):
        raise ValueError("Destination slot must be in the range 1..7")
    if donor_slot not in range(1, SCROLL_EFFECT_COUNT + 1):
        raise ValueError("Donor slot must be in the range 1..7")
    for label, offset in (
        ("destination", destination_record_offset),
        ("donor", donor_record_offset),
    ):
        if not (0 <= offset and offset + SCROLL_RECORD_SIZE <= len(save_data)):
            raise ValueError(f"The {label} record range is outside the save")

    destination_start = (
        destination_record_offset
        + SCROLL_EFFECT_START
        + (destination_slot - 1) * SCROLL_EFFECT_STRIDE
    )
    donor_start = (
        donor_record_offset + SCROLL_EFFECT_START + (donor_slot - 1) * SCROLL_EFFECT_STRIDE
    )
    edited = bytearray(save_data)
    before = bytes(edited[destination_start : destination_start + SCROLL_EFFECT_STRIDE])
    donor = bytes(edited[donor_start : donor_start + SCROLL_EFFECT_STRIDE])
    edited[destination_start : destination_start + SCROLL_EFFECT_STRIDE] = donor
    old_checksum, new_checksum = patch_user_checksum(edited)
    return bytes(edited), {
        "destination_record_offset": destination_record_offset,
        "destination_slot": destination_slot,
        "donor_record_offset": donor_record_offset,
        "donor_slot": donor_slot,
        "changed_effect_bytes": sum(left != right for left, right in zip(before, donor)),
        "old_effect_hex": before.hex(),
        "new_effect_hex": donor.hex(),
        "old_checksum": old_checksum,
        "new_checksum": new_checksum,
    }


def parse_int(value: str) -> int:
    return int(value, 0)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_kind(path: Path) -> str:
    with path.open("rb") as stream:
        header = stream.read(6)
    if header == b"RNNUSR":
        return "decrypted_nioh3_user"
    if header[:4] == b"NIOH":
        return "decrypted_legacy_nioh"
    return "opaque_or_encrypted"


def nioh3_is_running() -> bool:
    result = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq Nioh3.exe", "/FO", "CSV", "/NH"],
        check=True,
        capture_output=True,
        text=True,
    )
    return "Nioh3.exe" in result.stdout


def snapshot(source: Path, output_dir: Path, label: str) -> Path:
    if nioh3_is_running():
        raise RuntimeError("Nioh3.exe is running. Exit the game before taking a snapshot.")
    if not source.is_file():
        raise FileNotFoundError(source)

    destination = output_dir / label
    destination.mkdir(parents=True, exist_ok=False)
    copied = destination / source.name
    shutil.copy2(source, copied)
    manifest = {
        "label": label,
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "source": str(source.resolve()),
        "file": copied.name,
        "size": copied.stat().st_size,
        "sha256": sha256(copied),
        "file_kind": file_kind(copied),
    }
    (destination / "manifest.json").write_text(
        json.dumps(manifest, indent=2), encoding="utf-8"
    )
    return destination


def changed_ranges(before: bytes, after: bytes) -> list[ByteRange]:
    if len(before) != len(after):
        raise ValueError(f"File sizes differ: {len(before)} != {len(after)}")

    ranges: list[ByteRange] = []
    start: int | None = None
    for offset, (left, right) in enumerate(zip(before, after)):
        if left != right and start is None:
            start = offset
        elif left == right and start is not None:
            ranges.append(ByteRange(start, offset))
            start = None
    if start is not None:
        ranges.append(ByteRange(start, len(before)))
    return ranges


def merge_ranges(ranges: list[ByteRange], max_gap: int) -> list[ByteRange]:
    if not ranges:
        return []
    merged = [ranges[0]]
    for current in ranges[1:]:
        previous = merged[-1]
        if current.start - previous.end <= max_gap:
            merged[-1] = ByteRange(previous.start, current.end)
        else:
            merged.append(current)
    return merged


def diff_report(before_path: Path, after_path: Path, max_gap: int) -> dict:
    before = before_path.read_bytes()
    after = after_path.read_bytes()
    exact = changed_ranges(before, after)
    merged = merge_ranges(exact, max_gap)
    return {
        "before": str(before_path.resolve()),
        "after": str(after_path.resolve()),
        "before_kind": file_kind(before_path),
        "after_kind": file_kind(after_path),
        "size": len(before),
        "changed_bytes": sum(item.length for item in exact),
        "exact_range_count": len(exact),
        "max_gap": max_gap,
        "ranges": [
            {**asdict(item), "length": item.length, "start_hex": hex(item.start), "end_hex": hex(item.end)}
            for item in merged
        ],
    }


def create_experiment(output_dir: Path, game_version: str) -> Path:
    if output_dir.exists():
        raise FileExistsError(output_dir)
    output_dir.mkdir(parents=True)
    manifest = {
        "schema": EXPERIMENT_SCHEMA,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "game_version": game_version,
        "stages": {},
    }
    manifest_path = output_dir / "experiment.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest_path


def load_experiment(experiment_dir: Path) -> dict:
    manifest_path = experiment_dir / "experiment.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != EXPERIMENT_SCHEMA:
        raise ValueError(f"Unsupported experiment schema in {manifest_path}")
    return manifest


def capture_experiment_stage(
    source: Path,
    experiment_dir: Path,
    stage: str,
    account_alias: str,
    note: str,
) -> Path:
    if stage not in EXPERIMENT_STAGES:
        raise ValueError(f"Unknown stage: {stage}")
    manifest = load_experiment(experiment_dir)
    if stage in manifest["stages"]:
        raise FileExistsError(f"Stage already captured: {stage}")
    if not source.is_file():
        raise FileNotFoundError(source)
    if file_kind(source) != "decrypted_nioh3_user":
        raise ValueError(
            "Propagation analysis requires a decrypted Nioh 3 user save with an RNNUSR header. "
            "The opaque original save is useful as a backup, but its encrypted byte diff is not."
        )

    captured_dir = snapshot(source, experiment_dir / "captures", stage)
    captured_file = captured_dir / source.name

    manifest["stages"][stage] = {
        "account_alias": account_alias,
        "note": note,
        "path": str(captured_file.relative_to(experiment_dir)),
        "size": captured_file.stat().st_size,
        "sha256": sha256(captured_file),
    }
    (experiment_dir / "experiment.json").write_text(
        json.dumps(manifest, indent=2), encoding="utf-8"
    )
    return captured_dir


def _verified_stage_path(experiment_dir: Path, stage: str, entry: dict) -> Path:
    path = experiment_dir / entry["path"]
    if not path.is_file():
        raise FileNotFoundError(path)
    expected_hash = entry["sha256"].lower()
    if path.stat().st_size != entry["size"] or sha256(path).lower() != expected_hash:
        raise ValueError(f"Captured stage failed integrity verification: {stage}")
    if file_kind(path) != "decrypted_nioh3_user":
        raise ValueError(f"Captured stage is not decrypted NIOH data: {stage}")
    return path


def analyze_experiment(experiment_dir: Path, max_gap: int) -> dict:
    manifest = load_experiment(experiment_dir)
    available = [stage for stage in EXPERIMENT_STAGES if stage in manifest["stages"]]
    if len(available) < 2:
        raise ValueError("Capture at least two experiment stages before analysis.")

    paths = {
        stage: _verified_stage_path(experiment_dir, stage, manifest["stages"][stage])
        for stage in available
    }
    comparisons = []
    for before_stage, after_stage in zip(available, available[1:]):
        report = diff_report(paths[before_stage], paths[after_stage], max_gap)
        comparisons.append(
            {
                "before_stage": before_stage,
                "after_stage": after_stage,
                "same_account": (
                    manifest["stages"][before_stage]["account_alias"]
                    == manifest["stages"][after_stage]["account_alias"]
                ),
                "changed_bytes": report["changed_bytes"],
                "exact_range_count": report["exact_range_count"],
                "ranges": report["ranges"],
            }
        )

    return {
        "schema": EXPERIMENT_SCHEMA,
        "game_version": manifest["game_version"],
        "available_stages": available,
        "complete": len(available) == len(EXPERIMENT_STAGES),
        "warning": (
            "Cross-account absolute offsets are not identity proof. "
            "Use recipient_before_receive -> recipient_after_receive to locate the received record, "
            "then compare record-relative fields with the source record."
        ),
        "comparisons": comparisons,
    }


def paired_u16_runs(data: bytes, record_size: int, start: int, end: int) -> list[dict]:
    candidates: dict[int, list[int]] = {}
    for offset in range(start, end - 4):
        first, second = struct.unpack_from("<HH", data, offset)
        if first == second and first not in (0, 0xFFFF):
            candidates.setdefault(offset % record_size, []).append(offset)

    runs: list[dict] = []
    for residue, offsets in candidates.items():
        run_start = previous = offsets[0]
        count = 1
        for offset in offsets[1:]:
            if offset == previous + record_size:
                count += 1
            else:
                if count >= 2:
                    runs.append(_run_dict(residue, run_start, previous, record_size, count))
                run_start = offset
                count = 1
            previous = offset
        if count >= 2:
            runs.append(_run_dict(residue, run_start, previous, record_size, count))
    return sorted(runs, key=lambda item: (-item["count"], item["start"]))


def _run_dict(residue: int, start: int, last: int, size: int, count: int) -> dict:
    return {
        "residue": residue,
        "start": start,
        "start_hex": hex(start),
        "end": last + size,
        "end_hex": hex(last + size),
        "record_size": size,
        "count": count,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Nioh 3 scroll save-data research tool")
    commands = parser.add_subparsers(dest="command", required=True)

    capture = commands.add_parser("snapshot", help="Copy a save after verifying the game is closed")
    capture.add_argument("source", type=Path)
    capture.add_argument("output_dir", type=Path)
    capture.add_argument("label")

    diff = commands.add_parser("diff", help="Report changed byte ranges between equal-sized files")
    diff.add_argument("before", type=Path)
    diff.add_argument("after", type=Path)
    diff.add_argument("--max-gap", type=parse_int, default=0)
    diff.add_argument("--output", type=Path)

    scan = commands.add_parser("scan-records", help="Find runs of records whose first two u16 values match")
    scan.add_argument("file", type=Path)
    scan.add_argument("--record-size", type=parse_int, required=True)
    scan.add_argument("--start", type=parse_int, default=0)
    scan.add_argument("--end", type=parse_int)
    scan.add_argument("--limit", type=int, default=20)

    experiment = commands.add_parser(
        "experiment-create", help="Create a controlled propagation experiment"
    )
    experiment.add_argument("output_dir", type=Path)
    experiment.add_argument("--game-version", required=True)

    capture_stage = commands.add_parser(
        "experiment-capture", help="Capture one decrypted experiment stage"
    )
    capture_stage.add_argument("source", type=Path)
    capture_stage.add_argument("experiment_dir", type=Path)
    capture_stage.add_argument("stage", choices=EXPERIMENT_STAGES)
    capture_stage.add_argument("--account", required=True, dest="account_alias")
    capture_stage.add_argument("--note", default="")

    analyze = commands.add_parser(
        "experiment-analyze", help="Verify and diff captured experiment stages"
    )
    analyze.add_argument("experiment_dir", type=Path)
    analyze.add_argument("--max-gap", type=parse_int, default=0x20)
    analyze.add_argument("--output", type=Path)

    catalog = commands.add_parser(
        "catalog", help="Extract the special-effect ID dictionary from a Cheat Engine table"
    )
    catalog.add_argument("cheat_table", type=Path)
    catalog.add_argument("--output", type=Path)

    parse_scroll = commands.add_parser(
        "parse-scroll", help="Decode one confirmed 0xE8 scroll record"
    )
    parse_scroll.add_argument("file", type=Path)
    parse_scroll.add_argument("--offset", type=parse_int, default=0)
    parse_scroll.add_argument("--catalog", type=Path, dest="catalog_path")
    parse_scroll.add_argument("--output", type=Path)

    transplant = commands.add_parser(
        "transplant-effect",
        help="Copy one complete 0x18 effect slot between confirmed scroll records",
    )
    transplant.add_argument("file", type=Path)
    transplant.add_argument("output", type=Path)
    transplant.add_argument("--destination-record", type=parse_int, required=True)
    transplant.add_argument("--donor-record", type=parse_int, required=True)
    transplant.add_argument("--destination-slot", type=int, required=True)
    transplant.add_argument("--donor-slot", type=int, required=True)
    transplant.add_argument("--report", type=Path)

    prepare_crypt = commands.add_parser(
        "prepare-encryption",
        help="Clear transient body-key material before passing a decrypted save to the crypt tool",
    )
    prepare_crypt.add_argument("file", type=Path)
    prepare_crypt.add_argument("output", type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "snapshot":
        print(snapshot(args.source, args.output_dir, args.label))
        return 0
    if args.command == "diff":
        report = diff_report(args.before, args.after, args.max_gap)
        rendered = json.dumps(report, indent=2)
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0

    if args.command == "experiment-create":
        print(create_experiment(args.output_dir, args.game_version))
        return 0
    if args.command == "experiment-capture":
        print(
            capture_experiment_stage(
                args.source,
                args.experiment_dir,
                args.stage,
                args.account_alias,
                args.note,
            )
        )
        return 0
    if args.command == "experiment-analyze":
        report = analyze_experiment(args.experiment_dir, args.max_gap)
        rendered = json.dumps(report, indent=2)
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0
    if args.command == "catalog":
        effect_catalog = load_special_effect_catalog(args.cheat_table)
        payload = [
            asdict(effect)
            for effect_id in sorted(effect_catalog)
            for effect in effect_catalog[effect_id]
        ]
        rendered = json.dumps(payload, indent=2, ensure_ascii=False)
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0
    if args.command == "parse-scroll":
        data = args.file.read_bytes()
        end = args.offset + SCROLL_RECORD_SIZE
        if not (0 <= args.offset and end <= len(data)):
            raise ValueError("The scroll record range is outside the file")
        effect_catalog = (
            load_special_effect_catalog(args.catalog_path) if args.catalog_path else None
        )
        payload = parse_scroll_record(data[args.offset:end], effect_catalog)
        rendered = json.dumps(payload, indent=2, ensure_ascii=False)
        if args.output:
            args.output.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0
    if args.command == "transplant-effect":
        if args.output.resolve() == args.file.resolve():
            raise ValueError("Refusing to overwrite the source save")
        edited, report = transplant_effect_slot(
            args.file.read_bytes(),
            args.destination_record,
            args.donor_record,
            args.destination_slot,
            args.donor_slot,
        )
        if args.output.exists():
            raise FileExistsError(args.output)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(edited)
        rendered = json.dumps(report, indent=2)
        if args.report:
            args.report.write_text(rendered, encoding="utf-8")
        print(rendered)
        return 0
    if args.command == "prepare-encryption":
        if args.output.resolve() == args.file.resolve():
            raise ValueError("Refusing to overwrite the source save")
        if args.output.exists():
            raise FileExistsError(args.output)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(prepare_decrypted_save_for_encryption(args.file.read_bytes()))
        print(args.output)
        return 0

    data = args.file.read_bytes()
    end = len(data) if args.end is None else args.end
    if not 0 <= args.start < end <= len(data):
        raise ValueError("The scan range is outside the file.")
    result = paired_u16_runs(data, args.record_size, args.start, end)[: args.limit]
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
