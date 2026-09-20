"""Export the final r5 debug-host acceptance identity link.

This is a bounded closure collector for Pro ticket R5-EVIDENCE-LINK. It rebuilds
the Tauri frontend and confirms the five Cargo artifacts with the shared target,
runs only the seven packaged-host resolver tests, retains their normally
temporary JSON evidence, and emits a self-contained research-handoff package.

It never opens the game, discovers a user save, publishes an artifact, or turns
the debug host into a release-candidate claim.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
import zipfile


ROOT = Path(__file__).resolve().parents[1]
TEST_PATH = ROOT / "tests/migration/test_packaged_host_resolver.py"
GAME_FILE_VERSION = "2.0.2.0"
ROLES = ("offline_search", "save", "runtime")
EXPECTED_TESTS = {
    "test_staged_layout_declares_the_roots_the_resolver_uses",
    "test_resolved_launch_matches_the_staged_manifest_for_every_role",
    "test_resolved_launch_really_starts_the_declared_binary",
    "test_a_package_without_the_manifest_keeps_the_shipped_python_graph",
    "test_a_damaged_manifest_fails_the_resolver_instead_of_falling_back",
    "test_real_host_process_resolves_the_staged_graph",
    "test_real_frontend_acceptance_on_the_rust_packaged_graph",
}
FROZEN_SOURCE_FILES = (
    "tests/migration/test_packaged_host_resolver.py",
    "tests/migration/cargo_target.py",
    "apps/tauri/verify-host-package.mjs",
    "apps/tauri/verify-packaged-frontend.mjs",
    "apps/tauri/build.mjs",
    "apps/tauri/entry.ts",
    "apps/tauri/src-tauri/Cargo.toml",
    "apps/tauri/src-tauri/Cargo.lock",
    "apps/tauri/src-tauri/build.rs",
    "apps/tauri/src-tauri/tauri.conf.json",
    "apps/launcher/Cargo.toml",
    "apps/launcher/Cargo.lock",
    "crates/nioh3-worker/Cargo.toml",
    "crates/nioh3-worker/Cargo.lock",
    "crates/nioh3-protected/Cargo.toml",
    "crates/nioh3-protected/Cargo.lock",
    "tools/stage_rust_workers.py",
    "tools/package_tauri.py",
    "tools/run_python_tests.ps1",
    "tools/export_r5_evidence_link.py",
)
IDENTITY_ROOTS = (
    "apps/tauri",
    "apps/launcher",
    "apps/workshop",
    "crates",
    "packages/contracts",
    "nioh3_scroll_editor/data",
    "bin",
)
EXCLUDED_PARTS = {
    ".git",
    ".codex_tmp",
    ".pytest_cache",
    "__pycache__",
    "deliverables",
    "node_modules",
    "target",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def file_identity(path: Path, *, relative_to: Path | None = None) -> dict[str, object]:
    resolved = path.resolve()
    label = (
        resolved.relative_to(relative_to.resolve()).as_posix()
        if relative_to is not None
        else str(resolved)
    )
    return {
        "path": label,
        "size": resolved.stat().st_size,
        "sha256": sha256_file(resolved),
    }


def write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def checked_output(arguments: list[str]) -> str:
    return subprocess.run(
        arguments,
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    ).stdout.strip()


def run_recorded(
    name: str,
    arguments: list[str],
    evidence_root: Path,
    environment: dict[str, str],
    *,
    timeout: int,
) -> dict[str, object]:
    logs = evidence_root / "command-logs"
    logs.mkdir(parents=True, exist_ok=True)
    started = utc_now()
    monotonic = time.monotonic()
    completed = subprocess.run(
        arguments,
        cwd=ROOT,
        env=environment,
        capture_output=True,
        timeout=timeout,
    )
    duration = time.monotonic() - monotonic
    stdout = completed.stdout.decode("utf-8", "replace")
    stderr = completed.stderr.decode("utf-8", "replace")
    stdout_path = logs / f"{name}.stdout.log"
    stderr_path = logs / f"{name}.stderr.log"
    stdout_path.write_text(stdout, encoding="utf-8")
    stderr_path.write_text(stderr, encoding="utf-8")
    return {
        "name": name,
        "arguments": arguments,
        "cwd": str(ROOT),
        "startedAtUtc": started,
        "finishedAtUtc": utc_now(),
        "durationSeconds": round(duration, 3),
        "exitCode": completed.returncode,
        "stdout": stdout_path.relative_to(evidence_root.parent).as_posix(),
        "stderr": stderr_path.relative_to(evidence_root.parent).as_posix(),
        "stdoutTail": stdout[-2000:],
        "stderrTail": stderr[-2000:],
    }


def relevant_source_files() -> list[Path]:
    paths: set[Path] = set()
    for relative in FROZEN_SOURCE_FILES:
        path = ROOT / relative
        if path.is_file():
            paths.add(path)
    for relative in IDENTITY_ROOTS:
        base = ROOT / relative
        if not base.exists():
            continue
        for path in base.rglob("*"):
            if not path.is_file():
                continue
            if any(part in EXCLUDED_PARTS for part in path.relative_to(ROOT).parts):
                continue
            paths.add(path)
    for relative in ("package.json", "package-lock.json"):
        path = ROOT / relative
        if path.is_file():
            paths.add(path)
    return sorted(paths, key=lambda item: item.relative_to(ROOT).as_posix())


def source_identity_manifest() -> dict[str, object]:
    files = [file_identity(path, relative_to=ROOT) for path in relevant_source_files()]
    canonical = "".join(
        f"{entry['sha256']}  {entry['path']}\n" for entry in files
    ).encode("utf-8")
    return {
        "schema": "nioh3-r5-source-identities/v1",
        "fileCount": len(files),
        "manifestDigest": hashlib.sha256(canonical).hexdigest(),
        "files": files,
    }


def copy_frozen_sources(destination: Path) -> None:
    for relative in FROZEN_SOURCE_FILES:
        source = ROOT / relative
        if not source.is_file():
            raise FileNotFoundError(f"frozen source is missing: {source}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def binary_identities(target: Path) -> dict[str, dict[str, object]]:
    paths = {
        "debugHost": target / "debug/nioh3-studio.exe",
        "debugSearchWorker": target / "debug/nioh3-readonly-worker.exe",
        "debugProtectedWorker": target / "debug/nioh3-protected-worker.exe",
        "releaseHostFixturePrerequisite": target / "release/nioh3-studio.exe",
        "releaseLauncherFixturePrerequisite": target / "release/Nioh3Launcher.exe",
    }
    missing = [str(path) for path in paths.values() if not path.is_file()]
    if missing:
        raise FileNotFoundError(f"built artifacts are missing: {missing}")
    return {name: file_identity(path) for name, path in paths.items()}


def parse_junit(path: Path, pytest_exit_code: int) -> dict[str, object]:
    tree = ET.parse(path)
    nodes: list[dict[str, object]] = []
    for case in tree.getroot().iter("testcase"):
        name = case.attrib.get("name", "")
        if name not in EXPECTED_TESTS:
            continue
        if case.find("failure") is not None:
            status = "failed"
            detail = case.find("failure").text
        elif case.find("error") is not None:
            status = "error"
            detail = case.find("error").text
        elif case.find("skipped") is not None:
            status = "skipped"
            detail = case.find("skipped").attrib.get("message")
        else:
            status = "passed"
            detail = None
        nodes.append(
            {
                "name": name,
                "classname": case.attrib.get("classname"),
                "timeSeconds": float(case.attrib.get("time", "0")),
                "status": status,
                "detail": detail,
            }
        )
    observed = {node["name"] for node in nodes}
    return {
        "schema": "nioh3-r5-seven-node-results/v1",
        "pytestExitCode": pytest_exit_code,
        "expectedCount": len(EXPECTED_TESTS),
        "observedCount": len(nodes),
        "missing": sorted(EXPECTED_TESTS - observed),
        "unexpected": sorted(observed - EXPECTED_TESTS),
        "passed": sum(node["status"] == "passed" for node in nodes),
        "failed": sum(node["status"] in {"failed", "error"} for node in nodes),
        "skipped": sum(node["status"] == "skipped" for node in nodes),
        "nodes": sorted(nodes, key=lambda node: str(node["name"])),
    }


def unique_file(root: Path, name: str) -> Path:
    matches = sorted(root.rglob(name))
    if len(matches) != 1:
        raise RuntimeError(f"expected exactly one {name}, found {len(matches)}")
    return matches[0]


def validate_link(
    retained_root: Path,
    results: dict[str, object],
    binaries: dict[str, dict[str, object]],
) -> dict[str, object]:
    host_path = unique_file(retained_root, "host-resolution.json")
    frontend_path = unique_file(retained_root, "packaged-frontend.json")
    handshakes_path = unique_file(retained_root, "role-handshakes.json")
    host = json.loads(host_path.read_text(encoding="utf-8"))
    frontend = json.loads(frontend_path.read_text(encoding="utf-8"))
    handshakes = json.loads(handshakes_path.read_text(encoding="utf-8"))
    checks: list[dict[str, object]] = []

    def check(name: str, condition: bool, observed: object) -> None:
        checks.append({"name": name, "passed": bool(condition), "observed": observed})
        if not condition:
            raise AssertionError(f"evidence link failed: {name}: {observed}")

    check(
        "seven nodes passed without skip",
        results["pytestExitCode"] == 0
        and results["passed"] == 7
        and results["failed"] == 0
        and results["skipped"] == 0,
        {
            "exit": results["pytestExitCode"],
            "passed": results["passed"],
            "failed": results["failed"],
            "skipped": results["skipped"],
        },
    )
    host_sha = binaries["debugHost"]["sha256"]
    check(
        "host verifier executed the recorded debug host",
        host["artifact"]["innerExe"]["sha256"] == host_sha,
        host["artifact"]["innerExe"],
    )
    check(
        "frontend verifier executed the same debug host",
        frontend["artifact"]["innerExe"]["sha256"] == host_sha,
        frontend["artifact"]["innerExe"],
    )
    check(
        "all resolved roles carry the explicit PC v2.02 version",
        all(host["gameFileVersions"].get(role) == GAME_FILE_VERSION for role in ROLES),
        host["gameFileVersions"],
    )
    selected = frontend["handshake"]["selectedContext"]
    check(
        "frontend handshake selected PC v2.02",
        selected.get("gameFileVersion") == GAME_FILE_VERSION,
        selected,
    )
    check(
        "selected resource identity is complete",
        all(
            isinstance(selected.get(key), str) and selected[key]
            for key in (
                "versionedResourceDir",
                "bundleDigest",
                "versionedDigest",
                "contextDigest",
            )
        ),
        selected,
    )
    check(
        "three declared roles launched and handshook",
        set(handshakes) == set(ROLES)
        and all(handshakes[role]["handshake"]["role"] == role for role in ROLES),
        {role: handshakes.get(role, {}).get("handshake", {}).get("role") for role in ROLES},
    )
    expected_worker_hashes = {
        "offline_search": binaries["debugSearchWorker"]["sha256"],
        "save": binaries["debugProtectedWorker"]["sha256"],
        "runtime": binaries["debugProtectedWorker"]["sha256"],
    }
    check(
        "host role binaries match the freshly confirmed debug workers",
        all(
            host["artifact"]["roleBinaries"][role]["sha256"]
            == expected_worker_hashes[role]
            for role in ROLES
        ),
        {
            role: host["artifact"]["roleBinaries"][role]["sha256"]
            for role in ROLES
        },
    )
    check(
        "frontend role binaries match the freshly confirmed debug workers",
        all(
            frontend["artifact"]["roleBinaries"][role]["sha256"]
            == expected_worker_hashes[role]
            for role in ROLES
        ),
        {
            role: frontend["artifact"]["roleBinaries"][role]["sha256"]
            for role in ROLES
        },
    )
    isolation = frontend.get("fixtureWrites", {})
    check(
        "frontend writes stayed on the synthetic fixture",
        isolation.get("isolatedCopy") is True
        and isolation.get("realSaveTouched") is False,
        isolation,
    )
    check(
        "debug evidence is not labeled release candidate",
        host.get("developmentBuild") is True
        and host.get("releaseCandidate") is False
        and frontend.get("developmentBuild") is True
        and frontend.get("releaseCandidate") is False,
        {
            "host": [host.get("developmentBuild"), host.get("releaseCandidate")],
            "frontend": [
                frontend.get("developmentBuild"),
                frontend.get("releaseCandidate"),
            ],
        },
    )
    return {
        "schema": "nioh3-r5-evidence-link-validation/v1",
        "status": "PASS_TO_LOCAL_RC",
        "hostResult": host_path.relative_to(retained_root.parent).as_posix(),
        "frontendResult": frontend_path.relative_to(retained_root.parent).as_posix(),
        "roleHandshakes": handshakes_path.relative_to(retained_root.parent).as_posix(),
        "selectedContext": selected,
        "checks": checks,
        "boundaries": {
            "automatedDebugWebViewFunctionalAcceptance": True,
            "manualVisualAcceptance": False,
            "oneFileInstallOrLaunchAcceptance": False,
            "realGameAcceptance": False,
            "userSaveOpened": False,
            "releaseAuthorized": False,
        },
    }


def write_hash_manifest(package: Path) -> None:
    entries = []
    for path in sorted(package.rglob("*")):
        if path.is_file() and path.name != "SHA256SUMS.txt":
            entries.append(
                f"{sha256_file(path)}  {path.relative_to(package).as_posix()}"
            )
    (package / "SHA256SUMS.txt").write_text("\n".join(entries) + "\n", encoding="utf-8")


def create_zip(package: Path, archive: Path) -> None:
    if archive.exists():
        raise FileExistsError(f"refusing to overwrite archive: {archive}")
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for path in sorted(package.rglob("*")):
            if path.is_file():
                output.write(path, f"{package.name}/{path.relative_to(package).as_posix()}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--zip", dest="archive", type=Path, required=True)
    parser.add_argument("--review-root", type=Path)
    args = parser.parse_args()

    package = args.output.resolve()
    archive = args.archive.resolve()
    if package.exists():
        raise SystemExit(f"refusing to overwrite evidence directory: {package}")
    package.mkdir(parents=True)
    evidence = package / "evidence"
    retained = evidence / "retained-run"
    project_source = package / "project-source"
    evidence.mkdir()
    retained.mkdir()
    project_source.mkdir()

    run_id = f"r5-evidence-link-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    environment = dict(os.environ)
    environment["NIOH3_ACCEPTANCE_EVIDENCE_ROOT"] = str(retained)
    environment["NIOH3_ACCEPTANCE_RUN_ID"] = run_id
    environment["NIOH3_ACCEPTANCE_GAME_FILE_VERSION"] = GAME_FILE_VERSION
    environment["NIOH3_PYTHON"] = sys.executable
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    target_raw = environment.get("CARGO_TARGET_DIR", "").strip()
    if not target_raw:
        raise SystemExit("CARGO_TARGET_DIR is required; invoke through tools/run_python_tests.ps1")
    target = Path(target_raw).resolve()
    target.mkdir(parents=True, exist_ok=True)

    node = shutil.which("node") or shutil.which("node.exe")
    cargo = shutil.which("cargo") or shutil.which("cargo.exe")
    if node is None or cargo is None:
        raise SystemExit("Node.js and Cargo are required")

    build_steps = (
        ("frontend", [node, "apps/tauri/build.mjs"], 900),
        (
            "debug-host",
            [cargo, "build", "--locked", "--offline", "--manifest-path", "apps/tauri/src-tauri/Cargo.toml", "--bin", "nioh3-studio"],
            3600,
        ),
        (
            "debug-search-worker",
            [cargo, "build", "--locked", "--offline", "--manifest-path", "crates/nioh3-worker/Cargo.toml", "--bin", "nioh3-readonly-worker"],
            3600,
        ),
        (
            "debug-protected-worker",
            [cargo, "build", "--locked", "--offline", "--manifest-path", "crates/nioh3-protected/Cargo.toml", "--bin", "nioh3-protected-worker"],
            3600,
        ),
        (
            "release-host-fixture-prerequisite",
            [cargo, "build", "--locked", "--offline", "--release", "--manifest-path", "apps/tauri/src-tauri/Cargo.toml", "--bin", "nioh3-studio"],
            3600,
        ),
        (
            "release-launcher-fixture-prerequisite",
            [cargo, "build", "--locked", "--offline", "--release", "--manifest-path", "apps/launcher/Cargo.toml", "--bin", "Nioh3Launcher"],
            3600,
        ),
    )
    build_records = []
    for name, command, timeout in build_steps:
        record = run_recorded(name, command, evidence, environment, timeout=timeout)
        build_records.append(record)
        write_json(evidence / "build-record.json", {"steps": build_records})
        if record["exitCode"] != 0:
            raise SystemExit(f"build step failed: {name}; see {record['stderr']}")

    sources_before = source_identity_manifest()
    write_json(evidence / "source-identities.json", sources_before)
    binaries_before = binary_identities(target)
    write_json(evidence / "binary-identities-before.json", binaries_before)
    copy_frozen_sources(project_source)

    junit = evidence / "seven-node-results.xml"
    test_command = [
        sys.executable,
        "-m",
        "pytest",
        "-q",
        "-vv",
        f"--junitxml={junit}",
        str(TEST_PATH),
    ]
    test_record = run_recorded(
        "seven-node-pytest",
        test_command,
        evidence,
        environment,
        timeout=7200,
    )
    if not junit.is_file():
        raise SystemExit("pytest produced no JUnit record")
    test_results = parse_junit(junit, int(test_record["exitCode"]))
    test_results["command"] = test_record
    write_json(evidence / "seven-node-results.json", test_results)

    binaries_after = binary_identities(target)
    write_json(evidence / "binary-identities-after.json", binaries_after)
    if binaries_before != binaries_after:
        raise SystemExit("debug/release artifact identities changed during the seven-node run")
    sources_after = source_identity_manifest()
    if sources_before != sources_after:
        raise SystemExit("source identity changed during the seven-node run")

    link = validate_link(retained, test_results, binaries_after)
    write_json(evidence / "link-validation.json", link)

    head = checked_output(["git", "rev-parse", "HEAD"])
    git_status = checked_output(["git", "status", "--short"])
    (evidence / "git-status.txt").write_text(git_status + "\n", encoding="utf-8")
    tool_versions = {
        "python": sys.version,
        "node": checked_output([node, "--version"]),
        "cargo": checked_output([cargo, "--version"]),
        "rustc": checked_output([shutil.which("rustc") or "rustc", "--version"]),
    }
    environment_record = {
        "schema": "nioh3-r5-evidence-link-environment/v1",
        "runId": run_id,
        "createdAtUtc": utc_now(),
        "repository": {
            "root": str(ROOT),
            "head": head,
            "dirty": bool(git_status.strip()),
            "sourceIdentityDigest": sources_before["manifestDigest"],
            "sourceIdentityFiles": sources_before["fileCount"],
        },
        "platform": platform.platform(),
        "gameFileVersion": GAME_FILE_VERSION,
        "cargoTarget": str(target),
        "tools": tool_versions,
        "artifactProfiles": {
            "executedHost": "debug, default Cargo features",
            "executedWorkers": "debug, default Cargo features",
            "releaseArtifacts": "fixture prerequisites only; not executed by the final debug-host nodes",
        },
        "boundaries": link["boundaries"],
    }
    write_json(package / "ENVIRONMENT.json", environment_record)

    if args.review_root is not None:
        review_root = args.review_root.resolve()
        review_out = evidence / "pro-r5-review"
        review_out.mkdir()
        for name in (
            "README.md",
            "BOUNDED_FOLLOWUP.md",
            "CLOSURE_VERDICTS.md",
            "TEST_ORACLES.md",
            "verdicts.json",
        ):
            source = review_root / name
            if not source.is_file():
                raise FileNotFoundError(f"review evidence is missing: {source}")
            shutil.copy2(source, review_out / name)

    (package / "README.md").write_text(
        """# Nioh 3 Studio v0.8.0 r5 evidence-link closure

Read `TASK_FOR_PRO.md`, then `evidence/link-validation.json`,
`evidence/seven-node-results.json`, and the two retained verifier results below
`evidence/retained-run/nodes/`.

This package closes only `R5-EVIDENCE-LINK`. The prior Pro review already closed
RF02, RF03, and RF05. The run rebuilt the Tauri frontend, confirmed the exact
debug host and two debug worker binaries through Cargo, and ran only the seven
packaged-host resolver nodes. Every node has an explicit passed/failed/skipped
record and the temporary verifier JSON was retained before cleanup.

Evidence grade: automated Windows debug-host/WebView2 functional acceptance on
synthetic saves. It is not manual visual acceptance, one-file installation or
startup acceptance, real-game acceptance, or release authorization.
""",
        encoding="utf-8",
    )
    (package / "TASK_FOR_PRO.md").write_text(
        """# Bounded review task: R5-EVIDENCE-LINK only

Determine whether the retained evidence now connects one frozen source identity
to the exact debug host/workers, the two actual JavaScript verifiers, the
explicit PC `2.0.2.0` selected resource context, the three synthetic worker
roles, and the final seven-node result.

Do not reopen RF02, RF03, RF05, RF01, RF04, or contract idempotence without a
new concrete contradiction. Do not perform a global repository audit. Confirm
`PASS_TO_LOCAL_RC` if the link is complete; otherwise name the smallest exact
missing or contradictory field.

Required distinctions: automated debug WebView2 functional acceptance was run;
manual visual acceptance, one-file user installation/startup, real-game work,
and real-user-save work were not run. This task does not authorize publication
or PC v2.02 game writes.
""",
        encoding="utf-8",
    )

    summary = {
        "schema": "nioh3-r5-evidence-link-summary/v1",
        "runId": run_id,
        "status": link["status"],
        "sevenNodeResults": {
            "exitCode": test_results["pytestExitCode"],
            "passed": test_results["passed"],
            "failed": test_results["failed"],
            "skipped": test_results["skipped"],
        },
        "debugArtifacts": binaries_after,
        "selectedContext": link["selectedContext"],
        "sourceIdentityDigest": sources_before["manifestDigest"],
        "sourceStableDuringRun": True,
        "artifactStableDuringRun": True,
        "boundaries": link["boundaries"],
    }
    write_json(evidence / "RUN_SUMMARY.json", summary)
    write_hash_manifest(package)
    create_zip(package, archive)
    print(
        json.dumps(
            {
                "ok": True,
                "status": link["status"],
                "package": str(package),
                "archive": str(archive),
                "archiveBytes": archive.stat().st_size,
                "archiveSha256": sha256_file(archive),
                "sevenNodesPassed": test_results["passed"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
