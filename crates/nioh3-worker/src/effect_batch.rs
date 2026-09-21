//! DirectCompute forward filter for partial ordinary-effect requests.
//!
//! Port of `nioh3_scroll_editor/effect_batch_filter.py`. A partial request
//! names only some of the ordinary slots, so it has no complete-composition
//! preimage: the shipped worker instead sweeps the whole seed family and
//! decides every candidate with the accelerator's batched constraint matcher
//! (`match_effect_constraints_d3d11`) before the certified forward generator
//! re-composes it.
//!
//! The native matcher is a necessary predicate, never the acceptance decision:
//! the per-Seed mask it returns is only one of the filters
//! [`PartialEffectVerifier`] applies, and that verifier re-composes the Seed
//! with the exact product generator and re-checks every query criterion from
//! the composed sequence (`effect_seed_solver._verify_effect_sequence`).

use std::collections::BTreeSet;
use std::sync::Arc;

use nioh3_domain::effect::{EffectTableIndex, GraceMap, NativeWeightContext};
use nioh3_domain::record::{ScrollRecord, ScrollRecordBytes};
use nioh3_domain::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_rarity5_grace_effect_sequence,
    materialize_ng3_rarity4_final_record, NG3_RECORD_TYPE,
};

use crate::effect_path::EffectPathError;
use crate::grace_map::CATEGORY_TO_TYPE;
use crate::preimage::{
    EffectCandidateInput, EffectMaskRequest, PreimageError, SpecialGroupInput,
    CATEGORY_CAPACITY_SLOTS,
};

/// `PC_V2_00_02_AUXILIARY_MODE_THRESHOLD`.
pub const PC_V2_00_02_AUXILIARY_MODE_THRESHOLD: u32 = 2000;

/// Seeds per native matcher call.
///
/// Mirrors `effect_seed_solver._iter_solution_prefetch`'s `batch_size`, which
/// is what the shipped worker passes to the DirectCompute matcher.
pub const PREDICATE_BATCH_SIZE: usize = 262_144;

/// Largest criterion-group count the shipped matcher accepts.
const MAX_CRITERION_GROUPS: usize = 32;

/// The packed candidate table plus the scalar configuration one forward
/// filter needs (`_candidate_configuration`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateConfiguration {
    pub candidates: Vec<EffectCandidateInput>,
    pub category_capacities: [u32; CATEGORY_CAPACITY_SLOTS],
    pub promotion_threshold: u32,
    pub minimum_roll_percent: u32,
    pub maximum_roll_percent: u32,
}

/// `_candidate_configuration`.
///
/// Every field is derived from the verified product tables: the effect rows in
/// table order, their native weights, the recovered rarity-4 finalizer weights
/// and the rarity roll range. Nothing is read from a captured fixture.
pub fn candidate_configuration(
    tables: &EffectTableIndex,
    playthrough: u8,
    rarity: u8,
    level: u16,
) -> Result<CandidateConfiguration, EffectPathError> {
    if !(3..=5).contains(&playthrough) {
        return Err(EffectPathError::Rejected(
            "DirectCompute partial-effect filtering requires NG3-NG5".to_string(),
        ));
    }
    if !(3..=5).contains(&rarity) {
        return Err(EffectPathError::Rejected(
            "DirectCompute partial-effect filtering supports rarity 3, 4, or 5".to_string(),
        ));
    }
    let record_type = *CATEGORY_TO_TYPE
        .get(usize::from(playthrough))
        .ok_or_else(|| {
            EffectPathError::Rejected(format!(
                "playthrough {playthrough} has no scroll record type"
            ))
        })?;
    let rarity_definition = tables
        .rarity_generation
        .get(usize::from(rarity))
        .copied()
        .ok_or_else(|| {
            EffectPathError::Data(format!(
                "rarity row {rarity} is missing from the product tables"
            ))
        })?;
    // The shipped builder passes `type_class_for_record_type(record_type,
    // rarity)` and `playthrough_progress(playthrough)` into `effect_weight`.
    // `EffectTableIndex::native_effect_weight_with_slot` derives exactly those
    // two from `NativeWeightContext`, so they are inputs rather than locals
    // here. The context below is the one both call sites share.
    let context = NativeWeightContext {
        record_type,
        rarity,
        playthrough,
        restricted_destination_slot: false,
        extra_selector: 0,
        rarity5_type_floor: 0,
    };

    // The shipped builder walks `effects_by_id.values()` sorted by row index,
    // which is the effect table's own order with the unused row 0 skipped.
    let mut rows: Vec<&nioh3_domain::effect::EffectDefinition> =
        tables.effects_in_row_order.iter().collect();
    rows.sort_by_key(|definition| definition.row_index);

    let mut candidates: Vec<EffectCandidateInput> = Vec::new();
    for effect in rows {
        if effect.row_index == 0
            || !tables
                .candidate_context_allowed(effect.effect_id, record_type, false)
                .map_err(|error| {
                    EffectPathError::Data(format!(
                        "candidate context for effect {}: {error:?}",
                        effect.effect_id
                    ))
                })?
        {
            continue;
        }
        let weight = tables
            .native_effect_weight(effect.effect_id, context)
            .map_err(|error| {
                EffectPathError::Data(format!(
                    "native effect weight for {}: {error:?}",
                    effect.effect_id
                ))
            })?;
        let promoted_weight = weight;
        let mut final_weight_common = 0;
        let mut final_weight_special = 0;
        if playthrough == 3 && rarity == 4 {
            final_weight_common = finalizer_weight(tables, effect.effect_id, context, 0x3C)?;
            final_weight_special = finalizer_weight(tables, effect.effect_id, context, 0x3E)?;
        }
        if !(weight != 0
            || promoted_weight != 0
            || final_weight_common != 0
            || final_weight_special != 0)
        {
            continue;
        }
        let group = tables.group_for_effect(effect.effect_id).ok_or_else(|| {
            EffectPathError::Data(format!(
                "effect {} has no group row in the product tables",
                effect.effect_id
            ))
        })?;
        let promoted = effect.normalization_flags & 0x08 != 0;
        let mut value_one_roll_mask = 0u32;
        if playthrough == 3 && rarity == 4 {
            let mut roll_percent = rarity_definition.minimum_roll_percent;
            while roll_percent <= rarity_definition.maximum_roll_percent {
                let roll = u8::try_from(roll_percent).map_err(|_| {
                    EffectPathError::Data(format!(
                        "rarity-{rarity} roll percent {roll_percent} does not fit in uint8"
                    ))
                })?;
                if tables
                    .resolved_effect_value(u32::from(effect.effect_id), roll, level)
                    .map_err(|error| {
                        EffectPathError::Data(format!(
                            "resolved effect value for {}: {error:?}",
                            effect.effect_id
                        ))
                    })?
                    == 1
                {
                    value_one_roll_mask |=
                        1 << (roll_percent - rarity_definition.minimum_roll_percent);
                }
                roll_percent += 1;
            }
        }
        candidates.push(EffectCandidateInput {
            effect_id: u32::from(effect.effect_id),
            group_key: u32::from(group.group_key),
            category_key: u32::from(group.category_key),
            conflict_mask_0: group.conflict_mask_0,
            conflict_mask_1: group.conflict_mask_1,
            normal_weight: if promoted { 0 } else { weight_bits(weight) },
            promoted_weight: if promoted {
                weight_bits(promoted_weight)
            } else {
                0
            },
            final_weight_common: weight_bits(final_weight_common),
            final_weight_special: weight_bits(final_weight_special),
            completion_candidate: u32::from(promoted),
            value_one_roll_mask,
        });
    }

    let capacities = tables
        .category_capacities(record_type, rarity)
        .map_err(|error| EffectPathError::Data(format!("category capacities: {error:?}")))?;
    Ok(CandidateConfiguration {
        candidates,
        category_capacities: capacities.map(u32::from),
        promotion_threshold: quantization(rarity_definition.promotion_probability_percent),
        minimum_roll_percent: rarity_definition.minimum_roll_percent,
        maximum_roll_percent: rarity_definition.maximum_roll_percent,
    })
}

/// `int(rarity.promotion_probability_percent * 100)`.
///
/// The table stores the percentage as a binary32 value; the shipped Python
/// expression multiplies it in double precision and truncates towards zero.
fn quantization(percent: f32) -> u32 {
    let product = f64::from(percent) * 100.0;
    if product.is_finite() && product >= 0.0 {
        product.trunc() as u32
    } else {
        0
    }
}

/// One recovered rarity-4 finalizer weight (`effect_weight` with slots 0x3C/3E).
fn finalizer_weight(
    tables: &EffectTableIndex,
    effect_id: u16,
    context: NativeWeightContext,
    weight_slot: usize,
) -> Result<i64, EffectPathError> {
    tables
        .native_effect_weight_with_slot(effect_id, context, weight_slot)
        .map_err(|error| {
            EffectPathError::Data(format!(
                "rarity-4 finalizer weight for effect {effect_id}: {error:?}"
            ))
        })
}

/// The packed `int` a native weight argument carries (`ctypes.c_uint32`).
fn weight_bits(value: i64) -> u32 {
    value as u32
}

/// `_special_group_lookup`.
///
/// Rarity 3 draws its primary from the fixed `0x0001` token; rarities 4 and 5
/// need the captured draw-1 Grace map, whose ranges cover every bucket, so the
/// lookup is either one row or all 65,536 of them.
pub fn special_group_lookup(
    tables: &EffectTableIndex,
    rarity: u8,
    special_mapping: Option<&GraceMap>,
) -> Result<Vec<SpecialGroupInput>, EffectPathError> {
    if rarity == 3 {
        let group = tables
            .group_for_effect(0x0001)
            .ok_or_else(|| EffectPathError::Data("effect 0x0001 has no group row".to_string()))?;
        return Ok(vec![SpecialGroupInput {
            group_key: u32::from(group.group_key),
            conflict_mask_0: group.conflict_mask_0,
            conflict_mask_1: group.conflict_mask_1,
            effect_id: 0x0001,
        }]);
    }
    let mapping = special_mapping.ok_or_else(|| {
        EffectPathError::Rejected(
            "rarity-4/5 effect filtering requires a Grace output map".to_string(),
        )
    })?;
    let mut groups: Vec<Option<SpecialGroupInput>> = vec![None; 0x1_0000];
    for range in &mapping.ranges {
        let key = u16::try_from(range.effect_id).map_err(|_| {
            EffectPathError::Data(format!(
                "Grace output map effect id 0x{:X} does not fit in uint16",
                range.effect_id
            ))
        })?;
        let group = tables.group_for_effect(key).ok_or_else(|| {
            EffectPathError::Data(format!(
                "Grace effect 0x{:04X} has no group row in the product tables",
                range.effect_id
            ))
        })?;
        let packed = SpecialGroupInput {
            group_key: u32::from(group.group_key),
            conflict_mask_0: group.conflict_mask_0,
            conflict_mask_1: group.conflict_mask_1,
            effect_id: range.effect_id,
        };
        for slot in groups
            .iter_mut()
            .take(usize::from(range.end) + 1)
            .skip(usize::from(range.start))
        {
            *slot = Some(packed);
        }
    }
    groups
        .into_iter()
        .map(|group| {
            group.ok_or_else(|| {
                EffectPathError::Data(
                    "Grace output map does not cover every first-draw bucket".to_string(),
                )
            })
        })
        .collect()
}

/// `_merged_rarity4_criterion_groups`: merge overlapping final requirements.
///
/// The rarity-4 finalizer-aware matcher accepts a `mask.bit_count() >= n - 1`
/// rule over these merged components, which is the lossless N-1 filter the
/// shipped worker uses instead of the exact per-item groups.
pub fn merged_rarity4_criterion_groups(
    primary_effect_ids: &[u32],
    required_secondary_ids: &[u32],
    required_secondary_id_groups: &[Vec<u32>],
) -> Vec<(u32, Vec<u32>)> {
    let mut pending: Vec<Vec<u32>> = Vec::new();
    if !primary_effect_ids.is_empty() {
        pending.push(sorted_unique(primary_effect_ids));
    }
    for effect_id in sorted_unique(required_secondary_ids) {
        pending.push(vec![effect_id]);
    }
    for group in required_secondary_id_groups {
        pending.push(sorted_unique(group));
    }
    let mut components: Vec<BTreeSet<u32>> = Vec::new();
    for values in pending {
        let mut merged: BTreeSet<u32> = values.into_iter().collect();
        let mut changed = true;
        while changed {
            changed = false;
            let mut retained: Vec<BTreeSet<u32>> = Vec::new();
            for component in components.drain(..) {
                if merged.intersection(&component).next().is_some() {
                    merged.extend(component);
                    changed = true;
                } else {
                    retained.push(component);
                }
            }
            components = retained;
        }
        components.push(merged);
    }
    let mut merged: Vec<Vec<u32>> = components
        .into_iter()
        .map(|component| component.into_iter().collect())
        .collect();
    merged.sort_by_key(|group| {
        (
            group.first().copied().unwrap_or(0),
            group.len(),
            group.clone(),
        )
    });
    merged.into_iter().map(|group| (2, group)).collect()
}

fn sorted_unique(values: &[u32]) -> Vec<u32> {
    let mut sorted: Vec<u32> = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
}

/// The complete static input of one forward-filter route.
///
/// Built once per compiled query, then reused by every batched native call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMaskSpec {
    pub candidates: Vec<EffectCandidateInput>,
    pub special_groups: Vec<SpecialGroupInput>,
    pub category_capacities: [u32; CATEGORY_CAPACITY_SLOTS],
    /// `(kind, keys)` pairs, in the shipped order.
    pub criterion_groups: Vec<(u32, Vec<u32>)>,
    pub rarity: u32,
    pub ordinary_slot_count: u32,
    pub slot_limit: u32,
    pub promotion_threshold: u32,
    pub consumes_special_draw: bool,
    pub minimum_roll_percent: u32,
    pub maximum_roll_percent: u32,
    pub apply_r4_finalizer: bool,
    pub auxiliary_mode_threshold: u32,
    /// `(1 << criterion_groups.len()) - 1`.
    pub target_mask: u32,
    /// Set for the merged rarity-4 filter: a Seed passes when its mask has at
    /// least this many bits.
    pub minimum_matched_groups: Option<u32>,
}

impl EffectMaskSpec {
    /// One native request over a Seed batch.
    pub fn request(&self, seeds: &[u32], preferred_vendor_id: u32) -> EffectMaskRequest {
        EffectMaskRequest {
            seeds: seeds.to_vec(),
            candidates: self.candidates.clone(),
            special_groups: self.special_groups.clone(),
            category_capacities: self.category_capacities.to_vec(),
            criterion_groups: self.criterion_groups.clone(),
            rarity: self.rarity,
            ordinary_slot_count: self.ordinary_slot_count,
            slot_limit: self.slot_limit,
            promotion_threshold: self.promotion_threshold,
            consumes_special_draw: self.consumes_special_draw,
            minimum_roll_percent: self.minimum_roll_percent,
            maximum_roll_percent: self.maximum_roll_percent,
            apply_r4_finalizer: self.apply_r4_finalizer,
            auxiliary_mode_threshold: self.auxiliary_mode_threshold,
            preferred_vendor_id,
        }
    }

    /// Whether one Seed's native mask passes the route's own criterion.
    pub fn accepts_mask(&self, mask: u32) -> bool {
        match self.minimum_matched_groups {
            Some(minimum) => mask.count_ones() >= minimum,
            None => mask == self.target_mask,
        }
    }
}

/// `partial_effect_batch_generator`'s criterion side, without the native call.
///
/// Returns `None` when the request carries no ordinary-effect constraint at
/// all, which is exactly when the shipped builder returns no filter.
// The argument list mirrors the shipped builder's keyword arguments one for one
// so the port stays directly comparable to `effect_batch_filter.py`.
#[allow(clippy::too_many_arguments)]
pub fn plan_effect_mask(
    tables: &EffectTableIndex,
    special_mapping: Option<&GraceMap>,
    playthrough: u8,
    rarity: u8,
    level: u16,
    primary_effect_ids: &[u32],
    required_secondary_ids: &[u32],
    required_secondary_id_groups: &[Vec<u32>],
) -> Result<Option<EffectMaskSpec>, EffectPathError> {
    let mut exact_groups: Vec<(u32, Vec<u32>)> = Vec::new();
    if !primary_effect_ids.is_empty() {
        exact_groups.push((0, sorted_unique(primary_effect_ids)));
    }
    let ordinary_kind = if primary_effect_ids.is_empty() { 2 } else { 1 };
    for effect_id in sorted_unique(required_secondary_ids) {
        exact_groups.push((ordinary_kind, vec![effect_id]));
    }
    for group in required_secondary_id_groups {
        exact_groups.push((ordinary_kind, sorted_unique(group)));
    }
    if exact_groups.is_empty() {
        return Ok(None);
    }
    let mut groups = exact_groups;
    let mut use_exact_r4_finalizer = false;
    let mut minimum_matched_groups: Option<u32> = None;
    if playthrough == 3 && rarity == 4 {
        let merged = merged_rarity4_criterion_groups(
            primary_effect_ids,
            required_secondary_ids,
            required_secondary_id_groups,
        );
        if merged.len() >= 3 {
            minimum_matched_groups = Some(merged.len() as u32 - 1);
            groups = merged;
        } else {
            use_exact_r4_finalizer = true;
        }
    }
    if groups.len() > MAX_CRITERION_GROUPS {
        return Err(EffectPathError::Rejected(format!(
            "the DirectCompute effect matcher accepts at most {MAX_CRITERION_GROUPS} criterion \
             groups, not {}",
            groups.len()
        )));
    }
    let configuration = candidate_configuration(tables, playthrough, rarity, level)?;
    let special_groups = special_group_lookup(tables, rarity, special_mapping)?;
    let target_mask = (1u32 << groups.len()) - 1;
    Ok(Some(EffectMaskSpec {
        candidates: configuration.candidates,
        special_groups,
        category_capacities: configuration.category_capacities,
        criterion_groups: groups,
        rarity: u32::from(rarity),
        ordinary_slot_count: if rarity == 5 { 5 } else { 4 },
        slot_limit: match rarity {
            5 => 6,
            4 => 5,
            _ => 4,
        },
        promotion_threshold: configuration.promotion_threshold,
        consumes_special_draw: matches!(rarity, 4 | 5),
        minimum_roll_percent: configuration.minimum_roll_percent,
        maximum_roll_percent: configuration.maximum_roll_percent,
        apply_r4_finalizer: use_exact_r4_finalizer,
        auxiliary_mode_threshold: PC_V2_00_02_AUXILIARY_MODE_THRESHOLD,
        target_mask,
        minimum_matched_groups,
    }))
}

/// The exact criteria the certified composition of one Seed must satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartialEffectCriteria {
    pub primary_effect_ids: Vec<u32>,
    pub required_secondary_ids: Vec<u32>,
    pub required_secondary_id_groups: Vec<Vec<u32>>,
    pub grace_effect_id: Option<u32>,
    pub minimum_roll_percent_by_effect_id: Vec<(u32, u32)>,
}

/// The final certified composition gate for the partial-effect route.
///
/// Port of `effect_seed_solver._verify_effect_sequence`: the Seed is
/// re-composed with the exact product generator and every query criterion is
/// decided from that composition. The native mask never substitutes for this.
#[derive(Debug)]
pub struct PartialEffectVerifier {
    tables: Arc<EffectTableIndex>,
    rarity: u8,
    playthrough: u8,
    level: u16,
    criteria: PartialEffectCriteria,
    r4_grace: Option<GraceMap>,
    r5_grace: Option<GraceMap>,
}

impl PartialEq for PartialEffectVerifier {
    /// Two verifiers are the same when they decide the same criteria on the
    /// same rarity, level and maps. The product tables are loaded data, so they
    /// are compared by identity rather than by value.
    fn eq(&self, other: &Self) -> bool {
        self.rarity == other.rarity
            && self.playthrough == other.playthrough
            && self.level == other.level
            && self.criteria == other.criteria
            && self.r4_grace == other.r4_grace
            && self.r5_grace == other.r5_grace
            && Arc::ptr_eq(&self.tables, &other.tables)
    }
}

impl Eq for PartialEffectVerifier {}

impl PartialEffectVerifier {
    /// Build the verifier for one partial request.
    pub fn build(
        tables: Arc<EffectTableIndex>,
        rarity: u8,
        playthrough: u8,
        level: u16,
        criteria: PartialEffectCriteria,
        r4_grace: Option<GraceMap>,
        r5_grace: Option<GraceMap>,
    ) -> Self {
        Self {
            tables,
            rarity,
            playthrough,
            level,
            criteria,
            r4_grace,
            r5_grace,
        }
    }

    /// The certified composition of one Seed, or the named reason it cannot be
    /// composed. A composition failure fails the job closed, exactly like the
    /// shipped layer, because a Seed the generator cannot compose must never
    /// silently disappear from the result set.
    fn compose(&self, seed: u32) -> Result<ScrollRecord, EffectPathError> {
        let composed = match self.rarity {
            3 => generate_ng3_rarity3_effect_sequence(&self.tables, seed, self.level),
            4 => {
                let grace = self.r4_grace.as_ref().ok_or_else(|| {
                    EffectPathError::Unsupported(
                        "the rarity-4 partial-effect route needs its captured draw-1 Grace map"
                            .to_string(),
                    )
                })?;
                let mut template = ScrollRecordBytes::zeroed();
                template.write_u16(0x00, NG3_RECORD_TYPE).map_err(|error| {
                    EffectPathError::Data(format!("rarity-4 template: {error:?}"))
                })?;
                materialize_ng3_rarity4_final_record(
                    &self.tables,
                    grace,
                    &template,
                    seed,
                    self.level,
                    0,
                    0,
                    0,
                )
                .map(|pair| pair.preview_sequence().clone())
            }
            5 => {
                let grace = self.r5_grace.as_ref().ok_or_else(|| {
                    EffectPathError::Unsupported(
                        "the rarity-5 partial-effect route needs its captured draw-1 Grace map"
                            .to_string(),
                    )
                })?;
                generate_rarity5_grace_effect_sequence(
                    &self.tables,
                    grace,
                    self.playthrough,
                    seed,
                    self.level,
                )
            }
            other => {
                return Err(EffectPathError::Unsupported(format!(
                    "the certified partial-effect composition is implemented for rarities 3, 4 \
                     and 5, not rarity {other}"
                )))
            }
        };
        composed.map_err(|error| {
            EffectPathError::Data(format!(
                "certified rarity-{} partial composition: {error:?}",
                self.rarity
            ))
        })
    }

    /// Whether the certified composition satisfies every query criterion.
    pub fn accepts(&self, seed: u32) -> Result<bool, EffectPathError> {
        let record = self.compose(seed)?;
        let Some(primary) = record.primary() else {
            return Ok(false);
        };
        let criteria = &self.criteria;
        if !criteria.primary_effect_ids.is_empty()
            && !criteria.primary_effect_ids.contains(&primary.effect_id)
        {
            return Ok(false);
        }
        let secondaries = record.secondaries();
        let mut ordinary_match_ids: BTreeSet<u32> =
            secondaries.iter().map(|effect| effect.effect_id).collect();
        if criteria.primary_effect_ids.is_empty() {
            ordinary_match_ids.insert(primary.effect_id);
        }
        if !criteria
            .required_secondary_ids
            .iter()
            .all(|effect_id| ordinary_match_ids.contains(effect_id))
        {
            return Ok(false);
        }
        if criteria.required_secondary_id_groups.iter().any(|group| {
            !group
                .iter()
                .any(|effect_id| ordinary_match_ids.contains(effect_id))
        }) {
            return Ok(false);
        }
        if let Some(expected) = criteria.grace_effect_id {
            let observed = if record.terminal_is_special {
                record.effects.last().map(|effect| effect.effect_id)
            } else {
                None
            };
            if observed != Some(expected) {
                return Ok(false);
            }
        }
        if !criteria.minimum_roll_percent_by_effect_id.is_empty() {
            let ordinary: Vec<&nioh3_domain::record::ScrollEffect> =
                std::iter::once(primary).chain(secondaries.iter()).collect();
            for (effect_id, minimum_roll) in &criteria.minimum_roll_percent_by_effect_id {
                let matching: Vec<&&nioh3_domain::record::ScrollEffect> = ordinary
                    .iter()
                    .filter(|effect| effect.effect_id == *effect_id)
                    .collect();
                if matching.is_empty()
                    && criteria.primary_effect_ids.contains(effect_id)
                    && !criteria.required_secondary_ids.contains(effect_id)
                {
                    continue;
                }
                if !matching
                    .iter()
                    .any(|effect| u32::from(effect.roll_percent) >= *minimum_roll)
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// Map a forward-filter failure onto the named route refusal.
pub fn unavailable(error: &PreimageError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;
    use crate::query_compile::QueryCompiler;

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
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        serde_json::from_str(&text).expect("evidence is valid JSON")
    }

    fn compiler() -> QueryCompiler {
        QueryCompiler::load(&repo_root().join("nioh3_scroll_editor").join("data"))
            .expect("the product tables load")
    }

    fn u32_list(value: &Value) -> Vec<u32> {
        value
            .as_array()
            .expect("array")
            .iter()
            .map(|item| item.as_u64().expect("integer") as u32)
            .collect()
    }

    /// The captured rarity-3 mask vector must reproduce field for field.
    #[test]
    fn the_rarity3_candidate_configuration_matches_the_captured_vector() {
        let vector = evidence("forward_filter_masks.json");
        let compiler = compiler();
        let rarity = vector["rarity"].as_u64().unwrap() as u8;
        let configuration =
            candidate_configuration(compiler.effect_index(), 3, rarity, 180).expect("configured");
        let expected: Vec<EffectCandidateInput> = vector["candidate_rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let values = u32_list(row);
                EffectCandidateInput {
                    effect_id: values[0],
                    group_key: values[1],
                    category_key: values[2],
                    conflict_mask_0: values[3],
                    conflict_mask_1: values[4],
                    normal_weight: values[5],
                    promoted_weight: values[6],
                    final_weight_common: values[7],
                    final_weight_special: values[8],
                    completion_candidate: values[9],
                    value_one_roll_mask: values[10],
                }
            })
            .collect();
        assert_eq!(configuration.candidates, expected);
        assert_eq!(
            configuration.category_capacities.to_vec(),
            u32_list(&vector["category_capacities"])
        );
        assert_eq!(
            configuration.promotion_threshold,
            vector["promotion_threshold"].as_u64().unwrap() as u32
        );
        assert_eq!(
            configuration.minimum_roll_percent,
            vector["minimum_roll_percent"].as_u64().unwrap() as u32
        );
        assert_eq!(
            configuration.maximum_roll_percent,
            vector["maximum_roll_percent"].as_u64().unwrap() as u32
        );
    }

    /// The same vector's packed plan must reproduce the shipped groups.
    #[test]
    fn the_rarity3_effect_mask_plan_matches_the_captured_vector() {
        let vector = evidence("forward_filter_masks.json");
        let compiler = compiler();
        let special_groups = special_group_lookup(compiler.effect_index(), 3, None)
            .expect("the rarity-3 special group resolves");
        let expected: Vec<SpecialGroupInput> = vector["special_groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let values = u32_list(row);
                SpecialGroupInput {
                    group_key: values[0],
                    conflict_mask_0: values[1],
                    conflict_mask_1: values[2],
                    effect_id: values[3],
                }
            })
            .collect();
        assert_eq!(special_groups, expected);

        let criterion_groups: Vec<(u32, Vec<u32>)> = vector["criterion_groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|group| (group[0].as_u64().unwrap() as u32, u32_list(&group[1])))
            .collect();
        let primary = criterion_groups[0].1.clone();
        let secondaries: Vec<u32> = criterion_groups[1..]
            .iter()
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        let spec = plan_effect_mask(
            compiler.effect_index(),
            None,
            3,
            3,
            180,
            &primary,
            &secondaries,
            &[],
        )
        .expect("the plan compiles")
        .expect("the request carries effect constraints");
        assert_eq!(spec.criterion_groups, criterion_groups);
        assert_eq!(
            spec.target_mask,
            vector["target_mask"].as_u64().unwrap() as u32
        );
        assert_eq!(spec.ordinary_slot_count, 4);
        assert_eq!(spec.slot_limit, 4);
        assert!(!spec.consumes_special_draw);
        assert!(!spec.apply_r4_finalizer);
        assert_eq!(spec.minimum_matched_groups, None);
    }

    /// A request with no ordinary-effect criterion compiles to no filter.
    #[test]
    fn a_request_without_effect_constraints_has_no_mask() {
        let compiler = compiler();
        assert!(
            plan_effect_mask(compiler.effect_index(), None, 3, 3, 180, &[], &[], &[])
                .expect("compiles")
                .is_none()
        );
    }

    /// Overlapping rarity-4 requirements merge exactly like the reference.
    ///
    /// The shipped merge is deliberately not a full transitive closure: each
    /// pending requirement is folded into the components that already exist,
    /// so a later requirement only joins the components it actually touches at
    /// insertion time. These three cases are the reference's own answers.
    #[test]
    fn overlapping_rarity4_requirements_merge_into_components() {
        let merged = merged_rarity4_criterion_groups(&[10], &[10, 11, 12], &[vec![12, 13]]);
        assert_eq!(
            merged,
            vec![(2, vec![10]), (2, vec![11]), (2, vec![12, 13])]
        );
        let disjoint = merged_rarity4_criterion_groups(&[10], &[20, 30], &[]);
        assert_eq!(disjoint, vec![(2, vec![10]), (2, vec![20]), (2, vec![30])]);
        let chained = merged_rarity4_criterion_groups(&[10], &[10, 11], &[vec![11, 12]]);
        assert_eq!(chained, vec![(2, vec![10]), (2, vec![11, 12])]);
    }

    /// The rarity-4/5 candidate tables, finalizer weights and merged groups
    /// must reproduce the shipped builder field for field.
    #[test]
    fn the_rarity4_and_rarity5_configurations_match_the_captured_vectors() {
        let vector = evidence("forward_filter_r4_r5_masks.json");
        let compiler = compiler();
        for key in ["rarity4", "rarity4_merged", "rarity5"] {
            let entry = &vector[key];
            let rarity = entry["rarity"].as_u64().unwrap() as u8;
            let configuration = candidate_configuration(compiler.effect_index(), 3, rarity, 180)
                .expect("configured");
            let expected: Vec<EffectCandidateInput> = entry["candidate_rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let values = u32_list(row);
                    EffectCandidateInput {
                        effect_id: values[0],
                        group_key: values[1],
                        category_key: values[2],
                        conflict_mask_0: values[3],
                        conflict_mask_1: values[4],
                        normal_weight: values[5],
                        promoted_weight: values[6],
                        final_weight_common: values[7],
                        final_weight_special: values[8],
                        completion_candidate: values[9],
                        value_one_roll_mask: values[10],
                    }
                })
                .collect();
            assert_eq!(configuration.candidates, expected, "{key} candidate table");
            assert_eq!(
                configuration.category_capacities.to_vec(),
                u32_list(&entry["category_capacities"]),
                "{key} category capacities"
            );
            assert_eq!(
                configuration.promotion_threshold,
                entry["promotion_threshold"].as_u64().unwrap() as u32,
                "{key} promotion threshold"
            );

            let primary = u32_list(&entry["primary"]);
            let secondaries = u32_list(&entry["secondaries"]);
            let groups: Vec<Vec<u32>> = entry["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(u32_list)
                .collect();
            let mapping = match rarity {
                4 => compiler.r4_grace(),
                5 => compiler.r5_grace(),
                _ => None,
            };
            let spec = plan_effect_mask(
                compiler.effect_index(),
                mapping,
                3,
                rarity,
                180,
                &primary,
                &secondaries,
                &groups,
            )
            .expect("the plan compiles")
            .expect("the request carries effect constraints");
            let exact: Vec<(u32, Vec<u32>)> = entry["exact_groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|group| (group[0].as_u64().unwrap() as u32, u32_list(&group[1])))
                .collect();
            let merged: Vec<(u32, Vec<u32>)> = entry["merged_groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|group| (group[0].as_u64().unwrap() as u32, u32_list(&group[1])))
                .collect();
            if rarity == 4 && merged.len() >= 3 {
                assert_eq!(spec.criterion_groups, merged, "{key} merged groups");
                assert_eq!(spec.minimum_matched_groups, Some(merged.len() as u32 - 1));
                assert!(!spec.apply_r4_finalizer, "{key} uses the merged filter");
            } else {
                assert_eq!(spec.criterion_groups, exact, "{key} exact groups");
                assert_eq!(spec.minimum_matched_groups, None);
                assert_eq!(spec.apply_r4_finalizer, rarity == 4, "{key} finalizer flag");
            }
            assert_eq!(spec.rarity, u32::from(rarity));
            assert_eq!(spec.ordinary_slot_count, if rarity == 5 { 5 } else { 4 });
            assert_eq!(
                spec.slot_limit,
                match rarity {
                    5 => 6,
                    4 => 5,
                    _ => 4,
                }
            );
            assert!(spec.consumes_special_draw);
            assert_eq!(spec.target_mask, (1u32 << spec.criterion_groups.len()) - 1);
        }
    }

    /// The Grace draw-1 pivot table, its cursor algebra and the certified
    /// verifier must reproduce the shipped solver's own anchor.
    ///
    /// The expected permutation, trial and Seed come from the shipped solver for
    /// Grace 0x6553 (`deliverables/m23d-preimage/scripts/probe_forward_filter_route.py`
    /// and `evidence/forward_filter_probes.json`): trial 393,530 of the Grace
    /// pivot is Seed 88,364,494, which composes primary 20781 with secondaries
    /// 6410 and 12028.
    #[test]
    fn the_grace_filtered_rarity5_route_matches_the_shipped_anchor() {
        let compiler = compiler();
        let map = compiler
            .r5_grace()
            .expect("the rarity-5 Grace map is loaded");
        let values = crate::effect_path::grace_pivot_values(0x6553, map)
            .expect("the Grace draw-1 preimage exists");
        assert_eq!(values.len(), 5964, "Grace 0x6553 covers 5,964 buckets");
        assert_eq!(
            &values[..8],
            &[0, 4721, 3478, 2235, 992, 5713, 4470, 3227],
            "the permuted pivot table must match joint_solver.permuted_pivot_values"
        );
        let window = crate::native_search::PivotWindow {
            start_index: 0,
            stop_index: 393_530,
            low16_stride: 0x9E37,
            draw_index: 1,
        };
        let seed =
            crate::search_backend::SearchBackend::replay_pivot_seed(&values, &window, 393_530)
                .expect("the cursor replays");
        assert_eq!(seed, 88_364_494);

        let verifier = PartialEffectVerifier::build(
            compiler.effect_index_arc(),
            5,
            3,
            180,
            PartialEffectCriteria {
                primary_effect_ids: vec![20781],
                required_secondary_ids: vec![6410, 12028],
                ..PartialEffectCriteria::default()
            },
            None,
            compiler.r5_grace().cloned(),
        );
        assert!(
            verifier.accepts(seed).expect("the certified composer runs"),
            "the shipped Seed must satisfy the shipped criteria"
        );
        assert!(
            !verifier
                .accepts(1_511_872_763)
                .expect("the certified composer runs"),
            "an unrelated Seed must not satisfy them"
        );
    }

    /// The route's own page loop must find the shipped Seed in a tiny window.
    ///
    /// This is the regression that matters for the Grace pivot: the shipped
    /// solver reports Seed 162,486,523 at trial 15 of the Grace draw-1 table for
    /// a Grace-only rarity-5 request, so a page over the first 100,000 trials
    /// must publish exactly that pair.
    #[test]
    fn the_grace_filtered_rarity5_page_finds_the_shipped_seed() {
        use crate::native_search::Accelerator;
        use crate::query::SearchQuery;
        use crate::search_backend::{MatchFilter, PageRequest, SearchBackend};

        let root = repo_root();
        let compiler = compiler();
        let accelerator =
            Arc::new(Accelerator::load(&root, None).expect("the shipped seed accelerator loads"));
        let query = SearchQuery::from_payload(&serde_json::json!({
            "playthrough": 3,
            "rarity": 5,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": 0x6553,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        }))
        .expect("the Grace-only rarity-5 request is valid");
        let compiled = compiler
            // The Grace-filtered route is rarity 5, so the
            // DirectCompute-gated rarity-3 pivot never applies.
            .compile(&query, &accelerator, false)
            .expect("the Grace partial route compiles");
        assert_eq!(
            compiled.route,
            crate::query_compile::Route::PartialEffectFilter
        );
        let preimage_path = crate::capabilities::effect_preimage_path(&root, None);
        let preimage =
            crate::preimage::PreimageAccelerator::load(&root, Some(&preimage_path)).map(Arc::new);
        let backend = SearchBackend::new(Some(Arc::clone(&accelerator)), preimage);
        let page_filter = compiled.page_filter.as_ref();
        let match_filter = page_filter.map(|filter| MatchFilter {
            primary: filter.primary.as_ref(),
            auxiliary: filter.auxiliary.as_ref(),
            effect_mask: filter.effect_mask.as_deref(),
            effect_verifier: filter.effect_verifier.as_deref(),
        });
        let request = PageRequest::chunk(0, 100_000, compiled.chunk_trials, 1);
        let page = backend
            .collect_page_filtered(
                &compiled.native,
                &request,
                &|| false,
                &mut |_| {},
                match_filter.as_ref(),
            )
            .expect("the Grace page runs");
        assert_eq!(
            page.matches.len(),
            1,
            "the Grace pivot page must publish the shipped Seed"
        );
        assert_eq!(page.matches[0].seed, 162_486_523);
        assert_eq!(page.matches[0].trial, 15);
    }

    /// A rarity-3 partial query names only some ordinary slots.
    #[test]
    fn a_rarity3_partial_plan_uses_exact_groups() {
        let compiler = compiler();
        let spec = plan_effect_mask(
            compiler.effect_index(),
            None,
            3,
            3,
            180,
            &[60020],
            &[12028],
            &[vec![16437, 39485]],
        )
        .expect("compiles")
        .expect("has constraints");
        assert_eq!(
            spec.criterion_groups,
            vec![(0, vec![60020]), (1, vec![12028]), (1, vec![16437, 39485])]
        );
        assert_eq!(spec.target_mask, 0b111);
        assert!(spec.accepts_mask(0b111));
        assert!(!spec.accepts_mask(0b101));
    }
}
