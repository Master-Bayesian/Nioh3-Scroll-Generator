from __future__ import annotations

import re
import argparse
import ast
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CAPTURE_DIR = ROOT / "research" / "possessed_enemy_capture"
TEXT_PATH = ROOT / "audit" / "runtime_sections" / "v2.0.1.0_20260902_title" / "Nioh3_v2.0.1.0.text.bin"
TEXT_RVA = 0x1000
SITE_PATTERN = re.compile(
    r"(?P<name>[a-zA-Z0-9_]+)\s*=\s*\{rva=0x(?P<rva>[0-9A-Fa-f]+),\s*expected=\{(?P<bytes>[^}]*)}}"
)
HEX_SITE_PATTERN = re.compile(
    r"(?P<name>[a-zA-Z0-9_]+)\s*=\s*\{rva=0x(?P<rva>[0-9A-Fa-f]+),\s*hex='(?P<bytes>[0-9A-Fa-f]+)'"
)
ANCHOR_PATTERN = re.compile(
    r"anchor_rva=0x(?P<rva>[0-9A-Fa-f]+),anchor_hex='(?P<bytes>[0-9A-Fa-f]+)'"
)
FORBIDDEN = (
    "writeBytes", "writeInteger", "writeQword", "writePointer", "writeString",
    "autoAssemble", "executeCode", "executeCodeEx", "createProcess", "shellExecute",
)
EXCLUDED_OBSERVERS = {
    "observer_common_ce.lua",
    "dump_possessed_enemy_runtime_ce.lua",
    "save_probe_json_ce.lua",
}


def parse_expected(text: str) -> list[tuple[str, int, bytes]]:
    sites: list[tuple[str, int, bytes]] = []
    for match in SITE_PATTERN.finditer(text):
        values = bytes(int(token.strip(), 0) for token in match.group("bytes").split(",") if token.strip())
        sites.append((match.group("name"), int(match.group("rva"), 16), values))
    for match in HEX_SITE_PATTERN.finditer(text):
        sites.append((match.group("name"), int(match.group("rva"), 16),
                      bytes.fromhex(match.group("bytes"))))
    return sites


def parse_anchors(text: str) -> list[tuple[int, bytes]]:
    return [
        (int(match.group("rva"), 16), bytes.fromhex(match.group("bytes")))
        for match in ANCHOR_PATTERN.finditer(text)
    ]


def parse_runner_phases(path: Path) -> dict[str, str]:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "PHASES" for target in node.targets
        ):
            value = ast.literal_eval(node.value)
            if not isinstance(value, dict) or not all(
                isinstance(key, str) and isinstance(item, str) for key, item in value.items()
            ):
                raise AssertionError("PHASES must be a string-to-string literal mapping")
            return value
    raise AssertionError(f"PHASES mapping not found in {path}")


def validate_collectors(text_path: Path, capture_dir: Path) -> dict[str, Any]:
    text_section = text_path.read_bytes()
    observer_files = sorted(capture_dir.glob("*_ce.lua"))
    phase_observers = [path for path in observer_files if path.name not in EXCLUDED_OBSERVERS]
    if not phase_observers:
        raise RuntimeError("No observer files found")
    runner_phases = parse_runner_phases(capture_dir / "run_possessed_enemy_observer.py")
    mapped_observers = set(runner_phases.values())
    actual_observers = {path.name for path in phase_observers}
    if mapped_observers != actual_observers:
        missing = sorted(actual_observers - mapped_observers)
        unknown = sorted(mapped_observers - actual_observers)
        raise AssertionError(f"Runner phase mapping mismatch; missing={missing}, unknown={unknown}")

    checked = 0
    checked_anchors = 0
    phase_results: list[dict[str, Any]] = []
    for path in phase_observers:
        source = path.read_text(encoding="utf-8")
        sites = parse_expected(source)
        if not sites:
            raise AssertionError(f"No signature-gated sites in {path.name}")
        if len(sites) > 4:
            raise AssertionError(f"Hardware breakpoint limit exceeded in {path.name}: {len(sites)}")
        for forbidden in FORBIDDEN:
            if forbidden in source:
                raise AssertionError(f"Forbidden mutation/process API {forbidden} in {path.name}")
        for name, rva, expected in sites:
            offset = rva - TEXT_RVA
            actual = text_section[offset:offset + len(expected)]
            if actual != expected:
                raise AssertionError(
                    f"PC v2.01 signature mismatch: {path.name}:{name} at 0x{rva:X}; "
                    f"expected {expected.hex(' ')}, got {actual.hex(' ')}"
                )
            checked += 1
        anchors = parse_anchors(source)
        for rva, expected in anchors:
            offset = rva - TEXT_RVA
            actual = text_section[offset:offset + len(expected)]
            if actual != expected:
                raise AssertionError(
                    f"PC v2.01 branch-anchor mismatch: {path.name} at 0x{rva:X}; "
                    f"expected {expected.hex(' ')}, got {actual.hex(' ')}"
                )
            checked_anchors += 1
        phase_results.append({"file": path.name, "sites": len(sites), "anchors": len(anchors)})
    return {"checked_sites": checked, "checked_anchors": checked_anchors, "phases": phase_results}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--text", type=Path, default=TEXT_PATH)
    parser.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    args = parser.parse_args()
    result = validate_collectors(args.text, args.capture_dir)
    for phase in result["phases"]:
        suffix = f", {phase['anchors']} branch anchors" if phase["anchors"] else ""
        print(f"PASS {phase['file']}: {phase['sites']} sites{suffix}")
    print(f"PASS {result['checked_sites']} total breakpoint signatures; all phases use at most four sites")
    if result["checked_anchors"]:
        print(f"PASS {result['checked_anchors']} non-breakpoint branch anchors")


if __name__ == "__main__":
    main()
