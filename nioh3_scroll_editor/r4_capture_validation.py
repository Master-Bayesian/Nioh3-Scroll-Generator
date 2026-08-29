"""Semantic validation of a fresh R4 capture against bundled offline data."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from .r4_finalizer_resource import (
    DEFAULT_RESOURCE_ROOT,
    REQUIRED_TABLES,
    R4FinalizerResourceBundle,
)
from .r4_table_bundle import R4FinalizerTableBundle


def _record(
    checks: dict[str, bool],
    mismatches: list[str],
    name: str,
    matches: bool,
) -> None:
    checks[name] = matches
    if not matches:
        mismatches.append(name)


def validate_r4_capture_against_resource(
    capture_root: str | Path,
    resource_root: str | Path = DEFAULT_RESOURCE_ROOT,
) -> dict[str, Any]:
    """Compare stable semantics while deliberately ignoring process addresses."""

    capture = R4FinalizerTableBundle(capture_root, verify=True)
    resource = R4FinalizerResourceBundle(resource_root, verify=True)
    checks: dict[str, bool] = {}
    mismatches: list[str] = []

    _record(
        checks,
        mismatches,
        "game_version",
        str(capture.manifest.get("expected_game_version"))
        == str(resource.manifest.get("game_version")),
    )
    _record(
        checks,
        mismatches,
        "effective_playthrough",
        capture.effective_playthrough
        == int(resource.manifest["source"]["effective_playthrough"]),
    )

    source_pe = capture.manifest.get("pe", {})
    resource_pe = resource.manifest["source"].get("pe", {})
    for field in (
        "machine",
        "section_count",
        "timestamp",
        "optional_header_magic",
        "size_of_image",
    ):
        _record(
            checks,
            mismatches,
            f"pe.{field}",
            source_pe.get(field) == resource_pe.get(field),
        )

    expected_signatures = resource.manifest["code_identity"].get("signatures", {})
    actual_signatures = capture.manifest.get("code_signatures", {})
    for name, expected in expected_signatures.items():
        actual = actual_signatures.get(name, {})
        _record(
            checks,
            mismatches,
            f"code_signature.{name}",
            bool(actual.get("matches"))
            and actual.get("rva") == expected.get("rva")
            and actual.get("actual") == expected.get("actual"),
        )

    expected_ranges = {
        str(item["name"]): item
        for item in resource.manifest["code_identity"].get("ranges", [])
    }
    actual_ranges = {
        str(item["name"]): item for item in capture.manifest.get("code", [])
    }
    for name, expected in expected_ranges.items():
        actual = actual_ranges.get(name, {})
        blob = actual.get("blob", {})
        _record(
            checks,
            mismatches,
            f"code_range.{name}",
            actual.get("begin_rva") == expected.get("begin_rva")
            and actual.get("end_rva") == expected.get("end_rva")
            and int(blob.get("size", -1)) == int(expected.get("size", -2))
            and str(blob.get("sha256", "")).upper()
            == str(expected.get("sha256", "")).upper(),
        )

    for name in REQUIRED_TABLES:
        _record(
            checks,
            mismatches,
            f"table.{name}",
            capture.table(name).row_store == resource.table(name).row_store,
        )

    for selector in range(1, 6):
        _record(
            checks,
            mismatches,
            f"playthrough.{selector}",
            capture.playthrough_progress(selector)
            == resource.playthrough_progress(selector),
        )
    _record(
        checks,
        mismatches,
        "mode_gate_bytes",
        capture.mode_gate_bytes() == resource.mode_gate_bytes(),
    )

    expected_constants = resource.manifest.get("float_constants", {})
    actual_constants = capture.manifest.get("float_constants", {})
    for name, expected in expected_constants.items():
        _record(
            checks,
            mismatches,
            f"float_constant.{name}",
            str(actual_constants.get(name, {}).get("bits", "")).upper()
            == str(expected.get("bits", "")).upper(),
        )

    capture_root_path = Path(capture_root)
    bonus_entries = capture.manifest["bonus_curve"]["rows"]
    _record(
        checks,
        mismatches,
        "bonus_curve.entry_count",
        len(bonus_entries) == int(resource.manifest["bonus_curve"]["entry_count"]),
    )
    if checks["bonus_curve.entry_count"]:
        bonus_matches = True
        for index, item in enumerate(bonus_entries):
            expected_entry = resource.bonus_curve_entry(index)
            if not bool(item.get("valid", False)):
                if expected_entry.row is not None:
                    bonus_matches = False
                    break
                continue
            relative = item.get("blob", {}).get("filename") or item.get("duplicate_of")
            if not relative:
                bonus_matches = False
                break
            if expected_entry.row != (capture_root_path / str(relative)).read_bytes():
                bonus_matches = False
                break
        _record(checks, mismatches, "bonus_curve.entries", bonus_matches)

    return {
        "schema": "nioh3-r4-finalizer-capture-validation/v1",
        "matches": not mismatches,
        "capture": str(Path(capture_root)),
        "resource": str(Path(resource_root)),
        "checks": checks,
        "mismatches": mismatches,
        "ignored_runtime_fields": [
            "pid",
            "module_base",
            "heap addresses",
            "table manager contexts",
            "runtime pointer vectors",
        ],
    }


__all__ = ["validate_r4_capture_against_resource"]
