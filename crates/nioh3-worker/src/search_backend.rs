//! Typed native query layer: one bounded page at a time.
//!
//! This module owns the query/paging contract the job layer drives. It never
//! parses protocol JSON, never reads product data files and never enumerates
//! seeds in Rust: the accelerator scans a bounded window, and every match it
//! returns is re-derived here with exact cursor algebra before the page is
//! handed on.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use nioh3_domain::rng::A_INV;

use crate::native_search::{
    absent_capabilities, Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, ExecutionPolicyGuard,
    NativeBackend, NativeCapabilities, NativeSearchError, PivotMatch, PivotWindow,
    R4PrimaryPivotSpec, MAX_AUXILIARY_TRIALS, MAX_NATURAL_TRIALS, MAX_R4_PRIMARY_TRIALS,
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
}

impl NativePivotQuery {
    /// The permuted pivot values the cursor walks.
    pub fn values(&self) -> &[u16] {
        match self {
            NativePivotQuery::Natural { values }
            | NativePivotQuery::R4Primary { values, .. }
            | NativePivotQuery::Auxiliary { values, .. } => values,
        }
    }

    /// The `low16_stride` this route's pivot uses.
    pub fn low16_stride(&self) -> u16 {
        match self {
            NativePivotQuery::Auxiliary { .. } => AUXILIARY_LOW16_STRIDE,
            _ => R4_PRIMARY_LOW16_STRIDE,
        }
    }

    /// Total number of trials in the pivot family.
    pub fn family_size(&self) -> u64 {
        self.values().len() as u64 * 0x1_0000
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
        }
    }
}

/// `low16_stride` the shipped natural/R4 pivot routes use.
pub const R4_PRIMARY_LOW16_STRIDE: u16 = 0x9E37;
/// `low16_stride` the fused auxiliary route uses.
pub const AUXILIARY_LOW16_STRIDE: u16 = 0x9E37;

/// One bounded page request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Exclusive resume cursor; the first scanned flat index.
    pub start_after_trial: u64,
    /// Maximum number of trials this page may scan.
    pub max_trials: u64,
    /// Largest window one native call may scan; clamped to the route's ABI cap.
    pub chunk_trials: u64,
    /// Stop after this many matches.
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
}

impl SearchBackend {
    /// Load the accelerator for one worker. `None` means no library passed
    /// ABI/policy/symbol validation; the worker must still answer `handshake`
    /// and must fail search closed.
    pub fn load(application_root: &Path, override_path: Option<&Path>) -> Option<Self> {
        Accelerator::load(application_root, override_path)
            .map(|accelerator| Self::from_shared(Arc::new(accelerator)))
    }

    /// Share one already-loaded accelerator between the query compiler and the
    /// bounded collector, so the DLL is opened once per worker.
    pub fn from_shared(accelerator: Arc<Accelerator>) -> Self {
        Self {
            accelerator: Some(accelerator),
        }
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
        let Some(accelerator) = self.accelerator.as_deref() else {
            return Err(NativeSearchError::Unavailable);
        };
        let chunk = request.effective_chunk(query)?;
        let family_size = query.family_size();
        let start = request.start_after_trial.min(family_size);
        let budget_stop = start.saturating_add(request.max_trials).min(family_size);
        let values = query.values();
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
            let (page, counts) = collect_window(accelerator, query, &window)?;
            native_calls += 1;
            backend = page.backend;
            for matched in &page.matches {
                verify_pivot_match(values, &window, *matched)?;
            }
            accumulate(&mut stage_counts, &counts)?;

            let remaining = request.page_size - matches.len();
            if page.matches.len() > remaining {
                // Recount the exact prefix ending at the last returned result so
                // pagination and every displayed intersection count stay exact.
                let cut = page.matches[remaining - 1].trial;
                let mut recount_window = window;
                recount_window.stop_index = cut;
                let (recount, recount_counts) =
                    collect_window(accelerator, query, &recount_window)?;
                native_calls += 1;
                for matched in &recount.matches {
                    verify_pivot_match(values, &recount_window, *matched)?;
                }
                if recount.matches.len() != remaining {
                    return Err(NativeSearchError::Rejected {
                        call: "collect_auxiliary_pivot_matches",
                    });
                }
                stage_counts = vec![0u64; query.stage_count()];
                accumulate(&mut stage_counts, &recount_counts)?;
                matches.extend(recount.matches);
                cursor = cut;
                progress(&ChunkProgress {
                    inspected_through_trial: cursor,
                    stage_counts: stage_counts.clone(),
                    matches: matches.len(),
                });
                break;
            }
            matches.extend(page.matches);
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
        }
    }
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

fn collect_window(
    accelerator: &Accelerator,
    query: &NativePivotQuery,
    window: &PivotWindow,
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
    use std::sync::{Mutex, MutexGuard};

    use super::*;
    use crate::native_search::SEED_ACCELERATOR_ABI_VERSION;

    /// The DLL keeps process-global diagnostics, so no two tests may overlap.
    static DLL_LOCK: Mutex<()> = Mutex::new(());

    const VALUES: [u16; 5] = [0x1234, 0xABCD, 0x0001, 0xFFFE, 0x00FF];
    const STRIDE: u16 = 0x9E37;

    fn lock() -> MutexGuard<'static, ()> {
        DLL_LOCK.lock().unwrap_or_else(|error| error.into_inner())
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
