//! Exact inverse plans for complete NG3 ordinary effect sets.
//!
//! Port of `nioh3_scroll_editor/effect_path_inverse.py`. The native generator
//! assigns no permanent RNG draw to an effect: the candidate pool changes after
//! every accepted effect and a successful promotion inserts a seven-draw
//! shuffle before the lotteries. This module therefore compiles every legal
//! output order into exact high-16 LCG intervals, which the DirectCompute
//! accelerator evaluates as a preimage sweep.
//!
//! The plans are acceleration hints only. Every Seed the accelerator reports is
//! re-composed by the certified forward generator before the route may publish
//! it, exactly like the shipped Python layer, which is why the plan graph is
//! never trusted on its own.

use nioh3_domain::effect::{
    CandidatePoolRequest, EffectTableIndex, GraceMap, NativeWeightContext, WeightedEffectCandidate,
    EFFECT_FLAG_PROMOTED,
};
use nioh3_domain::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_ng3_rarity4_stage_one_effect_sequence,
    generate_rarity5_grace_effect_sequence,
};
use std::sync::Arc;

use crate::grace_map::CATEGORY_TO_TYPE;
use crate::preimage::{
    EffectPathInput, PathConstraintInput, PreimagePlanParams, PATH_CONSTRAINT_LIMIT,
};

/// `LCG_MULTIPLIER` from the shipped seed math.
pub const LCG_MULTIPLIER: u32 = 0x0001_0DCD;
/// `LCG_INCREMENT` from the shipped seed math.
pub const LCG_INCREMENT: u32 = 1;
/// The rarity whose complete composition this module compiles first.
pub const RARITY_GROWING: u8 = 3;
/// The rarity-3 fixed growing token (`rarity 3 uses the fixed 0x0001 token`).
pub const RARITY3_GROWING_TOKEN: u32 = 0x0001;
/// The most draw constraints one native path descriptor can carry.
pub const MAX_PATH_CONSTRAINTS: usize = PATH_CONSTRAINT_LIMIT;
/// `joint_solver.PIVOT_BUCKET_STRIDE`: the cursor permutation stride.
pub const PIVOT_BUCKET_STRIDE: u32 = 0x9E37;

/// Why a plan could not be compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectPathError {
    /// The request shape this build does not compile yet.
    Unsupported(String),
    /// The request violates the shipped generation contract.
    Rejected(String),
    /// A product table the compiler needs is inconsistent.
    Data(String),
}

impl std::fmt::Display for EffectPathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectPathError::Unsupported(message)
            | EffectPathError::Rejected(message)
            | EffectPathError::Data(message) => formatter.write_str(message),
        }
    }
}

/// One inclusive high-16 interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct U16Run {
    pub start: u16,
    pub end: u16,
}

impl U16Run {
    pub fn new(start: u16, end: u16) -> Result<Self, EffectPathError> {
        if start > end {
            return Err(EffectPathError::Rejected(format!(
                "invalid uint16 run {start}..{end}"
            )));
        }
        Ok(Self { start, end })
    }

    /// `U16Run.bucket_count`.
    pub fn bucket_count(self) -> u32 {
        u32::from(self.end) - u32::from(self.start) + 1
    }

    /// `_u16_in_runs` for one interval.
    pub fn contains(self, value: u16) -> bool {
        (self.start..=self.end).contains(&value)
    }
}

/// One exact lottery constraint: the draw index and the values it may take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotteryConstraint {
    pub source_slot: u8,
    pub draw_index: u32,
    pub effect_id: u32,
    pub candidate_count: u32,
    pub total_weight: u32,
    pub allowed_u16: Vec<U16Run>,
}

/// One exact output order and promotion outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledEffectPath {
    pub ordered_effect_ids: Vec<u32>,
    pub promoted_slot: Option<u8>,
    pub constraints: Vec<LotteryConstraint>,
}

/// A complete ordinary set with every ordinary slot named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullCompositionRequest {
    pub rarity: u8,
    pub primary_effect_id: u32,
    pub secondary_effect_ids: Vec<u32>,
    pub stage_special_effect_id: Option<u32>,
    pub natural_only: bool,
    pub playthrough: u8,
}

impl FullCompositionRequest {
    /// `FullCompositionRequest.__post_init__`.
    pub fn validate(&self) -> Result<(), EffectPathError> {
        if !(3..=5).contains(&self.rarity) {
            return Err(EffectPathError::Rejected(
                "complete scroll inversion supports rarity 3, 4, or 5".to_string(),
            ));
        }
        if !(3..=5).contains(&self.playthrough) {
            return Err(EffectPathError::Rejected(
                "effect inversion supports playthrough 3, 4, or 5".to_string(),
            ));
        }
        if matches!(self.rarity, 3 | 4) && self.playthrough != 3 {
            return Err(EffectPathError::Rejected(
                "rarity 3/4 generation is currently certified for NG3".to_string(),
            ));
        }
        let mut ordinary = vec![self.primary_effect_id];
        ordinary.extend(self.secondary_effect_ids.iter().copied());
        let expected = if self.rarity == 5 { 5 } else { 4 };
        if ordinary.len() != expected {
            return Err(EffectPathError::Rejected(format!(
                "a complete rarity-{} ordinary set requires {expected} distinct IDs",
                self.rarity
            )));
        }
        let mut sorted = ordinary.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() != ordinary.len() {
            return Err(EffectPathError::Rejected(format!(
                "a complete rarity-{} ordinary set requires {expected} distinct IDs",
                self.rarity
            )));
        }
        match (self.rarity, self.stage_special_effect_id) {
            (3, None) | (3, Some(RARITY3_GROWING_TOKEN)) => {}
            (3, Some(_)) => {
                return Err(EffectPathError::Rejected(
                    "rarity 3 uses the fixed 0x0001 growing token".to_string(),
                ))
            }
            (4, None) => {
                return Err(EffectPathError::Rejected(
                    "rarity 4 stage-one inversion requires its draw-1 token".to_string(),
                ))
            }
            (5, None) => {
                return Err(EffectPathError::Rejected(
                    "rarity 5 inversion requires its draw-1 Grace".to_string(),
                ))
            }
            _ => {}
        }
        Ok(())
    }

    fn special_id(&self) -> u32 {
        self.stage_special_effect_id
            .unwrap_or(RARITY3_GROWING_TOKEN)
    }
}

/// A complete ordinary layout with exactly one unspecified effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneWildcardCompositionRequest {
    pub rarity: u8,
    pub required_effect_ids: Vec<u32>,
    pub stage_special_effect_id: Option<u32>,
    pub natural_only: bool,
    pub playthrough: u8,
}

impl OneWildcardCompositionRequest {
    /// `OneWildcardCompositionRequest.__post_init__`.
    pub fn validate(&self) -> Result<(), EffectPathError> {
        if !(4..=5).contains(&self.rarity) {
            return Err(EffectPathError::Rejected(
                "one-wildcard inversion supports rarity 4 or 5".to_string(),
            ));
        }
        if !(3..=5).contains(&self.playthrough) {
            return Err(EffectPathError::Rejected(
                "effect inversion supports playthrough 3, 4, or 5".to_string(),
            ));
        }
        if self.rarity == 4 && self.playthrough != 3 {
            return Err(EffectPathError::Rejected(
                "rarity-4 generation is currently certified for NG3".to_string(),
            ));
        }
        let expected = if self.rarity == 4 { 3 } else { 4 };
        let mut sorted = self.required_effect_ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if self.required_effect_ids.len() != expected || sorted.len() != expected {
            return Err(EffectPathError::Rejected(format!(
                "rarity-{} one-wildcard inversion requires {expected} distinct ordinary IDs",
                self.rarity
            )));
        }
        if self.stage_special_effect_id.is_none() {
            return Err(EffectPathError::Rejected(
                "one-wildcard inversion requires its draw-1 special effect".to_string(),
            ));
        }
        Ok(())
    }
}

/// The request a compiled plan belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionRequest {
    Full(FullCompositionRequest),
    OneWildcard(OneWildcardCompositionRequest),
}

impl CompositionRequest {
    pub fn rarity(&self) -> u8 {
        match self {
            CompositionRequest::Full(request) => request.rarity,
            CompositionRequest::OneWildcard(request) => request.rarity,
        }
    }
}

/// A finite pivot preimage plus exact path predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledEffectPlan {
    pub request: CompositionRequest,
    pub promotion_draw_index: u32,
    pub promotion_probability_percent: u32,
    pub shuffle_draw_start: u32,
    pub slot_limit: u8,
    pub shared_constraints: Vec<(u32, Vec<U16Run>)>,
    pub paths: Vec<CompiledEffectPath>,
    pub pivot_draw_index: u32,
    pub pivot_allowed_u16: Vec<U16Run>,
    pub pivot_affine_addend: u32,
    pub pivot_inverse_multiplier: u32,
}

impl CompiledEffectPlan {
    /// `CompiledEffectPlan.pivot_state_count`.
    pub fn pivot_state_count(&self) -> u64 {
        self.pivot_allowed_u16
            .iter()
            .map(|run| u64::from(run.bucket_count()))
            .sum::<u64>()
            * 0x1_0000
    }

    /// The number of packed path descriptors the native sweep needs.
    pub fn native_path_count(&self) -> Result<usize, EffectPathError> {
        Ok(native_path_descriptors(self)?.len())
    }
}

/// `lcg_affine_for_draw`: `(m, c)` with `state_k = m * seed + c mod 2^32`.
pub fn lcg_affine_for_draw(draw_index: u32) -> (u32, u32) {
    let mut multiplier: u32 = 1;
    let mut addend: u32 = 0;
    for _ in 0..draw_index {
        multiplier = LCG_MULTIPLIER.wrapping_mul(multiplier);
        addend = LCG_MULTIPLIER
            .wrapping_mul(addend)
            .wrapping_add(LCG_INCREMENT);
    }
    (multiplier, addend)
}

/// `pow(multiplier, -1, 1 << 32)` for an odd multiplier.
pub fn mod_inverse_2_32(value: u32) -> u32 {
    // Newton iteration on the 2-adic inverse: accurate to 1 bit, then 2, 4, ...
    let mut inverse = value;
    for _ in 0..5 {
        inverse = inverse.wrapping_mul(2u32.wrapping_sub(value.wrapping_mul(inverse)));
    }
    inverse
}

/// The shipped low-32-bit mask view of one table weight.
fn weight_bits(candidate: &WeightedEffectCandidate) -> u32 {
    candidate.weight as u32
}

/// `_winner_for_u16`: the effect a lottery picks for one high-16 value.
fn winner_for_u16(candidates: &[WeightedEffectCandidate], random_u16: u16) -> Option<u32> {
    let positive: Vec<&WeightedEffectCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.weight != 0)
        .collect();
    if positive.is_empty() {
        return None;
    }
    let total = positive.iter().fold(0u32, |sum, candidate| {
        sum.wrapping_add(weight_bits(candidate))
    });
    let upper_count = total.wrapping_add(1);
    if upper_count == 0 {
        return None;
    }
    let mut ticket = crate::query_compile::game_random_int_from_u16(random_u16, upper_count);
    if ticket > total {
        ticket = total;
    }
    for candidate in positive {
        let weight = weight_bits(candidate);
        if ticket <= weight {
            return Some(u32::from(candidate.effect_id));
        }
        ticket = ticket.wrapping_sub(weight);
    }
    None
}

/// `_first_u16_with_random_int_at_least`.
///
/// Returns `0..=0x10000`: the sentinel one past the last value is meaningful,
/// so the caller compares in `u32` space rather than truncating to `u16`.
fn first_u16_with_random_int_at_least(count: u32, target: u32) -> u32 {
    if target == 0 {
        return 0;
    }
    if target >= count {
        return 0x1_0000;
    }
    let (mut low, mut high) = (0u32, 0x1_0000u32);
    while low < high {
        let middle = (low + high) / 2;
        if crate::query_compile::game_random_int_from_u16(middle as u16, count) >= target {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// `weighted_lottery_u16_runs`: invert one exact inclusive weighted lottery.
pub fn weighted_lottery_u16_runs(
    candidates: &[WeightedEffectCandidate],
    target_effect_id: u32,
) -> Result<Vec<U16Run>, EffectPathError> {
    let positive: Vec<&WeightedEffectCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.weight != 0)
        .collect();
    if positive.is_empty() {
        return Ok(Vec::new());
    }
    let total = positive.iter().fold(0u32, |sum, candidate| {
        sum.wrapping_add(weight_bits(candidate))
    });
    let upper_count = total.wrapping_add(1);
    if upper_count == 0 {
        return Err(EffectPathError::Data(
            "native total+1 wrapped to zero".to_string(),
        ));
    }
    let mut ticket_start: u32 = 0;
    let mut target_interval: Option<(u32, u32)> = None;
    for (index, candidate) in positive.iter().enumerate() {
        let weight = weight_bits(candidate);
        let ticket_end = if index == 0 {
            weight
        } else {
            ticket_start.wrapping_add(weight).wrapping_sub(1)
        };
        if u32::from(candidate.effect_id) == target_effect_id {
            target_interval = Some((ticket_start, ticket_end.min(total)));
            break;
        }
        ticket_start = ticket_end.wrapping_add(1);
    }
    let Some((minimum, maximum)) = target_interval else {
        return Ok(Vec::new());
    };
    let start = first_u16_with_random_int_at_least(upper_count, minimum);
    let stop = first_u16_with_random_int_at_least(upper_count, maximum + 1);
    if start >= stop || start >= 0x1_0000 {
        return Ok(Vec::new());
    }
    let end = stop - 1;
    if winner_for_u16(candidates, start as u16) != Some(target_effect_id)
        || winner_for_u16(candidates, end as u16) != Some(target_effect_id)
    {
        return Err(EffectPathError::Data(
            "weighted-lottery inverse boundary mismatch".to_string(),
        ));
    }
    Ok(vec![U16Run {
        start: start as u16,
        end: end as u16,
    }])
}

/// `NG3_RARITY3_PROMOTION_STATES`: every promotion outcome a rarity-3 primary
/// lottery can follow, in the shipped order (`None` is "no promotion").
pub const NG3_RARITY3_PROMOTION_STATES: [Option<u8>; 5] =
    [None, Some(0), Some(1), Some(2), Some(3)];

/// One NG3 rarity-3 primary family inside a DirectCompute cursor
/// (`effect_path_inverse.PrimaryPivotFamily`).
///
/// Rarity 3 draws its first ordinary lottery at draw 2, and a successful
/// promotion trial moves that lottery behind the seven-draw shuffle to draw 9.
/// A family therefore owns both an ordinary promotion outcome at
/// [`Self::promotion_draw_index`] and the allowed high-16 states at
/// [`Self::pivot_draw_index`]. Both parts are plain high-16 intervals, so the
/// fixed-draw collector enforces them without any per-Seed Python filter; the
/// certified forward generator still replays every reported Seed before it is
/// published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryPivotFamily {
    /// The promotion outcomes this family covers.
    pub promoted_states: Vec<Option<u8>>,
    /// The draw whose high-16 buckets this family enumerates.
    pub pivot_draw_index: u32,
    /// The allowed high-16 buckets, sorted and coalesced.
    pub pivot_allowed_u16: Vec<U16Run>,
    /// The promotion trial's draw index (the first draw).
    pub promotion_draw_index: u32,
    /// The draw-1 high-16 interval that selects this family's outcome.
    pub promotion_u16_runs: Vec<U16Run>,
}

impl PrimaryPivotFamily {
    /// `PrimaryPivotFamily.requires_promotion`.
    pub fn requires_promotion(&self) -> bool {
        !self.promoted_states.contains(&None)
    }

    /// `PrimaryPivotFamily.pivot_state_count`: the family's 65,536-trial states.
    pub fn pivot_state_count(&self) -> u64 {
        self.pivot_allowed_u16
            .iter()
            .map(|run| u64::from(run.bucket_count()))
            .sum::<u64>()
            * 0x1_0000
    }

    /// The family's high-16 buckets in ascending native order.
    pub fn values(&self) -> Vec<u16> {
        self.pivot_allowed_u16
            .iter()
            .flat_map(|run| run.start..=run.end)
            .collect()
    }
}

/// `_merge_u16_runs`: sorted, coalesced runs for one union of allowed states.
fn merge_u16_runs(runs: &[U16Run]) -> Vec<U16Run> {
    let mut sorted = runs.to_vec();
    sorted.sort_unstable();
    let mut merged: Vec<U16Run> = Vec::with_capacity(sorted.len());
    for run in sorted {
        if let Some(last) = merged.last_mut() {
            if u32::from(run.start) <= u32::from(last.end) + 1 {
                last.end = last.end.max(run.end);
                continue;
            }
        }
        merged.push(run);
    }
    merged
}

/// The draw the ordinary primary lottery moves to for one promotion outcome
/// (`_lottery_start_draw` for rarity 3).
fn rarity3_lottery_start_draw(promoted_slot: Option<u8>) -> u32 {
    if promoted_slot.is_some() {
        9
    } else {
        2
    }
}

/// `_lottery_candidate_pool`: the position-0 pool of one promotion outcome.
fn lottery_candidate_pool(
    tables: &EffectTableIndex,
    record_type: u16,
    rarity: u8,
    promoted: bool,
    remaining_category_capacities: [u16; 32],
) -> Result<Vec<WeightedEffectCandidate>, EffectPathError> {
    let request = CandidatePoolRequest {
        context: NativeWeightContext {
            record_type,
            rarity,
            playthrough: 3,
            restricted_destination_slot: false,
            extra_selector: 0,
            rarity5_type_floor: 0,
        },
        destination_category_and_flags: 0x40,
        destination_effect_flags: if promoted { EFFECT_FLAG_PROMOTED } else { 0 },
        remaining_category_capacities,
        special_effect_id: Some(RARITY3_GROWING_TOKEN),
        alternate_runtime_context: false,
    };
    tables
        .weighted_candidate_pool(&request, &[])
        .map_err(|error| EffectPathError::Data(format!("primary pivot candidate pool: {error:?}")))
}

/// `compile_ng3_rarity3_primary_pivot_families`.
///
/// Compiles the NG3 rarity-3 primary families for every promotion state. The
/// position-0 pool of [`compile_full_composition_plans`] is reused verbatim:
/// the un-promoted lottery draws at draw 2 with the ordinary destination
/// flags, and a promoted source slot 0 draws at draw 9 with the promoted
/// effect flag, so the promoted outcome owns a second allowed-state set.
/// States that share a draw merge into one sorted run set, each family also
/// carries the draw-1 promotion interval of the full composition compiler, and
/// the returned order is stable: the un-promoted family first, then the
/// promoted family.
pub fn compile_ng3_rarity3_primary_pivot_families(
    primary_effect_ids: &[u32],
    tables: &EffectTableIndex,
) -> Result<Vec<PrimaryPivotFamily>, EffectPathError> {
    let mut requested = primary_effect_ids.to_vec();
    requested.sort_unstable();
    requested.dedup();
    if requested.is_empty() {
        return Err(EffectPathError::Rejected(
            "a primary pivot requires at least one effect ID".to_string(),
        ));
    }
    let promotion_threshold = promotion_layout(tables, RARITY_GROWING)? * 100;
    // `random_int(u16, 10_000) < threshold` is monotone in the draw-1 state, so
    // the promotion outcome is one contiguous high-16 interval.
    let promotion_end = first_u16_with_random_int_at_least(10_000, promotion_threshold);
    let record_type = CATEGORY_TO_TYPE[usize::from(RARITY_GROWING)];
    let capacities = tables
        .category_capacities(record_type, RARITY_GROWING)
        .map_err(|error| EffectPathError::Data(format!("category capacities: {error:?}")))?;
    // The un-promoted draw first, then the promoted draw, exactly like the
    // shipped family order.
    let unpromoted_draw = rarity3_lottery_start_draw(None);
    let promoted_draw = rarity3_lottery_start_draw(Some(0));
    let mut draw_states: Vec<(u32, Vec<Option<u8>>, Vec<U16Run>)> = vec![
        (unpromoted_draw, Vec::new(), Vec::new()),
        (promoted_draw, Vec::new(), Vec::new()),
    ];
    for promoted_slot in NG3_RARITY3_PROMOTION_STATES {
        let pool = lottery_candidate_pool(
            tables,
            record_type,
            RARITY_GROWING,
            promoted_slot == Some(0),
            capacities,
        )?;
        let mut runs: Vec<U16Run> = Vec::new();
        for effect_id in &requested {
            runs.extend(weighted_lottery_u16_runs(&pool, *effect_id)?);
        }
        if runs.is_empty() {
            continue;
        }
        let draw_index = rarity3_lottery_start_draw(promoted_slot);
        let entry = draw_states
            .iter_mut()
            .find(|(candidate, _, _)| *candidate == draw_index)
            .ok_or_else(|| {
                EffectPathError::Data(format!("no compiled pivot family for draw {draw_index}"))
            })?;
        entry.1.push(promoted_slot);
        entry.2.extend(runs);
    }
    let mut families: Vec<PrimaryPivotFamily> = Vec::with_capacity(draw_states.len());
    for (draw_index, states, runs) in draw_states {
        if states.is_empty() {
            continue;
        }
        let promotion_u16_runs = if states.contains(&None) {
            // The un-promoted family needs the draw-1 promotion trial to fail.
            if promotion_end < 0x1_0000 {
                vec![U16Run {
                    start: promotion_end as u16,
                    end: 0xFFFF,
                }]
            } else {
                Vec::new()
            }
        } else if promotion_end > 0 {
            // Every remaining state is a successful promotion outcome.
            vec![U16Run {
                start: 0,
                end: (promotion_end - 1) as u16,
            }]
        } else {
            Vec::new()
        };
        if promotion_u16_runs.is_empty() {
            continue;
        }
        families.push(PrimaryPivotFamily {
            promoted_states: states,
            pivot_draw_index: draw_index,
            pivot_allowed_u16: merge_u16_runs(&runs),
            promotion_draw_index: 1,
            promotion_u16_runs,
        });
    }
    Ok(families)
}

/// The native sweep of one pivot family: its value table, descriptors and
/// scalar parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryPivotNativePlan {
    /// The family's high-16 buckets in ascending order (the pivot table).
    pub values: Vec<u16>,
    /// The packed path descriptors the fixed-draw collector evaluates.
    pub descriptors: Vec<EffectPathInput>,
    /// The scalar parameters of the same native call.
    pub params: PreimagePlanParams,
}

/// Build the native sweep of one compiled family.
///
/// Mirrors the synthetic plan
/// `effect_preimage_accelerator.collect_fixed_draw_pivot_seeds_d3d11` builds
/// for the same family: one path whose only constraints are the family's
/// promotion interval, no promotion probability of its own, the family's
/// bucket list as the pivot table, and the shipped rarity-3 slot layout.
pub fn primary_pivot_native_plan(
    family: &PrimaryPivotFamily,
) -> Result<PrimaryPivotNativePlan, EffectPathError> {
    let constraints: Vec<LotteryConstraint> = family
        .promotion_u16_runs
        .iter()
        .map(|run| LotteryConstraint {
            source_slot: 0,
            draw_index: family.promotion_draw_index,
            effect_id: 0,
            candidate_count: 0,
            total_weight: 0,
            allowed_u16: vec![*run],
        })
        .collect();
    let request = FullCompositionRequest {
        rarity: RARITY_GROWING,
        primary_effect_id: 0,
        secondary_effect_ids: vec![1, 2, 3],
        stage_special_effect_id: None,
        natural_only: true,
        playthrough: 3,
    };
    let pivot_values: Vec<U16Run> = family
        .pivot_allowed_u16
        .iter()
        .flat_map(|run| {
            (run.start..=run.end).map(|value| U16Run {
                start: value,
                end: value,
            })
        })
        .collect();
    let plan = build_plan(
        CompositionRequest::Full(request),
        vec![CompiledEffectPath {
            ordered_effect_ids: Vec::new(),
            promoted_slot: None,
            constraints,
        }],
        Vec::new(),
        family.promotion_draw_index,
        0,
        1,
        4,
        family.pivot_draw_index,
        pivot_values,
    );
    Ok(PrimaryPivotNativePlan {
        values: family.values(),
        descriptors: native_path_descriptors(&plan)?,
        params: PreimagePlanParams {
            pivot_draw_index: plan.pivot_draw_index,
            pivot_affine_addend: plan.pivot_affine_addend,
            pivot_inverse_multiplier: plan.pivot_inverse_multiplier,
            promotion_draw_index: plan.promotion_draw_index,
            promotion_probability_percent: plan.promotion_probability_percent,
            shuffle_draw_start: plan.shuffle_draw_start,
            rarity: RARITY_GROWING,
            slot_limit: plan.slot_limit,
            maximum_draw: family.promotion_draw_index,
        },
    })
}

/// Whether every requested secondary can be drawn into a normal ordinary slot.
///
/// The shipped layer refuses a set whose secondary can only occupy the single
/// deep slot (`validate_effect_request_feasibility`), because that slot becomes
/// the primary. The position-1 pool is the largest a normal slot can ever have:
/// every category capacity is still available and only the primary is already
/// accepted, so conflicts can only grow and capacities can only shrink later. A
/// secondary missing from it is therefore deep-slot-only for every position.
pub fn secondaries_fit_normal_slots(
    request: &FullCompositionRequest,
    tables: &EffectTableIndex,
    special_id: u32,
) -> Result<bool, EffectPathError> {
    let record_type = CATEGORY_TO_TYPE
        .get(usize::from(request.playthrough))
        .copied()
        .ok_or_else(|| {
            EffectPathError::Rejected(format!(
                "playthrough {} has no scroll record type",
                request.playthrough
            ))
        })?;
    let mut capacities = tables
        .category_capacities(record_type, request.rarity)
        .map_err(|error| EffectPathError::Data(format!("category capacities: {error:?}")))?;
    let accepted = vec![request.primary_effect_id];
    let source_slot = *source_slots(request.rarity)?
        .get(1)
        .ok_or_else(|| EffectPathError::Rejected("no normal slot for this rarity".to_string()))?;
    let pool = slot_pool(
        tables,
        request.rarity,
        request.playthrough,
        // Position 1 is a normal slot: no primary category flag, no promoted flag.
        1,
        source_slot,
        None,
        &mut capacities,
        &accepted,
        special_id,
    )?;
    Ok(request.secondary_effect_ids.iter().all(|effect_id| {
        pool.iter()
            .any(|candidate| u32::from(candidate.effect_id) == *effect_id)
    }))
}

/// `joint_solver.permuted_pivot_values`: the stable bucket permutation the
/// mathematical cursor walks.
///
/// The stride is `0x9E37` reduced into the bucket count, forced odd and then
/// advanced until it is coprime with that count, so a pivot over a subset of
/// buckets (the Grace draw-1 runs, for example) keeps a one-to-one cursor.
pub fn permuted_pivot_values(runs: &[U16Run]) -> Vec<u16> {
    let mut values: Vec<u16> = Vec::new();
    for run in runs {
        for value in run.start..=run.end {
            values.push(value);
            if value == u16::MAX {
                break;
            }
        }
    }
    let count = values.len();
    if count == 0 {
        return values;
    }
    let mut stride = PIVOT_BUCKET_STRIDE % count as u32;
    if stride == 0 {
        stride = 1;
    }
    if stride.is_multiple_of(2) {
        stride += 1;
    }
    while gcd_u32(stride, count as u32) != 1 {
        stride += 2;
    }
    (0..count)
        .map(|index| values[(index as u32 * stride % count as u32) as usize])
        .collect()
}

/// Greatest common divisor used by [`permuted_pivot_values`].
fn gcd_u32(left: u32, right: u32) -> u32 {
    let (mut a, mut b) = (left, right);
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

/// The draw-1 pivot table of one Grace token: its runs, permuted.
pub fn grace_pivot_values(grace_id: u32, map: &GraceMap) -> Result<Vec<u16>, EffectPathError> {
    let runs = first_u16_ranges_for_grace(grace_id, map)?;
    Ok(permuted_pivot_values(&runs))
}

/// `grace_map.first_u16_ranges_for_grace`: the draw-1 preimage of one token.
pub fn first_u16_ranges_for_grace(
    grace_id: u32,
    map: &GraceMap,
) -> Result<Vec<U16Run>, EffectPathError> {
    let mut runs: Vec<U16Run> = Vec::new();
    for range in &map.ranges {
        if range.effect_id == grace_id {
            runs.push(U16Run {
                start: range.start,
                end: range.end,
            });
        }
    }
    if runs.is_empty() {
        return Err(EffectPathError::Rejected(format!(
            "grace ID 0x{grace_id:X} is not present in the grace output map"
        )));
    }
    Ok(runs)
}

/// The pool one lottery slot draws from.
#[allow(clippy::too_many_arguments)]
fn slot_pool(
    tables: &EffectTableIndex,
    rarity: u8,
    playthrough: u8,
    position: usize,
    source_slot: u8,
    promoted_slot: Option<u8>,
    capacities: &mut [u16; 32],
    accepted: &[u32],
    special_id: u32,
) -> Result<Vec<WeightedEffectCandidate>, EffectPathError> {
    let record_type = CATEGORY_TO_TYPE
        .get(usize::from(playthrough))
        .copied()
        .ok_or_else(|| {
            EffectPathError::Rejected(format!(
                "playthrough {playthrough} has no scroll record type"
            ))
        })?;
    let request = CandidatePoolRequest {
        context: NativeWeightContext {
            record_type,
            rarity,
            playthrough,
            restricted_destination_slot: false,
            extra_selector: 0,
            rarity5_type_floor: 0,
        },
        destination_category_and_flags: if position == 0 { 0x40 } else { 0 },
        destination_effect_flags: if Some(source_slot) == promoted_slot {
            EFFECT_FLAG_PROMOTED
        } else {
            0
        },
        remaining_category_capacities: *capacities,
        special_effect_id: Some(special_id),
        alternate_runtime_context: false,
    };
    tables
        .weighted_candidate_pool(&request, accepted)
        .map_err(|error| EffectPathError::Data(format!("weighted candidate pool: {error:?}")))
}

/// The rarities' source slots and lottery draw base (`_compile_paths_...`).
fn lottery_layout(rarity: u8, promoted: bool) -> Result<[(u8, u32); 5], EffectPathError> {
    let start = if promoted { 9 } else { 2 };
    let promoted_start = if promoted { 10 } else { 3 };
    match rarity {
        3 => Ok([
            (0, start),
            (1, start + 3),
            (2, start + 6),
            (3, start + 9),
            (0, 0),
        ]),
        4 => Ok([
            (1, promoted_start),
            (2, promoted_start + 3),
            (3, promoted_start + 6),
            (4, promoted_start + 9),
            (0, 0),
        ]),
        5 => Ok([
            (1, promoted_start),
            (2, promoted_start + 3),
            (3, promoted_start + 6),
            (4, promoted_start + 9),
            (5, promoted_start + 12),
        ]),
        other => Err(EffectPathError::Rejected(format!(
            "no lottery layout for rarity {other}"
        ))),
    }
}

fn source_slots(rarity: u8) -> Result<&'static [u8], EffectPathError> {
    match rarity {
        3 => Ok(&[0, 1, 2, 3]),
        4 => Ok(&[1, 2, 3, 4]),
        5 => Ok(&[1, 2, 3, 4, 5]),
        other => Err(EffectPathError::Rejected(format!(
            "no source slots for rarity {other}"
        ))),
    }
}

/// `_compile_paths_for_promotion_slot`.
fn compile_paths_for_promotion_slot(
    request: &FullCompositionRequest,
    promoted_slot: Option<u8>,
    tables: &EffectTableIndex,
) -> Result<Vec<CompiledEffectPath>, EffectPathError> {
    let rarity = request.rarity;
    let special_id = request.special_id();
    let slots = source_slots(rarity)?;
    let layout = lottery_layout(rarity, promoted_slot.is_some())?;
    let record_type = CATEGORY_TO_TYPE
        .get(usize::from(request.playthrough))
        .copied()
        .ok_or_else(|| {
            EffectPathError::Rejected(format!(
                "playthrough {} has no scroll record type",
                request.playthrough
            ))
        })?;
    let mut built: Vec<CompiledEffectPath> = Vec::new();
    let mut secondary_order = request.secondary_effect_ids.clone();
    secondary_order.sort_unstable();
    loop {
        let mut ordered = vec![request.primary_effect_id];
        ordered.extend(secondary_order.iter().copied());
        let mut capacities = tables
            .category_capacities(record_type, rarity)
            .map_err(|error| EffectPathError::Data(format!("category capacities: {error:?}")))?;
        let mut accepted: Vec<u32> = Vec::new();
        let mut constraints: Vec<LotteryConstraint> = Vec::new();
        for (position, source_slot) in slots.iter().enumerate() {
            let effect_id = ordered[position];
            let draw_index = layout[position].1;
            let pool = slot_pool(
                tables,
                rarity,
                request.playthrough,
                position,
                *source_slot,
                promoted_slot,
                &mut capacities,
                &accepted,
                special_id,
            )?;
            let runs = weighted_lottery_u16_runs(&pool, effect_id)?;
            if runs.is_empty() {
                break;
            }
            let total_weight = pool.iter().fold(0u32, |sum, candidate| {
                sum.wrapping_add(weight_bits(candidate))
            });
            constraints.push(LotteryConstraint {
                source_slot: *source_slot,
                draw_index,
                effect_id,
                candidate_count: pool.len() as u32,
                total_weight,
                allowed_u16: runs,
            });
            accepted.push(effect_id);
            let group = tables.group_for_effect_u32(effect_id).map_err(|error| {
                EffectPathError::Data(format!("group for effect {effect_id}: {error:?}"))
            })?;
            let category = usize::from(group.category_key);
            if category >= capacities.len() || capacities[category] == 0 {
                break;
            }
            capacities[category] -= 1;
        }
        if constraints.len() == slots.len() {
            built.push(CompiledEffectPath {
                ordered_effect_ids: ordered,
                promoted_slot,
                constraints,
            });
        }
        if !next_permutation(&mut secondary_order) {
            break;
        }
    }
    Ok(built)
}

/// Advance a sequence to its next lexicographic permutation.
fn next_permutation(values: &mut [u32]) -> bool {
    if values.len() < 2 {
        return false;
    }
    let mut pivot = values.len() - 2;
    loop {
        if values[pivot] < values[pivot + 1] {
            break;
        }
        if pivot == 0 {
            return false;
        }
        pivot -= 1;
    }
    let mut successor = values.len() - 1;
    while values[successor] <= values[pivot] {
        successor -= 1;
    }
    values.swap(pivot, successor);
    values[pivot + 1..].reverse();
    true
}

#[allow(clippy::too_many_arguments)]
fn build_plan(
    request: CompositionRequest,
    paths: Vec<CompiledEffectPath>,
    shared_constraints: Vec<(u32, Vec<U16Run>)>,
    promotion_draw_index: u32,
    promotion_probability_percent: u32,
    shuffle_draw_start: u32,
    slot_limit: u8,
    pivot_draw_index: u32,
    pivot_allowed_u16: Vec<U16Run>,
) -> CompiledEffectPlan {
    let (multiplier, addend) = lcg_affine_for_draw(pivot_draw_index);
    CompiledEffectPlan {
        request,
        promotion_draw_index,
        promotion_probability_percent,
        shuffle_draw_start,
        slot_limit,
        shared_constraints,
        paths,
        pivot_draw_index,
        pivot_allowed_u16,
        pivot_affine_addend: addend,
        pivot_inverse_multiplier: mod_inverse_2_32(multiplier),
    }
}

/// The one-trial promotion layout (`rarity_generation[rarity]`).
fn promotion_layout(tables: &EffectTableIndex, rarity: u8) -> Result<u32, EffectPathError> {
    let definition = tables.rarity_generation(rarity).map_err(|error| {
        EffectPathError::Data(format!("rarity {rarity} generation row: {error:?}"))
    })?;
    if definition.promotion_trials != 1 {
        return Err(EffectPathError::Rejected(
            "path inversion requires the verified one-trial layout".to_string(),
        ));
    }
    Ok(definition.promotion_probability_percent as u32)
}

/// The primary-lottery constraint every path shares, if it is identical.
fn shared_primary_constraint(paths: &[CompiledEffectPath]) -> Option<(u32, Vec<U16Run>)> {
    let first = paths.first()?.constraints.first()?;
    let candidate = (first.draw_index, first.allowed_u16.clone());
    for path in paths {
        let constraint = path.constraints.first()?;
        if constraint.draw_index != candidate.0 || constraint.allowed_u16 != candidate.1 {
            return None;
        }
    }
    Some(candidate)
}

/// `compile_full_composition_plans`.
///
/// `special_runs` is the draw-1 preimage of the request's stage token. Rarity 3
/// uses the fixed growing token and ignores it; the Grace-mapped rarities get it
/// from a captured map, so this compiler never invents one.
pub fn compile_full_composition_plans(
    request: &FullCompositionRequest,
    tables: &EffectTableIndex,
    special_runs: &[U16Run],
) -> Result<Vec<CompiledEffectPlan>, EffectPathError> {
    request.validate()?;
    let probability = promotion_layout(tables, request.rarity)?;
    if request.rarity == 3 {
        let mut plans: Vec<CompiledEffectPlan> = Vec::new();
        for promoted_slot in [None, Some(0), Some(1), Some(2), Some(3)] {
            let paths = compile_paths_for_promotion_slot(request, promoted_slot, tables)?;
            if paths.is_empty() {
                continue;
            }
            let (pivot_draw, raw_runs) = shared_primary_constraint(&paths).ok_or_else(|| {
                EffectPathError::Data("rarity-3 primary inverse changed across orders".to_string())
            })?;
            plans.push(build_plan(
                CompositionRequest::Full(request.clone()),
                paths,
                Vec::new(),
                1,
                probability,
                2,
                4,
                pivot_draw,
                raw_runs,
            ));
        }
        if plans.is_empty() {
            return Err(EffectPathError::Rejected(
                "the complete rarity-3 composition has no legal path".to_string(),
            ));
        }
        return Ok(plans);
    }

    if special_runs.is_empty() {
        return Err(EffectPathError::Unsupported(
            "the Grace-mapped complete composition needs the captured draw-1 preimage of its \
             stage token, which this worker does not have for this context"
                .to_string(),
        ));
    }
    let promoted_slots: Vec<Option<u8>> = if request.rarity == 5 {
        vec![None, Some(1), Some(2), Some(3), Some(4), Some(5)]
    } else {
        vec![None, Some(1), Some(2), Some(3), Some(4)]
    };
    let mut paths: Vec<CompiledEffectPath> = Vec::new();
    for promoted_slot in promoted_slots {
        paths.extend(compile_paths_for_promotion_slot(
            request,
            promoted_slot,
            tables,
        )?);
    }
    if paths.is_empty() {
        return Err(EffectPathError::Rejected(format!(
            "the complete rarity-{} composition has no legal path",
            request.rarity
        )));
    }
    let mut pivot_draw = 1u32;
    let mut pivot_runs = special_runs.to_vec();
    if let Some((primary_draw, primary_runs)) = shared_primary_constraint(&paths) {
        let primary_buckets: u32 = primary_runs.iter().map(|run| run.bucket_count()).sum();
        let special_buckets: u32 = special_runs.iter().map(|run| run.bucket_count()).sum();
        if primary_buckets < special_buckets {
            pivot_draw = primary_draw;
            pivot_runs = primary_runs;
        }
    }
    Ok(vec![build_plan(
        CompositionRequest::Full(request.clone()),
        paths,
        vec![(1, special_runs.to_vec())],
        2,
        probability,
        3,
        if request.rarity == 5 { 6 } else { 5 },
        pivot_draw,
        pivot_runs,
    )])
}

/// `_compile_one_wildcard_paths_for_promotion_slot`.
fn compile_one_wildcard_paths(
    request: &OneWildcardCompositionRequest,
    promoted_slot: Option<u8>,
    tables: &EffectTableIndex,
) -> Result<Vec<CompiledEffectPath>, EffectPathError> {
    let rarity = request.rarity;
    let special_id = request.stage_special_effect_id.ok_or_else(|| {
        EffectPathError::Rejected(
            "one-wildcard inversion requires its draw-1 special effect".to_string(),
        )
    })?;
    let slots = source_slots(rarity)?;
    let layout = lottery_layout(rarity, promoted_slot.is_some())?;
    let record_type = CATEGORY_TO_TYPE
        .get(usize::from(request.playthrough))
        .copied()
        .ok_or_else(|| {
            EffectPathError::Rejected(format!(
                "playthrough {} has no scroll record type",
                request.playthrough
            ))
        })?;
    let capacities = tables
        .category_capacities(record_type, rarity)
        .map_err(|error| EffectPathError::Data(format!("category capacities: {error:?}")))?;
    let mut built: Vec<CompiledEffectPath> = Vec::new();
    let mut required = request.required_effect_ids.clone();
    required.sort_unstable();
    let context = WildcardContext {
        request,
        tables,
        rarity,
        slots,
        layout: &layout,
        promoted_slot,
        special_id,
    };
    visit_wildcard(
        &context,
        0,
        Vec::new(),
        capacities,
        required,
        false,
        Vec::new(),
        &mut built,
    )?;
    Ok(built)
}

/// The invariant inputs of one one-wildcard path search.
struct WildcardContext<'a> {
    request: &'a OneWildcardCompositionRequest,
    tables: &'a EffectTableIndex,
    rarity: u8,
    slots: &'a [u8],
    layout: &'a [(u8, u32); 5],
    promoted_slot: Option<u8>,
    special_id: u32,
}

/// `_compile_one_wildcard_paths_for_promotion_slot.visit`.
#[allow(clippy::too_many_arguments)]
fn visit_wildcard(
    context: &WildcardContext<'_>,
    position: usize,
    accepted: Vec<u32>,
    capacities: [u16; 32],
    remaining_required: Vec<u32>,
    wildcard_used: bool,
    constraints: Vec<LotteryConstraint>,
    built: &mut Vec<CompiledEffectPath>,
) -> Result<(), EffectPathError> {
    let slots = context.slots;
    if position == slots.len() {
        if remaining_required.is_empty() && wildcard_used {
            built.push(CompiledEffectPath {
                ordered_effect_ids: accepted,
                promoted_slot: context.promoted_slot,
                constraints,
            });
        }
        return Ok(());
    }
    let positions_left = slots.len() - position;
    if remaining_required.len() > positions_left {
        return Ok(());
    }
    let source_slot = slots[position];
    let pool = slot_pool(
        context.tables,
        context.rarity,
        context.request.playthrough,
        position,
        source_slot,
        context.promoted_slot,
        &mut capacities.clone(),
        &accepted,
        context.special_id,
    )?;
    let can_use_wildcard = !wildcard_used && positions_left > remaining_required.len();
    for candidate in &pool {
        let effect_id = u32::from(candidate.effect_id);
        let is_required = remaining_required.contains(&effect_id);
        if !is_required && !can_use_wildcard {
            continue;
        }
        let runs = weighted_lottery_u16_runs(&pool, effect_id)?;
        if runs.is_empty() {
            continue;
        }
        let group = context
            .tables
            .group_for_effect_u32(effect_id)
            .map_err(|error| {
                EffectPathError::Data(format!("group for effect {effect_id}: {error:?}"))
            })?;
        let category = usize::from(group.category_key);
        if category >= capacities.len() || capacities[category] == 0 {
            continue;
        }
        let mut next_capacities = capacities;
        next_capacities[category] -= 1;
        let total_weight = pool
            .iter()
            .fold(0u32, |sum, item| sum.wrapping_add(weight_bits(item)));
        let mut next_accepted = accepted.clone();
        next_accepted.push(effect_id);
        let mut next_required = remaining_required.clone();
        if is_required {
            next_required.retain(|value| *value != effect_id);
        }
        let mut next_constraints = constraints.clone();
        next_constraints.push(LotteryConstraint {
            source_slot,
            draw_index: context.layout[position].1,
            effect_id,
            candidate_count: pool.len() as u32,
            total_weight,
            allowed_u16: runs,
        });
        visit_wildcard(
            context,
            position + 1,
            next_accepted,
            next_capacities,
            next_required,
            wildcard_used || !is_required,
            next_constraints,
            built,
        )?;
    }
    Ok(())
}

/// `compile_one_wildcard_composition_plans`.
pub fn compile_one_wildcard_composition_plans(
    request: &OneWildcardCompositionRequest,
    tables: &EffectTableIndex,
    special_runs: &[U16Run],
) -> Result<Vec<CompiledEffectPlan>, EffectPathError> {
    request.validate()?;
    let probability = promotion_layout(tables, request.rarity)?;
    if special_runs.is_empty() {
        return Err(EffectPathError::Unsupported(
            "the one-wildcard composition needs the captured draw-1 preimage of its stage \
             token, which this worker does not have for this context"
                .to_string(),
        ));
    }
    let slot_limit: u8 = if request.rarity == 4 { 5 } else { 6 };
    let mut promoted_slots: Vec<Option<u8>> = vec![None];
    for slot in 1..slot_limit {
        promoted_slots.push(Some(slot));
    }
    let mut paths: Vec<CompiledEffectPath> = Vec::new();
    for promoted_slot in promoted_slots {
        paths.extend(compile_one_wildcard_paths(request, promoted_slot, tables)?);
    }
    if paths.is_empty() {
        return Err(EffectPathError::Rejected(
            "the one-wildcard composition has no legal native path".to_string(),
        ));
    }
    Ok(vec![build_plan(
        CompositionRequest::OneWildcard(request.clone()),
        paths,
        vec![(1, special_runs.to_vec())],
        2,
        probability,
        3,
        slot_limit,
        1,
        special_runs.to_vec(),
    )])
}

/// `_u16_in_runs`.
fn u16_in_runs(value: u16, runs: &[U16Run]) -> bool {
    runs.iter().any(|run| run.contains(value))
}

/// `_promoted_slot_from_draws`.
fn promoted_slot_from_draws(
    draws: &[u16],
    rarity: u8,
    slot_limit: u8,
) -> Result<u8, EffectPathError> {
    if draws.len() != 7 {
        return Err(EffectPathError::Rejected(
            "the promotion shuffle consumes exactly seven draws".to_string(),
        ));
    }
    let mut order: Vec<u8> = (0..7u8).collect();
    for (position, random_u16) in draws.iter().enumerate() {
        let swap_index = crate::query_compile::game_random_int_from_u16(*random_u16, 7) as usize;
        order.swap(position, swap_index);
    }
    for slot_index in order {
        if slot_index >= slot_limit {
            continue;
        }
        if matches!(rarity, 4 | 5) && slot_index == 0 {
            continue;
        }
        return Ok(slot_index);
    }
    Err(EffectPathError::Data(
        "promotion shuffle had no eligible slot".to_string(),
    ))
}

/// `is_natural_scroll_id`: the necessary and sufficient natural output shape.
pub fn is_natural_scroll_id(seed: u32) -> bool {
    (seed & 0xF000_0000) == 0 && (seed & 0xFFFF) != 0
}

/// The certified recomposition gate for the accelerator's own hits.
///
/// Port of `verify_complete_matches`. A DirectCompute hit is only accepted when
/// the certified forward generator, run on the ported product tables, composes
/// exactly one of the requested ordinary sets: the plan's interval predicates
/// are acceleration hints, never the acceptance decision.
#[derive(Debug)]
pub struct PreimageVerifier {
    tables: Arc<EffectTableIndex>,
    rarity: u8,
    /// Accepted `(primary, sorted secondaries)` compositions, in request order.
    accepted: Vec<(u32, Vec<u32>)>,
    /// The draw-1 token the composition must terminate with, for the
    /// Grace-mapped rarities that carry one.
    stage_special_effect_id: Option<u32>,
    grace_map: Option<GraceMap>,
    playthrough: u8,
    level: u16,
    natural_only: bool,
    /// `minimum_roll_percent_by_effect_id`: plain roll minimums, which the job
    /// layer does not re-check, so the route's own acceptance decides them from
    /// the composed sequence exactly like `_sequence_satisfies_roll_filters`.
    minimum_rolls: Vec<(u32, u32)>,
    /// One-wildcard mode: every listed effect must appear in an ordinary slot.
    wildcard_required: Option<Vec<u32>>,
}

/// Everything the certified recomposition gate needs for one route.
pub struct PreimageVerifierSpec {
    pub tables: Arc<EffectTableIndex>,
    pub rarity: u8,
    pub accepted: Vec<(u32, Vec<u32>)>,
    pub stage_special_effect_id: Option<u32>,
    pub grace_map: Option<GraceMap>,
    pub playthrough: u8,
    pub level: u16,
    pub natural_only: bool,
    pub minimum_rolls: Vec<(u32, u32)>,
    /// One-wildcard mode: accept any composition whose ordinary slots contain
    /// every listed effect, instead of matching one exact set.
    pub wildcard_required: Option<Vec<u32>>,
}

impl PartialEq for PreimageVerifier {
    /// Two verifiers are the same when they accept the same compositions on the
    /// same rarity and level. The product tables they share are loaded data, so
    /// they are compared by identity rather than by value.
    fn eq(&self, other: &Self) -> bool {
        self.rarity == other.rarity
            && self.level == other.level
            && self.natural_only == other.natural_only
            && self.accepted == other.accepted
            && self.minimum_rolls == other.minimum_rolls
            && self.wildcard_required == other.wildcard_required
            && self.stage_special_effect_id == other.stage_special_effect_id
            && self.playthrough == other.playthrough
            && Arc::ptr_eq(&self.tables, &other.tables)
    }
}

impl Eq for PreimageVerifier {}

impl PreimageVerifier {
    pub fn new(
        tables: Arc<EffectTableIndex>,
        rarity: u8,
        mut accepted: Vec<(u32, Vec<u32>)>,
        level: u16,
        natural_only: bool,
        minimum_rolls: Vec<(u32, u32)>,
    ) -> Self {
        Self::build(PreimageVerifierSpec {
            tables,
            rarity,
            accepted: std::mem::take(&mut accepted),
            stage_special_effect_id: None,
            grace_map: None,
            playthrough: 3,
            level,
            natural_only,
            minimum_rolls,
            wildcard_required: None,
        })
    }

    /// Build the certified recomposition gate for one route.
    pub fn build(spec: PreimageVerifierSpec) -> Self {
        let PreimageVerifierSpec {
            tables,
            rarity,
            mut accepted,
            stage_special_effect_id,
            grace_map,
            playthrough,
            level,
            natural_only,
            minimum_rolls,
            wildcard_required,
        } = spec;
        for (_, secondaries) in accepted.iter_mut() {
            secondaries.sort_unstable();
        }
        Self {
            tables,
            rarity,
            accepted,
            stage_special_effect_id,
            grace_map,
            playthrough,
            level,
            natural_only,
            minimum_rolls,
            wildcard_required,
        }
    }

    /// Accepted `(primary, sorted secondaries)` compositions.
    pub fn accepted_compositions(&self) -> &[(u32, Vec<u32>)] {
        &self.accepted
    }

    /// Whether every requested plain roll minimum is met by some ordinary slot.
    fn rolls_satisfied(&self, ordinary: &[&nioh3_domain::record::ScrollEffect]) -> bool {
        self.minimum_rolls.iter().all(|(effect_id, minimum)| {
            ordinary.iter().any(|effect| {
                effect.effect_id == *effect_id && u32::from(effect.roll_percent) >= *minimum
            })
        })
    }

    /// Whether the certified generator composes one of the requested sets.
    ///
    /// A composition failure is an error, not a rejection: the shipped layer
    /// propagates it, so a Seed the certified generator cannot even compose must
    /// fail the job closed instead of quietly disappearing from the result set.
    pub fn accepts(&self, seed: u32) -> Result<bool, EffectPathError> {
        if self.natural_only && !is_natural_scroll_id(seed) {
            return Ok(false);
        }
        let record = match self.rarity {
            RARITY_GROWING => generate_ng3_rarity3_effect_sequence(&self.tables, seed, self.level),
            4 => generate_ng3_rarity4_stage_one_effect_sequence(
                &self.tables,
                self.grace_map.as_ref().ok_or_else(|| {
                    EffectPathError::Unsupported(
                        "the rarity-4 recomposition gate needs its captured draw-1 Grace map"
                            .to_string(),
                    )
                })?,
                seed,
                self.level,
            ),
            5 => generate_rarity5_grace_effect_sequence(
                &self.tables,
                self.grace_map.as_ref().ok_or_else(|| {
                    EffectPathError::Unsupported(
                        "the rarity-5 recomposition gate needs its captured draw-1 Grace map"
                            .to_string(),
                    )
                })?,
                self.playthrough,
                seed,
                self.level,
            ),
            other => {
                return Err(EffectPathError::Unsupported(format!(
                    "the certified recomposition gate is implemented for rarities 3, 4 and 5, \
                     not rarity {other}"
                )))
            }
        }
        .map_err(|error| {
            EffectPathError::Data(format!(
                "certified rarity-{} composition: {error:?}",
                self.rarity
            ))
        })?;
        let Some(primary) = record.primary() else {
            return Ok(false);
        };
        let primary = primary.effect_id;
        let ordinary_slots = match (&self.wildcard_required, self.accepted.first()) {
            (Some(required), _) => required.len() + 1,
            (None, Some((_, ids))) => ids.len() + 1,
            (None, None) => 1,
        };
        let ordinary: Vec<&nioh3_domain::record::ScrollEffect> =
            record.effects.iter().take(ordinary_slots).collect();
        if !self.rolls_satisfied(&ordinary) {
            return Ok(false);
        }
        if let Some(expected_special) = self.stage_special_effect_id {
            // The draw-1 token occupies the slot after the ordinary ones: slot 5
            // for a rarity-4 stage-one record and slot 6 for rarity 5.
            let observed = record
                .effects
                .get(ordinary_slots)
                .map(|effect| effect.effect_id);
            if observed != Some(expected_special) {
                return Ok(false);
            }
        }
        let mut secondaries: Vec<u32> = record
            .effects
            .iter()
            .skip(1)
            .take(ordinary_slots.saturating_sub(1))
            .map(|effect| effect.effect_id)
            .collect();
        secondaries.sort_unstable();
        if let Some(required) = &self.wildcard_required {
            // One-wildcard mode: the required ordinary effects must all appear,
            // and the unspecified slot may be anything the generator produced.
            let mut present: Vec<u32> = std::iter::once(primary)
                .chain(secondaries.iter().copied())
                .collect();
            present.sort_unstable();
            return Ok(required.iter().all(|effect_id| present.contains(effect_id)));
        }
        Ok(self.accepted.iter().any(|(expected_primary, expected)| {
            *expected_primary == primary && *expected == secondaries
        }))
    }
}

/// `seed_satisfies_compiled_plan`.
pub fn seed_satisfies_compiled_plan(plan: &CompiledEffectPlan, seed: u32) -> bool {
    let natural_only = match &plan.request {
        CompositionRequest::Full(request) => request.natural_only,
        CompositionRequest::OneWildcard(request) => request.natural_only,
    };
    if natural_only && !is_natural_scroll_id(seed) {
        return false;
    }
    let max_draw = plan
        .paths
        .iter()
        .flat_map(|path| path.constraints.iter().map(|item| item.draw_index))
        .max()
        .unwrap_or(plan.promotion_draw_index);
    let mut outputs = vec![0u16; max_draw as usize + 1];
    let mut state = seed;
    for draw in 1..=max_draw {
        state = LCG_MULTIPLIER
            .wrapping_mul(state)
            .wrapping_add(LCG_INCREMENT);
        outputs[draw as usize] = (state >> 16) as u16;
    }
    for (draw_index, runs) in &plan.shared_constraints {
        let value = outputs.get(*draw_index as usize).copied().unwrap_or(0);
        if !u16_in_runs(value, runs) {
            return false;
        }
    }
    let promotion_value = outputs
        .get(plan.promotion_draw_index as usize)
        .copied()
        .unwrap_or(0);
    let promoted = crate::query_compile::game_random_int_from_u16(promotion_value, 10_000)
        < plan.promotion_probability_percent * 100;
    let promoted_slot = if promoted {
        let start = plan.shuffle_draw_start as usize;
        let draws: Vec<u16> = outputs.iter().skip(start).take(7).copied().collect();
        promoted_slot_from_draws(&draws, plan.request.rarity(), plan.slot_limit).ok()
    } else {
        None
    };
    plan.paths.iter().any(|path| {
        path.promoted_slot == promoted_slot
            && path.constraints.iter().all(|item| {
                u16_in_runs(
                    outputs.get(item.draw_index as usize).copied().unwrap_or(0),
                    &item.allowed_u16,
                )
            })
    })
}

/// `plan_trial_for_seed`: the zero-based pivot-family trial of one Seed.
pub fn plan_trial_for_seed(plan: &CompiledEffectPlan, seed: u32) -> Result<u64, EffectPathError> {
    let mut state = seed;
    for _ in 0..plan.pivot_draw_index {
        state = LCG_MULTIPLIER
            .wrapping_mul(state)
            .wrapping_add(LCG_INCREMENT);
    }
    let high = (state >> 16) as u16;
    let mut high_index: Option<u64> = None;
    let mut seen: u64 = 0;
    for run in &plan.pivot_allowed_u16 {
        if run.contains(high) {
            high_index = Some(seen + u64::from(high - run.start));
            break;
        }
        seen += u64::from(run.bucket_count());
    }
    let Some(high_index) = high_index else {
        return Err(EffectPathError::Rejected(
            "Seed is outside the compiled pivot preimage".to_string(),
        ));
    };
    Ok(high_index * 0x1_0000 + u64::from(state & 0xFFFF))
}

/// `_expand_paths`: the packed descriptors the native sweep evaluates.
pub fn native_path_descriptors(
    plan: &CompiledEffectPlan,
) -> Result<Vec<EffectPathInput>, EffectPathError> {
    let mut expanded: Vec<EffectPathInput> = Vec::new();
    for path in &plan.paths {
        let mut constraints: Vec<(u32, Vec<U16Run>)> = plan.shared_constraints.clone();
        for item in &path.constraints {
            constraints.push((item.draw_index, item.allowed_u16.clone()));
        }
        if constraints.len() > MAX_PATH_CONSTRAINTS {
            return Err(EffectPathError::Rejected(
                "native effect plans support at most six draw constraints".to_string(),
            ));
        }
        let mut selection = vec![0usize; constraints.len()];
        // Cartesian product with the last constraint varying fastest, the same
        // order `itertools.product` uses in the shipped compiler.
        'product: loop {
            let mut descriptor = EffectPathInput {
                promoted_slot: path.promoted_slot.map_or(-1, i32::from),
                constraint_count: constraints.len() as u32,
                ..EffectPathInput::default()
            };
            for (index, (draw_index, runs)) in constraints.iter().enumerate() {
                let run = runs[selection[index]];
                descriptor.constraints[index] = PathConstraintInput {
                    draw_index: *draw_index,
                    start_u16: u32::from(run.start),
                    end_u16: u32::from(run.end),
                    reserved: 0,
                };
            }
            expanded.push(descriptor);
            let mut index = constraints.len();
            loop {
                if index == 0 {
                    break 'product;
                }
                index -= 1;
                selection[index] += 1;
                if selection[index] < constraints[index].1.len() {
                    continue 'product;
                }
                selection[index] = 0;
            }
        }
    }
    if expanded.is_empty() {
        return Err(EffectPathError::Rejected(
            "compiled effect plan contains no native paths".to_string(),
        ));
    }
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn evidence(name: &str) -> Value {
        let path = repo_root()
            .join("deliverables")
            .join("m23d-preimage")
            .join("evidence")
            .join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("missing vector {}: {error}", path.display()));
        serde_json::from_str(&text).expect("vector is valid JSON")
    }

    fn shipped_tables() -> EffectTableIndex {
        let root = repo_root().join("nioh3_scroll_editor").join("data");
        let bytes = nioh3_data::load_effect_resource(&root).expect("effect resource loads");
        EffectTableIndex::from_resource(&bytes).expect("effect tables decode")
    }

    fn request_from(vector: &Value) -> FullCompositionRequest {
        FullCompositionRequest {
            rarity: vector["rarity"].as_u64().unwrap() as u8,
            primary_effect_id: vector["primary_effect_ids"][0].as_u64().unwrap() as u32,
            secondary_effect_ids: vector["required_secondary_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            stage_special_effect_id: None,
            natural_only: true,
            playthrough: vector["playthrough"].as_u64().unwrap() as u8,
        }
    }

    fn runs_from(value: &Value) -> Vec<U16Run> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|run| U16Run {
                start: run[0].as_u64().unwrap() as u16,
                end: run[1].as_u64().unwrap() as u16,
            })
            .collect()
    }

    /// Compare one compiled plan with the shipped reference graph, field by
    /// field, including every path constraint and interval.
    fn assert_plan_matches(plan: &CompiledEffectPlan, reference: &Value) {
        assert_eq!(
            plan.promotion_draw_index,
            reference["promotion_draw_index"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.promotion_probability_percent,
            reference["promotion_probability_percent"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.shuffle_draw_start,
            reference["shuffle_draw_start"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.slot_limit,
            reference["slot_limit"].as_u64().unwrap() as u8
        );
        assert_eq!(
            plan.pivot_draw_index,
            reference["pivot_draw_index"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.pivot_affine_addend,
            reference["pivot_affine_addend"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.pivot_inverse_multiplier,
            reference["pivot_inverse_multiplier"].as_u64().unwrap() as u32
        );
        assert_eq!(
            plan.pivot_allowed_u16,
            runs_from(&reference["pivot_allowed_u16"])
        );
        assert_eq!(
            plan.pivot_state_count(),
            reference["pivot_state_count"].as_u64().unwrap()
        );
        let expected_shared: Vec<(u32, Vec<U16Run>)> = reference["shared_constraints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (entry[0].as_u64().unwrap() as u32, runs_from(&entry[1])))
            .collect();
        assert_eq!(plan.shared_constraints, expected_shared);
        let paths = reference["paths"].as_array().unwrap();
        assert_eq!(plan.paths.len(), paths.len(), "path count");
        for (path, reference_path) in plan.paths.iter().zip(paths) {
            assert_eq!(
                path.ordered_effect_ids,
                reference_path["ordered_effect_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() as u32)
                    .collect::<Vec<u32>>()
            );
            assert_eq!(
                path.promoted_slot.map(u64::from),
                reference_path["promoted_slot"].as_u64()
            );
            let constraints = reference_path["constraints"].as_array().unwrap();
            assert_eq!(path.constraints.len(), constraints.len());
            for (constraint, reference_constraint) in path.constraints.iter().zip(constraints) {
                assert_eq!(
                    constraint.source_slot,
                    reference_constraint["source_slot"].as_u64().unwrap() as u8
                );
                assert_eq!(
                    constraint.draw_index,
                    reference_constraint["draw_index"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    constraint.effect_id,
                    reference_constraint["effect_id"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    constraint.candidate_count,
                    reference_constraint["candidate_count"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    constraint.total_weight,
                    reference_constraint["total_weight"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    constraint.allowed_u16,
                    runs_from(&reference_constraint["allowed_u16"])
                );
            }
        }
    }

    /// Compare the packed descriptors with the shipped expansion, row for row.
    fn assert_descriptors_match(plan: &CompiledEffectPlan, reference: &Value) {
        let descriptors = native_path_descriptors(plan).expect("descriptors expand");
        let rows = reference["rows"].as_array().unwrap();
        assert_eq!(
            descriptors.len(),
            reference["count"].as_u64().unwrap() as usize
        );
        assert_eq!(descriptors.len(), rows.len());
        for (descriptor, row) in descriptors.iter().zip(rows) {
            assert_eq!(
                descriptor.promoted_slot,
                row["promoted_slot"].as_i64().unwrap() as i32
            );
            assert_eq!(
                descriptor.constraint_count,
                row["constraint_count"].as_u64().unwrap() as u32
            );
            let constraints = row["constraints"].as_array().unwrap();
            assert_eq!(descriptor.constraint_count as usize, constraints.len());
            for (index, constraint) in constraints.iter().enumerate() {
                assert_eq!(
                    descriptor.constraints[index].draw_index,
                    constraint[0].as_u64().unwrap() as u32
                );
                assert_eq!(
                    descriptor.constraints[index].start_u16,
                    constraint[1].as_u64().unwrap() as u32
                );
                assert_eq!(
                    descriptor.constraints[index].end_u16,
                    constraint[2].as_u64().unwrap() as u32
                );
            }
        }
    }

    /// The ported compiler must reproduce the shipped plan graph exactly: every
    /// path, every candidate count and weight, every allowed interval and both
    /// pivot affine constants.
    #[test]
    fn compiled_plans_match_the_shipped_reference_vectors() {
        let vector = evidence("complete_r3_plans.json");
        let tables = shipped_tables();
        let request = request_from(&vector);
        let plans = compile_full_composition_plans(&request, &tables, &[]).expect("plans compile");
        let expected = vector["plans"].as_array().unwrap();
        assert_eq!(plans.len(), expected.len(), "plan count");
        for (plan, reference) in plans.iter().zip(expected) {
            assert_plan_matches(plan, reference);
        }
        assert_eq!(
            plans
                .iter()
                .map(CompiledEffectPlan::pivot_state_count)
                .sum::<u64>(),
            vector["family_size"].as_u64().unwrap()
        );
    }

    /// The packed descriptors are what the DLL actually evaluates, so they must
    /// match the shipped expansion row for row.
    #[test]
    fn native_descriptors_match_the_shipped_expansion() {
        let vector = evidence("complete_r3_plans.json");
        let tables = shipped_tables();
        let request = request_from(&vector);
        let plans = compile_full_composition_plans(&request, &tables, &[]).expect("plans compile");
        let expected = vector["descriptors"].as_array().unwrap();
        assert_eq!(plans.len(), expected.len());
        for (plan, reference) in plans.iter().zip(expected) {
            assert_descriptors_match(plan, reference);
        }
    }

    /// The interval predicates and the affine constants must agree with the
    /// shipped evaluator for the vectors' own Seeds, positive and negative.
    #[test]
    fn seed_satisfaction_and_plan_trial_match_the_shipped_reference() {
        let plans_vector = evidence("complete_r3_plans.json");
        let window_vector = evidence("preimage_windows.json");
        let tables = shipped_tables();
        let request = request_from(&plans_vector);
        let plans = compile_full_composition_plans(&request, &tables, &[]).expect("plans compile");
        assert_eq!(
            lcg_affine_for_draw(1),
            (
                plans_vector["affine_draw_1"][0].as_u64().unwrap() as u32,
                plans_vector["affine_draw_1"][1].as_u64().unwrap() as u32
            )
        );
        for entry in window_vector["plan_satisfaction"].as_array().unwrap() {
            let seed = entry["seed"].as_u64().unwrap() as u32;
            let expected = entry["plans"].as_array().unwrap();
            for (plan, reference) in plans.iter().zip(expected) {
                assert_eq!(
                    seed_satisfies_compiled_plan(plan, seed),
                    reference.as_u64().unwrap() == 1,
                    "seed {seed}"
                );
            }
        }
        for (plan_index, reference) in window_vector["per_plan"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for pair in reference["plan_trial_for_seed"].as_array().unwrap() {
                let seed = pair[0].as_u64().unwrap() as u32;
                assert_eq!(
                    plan_trial_for_seed(&plans[plan_index], seed).expect("seed is a pivot state"),
                    pair[1].as_u64().unwrap()
                );
            }
        }
    }

    /// Plain roll minimums are the one post-acceptance criterion the job layer
    /// does not re-check, so the route decides them from its own composition.
    #[test]
    fn the_verifier_enforces_plain_roll_minimums() {
        let vector = evidence("complete_r3_plans.json");
        let tables = std::sync::Arc::new(shipped_tables());
        let request = request_from(&vector);
        let level = vector["level"].as_u64().unwrap() as u16;
        let seed = vector["seed"].as_u64().unwrap() as u32;
        let record = generate_ng3_rarity3_effect_sequence(&tables, seed, level)
            .expect("the vector Seed composes");
        let rolls: Vec<(u32, u32)> = record
            .effects
            .iter()
            .take(request.secondary_effect_ids.len() + 1)
            .map(|effect| (effect.effect_id, u32::from(effect.roll_percent)))
            .collect();
        assert_eq!(rolls.len(), 4);
        let accepted = vec![(
            request.primary_effect_id,
            request.secondary_effect_ids.clone(),
        )];
        let satisfied = PreimageVerifier::new(
            std::sync::Arc::clone(&tables),
            request.rarity,
            accepted.clone(),
            level,
            true,
            vec![rolls[0]],
        );
        assert!(satisfied.accepts(seed).expect("seed composes"));
        let impossible = PreimageVerifier::new(
            tables,
            request.rarity,
            accepted,
            level,
            true,
            vec![(rolls[0].0, rolls[0].1 + 1)],
        );
        assert!(!impossible.accepts(seed).expect("seed composes"));
    }

    /// The certified forward generator decides acceptance, so re-composing the
    /// vector's own Seeds must reproduce the shipped verified set.
    #[test]
    fn forward_verification_matches_the_shipped_verified_set() {
        let plans_vector = evidence("complete_r3_plans.json");
        let window_vector = evidence("preimage_windows.json");
        let tables = shipped_tables();
        let request = request_from(&plans_vector);
        let level = plans_vector["level"].as_u64().unwrap() as u16;
        let raw: Vec<u32> = window_vector["per_plan"][0]["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| pair[0].as_u64().unwrap() as u32)
            .collect();
        let mut verified: Vec<u32> = raw
            .iter()
            .copied()
            .filter(|seed| composes_to(&tables, &request, level, *seed))
            .collect();
        verified.sort_unstable();
        verified.dedup();
        let expected: Vec<u32> = window_vector["verified_seeds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(verified, expected);
        assert!(!verified.is_empty(), "the vector window must contain hits");
    }

    /// `verify_complete_matches` for the rarity-3 shape, over the ported domain
    /// sequence rather than a Python call.
    fn composes_to(
        tables: &EffectTableIndex,
        request: &FullCompositionRequest,
        level: u16,
        seed: u32,
    ) -> bool {
        if request.natural_only && !is_natural_scroll_id(seed) {
            return false;
        }
        let Ok(record) = generate_ng3_rarity3_effect_sequence(tables, seed, level) else {
            return false;
        };
        let Some(primary) = record.primary() else {
            return false;
        };
        if primary.effect_id != request.primary_effect_id {
            return false;
        }
        let mut secondaries: Vec<u32> = record
            .effects
            .iter()
            .skip(1)
            .take(request.secondary_effect_ids.len())
            .map(|effect| effect.effect_id)
            .collect();
        secondaries.sort_unstable();
        let mut expected = request.secondary_effect_ids.clone();
        expected.sort_unstable();
        secondaries == expected
    }

    /// The one-wildcard planner must reproduce the shipped rarity-4 graph and
    /// must still enumerate the Seed's own legal layout.
    #[test]
    fn the_one_wildcard_planner_matches_the_shipped_reference() {
        let vector = evidence("one_wildcard_r4_plan.json");
        let tables = shipped_tables();
        let request = OneWildcardCompositionRequest {
            rarity: vector["rarity"].as_u64().unwrap() as u8,
            required_effect_ids: vector["required_effect_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            stage_special_effect_id: Some(
                vector["stage_special_effect_id"].as_u64().unwrap() as u32
            ),
            natural_only: true,
            playthrough: vector["playthrough"].as_u64().unwrap() as u8,
        };
        let grace_runs = runs_from(&vector["grace_runs"]);
        let plans = compile_one_wildcard_composition_plans(&request, &tables, &grace_runs)
            .expect("the one-wildcard planner compiles");
        let expected_plans = vector["plans"].as_array().unwrap();
        let expected_descriptors = vector["descriptors"].as_array().unwrap();
        assert_eq!(plans.len(), expected_plans.len());
        for ((plan, plan_reference), descriptor_reference) in
            plans.iter().zip(expected_plans).zip(expected_descriptors)
        {
            assert_plan_matches(plan, plan_reference);
            assert_descriptors_match(plan, descriptor_reference);
        }
        assert_eq!(
            plans
                .iter()
                .map(CompiledEffectPlan::pivot_state_count)
                .sum::<u64>(),
            vector["family_size"].as_u64().unwrap()
        );
        let observed: Vec<u32> = vector["seed_ordered_effect_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        assert!(
            plans
                .iter()
                .flat_map(|plan| plan.paths.iter())
                .any(|path| path.ordered_effect_ids == observed),
            "the Seed's own layout must be one of the enumerated native paths"
        );
        for path in plans.iter().flat_map(|plan| plan.paths.iter()) {
            assert_eq!(path.constraints.len(), 4);
            for effect_id in &request.required_effect_ids {
                assert!(
                    path.ordered_effect_ids.contains(effect_id),
                    "every legal path must contain the required IDs"
                );
            }
        }
    }

    /// A requested secondary that only the single deep slot can produce makes the
    /// set structurally impossible, which the shipped layer refuses before any
    /// search. The feasible set from the reference vectors must pass and the
    /// deep-slot-only set must be refused.
    #[test]
    fn rarity5_rejects_secondaries_that_only_the_deep_slot_can_produce() {
        let tables = shipped_tables();
        let feasible = FullCompositionRequest {
            rarity: 5,
            primary_effect_id: 20781,
            secondary_effect_ids: vec![6410, 12028, 28203, 41127],
            stage_special_effect_id: Some(0x6553),
            natural_only: true,
            playthrough: 3,
        };
        assert!(secondaries_fit_normal_slots(&feasible, &tables, 0x6553).expect("pool"));
        let deep_only = FullCompositionRequest {
            rarity: 5,
            primary_effect_id: 41041,
            secondary_effect_ids: vec![13555, 15994, 44634, 54282],
            stage_special_effect_id: Some(0x6553),
            natural_only: true,
            playthrough: 3,
        };
        assert!(!secondaries_fit_normal_slots(&deep_only, &tables, 0x6553).expect("pool"));
    }

    /// A rarity-5 complete composition terminates in its selected Grace, so the
    /// draw-1 preimage of that Grace is the route and must come from the
    /// captured map. This checks the ported graph, the grace runs and the
    /// certified recomposition against the shipped reference.
    #[test]
    fn the_rarity5_route_matches_the_shipped_reference() {
        let vector = evidence("complete_r5_plans.json");
        let resource =
            nioh3_data::load_effect_resource(&repo_root().join("nioh3_scroll_editor").join("data"))
                .expect("effect resource loads");
        let tables = std::sync::Arc::new(
            EffectTableIndex::from_resource(&resource).expect("effect tables decode"),
        );
        let map = resource
            .grace_maps
            .iter()
            .find(|map| map.rarity == 5)
            .cloned()
            .expect("the rarity-5 Grace map ships with the resource");
        let grace_id = vector["stage_special_effect_id"].as_u64().unwrap() as u32;
        let runs = first_u16_ranges_for_grace(grace_id, &map).expect("grace runs");
        assert_eq!(runs, runs_from(&vector["grace_runs"]));
        let request = FullCompositionRequest {
            rarity: vector["rarity"].as_u64().unwrap() as u8,
            primary_effect_id: vector["primary_effect_id"].as_u64().unwrap() as u32,
            secondary_effect_ids: vector["secondary_effect_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect(),
            stage_special_effect_id: Some(grace_id),
            natural_only: true,
            playthrough: vector["playthrough"].as_u64().unwrap() as u8,
        };
        let plans = compile_full_composition_plans(&request, &tables, &runs)
            .expect("the rarity-5 composition compiles");
        let expected_plans = vector["plans"].as_array().unwrap();
        assert_eq!(plans.len(), expected_plans.len());
        for (plan, reference) in plans.iter().zip(expected_plans) {
            assert_plan_matches(plan, reference);
        }
        assert_eq!(
            plans
                .iter()
                .map(CompiledEffectPlan::pivot_state_count)
                .sum::<u64>(),
            vector["family_size"].as_u64().unwrap()
        );
        let level = vector["level"].as_u64().unwrap() as u16;
        let verifier = PreimageVerifier::build(PreimageVerifierSpec {
            tables: std::sync::Arc::clone(&tables),
            rarity: request.rarity,
            accepted: vec![(
                request.primary_effect_id,
                request.secondary_effect_ids.clone(),
            )],
            stage_special_effect_id: Some(grace_id),
            grace_map: Some(map),
            playthrough: request.playthrough,
            level,
            natural_only: true,
            minimum_rolls: Vec::new(),
            wildcard_required: None,
        });
        for pair in vector["window_verified_seeds"].as_array().unwrap().iter() {
            let seed = pair.as_u64().unwrap() as u32;
            assert!(
                verifier.accepts(seed).expect("the Seed composes"),
                "the shipped verified rarity-5 Seed {seed} must be accepted"
            );
        }
        assert!(!vector["window_verified_seeds"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(
            !verifier
                .accepts(0x0FFF_FFFF)
                .expect("an arbitrary Seed still composes"),
            "a Seed outside the requested composition must not be accepted"
        );
    }
}
