//! Exact standalone search over owned equipment with required sets.
//! The cursor covers the full unexcluded domain, with every early exclusion
//! justified by an optimistic piece-count bound.

use super::{
    canonical_score_bits, rank, Counts, Plan, Request, Scored, Search, SetPlanning, KEEP,
    MAX_EXACT_COVERAGE,
};
use crate::{
    engine::{Backpack, Candidate, Engine, Outcome, ReadBounds, SweepOutcome, Work},
    gpu::MainDevice,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::Instant;

#[path = "exact_audit.rs"]
mod audit;
pub use audit::{ExactAudit, ExactAuditBatch};

const RETAINED_FAILURES: usize = 128;
const CHECKPOINT_SCHEMA: u32 = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScorerKind {
    Cpu,
    Cuda,
}

/// Either order is a permutation of the exact planned domain.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationOrder {
    Linear,
    Spread,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SetPruning {
    Proven,
    Unpruned,
}

enum Scorer {
    Cpu(Option<Box<ReadBounds>>),
    Cuda(Box<MainDevice>),
}

/// Counts describe actual work, not additional covered outfits. Timers are
/// opt-in and exclude final report serialization and the independent audit.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ExactPerformance {
    pub timing_enabled: bool,
    pub open_seconds: f64,
    pub batches: u64,
    pub candidates_scheduled: u64,
    pub batches_committed: u64,
    pub cpu_executed_candidates: u64,
    pub cpu_certified_candidates: u64,
    pub cpu_overlay_layout_rebuilds: u64,
    pub cpu_overlay_layout_reuses: u64,
    pub cpu_preparation_seconds: f64,
    /// Certificate lookup/recording and MAIN, after preparation.
    pub cpu_scoring_seconds: f64,
    /// Shared preparation plus Rust/native CUDA call; native details below.
    pub cuda_batch_seconds: f64,
    pub ranking_seconds: f64,
    pub advance_seconds: f64,
    pub host_preparation: crate::engine::PreparationStats,
    pub device: Option<crate::gpu::PreparedDeviceStats>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct RankedOrdinal {
    ordinal: u64,
    score_bits: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExactCheckpoint {
    schema: u32,
    pub domain_sha256: String,
    pub scorer: ScorerKind,
    pub order: EnumerationOrder,
    pub set_pruning: SetPruning,
    pub requested: u64,
    /// Assignments proven unable to reach the requested set tiers.
    pub excluded_by_set_bound: u64,
    pub planned: u64,
    pub completed: u64,
    pub answered: u64,
    pub refused: u64,
    pub no_path: u64,
    pub outside_limits: u64,
    pub certified: u64,
    pub failures: BTreeMap<String, u64>,
    pub unanswered_samples: Vec<u64>,
    rank: Vec<RankedOrdinal>,
}

impl ExactCheckpoint {
    pub fn complete(&self) -> bool {
        self.schema == CHECKPOINT_SCHEMA
            && self.completed == self.planned
            && self.planned.checked_add(self.excluded_by_set_bound) == Some(self.requested)
            && self.no_path == 0
            && self.refused == 0
    }
}

/// Pinned request/domain. The cursor uses the exact planner's disjoint spaces.
pub struct ExactSweep<'a> {
    search: Search<'a>,
    request: Request,
    plan: Plan,
    starts: Vec<u64>,
    stride: u64,
    swaps: Vec<(u32, usize)>,
    identity: ExactCheckpoint,
    scorer: Scorer,
    // One scratch owner for the entire solve. Never borrow it across a native
    // asynchronous operation: the prepared device ABI drains before returning.
    work: Work,
    performance: ExactPerformance,
}

fn put(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn stride_for(order: EnumerationOrder, size: u64) -> u64 {
    if order == EnumerationOrder::Linear || size <= 1 {
        return 1;
    }
    fn gcd(mut a: u64, mut b: u64) -> u64 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }
    let start = (u128::from(size) * 618_033_988 / 1_000_000_000) as u64;
    // 1 is coprime to every size, so this always finds a permutation.
    (start.max(1)..size)
        .find(|&value| gcd(value, size) == 1)
        .unwrap_or(1)
}

impl<'a> ExactSweep<'a> {
    pub fn open(
        engine: &'a Engine,
        pack: &'a Backpack,
        request: Request,
        kind: ScorerKind,
        order: EnumerationOrder,
        swaps: &[(u32, usize)],
        set_pruning: SetPruning,
    ) -> Result<Self, String> {
        let opened = Instant::now();
        if request.move_points {
            return Err("exact sweep requires fixed attributes".into());
        }
        if request.limits.max_load.is_nan() {
            return Err("max_load cannot be NaN".into());
        }
        if request.sets.is_empty()
            || request.sets.iter().any(|rule| {
                rule.at_least == 0
                    || rule.at_least > rule.options.len()
                    || rule
                        .options
                        .iter()
                        .any(|option| option.key == 0 || option.pieces == 0)
            })
        {
            return Err("exact sweep requires at least one set and a positive minimum tier".into());
        }
        let mut work = engine.work_with_weapon_swaps(pack, swaps)?;
        for &(_, first, count) in &pack.ranges {
            let start = first as usize;
            let end = start
                .checked_add(count as usize)
                .ok_or("backpack range overflows")?;
            if count == 0 || pack.pool.get(start..end).is_none() {
                return Err("backpack range leaves its items".into());
            }
        }
        let search = Search::new(engine, pack);
        let inputs = match set_pruning {
            SetPruning::Proven => {
                Some(engine.exact_set_capacities(pack, &Search::rule_keys(&request), swaps))
            }
            SetPruning::Unpruned => None,
        };
        let planning = inputs
            .as_ref()
            .map_or(SetPlanning::NoPruning, SetPlanning::ExactCapacity);
        let plan = search.build_plan_with_sets(&request, Some(MAX_EXACT_COVERAGE), planning);
        let planned = plan.size();
        let excluded_by_set_bound = plan
            .full_size
            .checked_sub(planned)
            .ok_or("exact sweep plan exceeds its requested domain")?;
        if !plan.is_complete()
            || (set_pruning == SetPruning::Unpruned && excluded_by_set_bound != 0)
        {
            return Err("exact sweep requires complete source coverage for its plan".into());
        }
        let bundle = engine
            .bundle_input_digest()
            .ok_or("immutable input bundle required")?;
        let mut fingerprint = Sha256::new();
        put(&mut fingerprint, b"nioh3-exact-sweep-v4");
        put(&mut fingerprint, bundle.as_bytes());
        if let Some(encoded) = engine.encoded_context() {
            put(&mut fingerprint, encoded.context_digest().as_bytes());
            put(&mut fingerprint, encoded.input_digest().as_bytes());
        }
        put(
            &mut fingerprint,
            &serde_json::to_vec(swaps).map_err(|error| error.to_string())?,
        );
        put(
            &mut fingerprint,
            &serde_json::to_vec(&(&pack.ranges, &pack.origin))
                .map_err(|error| error.to_string())?,
        );
        for item in &pack.pool {
            put(&mut fingerprint, item);
        }
        let set_rules: Vec<_> = request
            .sets
            .iter()
            .map(|rule| {
                (
                    rule.at_least,
                    rule.options
                        .iter()
                        .map(|option| (option.key, option.pieces))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        put(
            &mut fingerprint,
            &serde_json::to_vec(&(
                request.limits.max_load.to_bits(),
                request.limits.min_defense,
                request.limits.min_health,
                set_rules,
                set_pruning,
            ))
            .map_err(|error| error.to_string())?,
        );
        let domain_sha256 = format!("{:x}", fingerprint.finalize());
        let starts = plan.starts();
        let mut scorer = match kind {
            ScorerKind::Cpu => Scorer::Cpu(
                engine
                    .supports_read_bounds()
                    .then(|| Box::new(ReadBounds::new())),
            ),
            ScorerKind::Cuda => {
                if !engine.supports_prepared_device() {
                    return Err("prepared CUDA does not support this event program".into());
                }
                let mut device = engine.open_prepared_device()?;
                let size = plan.size();
                let probe: Vec<_> = (0..size.min(16))
                    .map(|n| plan.nth(&starts, n * (size / size.min(16)), engine.start_attrs))
                    .collect();
                let device_scores = search.prepared_device_outcomes_with_work(
                    &mut device,
                    &probe,
                    &request,
                    &mut work,
                )?;
                for (candidate, result) in probe.iter().zip(device_scores) {
                    if search.candidate_outcome(candidate, &request, &mut work) != result {
                        return Err("prepared CUDA did not match the host score probe".into());
                    }
                }
                Scorer::Cuda(Box::new(device))
            }
        };
        // Startup probes are measured in open_seconds, not sweep counters.
        work.reset_preparation_stats();
        if let Scorer::Cuda(device) = &mut scorer {
            device.reset_statistics();
        }
        Ok(Self {
            identity: ExactCheckpoint {
                schema: CHECKPOINT_SCHEMA,
                domain_sha256,
                scorer: kind,
                order,
                set_pruning,
                requested: plan.full_size,
                excluded_by_set_bound,
                planned,
                completed: 0,
                answered: 0,
                refused: 0,
                no_path: 0,
                outside_limits: 0,
                certified: 0,
                failures: BTreeMap::new(),
                unanswered_samples: Vec::new(),
                rank: Vec::new(),
            },
            stride: stride_for(order, plan.size()),
            swaps: swaps.to_vec(),
            search,
            request,
            plan,
            starts,
            scorer,
            work,
            performance: ExactPerformance {
                open_seconds: opened.elapsed().as_secs_f64(),
                ..ExactPerformance::default()
            },
        })
    }

    pub fn performance(&self) -> ExactPerformance {
        let mut stats = self.performance.clone();
        stats.host_preparation = self.work.preparation_stats();
        if let Scorer::Cpu(Some(bounds)) = &self.scorer {
            (stats.cpu_overlay_layout_rebuilds, stats.cpu_overlay_layout_reuses) =
                bounds.overlay_layout_counts();
        }
        stats.device = match &self.scorer {
            Scorer::Cpu(_) => None,
            Scorer::Cuda(device) => Some(device.stats()),
        };
        stats
    }

    /// Diagnostic timing is optional; old DLLs retain correct scoring without
    /// the optional native timing ABI. A profile setup error is explicit.
    pub fn enable_profiling(&mut self, enabled: bool) -> Result<bool, String> {
        self.performance.timing_enabled = enabled;
        match &mut self.scorer {
            Scorer::Cpu(_) => Ok(false),
            Scorer::Cuda(device) => device.enable_profiling(enabled),
        }
    }

    /// Reference controls for host acceptance. Reuse changes execution work,
    /// never the candidate domain, ordering or exact result.
    pub fn set_reuse_enabled(&mut self, enabled: bool) {
        match &mut self.scorer {
            Scorer::Cpu(bounds) => {
                *bounds = (enabled && self.search.engine.supports_read_bounds())
                    .then(|| Box::new(ReadBounds::new()));
            }
            Scorer::Cuda(device) => device.set_reuse_enabled(enabled),
        }
    }

    pub fn work(&self) -> Result<Work, &'static str> {
        self.search
            .engine
            .work_with_weapon_swaps(self.search.pack, &self.swaps)
    }

    pub fn displaced_weapon_slot(&self, candidate: &Candidate) -> Option<usize> {
        let row = candidate.items[crate::engine::position_of(0)?]?;
        self.swaps
            .iter()
            .find_map(|&(owned, slot)| (owned == row).then_some(slot))
    }

    pub fn start(&self) -> ExactCheckpoint {
        self.identity.clone()
    }

    fn candidate(&self, ordinal: u64) -> Candidate {
        let index =
            (u128::from(ordinal) * u128::from(self.stride) % u128::from(self.plan.size())) as u64;
        self.plan
            .nth(&self.starts, index, self.search.engine.start_attrs)
    }

    pub fn candidate_at(&self, position: u64) -> Option<Candidate> {
        (position < self.plan.size()).then(|| self.candidate(position))
    }

    fn ranked(&self, row: RankedOrdinal) -> Scored {
        Scored {
            bits: row.score_bits,
            candidate: self.candidate(row.ordinal),
        }
    }

    pub fn validate(&self, state: &ExactCheckpoint) -> Result<(), String> {
        let base = &self.identity;
        let count = [
            state.answered,
            state.refused,
            state.no_path,
            state.outside_limits,
        ]
        .into_iter()
        .try_fold(0u64, u64::checked_add);
        if state.schema != base.schema
            || state.domain_sha256 != base.domain_sha256
            || state.scorer != base.scorer
            || state.order != base.order
            || state.set_pruning != base.set_pruning
            || state.requested != base.requested
            || state.excluded_by_set_bound != base.excluded_by_set_bound
            || state.planned != base.planned
            || state.completed > base.planned
            || count != Some(state.completed)
            || state
                .failures
                .values()
                .copied()
                .try_fold(0u64, u64::checked_add)
                != state.no_path.checked_add(state.refused)
            || state.unanswered_samples.len()
                != state
                    .no_path
                    .saturating_add(state.refused)
                    .min(RETAINED_FAILURES as u64) as usize
            || state
                .unanswered_samples
                .iter()
                .any(|&ordinal| ordinal >= state.completed)
            || state
                .unanswered_samples
                .windows(2)
                .any(|adjacent| adjacent[0] >= adjacent[1])
            || state.certified > state.answered
            || state.rank.len() != (state.answered.min(KEEP as u64) as usize)
        {
            return Err("checkpoint does not cover a valid prefix of this exact search".into());
        }
        let mut ordinals = std::collections::HashSet::new();
        if state.rank.iter().any(|row| {
            row.ordinal >= state.completed
                || !ordinals.insert(row.ordinal)
                || canonical_score_bits(row.score_bits) != Some(row.score_bits)
        }) || state
            .rank
            .windows(2)
            .any(|pair| rank(&self.ranked(pair[0])) > rank(&self.ranked(pair[1])))
        {
            return Err("checkpoint contains an invalid rank".into());
        }
        Ok(())
    }

    pub fn best(&self, state: &ExactCheckpoint) -> Result<Vec<Scored>, String> {
        self.validate(state)?;
        Ok(state
            .rank
            .iter()
            .copied()
            .map(|row| self.ranked(row))
            .collect())
    }

    /// Score one batch and publish its prefix only once all outcomes are available.
    pub fn advance(&mut self, state: &mut ExactCheckpoint, batch: usize) -> Result<(), String> {
        self.advance_controlled(state, batch, &|| false)
    }

    /// Cancellation observed before publication discards the entire pending
    /// batch. The synchronous native call finishes cleanup before this returns.
    pub fn advance_controlled(
        &mut self,
        state: &mut ExactCheckpoint,
        batch: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        let check_cancelled = || {
            if cancelled() {
                Err(crate::search_control::SearchInterrupted::Cancelled.to_string())
            } else {
                Ok(())
            }
        };
        check_cancelled()?;
        let started = self.performance.timing_enabled.then(Instant::now);
        self.validate(state)?;
        if !(1..=4096).contains(&batch) {
            return Err("a scoring batch must contain 1..4096 outfits".into());
        }
        let end = (state.completed + batch as u64).min(self.plan.size());
        let candidates: Vec<_> = (state.completed..end).map(|i| self.candidate(i)).collect();
        if candidates.is_empty() {
            return Ok(());
        }
        let mut counts = Counts::default();
        self.performance.batches += 1;
        self.performance.candidates_scheduled += candidates.len() as u64;
        let work = &mut self.work;
        let outcomes = match &mut self.scorer {
            Scorer::Cuda(device) => {
                let timed = self.performance.timing_enabled.then(Instant::now);
                let result = self.search.prepared_device_outcomes_with_work_controlled(
                    device, &candidates, &self.request, work, cancelled,
                );
                if let Some(timed) = timed {
                    self.performance.cuda_batch_seconds += timed.elapsed().as_secs_f64();
                }
                result?
            }
            Scorer::Cpu(bounds) => {
                let mut outcomes = Vec::with_capacity(candidates.len());
                for candidate in &candidates {
                    check_cancelled()?;
                    let timed = self.performance.timing_enabled.then(Instant::now);
                    let prepared = self.search.prepare_candidate(candidate, &self.request, work);
                    if let Some(timed) = timed {
                        self.performance.cpu_preparation_seconds += timed.elapsed().as_secs_f64();
                    }
                    let prepared = match prepared {
                        Ok(prepared) => prepared,
                        Err(outcome) => { outcomes.push(outcome); continue; }
                    };
                    let timed = self.performance.timing_enabled.then(Instant::now);
                    let outcome = if let Some(bound) = bounds.as_mut() {
                        match prepared.evaluate_with_bound(bound) {
                            SweepOutcome::Executed(outcome) => {
                                self.performance.cpu_executed_candidates += 1;
                                outcome
                            }
                            SweepOutcome::Certified(bits) => {
                                counts.certified += 1;
                                self.performance.cpu_certified_candidates += 1;
                                Outcome::Damage(bits)
                            }
                        }
                    } else {
                        self.performance.cpu_executed_candidates += 1;
                        prepared.evaluate()
                    };
                    if let Some(timed) = timed {
                        self.performance.cpu_scoring_seconds += timed.elapsed().as_secs_f64();
                    }
                    outcomes.push(outcome);
                }
                outcomes
            }
        };
        check_cancelled()?;
        if outcomes.len() != candidates.len() {
            return Err("scorer returned incomplete batch outcomes".into());
        }
        let ranked_started = self.performance.timing_enabled.then(Instant::now);
        // All state mutation below targets a pending copy until the last
        // cancellation check. Failed/cancelled submissions cannot advance it.
        let mut next = state.clone();
        let mut ranked: Vec<_> = state
            .rank
            .iter()
            .map(|row| (*row, self.ranked(*row)))
            .collect();
        for (i, (candidate, outcome)) in candidates.into_iter().zip(outcomes).enumerate() {
            let failure = match outcome {
                Outcome::Refused(reason) => Some(reason),
                Outcome::NoPath => Some("MAIN returned no path"),
                Outcome::Damage(bits) if canonical_score_bits(bits).is_none() => {
                    Some("nonfinite or negative damage")
                }
                _ => None,
            };
            if let Some(reason) = failure {
                *next.failures.entry(reason.to_owned()).or_default() += 1;
                if next.unanswered_samples.len() < RETAINED_FAILURES {
                    next.unanswered_samples.push(state.completed + i as u64);
                }
            }
            if let Some(bits) = Search::tally_outcome(outcome, &mut counts) {
                ranked.push((
                    RankedOrdinal {
                        ordinal: state.completed + i as u64,
                        score_bits: bits,
                    },
                    Scored { candidate, bits },
                ));
            }
        }
        ranked.sort_unstable_by_key(|(_, scored)| rank(scored));
        next.rank = ranked.into_iter().take(KEEP).map(|(row, _)| row).collect();
        next.answered += counts.answered;
        next.refused += counts.refused;
        next.no_path += counts.no_path;
        next.outside_limits += counts.outside_limits;
        next.certified += counts.certified;
        next.completed = end;
        check_cancelled()?;
        *state = next;
        self.performance.batches_committed += 1;
        if let Some(timed) = ranked_started {
            self.performance.ranking_seconds += timed.elapsed().as_secs_f64();
        }
        if let Some(timed) = started {
            self.performance.advance_seconds += timed.elapsed().as_secs_f64();
        }
        Ok(())
    }
}
