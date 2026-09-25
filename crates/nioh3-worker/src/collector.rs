//! Search batch and candidate-materialization contract.
//!
//! Mirrors the two layers the shipped Python worker splits a search across:
//! `search_application.collect_offline_ng3_search_batch` (bounded pages of
//! verified pivot matches plus an exact intersection report) and
//! `search_jobs.SearchJobs._run` (the job state machine that owns the budget,
//! the checkpoint, the private candidate records and the resume token).
//!
//! The bounded collector is supplied by the native-search component
//! (`native_search.rs` / `search_backend.rs`); it returns verified matches only
//! and never materializes a candidate or a full preview. This crate materializes
//! the accepted matches alone, so a page that inspects 100M trials still builds
//! at most `result_count` candidates.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::model::Candidate;
use crate::native_search::PivotMatch;
use crate::query::SearchQuery;

/// Cumulative survivors after one user-selected constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersectionStageCount {
    pub kind: String,
    pub values: Vec<u64>,
    pub count: u64,
}

/// Exact cumulative counts for the inspected portion of one pivot family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntersectionReport {
    pub start_after_trial: u64,
    pub inspected_through_trial: u64,
    pub family_size: u64,
    pub fixed_seed_count: u64,
    pub stages: Vec<IntersectionStageCount>,
    pub complete_match_count: u64,
    pub exhausted_family: bool,
}

/// One bounded solver page.
#[derive(Debug, Clone, Default)]
pub struct SearchBatch {
    pub matches: Vec<PivotMatch>,
    pub next_start_after_trial: Option<u64>,
    pub intersection_report: Option<IntersectionReport>,
    /// Whether the page streamed matches into materialization as it scanned.
    pub streamed: bool,
}

/// Arguments for one bounded page, in `search_jobs._run` terms.
#[derive(Debug, Clone)]
pub struct BatchRequest<'a> {
    pub query: &'a SearchQuery,
    pub level: u16,
    /// `result_count - len(committed candidates)`.
    pub result_count: usize,
    /// `min(page_trials, stop - cursor)`; unbounded only under continuation.
    pub max_trials_per_batch: u64,
    pub start_after_trial: u64,
    /// The job's `allow_cpu_fallback`. The collector owns the execution policy:
    /// it must pin `AllowBulkCpu` when this is true and `StrictGpu` when it is
    /// false, on the calling (job) thread, and restore on drop so a failed or
    /// cancelled page cannot leak the opt-in into the next job. The job layer
    /// never mutates global native policy itself.
    pub allow_cpu_fallback: bool,
}

/// A collector failure, carrying the wire error code a failed job reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorError {
    pub code: String,
    pub message: String,
}

impl CollectorError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    /// A backend that cannot run this query at all.
    ///
    /// Kept distinct from `INVALID_REQUEST` so a caller can tell "the search
    /// backend is unavailable" apart from "the request was malformed", and
    /// distinct from the collector's `SEARCH_FAILED` for per-page failures.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new("SEARCH_BACKEND_UNAVAILABLE", message)
    }
}

/// Bounded, cancellable pivot collection for one compiled query.
///
/// Contract the native component must hold:
///
/// * a page never returns a trial at or below `request.start_after_trial`;
/// * `next_start_after_trial` is the exclusive resume point and is `None` only
///   when the family is exhausted;
/// * `cancelled` is polled between native chunks and a cancelled page keeps the
///   matches and the exact checkpoint it already reached;
/// * the intersection report is cumulative for the inspected window, and
///   `exhausted_family` means the whole family has been inspected.
pub trait SearchCollector: Send + Sync {
    fn collect(
        &self,
        request: &BatchRequest<'_>,
        progress: &mut dyn FnMut(&IntersectionReport),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SearchBatch, CollectorError>;
}

/// A materialized match: the candidate record kept privately plus the exact
/// candidate payload the job publishes.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedCandidate {
    pub candidate: Candidate,
    pub payload: Value,
    /// Whether the candidate's composed auxiliary output satisfies the query's
    /// requested auxiliary criteria (terrain effect keys, special-rule keys and
    /// enemy lookup keys, with their any-of groups).
    ///
    /// Only `Some(true)` accepts. This is mandatory post-acceptance work for any
    /// route whose pivot narrows on something other than the auxiliary criteria
    /// (for example the R4 primary path), and it is a harmless recheck for the
    /// fused auxiliary path whose pivot already packs them. `None` rejects, so a
    /// source that does not evaluate the criteria cannot let a candidate through.
    pub auxiliary_match: Option<bool>,
    /// Whether the candidate satisfies the query's mandatory enemy-occurrence
    /// groups under `enemy_state_search.enemy_occurrence_groups_status`.
    ///
    /// `Some(true)` is the only accepting value; `Some(false)` and `None` both
    /// reject, so a source that cannot evaluate the groups can never let an
    /// unverified candidate through. It is `Some(true)` when the query has no
    /// groups to evaluate.
    pub enemy_occurrence_match: Option<bool>,
}

/// Turns an accepted pivot match into a candidate, exactly like the shipped
/// `materialize_match` closure. Only accepted matches reach this call, so a page
/// that inspects 100M trials still composes at most `result_count` previews.
///
/// `trial` is the 1-based solver trial the match came from; it becomes the
/// candidate's `joint_search_trial`, which is the wire `cursor`.
pub trait CandidateSource: Send + Sync {
    /// Compose one accepted match.
    ///
    /// `grace` is the registered save-bound Grace map of a cached NG4/NG5 job,
    /// or `None` for an NG3 job, which always composes the bundled map. The
    /// certified composer needs it because the playthrough's record type and
    /// draw-1 partition are exactly what the cached route replaces.
    fn materialize(
        &self,
        query: &SearchQuery,
        seed: u32,
        trial: u64,
        grace: Option<&nioh3_domain::effect::GraceMap>,
    ) -> Result<MaterializedCandidate, CollectorError>;

    /// Resolve the parts of a parsed query that need the context-bound tables.
    ///
    /// `SearchQuery.from_payload` resolves terrain option ids to their exact
    /// row union and checks every selected Grace against the rarity's final
    /// Grace set. The parser cannot see the tables, so the job layer calls this
    /// once, before the query is compiled, and the compiler, the page filters
    /// and the final acceptance all read the same resolved query. The default
    /// leaves the query unchanged, for sources that carry no tables.
    fn resolve_query(&self, query: &mut SearchQuery) -> Result<(), CollectorError> {
        let _ = query;
        Ok(())
    }
}

/// Message a failed job reports while no bounded collector is compiled in.
pub const MISSING_COLLECTOR_MESSAGE: &str = "offline search requires the bounded native pivot collector, which is not compiled into this development worker yet";

/// Compiles one validated query into the bounded collector for that job.
///
/// The table-derived compilation (`_terrain_batch_configuration`,
/// `_enemy_batch_configuration`, `_special_rule_batch_configuration`,
/// `choose_pivot`, `permuted_pivot_values` and the R4 primary configuration)
/// lives in `crate::query_compile`, which owns the mapping from
/// [`SearchQuery`] to `search_backend::NativePivotQuery`.
pub trait SearchFactory: Send + Sync {
    fn collector(&self, query: &SearchQuery) -> Result<Arc<dyn SearchCollector>, CollectorError>;

    /// The structural preflight `collector` applies before compiling, on its
    /// own: `Some(Err(reason))` when the query can have no solution, and `None`
    /// when this factory cannot judge it.
    fn feasibility(&self, query: &SearchQuery) -> Option<Result<(), String>> {
        let _ = query;
        None
    }

    /// Compile the save-bound cache route for a playthrough-4/5 rarity-5 job.
    ///
    /// The measured-map pivot belongs to the native compiler, which owns
    /// `query_compile`/`search_backend`; this hook lets the job layer hand it the
    /// validated map without that file having to change shape. Until the native
    /// cache route lands, the default refuses with a named
    /// [`CollectorError::unavailable`] instead of claiming an unavailable search.
    fn cached_collector(
        &self,
        query: &SearchQuery,
        cache: &crate::grace_map::GraceOutputMap,
    ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
        let _ = (query, cache);
        Err(CollectorError::unavailable(
            "the save-bound NG4/NG5 cache search route is not compiled into this \
             development worker yet",
        ))
    }
}

/// The native bounded collector the job layer mounts at startup.
///
/// Delegates to `query_compile::native_factory`, which compiles a
/// [`SearchQuery`] into a `search_backend::NativePivotQuery` and returns the
/// collector for that job. A missing accelerator or an unusable table resource
/// is reported per query as a typed [`CollectorError`] instead of a silent
/// fallback.
pub fn native_factory(
    application_root: &Path,
    accelerator_override: Option<&Path>,
    data_root: &Path,
    resource_version: Option<(u16, u16, u16, u16)>,
) -> Option<Arc<dyn SearchFactory>> {
    crate::query_compile::native_factory(
        application_root,
        accelerator_override,
        data_root,
        resource_version,
    )
}
