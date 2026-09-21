"""RW07: the NG3/NG4/NG5 rarity-5 routes must consume the selected tables.

The pre-fix defect was that a version-bound worker resolved its generation
tables from the explicit game file version and then handed them to
``SearchJobs``, but the NG3 rarity-5 early return and the shared NG4/NG5
rarity-5 collector dropped them.  Every rarity-5 primary/effect pool then came
from the shipped PC v2.00.02 baseline loader instead of the selected index, so
two different identities could run one set of tables.

These tests import the production modules and execute each of the three
playthrough routes to a definite boundary (the R5 sequence call or the
collector boundary).  They do not stub the collectors, and the observable is
the resource/marker identity the production code actually passed downstream,
never a spy that re-implements the algorithm.  The shipped baseline loader is
poisoned so that a silent fallback fails the run instead of quietly producing
the old tables.
"""

from __future__ import annotations

import ctypes
import sys
from pathlib import Path
import unittest

from nioh3_scroll_editor import seed_accelerator
from nioh3_scroll_editor.effect_generation_tables import (
    EffectGenerationTableIndex,
    effect_generation_tables_for_game_version,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.effect_sequence import (
    generate_rarity5_any_grace_primary_effect_ids,
    generate_rarity5_grace_effect_sequence,
    generate_rarity5_grace_primary_effect_ids,
    rarity5_primary_effect_lookups,
    take_rarity5_primary_pool,
)
from nioh3_scroll_editor.effect_seed_solver import EffectSeedRequest
from nioh3_scroll_editor.grace_map import GraceOutputMap, GraceRange
from nioh3_scroll_editor.search_application import (
    collect_offline_ng3_search_batch,
    collect_offline_rarity5_search_batch,
)
from nioh3_scroll_editor.search_jobs import SearchJobs
from nioh3_scroll_editor.search_worker import resolve_worker_context
from nioh3_scroll_editor.seed_accelerator import (
    native_seed_acceleration_available,
    seed_acceleration_execution_policy,
)


ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
V202 = (2, 0, 2, 0)
SEED = 10_030_700


def _flat_grace_map(playthrough: int, grace_id: int) -> GraceOutputMap:
    """A complete one-bucket partition, so the marked Grace is what is drawn."""

    return GraceOutputMap(
        record_type={3: 0xE604, 4: 0xDD82, 5: 0xD523}[playthrough],
        rarity=5,
        playthrough=3,
        effect_slot=6,
        ranges=(GraceRange(0, 0xFFFF, grace_id),),
    )


class _RecordingTables(EffectGenerationTableIndex):
    """The real v2.02 index, tagged with the marker the caller selected."""

    marker: str

    def __init__(self, base: EffectGenerationTableIndex, marker: str):
        # Reuse the already-parsed indexes instead of re-reading the payload.
        self.__dict__.update(base.__dict__)
        self.marker = marker


class _PoisonedBaseline:
    """Fail the moment a generation route reaches a shipped default loader.

    ``load_default_effect_generation_tables`` and
    ``load_default_r4_finalizer_resource`` are bound separately in every module
    that uses them, so every loaded binding is patched, not only the three the
    R5 sequence layer imports.  A version-bound route that falls back through
    the solver preflight, the DirectCompute batch filter, or a path inverse then
    fails instead of quietly reusing PC v2.00.02 tables.

    The auxiliary generator is the single recorded exception: it reads a
    version-invariant sub-resource of the r4 bundle (mode gating) rather than a
    generation table.  ``test_auxiliary_output_is_version_invariant`` measures
    that equivalence, so its default is observed and allowed while every
    generation-table fallback stays fatal.
    """

    calls: list[str]

    def __init__(self, test: unittest.TestCase):
        self.test = test
        self.calls = []

    def __enter__(self):
        import nioh3_scroll_editor.auxiliary_generation as auxiliary_generation

        allowed = {(auxiliary_generation, "load_default_r4_finalizer_resource")}
        watched = (
            "load_default_effect_generation_tables",
            "load_default_r4_finalizer_resource",
        )
        targets: list = []
        for module_name, module in list(sys.modules.items()):
            if module is None or not module_name.startswith("nioh3_scroll_editor"):
                continue
            if module not in targets:
                targets.append(module)
        self._saved = []
        for module in targets:
            for attribute in watched:
                original = getattr(module, attribute, None)
                if not callable(original):
                    continue
                self._saved.append((module, attribute, original))
                setattr(
                    module,
                    attribute,
                    self._poison(attribute, original, (module, attribute) not in allowed),
                )
        return self

    def _poison(self, name, original, fatal: bool):
        def observe(*args, **kwargs):
            self.calls.append(name)
            if fatal:
                raise AssertionError(
                    f"the shipped legacy default loader {name} was reached"
                )
            return original(*args, **kwargs)

        return observe

    def __exit__(self, *_exc):
        for module, name, original in self._saved:
            setattr(module, name, original)
        return False


class MarkerEvidence:
    """Record what the production call actually handed to the sequence layer."""

    def __init__(self):
        self.markers: list[str | None] = []

    def record(self, tables) -> None:
        self.markers.append(getattr(tables, "marker", None))


class SelectedTablesReachTheSequenceTests(unittest.TestCase):
    """The R5 sequence layer consumes the caller's index, not the baseline."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.v202 = effect_generation_tables_for_game_version(V202)
        cls.shipped = load_default_effect_generation_tables()

    def setUp(self) -> None:
        # The batch route reaches the shipped ABI-v2 Seed accelerator DLL, whose
        # default policy is strict GPU. A hosted runner has no CUDA device, so the
        # DLL would refuse this valid request before the selected index is used.
        # Opt this fixture into the DLL's bulk-CPU fallback the same way
        # tests/test_auxiliary_generation.py does; the product default stays
        # strict, and tests/test_backend_freeze.py keeps asserting that strict GPU
        # never silently enters the CPU loop.
        self.enterContext(seed_acceleration_execution_policy(allow_bulk_cpu=True))

    def test_marker_index_is_the_one_used_for_effect_pools(self) -> None:
        marked = _RecordingTables(self.v202, "v2.02")

        with _PoisonedBaseline(self):
            sequence = generate_rarity5_grace_effect_sequence(
                SEED,
                playthrough=3,
                level=180,
                tables=marked,
                grace_mapping=_flat_grace_map(3, 0x6553),
            )

        # Independent output evidence: the R5 Grace slot is the marked draw and
        # the sequence is a complete six-effect result.
        self.assertEqual(sequence.grace.effect_id, 0x6553)
        self.assertEqual(len(sequence.effects), 6)
        self.assertGreater(sequence.random_draws, 0)

    def test_primary_lookup_uses_the_selected_index(self) -> None:
        marked = _RecordingTables(self.v202, "v2.02")

        with _PoisonedBaseline(self):
            normal, promoted = rarity5_primary_effect_lookups(
                0x6553, 0xE604, 3, marked
            )
            self.assertEqual(len(normal), 0x10000)
            self.assertEqual(len(promoted), 0x10000)

    def test_omitting_tables_is_refused_not_defaulted(self) -> None:
        with self.assertRaises(TypeError):
            take_rarity5_primary_pool(0x6553, False, 0xE604, 3, None)
        with self.assertRaises(TypeError):
            rarity5_primary_effect_lookups(0x6553, 0xE604, 3, None)

    def test_batched_primary_ids_use_the_selected_index(self) -> None:
        """Both batch entry points must accept and use the selected index.

        With the poisoned baseline in place, success proves the batch routes
        resolved their pools through the caller's index.  The two indexes give
        the same primary pools for this shipped pair, so the assertion is
        deliberately about the resource used, not about divergent numbers:
        R5 primary output being stable across v2.00.02/v2.02 is itself an
        observation, not something this ticket changes.
        """

        marked = _RecordingTables(self.v202, "v2.02")
        seeds = (SEED, SEED + 1, SEED + 2)

        with _PoisonedBaseline(self):
            fixed = generate_rarity5_grace_primary_effect_ids(
                seeds, playthrough=3, grace_id=0x6553, tables=marked
            )
            any_grace = generate_rarity5_any_grace_primary_effect_ids(
                seeds, playthrough=3, tables=marked
            )

        self.assertEqual(len(fixed), len(seeds))
        self.assertEqual(len(any_grace), len(seeds))
        self.assertTrue(all(0 <= effect_id <= 0xFFFF for effect_id in any_grace))
        # The batch routes and the single-Seed route must agree under one index.
        single = all(
            generate_rarity5_grace_effect_sequence(
                seed,
                playthrough=3,
                level=180,
                tables=marked,
                grace_mapping=_flat_grace_map(3, 0x6553),
            ).effects[0].effect_id
            == effect_id
            for seed, effect_id in zip(seeds, fixed)
        )
        self.assertTrue(single, "batched and single-Seed primary routes disagree")


@unittest.skipUnless(
    native_seed_acceleration_available(),
    "native Seed accelerator is unavailable",
)
class HostedNoGpuBatchRouteTests(unittest.TestCase):
    """The hosted no-CUDA condition, reproduced deterministically.

    The hosted runner loads the same tracked ABI-v2 DLL but has no CUDA device,
    while the DLL's default policy is strict GPU, so a valid batch request was
    refused with "native primary batch accelerator rejected valid input". The
    DLL's own test hook reproduces that condition on a machine that does have a
    device, so this proves the fixture opt-in carries the route while the
    product default still refuses.
    """

    @classmethod
    def setUpClass(cls) -> None:
        cls.v202 = effect_generation_tables_for_game_version(V202)

    def test_cpu_opt_in_carries_the_batch_and_strict_gpu_still_refuses(self) -> None:
        library = seed_accelerator._load_accelerator()
        if library is None:
            self.skipTest("ABI-v2 Seed accelerator DLL is unavailable")
        force_failure = library.seed_accelerator_test_force_cuda_failure
        force_failure.argtypes = (ctypes.c_int,)
        force_failure.restype = None
        marked = _RecordingTables(self.v202, "v2.02")
        seeds = (SEED, SEED + 1, SEED + 2)
        force_failure(1)
        try:
            # Strict GPU is still fail-closed: the same valid request is refused,
            # so nothing in the product default entered the CPU loop.
            with seed_acceleration_execution_policy(allow_bulk_cpu=False):
                with self.assertRaises(RuntimeError):
                    generate_rarity5_grace_primary_effect_ids(
                        seeds, playthrough=3, grace_id=0x6553, tables=marked
                    )
            with seed_acceleration_execution_policy(allow_bulk_cpu=True):
                fixed = generate_rarity5_grace_primary_effect_ids(
                    seeds, playthrough=3, grace_id=0x6553, tables=marked
                )
                any_grace = generate_rarity5_any_grace_primary_effect_ids(
                    seeds, playthrough=3, tables=marked
                )
        finally:
            force_failure(0)
            library.seed_accelerator_set_execution_policy(
                seed_accelerator.EXECUTION_POLICY_STRICT_GPU
            )
        self.assertEqual(len(fixed), len(seeds))
        self.assertEqual(len(any_grace), len(seeds))
        self.assertTrue(all(0 <= effect_id <= 0xFFFF for effect_id in any_grace))


class ThreeRouteTablePropagationTests(unittest.TestCase):
    """NG3, NG4, and NG5 rarity-5 routes all carry the selected index."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.v202 = effect_generation_tables_for_game_version(V202)

    def setUp(self) -> None:
        # These routes reach the shipped ABI-v2 Seed accelerator DLL, whose
        # default policy is strict GPU. A hosted runner has no CUDA device, so
        # the DLL would refuse this valid request before the selected index is
        # used. Opt this fixture into the bulk-CPU fallback the same way the
        # product does when a search allows CPU replay and the sibling fixture
        # above does; the product default stays strict and
        # tests/test_backend_freeze.py keeps asserting that strict GPU never
        # silently enters the CPU loop.
        self.enterContext(seed_acceleration_execution_policy(allow_bulk_cpu=True))

    def _collector_page(self, playthrough: int, tables, monkey) -> None:
        request = EffectSeedRequest(
            playthrough=playthrough,
            rarity=5,
            grace_effect_id=0x6553,
        )
        collect_offline_rarity5_search_batch(
            request,
            grace_mapping=_flat_grace_map(playthrough, 0x6553),
            level=180,
            result_count=1,
            max_trials_per_batch=200_000,
            tables=tables,
            allow_cpu_fallback=True,
        )

    def test_ng3_early_return_forwards_the_selected_index(self) -> None:
        marked = _RecordingTables(self.v202, "v2.02")
        seen = MarkerEvidence()

        import nioh3_scroll_editor.search_application as application

        real = application.generate_rarity5_grace_effect_sequence

        def observe(*args, **kwargs):
            seen.record(kwargs.get("tables"))
            return real(*args, **kwargs)

        application.generate_rarity5_grace_effect_sequence = observe
        try:
            baseline = _PoisonedBaseline(self)
            with baseline:
                collect_offline_ng3_search_batch(
                    EffectSeedRequest(
                        playthrough=3, rarity=5, grace_effect_id=0x6553
                    ),
                    grace_mapping=_flat_grace_map(3, 0x6553),
                    level=180,
                    result_count=1,
                    max_trials_per_batch=200_000,
                    tables=marked,
                    allow_cpu_fallback=True,
                )
        finally:
            application.generate_rarity5_grace_effect_sequence = real

        # The production NG3 route reached the R5 sequence layer, and what it
        # handed over was the marked index - not None, not the shipped baseline.
        self.assertTrue(seen.markers, "the NG3/R5 route never reached the sequence layer")
        self.assertIn("v2.02", seen.markers)
        self.assertNotIn(None, seen.markers)
        self.assertNotIn("load_default_effect_generation_tables", baseline.calls)

    def test_ng4_and_ng5_collectors_receive_the_selected_index(self) -> None:
        for playthrough in (4, 5):
            with self.subTest(playthrough=playthrough):
                marked = _RecordingTables(self.v202, "v2.02")
                seen = MarkerEvidence()

                import nioh3_scroll_editor.search_application as application

                real = application.generate_rarity5_grace_effect_sequence

                def observe(*args, **kwargs):
                    seen.record(kwargs.get("tables"))
                    return real(*args, **kwargs)

                application.generate_rarity5_grace_effect_sequence = observe
                try:
                    baseline = _PoisonedBaseline(self)
                    with baseline:
                        self._collector_page(playthrough, marked, None)
                finally:
                    application.generate_rarity5_grace_effect_sequence = real

                self.assertIn("v2.02", seen.markers)
                self.assertNotIn(None, seen.markers)
                self.assertNotIn(
                    "load_default_effect_generation_tables", baseline.calls
                )

    def test_search_jobs_forwards_tables_on_every_rarity5_route(self) -> None:
        """The job boundary must not special-case only the NG3 collector."""

        import inspect

        source = inspect.getsource(SearchJobs._run)
        self.assertIn("collector_kwargs['tables'] = self.generation_tables", source)
        self.assertNotIn("collector is self.collector", source)

    def test_version_bound_context_without_tables_fails_closed(self) -> None:
        context, tables = resolve_worker_context(V202)
        self.assertIsNotNone(tables)

        jobs = SearchJobs(context=context, generation_tables=None)
        try:
            with self.assertRaises(Exception) as raised:
                jobs.start(
                    {
                        "context_digest": context.context_digest,
                        "allow_cpu_fallback": True,
                        "result_count": 1,
                        "job_trials": 1,
                        "page_trials": 1,
                        "resume_token": None,
                        "query": {
                            "playthrough": 3,
                            "rarity": 5,
                            "level": 180,
                            "grace_effect_id": 0x6553,
                            "primary_effect_ids": [],
                            "required_secondary_ids": [],
                            "required_secondary_id_groups": [],
                            "minimum_roll_percent_by_effect_id": [],
                            "auxiliary": {
                                "required_terrain_effect_keys": [],
                                "required_terrain_effect_key_groups": [],
                                "required_special_rule_keys": [],
                                "required_special_rule_key_groups": [],
                                "required_enemy_lookup_keys": [],
                                "required_enemy_lookup_key_groups": [],
                            },
                        },
                    }
                )
            self.assertEqual(getattr(raised.exception, "code", None), "RESOURCE_MISMATCH")
        finally:
            jobs.shutdown()


class IndependentResourceEvidenceTests(unittest.TestCase):
    """The v2.02 index is a different resource from the shipped baseline."""

    def test_auxiliary_output_is_version_invariant(self) -> None:
        """The one recorded default is a shared, version-invariant sub-resource.

        ``generate_complete_auxiliary`` reads the r4 bundle for mode gating, a
        blob the pc_v2_02 resource did not change.  Over this sample the two
        bundles produce identical auxiliary output, so the recorded
        ``load_default_r4_finalizer_resource`` is not a generation-table
        fallback.  If a future payload changes an auxiliary-relevant row this
        test fails and the auxiliary route must be rebound to the selected
        resource.
        """

        from dataclasses import asdict

        from nioh3_scroll_editor.auxiliary_generation import generate_complete_auxiliary
        from nioh3_scroll_editor.r4_finalizer_resource import (
            load_default_r4_finalizer_resource,
            load_r4_finalizer_resource_for_version,
        )

        shipped = load_default_r4_finalizer_resource()
        versioned = load_r4_finalizer_resource_for_version(V202)
        for playthrough in (3, 4, 5):
            for index in range(25):
                seed = 2_000_000 + index * 99_991
                with self.subTest(playthrough=playthrough, index=index):
                    self.assertEqual(
                        asdict(
                            generate_complete_auxiliary(
                                seed, playthrough, resource=shipped
                            )
                        ),
                        asdict(
                            generate_complete_auxiliary(
                                seed, playthrough, resource=versioned
                            )
                        ),
                    )

    def test_selected_index_is_not_the_shipped_resource(self) -> None:
        v202 = effect_generation_tables_for_game_version(V202)
        shipped = load_default_effect_generation_tables()

        self.assertIsNot(v202, shipped)
        self.assertNotEqual(v202.resource.root, shipped.resource.root)
        self.assertEqual(v202.resource.root, DATA_ROOT / "r4_finalizer/pc_v2_02/resource_v1")
        self.assertNotEqual(
            len(v202.optional_multipliers_by_key),
            len(shipped.optional_multipliers_by_key),
        )


if __name__ == "__main__":
    unittest.main()
