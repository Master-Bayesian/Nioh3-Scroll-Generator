"""Stage built Rust worker binaries under their packaged names.

This stages the shipped packaged graph. It does not build anything: it maps the
cargo outputs of `crates/nioh3-worker` and `crates/nioh3-protected` onto the two
names the broker already resolves in a packaged host
(`worker/nioh3-search-worker.exe`, `worker/nioh3-protected-worker.exe`) and
writes `worker-backend.json`, the explicit manifest the packaged host reads for
its worker graph.

The PyInstaller worker is still built and kept for development, parity and the
legacy Tk path, so this tool refuses to stage a workers directory that still
contains PyInstaller output; that keeps a package from silently mixing both
graphs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROLES = (
    ("nioh3-readonly-worker.exe", "nioh3-search-worker.exe", "nioh3-worker"),
    ("nioh3-protected-worker.exe", "nioh3-protected-worker.exe", "nioh3-protected"),
)

# The exact argument vectors a packaged host must pass once it selects the Rust
# backends. Both Rust hosts resolve their resource roots from arguments only
# (never the working directory; confirmed by the protected owner), and
# `application_root` is derived by the worker from `data_root.parent().parent()`,
# which is what places `bin/` beside the staged generation tables.
#
# The read-only worker now has an explicit packaged launch mode
# (`--packaged-worker`), so this contract is not a development invocation
# dressed up as one; the development acknowledgement stays the only shape the
# development environment variable accepts.
INVOCATION = {
    "offline_search": {
        "mode": "packaged",
        "binary": "nioh3-search-worker.exe",
        "argv": [
            "--packaged-worker",
            "--data-root",
            "<runtime>/worker/runtime/nioh3_scroll_editor/data",
            "--contract-dir",
            "<runtime>/packages/contracts",
        ],
    },
    "save": {
        "mode": "packaged",
        "binary": "nioh3-protected-worker.exe",
        "argv": [
            "--role",
            "save",
            "--state-root",
            "<state root>",
            "--data-root",
            "<runtime>/worker/runtime/nioh3_scroll_editor/data",
            "--contract-dir",
            "<runtime>/packages/contracts",
        ],
    },
    "runtime": {
        "mode": "packaged",
        "binary": "nioh3-protected-worker.exe",
        "argv": [
            "--role",
            "runtime",
            "--state-root",
            "<state root>",
            "--data-root",
            "<runtime>/worker/runtime/nioh3_scroll_editor/data",
            "--contract-dir",
            "<runtime>/packages/contracts",
        ],
    },
}

# The build/host mode contract a packaged Rust runtime must satisfy. The identity
# acceptance re-derives every one of these independently instead of trusting the
# manifest, so a manifest cannot self-assert a successful launch.
LAUNCH_CONTRACT = {
    "schema": "nioh3-worker-launch-contract/v1",
    "roles": ["offline_search", "save", "runtime"],
    "roleBinaries": {
        "offline_search": "nioh3-search-worker.exe",
        "save": "nioh3-protected-worker.exe",
        "runtime": "nioh3-protected-worker.exe",
    },
    "allowedFlags": [
        "--packaged-worker",
        "--dev-preview-only",
        "--data-root",
        "--contract-dir",
        "--state-root",
        "--role",
        "--accelerator",
    ],
    "requiredFlags": {
        "offline_search": ["--packaged-worker"],
        "save": [],
        "runtime": [],
    },
    "flagValues": ["--data-root", "--contract-dir", "--state-root", "--role", "--accelerator"],
    "requiredResources": [
        "worker/runtime/nioh3_scroll_editor/data",
        "packages/contracts",
        "worker/runtime/bin/nioh3_seed_accelerator.dll",
        "worker/runtime/bin/nioh3_effect_preimage_accelerator.dll",
    ],
    "contractSchemas": ["request.schema.json", "response.schema.json"],
    "protectedSchemas": ["protected-request.schema.json", "protected-response.schema.json"],
    # Binaries, contracts and generation tables are package-confined; user state
    # never is. The broker owns the state root and passes it at launch; the
    # manifest may only carry the placeholder, so it can never choose a write
    # path, and the state cannot live inside the extracted one-file cache.
    "stateRoot": {
        "policy": "broker-injected-external",
        "placeholder": "<state root>",
        "userLocation": "%LOCALAPPDATA%/Nioh3ScrollGenerator",
        "persists": ["settings pointer", "v2-operations transaction records", "restart/backup state"],
        "packageConfined": False,
    },
    "representativeRequests": {
        "offline_search": {"method": "candidate.preview", "params": {"seed": 1, "rarity": 3, "level": 180}},
        "save": {"method": "save.discover", "params": {}},
        "runtime": {"method": "runtime.status", "params": {}},
    },
    "contractDigests": {},
}


def _digest_pair(root: Path, names: tuple[str, str]) -> str:
    """SHA-256 of the two schema files in order, mirroring the workers."""

    digest = hashlib.sha256()
    for name in names:
        path = root / "packages" / "contracts" / name
        if not path.is_file():
            raise SystemExit(f"contract schema is missing: {path}")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def contract_digests(root: Path) -> dict[str, str]:
    """The contract digest each role set must reproduce from its package."""

    return {
        "offline_search": _digest_pair(
            root, ("request.schema.json", "response.schema.json")
        ),
        "protected": _digest_pair(
            root, ("protected-request.schema.json", "protected-response.schema.json")
        ),
    }

DECRYPTOR = {
    "path": "bin/Nioh_Savefile_decrypt.exe",
    "shippedInRustGraph": False,
    "decision": "oracle_only",
    "evidence": [
        "crates/nioh3-protected/src/save_app.rs uses nioh3_save modules (native codec)",
        "no std::process::Command in crates/nioh3-save or crates/nioh3-protected",
        "executing consumers are Python-side: nioh3_scroll_editor/savegame.py, packaging/*.spec, research scripts",
        "protected owner confirmed the Rust host needs no external decryptor (pure-Rust codec)",
        "the M3-b save acceptance records that the Rust save path makes no external-process call",
    ],
    "openItem": "none - the pure-Rust codec is the product path and the decryptor stays a test oracle",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


# The worker crates are standalone workspaces, so each ships its own lockfile.
# Recording them, with the compiler and the exact feature set, is what makes the
# packaged worker build reproducible instead of merely described.
WORKER_CRATES = ("nioh3-worker", "nioh3-protected")
PRODUCTION_FEATURES: tuple[str, ...] = ()
# Development-only features that exist anywhere in the built graph - the two
# worker crates and their `nioh3-runtime` dependency - and that a packaged
# binary must therefore be able to prove it did *not* enable.
EXCLUDED_FEATURES = ("test-fake", "test-helper")


def build_commands(profile: str) -> list[list[str]]:
    """The exact cargo invocations that produce the two staged worker binaries.

    Mirrors the `-WorkerBackend rust` branch of `tools/build_tauri.ps1`. The
    command line is recorded beside the feature set so the "no development
    feature was compiled in" claim is anchored to what actually ran, not to a
    comment: neither command passes `--features` or `--all-features`.
    """

    profile_flag = "--release" if profile == "release" else f"--profile={profile}"
    return [
        [
            "cargo",
            "build",
            profile_flag,
            "--locked",
            "--manifest-path",
            "crates/nioh3-worker/Cargo.toml",
            "--bin",
            "nioh3-readonly-worker",
        ],
        [
            "cargo",
            "build",
            profile_flag,
            "--locked",
            "--manifest-path",
            "crates/nioh3-protected/Cargo.toml",
            "--bin",
            "nioh3-protected-worker",
        ],
    ]


def tool_version(program: str) -> str | None:
    """The compiler's own version string, or None when it is unavailable."""

    try:
        return subprocess.check_output(
            [program, "--version"], text=True, encoding="utf-8", stderr=subprocess.DEVNULL
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def build_environment(root: Path, profile: str) -> dict:
    """The worker build provenance the package carries in `workerBuildEnvironment`."""

    locks = {}
    for crate in WORKER_CRATES:
        lock = root / "crates" / crate / "Cargo.lock"
        if lock.is_file():
            locks[f"crates/{crate}/Cargo.lock"] = sha256(lock)
    git = lambda *args: subprocess.check_output(
        ["git", *args], cwd=root, text=True, encoding="utf-8"
    ).strip()
    return {
        "cargo": tool_version("cargo"),
        "rustc": tool_version("rustc"),
        "target": "x86_64-pc-windows-msvc",
        "profile": profile,
        "buildCommands": build_commands(profile),
        "features": list(PRODUCTION_FEATURES),
        "excludedFeatures": list(EXCLUDED_FEATURES),
        "locks": locks,
        "sourceCommit": git("rev-parse", "HEAD"),
        "sourceDirty": bool(git("status", "--porcelain")),
    }


def stage(
    binaries: Path, workers: Path, profile: str = "release", root: Path | None = None
) -> dict:
    """Copy the two cargo binaries into the packaged names and write the manifest."""

    source_dir = binaries / profile
    if not source_dir.is_dir():
        raise SystemExit(f"cargo output directory is missing: {source_dir}")
    workers.mkdir(parents=True, exist_ok=True)
    if (workers / "python-build-environment.json").is_file() or any(
        path.is_dir() for path in workers.glob("*/_internal")
    ):
        raise SystemExit(
            "refusing to stage Rust workers into a directory that still holds "
            "PyInstaller output; a package must not mix both backends"
        )
    binaries_manifest = []
    for cargo_name, packaged_name, crate in ROLES:
        source = source_dir / cargo_name
        if not source.is_file():
            raise SystemExit(f"missing Rust worker binary: {source}")
        destination = workers / packaged_name
        shutil.copy2(source, destination)
        binaries_manifest.append(
            {
                "crate": crate,
                "cargoName": cargo_name,
                "packagedName": packaged_name,
                "sha256": sha256(destination),
                "size": destination.stat().st_size,
            }
        )
    manifest = {
        "schema": "nioh3-worker-backend/v1",
        "backend": "rust",
        "profile": profile,
        "defaultGraph": "rust",
        "pythonGraph": "development-parity-and-legacy-tk-only",
        "defaultGraphDecision": (
            "owner-authorized local default backend switch: the packaged product "
            "selects the staged Rust graph from this manifest. The legacy Python "
            "worker remains in the tree as a development, parity and oracle "
            "backend and is no longer part of the shipped graph. The protected "
            "G5-G7 gates and every live-game flow stay open and are not claimed "
            "by this manifest."
        ),
        "binaries": binaries_manifest,
        "invocation": INVOCATION,
        "launchContract": LAUNCH_CONTRACT,
        "excludedProductionResources": [
            "packaging/search-worker.spec",
            "packaging/protected-worker.spec",
            "launch_search_worker.py",
            "launch_protected_worker.py",
            "python-build-environment.json",
        ],
        "decryptor": DECRYPTOR,
    }
    if root is not None:
        manifest["buildEnvironment"] = build_environment(root, profile)
    if root is not None:
        digests = contract_digests(root)
        manifest["launchContract"] = dict(LAUNCH_CONTRACT)
        manifest["launchContract"]["contractDigests"] = digests
    # The packager copies `licenses/` verbatim. The Python distribution notices
    # the shipped graph bundles do not exist in this graph, so the directory
    # carries an explicit notice instead of being silently absent.
    notices = workers / "licenses"
    notices.mkdir(parents=True, exist_ok=True)
    (notices / "NO-PYTHON-RUNTIME.txt").write_text(
        "This runtime ships no Python interpreter, no PyInstaller worker "
        "and no Python distribution notices. The Rust crate licences are "
        "recorded in dependency-manifest.json; the save codec's MIT provenance "
        "(third_party/nioh_savefile_decrypt) is recorded as "
        "licenses/Nioh-Savedata-Decryption-Tool-LICENSE.\n",
        encoding="utf-8",
    )
    (workers / "worker-backend.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binaries", type=Path, required=True)
    parser.add_argument("--workers", type=Path, required=True)
    parser.add_argument("--profile", default="release", choices=("release", "debug"))
    parser.add_argument(
        "--root",
        type=Path,
        help=(
            "repository root whose contract schemas, lockfiles and commit the "
            "manifest records (default: this checkout)"
        ),
    )
    args = parser.parse_args()
    root = args.root or Path(__file__).resolve().parents[1]
    manifest = stage(args.binaries, args.workers, args.profile, root)
    print(
        json.dumps(
            {
                "backend": manifest["backend"],
                "workers": [entry["packagedName"] for entry in manifest["binaries"]],
                "graph": "shipped-packaged",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
