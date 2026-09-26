//! Typed native query layer: one bounded page at a time.
//!
//! This module owns the query/paging contract the job layer drives. It never
//! parses protocol JSON, never reads product data files and never enumerates
//! seeds in Rust: the accelerator scans a bounded window, and every match it
//! returns is re-derived here with exact cursor algebra before the page is
//! handed on.

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use nioh3_domain::rng::A_INV;

use crate::effect_batch::{EffectMaskSpec, PartialEffectVerifier, PREDICATE_BATCH_SIZE};
use crate::effect_path::{
    native_path_descriptors, CompiledEffectPlan, EffectPathError, PreimageVerifier, U16Run,
};
use crate::native_search::{
    absent_capabilities, Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, ExecutionPolicyGuard,
    NativeBackend, NativeCapabilities, NativeSearchError, PivotMatch, PivotWindow,
    PrimaryEffectSpec, R4PrimaryPivotSpec, MAX_AUXILIARY_TRIALS, MAX_NATURAL_TRIALS,
    MAX_R4_PRIMARY_TRIALS,
};
use crate::preimage::{
    EffectPathInput, PreimageAccelerator, PreimageError, PreimagePlanParams, PreimagePolicy,
    DEFAULT_OUTPUT_CAPACITY, MAX_OUTPUT_CAPACITY, MAX_PREIMAGE_TRIALS,
};

/// Result capacity one fused auxiliary chunk may return.
///
/// Mirrors the Python reference's `output_capacity=1_000_000` default.
pub const DEFAULT_AUXILIARY_OUTPUT_CAPACITY: u64 = 1_000_000;

/// Serializes every native call in this process.
///
/// The accelerator keeps process-global diagnostics, so two concurrent page
/// loops would race them even though the execution policy is locked. Lock order
/// is policy first, then this lock, everywhere, so a job-scoped
/// [`ExecutionPolicyGuard`] can never deadlock against a page loop.
static NATIVE_CALL_LOCK: Mutex<()> = Mutex::new(());

fn native_call_lock() -> MutexGuard<'static, ()> {
    NATIVE_CALL_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// One compiled native query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativePivotQuery {
    /// Plain natural-pivot enumeration over one permuted value table.
    Natural { values: Vec<u16> },
    /// Rarity-4 primary-effect pivot scan with the recovered context matrices.
    R4Primary {
        values: Vec<u16>,
        spec: R4PrimaryPivotSpec,
    },
    /// Fused auxiliary scan: terrain, enemy groups, scratch groups and rules.
    Auxiliary {
        values: Vec<u16>,
        spec: AuxiliaryPivotSpec,
    },
    /// Complete-composition effect-preimage sweep over the concatenated plan
    /// families (`effect_preimage_search.collect_full_composition_preimage_page`).
    ///
    /// The cursor space is the concatenation of every plan's pivot family, in
    /// plan order: a trial is `plan_offset + local native trial + 1`. The
    /// accelerator reports its own `(Seed, local trial)` pairs, and each pair is
    /// only accepted after [`PreimageVerifier`] re-composes the Seed with the
    /// certified forward generator, so this route has no cursor-to-Seed replay
    /// check to add.
    EffectPreimage {
        plans: Arc<Vec<CompiledEffectPlan>>,
        verifier: Arc<PreimageVerifier>,
    },
    /// NG3 rarity-3 named-primary pivot families
    /// (`search_application.collect_offline_ng3_rarity3_primary_pivot_search_batch`).
    ///
    /// The cursor space is the concatenation of every compiled family, in
    /// family order: a one-based trial is `family_offset + local native trial`,
    /// exactly like the shipped `CompiledPivotFamily` cursor. The
    /// accelerator's own trials are pivot-value-major (value index major, the
    /// state's low sixteen bits minor), so every returned pair is re-derived
    /// here from its trial before the page hands it on.
    PrimaryPivot {
        families: Arc<Vec<PrimaryPivotFamilySpec>>,
    },
}

/// One NG3 rarity-3 primary-pivot family in its native form.
///
/// `values` is the family's ascending high-16 bucket list (the pivot table
/// the fixed-draw collector enumerates), `descriptors` are the packed path
/// constraints that enforce the family's promotion interval, and `params`
/// carries the same scalar configuration the shipped synthetic plan uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryPivotFamilySpec {
    pub values: Vec<u16>,
    pub descriptors: Vec<EffectPathInput>,
    pub params: PreimagePlanParams,
    /// The family's promotion interval, kept for diagnostics and tests.
    pub promotion_u16_runs: Vec<U16Run>,
}

impl PrimaryPivotFamilySpec {
    /// `PrimaryPivotFamily.pivot_state_count`: the family's 65,536-trial states.
    pub fn pivot_state_count(&self) -> u64 {
        self.values.len() as u64 * 0x1_0000
    }
}

impl NativePivotQuery {
    /// The permuted pivot values the cursor walks.
    pub fn values(&self) -> &[u16] {
        match self {
            NativePivotQuery::Natural { values }
            | NativePivotQuery::R4Primary { values, .. }
            | NativePivotQuery::Auxiliary { values, .. } => values,
            // The preimage route's cursor space is the concatenated plan
            // families, not one permuted value table: it reports no flat value
            // table, and its acceptance check is the certified recomposition.
            NativePivotQuery::EffectPreimage { .. } => &[],
            // The primary-pivot route's cursor space is the concatenation of
            // its own families, so it reports no single flat value table
            // either; its acceptance check is the family replay below.
            NativePivotQuery::PrimaryPivot { .. } => &[],
        }
    }

    /// The `low16_stride` this route's pivot uses.
    pub fn low16_stride(&self) -> u16 {
        match self {
            NativePivotQuery::Auxiliary { .. } => AUXILIARY_LOW16_STRIDE,
            // Each plan family is enumerated low16-minor by the accelerator, so
            // the stride is one; no cursor replay uses it on this route.
            NativePivotQuery::EffectPreimage { .. } => 1,
            // The fixed-draw collector owns the low-16 order of a pivot family
            // (the shipped adapter discards `low16_stride`), so this value is
            // unused by the route.
            NativePivotQuery::PrimaryPivot { .. } => 1,
            _ => R4_PRIMARY_LOW16_STRIDE,
        }
    }

    /// Total number of trials in the pivot family.
    pub fn family_size(&self) -> u64 {
        match self {
            NativePivotQuery::EffectPreimage { plans, .. } => plans
                .iter()
                .map(CompiledEffectPlan::pivot_state_count)
                .sum(),
            NativePivotQuery::PrimaryPivot { families } => families
                .iter()
                .map(PrimaryPivotFamilySpec::pivot_state_count)
                .sum(),
            query => query.values().len() as u64 * 0x1_0000,
        }
    }

    /// Native stage-count vector length for this route.
    ///
    /// Only the fused auxiliary route reports native stages. The plain natural
    /// and R4-primary scans return no stage vector rather than a fabricated one.
    pub fn stage_count(&self) -> usize {
        match self {
            NativePivotQuery::Auxiliary { spec, .. } => spec.stage_count(),
            _ => 0,
        }
    }

    /// The largest window one native call on this route may scan.
    pub fn max_chunk_trials(&self) -> u64 {
        match self {
            NativePivotQuery::Natural { .. } => MAX_NATURAL_TRIALS,
            NativePivotQuery::R4Primary { .. } => MAX_R4_PRIMARY_TRIALS,
            NativePivotQuery::Auxiliary { .. } => MAX_AUXILIARY_TRIALS,
            NativePivotQuery::EffectPreimage { .. } => MAX_PREIMAGE_TRIALS,
            NativePivotQuery::PrimaryPivot { .. } => MAX_PRIMARY_PIVOT_TRIALS,
        }
    }
}

/// `low16_stride` the shipped natural/R4 pivot routes use.
pub const R4_PRIMARY_LOW16_STRIDE: u16 = 0x9E37;
/// `low16_stride` the fused auxiliary route uses.
pub const AUXILIARY_LOW16_STRIDE: u16 = 0x9E37;
/// Largest window one fixed-draw pivot-family call may scan.
///
/// `effect_preimage_accelerator.collect_fixed_draw_pivot_seeds_d3d11` refuses a
/// chunk above 8,000,000 trials, and the shipped pivot-family collector
/// (`CompiledPivotFamily` with its 8,000,000-trial chunk) never exceeds it.
pub const MAX_PRIMARY_PIVOT_TRIALS: u64 = 8_000_000;

/// One bounded page request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Exclusive resume cursor; the first scanned flat index.
    pub start_after_trial: u64,
    /// Maximum number of trials this page may scan.
    pub max_trials: u64,
    /// Largest window one native call may scan; clamped to the route's ABI cap.
    pub chunk_trials: u64,
    /// Stop after this many accepted matches.
    ///
    /// This is also the page's cursor contract: the shipped complete-composition
    /// route walks the page's matches until it has published the caller's
    /// pending count and reports *that* match's trial, so a page sized by the
    /// pending count reproduces the shipped cursor exactly.
    pub page_size: usize,
}

impl PageRequest {
    /// A request that scans one chunk and stops at `page_size` matches.
    pub fn chunk(
        start_after_trial: u64,
        max_trials: u64,
        chunk_trials: u64,
        page_size: usize,
    ) -> Self {
        Self {
            start_after_trial,
            max_trials,
            chunk_trials,
            page_size,
        }
    }

    fn effective_chunk(&self, query: &NativePivotQuery) -> Result<u64, NativeSearchError> {
        if self.page_size == 0 {
            return Err(NativeSearchError::InvalidInput(
                "page_size must be positive",
            ));
        }
        if self.chunk_trials == 0 {
            return Err(NativeSearchError::InvalidInput(
                "chunk_trials must be positive",
            ));
        }
        Ok(self.chunk_trials.min(query.max_chunk_trials()))
    }
}

/// One page of native matches plus the exact accounting the report needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedPage {
    pub matches: Vec<PivotMatch>,
    /// Exclusive resume cursor: the next page scans from this flat index.
    pub next_cursor: u64,
    /// The cursor reaches the end of the pivot family.
    pub exhausted: bool,
    /// The caller's cancellation check stopped the page between chunks.
    pub cancelled: bool,
    /// Cumulative native stage counts for the scanned window.
    ///
    /// Empty for the plain natural and R4-primary routes; the fused auxiliary
    /// route reports `1 + has_terrain + enemy_groups + rule_groups` entries.
    pub stage_counts: Vec<u64>,
    /// Native natural-seed count for the scanned window (`stage_counts[0]`),
    /// zero when the route reports no stage vector.
    pub fixed_seed_count: u64,
    /// Which accelerator path served the page.
    pub backend: NativeBackend,
    /// Number of native calls this page made.
    pub native_calls: u32,
}

/// One native chunk's cumulative state, for intra-page progress reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkProgress {
    /// Flat cursor the page has inspected through.
    pub inspected_through_trial: u64,
    /// Cumulative native stage counts for the page so far.
    pub stage_counts: Vec<u64>,
    /// Matches kept by the page so far.
    pub matches: usize,
}

/// The single owner of the loaded accelerator for search.
pub struct SearchBackend {
    accelerator: Option<Arc<Accelerator>>,
    /// The verified effect-preimage helper, or the named reason it is unusable.
    ///
    /// The failure is kept rather than dropped so the route can report an absent
    /// library, a substituted artifact and a missing export distinctly.
    preimage: Result<Arc<PreimageAccelerator>, PreimageError>,
    /// The effect-preimage execution policy the current job pinned.
    pinned_preimage_policy: AtomicU8,
}

impl SearchBackend {
    /// Load the accelerator for one worker. `None` means no library passed
    /// ABI/policy/symbol validation; the worker must still answer `handshake`
    /// and must fail search closed.
    pub fn load(application_root: &Path, override_path: Option<&Path>) -> Option<Self> {
        let accelerator = Accelerator::load(application_root, override_path).map(Arc::new);
        // The seed accelerator stays the primary helper: without it no bounded
        // search can run, so the worker reports the backend as absent exactly as
        // before. The effect-preimage helper is optional per route, so its own
        // load failure is carried into the backend instead of hiding the seed
        // accelerator's state.
        accelerator.as_ref()?;
        let preimage_path = crate::capabilities::effect_preimage_path(application_root, None);
        let preimage =
            PreimageAccelerator::load(application_root, Some(&preimage_path)).map(Arc::new);
        Some(Self::new(accelerator, preimage))
    }

    /// Share one already-loaded accelerator between the query compiler and the
    /// bounded collector, so the DLL is opened once per worker.
    pub fn from_shared(accelerator: Arc<Accelerator>) -> Self {
        Self::new(
            Some(accelerator),
            Err(PreimageError::Absent(
                "this backend was built from a shared seed accelerator only, so no \
                 effect-preimage helper was loaded"
                    .to_string(),
            )),
        )
    }

    /// Share one seed accelerator and one effect-preimage helper.
    pub fn new(
        accelerator: Option<Arc<Accelerator>>,
        preimage: Result<Arc<PreimageAccelerator>, PreimageError>,
    ) -> Self {
        Self {
            accelerator,
            preimage,
            pinned_preimage_policy: AtomicU8::new(PreimagePolicy::StrictGpu.raw()),
        }
    }

    /// Pin the effect-preimage policy for one job; drop restores the previous.
    pub fn pin_preimage_policy(&self, policy: PreimagePolicy) -> PreimagePolicyGuard<'_> {
        let previous = self
            .pinned_preimage_policy
            .swap(policy.raw(), Ordering::SeqCst);
        PreimagePolicyGuard {
            backend: self,
            previous,
        }
    }

    /// The effect-preimage policy in force for the current job.
    pub fn pinned_preimage_policy(&self) -> PreimagePolicy {
        PreimagePolicy::from_raw(self.pinned_preimage_policy.load(Ordering::SeqCst))
    }

    /// The verified helper, or the named reason it is unusable.
    pub fn preimage(&self) -> Result<&Arc<PreimageAccelerator>, &PreimageError> {
        self.preimage.as_ref()
    }

    /// Real capabilities, or the absent report when nothing loaded.
    pub fn capabilities(&self) -> NativeCapabilities {
        match &self.accelerator {
            Some(accelerator) => accelerator.capabilities(),
            None => absent_capabilities(),
        }
    }

    /// Whether a library is loaded and usable.
    pub fn is_available(&self) -> bool {
        self.accelerator.is_some()
    }

    /// Pin the execution policy for one job; drop restores the previous policy.
    pub fn pin_policy(
        &self,
        policy: ExecutionPolicy,
    ) -> Result<ExecutionPolicyGuard<'_>, NativeSearchError> {
        match &self.accelerator {
            Some(accelerator) => accelerator.pin_policy(policy),
            None => Err(NativeSearchError::Unavailable),
        }
    }

    /// Apply the shipped per-seed auxiliary predicate to one batch of page
    /// matches, returning one verdict per seed in input order.
    ///
    /// This is the placement the shipped worker uses before it composes
    /// anything: the native terrain/enemy/rule matchers decide the caller's
    /// auxiliary criteria for a whole batch of candidate seeds. The job layer
    /// still verifies every accepted candidate against the composed auxiliary
    /// output, so this can only remove seeds the native matcher already
    /// rejected, never admit one.
    pub fn auxiliary_criteria_selected(
        &self,
        spec: &AuxiliaryPivotSpec,
        seeds: &[u32],
    ) -> Result<Vec<bool>, NativeSearchError> {
        let Some(accelerator) = self.accelerator.as_deref() else {
            return Err(NativeSearchError::Unavailable);
        };
        // The DLL keeps process-global diagnostics and scratch buffers, so the
        // predicate takes the same lock a page collection does.
        let _call_lock = native_call_lock();
        accelerator.auxiliary_criteria_selected(spec, seeds)
    }

    /// Collect one bounded, non-overlapping page.
    ///
    /// Cancellation is honoured between native chunks, so the caller never
    /// waits for a whole family scan. The policy pinned by the caller applies;
    /// with no pin held this pins strict GPU for the duration of the call.
    pub fn collect_page(
        &self,
        query: &NativePivotQuery,
        request: &PageRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CollectedPage, NativeSearchError> {
        self.collect_page_with_progress(query, request, cancelled, &mut |_| {})
    }

    /// Collect one bounded page, reporting cumulative progress after each
    /// native chunk.
    ///
    /// The job layer uses this so a 100,000,000-trial page publishes progress
    /// instead of going silent until the last chunk returns.
    pub fn collect_page_with_progress(
        &self,
        query: &NativePivotQuery,
        request: &PageRequest,
        cancelled: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&ChunkProgress),
    ) -> Result<CollectedPage, NativeSearchError> {
        self.collect_page_filtered(query, request, cancelled, progress, None)
    }

    /// Collect one bounded page whose `page_size` counts accepted matches.
    ///
    /// `filter` holds the shipped predicates for routes whose pivot does not
    /// already pack the caller's criteria. With a filter, a raw native match is
    /// decided *before* it counts towards `page_size`, which is the placement
    /// the shipped solver uses: a page ends when it has `page_size` accepted
    /// candidates or when the trial budget is exhausted, so a page whose
    /// candidates are all rejected still covers its whole window in one scan.
    /// Without a filter the raw matches are the accepted ones.
    pub fn collect_page_filtered(
        &self,
        query: &NativePivotQuery,
        request: &PageRequest,
        cancelled: &dyn Fn() -> bool,
        progress: &mut dyn FnMut(&ChunkProgress),
        filter: Option<&MatchFilter<'_>>,
    ) -> Result<CollectedPage, NativeSearchError> {
        let Some(accelerator) = self.accelerator.as_deref() else {
            return Err(NativeSearchError::Unavailable);
        };
        let chunk = request.effective_chunk(query)?;
        let family_size = query.family_size();
        let start = request.start_after_trial.min(family_size);
        let budget_stop = start.saturating_add(request.max_trials).min(family_size);
        let mut stage_counts = vec![0u64; query.stage_count()];
        let mut matches: Vec<PivotMatch> = Vec::new();
        let mut cursor = start;
        let mut cancelled_observed = false;
        let mut backend = NativeBackend::NotUsed;
        let mut native_calls: u32 = 0;

        // One pin per call when the caller holds none. `pin_policy` is
        // re-entrant, so a job-scoped guard stays in force.
        let _inner_pin = if accelerator.pinned_policy() == ExecutionPolicy::StrictGpu {
            Some(accelerator.pin_policy(ExecutionPolicy::StrictGpu)?)
        } else {
            None
        };
        let _call_lock = native_call_lock();

        while matches.len() < request.page_size && cursor < budget_stop {
            if cancelled() {
                cancelled_observed = true;
                break;
            }
            let stop = budget_stop.min(cursor.saturating_add(chunk));
            let window = PivotWindow {
                start_index: cursor,
                stop_index: stop,
                low16_stride: query.low16_stride(),
                draw_index: query.draw_index(),
            };
            // Keep the cumulative total before this chunk. If the page ends
            // inside the chunk, the exact-prefix recount replaces only this
            // chunk's contribution, not the totals from earlier full chunks.
            let counts_before_chunk = stage_counts.clone();
            let (page, counts) = collect_window(
                accelerator,
                self.preimage(),
                self.pinned_preimage_policy(),
                query,
                &window,
                request.page_size,
            )?;
            native_calls += 1;
            backend = page.backend;
            for matched in &page.matches {
                verify_window_match(query, &window, *matched)?;
            }
            accumulate(&mut stage_counts, &counts)?;

            let filtered = match filter {
                Some(filter) => filter_matches(
                    accelerator,
                    self.preimage(),
                    self.pinned_preimage_policy(),
                    filter,
                    &page.matches,
                    request
                        .page_size
                        .saturating_sub(matches.len())
                        .saturating_add(1),
                    cancelled,
                )?,
                None => FilteredMatches {
                    matches: page.matches,
                    stopped_at: None,
                },
            };
            if let Some(stopped_at) = filtered.stopped_at {
                // Cancellation was observed while the window was being decided.
                // The page keeps every accepted match and reports the exclusive
                // cursor it actually decided through, so a resume re-decides the
                // interrupted raw match and can neither replay a published
                // candidate nor skip an undecided one.
                matches.extend(filtered.matches);
                cursor = stopped_at;
                cancelled_observed = true;
                progress(&ChunkProgress {
                    inspected_through_trial: cursor,
                    stage_counts: stage_counts.clone(),
                    matches: matches.len(),
                });
                break;
            }
            let accepted = filtered.matches;
            let remaining = request.page_size - matches.len();
            if accepted.len() > remaining {
                // The preimage window already reports its verified matches in
                // trial order and never reports native stage counts, so the page
                // can end on the last published candidate directly. Re-sweeping a
                // narrower window here would double the GPU work for no gain,
                // because the acceptance decision is the certified recomposition,
                // not a window-bounded recount.
                if matches!(query, NativePivotQuery::EffectPreimage { .. }) {
                    cursor = accepted[remaining - 1].trial;
                    matches.extend(accepted.into_iter().take(remaining));
                    progress(&ChunkProgress {
                        inspected_through_trial: cursor,
                        stage_counts: stage_counts.clone(),
                        matches: matches.len(),
                    });
                    break;
                }
                // Recount the exact prefix ending at the last returned result so
                // pagination and every displayed intersection count stay exact.
                let cut = accepted[remaining - 1].trial;
                let mut recount_window = window;
                recount_window.stop_index = cut;
                let (recount, recount_counts) = collect_window(
                    accelerator,
                    self.preimage(),
                    self.pinned_preimage_policy(),
                    query,
                    &recount_window,
                    request.page_size,
                )?;
                native_calls += 1;
                for matched in &recount.matches {
                    verify_window_match(query, &recount_window, *matched)?;
                }
                // The recount is the bounded exact prefix that proves the cut,
                // so it always runs to completion; a cancellation that lands
                // inside it is observed by the caller's own check after the
                // page returns, exactly like a chunk boundary.
                let recount_accepted = match filter {
                    Some(filter) => {
                        filter_matches(
                            accelerator,
                            self.preimage(),
                            self.pinned_preimage_policy(),
                            filter,
                            &recount.matches,
                            request
                                .page_size
                                .saturating_sub(matches.len())
                                .saturating_add(1),
                            &|| false,
                        )?
                        .matches
                    }
                    None => recount.matches,
                };
                if recount_accepted.len() != remaining {
                    return Err(NativeSearchError::Rejected {
                        call: "collect_auxiliary_pivot_matches",
                    });
                }
                stage_counts = counts_before_chunk;
                accumulate(&mut stage_counts, &recount_counts)?;
                matches.extend(recount_accepted);
                cursor = cut;
                progress(&ChunkProgress {
                    inspected_through_trial: cursor,
                    stage_counts: stage_counts.clone(),
                    matches: matches.len(),
                });
                break;
            }
            matches.extend(accepted);
            cursor = stop;
            progress(&ChunkProgress {
                inspected_through_trial: cursor,
                stage_counts: stage_counts.clone(),
                matches: matches.len(),
            });
        }

        let exhausted = cancelled_observed || cursor >= family_size;
        Ok(CollectedPage {
            matches,
            next_cursor: cursor,
            exhausted,
            cancelled: cancelled_observed,
            fixed_seed_count: stage_counts.first().copied().unwrap_or(0),
            stage_counts,
            backend,
            native_calls,
        })
    }

    /// Derive the seed a 1-based trial must produce, using the domain LCG
    /// inverse the shipped ABI uses.
    pub fn replay_pivot_seed(
        values: &[u16],
        window: &PivotWindow,
        trial: u64,
    ) -> Result<u32, NativeSearchError> {
        if values.is_empty() {
            return Err(NativeSearchError::InvalidInput(
                "invalid native pivot range",
            ));
        }
        let flat_index = trial.checked_sub(1).ok_or(NativeSearchError::InvalidInput(
            "invalid native pivot range",
        ))?;
        let value_count = values.len() as u64;
        let low_index = flat_index / value_count;
        let bucket_index = (flat_index % value_count) as u32;
        let low16 = (low_index as u32).wrapping_mul(u32::from(window.low16_stride)) as u16;
        let rotation = (low_index % value_count) as u32;
        let high16 = values[((rotation + bucket_index) % value_count as u32) as usize];
        let mut seed = (u32::from(high16) << 16) | u32::from(low16);
        for _ in 0..window.draw_index {
            seed = A_INV.wrapping_mul(seed.wrapping_sub(1));
        }
        Ok(seed)
    }

    /// Test-only hook: force the native CUDA path to fail.
    ///
    /// Exists so the strict-GPU refusal and the bulk-CPU opt-in can be verified
    /// on a CUDA-capable machine without touching the accelerator binary.
    #[doc(hidden)]
    pub fn force_cuda_failure(&self, enabled: bool) {
        if let Some(accelerator) = &self.accelerator {
            accelerator.force_cuda_failure(enabled);
        }
    }

    /// Test-only hook: how often the native bulk CPU path actually ran.
    #[doc(hidden)]
    pub fn bulk_cpu_call_count(&self) -> u64 {
        self.accelerator
            .as_ref()
            .map_or(0, |accelerator| accelerator.bulk_cpu_call_count())
    }
}

impl NativePivotQuery {
    fn draw_index(&self) -> u32 {
        match self {
            NativePivotQuery::Natural { .. } => 1,
            NativePivotQuery::R4Primary { .. } => 1,
            NativePivotQuery::Auxiliary { spec, .. } => spec.draw_index,
            NativePivotQuery::EffectPreimage { plans, .. } => {
                plans.first().map_or(1, |plan| plan.pivot_draw_index)
            }
            // Each family carries its own pivot draw; the first family's is the
            // declared draw of the shipped combined cursor.
            NativePivotQuery::PrimaryPivot { families } => families
                .first()
                .map_or(1, |family| family.params.pivot_draw_index),
        }
    }
}

/// Restores the previous effect-preimage policy when the job ends.
pub struct PreimagePolicyGuard<'a> {
    backend: &'a SearchBackend,
    previous: u8,
}

impl Drop for PreimagePolicyGuard<'_> {
    fn drop(&mut self) {
        self.backend
            .pinned_preimage_policy
            .store(self.previous, Ordering::SeqCst);
    }
}

/// The predicates a page applies before a raw native match counts as accepted.
///
/// Order matters and mirrors the shipped solver: the batched primary-effect
/// predicate runs first, then the auxiliary criteria are decided only for its
/// survivors (`effect_seed_solver._iter_solution_prefetch` eligibility).
///
/// The partial-effect forward filter adds two more stages: the accelerator's
/// batched constraint mask (a necessary predicate) and then the certified
/// recomposition, which is the route's actual acceptance decision.
#[derive(Clone, Copy)]
pub struct MatchFilter<'a> {
    pub primary: Option<&'a PrimaryEffectSpec>,
    pub auxiliary: Option<&'a AuxiliaryPivotSpec>,
    /// `match_effect_constraints_d3d11` over the surviving Seeds.
    pub effect_mask: Option<&'a EffectMaskSpec>,
    /// The certified composition gate every surviving Seed must pass.
    pub effect_verifier: Option<&'a PartialEffectVerifier>,
}

/// The outcome of deciding one window's raw native matches.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FilteredMatches {
    /// The accepted matches, in trial order, never more than the caller's limit.
    matches: Vec<PivotMatch>,
    /// `Some(cursor)` when cancellation stopped the decision: the exclusive
    /// cursor the page has really decided through. `None` when the whole window
    /// was decided.
    stopped_at: Option<u64>,
}

/// Decide one window's raw native matches with the shipped predicates.
///
/// Runs inside the page loop, so the caller already holds the native call lock
/// and passes the accelerator directly.
///
/// The window is decided in `PREDICATE_BATCH_SIZE` slices, exactly like the
/// shipped `_iter_solution_prefetch` batches, and `cancelled` is polled before
/// every slice and before every survivor's recomposition. That placement is the
/// shipped one: a cancellation never waits for a whole window's verification,
/// and the checkpoint it produces is the exclusive cursor of the last decided
/// raw match, so a resume re-decides the interrupted match instead of replaying
/// a published candidate or skipping an undecided one.
fn filter_matches(
    accelerator: &Accelerator,
    preimage: Result<&Arc<PreimageAccelerator>, &PreimageError>,
    policy: PreimagePolicy,
    filter: &MatchFilter<'_>,
    raw: &[PivotMatch],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<FilteredMatches, NativeSearchError> {
    let mut accepted: Vec<PivotMatch> = Vec::with_capacity(limit.min(raw.len()));
    if raw.is_empty() || limit == 0 {
        return Ok(FilteredMatches {
            matches: accepted,
            stopped_at: None,
        });
    }
    let mut index = 0usize;
    while index < raw.len() && accepted.len() < limit {
        if cancelled() {
            return Ok(FilteredMatches {
                matches: accepted,
                stopped_at: Some(raw[index].trial - 1),
            });
        }
        let batch_end = (index + PREDICATE_BATCH_SIZE).min(raw.len());
        let mut kept: Vec<PivotMatch> = raw[index..batch_end].to_vec();
        if let Some(mask) = filter.effect_mask {
            kept = effect_mask_matches(preimage, policy, mask, kept)?;
        }
        if let Some(primary) = filter.primary {
            if !kept.is_empty() {
                let seeds: Vec<u32> = kept.iter().map(|matched| matched.seed).collect();
                let selected = accelerator.primary_effect_selected(primary, &seeds)?;
                kept = kept
                    .into_iter()
                    .zip(selected)
                    .filter_map(|(matched, keep)| keep.then_some(matched))
                    .collect();
            }
        }
        if let Some(auxiliary) = filter.auxiliary {
            if !kept.is_empty() {
                let seeds: Vec<u32> = kept.iter().map(|matched| matched.seed).collect();
                let selected = accelerator.auxiliary_criteria_selected(auxiliary, &seeds)?;
                kept = kept
                    .into_iter()
                    .zip(selected)
                    .filter_map(|(matched, keep)| keep.then_some(matched))
                    .collect();
            }
        }
        // The native mask is never the acceptance decision: every survivor is
        // re-composed with the certified generator and re-checked against every
        // query criterion. A composition failure is an error, not a rejection.
        //
        // The recomposition is the expensive stage, so it walks the survivors in
        // trial order and stops at `limit`: the page only needs the first
        // `page_size` accepted matches, and one more to prove it must cut the
        // cursor at an accepted trial. Without this bound a bounded page pays
        // for every survivor of its whole window.
        for matched in kept {
            if cancelled() {
                return Ok(FilteredMatches {
                    matches: accepted,
                    stopped_at: Some(matched.trial - 1),
                });
            }
            let keep = match filter.effect_verifier {
                Some(verifier) => verifier
                    .accepts(matched.seed)
                    .map_err(|error| map_effect_path_error(&error))?,
                None => true,
            };
            if keep {
                accepted.push(matched);
                if accepted.len() >= limit {
                    break;
                }
            }
        }
        index = batch_end;
    }
    Ok(FilteredMatches {
        matches: accepted,
        stopped_at: None,
    })
}

/// Apply the accelerator's batched forward filter to the surviving Seeds.
///
/// The mask is a necessary predicate, so a Seed it rejects can be dropped here
/// without changing the result set. Under the strict-GPU policy a missing
/// backend is a named refusal; under the explicit CPU opt-in the filter is
/// skipped and the certified recomposition still decides every Seed, which is
/// exactly the shipped `allow_cpu_fallback` behaviour.
fn effect_mask_matches(
    preimage: Result<&Arc<PreimageAccelerator>, &PreimageError>,
    policy: PreimagePolicy,
    mask: &EffectMaskSpec,
    matches: Vec<PivotMatch>,
) -> Result<Vec<PivotMatch>, NativeSearchError> {
    let strict = matches!(policy, PreimagePolicy::StrictGpu);
    let accelerator = match preimage {
        Ok(accelerator) => accelerator,
        Err(error) => {
            return if strict {
                Err(NativeSearchError::PreimageUnavailable(error.to_string()))
            } else {
                Ok(matches)
            }
        }
    };
    if accelerator.require_backend(policy).is_err() {
        return if strict {
            Err(NativeSearchError::PreimageUnavailable(
                accelerator
                    .require_backend(policy)
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "no DirectCompute backend".to_string()),
            ))
        } else {
            Ok(matches)
        };
    }
    let vendor = accelerator.configured_vendor_id();
    let mut kept: Vec<PivotMatch> = Vec::with_capacity(matches.len());
    for chunk in matches.chunks(PREDICATE_BATCH_SIZE) {
        let seeds: Vec<u32> = chunk.iter().map(|matched| matched.seed).collect();
        let request = mask.request(&seeds, vendor);
        let masks = accelerator
            .match_effect_constraints(&request)
            .map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
        match masks {
            Some(masks) => {
                for (matched, observed) in chunk.iter().zip(masks) {
                    if mask.accepts_mask(observed) {
                        kept.push(*matched);
                    }
                }
            }
            // The library reported no backend for this call. The strict policy
            // must not quietly continue; the explicit opt-in falls back to the
            // certified composition alone, like the shipped worker.
            None if strict => {
                return Err(NativeSearchError::PreimageUnavailable(
                    "the DirectCompute partial-effect matcher reported no usable backend"
                        .to_string(),
                ))
            }
            None => kept.extend(chunk.iter().copied()),
        }
    }
    Ok(kept)
}

/// Exact replay check: a returned match must reproduce from its own cursor.
pub fn verify_pivot_match(
    values: &[u16],
    window: &PivotWindow,
    matched: PivotMatch,
) -> Result<(), NativeSearchError> {
    if matched.trial <= window.start_index || matched.trial > window.stop_index {
        return Err(NativeSearchError::InvalidInput(
            "native match outside the scanned window",
        ));
    }
    let expected = SearchBackend::replay_pivot_seed(values, window, matched.trial)?;
    if expected != matched.seed {
        return Err(NativeSearchError::Rejected {
            call: "native_pivot_replay",
        });
    }
    Ok(())
}

/// The acceptance check for one returned match, per route.
///
/// The cursor-replay routes re-derive the Seed from the trial they report, so a
/// wrong cursor can never be published. The effect-preimage route instead
/// accepts a Seed only after the certified forward generator re-composed it
/// inside [`collect_window`], which is the same placement the shipped Python page
/// uses, so only the window bounds remain to re-check here.
fn verify_window_match(
    query: &NativePivotQuery,
    window: &PivotWindow,
    matched: PivotMatch,
) -> Result<(), NativeSearchError> {
    if matched.trial <= window.start_index || matched.trial > window.stop_index {
        return Err(NativeSearchError::InvalidInput(
            "native match outside the scanned window",
        ));
    }
    match query {
        NativePivotQuery::EffectPreimage { .. } => Ok(()),
        NativePivotQuery::PrimaryPivot { families } => {
            let expected = replay_primary_pivot_seed(families, matched.trial)?;
            if expected != matched.seed {
                return Err(NativeSearchError::Rejected {
                    call: "native_primary_pivot_replay",
                });
            }
            Ok(())
        }
        other => verify_pivot_match(other.values(), window, matched),
    }
}

/// Derive the Seed a one-based global trial must produce on the pivot-family
/// route, using the domain LCG inverse the shipped ABI uses.
///
/// The fixed-draw collector's cursor is pivot-value-major: for a zero-based
/// family-local trial the bucket index is `trial / 65,536` into the family's
/// ascending bucket list and the minor key is `trial % 65,536` (the state's
/// low sixteen bits). The state at the family's pivot draw is therefore
/// reconstructed exactly, and the Seed is that state inverted back through the
/// LCG.
pub fn replay_primary_pivot_seed(
    families: &[PrimaryPivotFamilySpec],
    trial: u64,
) -> Result<u32, NativeSearchError> {
    if trial == 0 {
        return Err(NativeSearchError::InvalidInput(
            "invalid native pivot range",
        ));
    }
    let mut offset: u64 = 0;
    for family in families {
        let family_size = family.pivot_state_count();
        if trial > offset && trial <= offset + family_size {
            let local = trial - offset - 1;
            let index = (local / 0x1_0000) as usize;
            let low16 = (local % 0x1_0000) as u32;
            let Some(high16) = family.values.get(index).copied() else {
                return Err(NativeSearchError::InvalidInput(
                    "invalid native pivot range",
                ));
            };
            let mut state = (u32::from(high16) << 16) | low16;
            for _ in 0..family.params.pivot_draw_index {
                state = A_INV.wrapping_mul(state.wrapping_sub(1));
            }
            return Ok(state);
        }
        offset += family_size;
    }
    Err(NativeSearchError::InvalidInput(
        "native match outside the compiled pivot families",
    ))
}

/// Map one plan-compiler failure onto the search error the collector reports.
fn map_effect_path_error(error: &EffectPathError) -> NativeSearchError {
    NativeSearchError::PreimageUnavailable(format!("effect-preimage plan: {error}"))
}

/// One preimage window: the concatenated plan families the window covers.
///
/// `window` is expressed in the route's cursor space, so every plan is clipped
/// to the window and swept with its own pivot parameters and packed paths. Each
/// returned Seed is re-composed by the certified generator before it becomes a
/// match, and the reported trial is `plan_offset + local trial + 1`, which is
/// exactly the shipped page's one-based cursor.
fn collect_preimage_window(
    preimage: Result<&Arc<PreimageAccelerator>, &PreimageError>,
    policy: PreimagePolicy,
    plans: &[CompiledEffectPlan],
    verifier: &PreimageVerifier,
    window: &PivotWindow,
    page_size: usize,
) -> Result<Vec<PivotMatch>, NativeSearchError> {
    let accelerator =
        preimage.map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
    accelerator
        .require_backend(policy)
        .map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
    let capacity = page_size
        .saturating_mul(8)
        .clamp(DEFAULT_OUTPUT_CAPACITY, MAX_OUTPUT_CAPACITY);
    let mut matches: Vec<PivotMatch> = Vec::new();
    let mut offset: u64 = 0;
    for plan in plans {
        let plan_size = plan.pivot_state_count();
        let plan_start = offset;
        let plan_stop = offset + plan_size;
        offset = plan_stop;
        let local_start = window.start_index.max(plan_start);
        let local_stop = window.stop_index.min(plan_stop);
        if local_start >= local_stop {
            continue;
        }
        let descriptors =
            native_path_descriptors(plan).map_err(|error| map_effect_path_error(&error))?;
        let pivot_values: Vec<u16> = plan
            .pivot_allowed_u16
            .iter()
            .flat_map(|run| run.start..=run.end)
            .collect();
        let maximum_draw = plan
            .paths
            .iter()
            .flat_map(|path| path.constraints.iter().map(|item| item.draw_index))
            .max()
            .unwrap_or(plan.promotion_draw_index);
        let params = PreimagePlanParams {
            pivot_draw_index: plan.pivot_draw_index,
            pivot_affine_addend: plan.pivot_affine_addend,
            pivot_inverse_multiplier: plan.pivot_inverse_multiplier,
            promotion_draw_index: plan.promotion_draw_index,
            promotion_probability_percent: plan.promotion_probability_percent,
            shuffle_draw_start: plan.shuffle_draw_start,
            rarity: plan.request.rarity(),
            slot_limit: plan.slot_limit,
            maximum_draw,
        };
        let hits = accelerator
            .collect_matches(
                &pivot_values,
                &descriptors,
                &params,
                local_start - plan_start,
                local_stop - plan_start,
                capacity,
                accelerator.configured_vendor_id(),
            )
            .map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
        for (seed, local_trial) in hits {
            if !verifier
                .accepts(seed)
                .map_err(|error| map_effect_path_error(&error))?
            {
                continue;
            }
            matches.push(PivotMatch {
                seed,
                trial: plan_start + local_trial + 1,
            });
        }
    }
    matches.sort_by_key(|matched| matched.trial);
    Ok(matches)
}

/// One pivot-family window: the concatenated families the window covers.
///
/// `window` is expressed in the route's global cursor space, so every family
/// is clipped to the window and swept with its own pivot parameters and
/// promotion descriptors. The reported trial is `family_offset + local trial`,
/// which is exactly the shipped one-based `CompiledPivotFamily` cursor, and the
/// result capacity per family call is the shipped
/// `max(100,000, ceil(window / 8))`, so a window can never be truncated below
/// the shipped route's own bound.
fn collect_primary_pivot_window(
    preimage: Result<&Arc<PreimageAccelerator>, &PreimageError>,
    policy: PreimagePolicy,
    families: &[PrimaryPivotFamilySpec],
    window: &PivotWindow,
    _page_size: usize,
) -> Result<Vec<PivotMatch>, NativeSearchError> {
    let accelerator =
        preimage.map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
    accelerator
        .require_backend(policy)
        .map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
    let mut matches: Vec<PivotMatch> = Vec::new();
    let mut offset: u64 = 0;
    for family in families {
        let family_size = family.pivot_state_count();
        let family_start = offset;
        let family_stop = offset + family_size;
        offset = family_stop;
        let local_start = window.start_index.max(family_start);
        let local_stop = window.stop_index.min(family_stop);
        if local_start >= local_stop {
            continue;
        }
        let span = local_stop - local_start;
        let capacity = span
            .div_ceil(8)
            .clamp(DEFAULT_OUTPUT_CAPACITY as u64, MAX_OUTPUT_CAPACITY as u64)
            as usize;
        let hits = accelerator
            .collect_matches(
                &family.values,
                &family.descriptors,
                &family.params,
                local_start - family_start,
                local_stop - family_start,
                capacity,
                accelerator.configured_vendor_id(),
            )
            .map_err(|error| NativeSearchError::PreimageUnavailable(error.to_string()))?;
        for (seed, local_trial) in hits {
            matches.push(PivotMatch {
                seed,
                trial: family_start + local_trial + 1,
            });
        }
    }
    matches.sort_by_key(|matched| matched.trial);
    Ok(matches)
}

fn collect_window(
    accelerator: &Accelerator,
    preimage: Result<&Arc<PreimageAccelerator>, &PreimageError>,
    policy: PreimagePolicy,
    query: &NativePivotQuery,
    window: &PivotWindow,
    page_size: usize,
) -> Result<(crate::native_search::AuxiliaryPivotPage, Vec<u64>), NativeSearchError> {
    match query {
        NativePivotQuery::Natural { values } => {
            let matches = accelerator.collect_natural_pivot_page(values, *window)?;
            Ok((
                crate::native_search::AuxiliaryPivotPage {
                    matches,
                    stage_counts: Vec::new(),
                    backend: accelerator.last_backend(),
                },
                Vec::new(),
            ))
        }
        NativePivotQuery::R4Primary { values, spec } => {
            let matches = accelerator.collect_r4_primary_pivot_page(values, spec, *window)?;
            Ok((
                crate::native_search::AuxiliaryPivotPage {
                    matches,
                    stage_counts: Vec::new(),
                    backend: accelerator.last_backend(),
                },
                Vec::new(),
            ))
        }
        NativePivotQuery::Auxiliary { values, spec } => {
            let page = accelerator.collect_auxiliary_pivot_page(
                values,
                spec,
                *window,
                DEFAULT_AUXILIARY_OUTPUT_CAPACITY,
            )?;
            let counts = page.stage_counts.clone();
            Ok((page, counts))
        }
        NativePivotQuery::EffectPreimage { plans, verifier } => {
            let matches =
                collect_preimage_window(preimage, policy, plans, verifier, window, page_size)?;
            let counts = Vec::new();
            Ok((
                crate::native_search::AuxiliaryPivotPage {
                    matches,
                    stage_counts: counts.clone(),
                    // The seed accelerator did not run for this route; the
                    // accelerator that did is reported by
                    // `PreimageAccelerator::last_backend()`.
                    backend: NativeBackend::NotUsed,
                },
                counts,
            ))
        }
        NativePivotQuery::PrimaryPivot { families } => {
            let matches =
                collect_primary_pivot_window(preimage, policy, families, window, page_size)?;
            let counts = Vec::new();
            Ok((
                crate::native_search::AuxiliaryPivotPage {
                    matches,
                    stage_counts: counts.clone(),
                    // The seed accelerator did not run for this route; the
                    // accelerator that did is reported by
                    // `PreimageAccelerator::last_backend()`.
                    backend: NativeBackend::NotUsed,
                },
                counts,
            ))
        }
    }
}

fn accumulate(total: &mut [u64], counts: &[u64]) -> Result<(), NativeSearchError> {
    if counts.len() != total.len() {
        return Err(NativeSearchError::Rejected {
            call: "native_auxiliary_stage_layout",
        });
    }
    for (slot, value) in total.iter_mut().zip(counts) {
        *slot += *value;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{mpsc, Arc, Mutex, MutexGuard};
    use std::time::Duration;

    use super::*;
    use crate::native_search::SEED_ACCELERATOR_ABI_VERSION;

    const VALUES: [u16; 5] = [0x1234, 0xABCD, 0x0001, 0xFFFE, 0x00FF];
    const STRIDE: u16 = 0x9E37;

    fn lock() -> MutexGuard<'static, ()> {
        crate::accelerator_test_lock()
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn shipped_dll() -> PathBuf {
        repo_root().join("bin").join("nioh3_seed_accelerator.dll")
    }

    fn loaded() -> SearchBackend {
        SearchBackend::load(&repo_root(), None).expect("shipped accelerator must load")
    }

    fn accelerator(backend: &SearchBackend) -> &Accelerator {
        backend
            .accelerator
            .as_deref()
            .expect("loaded backend has an accelerator")
    }

    fn assert_direct_policy(backend: &SearchBackend, expected: ExecutionPolicy) {
        let accelerator = accelerator(backend);
        assert_eq!(accelerator.pinned_policy(), expected);
        accelerator.force_cuda_failure(true);
        let result = accelerator.collect_natural_pivot_page(&VALUES, window(0, 2_000));
        let served_by = accelerator.last_backend();
        accelerator.force_cuda_failure(false);
        match expected {
            ExecutionPolicy::StrictGpu => assert!(matches!(
                result,
                Err(NativeSearchError::CudaUnavailable { .. })
            )),
            ExecutionPolicy::AllowBulkCpu => {
                result.expect("bulk CPU policy must accept the forced CUDA fallback");
                assert_eq!(served_by, NativeBackend::NativeCpu);
            }
        }
    }

    /// Independent bounded oracle for the pivot cursor algebra: enumerate the
    /// window and keep the natural seeds the native collector must find.
    fn local_natural_matches(
        values: &[u16],
        start: u64,
        stop: u64,
        stride: u16,
        draw: u32,
    ) -> Vec<PivotMatch> {
        let count = values.len() as u64;
        let mut matches = Vec::new();
        for flat_index in start..stop {
            let low_index = flat_index / count;
            let bucket_index = (flat_index % count) as u32;
            let low16 = (low_index as u32).wrapping_mul(u32::from(stride)) as u16;
            let rotation = (low_index % count) as u32;
            let high16 = values[((rotation + bucket_index) % count as u32) as usize];
            let mut seed = (u32::from(high16) << 16) | u32::from(low16);
            for _ in 0..draw {
                seed = A_INV.wrapping_mul(seed.wrapping_sub(1));
            }
            if seed & 0xF000_0000 == 0 && seed & 0xFFFF != 0 {
                matches.push(PivotMatch {
                    seed,
                    trial: flat_index + 1,
                });
            }
        }
        matches
    }

    fn synthetic_auxiliary_spec() -> AuxiliaryPivotSpec {
        AuxiliaryPivotSpec {
            draw_index: 1,
            playthrough: 3,
            mode_threshold: 0,
            filtered_terrain_rows: vec![0],
            terrain_row_count: 1,
            allowed_terrain_rows: vec![1],
            has_terrain_constraint: false,
            descriptor_thresholds: [0, 0, 0],
            selector_threshold: 0,
            role_five_threshold: 0,
            selector_value: 0,
            enemy_rows: vec![0u8; crate::native_search::ENEMY_ROW_BYTES],
            terrains: vec![0u8; crate::native_search::TERRAIN_ROW_BYTES],
            contexts: vec![0u8; crate::native_search::CONTEXT_ROW_BYTES],
            enemy_criterion_groups: Vec::new(),
            enemy_group_count: 0,
            scratch_group_count: 0,
            rule_rows: Vec::new(),
            rule_criterion_groups: Vec::new(),
        }
    }

    #[test]
    fn probes_real_identity_and_capabilities() {
        let _lock = lock();
        assert!(shipped_dll().is_file(), "shipped accelerator missing");
        let backend = loaded();
        let capabilities = backend.capabilities();
        assert!(capabilities.available);
        assert_eq!(capabilities.abi, SEED_ACCELERATOR_ABI_VERSION);
        assert!(
            capabilities.build_id.starts_with("sha256:"),
            "unexpected build id {}",
            capabilities.build_id
        );
        assert!(capabilities.bulk_cpu_requires_opt_in);
        assert_eq!(capabilities.directcompute_effect_filter, None);

        // The CUDA flag is a real probe: when it is true a strict scan must
        // work, and when it is false a strict scan must fail closed.
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let request = PageRequest::chunk(0, 2_000, 2_000, 10_000);
        let strict = backend.collect_page(&query, &request, &|| false);
        if capabilities.cuda_seed_acceleration {
            let page = strict.expect("probed CUDA must serve a strict scan");
            assert_eq!(page.backend, NativeBackend::Cuda);
        } else {
            assert!(matches!(
                strict,
                Err(NativeSearchError::CudaUnavailable { .. })
            ));
        }
    }

    #[test]
    fn absent_accelerator_is_supported_and_never_panics() {
        let _lock = lock();
        let missing = repo_root()
            .join("bin")
            .join("nioh3_seed_accelerator.absent.dll");
        assert!(SearchBackend::load(&repo_root(), Some(&missing)).is_none());
        let capabilities = crate::native_search::absent_capabilities();
        assert!(!capabilities.available);
        assert!(!capabilities.cuda_seed_acceleration);
        assert!(capabilities.bulk_cpu_requires_opt_in);
        assert_eq!(capabilities.directcompute_effect_filter, None);
    }

    #[test]
    fn natural_page_equals_bounded_local_oracle() {
        let _lock = lock();
        let backend = loaded();
        let _pin = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("policy pin");
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let request = PageRequest::chunk(0, 20_000, 20_000, 100_000);
        let page = backend
            .collect_page(&query, &request, &|| false)
            .expect("natural page");
        assert_eq!(
            page.matches,
            local_natural_matches(&VALUES, 0, 20_000, STRIDE, 1)
        );
        assert_eq!(page.next_cursor, 20_000);
        assert!(!page.exhausted);
        assert!(page.stage_counts.is_empty());
        assert!(page
            .matches
            .windows(2)
            .all(|pair| pair[0].trial < pair[1].trial));
    }

    #[test]
    fn natural_page_resumes_exactly_and_truncates_at_the_cursor() {
        let _lock = lock();
        let backend = loaded();
        let _pin = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("policy pin");
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let whole = backend
            .collect_page(
                &query,
                &PageRequest::chunk(0, 40_000, 40_000, 100_000),
                &|| false,
            )
            .expect("whole window");
        assert_eq!(
            whole.matches,
            local_natural_matches(&VALUES, 0, 40_000, STRIDE, 1)
        );
        let small = backend
            .collect_page(&query, &PageRequest::chunk(0, 40_000, 40_000, 10), &|| {
                false
            })
            .expect("truncated page");
        assert_eq!(small.matches.len(), 10);
        assert_eq!(small.matches, whole.matches[..10].to_vec());
        assert_eq!(small.next_cursor, small.matches[9].trial);

        let resume = small.next_cursor;
        assert_eq!(
            resume,
            small.matches[small.matches.len() - 1].trial,
            "cursor must sit on the last returned trial"
        );
        let resumed = backend
            .collect_page(
                &query,
                &PageRequest::chunk(resume, 40_000, 40_000, 100_000),
                &|| false,
            )
            .expect("resumed page");
        assert_eq!(
            resumed.matches,
            local_natural_matches(&VALUES, resume, resume + 40_000, STRIDE, 1)
        );
        assert!(resumed.matches.first().map_or(0, |item| item.trial) > small.next_cursor);
    }

    #[test]
    fn cancellation_between_chunks_stops_early() {
        let _lock = lock();
        let backend = loaded();
        let _pin = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("policy pin");
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let polls = Mutex::new(0usize);
        let request = PageRequest::chunk(0, 100_000, 5_000, 100_000);
        let page = backend
            .collect_page(&query, &request, &|| {
                let mut count = polls.lock().unwrap();
                *count += 1;
                *count > 1
            })
            .expect("cancelled page");
        assert!(page.cancelled);
        assert_eq!(page.native_calls, 1);
        assert_eq!(page.next_cursor, 5_000);
        assert!(page.exhausted);
        assert_eq!(
            page.matches,
            local_natural_matches(&VALUES, 0, 5_000, STRIDE, 1)
        );
    }

    #[test]
    fn fused_auxiliary_page_reports_native_stage_counts() {
        let _lock = lock();
        let backend = loaded();
        let _pin = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("policy pin");
        let query = NativePivotQuery::Auxiliary {
            values: VALUES.to_vec(),
            spec: synthetic_auxiliary_spec(),
        };
        let request = PageRequest::chunk(0, 20_000, 20_000, 100_000);
        let page = backend
            .collect_page(&query, &request, &|| false)
            .expect("fused auxiliary page");
        assert_eq!(page.stage_counts.len(), 1);
        let mut spec = synthetic_auxiliary_spec();
        spec.scratch_group_count = 1;
        spec.enemy_criterion_groups = vec![vec![0x1234u32]];
        spec.rule_rows = vec![0u8; crate::native_search::RULE_ROW_BYTES];
        spec.rule_criterion_groups = vec![vec![0x1234u16]];
        let rule_query = NativePivotQuery::Auxiliary {
            values: VALUES.to_vec(),
            spec,
        };
        let rule_page = backend
            .collect_page(&rule_query, &request, &|| false)
            .expect("fused auxiliary rule page");
        assert_eq!(rule_page.stage_counts.len(), 2);
        assert_eq!(rule_page.fixed_seed_count, rule_page.stage_counts[0]);
        assert!(rule_page.stage_counts[1] <= rule_page.stage_counts[0]);
        let oracle = local_natural_matches(&VALUES, 0, 20_000, STRIDE, 1);
        assert!(rule_page
            .matches
            .iter()
            .all(|matched| oracle.contains(matched)));
    }

    #[test]
    fn auxiliary_final_truncated_chunk_recount_keeps_prior_100_plus_tail_5_counts() {
        let _lock = lock();
        let backend = loaded();
        let _pin = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("policy pin");
        let query = NativePivotQuery::Auxiliary {
            values: VALUES.to_vec(),
            spec: synthetic_auxiliary_spec(),
        };
        let oracle = local_natural_matches(&VALUES, 0, 100_000, STRIDE, 1);
        assert!(oracle.len() > 115, "oracle must span two chunks");
        let first_chunk_trials = oracle[99].trial;
        assert!(
            oracle[104].trial < first_chunk_trials.saturating_mul(2),
            "the second chunk must contain the five-result tail"
        );

        let page = backend
            .collect_page(
                &query,
                &PageRequest::chunk(
                    0,
                    first_chunk_trials.saturating_mul(2),
                    first_chunk_trials,
                    105,
                ),
                &|| false,
            )
            .expect("two-chunk truncated auxiliary page");

        assert_eq!(
            page.native_calls, 3,
            "full, final, and prefix recount calls"
        );
        assert_eq!(page.matches, oracle[..105]);
        assert_eq!(page.next_cursor, oracle[104].trial);
        assert_eq!(page.stage_counts, vec![105]);
        assert_eq!(page.fixed_seed_count, 105);

        let resumed = backend
            .collect_page(
                &query,
                &PageRequest::chunk(page.next_cursor, first_chunk_trials, first_chunk_trials, 10),
                &|| false,
            )
            .expect("resume after the truncated cursor");
        assert_eq!(resumed.matches, oracle[105..115]);
        assert_eq!(resumed.next_cursor, oracle[114].trial);
        assert!(resumed.matches[0].trial > page.next_cursor);
    }

    #[test]
    fn nested_policy_allow_strict_drop_restores_allow_then_strict_default() {
        let _lock = lock();
        let backend = loaded();
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
        let outer = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("outer allow pin");
        assert_direct_policy(&backend, ExecutionPolicy::AllowBulkCpu);
        let inner = backend
            .pin_policy(ExecutionPolicy::StrictGpu)
            .expect("inner strict pin");
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
        drop(inner);
        assert_direct_policy(&backend, ExecutionPolicy::AllowBulkCpu);
        drop(outer);
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
    }

    #[test]
    fn nested_policy_strict_allow_drop_restores_strict_in_both_scopes() {
        let _lock = lock();
        let backend = loaded();
        let outer = backend
            .pin_policy(ExecutionPolicy::StrictGpu)
            .expect("outer strict pin");
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
        let inner = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("inner allow pin");
        assert_direct_policy(&backend, ExecutionPolicy::AllowBulkCpu);
        drop(inner);
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
        drop(outer);
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
    }

    #[test]
    fn policy_guard_error_return_restores_native_and_mirror_to_strict_gpu() {
        let _lock = lock();
        let backend = loaded();
        let result = (|| -> Result<(), NativeSearchError> {
            let _guard = backend.pin_policy(ExecutionPolicy::AllowBulkCpu)?;
            assert_direct_policy(&backend, ExecutionPolicy::AllowBulkCpu);
            Err(NativeSearchError::Rejected {
                call: "expected_test_error",
            })
        })();
        assert!(matches!(
            result,
            Err(NativeSearchError::Rejected {
                call: "expected_test_error"
            })
        ));
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
    }

    #[test]
    fn policy_guard_concurrency_isolates_threads_and_finishes_strict_gpu() {
        let _lock = lock();
        let backend = Arc::new(loaded());
        let (holding_tx, holding_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first_backend = Arc::clone(&backend);
        let first = std::thread::spawn(move || {
            let guard = first_backend
                .pin_policy(ExecutionPolicy::AllowBulkCpu)
                .expect("first-thread allow pin");
            holding_tx
                .send(accelerator(&first_backend).pinned_policy())
                .expect("publish first policy");
            release_rx.recv().expect("release first thread");
            drop(guard);
        });
        assert_eq!(holding_rx.recv().unwrap(), ExecutionPolicy::AllowBulkCpu);

        let (attempting_tx, attempting_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let second_backend = Arc::clone(&backend);
        let second = std::thread::spawn(move || {
            attempting_tx.send(()).expect("publish second attempt");
            let guard = second_backend
                .pin_policy(ExecutionPolicy::StrictGpu)
                .expect("second-thread strict pin");
            acquired_tx
                .send(accelerator(&second_backend).pinned_policy())
                .expect("publish second policy");
            drop(guard);
        });
        attempting_rx.recv().expect("second thread started");
        assert!(
            acquired_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "second thread must wait while the first thread owns the policy"
        );
        release_tx.send(()).expect("release first owner");
        assert_eq!(
            acquired_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            ExecutionPolicy::StrictGpu
        );
        first.join().expect("first policy thread");
        second.join().expect("second policy thread");
        assert_direct_policy(&backend, ExecutionPolicy::StrictGpu);
    }

    #[test]
    fn strict_policy_refuses_bulk_cpu_and_opt_in_does_not_leak() {
        let _lock = lock();
        let backend = loaded();
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let request = PageRequest::chunk(0, 2_000, 2_000, 10_000);
        let push = || backend.collect_page(&query, &request, &|| false);

        // Forcing the CUDA failure hook reaches the same branch on a
        // CUDA-capable and a CUDA-less machine, so the rollback proof does not
        // depend on the host GPU.
        backend.force_cuda_failure(true);
        let before = backend
            .accelerator
            .as_ref()
            .map_or(0, |accelerator| accelerator.bulk_cpu_call_count());

        let strict = push();
        assert!(
            matches!(strict, Err(NativeSearchError::CudaUnavailable { .. })),
            "strict-GPU must fail closed, got {strict:?}"
        );

        let bulk = {
            let _pin = backend
                .pin_policy(ExecutionPolicy::AllowBulkCpu)
                .expect("bulk pin");
            push().expect("bulk cpu opt-in must scan")
        };
        assert_eq!(bulk.backend, NativeBackend::NativeCpu);
        let after = backend
            .accelerator
            .as_ref()
            .map_or(0, |accelerator| accelerator.bulk_cpu_call_count());
        assert!(after > before, "bulk CPU path must be counted");

        // Dropping the pin must restore strict GPU: the next call fails again
        // without touching the bulk CPU counter.
        let strict_after = push();
        assert!(matches!(
            strict_after,
            Err(NativeSearchError::CudaUnavailable { .. })
        ));
        let final_count = backend
            .accelerator
            .as_ref()
            .map_or(0, |accelerator| accelerator.bulk_cpu_call_count());
        assert_eq!(final_count, after);
        backend.force_cuda_failure(false);
    }

    #[test]
    fn validation_rejects_inputs_outside_the_native_abi() {
        let _lock = lock();
        assert!(matches!(
            SearchBackend::replay_pivot_seed(&[], &window(0, 1), 1),
            Err(NativeSearchError::InvalidInput(_))
        ));
        assert!(matches!(
            SearchBackend::replay_pivot_seed(&VALUES, &window(0, 1), 0),
            Err(NativeSearchError::InvalidInput(_))
        ));
        assert!(matches!(
            verify_pivot_match(&VALUES, &window(0, 10), PivotMatch { seed: 1, trial: 11 }),
            Err(NativeSearchError::InvalidInput(_))
        ));

        // The page driver clamps one native call to the route's ABI cap, so an
        // oversized budget is scanned in capped chunks instead of failing.
        let query = NativePivotQuery::Natural {
            values: VALUES.to_vec(),
        };
        let backend = loaded();
        let oversized = PageRequest::chunk(0, MAX_NATURAL_TRIALS + 3, MAX_NATURAL_TRIALS + 3, 4);
        // The clamped page must also run on a host without a CUDA device, so the
        // test pins its own explicit bulk-CPU policy; the production default
        // stays StrictGpu.
        let _policy = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("the test's explicit bulk-CPU policy is accepted");
        let page = backend
            .collect_page(&query, &oversized, &|| false)
            .expect("clamped page");
        assert_eq!(page.matches.len(), 4);

        // The ABI bound itself still fails closed when a caller asks for an
        // explicitly oversized window.
        let accelerator = Accelerator::load(&repo_root(), None).expect("accelerator");
        let too_wide = PivotWindow {
            start_index: 0,
            stop_index: MAX_NATURAL_TRIALS + 1,
            low16_stride: STRIDE,
            draw_index: 1,
        };
        assert!(matches!(
            accelerator.collect_natural_pivot_page(&VALUES, too_wide),
            Err(NativeSearchError::InvalidInput(_))
        ));
        assert!(matches!(
            backend.collect_page(&query, &PageRequest::chunk(0, 10, 10, 0), &|| false),
            Err(NativeSearchError::InvalidInput(_))
        ));
    }

    fn window(start: u64, stop: u64) -> PivotWindow {
        PivotWindow {
            start_index: start,
            stop_index: stop,
            low16_stride: STRIDE,
            draw_index: 1,
        }
    }
}
