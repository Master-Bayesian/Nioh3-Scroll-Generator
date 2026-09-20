"""Executable regression for the migration gates' Cargo target defaults.

The failure this locks down: a parity gate defaulted its cargo target to a path
under the repository, the repository volume filled up, and the gates failed at
setup with "There is not enough space on the disk" instead of reporting their
real result.

The rule: `CARGO_TARGET_DIR` always wins, so an operator or CI can point the
build at any volume; otherwise the target lives under the project build root,
which is `D:/Nioh3_v080_deliverables/build-cache/<gate>` on a local Windows host
and the platform temp directory on CI or a non-Windows host. No migration gate
may default a cargo target into the checkout or into the `C:` system temp on
this host. `tests/migration/test_build_root_policy.py` owns the root policy.
"""
from __future__ import annotations

import os
import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MIGRATION = ROOT / "tests" / "migration"
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.migration import cargo_target  # noqa: E402
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402


def migration_gate_files() -> list[Path]:
    return sorted(
        path
        for path in MIGRATION.glob("test_*.py")
        if path.is_file() and path.name != Path(__file__).name
    )


def test_an_explicit_cargo_target_dir_always_wins(monkeypatch) -> None:
    explicit = str(Path(tempfile.gettempdir()) / "nioh3-explicit-override-probe")
    monkeypatch.setenv("CARGO_TARGET_DIR", explicit)
    assert resolved_cargo_target_dir("probe") == explicit

    # A blank value is not an override; the external fallback must still apply.
    monkeypatch.setenv("CARGO_TARGET_DIR", "   ")
    assert resolved_cargo_target_dir("probe") != "   "


def test_the_portable_fallback_is_external_and_named_per_gate(monkeypatch) -> None:
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.delenv("NIOH3_BUILD_ROOT", raising=False)
    monkeypatch.setattr(cargo_target, "_host_is_local_windows", lambda local_root: False)
    fallback = Path(resolved_cargo_target_dir("unit-probe"))
    assert fallback == Path(tempfile.gettempdir()) / "nioh3-unit-probe-target"
    assert fallback.is_absolute()
    assert ROOT not in fallback.parents, "a gate must never default into the checkout"
    assert "codex_tmp" not in str(fallback)


def test_no_migration_gate_defaults_a_cargo_target_into_the_repository() -> None:
    offenders = []
    for path in migration_gate_files():
        text = path.read_text(encoding="utf-8")
        if "CARGO_TARGET_DIR" not in text:
            continue
        # Only a gate that actually launches cargo is subject to this rule. A
        # gate that merely asserts on a workflow file names the variable inside
        # an expected-command string and builds nothing.
        if "subprocess" not in text:
            continue
        for line_number, line in enumerate(text.splitlines(), start=1):
            if "CARGO_TARGET_DIR" not in line:
                continue
            if re.search(r"ROOT\s*/", line) or ".codex_tmp" in line:
                offenders.append(f"{path.name}:{line_number}: {line.strip()}")
    assert offenders == [], "a cargo target default must not live in the repository: " + "; ".join(
        offenders
    )


def test_every_gate_that_builds_cargo_resolves_through_shared_or_external_temp() -> None:
    """One behaviour, wherever the code lives: override wins, fallback is temp.

    The shared `cargo_target.resolved_cargo_target_dir` is the expected form. A
    gate that keeps its own resolver is still acceptable only while that
    resolver's fallback is the platform temp directory, never the checkout.
    """

    weak = []
    for path in migration_gate_files():
        text = path.read_text(encoding="utf-8")
        if "CARGO_TARGET_DIR" not in text:
            continue
        # Same scope rule as above: the subject is a gate that builds cargo.
        if "subprocess" not in text:
            continue
        # Compliant forms: the shared module, a delegation to another gate's
        # shared resolver, or a local resolver whose fallback is the temp dir.
        if "tests.migration.cargo_target" in text:
            continue
        if "resolved_cargo_target_dir" in text:
            continue
        if "def " in text and "gettempdir" in text:
            continue
        weak.append(path.name)
    assert weak == [], (
        "these gates set CARGO_TARGET_DIR without a shared or external-temp resolver: "
        + ", ".join(weak)
    )


def test_the_shared_resolver_is_one_module_used_by_the_migrated_gates() -> None:
    assert os.path.isfile(MIGRATION / "cargo_target.py")
    consumers = [
        path.name
        for path in migration_gate_files()
        if "tests.migration.cargo_target" in path.read_text(encoding="utf-8")
    ]
    expected = {
        "test_application_worker_parity.py",
        "test_effect_parity.py",
        "test_effect_resource_adapter_parity.py",
        "test_enemy_parity.py",
        "test_preview_worker_parity.py",
        "test_preview_parity.py",
        "test_r4_finalizer_parity.py",
        "test_rng_parity.py",
        "test_save_read_parity.py",
        "test_save_transaction_parity.py",
        "test_search_worker_parity.py",
        "test_runtime_live_add_parity.py",
        "test_runtime_mutation_parity.py",
        "test_runtime_read_parity.py",
    }
    assert set(consumers) >= expected, (
        "these gates must import the shared resolver: " + ", ".join(sorted(expected - set(consumers)))
    )
