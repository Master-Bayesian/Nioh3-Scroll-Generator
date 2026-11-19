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
    Cuda(MainDevice),
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
        let scorer = match kind {
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
                Scorer::Cuda(device)
            }
        };
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
        })
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
        let mut work = self.work()?;
        let outcomes = match &mut self.scorer {
            Scorer::Cuda(device) => self.search.prepared_device_outcomes_with_work(
                device,
                &candidates,
                &self.request,
                &mut work,
            )?,
            Scorer::Cpu(bounds) => {
                let engine = self.search.engine;
                candidates
                    .iter()
                    .map(|candidate| {
                        if let Err(outcome) =
                            self.search
                                .set_admission(candidate, &self.request, &mut work)
                        {
                            return outcome;
                        }
                        if let Some(bound) = bounds.as_mut() {
                            return match engine.evaluate_with_bound(
                                candidate,
                                self.search.pack,
                                &self.request.limits,
                                &mut work,
                                bound,
                            ) {
                                SweepOutcome::Executed(outcome) => outcome,
                                SweepOutcome::Certified(bits) => {
                                    counts.certified += 1;
                                    Outcome::Damage(bits)
                                }
                            };
                        }
                        engine.evaluate(
                            candidate,
                            self.search.pack,
                            &self.request.limits,
                            &mut work,
                        )
                    })
                    .collect()
            }
        };
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
                *state.failures.entry(reason.to_owned()).or_default() += 1;
                if state.unanswered_samples.len() < RETAINED_FAILURES {
                    state.unanswered_samples.push(state.completed + i as u64);
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
        state.rank = ranked.into_iter().take(KEEP).map(|(row, _)| row).collect();
        state.answered += counts.answered;
        state.refused += counts.refused;
        state.no_path += counts.no_path;
        state.outside_limits += counts.outside_limits;
        state.certified += counts.certified;
        state.completed = end;
        Ok(())
    }
}
