"""The search worker bootstrap resolves the production identity by explicit version.

The production identity is the worker's own explicit game file version
(``--game-file-version <A.B.C.D>``), which ``main`` resolves through
``resolve_worker_context``.  These tests drive that production path directly:
the shipped worker is launched exactly as the host launches it, over the framed
protocol, and the handshake it publishes must carry the resolved v2.02 identity,
its selected-bundle proof fields, and its selected table set.  A launch that
names no version, a malformed spelling, or an unregistered version must stop
before any frame is written.

The discovery helper ``resolve_generation_tables`` still exists, but it is a
diagnostics convenience: it is never the production identity, and this file
proves the explicit-version channel instead of only re-driving that helper.
"""

from __future__ import annotations

import io
from pathlib import Path
import subprocess
import sys

from nioh3_scroll_editor.core_services import CoreErrorCode, CoreServiceError
from nioh3_scroll_editor.r4_finalizer_resource import (
    DEFAULT_RESOURCE_ROOT,
    load_default_r4_finalizer_resource,
)
from nioh3_scroll_editor.search_worker import (
    parse_options,
    resolve_generation_tables,
    resolve_worker_context,
)
from nioh3_scroll_editor.worker_transport import read_frame, write_frame


ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"

V202 = (2, 0, 2, 0)
V20002 = (2, 0, 0, 2)
V202_RESOURCE_DIR = "r4_finalizer/pc_v2_02/resource_v1"

# Every field a production handshake publishes: the shared legacy-compatible
# keys first, then the version-bound authority and its separate proof fields.
PRODUCTION_CONTEXT_FIELDS = {
    "product_version",
    "game_profile",
    "resources_digest",
    "algorithm_version",
    "policy_version",
    "seed_accelerator_abi",
    "seed_accelerator_build_id",
    "context_digest",
    "game_file_version",
    "versioned_resource_dir",
    "bundle_digest",
    "versioned_digest",
    "legacy_context_digest",
    "production_authority",
}


def _launch(*arguments: str, frames: list[tuple[str, dict]] | None = None):
    """Launch the shipped worker exactly as the host does, over framed JSON."""

    payload = b""
    if frames:
        stream = io.BytesIO()
        for index, (method, params) in enumerate(frames):
            write_frame(
                stream,
                {"protocol": 1, "id": str(index), "method": method, "params": params},
            )
        payload = stream.getvalue()
    return subprocess.run(
        [sys.executable, "-m", "nioh3_scroll_editor.search_worker", *arguments],
        input=payload,
        capture_output=True,
        cwd=ROOT,
        timeout=180,
    )


def _production_handshake_context() -> dict:
    """The context the real explicit-version production handshake publishes."""

    completed = _launch("--game-file-version", "2.0.2.0", frames=[("handshake", {})])
    assert completed.returncode == 0, completed.stderr.decode("utf-8", "replace")
    response = read_frame(io.BytesIO(completed.stdout))
    assert response is not None and response.get("ok") is True, response
    return response["result"]["context"]


def test_production_handshake_publishes_the_resolved_v202_identity() -> None:
    """The real explicit-version launch, not the diagnostics helper, is identity."""

    context = _production_handshake_context()

    assert set(context) == PRODUCTION_CONTEXT_FIELDS
    assert context["production_authority"] is True
    assert context["game_file_version"] == "2.0.2.0"
    assert context["versioned_resource_dir"] == V202_RESOURCE_DIR
    for field in (
        "bundle_digest",
        "versioned_digest",
        "context_digest",
        "legacy_context_digest",
    ):
        assert len(context[field]) == 64, field
        int(context[field], 16)
    # The version-bound authority is not the pre-version proof digest, and the
    # proof digest is carried beside it rather than standing in for it.
    assert context["context_digest"] != context["legacy_context_digest"]


def test_production_resolver_derives_the_v202_bundle_and_tables() -> None:
    """One explicit version selects one bundle and one table set."""

    context, tables = resolve_worker_context(V202)

    assert context.game_file_version == V202
    assert context.dotted_game_file_version == "2.0.2.0"
    assert context.versioned_resource_dir == V202_RESOURCE_DIR
    assert context.to_payload()["production_authority"] is True
    assert tables is not None
    assert tables.resource.root == DATA_ROOT / V202_RESOURCE_DIR
    assert tables.resource.manifest["game_version"] == "PC v2.02"
    assert len(tables.optional_multipliers_by_key) == 2954
    assert 0xD56F in tables.optional_multipliers_by_key
    assert tables.resource.root != DEFAULT_RESOURCE_ROOT


def test_explicit_version_is_the_only_identity_selector() -> None:
    """`parse_options` returns the exact version tuple, or refuses."""

    assert parse_options(["--game-file-version", "2.0.2.0"]) == V202
    assert parse_options(["--game-file-version", "2.0.0.2"]) == V20002
    assert parse_options(["--legacy-test-context"]) == "legacy"
    assert parse_options(["--help"]) is None


def test_each_registered_version_resolves_its_own_tables() -> None:
    """One explicit version selects one versioned resource, never a shared one."""

    _, v202_tables = resolve_worker_context(V202)
    _, baseline_tables = resolve_worker_context(V20002)
    assert v202_tables.resource.root != baseline_tables.resource.root
    assert v202_tables.resource.manifest["game_version"] == "PC v2.02"
    assert baseline_tables.resource.manifest["game_version"] == "PC v2.00.02"
    assert len(v202_tables.optional_multipliers_by_key) == 2954
    assert len(baseline_tables.optional_multipliers_by_key) == 2951


def test_production_launch_fails_closed_without_a_resolvable_version() -> None:
    """Missing, malformed and unregistered versions all stop before any frame."""

    # No flag at all: a usage refusal, and nothing on stdout.
    missing = _launch()
    assert missing.returncode == 2
    assert missing.stdout == b""
    assert b"--game-file-version" in missing.stderr

    # A malformed spelling is refused the same way rather than coerced.
    malformed = _launch("--game-file-version", "2.0.2")
    assert malformed.returncode == 2
    assert malformed.stdout == b""
    assert b"four-part" in malformed.stderr

    # A well-formed but unregistered version parses, then fails closed with the
    # shipped RESOURCE_MISMATCH code and no fallback to the shipped baseline.
    for version in ("9.9.9.9", "2.0.3.0"):
        unregistered = _launch("--game-file-version", version)
        assert unregistered.returncode == 1, unregistered.stderr.decode("utf-8", "replace")
        assert unregistered.stdout == b""
        assert b"RESOURCE_MISMATCH" in unregistered.stderr


def test_unregistered_version_fails_closed_without_a_default() -> None:
    """An unregistered version must not fall back to the shipped baseline."""

    for version in ((9, 9, 9, 9), (2, 0, 3, 0)):
        try:
            resolve_worker_context(version)
        except CoreServiceError as error:
            assert error.code is CoreErrorCode.RESOURCE_MISMATCH
        else:  # pragma: no cover - a silent fallback is the defect this guards
            raise AssertionError(f"{version} must fail closed, not resolve")


def test_discovery_is_not_the_production_identity_source() -> None:
    """The diagnostics helper never substitutes for an explicit version.

    Discovery may return the installed build's tables; the production launch
    must still refuse when no version is named, so a discoverable install can
    never silently select the identity a job runs under.  The helper is probed
    only as a control for that refusal.
    """

    resolve_generation_tables()
    refused = _launch()
    assert refused.returncode == 2
    assert refused.stdout == b""
    assert b"requires" in refused.stderr


def test_default_loader_is_not_mutated_by_the_explicit_identity() -> None:
    """Resolving a versioned identity must not disturb the shipped baseline."""

    resolve_worker_context(V202)
    bundled = load_default_r4_finalizer_resource()
    assert bundled.root == DEFAULT_RESOURCE_ROOT
    assert bundled.manifest["game_version"] == "PC v2.00.02"
    assert bundled.table("optional_multiplier").row_count == 2951
