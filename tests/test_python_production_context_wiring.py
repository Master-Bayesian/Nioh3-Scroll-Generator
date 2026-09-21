"""Production identity wiring for the Python search-worker entry point.

The worker now takes the same explicit exact game file version the Rust worker
takes, derives its context and generation tables from that one version, and
refuses to start without it. These tests are the cross-role gate for that wiring:
they pin the v2.00.02/v2.02 goldens through the worker's own entry point, prove
the pre-version digest is proof-only, and prove the silent-fallback paths closed.
"""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import unittest

from nioh3_scroll_editor.core_services import CandidateApplicationService, CoreErrorCode
from nioh3_scroll_editor.effect_generation_tables import load_default_effect_generation_tables
from nioh3_scroll_editor.resolved_context import (
    LegacyGenerationContext,
    ResolvedGenerationContext,
    capture_legacy_context,
)
from nioh3_scroll_editor.search_jobs import SearchJobs
from nioh3_scroll_editor.search_worker import (
    WorkerStartupError,
    parse_file_version_argument,
    parse_options,
    resolve_worker_context,
)


ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
V202 = (2, 0, 2, 0)
V20002 = (2, 0, 0, 2)

# Pinned for the v0.8.0 product identity and re-asserted on the Python side in
# tests/test_resolved_context.py.
V202_CONTEXT_DIGEST = "6f1292895f25937005f736b3170ccfd11b295aa7c284d3744339f6bbfd1a8712"
V20002_CONTEXT_DIGEST = "d866b3445427d264dfd58b4da29681075d8b1d8d227c67c3b4e409bc10f1174c"
SHARED_LEGACY_DIGEST = "4a38a6d3d14b3a2c24bbb946c9595d30b1098662b07299af2b617e927052d61f"

# The pinned goldens were captured with no accelerator; this host may have the
# DLL loaded, so the goldens are only meaningful for the accelerator-free
# identity. Structural tests exercise the host identity instead.
NO_ACCELERATOR = (None, None)

SEED = 10030700


def _run_worker_cli(*arguments: str) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-m", "nioh3_scroll_editor.search_worker", *arguments],
        cwd=ROOT,
        capture_output=True,
        timeout=180,
    )


def _context_for(file_version: tuple[int, int, int, int]) -> ResolvedGenerationContext:
    return ResolvedGenerationContext.capture(
        file_version=file_version,
        accelerator=NO_ACCELERATOR,
        data_root=DATA_ROOT,
    )


def _assert_startup_refusal(test: unittest.TestCase, *arguments: str) -> None:
    completed = _run_worker_cli(*arguments)
    # A usage-level refusal exits 2; an identity that parses but cannot resolve
    # exits 1 with the shipped RESOURCE_MISMATCH code. Both stop the process
    # before a frame is written, and both name the refusal on stderr.
    if completed.returncode == 2:
        test.assertNotEqual(completed.stderr, b"", "refusal must be explained")
    else:
        test.assertEqual(
            completed.returncode,
            1,
            f"unexpected exit; stdout={completed.stdout!r} stderr={completed.stderr!r}",
        )
        test.assertIn(b"RESOURCE_MISMATCH", completed.stderr)
    test.assertEqual(completed.stdout, b"", "no frame may be written before identity resolves")


class VersionArgumentTests(unittest.TestCase):
    def test_four_part_version_parses_and_other_spellings_are_refused(self) -> None:
        self.assertEqual(parse_file_version_argument("2.0.2.0"), (2, 0, 2, 0))

        for spelling in ("2.0.2", "2.0.2.0.1", "2.0.2.0.0", "2.0.x.0", "2.0..0", "-2.0"):
            with self.subTest(spelling=spelling):
                with self.assertRaises(WorkerStartupError):
                    parse_file_version_argument(spelling)

    def test_version_part_outside_sixteen_bits_is_refused(self) -> None:
        with self.assertRaises(WorkerStartupError):
            parse_file_version_argument("2.0.2.65536")

    def test_missing_version_is_a_refusal_not_a_default(self) -> None:
        with self.assertRaises(WorkerStartupError) as raised:
            parse_options([])
        self.assertIn("--game-file-version", str(raised.exception))

    def test_version_and_legacy_opt_in_are_mutually_exclusive(self) -> None:
        with self.assertRaises(WorkerStartupError) as raised:
            parse_options(["--game-file-version", "2.0.2.0", "--legacy-test-context"])
        self.assertIn("not both", str(raised.exception))

    def test_help_is_not_a_startup_failure(self) -> None:
        self.assertIsNone(parse_options(["--help"]))

    def test_unknown_argument_is_refused(self) -> None:
        with self.assertRaises(WorkerStartupError):
            parse_options(["--game-file-version", "2.0.2.0", "--accelerator", "x"])


class ProductionResolutionTests(unittest.TestCase):
    def test_resolution_derives_context_and_tables_from_one_version(self) -> None:
        context, tables = resolve_worker_context(V202)

        self.assertIsInstance(context, ResolvedGenerationContext)
        self.assertNotIsInstance(context, LegacyGenerationContext)
        self.assertEqual(context.dotted_game_file_version, "2.0.2.0")
        self.assertEqual(context.versioned_resource_dir, "r4_finalizer/pc_v2_02/resource_v1")
        self.assertIsNotNone(tables)
        self.assertIsNot(tables.resource, load_default_effect_generation_tables().resource)
        self.assertEqual(len(tables.optional_multipliers_by_key), 2954)

    def test_v20002_resolution_selects_the_shipped_baseline_payload(self) -> None:
        context, tables = resolve_worker_context(V20002)

        self.assertEqual(context.dotted_game_file_version, "2.0.0.2")
        self.assertEqual(context.versioned_resource_dir, "r4_finalizer/pc_v2_00_02/resource_v1")
        self.assertEqual(len(tables.optional_multipliers_by_key), 2951)

    def test_unknown_version_is_refused_before_any_work(self) -> None:
        for version in ((2, 0, 9, 9), (3, 0, 0, 0), (0, 0, 0, 0)):
            with self.subTest(version=version):
                with self.assertRaises(Exception) as raised:
                    resolve_worker_context(version)
                self.assertEqual(
                    getattr(raised.exception, "code", None),
                    CoreErrorCode.RESOURCE_MISMATCH,
                )

    def test_legacy_opt_in_is_the_only_non_production_resolution(self) -> None:
        context, tables = resolve_worker_context("legacy")

        self.assertIsInstance(context, LegacyGenerationContext)
        self.assertIsNone(tables)
        self.assertFalse(context.to_payload()["production_authority"])


class GoldenIdentityTests(unittest.TestCase):
    def test_worker_context_matches_both_pinned_version_goldens(self) -> None:
        v202 = _context_for(V202)
        v20002 = _context_for(V20002)

        self.assertEqual(v202.context_digest, V202_CONTEXT_DIGEST)
        self.assertEqual(v20002.context_digest, V20002_CONTEXT_DIGEST)
        self.assertNotEqual(v202.context_digest, v20002.context_digest)
        self.assertEqual(v202.legacy_context_digest, v20002.legacy_context_digest)
        self.assertEqual(v202.legacy_context_digest, SHARED_LEGACY_DIGEST)

    def test_handshake_payload_publishes_the_proof_fields(self) -> None:
        payload = _context_for(V202).to_payload()

        self.assertIs(payload["production_authority"], True)
        self.assertEqual(payload["game_file_version"], "2.0.2.0")
        self.assertEqual(payload["versioned_resource_dir"], "r4_finalizer/pc_v2_02/resource_v1")
        for field in ("bundle_digest", "versioned_digest", "legacy_context_digest", "context_digest"):
            self.assertEqual(len(payload[field]), 64, field)

    def test_legacy_payload_declares_no_production_authority(self) -> None:
        payload = resolve_worker_context("legacy")[0].to_payload()

        self.assertIs(payload["production_authority"], False)
        self.assertEqual(payload["context_digest"], payload["legacy_context_digest"])
        # The legacy opt-in reproduces the shipped pre-version identity for this
        # host (accelerator included); it is never the production authority and
        # never the primary digest a version-bound context publishes.
        self.assertEqual(
            payload["context_digest"],
            capture_legacy_context(data_root=DATA_ROOT).context_digest,
        )

    def test_legacy_identity_is_not_the_primary_identity_of_any_version(self) -> None:
        legacy = resolve_worker_context("legacy")[0]

        for version in (V202, V20002):
            with self.subTest(version=version):
                context, _ = resolve_worker_context(version)
                # The legacy opt-in reproduces the same non-authoritative proof
                # digest, which is precisely why it cannot be the authority.
                self.assertNotEqual(context.context_digest, legacy.context_digest)
                self.assertEqual(context.legacy_context_digest, legacy.context_digest)
                self.assertIs(
                    context.to_payload()["production_authority"],
                    True,
                    "a version-bound context is the production authority",
                )


class AuthorityFlowTests(unittest.TestCase):
    def test_search_jobs_authority_is_the_resolved_context_digest(self) -> None:
        context, tables = resolve_worker_context(V202)
        # Tables and context must come from the same explicit version, and the
        # jobs service must adopt that exact context object as its authority.
        self.assertEqual(
            tables.resource.root,
            DATA_ROOT / context.versioned_resource_dir,
        )
        jobs = SearchJobs(context=context, generation_tables=tables)
        try:
            self.assertEqual(jobs.service.context.context_digest, context.context_digest)
            self.assertEqual(jobs.service.context, context)
            self.assertEqual(jobs.generation_tables, tables)
        finally:
            jobs.shutdown()

    def test_same_explicit_version_is_deterministic(self) -> None:
        first, _ = resolve_worker_context(V202)
        second, _ = resolve_worker_context(V202)

        self.assertEqual(first.context_digest, second.context_digest)
        self.assertEqual(first.canonical_payload(), second.canonical_payload())

    def test_two_versions_do_not_share_production_authority(self) -> None:
        v202, _ = resolve_worker_context(V202)
        v20002, _ = resolve_worker_context(V20002)

        self.assertNotEqual(v202.context_digest, v20002.context_digest)
        # The proof digest is shared, which is exactly why it cannot authorize.
        self.assertEqual(v202.legacy_context_digest, v20002.legacy_context_digest)

    def test_legacy_context_never_authorizes_a_job(self) -> None:
        legacy, _ = resolve_worker_context("legacy")
        jobs = SearchJobs(context=legacy)
        try:
            self.assertIsInstance(jobs.service.context, LegacyGenerationContext)
            self.assertFalse(jobs.service.context.to_payload()["production_authority"])
        finally:
            jobs.shutdown()

    def test_candidate_identity_moves_with_the_resolved_context(self) -> None:
        v202, _ = resolve_worker_context(V202)
        v20002, _ = resolve_worker_context(V20002)

        candidate = _candidate()
        service_202 = CandidateApplicationService(context=v202)
        service_20002 = CandidateApplicationService(context=v20002)
        self.assertNotEqual(
            service_202.preview(candidate).candidate_id,
            service_20002.preview(candidate).candidate_id,
        )
        # Same explicit context twice reproduces one candidate identity.
        self.assertEqual(
            service_202.preview(candidate).candidate_id,
            CandidateApplicationService(context=resolve_worker_context(V202)[0])
            .preview(candidate)
            .candidate_id,
        )


def _candidate():
    from nioh3_scroll_editor.auxiliary_generation import generate_complete_auxiliary
    from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence
    from nioh3_scroll_editor.models import ScrollCandidate

    sequence = generate_ng3_certified_effect_sequence(SEED, rarity=4, level=180)
    return ScrollCandidate.from_effect_sequence(
        sequence, auxiliary=generate_complete_auxiliary(SEED, 3)
    )


class NoSilentFallbackTests(unittest.TestCase):
    def test_worker_refuses_to_start_without_a_version(self) -> None:
        _assert_startup_refusal(self)

    def test_worker_refuses_a_malformed_version(self) -> None:
        _assert_startup_refusal(self, "--game-file-version", "2.0.2")

    def test_worker_refuses_an_unregistered_version(self) -> None:
        _assert_startup_refusal(self, "--game-file-version", "9.9.9.9")

    def test_discovery_is_not_a_production_identity_source(self) -> None:
        """A discoverable install must not become production authority.

        This host resolves generation tables by discovery for diagnostics; the
        production entry point must nevertheless refuse when no explicit
        version is named, so a discovered build can never silently select the
        identity a job runs under.
        """
        from nioh3_scroll_editor.search_worker import resolve_generation_tables

        # Probe discovery first: its answer must not change the refusal below.
        resolve_generation_tables()
        _assert_startup_refusal(self)

    def test_help_exits_zero_and_prints_usage(self) -> None:
        completed = _run_worker_cli("--help")
        self.assertEqual(completed.returncode, 0)
        self.assertIn(b"--game-file-version", completed.stdout)


if __name__ == "__main__":
    unittest.main()
