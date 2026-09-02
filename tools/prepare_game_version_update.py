"""Build a fail-closed migration report for a new Nioh 3 executable.

The tool compares two read-only live PE section dumps. It relocates known code
and read-only-data sites by voting on unique unchanged context anchors around
each baseline RVA. The output is a candidate research profile, not an approval
to enable the new version in the product.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, Sequence


PROJECT_ROOT = Path(__file__).resolve().parents[1]
PROFILE_SCHEMA = "nioh3-game-version-research-profile/v1"
REPORT_SCHEMA = "nioh3-game-version-migration-report/v1"


def parse_int(value: object) -> int:
    if isinstance(value, int):
        return value
    return int(str(value), 0)


@dataclass(frozen=True, slots=True)
class SectionImage:
    name: str
    rva: int
    data: bytes
    sha256: str


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return payload


def load_sections(manifest_path: Path) -> tuple[dict[str, Any], dict[str, SectionImage]]:
    manifest = load_json(manifest_path)
    sections: dict[str, SectionImage] = {}
    for raw in manifest.get("sections", ()):
        name = str(raw["name"])
        path = manifest_path.parent / str(raw["filename"])
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest().upper()
        expected = str(raw["sha256"]).upper()
        if digest != expected:
            raise ValueError(f"section hash mismatch for {path}")
        sections[name] = SectionImage(
            name=name,
            rva=parse_int(raw["rva"]),
            data=data,
            sha256=digest,
        )
    return manifest, sections


def project_relative(path: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(PROJECT_ROOT).as_posix()
    except ValueError:
        return str(resolved)


def unique_anchor_votes(
    baseline: SectionImage,
    target: SectionImage,
    baseline_rva: int,
    *,
    radius: int,
    anchor_size: int,
    stride: int,
) -> dict[int, int]:
    baseline_offset = baseline_rva - baseline.rva
    if not 0 <= baseline_offset < len(baseline.data):
        raise ValueError(f"RVA 0x{baseline_rva:X} is outside {baseline.name}")
    votes: dict[int, int] = {}
    for relative in range(-radius, radius + 1, stride):
        start = baseline_offset + relative
        stop = start + anchor_size
        if start < 0 or stop > len(baseline.data):
            continue
        anchor = baseline.data[start:stop]
        first = target.data.find(anchor)
        if first < 0 or target.data.find(anchor, first + 1) >= 0:
            continue
        target_rva = target.rva + first
        anchor_baseline_rva = baseline.rva + start
        delta = target_rva - anchor_baseline_rva
        votes[delta] = votes.get(delta, 0) + 1
    return votes


def relocate_site(
    name: str,
    raw_site: Mapping[str, Any],
    baseline: SectionImage,
    target: SectionImage,
    *,
    radius: int,
    anchor_size: int,
    stride: int,
) -> dict[str, Any]:
    old_rva = parse_int(raw_site["rva"])
    votes = unique_anchor_votes(
        baseline,
        target,
        old_rva,
        radius=radius,
        anchor_size=anchor_size,
        stride=stride,
    )
    ranking = sorted(votes.items(), key=lambda item: (-item[1], abs(item[0])))
    winner = ranking[0] if ranking else None
    runner_up_votes = ranking[1][1] if len(ranking) > 1 else 0
    resolved = bool(winner and winner[1] >= 5 and winner[1] > runner_up_votes)
    new_rva = old_rva + winner[0] if resolved and winner is not None else None
    signature_size = parse_int(
        raw_site.get("signature_size", raw_site.get("size", anchor_size))
    )
    old_offset = old_rva - baseline.rva
    old_signature = baseline.data[old_offset : old_offset + signature_size]
    new_signature = b""
    if new_rva is not None:
        new_offset = new_rva - target.rva
        new_signature = target.data[new_offset : new_offset + signature_size]
        if len(new_signature) != signature_size:
            resolved = False
            new_rva = None
    result: dict[str, Any] = {
        "name": name,
        "status": "resolved" if resolved else "unresolved",
        "baseline_rva": f"0x{old_rva:X}",
        "target_rva": f"0x{new_rva:X}" if new_rva is not None else None,
        "delta": f"{winner[0]:+#x}" if winner is not None else None,
        "winning_anchor_votes": winner[1] if winner is not None else 0,
        "runner_up_anchor_votes": runner_up_votes,
        "baseline_signature": old_signature.hex(" ").upper(),
        "target_signature": new_signature.hex(" ").upper() if new_signature else None,
        "signature_unchanged": bool(new_signature and new_signature == old_signature),
        "top_delta_candidates": [
            {"delta": f"{delta:+#x}", "votes": count}
            for delta, count in ranking[:5]
        ],
    }
    if "range_size" in raw_site:
        result["range_size"] = parse_int(raw_site["range_size"])
    return result


def apply_relocation_overrides(
    report: dict[str, Any],
    baseline_profile_path: Path,
    target_manifest_path: Path,
    overrides: Mapping[str, Mapping[str, int]],
) -> None:
    """Apply explicit, evidence-backed site RVAs without hiding their origin."""

    baseline_profile = load_json(baseline_profile_path)
    _, target_sections = load_sections(target_manifest_path)
    section_names = {"text_sites": ".text", "rdata_sites": ".rdata"}
    for group_name, group_overrides in overrides.items():
        if not group_overrides:
            continue
        known_sites = baseline_profile.get(group_name, {})
        unknown = set(group_overrides).difference(known_sites)
        if unknown:
            raise ValueError(
                f"unknown {group_name} override(s): " + ", ".join(sorted(unknown))
            )
        target_section = target_sections[section_names[group_name]]
        report_items = {
            str(item["name"]): item
            for item in report["relocations"][group_name]
        }
        for name, target_rva in group_overrides.items():
            source_site = known_sites[name]
            baseline_rva = parse_int(source_site["rva"])
            size = parse_int(
                source_site.get(
                    "signature_size", source_site.get("size", 16)
                )
            )
            offset = target_rva - target_section.rva
            target_signature = target_section.data[offset : offset + size]
            if len(target_signature) != size:
                raise ValueError(
                    f"{group_name}.{name} override is outside the target section"
                )
            item = report_items[name]
            item.update(
                {
                    "status": "resolved_by_explicit_override",
                    "target_rva": f"0x{target_rva:X}",
                    "delta": f"{target_rva - baseline_rva:+#x}",
                    "target_signature": target_signature.hex(" ").upper(),
                    "signature_unchanged": (
                        target_signature.hex(" ").upper()
                        == str(item["baseline_signature"])
                    ),
                    "override_requires_independent_evidence": True,
                }
            )

    unresolved = [
        item["name"]
        for group in report["relocations"].values()
        for item in group
        if not str(item["status"]).startswith("resolved")
    ]
    changed_signatures = [
        item["name"]
        for group in report["relocations"].values()
        for item in group
        if str(item["status"]).startswith("resolved")
        and not item["signature_unchanged"]
    ]
    report["unresolved_sites"] = unresolved
    report["changed_signature_sites"] = changed_signatures
    report["gates"]["all_anchor_relocations_resolved"] = not unresolved
    report["gates"]["all_site_signatures_unchanged"] = not changed_signatures


def build_report(
    baseline_profile_path: Path,
    target_manifest_path: Path,
    *,
    radius: int = 1024,
    anchor_size: int = 16,
    stride: int = 16,
) -> dict[str, Any]:
    profile = load_json(baseline_profile_path)
    if profile.get("schema") != PROFILE_SCHEMA:
        raise ValueError("unsupported baseline research profile schema")
    baseline_manifest_path = PROJECT_ROOT / str(
        profile["section_dump"]["manifest"]
    )
    baseline_manifest, baseline_sections = load_sections(baseline_manifest_path)
    target_manifest, target_sections = load_sections(target_manifest_path)

    relocations: dict[str, list[dict[str, Any]]] = {}
    for profile_key, section_name in (("text_sites", ".text"), ("rdata_sites", ".rdata")):
        baseline_section = baseline_sections[section_name]
        target_section = target_sections[section_name]
        relocations[profile_key] = [
            relocate_site(
                name,
                raw_site,
                baseline_section,
                target_section,
                radius=radius,
                anchor_size=anchor_size,
                stride=stride,
            )
            for name, raw_site in profile.get(profile_key, {}).items()
        ]

    unresolved = [
        item["name"]
        for group in relocations.values()
        for item in group
        if item["status"] != "resolved"
    ]
    changed_signatures = [
        item["name"]
        for group in relocations.values()
        for item in group
        if item["status"] == "resolved" and not item["signature_unchanged"]
    ]
    target_version = str(
        target_manifest.get("file_version_text")
        or target_manifest.get("game_version")
        or "unknown"
    )
    return {
        "schema": REPORT_SCHEMA,
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_profile": str(baseline_profile_path.resolve()),
        "baseline_version": profile["display_version"],
        "target_version": target_version,
        "target_executable": target_manifest.get("executable"),
        "section_changes": {
            name: {
                "baseline_rva": f"0x{baseline_sections[name].rva:X}",
                "target_rva": f"0x{target_sections[name].rva:X}",
                "baseline_size": len(baseline_sections[name].data),
                "target_size": len(target_sections[name].data),
                "baseline_sha256": baseline_sections[name].sha256,
                "target_sha256": target_sections[name].sha256,
            }
            for name in sorted(set(baseline_sections).intersection(target_sections))
        },
        "relocations": relocations,
        "data_sites": {
            "status": "requires_runtime_validation",
            "sites": profile.get("data_sites", {}),
        },
        "gates": {
            "all_anchor_relocations_resolved": not unresolved,
            "all_site_signatures_unchanged": not changed_signatures,
            "runtime_data_sites_validated": False,
            "parameter_tables_compared": False,
            "native_parity_validated": False,
            "save_layout_validated": False,
            "product_enablement_allowed": False,
        },
        "unresolved_sites": unresolved,
        "changed_signature_sites": changed_signatures,
        "limitations": [
            "Anchor relocation proves only that surrounding bytes moved together.",
            "Changed code, runtime data pointers, parameter tables, native parity, and save layout require separate validation.",
            "This report must never enable a game version automatically.",
        ],
    }


def build_candidate_profile(
    baseline_profile_path: Path,
    target_manifest_path: Path,
    report: Mapping[str, Any],
    *,
    display_version: str,
    data_site_overrides: Mapping[str, int],
) -> dict[str, Any]:
    """Build an explicitly unapproved profile from one migration report.

    Relocated code and rdata sites are tied to the captured target-section
    hashes. Runtime data pointers are never shifted heuristically: callers must
    provide each observed candidate RVA explicitly, and later validation still
    has to promote it.
    """

    baseline = load_json(baseline_profile_path)
    target_manifest = load_json(target_manifest_path)
    target_version = tuple(int(part) for part in target_manifest["file_version"])
    if len(target_version) != 4:
        raise ValueError("target manifest file_version must contain four integers")

    relocated_groups: dict[str, dict[str, Any]] = {}
    for group_name in ("text_sites", "rdata_sites"):
        source_group = baseline.get(group_name, {})
        migrated_group: dict[str, Any] = {}
        report_items = {
            str(item["name"]): item
            for item in report["relocations"][group_name]
        }
        for name, source_site in source_group.items():
            result = report_items[name]
            migrated: dict[str, Any] = {
                key: value for key, value in source_site.items() if key != "rva"
            }
            migrated["baseline_rva"] = source_site["rva"]
            migrated["relocation_status"] = result["status"]
            migrated["rva"] = result["target_rva"]
            if result["target_signature"] is not None:
                migrated["captured_signature"] = result["target_signature"]
            migrated_group[name] = migrated
        relocated_groups[group_name] = migrated_group

    data_sites: dict[str, Any] = {}
    unknown_overrides = set(data_site_overrides).difference(
        baseline.get("data_sites", {})
    )
    if unknown_overrides:
        raise ValueError(
            "unknown data-site override(s): " + ", ".join(sorted(unknown_overrides))
        )
    for name, source_site in baseline.get("data_sites", {}).items():
        candidate_rva = data_site_overrides.get(name)
        data_sites[name] = {
            "baseline_rva": source_site["rva"],
            "rva": f"0x{candidate_rva:X}" if candidate_rva is not None else None,
            "validation_status": (
                "candidate_requires_runtime_validation"
                if candidate_rva is not None
                else "unresolved"
            ),
        }

    version_slug = "_".join(str(part) for part in target_version)
    return {
        "schema": PROFILE_SCHEMA,
        "profile_id": f"pc_v{version_slug}_candidate",
        "display_version": display_version,
        "file_version": list(target_version),
        "approval_status": "candidate",
        "product_enablement_allowed": False,
        "provenance": {
            "baseline_profile": project_relative(baseline_profile_path),
            "migration_report": project_relative(Path(str(report["output_path"])))
            if report.get("output_path")
            else None,
            "target_manifest_sha256": hashlib.sha256(
                target_manifest_path.read_bytes()
            ).hexdigest().upper(),
        },
        "section_dump": {"manifest": project_relative(target_manifest_path)},
        **relocated_groups,
        "data_sites": data_sites,
        "resources": {
            "status": "pending_target_table_comparison",
            "baseline": baseline.get("resources", {}),
        },
        "gates": dict(report["gates"]),
        "limitations": [
            "This candidate profile is generated from relocation evidence only.",
            "It must not be used for product enablement until every gate passes.",
        ],
    }


def parse_named_rva(value: str) -> tuple[str, int]:
    name, separator, raw_rva = value.partition("=")
    if not separator or not name.strip() or not raw_rva.strip():
        raise argparse.ArgumentTypeError("expected NAME=RVA")
    try:
        rva = int(raw_rva, 0)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"invalid RVA in {value!r}") from error
    if rva < 0:
        raise argparse.ArgumentTypeError("RVA must be non-negative")
    return name.strip(), rva


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Prepare a fail-closed Nioh 3 game-version migration report"
    )
    parser.add_argument("--baseline-profile", type=Path, required=True)
    parser.add_argument("--target-manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--candidate-profile-output", type=Path)
    parser.add_argument("--target-display-version")
    parser.add_argument(
        "--data-site-rva",
        action="append",
        default=[],
        type=parse_named_rva,
        metavar="NAME=RVA",
        help="Observed target data-site RVA; repeat for multiple sites",
    )
    parser.add_argument(
        "--text-site-rva",
        action="append",
        default=[],
        type=parse_named_rva,
        metavar="NAME=RVA",
        help="Evidence-backed target text-site RVA override",
    )
    parser.add_argument(
        "--rdata-site-rva",
        action="append",
        default=[],
        type=parse_named_rva,
        metavar="NAME=RVA",
        help="Evidence-backed target rdata-site RVA override",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.output.exists():
        raise FileExistsError(f"refusing to overwrite existing report: {args.output}")
    if args.candidate_profile_output is not None and args.candidate_profile_output.exists():
        raise FileExistsError(
            "refusing to overwrite existing candidate profile: "
            f"{args.candidate_profile_output}"
        )
    if args.candidate_profile_output is not None and not args.target_display_version:
        raise ValueError(
            "--target-display-version is required with --candidate-profile-output"
        )
    report = build_report(args.baseline_profile, args.target_manifest)
    text_site_overrides = dict(args.text_site_rva)
    rdata_site_overrides = dict(args.rdata_site_rva)
    if len(text_site_overrides) != len(args.text_site_rva):
        raise ValueError("duplicate --text-site-rva name")
    if len(rdata_site_overrides) != len(args.rdata_site_rva):
        raise ValueError("duplicate --rdata-site-rva name")
    apply_relocation_overrides(
        report,
        args.baseline_profile,
        args.target_manifest,
        {
            "text_sites": text_site_overrides,
            "rdata_sites": rdata_site_overrides,
        },
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report["output_path"] = str(args.output.resolve())
    args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    candidate_output: str | None = None
    if args.candidate_profile_output is not None:
        data_site_overrides = dict(args.data_site_rva)
        if len(data_site_overrides) != len(args.data_site_rva):
            raise ValueError("duplicate --data-site-rva name")
        candidate = build_candidate_profile(
            args.baseline_profile,
            args.target_manifest,
            report,
            display_version=args.target_display_version,
            data_site_overrides=data_site_overrides,
        )
        args.candidate_profile_output.parent.mkdir(parents=True, exist_ok=True)
        args.candidate_profile_output.write_text(
            json.dumps(candidate, indent=2), encoding="utf-8"
        )
        candidate_output = str(args.candidate_profile_output.resolve())
    print(
        json.dumps(
            {
                "output": str(args.output.resolve()),
                "candidate_profile_output": candidate_output,
                "target_version": report["target_version"],
                "gates": report["gates"],
                "unresolved_sites": report["unresolved_sites"],
                "changed_signature_sites": report["changed_signature_sites"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
