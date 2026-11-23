//! Outfit search over disjoint spaces of equipment and set choices.
//!
//! Fixed-attribute plans retain every admitted candidate. Attribute-moving plans use
//! profile screening and polish their shortlist. Strict CPU MAIN can certify
//! equivalent prepared inputs with the bytes read during a successful execution.

use crate::engine::{
    position_of, slot_of, Backpack, Candidate, Engine, ExactSetCapacities, Facts, Kernel, Limits,
    Outcome, PreparedCandidate, ReadBounds, SetState, SweepOutcome, SLOTS,
};
use crate::gpu::{CardOutcome, Gpu, GpuSweepError, MainDevice, NkSpace, Sweep};
pub use crate::pipeline::SET_REDUCTION_GROUP;
use crate::pipeline::{Profile, SetCapacity};
use crate::search_control::{
    RunControl, SearchBackend, SearchInterrupted, SearchObserver, SearchProgress, SearchStage,
};
use std::collections::{BinaryHeap, HashSet};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[path = "exact_sweep.rs"]
mod exact_sweep;
pub use exact_sweep::{
    EnumerationOrder, ExactAudit, ExactAuditBatch, ExactCheckpoint, ExactPerformance, ExactSweep,
    ScorerKind, SetPruning,
};

const KEEP: usize = 400;
const WEIGHT_REFLECTION_GROUP: u16 = 0x267E;
const SHOWN: usize = 40;
const POLISH_ROUNDS: usize = 40;
const POINT_PASSES: usize = 4;
const MISSED_SAMPLE: usize = 200;
const CHUNK: u64 = 2048;
const CPU_CERTIFICATE_PREFERENCE_LIMIT: u64 = 32_768;
/// Protocol counts are JSON numbers consumed by JavaScript. Do not publish
/// saturated u64 products or integers the renderer cannot represent exactly.
const MAX_EXACT_COVERAGE: u64 = (1u64 << 53) - 1;
/// Outfits a search evaluates on both processors before it trusts the card, and card answers kept per sweep.
const VERIFY: usize = 512;
const PREPARED_DEVICE_BATCH: u64 = 128;
const PREPARED_DEVICE_VERIFY: u64 = 16;
const CARD_HITS: usize = 1 << 22;
const CARD_DIRECT_LIMIT: u64 = 8 * CARD_HITS as u64;
const CARD_MISSES: usize = 1 << 16;
/// Reasons that begin with this mean the kernel computed something else than the engine; it is never used again.
const DISAGREES: &str = "the graphics-card kernel disagrees with the engine";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetOption {
    /// The set's side key, as the group table names it.
    pub key: u32,
    /// The set effect that must be active: worn pieces, plus one when a worn item lowers the requirement.
    pub pieces: usize,
}

/// At least `at_least` of the options must hold at once, so "two of these three sets" is one rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetRule {
    pub options: Vec<SetOption>,
    pub at_least: usize,
}

/// Active effect tier of a set: the pieces worn, plus the reduction once any piece is worn.
pub fn active_pieces(worn: usize, reduction: bool) -> usize {
    if worn == 0 {
        0
    } else {
        worn + usize::from(reduction)
    }
}

impl SetRule {
    fn holds(&self, worn: impl Fn(u32) -> Option<usize>, reduction: bool) -> bool {
        let met = self
            .options
            .iter()
            .filter(|o| worn(o.key).is_some_and(|n| active_pieces(n, reduction) >= o.pieces))
            .count();
        met >= self.at_least
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    /// Every rule must hold; none means any outfit.
    pub sets: Vec<SetRule>,
    pub limits: Limits,
    /// Move attribute points between attributes while polishing, keeping their total.
    pub move_points: bool,
    pub threads: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Scored {
    pub bits: u32,
    pub candidate: Candidate,
}

impl Scored {
    pub fn damage(&self) -> f32 {
        f32::from_bits(self.bits)
    }
}

/// Better first: more damage, then the smaller candidate, so equal damage always ranks the same way.
fn rank(s: &Scored) -> (std::cmp::Reverse<u32>, Candidate) {
    (std::cmp::Reverse(s.bits), s.candidate)
}

struct Ranked(Scored);

impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        rank(&self.0) == rank(&other.0)
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        rank(&self.0).cmp(&rank(&other.0))
    }
}

/// The best `KEEP` seen, with the worst on top so it can be dropped.
#[derive(Default)]
struct Best(BinaryHeap<Ranked>);

impl Best {
    fn offer(&mut self, s: Scored) {
        if self.0.len() < KEEP {
            self.0.push(Ranked(s));
        } else if self.0.peek().is_some_and(|worst| rank(&s) < rank(&worst.0)) {
            self.0.pop();
            self.0.push(Ranked(s));
        }
    }

    fn strictly_below_cutoff(&self, bits: u32) -> bool {
        self.0.len() == KEEP && self.0.peek().is_some_and(|worst| bits < worst.0.bits)
    }

    fn sorted(self) -> Vec<Scored> {
        let mut v: Vec<Scored> = self.0.into_iter().map(|r| r.0).collect();
        v.sort_by_key(rank);
        v
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub answered: u64,
    pub refused: u64,
    pub no_path: u64,
    pub outside_limits: u64,
    /// Answered outfits whose exact score was certified without another MAIN run.
    pub certified: u64,
    /// Certified outfits strictly below a full ranking heap's cutoff.
    pub bound_pruned: u64,
    pub card_batches: u64,
    pub card_overflows: u64,
}

impl Counts {
    fn add(&mut self, o: &Counts) {
        self.answered += o.answered;
        self.refused += o.refused;
        self.no_path += o.no_path;
        self.outside_limits += o.outside_limits;
        self.certified += o.certified;
        self.bound_pruned += o.bound_pruned;
        self.card_batches += o.card_batches;
        self.card_overflows += o.card_overflows;
    }
}

/// Items of one position that stay in the search, and the whole group they were chosen from.
#[derive(Clone, Debug)]
struct Group {
    kept: Vec<u32>,
    all: Vec<u32>,
}

#[derive(Clone, Debug)]
struct Space {
    /// For each searched position, the items it may hold.
    choices: Vec<(usize, Vec<u32>)>,
    size: u64,
}

impl Space {
    fn new(choices: Vec<(usize, Vec<u32>)>) -> Space {
        let size = choices
            .iter()
            .try_fold(1u64, |a, (_, c)| a.checked_mul(c.len() as u64))
            .unwrap_or(u64::MAX);
        Space { choices, size }
    }

    // Split only choice lists. The parts are a disjoint cover of the same
    // Cartesian product and retain the original pool row numbers.
    fn split_for(
        &self,
        limit: u64,
        emit: &mut impl FnMut(Space) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.size <= limit {
            return emit(self.clone());
        }
        let axis = self
            .choices
            .iter()
            .rposition(|(_, rows)| rows.len() > 1)
            .ok_or("a card space cannot be split to its result capacity")?;
        let rows = &self.choices[axis].1;
        let rest = self.size / rows.len() as u64;
        let width = (limit / rest).max(1) as usize;
        for slice in rows.chunks(width) {
            let mut choices = self.choices.clone();
            choices[axis].1 = slice.to_vec();
            Space::new(choices).split_for(limit, emit)?;
        }
        Ok(())
    }

    fn candidate(&self, mut index: u64, attrs: [i32; 7]) -> Candidate {
        let mut items = [None; SLOTS];
        for (position, rows) in &self.choices {
            let n = rows.len() as u64;
            items[*position] = Some(rows[(index % n) as usize]);
            index /= n;
        }
        Candidate { attrs, items }
    }
}

enum SetPlanning<'a> {
    ExistingProfiles,
    ExactCapacity(&'a ExactSetCapacities),
    NoPruning,
}

/// The equipment slots that hold accessories.
const ACCESSORY_SLOTS: [u32; 2] = [9, 10];

/// One way to fill a searched unit (a position, or the accessory pair): the pieces it adds to each rule key, whether it
/// lowers set requirements, and its alternative position assignments, each part of its own space.
struct Choice {
    pieces: Vec<usize>,
    reduction: bool,
    parts: Vec<Vec<(usize, Vec<u32>)>>,
}

struct Unit {
    choices: Vec<Choice>,
}

struct SetBounds<'a> {
    keys: &'a [u32],
    rules: &'a [SetRule],
    remaining: Vec<SetCapacity>,
}

impl<'a> SetBounds<'a> {
    fn new(units: &[Unit], keys: &'a [u32], rules: &'a [SetRule]) -> Self {
        let mut remaining = vec![SetCapacity::empty(keys)];
        for unit in units {
            let previous = &remaining[remaining.len() - 1];
            let pieces = previous
                .pieces
                .iter()
                .enumerate()
                .map(|(i, count)| {
                    count + unit.choices.iter().map(|c| c.pieces[i]).max().unwrap_or(0)
                })
                .collect();
            let reduction = previous.reduction || unit.choices.iter().any(|c| c.reduction);
            remaining.push(SetCapacity { pieces, reduction });
        }
        Self {
            keys,
            rules,
            remaining,
        }
    }

    fn can_complete(&self, remaining: usize, worn: &[usize], reduction: bool) -> bool {
        let capacity = &self.remaining[remaining];
        let optimistic_pieces = |key| {
            self.keys
                .iter()
                .position(|&k| k == key)
                .map(|i| worn[i] + capacity.pieces[i])
        };
        // Each completion is componentwise below these capacities. Options can
        // compete for a slot, which can only make the optimistic rule looser.
        self.rules
            .iter()
            .all(|rule| rule.holds(optimistic_pieces, reduction || capacity.reduction))
    }

    fn visit(
        &self,
        units: &[Unit],
        pick: &mut [usize],
        worn: &mut [usize],
        reduction: bool,
        emit: &mut impl FnMut(&[usize]) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if !self.can_complete(units.len(), worn, reduction) {
            return ControlFlow::Continue(());
        }
        let Some((unit, rest)) = units.split_last() else {
            return emit(pick);
        };
        // Assign the slowest-changing unit first to preserve the outfit indexes.
        for (i, choice) in unit.choices.iter().enumerate() {
            pick[rest.len()] = i;
            for (w, p) in worn.iter_mut().zip(&choice.pieces) {
                *w += p;
            }
            let flow = self.visit(rest, pick, worn, reduction || choice.reduction, emit);
            for (w, p) in worn.iter_mut().zip(&choice.pieces) {
                *w -= p;
            }
            flow?;
        }
        ControlFlow::Continue(())
    }
}

/// Equipment assignments after set feasibility and optional profile screening.
pub struct Plan {
    spaces: Vec<Space>,
    groups: Vec<(usize, Group)>,
    pub full_size: u64,
    /// Requested outfits whose weapon source inputs are not independently produced.
    /// Counted before set-bound pruning; no formula value is assigned to them.
    pub source_uncomputed: u64,
    complete: bool,
    /// Native constraints and the loaded kernel capability are distinct facts.
    native_set_constraints: bool,
    gpu_native_sets: bool,
    legacy_source_risk: bool,
}

impl Plan {
    pub fn is_complete(&self) -> bool {
        self.complete && self.source_uncomputed == 0 && self.full_size <= MAX_EXACT_COVERAGE
    }

    /// Native set predicates use the CPU only when the loaded DLL lacks their exports.
    /// Other small complete searches can avoid GPU contention using read witnesses.
    pub fn prefers_bounded_cpu(&self) -> bool {
        self.legacy_source_risk
            || (self.native_set_constraints && !self.gpu_native_sets)
            || (self.complete
                && self.size() <= CPU_CERTIFICATE_PREFERENCE_LIMIT
                && !std::env::var("NIOH3_LOADOUT_GPU").is_ok_and(|mode| mode == "1"))
    }

    /// Iterate the same assignments that the production sweep covers.
    pub fn candidates(&self, attrs: [i32; 7]) -> impl Iterator<Item = Candidate> + '_ {
        self.spaces
            .iter()
            .flat_map(move |space| (0..space.size).map(move |index| space.candidate(index, attrs)))
    }

    /// Per position class: (position, items kept, items in the class).
    pub fn kept_counts(&self) -> Vec<(usize, usize, usize)> {
        self.groups
            .iter()
            .map(|(p, g)| (*p, g.kept.len(), g.all.len()))
            .collect()
    }

    pub fn spaces(&self) -> usize {
        self.spaces.len()
    }

    pub fn size(&self) -> u64 {
        Self::space_size(&self.spaces)
    }

    fn space_size(spaces: &[Space]) -> u64 {
        spaces
            .iter()
            .map(|s| s.size)
            .fold(0u64, u64::saturating_add)
    }

    /// Where each space starts in the concatenated numbering.
    fn starts(&self) -> Vec<u64> {
        Self::space_starts(&self.spaces)
    }

    fn space_starts(spaces: &[Space]) -> Vec<u64> {
        spaces
            .iter()
            .scan(0u64, |at, s| {
                let here = *at;
                *at = at.saturating_add(s.size);
                Some(here)
            })
            .collect()
    }

    /// Outfit `i` of the concatenated numbering.
    fn nth(&self, starts: &[u64], i: u64, attrs: [i32; 7]) -> Candidate {
        Self::space_candidate(&self.spaces, starts, i, attrs)
    }

    fn space_candidate(spaces: &[Space], starts: &[u64], i: u64, attrs: [i32; 7]) -> Candidate {
        let k = starts.partition_point(|&s| s <= i) - 1;
        spaces[k].candidate(i - starts[k], attrs)
    }

    fn visit_card_batches(
        &self,
        capacity: u64,
        mut visit: impl FnMut(&[Space]) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut current = Vec::new();
        let mut size = 0;
        for space in &self.spaces {
            space.split_for(capacity, &mut |part| {
                if size + part.size > capacity {
                    visit(&current)?;
                    current.clear();
                    size = 0;
                }
                size += part.size;
                current.push(part);
                Ok(())
            })?;
        }
        if !current.is_empty() {
            visit(&current)?;
        }
        Ok(())
    }

    /// The spaces as the card's index lists.
    fn card_spaces(spaces: &[Space]) -> (Vec<NkSpace>, Vec<u32>) {
        let mut lists = Vec::new();
        let spaces = spaces
            .iter()
            .map(|s| {
                let mut sp = NkSpace {
                    n: s.choices.len() as u32,
                    slot: [0; SLOTS],
                    offset: [0; SLOTS],
                    radix: [0; SLOTS],
                };
                for (a, (position, rows)) in s.choices.iter().enumerate() {
                    sp.slot[a] = slot_of(*position) as i32;
                    sp.offset[a] = lists.len() as u32;
                    sp.radix[a] = rows.len() as u32;
                    lists.extend_from_slice(rows);
                }
                sp
            })
            .collect();
        (spaces, lists)
    }
}

#[derive(Default)]
struct CardRanking {
    best: Best,
    counts: Counts,
    missed: Vec<Candidate>,
}

impl CardRanking {
    // The running maximum never exceeds the final maximum. Every score at
    // least this final threshold therefore passed the device's admission
    // test. Once that covers KEEP entries, the partition's top is complete.
    fn covers_top(sw: &Sweep, keep: f32) -> bool {
        if keep == 0.0 {
            return true;
        }
        let top = sw
            .hits
            .iter()
            .map(|hit| f32::from_bits(hit.bits))
            .fold(0.0f32, f32::max);
        sw.hits
            .iter()
            .filter(|hit| f32::from_bits(hit.bits) >= top * keep)
            .take(KEEP)
            .count()
            == KEEP
    }

    fn merge(&mut self, spaces: &[Space], attrs: [i32; 7], sw: Sweep) -> Result<(), String> {
        let total = Plan::space_size(spaces);
        if sw.counts.iter().sum::<u64>() != total || sw.hit_total != sw.hits.len() as u64 {
            return Err(
                "card batch did not cover its complete rank buffer and candidate space".into(),
            );
        }
        self.counts.add(&Counts {
            refused: sw.counts[0],
            answered: sw.counts[1],
            no_path: sw.counts[2],
            outside_limits: sw.counts[3],
            card_batches: 1,
            ..Counts::default()
        });
        for h in sw.hits {
            let space = spaces
                .get(h.space as usize)
                .ok_or("kernel named an unknown space")?;
            if h.index >= space.size {
                return Err("kernel named an outfit outside its space".into());
            }
            let bits = canonical_score_bits(h.bits)
                .ok_or("kernel returned nonfinite or negative damage")?;
            if !self.best.strictly_below_cutoff(bits) {
                self.best.offer(Scored {
                    bits,
                    candidate: space.candidate(h.index, attrs),
                });
            }
        }
        if sw.missed.iter().any(|&i| i >= total) {
            return Err("kernel named an outfit outside the batch".into());
        }
        if !sw.missed.is_empty() {
            let starts = Plan::space_starts(spaces);
            let step = sw.missed.len().div_ceil(VERIFY).max(1);
            self.missed.extend(
                sw.missed
                    .into_iter()
                    .step_by(step)
                    .map(|i| Plan::space_candidate(spaces, &starts, i, attrs)),
            );
        }
        Ok(())
    }
}

/// Whether an outfit puts one item in both accessory slots, which the game does not allow.
fn wears_twice(c: &Candidate) -> bool {
    let [one, two] = ACCESSORY_SLOTS.map(|s| position_of(s).unwrap_or(0));
    c.items[one].is_some() && c.items[one] == c.items[two]
}

/// Unsigned bit ordering agrees with finite nonnegative damage ordering after
/// canonicalizing both encodings of zero. Zero remains a feasible result.
fn canonical_score_bits(bits: u32) -> Option<u32> {
    let value = f32::from_bits(bits);
    (value.is_finite() && value >= 0.0).then_some(if value == 0.0 { 0 } else { bits })
}

fn count_damage(counts: &mut Counts, bits: u32) -> Option<u32> {
    match canonical_score_bits(bits) {
        Some(score) => {
            counts.answered += 1;
            Some(score)
        }
        None => {
            counts.refused += 1;
            None
        }
    }
}

/// Equal admitted damage bits (with canonical zero), equal failure categories,
/// or a valid card answer where the CPU has no compiled path. Merged card paths
/// need their separate original-MAIN acceptance; this check does not prove them.
pub fn agree(cpu: &Outcome, card: &CardOutcome) -> bool {
    matches!(
        (cpu, card),
        (Outcome::Refused(_), CardOutcome::Refused)
            | (Outcome::NoPath, CardOutcome::NoPath)
            | (Outcome::OutsideLimits, CardOutcome::OutsideLimits)
    ) || matches!((cpu, card), (Outcome::NoPath, CardOutcome::Damage(bits)) if canonical_score_bits(*bits).is_some())
        || matches!((cpu, card), (Outcome::Damage(a), CardOutcome::Damage(b))
            if canonical_score_bits(*a).is_some_and(|score| Some(score) == canonical_score_bits(*b)))
}

fn count(counts: &mut Counts, o: &CardOutcome) -> Option<u32> {
    match *o {
        CardOutcome::Damage(bits) => count_damage(counts, bits),
        CardOutcome::Refused => {
            counts.refused += 1;
            None
        }
        CardOutcome::NoPath => {
            counts.no_path += 1;
            None
        }
        CardOutcome::OutsideLimits => {
            counts.outside_limits += 1;
            None
        }
    }
}

/// Which processor evaluated the outfits of a search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backend {
    Card,
    /// The CPU, with why the card was not used.
    Cpu(String),
}

/// A terminal search failure is not an unanswered candidate or an infeasible domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchFailure {
    DomainCountTooLarge,
    SourceAttributesUnproven,
    CpuFallbackBudget {
        planned: u64,
        limit: u64,
        reason: String,
    },
}
impl std::fmt::Display for SearchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DomainCountTooLarge => f.write_str("LOADOUT_DOMAIN_COUNT_TOO_LARGE"),
            Self::SourceAttributesUnproven => f.write_str("LOADOUT_SOURCE_ATTRIBUTES_UNPROVEN"),
            Self::CpuFallbackBudget {
                planned,
                limit,
                reason,
            } => {
                write!(f,
                "LOADOUT_SPACE_TOO_LARGE_CPU: {planned} outfits exceed {limit}; backend: {reason}")
            }
        }
    }
}

pub struct Report {
    pub failure: Option<SearchFailure>,
    pub best: Vec<(Scored, Option<Facts>)>,
    pub backend: Backend,
    pub counts: Counts,
    pub searched: u64,
    pub full_size: u64,
    /// Requested Cartesian outfits excluded by source admission before any set pruning.
    /// Distinct from a formula path failure or an evaluated constraint rejection.
    pub source_uncomputed: u64,
    pub polish_rounds: usize,
    pub seconds: f64,
    /// A sample of outfits no compiled path covers; any of them could be better than the best shown.
    pub missed: Vec<Candidate>,
    pub complete: bool,
}

/// Optional caller policy: an omitted axis keeps the bound character's item.
/// Enhanced pools compare with the corresponding enhanced current record.
#[derive(Clone, Copy, Debug)]
pub enum CurrentChoicePolicy {
    Captured,
    Enhanced,
}

/// Source-production scope is separate from the player's requested weapon TYPE.
/// A limited source producer still enumerates the requested domain and reports
/// uncomputed outfits; it cannot silently turn a TYPE request into a weapon pin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceAdmissionPolicy {
    /// The installed source contract admits every submitted same-TYPE record.
    #[default]
    OwnedType,
    /// Only the unchanged captured weapon body has an admitted source profile.
    /// Changed weapons remain uncomputed until their common producer is closed.
    CapturedWeaponOnly,
}

impl SourceAdmissionPolicy {
    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::OwnedType => None,
            Self::CapturedWeaponOnly => Some("weapon_source_rebuild_unproven"),
        }
    }
}

pub struct Search<'a> {
    engine: &'a Engine,
    pack: &'a Backpack,
    /// The attack's live groups plus the set reduction, which moves damage only through set tiers.
    live: HashSet<u16>,
    control: Option<&'a RunControl<'a>>,
    cpu_fallback_limit: Option<u64>,
    current_choices: Option<CurrentChoicePolicy>,
    source_admission: SourceAdmissionPolicy,
    legacy_pool_risk: bool,
}

impl<'a> Search<'a> {
    pub fn new(engine: &'a Engine, pack: &'a Backpack) -> Search<'a> {
        let mut live = engine.live_groups.clone();
        live.insert(SET_REDUCTION_GROUP);
        Search {
            engine,
            pack,
            live,
            control: None,
            cpu_fallback_limit: None,
            current_choices: None,
            source_admission: SourceAdmissionPolicy::OwnedType,
            legacy_pool_risk: engine.legacy_source_pool_risk(pack),
        }
    }

    /// Preserve the caller's CPU budget even after card admission or execution fails.
    pub fn with_cpu_fallback_limit(mut self, limit: u64) -> Self {
        self.cpu_fallback_limit = Some(limit);
        self
    }

    /// Explicitly include retaining each currently bound slot when its record
    /// is absent from the submitted pool. This does not invent an unequip action.
    pub fn with_current_choices(mut self, policy: CurrentChoicePolicy) -> Self {
        self.current_choices = Some(policy);
        self
    }

    /// Retain all requested TYPE rows in coverage accounting while limiting
    /// native execution to inputs the installed source producer can rebuild.
    pub fn with_source_admission(mut self, policy: SourceAdmissionPolicy) -> Self {
        self.source_admission = policy;
        self
    }

    fn weapon_row_admitted(&self, row: u32) -> bool {
        match self.source_admission {
            SourceAdmissionPolicy::OwnedType => true,
            SourceAdmissionPolicy::CapturedWeaponOnly => {
                self.pack.pool[row as usize].as_slice() == self.engine.worn_item(9)
            }
        }
    }

    fn legacy_source_risk(&self, request: &Request) -> bool {
        self.legacy_pool_risk || (request.move_points && !self.engine.has_dynamic_sources())
    }

    fn legacy_card_admission(&self, attrs: &[i32; 7]) -> Result<(), String> {
        if self.legacy_pool_risk
            || (!self.engine.has_dynamic_sources() && *attrs != self.engine.start_attrs)
        {
            return Err("legacy shared-source dependencies require guarded CPU evaluation".into());
        }
        Ok(())
    }

    fn current_record(&self, position: usize) -> Option<[u8; crate::engine::ITEM]> {
        let policy = self.current_choices?;
        let mut current =
            <[u8; crate::engine::ITEM]>::try_from(self.engine.worn_item(position)).ok()?;
        if matches!(policy, CurrentChoicePolicy::Enhanced) && current[0x30] <= 5 {
            self.engine.enhance(&mut current);
        }
        Some(current)
    }

    fn current_missing(&self, position: usize, rows: &[u32]) -> bool {
        self.current_record(position).is_some_and(|current| {
            !rows
                .iter()
                .any(|&row| self.pack.pool[row as usize] == current)
        })
    }

    fn current_accessories_missing(&self, rows: &[u32]) -> [bool; 2] {
        let mut represented = Vec::new();
        ACCESSORY_SLOTS.map(|slot| {
            let Some(position) = position_of(slot) else {
                return false;
            };
            let Some(current) = self.current_record(position) else {
                return false;
            };
            // The two captured records can be byte-identical while belonging
            // to distinct owned items. A single pool row represents only one.
            if let Some(&row) = rows.iter().find(|&&row| {
                !represented.contains(&row) && self.pack.pool[row as usize] == current
            }) {
                represented.push(row);
                false
            } else {
                true
            }
        })
    }

    fn cpu_budget_failure(&self, plan: &Plan, t: Instant, reason: &str) -> Option<Report> {
        let limit = self.cpu_fallback_limit?;
        if plan.size() <= limit {
            return None;
        }
        Some(Report {
            failure: Some(SearchFailure::CpuFallbackBudget {
                planned: plan.size(),
                limit,
                reason: reason.to_owned(),
            }),
            best: Vec::new(),
            backend: Backend::Cpu(reason.to_owned()),
            counts: Counts::default(),
            searched: 0,
            full_size: plan.full_size,
            source_uncomputed: plan.source_uncomputed,
            polish_rounds: 0,
            seconds: t.elapsed().as_secs_f64(),
            missed: Vec::new(),
            complete: false,
        })
    }

    fn stopped(&self) -> bool {
        self.control.is_some_and(|control| !control.checkpoint())
    }

    fn publish(
        &self,
        stage: SearchStage,
        backend: SearchBackend,
        completed: u64,
        total: Option<u64>,
        attempt: u32,
    ) {
        if let Some(control) = self.control {
            let mut progress = SearchProgress::new(stage, backend, completed, total);
            progress.attempt = attempt;
            control.report(progress);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn card_sweep(
        &self,
        g: &mut Gpu,
        spaces: &[NkSpace],
        lists: &[u32],
        attrs: &[i32; 7],
        limits: &Limits,
        sets: &[SetRule],
        keep: f32,
        capacity: usize,
        miss_capacity: usize,
        settled: u64,
        total: u64,
        batch: u64,
    ) -> Result<Sweep, String> {
        self.legacy_card_admission(attrs)?;
        let Some(control) = self.control else {
            return g.sweep_with_sets(
                spaces,
                lists,
                attrs,
                limits,
                sets,
                keep,
                capacity,
                miss_capacity,
            );
        };
        if !control.checkpoint() {
            return Err("search interrupted".into());
        }
        if !g.supports_observed_search() {
            // Legacy kernels remain usable through small host-owned partitions.
            // Stop is checked before and after each synchronous foreign call.
            let sweep = g.sweep_with_sets(
                spaces,
                lists,
                attrs,
                limits,
                sets,
                keep,
                capacity,
                miss_capacity,
            )?;
            if !control.checkpoint() {
                return Err("search interrupted".into());
            }
            return Ok(sweep);
        }
        let mut observer = |completed, batch_total| {
            let mut progress = SearchProgress::new(
                SearchStage::Evaluation,
                SearchBackend::Card,
                settled,
                Some(total),
            );
            progress.batch_completed = completed;
            progress.batch_total = Some(batch_total);
            progress.batch_index = batch;
            control.report(progress)
        };
        match g.sweep_observed_with_sets(
            spaces,
            lists,
            attrs,
            limits,
            sets,
            keep,
            capacity,
            miss_capacity,
            &mut observer,
        ) {
            Ok(sweep) => Ok(sweep),
            Err(GpuSweepError::Cancelled) => Err("search interrupted".into()),
            Err(GpuSweepError::ObserverFailed) => {
                control.observer_failed();
                Err("search observer failed".into())
            }
            Err(GpuSweepError::Failure(message)) => Err(message),
        }
    }

    /// An observed run never returns a ranking after cancellation or observer
    /// failure. The scoped control is private and cannot escape into legacy runs.
    pub fn run_limited_by_observed(
        &self,
        request: &Request,
        limit_for: impl FnOnce(&Plan) -> u64,
        observer: &dyn SearchObserver,
    ) -> Result<Option<(u64, Report)>, SearchInterrupted> {
        let control = RunControl::new(observer);
        let scoped = Search {
            engine: self.engine,
            pack: self.pack,
            live: self.live.clone(),
            control: Some(&control),
            cpu_fallback_limit: self.cpu_fallback_limit,
            current_choices: self.current_choices,
            source_admission: self.source_admission,
            legacy_pool_risk: self.legacy_pool_risk,
        };
        scoped.publish(SearchStage::Planning, SearchBackend::Unselected, 0, None, 0);
        control.outcome()?;
        let result = scoped.run_limited_by(request, limit_for);
        control.outcome()?;
        Ok(result)
    }

    fn profiles(&self, rows: &[u32], live: &HashSet<u16>) -> Vec<Option<Profile>> {
        let e = self.engine;
        rows.iter()
            .map(|&r| {
                e.tables().profile(
                    e.character(),
                    &e.start_attrs,
                    &self.pack.pool[r as usize],
                    live,
                )
            })
            .collect()
    }

    /// Sets present in the pool, with how many items each searched slot holds.
    pub fn sets(&self) -> Vec<(u32, Vec<(u32, usize)>)> {
        let mut out: Vec<(u32, Vec<(u32, usize)>)> = Vec::new();
        for &(slot, first, count) in &self.pack.ranges {
            let rows: Vec<u32> = (first..first + count).collect();
            for p in self.profiles(&rows, &self.live).into_iter().flatten() {
                for &key in &p.sides {
                    let at = match out.iter().position(|s| s.0 == key) {
                        Some(i) => i,
                        None => {
                            out.push((key, Vec::new()));
                            out.len() - 1
                        }
                    };
                    match out[at].1.iter_mut().find(|e| e.0 == slot) {
                        Some(e) => e.1 += 1,
                        None => out[at].1.push((slot, 1)),
                    }
                }
            }
        }
        out.sort_by_key(|(key, by)| {
            (
                std::cmp::Reverse(by.iter().map(|e| e.1).sum::<usize>()),
                *key,
            )
        });
        out
    }

    /// Pool rows of searched slots that carry the set `key`.
    pub fn set_rows(&self, key: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for &(_, first, count) in &self.pack.ranges {
            let rows: Vec<u32> = (first..first + count).collect();
            for (r, p) in rows.iter().zip(self.profiles(&rows, &self.live)) {
                if p.is_some_and(|p| p.sides.contains(&key)) {
                    out.push(*r);
                }
            }
        }
        out
    }

    fn kept(&self, rows: &[u32], slot: u32, request: &Request) -> Vec<u32> {
        // A candidate can change the completed core mask and the guardian8508
        // gate. Local item profiles do not bound either rebuilt source stage.
        if self.engine.has_dynamic_sources() {
            return rows.to_vec();
        }
        if self.legacy_source_risk(request) {
            return rows.to_vec();
        }
        // Fixed-hit dominance does not certify candidate-dependent depletion.
        if self.engine.is_held_sequence() {
            return rows.to_vec();
        }
        // Physical weapon dominance does not order categorical elements
        // or the target's response to their separate damage channels.
        if slot == 0 && self.engine.uses_weapon_element() {
            return rows.to_vec();
        }
        let mut live = self.live.clone();
        if request.limits.min_defense > 0 || request.limits.min_health > 0 {
            live.extend(&self.engine.survival_groups);
        }
        // A partial search retains the captured gear in all other positions.
        // Without a proven full-outfit upper bound, any finite load constraint
        // must preserve lighter alternatives during dominance.
        let watch_weight = request.limits.max_load.is_finite();
        let weight_affects_damage = self.live.contains(&WEIGHT_REFLECTION_GROUP);
        let profiles = self.profiles(rows, &live);
        let beats = |a: &Profile, b: &Profile| {
            if a.sides != b.sides && !b.sides.is_empty() {
                return false;
            }
            // The native weight reflection reads the combined outfit weight.
            // Different contributions cannot establish slot-local dominance.
            if (weight_affects_damage && a.weight != b.weight)
                || (watch_weight && a.weight > b.weight)
            {
                return false;
            }
            if request.limits.min_defense > 0 {
                match (a.defense, b.defense) {
                    (Some(a), Some(b)) if a >= b => {}
                    _ => return false,
                }
            }
            if a.sides == b.sides && !b.sides.is_empty() && a.level < b.level {
                return false;
            }
            if !a.flag && b.flag {
                return false;
            }
            b.values
                .iter()
                .all(|&(k, v)| a.values.iter().find(|e| e.0 == k).map_or(0, |e| e.1) >= v)
        };
        // A weapon's attack comes from its model's tables, its level (with familiarity) and its +N, all growing
        // with those numbers, so a weapon only beats another of the same model, rarity and flags that it matches
        // or exceeds in level, +N, familiarity and every live effect.
        let pool = &self.pack.pool;
        let weapon_beats = |a: u32, b: u32| {
            let (x, y) = (&pool[a as usize], &pool[b as usize]);
            let (u16_at, u32_at) = (
                |r: &[u8], o: usize| u16::from_le_bytes([r[o], r[o + 1]]),
                |r: &[u8], o: usize| u32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]),
            );
            u16_at(x, 0) == u16_at(y, 0)
                && x[0x30] == y[0x30]
                && x[0x31] == y[0x31]
                && x[0x32] == y[0x32]
                && u32_at(x, 0x18) == u32_at(y, 0x18)
                && u16_at(x, 6) >= u16_at(y, 6)
                && u16_at(x, 0xA) >= u16_at(y, 0xA)
                && u32_at(x, 0x14) >= u32_at(y, 0x14)
        };
        let beats_at = |j: usize, i: usize| {
            let (Some(q), Some(p)) = (&profiles[j], &profiles[i]) else {
                return false;
            };
            beats(q, p) && (slot != 0 || weapon_beats(rows[j], rows[i]))
        };
        let mut out = Vec::new();
        for (i, p) in profiles.iter().enumerate() {
            // An item the tables cannot describe is never dropped.
            if p.is_none() {
                out.push(rows[i]);
                continue;
            }
            let dropped =
                (0..rows.len()).any(|j| j != i && beats_at(j, i) && (!beats_at(i, j) || j < i));
            if !dropped {
                out.push(rows[i]);
            }
        }
        out
    }

    /// Every set key a rule names, in first-seen order.
    fn rule_keys(request: &Request) -> Vec<u32> {
        let mut keys = Vec::new();
        for o in request.sets.iter().flat_map(|r| &r.options) {
            if !keys.contains(&o.key) {
                keys.push(o.key);
            }
        }
        keys
    }

    /// One position's items split by which named sets they carry and whether they lower set requirements, so
    /// dominance never trades one class for another and every outfit lands in exactly one space. With `layers` 2,
    /// an item stays unless two kept items beat it, as a pair of positions drawing from one pool needs.
    fn classes(
        &self,
        rows: &[u32],
        slot: u32,
        keys: &[u32],
        request: &Request,
        layers: Option<usize>,
        planning: &SetPlanning<'_>,
    ) -> Vec<(SetCapacity, Group)> {
        let mut classes: Vec<(SetCapacity, Vec<u32>)> = Vec::new();
        let profiles = match planning {
            SetPlanning::ExistingProfiles => self.profiles(rows, &self.live),
            SetPlanning::ExactCapacity(_) | SetPlanning::NoPruning => Vec::new(),
        };
        for (index, &row) in rows.iter().enumerate() {
            let class = match planning {
                SetPlanning::NoPruning => SetCapacity::empty(keys),
                SetPlanning::ExactCapacity(inputs) => inputs.rows[row as usize].clone(),
                SetPlanning::ExistingProfiles => profiles[index].as_ref().map_or_else(
                    // Unknown producer state cannot prove a set impossible.
                    || SetCapacity {
                        pieces: vec![7; keys.len()],
                        reduction: true,
                    },
                    |p| {
                        let lines = self
                            .engine
                            .tables()
                            .effect_lines(&self.pack.pool[row as usize]);
                        SetCapacity {
                            // Free sets can count repeated lines; fixed sets can
                            // deduplicate them. Count each possible submission.
                            pieces: keys
                                .iter()
                                .map(|key| {
                                    if p.sides.contains(key) {
                                        lines.iter().filter(|line| line.2 == *key).count()
                                    } else {
                                        0
                                    }
                                })
                                .collect(),
                            // An entry with value zero still lowers requirements.
                            reduction: p.values.iter().any(|e| e.0 == SET_REDUCTION_GROUP),
                        }
                    },
                ),
            };
            match classes.iter_mut().find(|c| c.0 == class) {
                Some(c) => c.1.push(row),
                None => classes.push((class, vec![row])),
            }
        }
        classes.sort_by(|a, b| a.0.cmp(&b.0));
        classes
            .into_iter()
            .map(|(class, all)| {
                let Some(layers) = layers else {
                    return (
                        class,
                        Group {
                            kept: all.clone(),
                            all,
                        },
                    );
                };
                // Beating is a strict order, so the items left after removing the kept layer that beat an item
                // include one that beats it too: two layers give every dropped item two distinct kept betters.
                let mut kept = Vec::new();
                let mut rest = all.clone();
                for _ in 0..layers {
                    let layer = self.kept(&rest, slot, request);
                    rest.retain(|r| !layer.contains(r));
                    kept.extend(layer);
                }
                kept.sort_unstable();
                (class, Group { kept, all })
            })
            .collect()
    }

    pub fn plan(&self, request: &Request) -> Plan {
        let complete_limit = (!request.move_points).then_some(u64::MAX);
        self.build_plan(request, complete_limit)
    }

    fn build_plan(&self, request: &Request, complete_limit: Option<u64>) -> Plan {
        self.build_plan_with_sets(request, complete_limit, SetPlanning::ExistingProfiles)
    }

    fn build_plan_with_sets(
        &self,
        request: &Request,
        complete_limit: Option<u64>,
        planning: SetPlanning<'_>,
    ) -> Plan {
        let complete = complete_limit.is_some();
        let legacy_source_risk = self.legacy_source_risk(request);
        let keys = Self::rule_keys(request);
        let mut units: Vec<Unit> = Vec::new();
        let mut groups: Vec<(usize, Group)> = Vec::new();
        let mut full_size = 1u64;
        let mut source_uncomputed = 0u64;
        let mut has_current_choices = false;
        for &(slot, first, count) in &self.pack.ranges {
            if ACCESSORY_SLOTS.contains(&slot) {
                continue;
            }
            let Some(position) = position_of(slot) else {
                continue;
            };
            let requested_rows: Vec<u32> = (first..first + count).collect();
            // Admission precedes all profile and set-bound logic. An unknown
            // weapon cannot disappear as an alleged infeasible or dominated row.
            let rows: Vec<u32> = requested_rows
                .iter()
                .copied()
                .filter(|&row| slot != 0 || self.weapon_row_admitted(row))
                .collect();
            let keep = if slot == 0 && !self.engine.current_weapon_matches_scene() {
                false
            } else if slot == 0
                && self.source_admission == SourceAdmissionPolicy::CapturedWeaponOnly
            {
                // None always keeps the actual captured body. An enhanced
                // hypothetical weapon is not that body, even if it came from
                // the same owned inventory record.
                self.current_choices.is_some() && rows.is_empty()
            } else {
                self.current_missing(position, &rows)
            };
            has_current_choices |= keep;
            let requested_count = u64::from(count) + u64::from(keep);
            source_uncomputed = source_uncomputed
                .saturating_mul(requested_count)
                .saturating_add(
                    full_size.saturating_mul((requested_rows.len() - rows.len()) as u64),
                );
            full_size = full_size.saturating_mul(requested_count);
            let classes = self.classes(
                &rows,
                slot,
                &keys,
                request,
                (!complete).then_some(1),
                &planning,
            );
            let mut choices: Vec<Choice> = classes
                .iter()
                .map(|(class, g)| Choice {
                    pieces: class.pieces.clone(),
                    reduction: class.reduction,
                    parts: vec![vec![(position, g.kept.clone())]],
                })
                .collect();
            if keep {
                choices.push(Choice {
                    pieces: vec![0; keys.len()],
                    reduction: false,
                    parts: vec![Vec::new()],
                });
            }
            units.push(Unit { choices });
            groups.extend(classes.into_iter().map(|(_, g)| (position, g)));
        }
        // A complete plan scores both orientations of each distinct pair.
        // The large screened plan retains its existing unordered search.
        let accessories: Vec<u32> = self
            .pack
            .ranges
            .iter()
            .filter(|r| ACCESSORY_SLOTS.contains(&r.0))
            .flat_map(|&(_, first, count)| first..first + count)
            .collect();
        if accessories.len() >= 2 || (self.current_choices.is_some() && !accessories.is_empty()) {
            let n = accessories.len() as u64;
            let divisor = if complete { 1 } else { 2 };
            let mut accessory_size = n * (n - 1) / divisor;
            let [one, two] = ACCESSORY_SLOTS.map(|s| position_of(s).unwrap_or(0));
            let classes = self.classes(
                &accessories,
                ACCESSORY_SLOTS[0],
                &keys,
                request,
                (!complete).then_some(2),
                &planning,
            );
            let mut choices = Vec::new();
            for (a, (ca, ga)) in classes.iter().enumerate() {
                for (cb, gb) in classes.iter().skip(a) {
                    let parts = if ca == cb {
                        ga.kept
                            .iter()
                            .enumerate()
                            .map(|(i, &row)| {
                                let others = if complete {
                                    ga.kept
                                        .iter()
                                        .copied()
                                        .filter(|&other| other != row)
                                        .collect()
                                } else {
                                    ga.kept[i + 1..].to_vec()
                                };
                                vec![(one, vec![row]), (two, others)]
                            })
                            .filter(|part| !part[1].1.is_empty())
                            .collect()
                    } else {
                        let mut parts = vec![vec![(one, ga.kept.clone()), (two, gb.kept.clone())]];
                        if complete {
                            parts.push(vec![(one, gb.kept.clone()), (two, ga.kept.clone())]);
                        }
                        parts
                    };
                    if !parts.is_empty() {
                        choices.push(Choice {
                            pieces: ca
                                .pieces
                                .iter()
                                .zip(&cb.pieces)
                                .map(|(a, b)| a + b)
                                .collect(),
                            reduction: ca.reduction || cb.reduction,
                            parts,
                        });
                    }
                }
            }
            let [keep_one, keep_two] = self.current_accessories_missing(&accessories);
            if keep_one {
                for (class, group) in &classes {
                    choices.push(Choice {
                        pieces: class.pieces.clone(),
                        reduction: class.reduction,
                        parts: vec![vec![(two, group.kept.clone())]],
                    });
                }
                accessory_size = accessory_size.saturating_add(n);
            }
            if keep_two {
                for (class, group) in &classes {
                    choices.push(Choice {
                        pieces: class.pieces.clone(),
                        reduction: class.reduction,
                        parts: vec![vec![(one, group.kept.clone())]],
                    });
                }
                accessory_size = accessory_size.saturating_add(n);
            }
            if keep_one && keep_two {
                choices.push(Choice {
                    pieces: vec![0; keys.len()],
                    reduction: false,
                    parts: vec![Vec::new()],
                });
                accessory_size = accessory_size.saturating_add(1);
            }
            has_current_choices |= keep_one || keep_two;
            // The legacy one-record pool retains both slots. An explicit current
            // policy additionally represents each legal one-slot replacement.
            if accessory_size == 0 {
                accessory_size = 1;
            }
            full_size = full_size.saturating_mul(accessory_size);
            source_uncomputed = source_uncomputed.saturating_mul(accessory_size);
            if !choices.is_empty() {
                units.push(Unit { choices });
            }
            for (_, g) in classes {
                groups.push((one, g.clone()));
                groups.push((two, g));
            }
        }
        // Refuse before traversing class products. Even a tiny admitted domain
        // cannot make an inexact requested coverage number publishable.
        if full_size > MAX_EXACT_COVERAGE {
            return Plan {
                spaces: Vec::new(),
                groups,
                full_size,
                source_uncomputed,
                complete: false,
                native_set_constraints: !request.sets.is_empty(),
                gpu_native_sets: !request.sets.is_empty() && self.engine.supports_set_constraints(),
                legacy_source_risk,
            };
        }
        let mut spaces = Vec::new();
        let mut size = 0u64;
        // Missing positions retain the bound character's actual equipment.
        // Reconstruct its native side state instead of subtracting profiles;
        // fixed sets and duplicate source submissions are not additive.
        let retained = match planning {
            SetPlanning::NoPruning => None,
            SetPlanning::ExactCapacity(inputs) => Some(inputs.retained.clone()),
            SetPlanning::ExistingProfiles
                if request.sets.is_empty()
                    || request.move_points
                    || self.engine.has_dynamic_sources()
                    || legacy_source_risk =>
            {
                None
            }
            SetPlanning::ExistingProfiles => {
                let mut work = self.engine.work();
                let state = if has_current_choices {
                    // Use the whole current side state as an optimistic bound
                    // when a replacement may retain its previous item.
                    let current = Candidate {
                        attrs: self.engine.start_attrs,
                        items: [None; SLOTS],
                    };
                    self.engine
                        .equipment_set_state(&current, self.pack, &mut work)
                } else {
                    self.engine
                        .retained_set_state(self.pack, &self.engine.start_attrs, &mut work)
                };
                state.ok().map(|state| SetCapacity {
                    pieces: keys
                        .iter()
                        .map(|key| {
                            state
                                .pieces
                                .iter()
                                .find(|entry| entry.0 == *key)
                                .map_or(0, |entry| entry.1)
                        })
                        .collect(),
                    reduction: state.reduction,
                })
            }
        };
        let rules = if retained.is_some() {
            &request.sets[..]
        } else {
            &[]
        };
        let mut initial = retained.unwrap_or_else(|| SetCapacity::empty(&keys));
        // Qualification may be gained after moving attributes; start-attribute
        // membership is not an upper bound for that search. It therefore uses
        // no set-bound pruning, just like unknown retained state. Exact
        // per-candidate admission later reports refusal rather than infeasible.
        // A dynamic SOURCE program can also change the inherited core state;
        // its per-item profiles are not certified set-feasibility bounds.
        let bounds = SetBounds::new(&units, &keys, rules);
        let visit = bounds.visit(
            &units,
            &mut vec![0; units.len()],
            &mut initial.pieces,
            initial.reduction,
            &mut |pick| {
                // Every way of combining the chosen choices' parts is one space.
                let mut partial = vec![Vec::new()];
                for (unit, &index) in units.iter().zip(pick) {
                    partial = partial
                        .iter()
                        .flat_map(|done| {
                            unit.choices[index].parts.iter().map(move |part| {
                                let mut next = done.clone();
                                next.extend(part.iter().cloned());
                                next
                            })
                        })
                        .collect();
                }
                for choices in partial {
                    let space = Space::new(choices);
                    size = size.saturating_add(space.size);
                    if complete_limit.is_some_and(|limit| size > limit) {
                        return ControlFlow::Break(());
                    }
                    if space.size > 0 {
                        spaces.push(space);
                    }
                }
                ControlFlow::Continue(())
            },
        );
        if visit.is_break() {
            return self.build_plan(request, None);
        }
        Plan {
            spaces,
            groups,
            full_size,
            source_uncomputed,
            complete,
            native_set_constraints: !request.sets.is_empty(),
            gpu_native_sets: !request.sets.is_empty() && self.engine.supports_set_constraints(),
            legacy_source_risk,
        }
    }

    fn evaluate_all(
        &self,
        list: &[Candidate],
        request: &Request,
        card: Option<&mut Gpu>,
        attempt: u32,
    ) -> (Vec<Option<u32>>, Counts) {
        if let Some(g) = card.filter(|g| {
            !self.legacy_source_risk(request)
                && (request.sets.is_empty() || g.supports_set_constraints())
        }) {
            let mut bits = Vec::with_capacity(list.len());
            let mut counts = Counts::default();
            let mut failed = false;
            for batch in list.chunks(if self.control.is_some() {
                512
            } else {
                list.len().max(1)
            }) {
                if self.stopped() {
                    return (vec![None; list.len()], counts);
                }
                match g.cases_with_sets(batch, &request.limits, &request.sets) {
                    Ok(out) => bits.extend(out.iter().map(|outcome| count(&mut counts, outcome))),
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
                self.publish(
                    SearchStage::Polishing,
                    SearchBackend::Card,
                    bits.len() as u64,
                    Some(list.len() as u64),
                    attempt,
                );
            }
            if !failed {
                return (bits, counts);
            }
        }
        let next = AtomicU64::new(0);
        let results = Mutex::new((vec![None; list.len()], Counts::default()));
        std::thread::scope(|scope| {
            for _ in 0..request.threads.max(1) {
                scope.spawn(|| {
                    let mut w = self.engine.work();
                    let mut local = Vec::new();
                    let mut counts = Counts::default();
                    loop {
                        let start = next.fetch_add(256, Ordering::Relaxed) as usize;
                        if start >= list.len() || self.stopped() {
                            break;
                        }
                        for (i, c) in list.iter().enumerate().skip(start).take(256) {
                            if self.stopped() {
                                break;
                            }
                            let bits = self.tally(c, request, &mut w, &mut counts);
                            local.push((i, bits));
                        }
                    }
                    if let Ok(mut r) = results.lock() {
                        for (i, b) in local {
                            r.0[i] = b;
                        }
                        r.1.add(&counts);
                    }
                });
            }
        });
        results.into_inner().unwrap_or_default()
    }

    /// The engine's outcome for each outfit, on every thread.
    fn outcomes(&self, list: &[Candidate], request: &Request) -> Vec<Outcome> {
        let next = AtomicU64::new(0);
        let results = Mutex::new(vec![Outcome::NoPath; list.len()]);
        std::thread::scope(|scope| {
            for _ in 0..request.threads.max(1) {
                scope.spawn(|| {
                    let mut w = self.engine.work();
                    let mut local = Vec::new();
                    loop {
                        let start = next.fetch_add(16, Ordering::Relaxed) as usize;
                        if start >= list.len() || self.stopped() {
                            break;
                        }
                        for (i, c) in list.iter().enumerate().skip(start).take(16) {
                            if self.stopped() {
                                break;
                            }
                            local.push((i, self.candidate_outcome(c, request, &mut w)));
                        }
                    }
                    if let Ok(mut r) = results.lock() {
                        for (i, o) in local {
                            r[i] = o;
                        }
                    }
                });
            }
        });
        results.into_inner().unwrap_or_default()
    }

    /// Fail unless the card gives the engine's exact outcome on outfits spread over the whole plan.
    fn verify(
        &self,
        g: &mut Gpu,
        plan: &Plan,
        attrs: [i32; 7],
        request: &Request,
    ) -> Result<(), String> {
        let total = plan.size();
        self.publish(
            SearchStage::Verification,
            SearchBackend::Card,
            0,
            Some(total),
            0,
        );
        if self.stopped() {
            return Err("search interrupted".into());
        }
        let n = (VERIFY as u64).min(total);
        let starts = plan.starts();
        let list: Vec<Candidate> = (0..n)
            .map(|k| {
                plan.nth(
                    &starts,
                    ((k as u128 * total as u128 / n as u128) as u64 + k % 7).min(total - 1),
                    attrs,
                )
            })
            .collect();
        self.cross_check(g, &list, request)
    }

    fn cross_check(
        &self,
        g: &mut Gpu,
        list: &[Candidate],
        request: &Request,
    ) -> Result<(), String> {
        let card = g.cases_with_sets(list, &request.limits, &request.sets)?;
        let cpu = self.outcomes(list, request);
        if self.stopped() {
            return Err("search interrupted".into());
        }
        match cpu.iter().zip(&card).position(|(a, b)| !agree(a, b)) {
            None => Ok(()),
            Some(k) => Err(format!(
                "{DISAGREES} on {:?}: engine {:?}, card {:?}",
                list[k], cpu[k], card[k]
            )),
        }
    }

    /// Small plans try one near-best pass. Large plans start in bounded
    /// partitions to avoid a speculative sweep that may overflow.
    fn sweep_card(
        &self,
        g: &mut Gpu,
        plan: &Plan,
        attrs: [i32; 7],
        request: &Request,
        capacity: usize,
    ) -> Result<(Vec<Scored>, Counts, Vec<Candidate>), String> {
        self.legacy_card_admission(&attrs)?;
        let total = plan.size();
        self.publish(
            SearchStage::Evaluation,
            SearchBackend::Card,
            0,
            Some(total),
            0,
        );
        if self.stopped() {
            return Err("search interrupted".into());
        }
        let mut ranked = CardRanking::default();
        let mut settled = 0u64;
        let mut batch_index = 0u64;
        let legacy_observer = self.control.is_some() && !g.supports_observed_search();
        let batch_capacity = if legacy_observer {
            capacity.min(512)
        } else {
            capacity
        };
        if total <= CARD_DIRECT_LIMIT && !legacy_observer {
            let (spaces, lists) = Plan::card_spaces(&plan.spaces);
            for keep in [0.9f32, 0.5, 0.0] {
                batch_index += 1;
                if batch_index > 1 {
                    self.publish(
                        SearchStage::Retrying,
                        SearchBackend::Card,
                        0,
                        Some(total),
                        0,
                    );
                }
                let sw = self.card_sweep(
                    g,
                    &spaces,
                    &lists,
                    &attrs,
                    &request.limits,
                    &request.sets,
                    keep,
                    capacity.min(total as usize),
                    CARD_MISSES,
                    0,
                    total,
                    batch_index,
                )?;
                if sw.hit_total > sw.hits.len() as u64 {
                    if std::env::var_os("NIOH3_E2E_PROFILE")
                        .is_some_and(|value| !value.is_empty() && value != "0")
                    {
                        let retained_best = sw.hits.iter().map(|hit| hit.bits).max().unwrap_or(0);
                        eprintln!(
                            "loadout card rank retry: retained={} generated={} retained_best={}",
                            sw.hits.len(),
                            sw.hit_total,
                            f32::from_bits(retained_best),
                        );
                    }
                    ranked.counts.card_overflows += 1;
                    break;
                }
                if CardRanking::covers_top(&sw, keep) {
                    if self.stopped() {
                        return Err("search interrupted".into());
                    }
                    ranked.merge(&plan.spaces, attrs, sw)?;
                    self.publish(
                        SearchStage::Evaluation,
                        SearchBackend::Card,
                        total,
                        Some(total),
                        0,
                    );
                    return self.finish_card_rank(g, ranked, request);
                }
            }
        }

        plan.visit_card_batches(batch_capacity as u64, |batch| {
            if self.stopped() {
                return Err("search interrupted".into());
            }
            batch_index += 1;
            let (spaces, lists) = Plan::card_spaces(batch);
            let size = Plan::space_size(batch) as usize;
            // Every finite nonnegative score in this batch fits, including zero ties.
            let keep = if size > KEEP { 0.9 } else { 0.0 };
            let mut sw = self.card_sweep(
                g,
                &spaces,
                &lists,
                &attrs,
                &request.limits,
                &request.sets,
                keep,
                size,
                CARD_MISSES,
                settled,
                total,
                batch_index,
            )?;
            if !CardRanking::covers_top(&sw, keep) {
                self.publish(
                    SearchStage::Retrying,
                    SearchBackend::Card,
                    settled,
                    Some(total),
                    0,
                );
                batch_index += 1;
                sw = self.card_sweep(
                    g,
                    &spaces,
                    &lists,
                    &attrs,
                    &request.limits,
                    &request.sets,
                    0.0,
                    size,
                    CARD_MISSES,
                    settled,
                    total,
                    batch_index,
                )?;
            }
            if self.stopped() {
                return Err("search interrupted".into());
            }
            ranked.merge(batch, attrs, sw)?;
            settled += size as u64;
            self.publish(
                SearchStage::Evaluation,
                SearchBackend::Card,
                settled,
                Some(total),
                0,
            );
            Ok(())
        })?;
        self.finish_card_rank(g, ranked, request)
    }

    fn finish_card_rank(
        &self,
        g: &mut Gpu,
        mut ranked: CardRanking,
        request: &Request,
    ) -> Result<(Vec<Scored>, Counts, Vec<Candidate>), String> {
        if self.stopped() {
            return Err("search interrupted".into());
        }
        ranked.missed.sort_unstable();
        // Check a bounded spread of missing outfits against the same CPU entry.
        let step = ranked.missed.len().div_ceil(VERIFY).max(1);
        let sample: Vec<_> = ranked.missed.iter().step_by(step).copied().collect();
        if !sample.is_empty() {
            self.cross_check(g, &sample, request)?;
        }
        ranked.missed.truncate(MISSED_SAMPLE);
        Ok((ranked.best.sorted(), ranked.counts, ranked.missed))
    }

    fn tally(
        &self,
        c: &Candidate,
        request: &Request,
        w: &mut crate::engine::Work,
        counts: &mut Counts,
    ) -> Option<u32> {
        Self::tally_outcome(self.candidate_outcome(c, request, w), counts)
    }

    fn prepare_candidate<'w>(
        &'w self,
        c: &Candidate,
        request: &Request,
        w: &'w mut crate::engine::Work,
    ) -> Result<PreparedCandidate<'w>, Outcome> {
        let admit = |state: &SetState| {
            request.sets.iter().all(|rule| {
                rule.holds(
                    |key| {
                        Some(
                            state
                                .pieces
                                .iter()
                                .find(|entry| entry.0 == key)
                                .map_or(0, |entry| entry.1),
                        )
                    },
                    state.reduction,
                )
            })
        };
        self.engine.prepare_for_search(
            c,
            self.pack,
            &request.limits,
            w,
            (!request.sets.is_empty()).then_some(admit),
        )
    }

    /// Deliberately unfused set-only reference for host acceptance. Search
    /// scoring uses prepare_candidate so it can retain this equipment state.
    fn set_admission(
        &self,
        c: &Candidate,
        request: &Request,
        w: &mut crate::engine::Work,
    ) -> Result<(), Outcome> {
        if request.sets.is_empty() {
            return Ok(());
        }
        let state = self
            .engine
            .equipment_set_state(c, self.pack, w)
            .map_err(Outcome::Refused)?;
        if request.sets.iter().all(|rule| {
            rule.holds(
                |key| {
                    Some(
                        state
                            .pieces
                            .iter()
                            .find(|entry| entry.0 == key)
                            .map_or(0, |entry| entry.1),
                    )
                },
                state.reduction,
            )
        }) {
            Ok(())
        } else {
            Err(Outcome::OutsideLimits)
        }
    }

    fn candidate_outcome(
        &self,
        c: &Candidate,
        request: &Request,
        w: &mut crate::engine::Work,
    ) -> Outcome {
        match self.prepare_candidate(c, request, w) {
            Ok(prepared) => prepared.evaluate(),
            Err(outcome) => outcome,
        }
    }

    fn tally_outcome(outcome: Outcome, counts: &mut Counts) -> Option<u32> {
        match outcome {
            Outcome::Damage(bits) => count_damage(counts, bits),
            Outcome::Refused(_) => {
                counts.refused += 1;
                None
            }
            Outcome::NoPath => {
                counts.no_path += 1;
                None
            }
            Outcome::OutsideLimits => {
                counts.outside_limits += 1;
                None
            }
        }
    }

    fn sweep(
        &self,
        plan: &Plan,
        attrs: [i32; 7],
        request: &Request,
    ) -> (Vec<Scored>, Counts, Vec<Candidate>) {
        let starts: Vec<u64> = plan
            .spaces
            .iter()
            .scan(0u64, |at, s| {
                let here = *at;
                *at = at.saturating_add(s.size);
                Some(here)
            })
            .collect();
        let total = plan.size();
        self.publish(
            SearchStage::Evaluation,
            SearchBackend::Cpu,
            0,
            Some(total),
            0,
        );
        let processed = AtomicU64::new(0);
        let next = AtomicU64::new(0);
        let merged = Mutex::new((Best::default(), Counts::default(), Vec::new()));
        let certify = plan.complete && self.engine.supports_read_bounds();
        // The captured runner is serialized. Keep one certificate cache for
        // the complete sweep and leave the other CPU cores free.
        let threads = if certify { 1 } else { request.threads.max(1) };
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    let mut w = self.engine.work();
                    let mut bounds = certify.then(ReadBounds::new);
                    let (mut best, mut counts, mut missed) =
                        (Best::default(), Counts::default(), Vec::new());
                    loop {
                        let start = next.fetch_add(CHUNK, Ordering::Relaxed);
                        if start >= total || self.stopped() {
                            break;
                        }
                        for i in start..(start + CHUNK).min(total) {
                            if self.stopped() {
                                break;
                            }
                            let k = starts.partition_point(|&s| s <= i) - 1;
                            let c = plan.spaces[k].candidate(i - starts[k], attrs);
                            let before = counts.no_path;
                            let evaluated = match self.prepare_candidate(&c, request, &mut w) {
                                Err(outcome) => SweepOutcome::Executed(outcome),
                                Ok(prepared) => match bounds.as_mut() {
                                    Some(bounds) => prepared.evaluate_with_bound(bounds),
                                    None => SweepOutcome::Executed(prepared.evaluate()),
                                },
                            };
                            let outcome = match evaluated {
                                SweepOutcome::Executed(outcome) => outcome,
                                SweepOutcome::Certified(bits) => {
                                    counts.certified += 1;
                                    if best.strictly_below_cutoff(bits) {
                                        counts.answered += 1;
                                        counts.bound_pruned += 1;
                                        processed.fetch_add(1, Ordering::Relaxed);
                                        continue;
                                    }
                                    Outcome::Damage(bits)
                                }
                            };
                            if let Some(bits) = Self::tally_outcome(outcome, &mut counts) {
                                best.offer(Scored { bits, candidate: c });
                            } else if counts.no_path > before && missed.len() < MISSED_SAMPLE {
                                missed.push(c);
                            }
                            processed.fetch_add(1, Ordering::Relaxed);
                        }
                        self.publish(
                            SearchStage::Evaluation,
                            SearchBackend::Cpu,
                            processed.load(Ordering::Relaxed),
                            Some(total),
                            0,
                        );
                    }
                    if let Ok(mut m) = merged.lock() {
                        for s in best.sorted() {
                            m.0.offer(s);
                        }
                        m.1.add(&counts);
                        m.2.extend(missed);
                    }
                });
            }
        });
        let (best, counts, mut missed) = merged.into_inner().unwrap_or_default();
        missed.sort_unstable();
        missed.truncate(MISSED_SAMPLE);
        (best.sorted(), counts, missed)
    }

    /// Give every dropped item back to the best outfits, one position at a time, until nothing improves.
    fn polish(
        &self,
        plan: &Plan,
        top: &mut Vec<Scored>,
        request: &Request,
        counts: &mut Counts,
        mut card: Option<&mut Gpu>,
    ) -> usize {
        for round in 0..POLISH_ROUNDS {
            if self.stopped() {
                return round;
            }
            let mut tries = Vec::new();
            if request.move_points {
                for row in top.iter().take(SHOWN) {
                    for from in 0..7 {
                        for to in 0..7 {
                            for step in [1, 3, 10] {
                                if from == to || row.candidate.attrs[from] - step < 5 {
                                    continue;
                                }
                                let mut c = row.candidate;
                                c.attrs[from] -= step;
                                c.attrs[to] += step;
                                tries.push(c);
                            }
                        }
                    }
                }
            }
            for row in top.iter() {
                for (position, group) in &plan.groups {
                    let Some(held) = row.candidate.items[*position] else {
                        continue;
                    };
                    if !group.kept.contains(&held) {
                        continue;
                    }
                    for &r in &group.all {
                        let mut c = row.candidate;
                        c.items[*position] = Some(r);
                        tries.push(c);
                    }
                }
            }
            let held: HashSet<Candidate> = top.iter().map(|s| s.candidate).collect();
            tries.sort_unstable();
            tries.dedup();
            tries.retain(|c| !held.contains(c) && !wears_twice(c));
            if tries.is_empty() {
                return round;
            }
            let before = top.first().map_or(0, |s| s.bits);
            self.publish(
                SearchStage::Polishing,
                if card.is_some() {
                    SearchBackend::Card
                } else {
                    SearchBackend::Cpu
                },
                0,
                Some(tries.len() as u64),
                round as u32 + 1,
            );
            let (bits, more) =
                self.evaluate_all(&tries, request, card.as_deref_mut(), round as u32 + 1);
            if self.stopped() {
                return round;
            }
            counts.add(&more);
            let mut best = Best::default();
            for s in top.drain(..) {
                best.offer(s);
            }
            for (c, b) in tries.iter().zip(bits) {
                if let Some(bits) = b {
                    best.offer(Scored {
                        bits,
                        candidate: *c,
                    });
                }
            }
            let mut merged = best.sorted();
            merged.dedup_by_key(|s| s.candidate);
            *top = merged;
            if top.first().map_or(0, |s| s.bits) <= before {
                return round + 1;
            }
        }
        POLISH_ROUNDS
    }

    pub fn run(&self, request: &Request) -> Report {
        let t = Instant::now();
        self.run_plan(request, self.plan(request), t)
    }

    /// Check the outfit limit and execute the same plan if it fits.
    pub fn run_limited(&self, request: &Request, limit: u64) -> Option<(u64, Report)> {
        self.run_limited_by(request, |_| limit)
    }

    pub fn run_limited_by(
        &self,
        request: &Request,
        limit_for: impl FnOnce(&Plan) -> u64,
    ) -> Option<(u64, Report)> {
        let t = Instant::now();
        let plan = self.plan(request);
        let size = plan.size();
        if self.stopped() {
            return None;
        }
        if plan.full_size > MAX_EXACT_COVERAGE {
            // A tiny admitted subset must not let an unrepresentable requested
            // domain bypass the ordinary execution-space budget.
            return Some((size, self.run_plan(request, plan, t)));
        }
        (size <= limit_for(&plan)).then(|| (size, self.run_plan(request, plan, t)))
    }

    /// Benchmark a complete fixed-attribute plan through the production card
    /// ranking path. A card error ends this run instead of starting a CPU sweep.
    #[doc(hidden)]
    pub fn run_complete_on_card(
        &self,
        request: &Request,
        limit: u64,
        capacity: usize,
    ) -> Result<Report, String> {
        if request.move_points {
            return Err("a complete card sweep uses fixed attributes".into());
        }
        if !request.sets.is_empty() && !self.engine.supports_set_constraints() {
            return Err(
                "native set constraints require CPU validation; card ABI has no set predicate"
                    .into(),
            );
        }
        let t = Instant::now();
        let plan = self.build_plan(request, Some(limit));
        if plan.full_size > MAX_EXACT_COVERAGE {
            return Err(SearchFailure::DomainCountTooLarge.to_string());
        }
        if !plan.complete {
            return Err(format!(
                "the complete plan exceeds the requested {limit} outfit budget"
            ));
        }
        if plan.source_uncomputed != 0 {
            return Err(format!(
                "{} requested outfits have uncomputed weapon sources",
                plan.source_uncomputed
            ));
        }
        let attrs = self.engine.start_attrs;
        self.legacy_card_admission(&attrs)?;
        let mut kernel = self.engine.kernel();
        let g = match &mut *kernel {
            Kernel::Ready(g) => g,
            Kernel::Unavailable(reason) => return Err(reason.clone()),
            Kernel::Untried => return Err("card kernel unavailable".into()),
        };
        g.use_pool(&self.pack.pool)?;
        self.verify(g, &plan, attrs, request)?;
        let (top, counts, missed) =
            self.sweep_card(g, &plan, attrs, request, capacity.clamp(1, CARD_HITS))?;
        Ok(Report {
            failure: None,
            best: self.ranked_facts(top, plan.complete),
            backend: Backend::Card,
            counts,
            searched: plan.size(),
            full_size: plan.full_size,
            source_uncomputed: plan.source_uncomputed,
            polish_rounds: 0,
            seconds: t.elapsed().as_secs_f64(),
            missed,
            complete: plan.is_complete() && counts.refused == 0 && counts.no_path == 0,
        })
    }

    fn prepared_device_outcomes(
        &self,
        device: &mut MainDevice,
        candidates: &[Candidate],
        request: &Request,
    ) -> Result<Vec<Outcome>, String> {
        let mut work = self.engine.work();
        self.prepared_device_outcomes_with_work(device, candidates, request, &mut work)
    }

    fn prepared_device_outcomes_with_work(
        &self,
        device: &mut MainDevice,
        candidates: &[Candidate],
        request: &Request,
        work: &mut crate::engine::Work,
    ) -> Result<Vec<Outcome>, String> {
        self.prepared_device_outcomes_with_work_controlled(
            device,
            candidates,
            request,
            work,
            &|| false,
        )
    }

    fn prepared_device_outcomes_with_work_controlled(
        &self,
        device: &mut MainDevice,
        candidates: &[Candidate],
        request: &Request,
        work: &mut crate::engine::Work,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Outcome>, String> {
        let check_stopped = || {
            if cancelled() {
                Err(SearchInterrupted::Cancelled.to_string())
            } else if self.stopped() {
                Err("search interrupted".to_owned())
            } else {
                Ok(())
            }
        };
        let mut outcomes = vec![Outcome::Damage(0); candidates.len()];
        let chunk = (4096 / self.engine.hit_count().max(1)).max(1);
        for (group, group_outcomes) in candidates.chunks(chunk).zip(outcomes.chunks_mut(chunk)) {
            let mut contacts = Vec::new();
            let mut lanes = Vec::new();
            for (index, candidate) in group.iter().enumerate() {
                check_stopped()?;
                match self
                    .prepare_candidate(candidate, request, work)
                    .and_then(PreparedCandidate::device_contacts)
                {
                    Ok(words) => {
                        lanes.extend(std::iter::repeat_n(index, words.len()));
                        contacts.extend(words);
                    }
                    Err(outcome) => group_outcomes[index] = outcome,
                }
            }
            check_stopped()?;
            let scores = device.run(&contacts)?;
            check_stopped()?;
            if scores.len() != lanes.len() {
                return Err("CUDA returned an incomplete contact batch".into());
            }
            for (index, score) in lanes.into_iter().zip(scores) {
                if let Outcome::Damage(prior) = group_outcomes[index] {
                    let total = f32::from_bits(prior) + f32::from_bits(score.damage.total);
                    group_outcomes[index] = if score.bail != 0 {
                        Outcome::NoPath
                    } else if total.is_finite() {
                        Outcome::Damage(total.to_bits())
                    } else {
                        Outcome::Refused("full MAIN produced non-finite damage")
                    };
                }
            }
        }
        Ok(outcomes)
    }

    fn sweep_prepared_device(
        &self,
        plan: &Plan,
        request: &Request,
    ) -> Result<(Vec<Scored>, Counts, Vec<Candidate>), String> {
        let mut device = self.engine.open_prepared_device()?;
        let starts = plan.starts();
        let total = plan.size();
        let make_candidate = |index| plan.nth(&starts, index, self.engine.start_attrs);
        let sample_size = PREPARED_DEVICE_VERIFY.min(total);
        self.publish(
            SearchStage::Verification,
            SearchBackend::Card,
            0,
            Some(sample_size),
            0,
        );
        let sample: Vec<_> = (0..sample_size)
            .map(|index| make_candidate(index * total / sample_size))
            .collect();
        let device_sample = self.prepared_device_outcomes(&mut device, &sample, request)?;
        let mut work = self.engine.work();
        for (candidate, device_outcome) in sample.iter().zip(device_sample) {
            if self.stopped() {
                return Err("search interrupted".into());
            }
            if self.candidate_outcome(candidate, request, &mut work) != device_outcome {
                return Err(format!("{DISAGREES} on prepared candidate {candidate:?}"));
            }
        }

        let mut best = Best::default();
        let mut counts = Counts::default();
        let mut missed = Vec::new();
        let mut completed = 0;
        self.publish(
            SearchStage::Evaluation,
            SearchBackend::Card,
            completed,
            Some(total),
            0,
        );
        while completed < total {
            let end = (completed + PREPARED_DEVICE_BATCH).min(total);
            let candidates: Vec<_> = (completed..end).map(make_candidate).collect();
            let outcomes = self.prepared_device_outcomes(&mut device, &candidates, request)?;
            for (candidate, outcome) in candidates.into_iter().zip(outcomes) {
                if let Some(bits) = Self::tally_outcome(outcome, &mut counts) {
                    best.offer(Scored { bits, candidate });
                } else if outcome == Outcome::NoPath && missed.len() < MISSED_SAMPLE {
                    missed.push(candidate);
                }
            }
            counts.card_batches += 1;
            completed = end;
            self.publish(
                SearchStage::Evaluation,
                SearchBackend::Card,
                completed,
                Some(total),
                0,
            );
            if self.stopped() {
                return Err("search interrupted".into());
            }
        }
        Ok((best.sorted(), counts, missed))
    }

    fn run_plan(&self, request: &Request, plan: Plan, t: Instant) -> Report {
        let invalid = if plan.full_size > MAX_EXACT_COVERAGE {
            Some(SearchFailure::DomainCountTooLarge)
        } else if request.move_points
            && (self.source_admission == SourceAdmissionPolicy::CapturedWeaponOnly
                || !self.engine.has_dynamic_sources())
        {
            Some(SearchFailure::SourceAttributesUnproven)
        } else {
            None
        };
        if let Some(failure) = invalid {
            return Report {
                backend: Backend::Cpu(failure.to_string()),
                failure: Some(failure),
                best: Vec::new(),
                counts: Counts::default(),
                searched: 0,
                full_size: plan.full_size,
                source_uncomputed: plan.source_uncomputed,
                polish_rounds: 0,
                seconds: t.elapsed().as_secs_f64(),
                missed: Vec::new(),
                complete: false,
            };
        }
        let space_size = plan.size();
        let mut prepared_refusal = None;
        if plan.is_complete()
            && !request.move_points
            && space_size != 0
            && self
                .cpu_fallback_limit
                .is_none_or(|limit| space_size <= limit)
            && (space_size > CPU_CERTIFICATE_PREFERENCE_LIMIT
                || std::env::var("NIOH3_LOADOUT_GPU").is_ok_and(|mode| mode == "1"))
            && self.engine.supports_prepared_device()
            && !self.stopped()
        {
            match self.sweep_prepared_device(&plan, request) {
                Ok((top, counts, missed)) => {
                    self.publish(
                        SearchStage::Finalizing,
                        SearchBackend::Card,
                        space_size,
                        Some(space_size),
                        0,
                    );
                    if !self.stopped() {
                        return Report {
                            failure: None,
                            best: self.ranked_facts(top, plan.complete),
                            backend: Backend::Card,
                            counts,
                            searched: space_size,
                            full_size: plan.full_size,
                            source_uncomputed: plan.source_uncomputed,
                            polish_rounds: 0,
                            seconds: t.elapsed().as_secs_f64(),
                            missed,
                            complete: counts.refused == 0 && counts.no_path == 0,
                        };
                    }
                }
                Err(reason) => prepared_refusal = Some(reason),
            }
            if self.stopped() {
                return Report {
                    failure: None,
                    best: Vec::new(),
                    backend: Backend::Card,
                    counts: Counts::default(),
                    searched: 0,
                    full_size: plan.full_size,
                    source_uncomputed: plan.source_uncomputed,
                    polish_rounds: 0,
                    seconds: t.elapsed().as_secs_f64(),
                    missed: Vec::new(),
                    complete: false,
                };
            }
        }
        let mut attrs = self.engine.start_attrs;
        let mut kernel = (prepared_refusal.is_none() && !plan.prefers_bounded_cpu())
            .then(|| self.engine.kernel());
        let refusal = prepared_refusal.or_else(|| match kernel.as_deref_mut() {
            Some(Kernel::Ready(g)) => g
                .use_pool(&self.pack.pool)
                .and_then(|()| self.verify(g, &plan, attrs, request))
                .err(),
            Some(Kernel::Unavailable(reason)) => Some(reason.clone()),
            Some(Kernel::Untried) => Some("the kernel was not loaded".into()),
            None if plan.legacy_source_risk => {
                Some("legacy shared-source dependencies require guarded CPU evaluation".into())
            }
            None if plan.native_set_constraints => Some(
                "native set constraints require CPU validation; card ABI has no set predicate"
                    .into(),
            ),
            None => Some("complete CPU sweep with read certificates".into()),
        });
        if let (Some(k), Some(r)) = (
            kernel.as_deref_mut(),
            refusal.as_ref().filter(|r| r.starts_with(DISAGREES)),
        ) {
            *k = Kernel::Unavailable(r.clone());
        }
        let profile = std::env::var_os("NIOH3_E2E_PROFILE")
            .is_some_and(|value| !value.is_empty() && value != "0");
        if let Some(reason) = refusal.as_ref().filter(|_| profile) {
            eprintln!("loadout CPU backend: {reason}");
        }
        let mut backend = refusal.map_or(Backend::Card, Backend::Cpu);
        if let Backend::Cpu(reason) = &backend {
            if let Some(failed) = self.cpu_budget_failure(&plan, t, reason) {
                return failed;
            }
        }
        let mut card = match (&backend, kernel.as_deref_mut()) {
            (Backend::Card, Some(Kernel::Ready(g))) => Some(g),
            _ => None,
        };
        let (mut counts, mut searched, mut rounds) = (Counts::default(), 0u64, 0usize);
        let mut top = Vec::new();
        let mut missed = Vec::new();
        for _ in 0..if request.move_points { POINT_PASSES } else { 1 } {
            if self.stopped() {
                break;
            }
            let swept = match card.as_deref_mut() {
                Some(g) => self.sweep_card(g, &plan, attrs, request, CARD_HITS),
                None => Err(String::new()),
            };
            let (found, c, m) = match swept {
                Ok(r) => r,
                Err(reason) => {
                    // Stop is an owned terminal outcome, never a GPU failure
                    // that should restart the entire domain on the CPU.
                    if self.stopped() {
                        break;
                    }
                    if card.take().is_some() {
                        if profile {
                            eprintln!("loadout card sweep fallback: {reason}");
                        }
                        backend = Backend::Cpu(reason.clone());
                    }
                    let fallback_reason = match &backend {
                        Backend::Cpu(reason) => reason.as_str(),
                        Backend::Card => reason.as_str(),
                    };
                    if let Some(failed) = self.cpu_budget_failure(&plan, t, fallback_reason) {
                        return failed;
                    }
                    self.sweep(&plan, attrs, request)
                }
            };
            if self.stopped() {
                break;
            }
            counts.add(&c);
            searched = searched.saturating_add(space_size);
            missed = m;
            top = found;
            if !plan.complete {
                rounds += self.polish(&plan, &mut top, request, &mut counts, card.as_deref_mut());
            }
            // Better points can make other gear legal or better, so search again around them until they settle.
            match top.first() {
                Some(s) if s.candidate.attrs != attrs => attrs = s.candidate.attrs,
                _ => break,
            }
        }

        let best = if self.stopped() {
            Vec::new()
        } else {
            self.publish(
                SearchStage::Finalizing,
                match backend {
                    Backend::Card => SearchBackend::Card,
                    Backend::Cpu(_) => SearchBackend::Cpu,
                },
                searched,
                Some(searched),
                0,
            );
            if self.stopped() {
                Vec::new()
            } else {
                self.ranked_facts(top, plan.complete)
            }
        };
        Report {
            failure: None,
            best,
            backend,
            counts,
            searched,
            full_size: plan.full_size,
            source_uncomputed: plan.source_uncomputed,
            polish_rounds: rounds,
            seconds: t.elapsed().as_secs_f64(),
            missed,
            complete: plan.is_complete() && counts.refused == 0 && counts.no_path == 0,
        }
    }

    fn ranked_facts(&self, mut top: Vec<Scored>, complete: bool) -> Vec<(Scored, Option<Facts>)> {
        if complete {
            // Both accessory orientations were scored. Show the higher
            // scoring orientation once per otherwise identical outfit.
            let mut seen = HashSet::new();
            let [one, two] = ACCESSORY_SLOTS.map(|s| position_of(s).unwrap_or(0));
            top.retain(|scored| {
                let mut outfit = scored.candidate;
                // None refers to the captured item in THAT slot. Swapping
                // None with a pool row changes which captured item is retained,
                // so those two assignments are different outfits.
                if outfit.items[one].is_some()
                    && outfit.items[two].is_some()
                    && outfit.items[one] > outfit.items[two]
                {
                    outfit.items.swap(one, two);
                }
                seen.insert(outfit)
            });
        }
        top.truncate(SHOWN);
        let mut w = self.engine.work();
        top.into_iter()
            .map(|s| (s, self.engine.facts(&s.candidate, self.pack, &mut w)))
            .collect()
    }
}

#[cfg(test)]
mod score_contract_tests {
    use super::*;
    use crate::gpu::NkHit;

    #[test]
    fn zero_is_a_feasible_cpu_and_card_result_and_negative_zero_is_canonical() {
        for bits in [0, (-0.0f32).to_bits(), 1.0f32.to_bits(), 1] {
            let expected = if f32::from_bits(bits) == 0.0 { 0 } else { bits };
            let mut cpu = Counts::default();
            let mut card = Counts::default();
            assert_eq!(
                Search::tally_outcome(Outcome::Damage(bits), &mut cpu),
                Some(expected)
            );
            assert_eq!(count(&mut card, &CardOutcome::Damage(bits)), Some(expected));
            assert_eq!(cpu.answered, 1);
            assert_eq!(cpu.refused, 0);
            assert_eq!(cpu, card);
        }
        assert!(agree(
            &Outcome::Damage(0),
            &CardOutcome::Damage((-0.0f32).to_bits())
        ));
    }

    #[test]
    fn nonfinite_and_negative_damage_are_refused_instead_of_ranked() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            let mut cpu = Counts::default();
            let mut card = Counts::default();
            assert_eq!(
                Search::tally_outcome(Outcome::Damage(value.to_bits()), &mut cpu),
                None
            );
            assert_eq!(
                count(&mut card, &CardOutcome::Damage(value.to_bits())),
                None
            );
            assert_eq!(cpu.answered, 0);
            assert_eq!(cpu.refused, 1);
            assert_eq!(cpu, card);
            assert!(!agree(
                &Outcome::NoPath,
                &CardOutcome::Damage(value.to_bits())
            ));
        }
    }

    #[test]
    fn zero_score_ties_keep_the_best_candidate_identity_even_when_it_arrives_last() {
        let mut best = Best::default();
        for id in (0..(KEEP as u32 + 50)).rev() {
            let mut items = [None; SLOTS];
            items[0] = Some(id);
            if !best.strictly_below_cutoff(0) {
                best.offer(Scored {
                    bits: 0,
                    candidate: Candidate {
                        attrs: [5; 7],
                        items,
                    },
                });
            }
        }
        assert!(
            !best.strictly_below_cutoff(0),
            "equal scores can improve the candidate tie order"
        );
        let ids: Vec<_> = best
            .sorted()
            .into_iter()
            .map(|s| s.candidate.items[0].unwrap())
            .collect();
        assert_eq!(ids, (0..KEEP as u32).collect::<Vec<_>>());
    }

    #[test]
    fn all_zero_card_overflow_requires_complete_batches_and_preserves_ties() {
        let attrs = [5; 7];
        let n = KEEP as u32 * 2 + 1;
        let plan = Plan {
            spaces: vec![Space::new(vec![(0, (0..n).rev().collect())])],
            groups: vec![],
            full_size: n as u64,
            source_uncomputed: 0,
            complete: true,
            native_set_constraints: false,
            gpu_native_sets: false,
            legacy_source_risk: false,
        };
        let overflow = Sweep {
            counts: [0, n as u64, 0, 0],
            hit_total: n as u64,
            hits: vec![NkHit {
                bits: 0,
                space: 0,
                index: 0,
            }],
            missed: vec![],
        };
        assert!(CardRanking::default()
            .merge(&plan.spaces, attrs, overflow)
            .is_err());
        let mut ranked = CardRanking::default();
        let mut batch_sizes = Vec::new();
        plan.visit_card_batches(73, |batch| {
            let size = Plan::space_size(batch);
            batch_sizes.push(size);
            let mut hits = Vec::new();
            for (space, rows) in batch.iter().enumerate() {
                for index in (0..rows.size).rev() {
                    hits.push(NkHit {
                        bits: if index % 2 == 0 {
                            0
                        } else {
                            (-0.0f32).to_bits()
                        },
                        space: space as u32,
                        index,
                    });
                }
            }
            let sw = Sweep {
                counts: [0, size, 0, 0],
                hit_total: size,
                hits,
                missed: vec![],
            };
            assert!(CardRanking::covers_top(&sw, 0.0));
            ranked.merge(batch, attrs, sw)
        })
        .unwrap();
        assert!(batch_sizes.iter().all(|&n| n <= 73));
        assert_eq!(batch_sizes.iter().sum::<u64>(), n as u64);
        assert_eq!(ranked.counts.answered, n as u64);
        let scored = ranked.best.sorted();
        assert!(scored.iter().all(|s| s.bits == 0));
        let ids: Vec<_> = scored
            .into_iter()
            .map(|s| s.candidate.items[0].unwrap())
            .collect();
        assert_eq!(ids, (0..KEEP as u32).collect::<Vec<_>>());
    }

    #[test]
    fn invalid_card_rank_word_triggers_fallback() {
        let spaces = vec![Space::new(vec![(0, vec![0])])];
        for value in [f32::NAN, f32::INFINITY, -1.0] {
            let sw = Sweep {
                counts: [0, 1, 0, 0],
                hit_total: 1,
                hits: vec![NkHit {
                    bits: value.to_bits(),
                    space: 0,
                    index: 0,
                }],
                missed: vec![],
            };
            assert!(CardRanking::default().merge(&spaces, [5; 7], sw).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ITEM;
    use std::collections::{BTreeMap, BTreeSet};

    /// A record the item table cannot describe, so dominance never drops it.
    fn unknown(id: u16) -> [u8; ITEM] {
        let mut r = [0u8; ITEM];
        r[..2].copy_from_slice(&id.to_le_bytes());
        r
    }

    #[test]
    fn set_capacity_pruning_preserves_every_feasible_combination_and_its_order() {
        let choice = |pieces, reduction| Choice {
            pieces,
            reduction,
            parts: vec![],
        };
        let units = [
            Unit {
                choices: vec![choice(vec![1, 1], false), choice(vec![0, 1], false)],
            },
            Unit {
                choices: vec![choice(vec![0, 0], false), choice(vec![1, 0], false)],
            },
            Unit {
                choices: vec![
                    choice(vec![0, 2], false),
                    choice(vec![1, 0], true),
                    choice(vec![0, 0], true),
                ],
            },
        ];
        let keys = [10, 20];
        let option = |key, pieces| SetOption { key, pieces };
        for a in 1..=5 {
            for b in 1..=5 {
                for at_least in 1..=2 {
                    let rules = [SetRule {
                        options: vec![option(10, a), option(20, b)],
                        at_least,
                    }];
                    let bounds = SetBounds::new(&units, &keys, &rules);
                    let mut visited = Vec::new();
                    let _ = bounds.visit(&units, &mut [0; 3], &mut [0; 2], false, &mut |pick| {
                        visited.push(pick.to_vec());
                        ControlFlow::Continue(())
                    });
                    let mut exhaustive = Vec::new();
                    for c in 0..3 {
                        for y in 0..2 {
                            for x in 0..2 {
                                let chosen = [
                                    &units[0].choices[x],
                                    &units[1].choices[y],
                                    &units[2].choices[c],
                                ];
                                let mut met = 0;
                                for (key, threshold) in [a, b].into_iter().enumerate() {
                                    let n: usize = chosen.iter().map(|item| item.pieces[key]).sum();
                                    if n != 0 && n + usize::from(c != 0) >= threshold {
                                        met += 1;
                                    }
                                }
                                if met >= at_least {
                                    exhaustive.push(vec![x, y, c]);
                                }
                            }
                        }
                    }
                    assert_eq!(visited, exhaustive, "thresholds {a}, {b}, need {at_least}");
                }
            }
        }
        let rules = [SetRule {
            options: vec![option(10, 4)],
            at_least: 1,
        }];
        let bounds = SetBounds::new(&units, &keys, &rules);
        assert!(bounds.can_complete(3, &[0, 0], false));
        assert!(
            !bounds.can_complete(2, &[0, 2], false),
            "infeasible accessory choice"
        );
        assert!(bounds.can_complete(2, &[1, 0], true));
    }

    #[test]
    fn complete_search_checks_both_accessory_orientations_without_duplicate_items() {
        let engine = Engine::audit_fixture();
        // Two head items, three accessories the renderer put in slot 9 and two in slot 10.
        let items: Vec<(u32, [u8; ITEM])> = [
            (4, 0xFFF0),
            (4, 0xFFF1),
            (9, 0xFFE0),
            (9, 0xFFE1),
            (9, 0xFFE2),
            (10, 0xFFE3),
            (10, 0xFFE4),
        ]
        .iter()
        .map(|&(slot, id)| (slot, unknown(id)))
        .collect();
        let pack = Backpack::from_slots(&items).expect("backpack");
        let request = Request {
            sets: Vec::new(),
            limits: Limits::NONE,
            move_points: false,
            threads: 1,
        };
        let plan = Search::new(&engine, &pack).plan(&request);
        let [one, two] = ACCESSORY_SLOTS.map(|s| position_of(s).unwrap());
        let accessories: BTreeSet<u32> = pack
            .ranges
            .iter()
            .filter(|r| r.0 >= 9)
            .flat_map(|r| r.1..r.1 + r.2)
            .collect();
        // Per choice of the other positions, evaluate both slot orientations exactly once.
        type Rest = Vec<(usize, Vec<u32>)>;
        let mut pairs: BTreeMap<Rest, Vec<(u32, u32)>> = BTreeMap::new();
        for space in &plan.spaces {
            let rest: Vec<_> = space
                .choices
                .iter()
                .filter(|c| c.0 != one && c.0 != two)
                .cloned()
                .collect();
            let a = &space
                .choices
                .iter()
                .find(|c| c.0 == one)
                .expect("first accessory slot")
                .1;
            let b = &space
                .choices
                .iter()
                .find(|c| c.0 == two)
                .expect("second accessory slot")
                .1;
            for &x in a {
                for &y in b {
                    assert_ne!(x, y, "one accessory in both slots");
                    pairs.entry(rest.clone()).or_default().push((x, y));
                }
            }
        }
        let n = accessories.len();
        assert_eq!(n, 5);
        for found in pairs.values() {
            let unique: BTreeSet<_> = found.iter().collect();
            assert_eq!(unique.len(), found.len(), "a pair searched twice");
            assert_eq!(found.len(), n * (n - 1));
            for &(x, y) in found {
                assert!(unique.contains(&(y, x)), "missing reverse orientation");
            }
        }
        assert_eq!(plan.size(), 2 * 20);
        assert_eq!(plan.full_size, 2 * 20);
        assert!(wears_twice(&Candidate {
            attrs: [0; 7],
            items: {
                let mut i = [None; SLOTS];
                i[one] = Some(3);
                i[two] = Some(3);
                i
            }
        }));
    }
}

#[cfg(test)]
#[path = "search_gpu_audit_tests.rs"]
mod hardware_audit_tests;

#[cfg(test)]
#[path = "search_control_e2e.rs"]
mod control_e2e_tests;

#[cfg(test)]
#[path = "search_set_audit_tests.rs"]
mod native_set_audit_tests;

#[cfg(test)]
#[path = "search_domain_tests.rs"]
mod domain_tests;
#[cfg(test)]
#[path = "legacy_source_search_tests.rs"]
mod legacy_source_guard_tests;
