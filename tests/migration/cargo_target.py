"""The one build/test root resolver for the migration gates and the test runner.

Two decisions live here and nowhere else, because a second copy of them is the
bug `tests/migration/test_cargo_target_defaults.py` refuses:

1. Where the project's build/test root is. An explicit `NIOH3_BUILD_ROOT`
   always wins. Otherwise a local Windows host with a D: drive builds on
   `D:/Nioh3_v080_deliverables` — the delivery volume the existing local gates
   already use — so neither the checkout volume nor the `C:` system temp takes
   the write. CI, non-Windows hosts and Windows hosts without a D: drive keep
   the platform temp directory they already build in. A local host never falls
   back to `C:` `%TEMP%` silently: an unusable D-backed root raises, so the
   operator picks the volume with `NIOH3_BUILD_ROOT`.
2. Where one gate's Cargo target is. `CARGO_TARGET_DIR` always wins, so an
   operator or CI can point every gate at a volume with room and share one
   cache; otherwise it is `<build root>/build-cache/<name>` on the D-backed
   root, or `<platform temp>/nioh3-<name>-target` in the portable fallback. A
   gate must never implicitly write a fresh target tree into the checkout,
   whose volume can be full (the disk-full failure this helper exists to
   prevent).

`tools/run_python_tests.ps1` mirrors decision 1 for `TEMP`/`TMP`, and
`tests/migration/test_build_root_policy.py` drives both resolvers with the same
inputs so the two cannot drift apart.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path


BUILD_ROOT_ENV = "NIOH3_BUILD_ROOT"
CARGO_TARGET_ENV = "CARGO_TARGET_DIR"
CI_ENV_VARS = ("CI", "GITHUB_ACTIONS")

# One root for Cargo targets and test temp on this host, not a second scheme.
LOCAL_BUILD_ROOT = Path("D:/Nioh3_v080_deliverables")
CARGO_CACHE_DIRNAME = "build-cache"
TEMP_DIRNAME = "tmp"
# The one Cargo target `tools/run_python_tests.ps1` shares across a pytest run.
PYTHON_TEST_CACHE_NAME = "python-tests"


def _platform_temp_root() -> Path:
    """The portable fallback root every other host already builds in."""

    return Path(tempfile.gettempdir())


def _host_is_local_windows(local_root: Path) -> bool:
    """True when this host must build on the local D: root, not the temp dir.

    CI runners keep their own temp directory even when the machine happens to
    have a D: drive, and a Windows host without that drive stays portable, so
    neither is broken by the local rule.
    """

    if os.name != "nt":
        return False
    if any(os.environ.get(name, "").strip() for name in CI_ENV_VARS):
        return False
    anchor = local_root.anchor
    return bool(anchor) and Path(anchor).exists()


def _resolve_root() -> tuple[Path, bool]:
    """The build/test root plus whether it is the host's dedicated root.

    A dedicated root owns `<root>/tmp` for test temp; the portable fallback *is*
    the platform temp directory, so nothing nests a redundant `tmp` under it.
    """

    configured = os.environ.get(BUILD_ROOT_ENV, "").strip()
    if configured:
        return Path(configured), True
    if _host_is_local_windows(LOCAL_BUILD_ROOT):
        return LOCAL_BUILD_ROOT, True
    return _platform_temp_root(), False


def _created_directory(path: Path) -> Path:
    try:
        path.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise RuntimeError(
            f"Cannot create the project build directory {path}: {error}. "
            f"Point {BUILD_ROOT_ENV} (or {CARGO_TARGET_ENV}) at a writable directory; "
            "falling back to the C: system temp is what this root exists to prevent."
        ) from error
    return path


def build_root() -> Path:
    """The one build/test root for this host, created when it is missing."""

    root, _ = _resolve_root()
    return _created_directory(root)


def temp_root() -> Path:
    """The directory that holds this project's test temp, created if missing."""

    root, dedicated = _resolve_root()
    return _created_directory((root / TEMP_DIRNAME) if dedicated else root)


def cargo_target_dir(name: str = "migration") -> Path:
    """The cargo target directory one gate must build into, created if missing."""

    configured = os.environ.get(CARGO_TARGET_ENV, "").strip()
    if configured:
        return _created_directory(Path(configured))
    root, dedicated = _resolve_root()
    if dedicated:
        return _created_directory(root / CARGO_CACHE_DIRNAME / name)
    return _created_directory(root / f"nioh3-{name}-target")


def resolved_cargo_target_dir(name: str = "migration") -> str:
    """The cargo target directory one gate must build into."""

    return str(cargo_target_dir(name))
