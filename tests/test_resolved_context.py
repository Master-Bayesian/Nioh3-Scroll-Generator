"""Cross-language gate for the Python resolved generation identity.

These tests pin the exact canonical bytes and digests the accepted Rust
contract in ``crates/nioh3-worker/src/context.rs`` produces, so the two roles
cannot drift apart silently. They also prove the fail-closed properties the
contract promises: an unknown or missing version never resolves, and the
pre-version digest is proof-only.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import unittest

from nioh3_scroll_editor.core_services import (
    GENERATION_ALGORITHM_VERSION,
    OPERATION_POLICY_VERSION,
    SUPPORTED_GAME_PROFILE,
    CoreErrorCode,
    CoreServiceError,
    GenerationContext,
)
from nioh3_scroll_editor.resolved_context import (
    LegacyGenerationContext,
    ResolvedGenerationContext,
    capture_legacy_context,
    dotted_file_version,
    normalize_file_version,
    resolve_selected_generation_bundle,
)
from nioh3_scroll_editor.version import APP_VERSION


ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"

V202 = (2, 0, 2, 0)
V20002 = (2, 0, 0, 2)
V201 = (2, 0, 1, 0)

# Pinned v0.8.0 product-identity goldens, all captured with no accelerator.
V202_CANONICAL = (
    '{"algorithm_version":"scroll-generation-v0.7-native-completion-1",'
    '"bundle_digest":"df15220de9e356755bd8b4e2ec33f4617cf0e7c140347898b8374fd75515acbf",'
    '"game_file_version":"2.0.2.0",'
    '"game_profile":"pc-v2.00.02-v2.01",'
    '"policy_version":"operation-policy-v1",'
    '"product_version":"0.8.0",'
    '"resources_digest":"a1e535ced07f15ebfe97b7d567dc081dc719448abbf4db6f57d1e76a826e32c3",'
    '"seed_accelerator_abi":null,'
    '"seed_accelerator_build_id":null,'
    '"versioned_digest":"09fa65803a0c058880f4d38900290b152eab89febb03615ce2576f4b020b358b",'
    '"versioned_resource_dir":"r4_finalizer/pc_v2_02/resource_v1"}'
)
LEGACY_CANONICAL = (
    '{"algorithm_version":"scroll-generation-v0.7-native-completion-1",'
    '"game_profile":"pc-v2.00.02-v2.01",'
    '"policy_version":"operation-policy-v1",'
    '"product_version":"0.8.0",'
    '"resources_digest":"a1e535ced07f15ebfe97b7d567dc081dc719448abbf4db6f57d1e76a826e32c3",'
    '"seed_accelerator_abi":null,'
    '"seed_accelerator_build_id":null}'
)
V202_CONTEXT_DIGEST = "1903eeaffde48d3b10ba5f9edbef88dae6205dcf2f2460d7fe69fc3361bb588e"
V202_BUNDLE_DIGEST = "df15220de9e356755bd8b4e2ec33f4617cf0e7c140347898b8374fd75515acbf"
V202_VERSIONED_DIGEST = "09fa65803a0c058880f4d38900290b152eab89febb03615ce2576f4b020b358b"
V20002_CONTEXT_DIGEST = "61b50195316954103a3768f50da59dd6dd3aa8726487c798aa98c11f26b7b7c4"
V20002_BUNDLE_DIGEST = "5e81a56c268a18a6f799447fdd7c445fea949c24ae555e0cfa75cf9016d5cd92"
V20002_VERSIONED_DIGEST = "915756776dc7c7a236bee49bef5f5ab19dd3a676bdcd405cf88c0fefed977532"
SHARED_LEGACY_DIGEST = "7c70ff81cc33efc06fdcad5b848ee598db2f15d30e5afb990513eaeadc5f47a8"
RESOURCES_DIGEST = "a1e535ced07f15ebfe97b7d567dc081dc719448abbf4db6f57d1e76a826e32c3"

# The Rust fixture captured its digest with no accelerator attached; this host
# may have the DLL loaded, so every golden comparison states the accelerator
# explicitly instead of inheriting it.
NO_ACCELERATOR = (None, None)


def _context(file_version: tuple[int, int, int, int]) -> ResolvedGenerationContext:
    return ResolvedGenerationContext.capture(
        file_version=file_version,
        accelerator=NO_ACCELERATOR,
        data_root=DATA_ROOT,
    )


class CanonicalEncodingTests(unittest.TestCase):
    def test_python_payload_matches_rust_golden_bytes_and_digest(self) -> None:
        context = _context(V202)
        self.assertEqual(context.canonical_payload(), V202_CANONICAL)
        self.assertEqual(
            hashlib.sha256(V202_CANONICAL.encode("utf-8")).hexdigest(),
            V202_CONTEXT_DIGEST,
        )
        self.assertEqual(context.context_digest, V202_CONTEXT_DIGEST)

    def test_canonical_encoding_is_sorted_compact_json(self) -> None:
        context = _context(V202)
        payload = json.loads(context.canonical_payload())
        self.assertEqual(
            context.canonical_payload(),
            json.dumps(payload, sort_keys=True, separators=(",", ":")),
        )
        # Eleven resolved keys; `context_digest` is never part of its own input.
        self.assertEqual(len(payload), 11)
        self.assertNotIn("context_digest", payload)
        self.assertNotIn("legacy_context_digest", payload)
        self.assertEqual(
            list(payload),
            [
                "algorithm_version",
                "bundle_digest",
                "game_file_version",
                "game_profile",
                "policy_version",
                "product_version",
                "resources_digest",
                "seed_accelerator_abi",
                "seed_accelerator_build_id",
                "versioned_digest",
                "versioned_resource_dir",
            ],
        )
        self.assertIsNone(payload["seed_accelerator_abi"])
        self.assertIsNone(payload["seed_accelerator_build_id"])

    def test_pinned_goldens_for_both_versions(self) -> None:
        v202 = _context(V202)
        v20002 = _context(V20002)

        self.assertEqual(v202.context_digest, V202_CONTEXT_DIGEST)
        self.assertEqual(v202.bundle_digest, V202_BUNDLE_DIGEST)
        self.assertEqual(v202.versioned_digest, V202_VERSIONED_DIGEST)
        self.assertEqual(v202.resources_digest, RESOURCES_DIGEST)
        self.assertEqual(v202.versioned_resource_dir, "r4_finalizer/pc_v2_02/resource_v1")
        self.assertEqual(v202.dotted_game_file_version, "2.0.2.0")

        self.assertEqual(v20002.context_digest, V20002_CONTEXT_DIGEST)
        self.assertEqual(v20002.bundle_digest, V20002_BUNDLE_DIGEST)
        self.assertEqual(v20002.versioned_digest, V20002_VERSIONED_DIGEST)
        self.assertEqual(v20002.resources_digest, RESOURCES_DIGEST)
        self.assertEqual(
            v20002.versioned_resource_dir, "r4_finalizer/pc_v2_00_02/resource_v1"
        )
        self.assertEqual(v20002.dotted_game_file_version, "2.0.0.2")

        # One fixed profile and one whole-root digest, two distinct identities.
        self.assertNotEqual(v202.context_digest, v20002.context_digest)
        self.assertEqual(v202.legacy_context_digest, v20002.legacy_context_digest)


class IdentityBindingTests(unittest.TestCase):
    def test_version_and_bundle_changes_alter_the_primary_digest(self) -> None:
        v202 = _context(V202)
        v20002 = _context(V20002)
        v201 = _context(V201)

        self.assertNotEqual(v202.bundle_digest, v20002.bundle_digest)
        self.assertNotEqual(v202.versioned_digest, v20002.versioned_digest)
        self.assertNotEqual(v202.context_digest, v20002.context_digest)

        # PC v2.01 aliases the shipped PC v2.00.02 payload, so it shares the
        # bundle identity but not the version-bound primary digest.
        self.assertEqual(v201.bundle_digest, v20002.bundle_digest)
        self.assertEqual(v201.versioned_digest, v20002.versioned_digest)
        self.assertEqual(v201.versioned_resource_dir, v20002.versioned_resource_dir)
        self.assertNotEqual(v201.context_digest, v20002.context_digest)

    def test_selected_bundle_does_not_hash_the_whole_data_root(self) -> None:
        bundle = resolve_selected_generation_bundle(DATA_ROOT, V202)
        # The selection lists selected inputs only; unrelated data-root files
        # must be absent, so this is not a whole-root hash.
        joined = "\n".join(bundle.files)
        self.assertNotIn("effect_names_multilingual.json", joined)
        self.assertNotIn("live_add_pc_v202_identity.json", joined)
        self.assertIn("enemy_states/pc_v2_01/native_tables.json", bundle.files)
        self.assertTrue(
            any(name.startswith("auxiliary_generation/") for name in bundle.files)
        )
        # A versioned file of the v2.02 identity lives in its own directory.
        for name in bundle.files:
            if name.startswith("r4_finalizer/"):
                self.assertTrue(name.startswith("r4_finalizer/pc_v2_02/"), name)

    def test_every_resolved_field_mutation_changes_the_primary_digest(self) -> None:
        from nioh3_scroll_editor.resolved_context import _resolved_canonical_payload

        context = _context(V202)
        baseline = context.context_digest
        base = {
            "product_version": context.product_version,
            "game_profile": context.game_profile,
            "game_file_version": context.game_file_version,
            "versioned_resource_dir": context.versioned_resource_dir,
            "bundle_digest": context.bundle_digest,
            "versioned_digest": context.versioned_digest,
            "resources_digest": context.resources_digest,
            "algorithm_version": context.algorithm_version,
            "policy_version": context.policy_version,
            "seed_accelerator_abi": context.seed_accelerator_abi,
            "seed_accelerator_build_id": context.seed_accelerator_build_id,
        }
        variants = {
            "game_file_version": {**base, "game_file_version": V20002},
            "versioned_resource_dir": {
                **base,
                "versioned_resource_dir": "r4_finalizer/pc_v2_00_02/resource_v1",
            },
            "bundle_digest": {**base, "bundle_digest": "0" * 64},
            "versioned_digest": {**base, "versioned_digest": "0" * 64},
            "resources_digest": {**base, "resources_digest": "0" * 64},
            "game_profile": {**base, "game_profile": "pc-v0.0.0-v0.0"},
            "product_version": {**base, "product_version": "0.0.0"},
            "algorithm_version": {**base, "algorithm_version": "scroll-generation-v0"},
            "policy_version": {**base, "policy_version": "operation-policy-v0"},
            "seed_accelerator_abi": {**base, "seed_accelerator_abi": 1},
            "seed_accelerator_build_id": {
                **base,
                "seed_accelerator_build_id": "mutant-build",
            },
        }
        for field, payload in variants.items():
            canonical = _resolved_canonical_payload(**payload)
            digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
            self.assertNotEqual(
                digest, baseline, f"mutating {field} must change the primary digest"
            )

    def test_capture_rejects_a_bundle_resolved_for_another_version(self) -> None:
        bundle = resolve_selected_generation_bundle(DATA_ROOT, V202)
        with self.assertRaises(CoreServiceError) as raised:
            ResolvedGenerationContext.from_bundle(
                game_profile=SUPPORTED_GAME_PROFILE,
                file_version=V20002,
                data_root=DATA_ROOT,
                bundle=bundle,
                accelerator=NO_ACCELERATOR,
            )
        self.assertEqual(raised.exception.code, CoreErrorCode.RESOURCE_MISMATCH)


class FailClosedTests(unittest.TestCase):
    def test_unknown_version_fails_closed_without_a_default(self) -> None:
        for unknown in [(9, 9, 9, 9), (2, 0, 3, 0), (1, 0, 0, 0), (2, 1, 0, 0)]:
            with self.assertRaises(CoreServiceError) as raised:
                ResolvedGenerationContext.capture(
                    file_version=unknown,
                    accelerator=NO_ACCELERATOR,
                    data_root=DATA_ROOT,
                )
            self.assertEqual(raised.exception.code, CoreErrorCode.RESOURCE_MISMATCH)
            self.assertIn("no offline generation resource", str(raised.exception))

    def test_unknown_version_fails_closed_before_the_root_is_read(self) -> None:
        # A missing root with an unknown version reports the version, so the
        # version check provably ran first and no fallback content was opened.
        missing = ROOT / "deliverables" / "definitely-not-present"
        with self.assertRaises(CoreServiceError) as raised:
            ResolvedGenerationContext.capture(
                file_version=(9, 9, 9, 9),
                accelerator=NO_ACCELERATOR,
                data_root=missing,
            )
        self.assertIn("no offline generation resource", str(raised.exception))

        # Control: the same missing root is reported for a registered version.
        with self.assertRaises(CoreServiceError) as control:
            ResolvedGenerationContext.capture(
                file_version=V20002,
                accelerator=NO_ACCELERATOR,
                data_root=missing,
            )
        self.assertEqual(control.exception.code, CoreErrorCode.RESOURCE_MISMATCH)
        self.assertNotIn("no offline generation resource", str(control.exception))

    def test_missing_or_malformed_version_is_rejected(self) -> None:
        with self.assertRaises(CoreServiceError):
            ResolvedGenerationContext.capture(
                file_version=None, accelerator=NO_ACCELERATOR, data_root=DATA_ROOT
            )
        for bad in [(2, 0, 2), (2, 0, 2, 0, 0), "2.0.2", (2, 0, 2, "x")]:
            with self.assertRaises(CoreServiceError):
                normalize_file_version(bad)
        self.assertEqual(normalize_file_version([2, 0, 2, 0]), V202)
        self.assertEqual(normalize_file_version("2.0.0.2"), V20002)
        self.assertEqual(dotted_file_version(V202), "2.0.2.0")

    def test_registered_versions_are_exactly_the_shipped_three(self) -> None:
        from nioh3_scroll_editor.resolved_context import VERSIONED_RESOURCE_DIRS

        self.assertEqual(
            set(VERSIONED_RESOURCE_DIRS), {(2, 0, 0, 2), (2, 0, 1, 0), (2, 0, 2, 0)}
        )


class LegacyIsProofOnlyTests(unittest.TestCase):
    def test_legacy_digest_matches_rust_and_is_not_the_primary_digest(self) -> None:
        legacy = capture_legacy_context(accelerator=NO_ACCELERATOR, data_root=DATA_ROOT)
        self.assertIsInstance(legacy, LegacyGenerationContext)
        self.assertEqual(legacy.context_digest, SHARED_LEGACY_DIGEST)
        self.assertEqual(
            hashlib.sha256(LEGACY_CANONICAL.encode("utf-8")).hexdigest(),
            SHARED_LEGACY_DIGEST,
        )

        for version in (V202, V20002):
            context = _context(version)
            self.assertEqual(context.legacy_context_digest, SHARED_LEGACY_DIGEST)
            self.assertNotEqual(context.context_digest, context.legacy_context_digest)

    def test_legacy_payload_declares_no_production_authority(self) -> None:
        legacy = capture_legacy_context(accelerator=NO_ACCELERATOR, data_root=DATA_ROOT)
        payload = legacy.to_payload()
        self.assertFalse(payload["production_authority"])
        self.assertEqual(payload["legacy_context_digest"], payload["context_digest"])

    def test_production_payload_declares_authority_and_carries_proof_fields(self) -> None:
        payload = _context(V202).to_payload()
        self.assertTrue(payload["production_authority"])
        self.assertEqual(payload["context_digest"], V202_CONTEXT_DIGEST)
        self.assertEqual(payload["legacy_context_digest"], SHARED_LEGACY_DIGEST)
        self.assertEqual(payload["game_file_version"], "2.0.2.0")
        self.assertEqual(payload["bundle_digest"], V202_BUNDLE_DIGEST)
        self.assertEqual(payload["versioned_digest"], V202_VERSIONED_DIGEST)
        # Legacy-compatible keys survive so an old reader still recognizes them.
        for key in (
            "product_version",
            "game_profile",
            "resources_digest",
            "algorithm_version",
            "policy_version",
            "seed_accelerator_abi",
            "seed_accelerator_build_id",
        ):
            self.assertIn(key, payload)

    def test_legacy_capture_is_not_the_shipped_production_identity(self) -> None:
        # The shipped GenerationContext is untouched by this ticket; this pins
        # that it still folds the pre-version seven-field identity.
        shipped = GenerationContext.capture(data_root=DATA_ROOT)
        self.assertEqual(shipped.context_digest, hashlib.sha256(
            json.dumps(
                {
                    "product_version": APP_VERSION,
                    "game_profile": SUPPORTED_GAME_PROFILE,
                    "resources_digest": shipped.resources_digest,
                    "algorithm_version": GENERATION_ALGORITHM_VERSION,
                    "policy_version": OPERATION_POLICY_VERSION,
                    "seed_accelerator_abi": shipped.seed_accelerator_abi,
                    "seed_accelerator_build_id": shipped.seed_accelerator_build_id,
                },
                sort_keys=True,
                separators=(",", ":"),
            ).encode("utf-8")
        ).hexdigest())
        self.assertEqual(shipped.resources_digest, RESOURCES_DIGEST)


if __name__ == "__main__":
    unittest.main()
