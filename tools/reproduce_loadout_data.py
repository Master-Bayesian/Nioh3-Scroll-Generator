"""Rebuild current planner data using the pinned maintainer handoff inputs.

Run with project-source/tools/run_python_tests.ps1, not ambient Python.
No game process, save access, or network extraction is used.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

PROJECT = Path(__file__).resolve().parents[1]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--handoff-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        raise FileExistsError("Use a fresh output directory")
    package = args.handoff_root.resolve()
    source = PROJECT
    research_source = package / "project-source"
    project = output / "project-source"
    output.mkdir(parents=True)
    shutil.copytree(research_source, project)
    for relative in ("apps/workshop", "tools", "nioh3_scroll_editor"):
        shutil.copytree(source / relative, project / relative, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns("__pycache__", "node_modules"))
    # Preserve immutable expectations; all exporters write only into this copy.
    assets = sorted((source / "apps/workshop").glob("loadout-*.json"))
    expected = {p.name: p.read_bytes() for p in assets}
    research = project / "deliverables"
    guardian = research / "guardian-spirit-catalog-20261008"
    clan = research / "clan-protection-catalog-20261010"
    skill = research / "skill-catalog-20261009"
    runtime = research / "load-capacity-research/disassembly/live-sections"
    calls = []
    environment = dict(os.environ, PYTHONUTF8="1", PYTHONDONTWRITEBYTECODE="1",
                       PYTHONPATH=str(package / "scripts/read_boundary"),
                       HANDOFF_REPLAY_ROOT=str(output))
    def run(script, *arguments):
        command = [sys.executable, str(project / script), *map(str, arguments)]
        completed = subprocess.run(command, cwd=project, env=environment,
                                   capture_output=True, text=True, encoding="utf-8")
        name = Path(script).stem + "-" + str(len(calls))
        (output / (name + ".log")).write_text(completed.stdout + completed.stderr, encoding="utf-8")
        calls.append(dict(script=script, returncode=completed.returncode))
        if completed.returncode:
            print(completed.stdout + completed.stderr)
            raise RuntimeError("Reproduction failed: " + script)
    for topic, script in [
        ("guardian-spirit-catalog-20261008", "build_native_catalog.py"),
        ("clan-protection-catalog-20261010", "build_native_catalog.py"),
        ("skill-catalog-20261009", "build_native_skill_catalog.py"),
    ]:
        run("deliverables/" + topic + "/native-extraction/" + script)
    for topic in ("guardian-spirit-catalog-20261008", "clan-protection-catalog-20261010"):
        run("deliverables/" + topic + "/native-extraction/validate_native_catalog.py")
    # Product clan/skill datasets are exact reviewed catalog copies.
    shutil.copyfile(clan / "clan_protections_native.json", project / "apps/workshop/loadout-clans.json")
    shutil.copyfile(skill / "skills_native.json", project / "apps/workshop/loadout-skills.json")
    def node(script, *arguments):
        completed = subprocess.run(["node", "--experimental-permission", "--allow-fs-read=" + str(project),
                                    "--allow-fs-write=" + str(project), str(project / script), *map(str, arguments)], cwd=project,
                                   capture_output=True, text=True, encoding="utf-8")
        (output / (Path(script).stem + ".log")).write_text(completed.stdout + completed.stderr, encoding="utf-8")
        calls.append(dict(script=script, returncode=completed.returncode))
        if completed.returncode:
            print(completed.stdout + completed.stderr)
            raise RuntimeError("Node projection failed: " + script)
    node("tools/export_loadout_guardians.mjs", guardian / "guardian_spirits_native.json")
    node("tools/export_loadout_soul_cores.mjs")
    node("tools/export_loadout_skill_scope.mjs", skill / "native-extraction")
    run("tools/export_loadout_sets.py", "--capture", skill / "native-extraction/discovery-02/capture.json",
        "--native-text", clan / "native-extraction/native-text-01.json", "--runtime-sections", runtime)
    run("tools/export_loadout_effect_catalog.py", "--guardian-catalog", guardian / "guardian_spirits_native.json",
        "--clan-catalog", clan / "clan_protections_native.json", "--native-text", clan / "native-extraction/native-text-01.json",
        "--capture", skill / "native-extraction/discovery-02/capture.json", "--runtime-sections", runtime)
    run("tools/build_loadout_armor_requirements.py", "--research-root", project,
        "--output", project / "apps/workshop/loadout-armor-requirements.json")
    run("tools/build_loadout_stats.py", "--armor-root", project,
        "--capacity-root", research / "load-capacity-research",
        "--validation-output", output / "capacity-validation.json",
        "--output", project / "apps/workshop/loadout-stats.json")
    run("tools/build_loadout_resources.py", "--research-root", project,
        "--validation-output", output / "resource-vectors.json",
        "--output", project / "apps/workshop/loadout-resources.json")
    results = []
    for name, raw in expected.items():
        actual = project / "apps/workshop" / name
        generated = actual.read_bytes()
        semantic = json.loads(raw) == json.loads(generated)
        canonical = raw.replace(b"\r\n", b"\n") == generated.replace(b"\r\n", b"\n")
        results.append(dict(asset=name, byte_equal=raw == generated, canonical_byte_equal=canonical, semantic_equal=semantic,
                            expected_sha256=hashlib.sha256(raw).hexdigest(), actual_sha256=digest(actual)))
        if not canonical or not semantic:
            raise ValueError("Regenerated asset differs: " + name)
    report = dict(status="passed", projections=len(results), commands=calls, results=results,
                  python_file_boundary="File opens restricted to isolated replay output and installed Python runtimes.",
                  node_file_boundary="Node permission mode restricts projection reads/writes to the isolated source copy.",
                  dependencies={name: importlib.metadata.version(name) for name in ("capstone", "pytest", "lupa")},
                  boundary="Package-contained source and native inputs only; installed Python/Node toolchains remain prerequisites.")
    (output / "reproduction.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dict(status="passed", assets=len(results),
                          byte_identical=sum(r["byte_equal"] for r in results),
                          canonical_byte_identical=sum(r["canonical_byte_equal"] for r in results))))

if __name__ == "__main__":
    main()
