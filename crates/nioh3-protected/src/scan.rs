//! Port of the `native.scan_next_candidate` seed loops and their filters.
//!
//! The shipped runtime search is a driver over one batch oracle: it proposes
//! seeds, asks the game's own generator for the records, optionally runs the
//! native completion pass, then accepts a record only when it satisfies the
//! caller's filters. This module ports that driver, including the parts that are
//! easy to get subtly wrong:
//!
//! - the batch sizing and cursor arithmetic are the shipped ones, so the same
//!   range is proposed in the same order with the same number of native calls;
//! - the completion pass runs only where `_should_finalize_native_record` says
//!   it does, and the stage-one record it consumes is published as the
//!   *installation* record beside the completed record - never instead of it;
//! - an accelerated Grace prediction that the game contradicts is a hard
//!   failure (a stale or wrong-context map must not read as an ordinary miss)
//!   while the non-accelerated path treats the same contradiction as a miss;
//! - cancellation is checked at the top of every batch, and progress is
//!   reported after every batch with the shipped field names.
//!
//! The loops deliberately know nothing about a game process: they drive
//! [`BatchOracle`], which the product binds to the native oracle and the gates
//! bind to a deterministic scripted one.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use nioh3_domain::preview::AuxiliaryPreview;
pub use nioh3_runtime::mutation::catalog::{EFFECT_START, EFFECT_STRIDE};
use nioh3_worker::model::RecordStage;

use crate::error::HostError;
use crate::maps::{category_for_record_type, DrawConstraint};
use crate::oracle::{BatchOracle, ORACLE_TIMEOUT_MS};

/// `emaki_exchange.SCROLL_RECORD_SIZE`.
pub const SCROLL_RECORD_SIZE: usize = 0xE8;
/// The seven effect slots a scroll record carries.
pub const EFFECT_COUNT: usize = 7;
/// `native.ScanProgress`: the progress object the shipped host reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanProgress {
    pub scanned: u64,
    pub current_seed: u32,
    pub joint_trial: Option<u64>,
}

impl ScanProgress {
    /// `dataclasses.asdict(update)`, with the same keys in the same order.
    pub fn to_json(&self) -> Value {
        json!({
            "scanned": self.scanned,
            "current_seed": self.current_seed,
            "joint_trial": self.joint_trial,
        })
    }
}

/// `native._should_finalize_native_record`.
///
/// Completion runs before filtering for rarity 4 in every playthrough except 4
/// and 5, and for rarity 3 in the two early playthroughs.
pub fn should_finalize_native_record(rarity: u8, playthrough: Option<u32>) -> bool {
    let early = matches!(playthrough, None | Some(1) | Some(2) | Some(3));
    (rarity == 4 && early) || (rarity == 3 && matches!(playthrough, Some(1) | Some(2)))
}

/// `models.AuxiliarySearchCriteria`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuxiliaryCriteria {
    pub required_terrain_effect_keys: Vec<u32>,
    pub required_terrain_effect_key_groups: Vec<Vec<u32>>,
    pub required_special_rule_keys: Vec<u32>,
    pub required_special_rule_key_groups: Vec<Vec<u32>>,
    pub required_enemy_lookup_keys: Vec<u32>,
    pub required_enemy_lookup_key_groups: Vec<Vec<u32>>,
}

impl AuxiliaryCriteria {
    pub fn is_empty(&self) -> bool {
        self.required_terrain_effect_keys.is_empty()
            && self.required_terrain_effect_key_groups.is_empty()
            && self.required_special_rule_keys.is_empty()
            && self.required_special_rule_key_groups.is_empty()
            && self.required_enemy_lookup_keys.is_empty()
            && self.required_enemy_lookup_key_groups.is_empty()
    }
}

/// `AuxiliarySearchCriteria.matches`.
///
/// The three sub-checks in the shipped order: terrain display keys, the
/// non-zero special-rule keys, then the enemy lookup keys of every composed
/// group, each requiring the requested set as a subset plus one hit per any-of
/// group.
pub fn auxiliary_criteria_match(criteria: &AuxiliaryCriteria, preview: &AuxiliaryPreview) -> bool {
    let terrain: Vec<u32> = preview
        .terrain
        .display_effect_keys
        .iter()
        .map(|key| u32::from(*key))
        .collect();
    if !contains_all(&terrain, &criteria.required_terrain_effect_keys)
        || !hits_every_group(&terrain, &criteria.required_terrain_effect_key_groups)
    {
        return false;
    }
    let rules: Vec<u32> = preview
        .special_rules
        .keys
        .iter()
        .filter(|key| **key != 0)
        .map(|key| u32::from(*key))
        .collect();
    if !contains_all(&rules, &criteria.required_special_rule_keys)
        || !hits_every_group(&rules, &criteria.required_special_rule_key_groups)
    {
        return false;
    }
    let enemies: Vec<u32> = preview
        .enemy_groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .map(|entry| entry.lookup_key)
        .collect();
    contains_all(&enemies, &criteria.required_enemy_lookup_keys)
        && hits_every_group(&enemies, &criteria.required_enemy_lookup_key_groups)
}

fn contains_all(actual: &[u32], required: &[u32]) -> bool {
    required.iter().all(|key| actual.contains(key))
}

fn hits_every_group(actual: &[u32], groups: &[Vec<u32>]) -> bool {
    groups
        .iter()
        .all(|group| group.iter().any(|key| actual.contains(key)))
}

/// The game-closed auxiliary generator, injected so the loops stay free of
/// product tables.
pub trait AuxiliarySource {
    /// `auxiliary_generation.generate_complete_auxiliary`.
    fn compose(&mut self, seed: u32, playthrough: u32) -> Result<AuxiliaryPreview, HostError>;
}

/// The filters one scan applies to every native record.
#[derive(Debug, Clone, Default)]
pub struct ScanFilters {
    pub rarity: u8,
    pub playthrough: Option<u32>,
    pub primary_effect_ids: BTreeSet<u32>,
    pub required_secondary_ids: BTreeSet<u32>,
    pub required_secondary_id_groups: Vec<BTreeSet<u32>>,
    pub grace_effect_id: Option<u32>,
    /// `native.scan_next_candidate`'s `grace_effect_slot`, default 6.
    pub grace_effect_slot: usize,
    pub required_slot5_effect_id: Option<u32>,
    pub record_stage: Option<RecordStage>,
    pub strict_grace_prediction: bool,
    pub auxiliary: AuxiliaryCriteria,
}

impl ScanFilters {
    pub fn new(rarity: u8, playthrough: Option<u32>) -> Self {
        Self {
            rarity,
            playthrough,
            grace_effect_slot: 6,
            strict_grace_prediction: true,
            ..Self::default()
        }
    }

    fn resolved_record_stage(&self) -> RecordStage {
        self.record_stage.unwrap_or(if self.rarity == 5 {
            RecordStage::FinalRecord
        } else {
            RecordStage::NativeStageOne
        })
    }
}

/// One accepted native record and the facts the payload needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanMatch {
    pub seed: u32,
    pub rarity: u8,
    pub playthrough: Option<u32>,
    pub record_stage: RecordStage,
    /// The record the scan accepted (completed when the completion pass ran).
    pub record: Vec<u8>,
    /// The stage-one record the game still has to complete, when the completion
    /// pass ran. It is published beside `record`, never instead of it.
    pub installation_record: Option<Vec<u8>>,
    /// 1-based joint-solver trial, or `None` for a seed scan.
    pub joint_search_trial: Option<u64>,
    /// The composed auxiliary half, when the caller constrained it.
    pub auxiliary: Option<AuxiliaryPreview>,
    /// Experimental rarity-3 raw prediction from a same-seed rarity-4 shadow.
    pub predicted_growth_grace_id: Option<u32>,
}

/// One seed-range scan.
#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub template: Vec<u8>,
    pub start_seed: u32,
    pub seed_step: u32,
    pub max_seeds: u64,
    pub level: u16,
    pub recommended_level: u16,
    pub transfer_count: u32,
    pub filters: ScanFilters,
    pub acceleration: ScanAcceleration,
}

/// The measured maps and cursors one search reuses.
///
/// Mirrors the shipped `**maps, **cursor` spread: primary maps resume by the
/// joint solver's pivot trial, a Grace-only map resumes by seed.
#[derive(Debug, Clone, Default)]
pub struct ScanAcceleration {
    pub maps: crate::maps::PreparedMaps,
    pub joint_start_after_trial: u64,
    pub grace_start_after_seed: Option<u32>,
}

/// `native.scan_next_candidate`'s argument validation, in the shipped order.
pub fn validate_scan_request(request: &ScanRequest) -> Result<(), HostError> {
    if request.seed_step == 0 {
        return Err(HostError::rejected(
            "seed_step must be between 1 and 0xFFFFFFFF",
        ));
    }
    if request.max_seeds == 0 {
        return Err(HostError::rejected("max_seeds must be positive"));
    }
    if let Some(playthrough) = request.filters.playthrough {
        if !(1..=5).contains(&playthrough) {
            return Err(HostError::rejected(
                "playthrough must be between 1 and 5, or None",
            ));
        }
    }
    if request.filters.grace_effect_id.is_none()
        && request.filters.required_slot5_effect_id.is_some()
    {
        return Err(HostError::rejected(
            "required_slot5_effect_id requires a selected special result",
        ));
    }
    Ok(())
}

/// `native.scan_next_candidate`: the branch dispatch, in the shipped order.
///
/// A primary map takes precedence over the Grace path, the accelerated Grace
/// path comes before the plain one, and the plain seed scan is the fallback.
pub fn scan_next_candidate(
    oracle: &mut dyn BatchOracle,
    request: &ScanRequest,
    auxiliary_source: Option<&mut dyn AuxiliarySource>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<Option<ScanMatch>, HostError> {
    match &request.acceleration.maps {
        crate::maps::PreparedMaps::PrimaryFirst(mapping) => {
            let mapping = mapping.clone();
            return scan_primary_candidates(
                oracle,
                request,
                &crate::maps::PrimaryMap::FirstDraw(mapping),
                true,
                auxiliary_source,
                cancelled,
                progress,
            );
        }
        crate::maps::PreparedMaps::Joint { grace, primary } => {
            let grace = grace.clone();
            let primary = primary.clone();
            let _ = grace;
            return scan_primary_candidates(
                oracle,
                request,
                &crate::maps::PrimaryMap::Grace(primary),
                false,
                auxiliary_source,
                cancelled,
                progress,
            );
        }
        crate::maps::PreparedMaps::Grace(_) => {}
        crate::maps::PreparedMaps::None => {}
    }
    if request.filters.grace_effect_id.is_some()
        && !request.acceleration.maps.is_empty()
        && matches!(request.filters.rarity, 3..=5)
    {
        return scan_grace_accelerated(oracle, request, auxiliary_source, cancelled, progress);
    }
    scan_seed_range(oracle, request, auxiliary_source, cancelled, progress)
}

/// `native._prefilter_auxiliary_items`: decide the game-closed auxiliary half
/// before any native effect call, keeping the composed value for the payload.
fn prefilter_auxiliary<I, F>(
    items: Vec<I>,
    mut seed_of_item: F,
    playthrough: Option<u32>,
    criteria: &AuxiliaryCriteria,
    source: Option<&mut dyn AuxiliarySource>,
) -> Result<(Vec<I>, std::collections::HashMap<u32, AuxiliaryPreview>), HostError>
where
    F: FnMut(&I) -> u32,
{
    if criteria.is_empty() {
        return Ok((items, std::collections::HashMap::new()));
    }
    let playthrough = playthrough.ok_or_else(|| {
        HostError::rejected("auxiliary constraints require an explicit playthrough")
    })?;
    let source = source.ok_or_else(|| {
        HostError::rejected("auxiliary constraints require the auxiliary generator")
    })?;
    let mut accepted = Vec::new();
    let mut results = std::collections::HashMap::new();
    for item in items {
        let seed = seed_of_item(&item);
        let composed = source.compose(seed, playthrough)?;
        if auxiliary_criteria_match(criteria, &composed) {
            results.insert(seed, composed);
            accepted.push(item);
        }
    }
    Ok((accepted, results))
}

/// The primary-map branches of `native.scan_next_candidate`.
///
/// The draw-1 map (`primary_first`) and the Grace-conditioned draw-2 map differ
/// only in the constraints they build, so both run one solver loop: enumerate
/// the exact intersection, prefilter the auxiliary half before any native call,
/// regenerate every proposed seed with the game, and accept the first record
/// that passes the caller's filters.
#[allow(clippy::too_many_arguments)]
fn scan_primary_candidates(
    oracle: &mut dyn BatchOracle,
    request: &ScanRequest,
    mapping: &crate::maps::PrimaryMap,
    first_draw: bool,
    mut auxiliary_source: Option<&mut dyn AuxiliarySource>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<Option<ScanMatch>, HostError> {
    validate_scan_request(request)?;
    let rarity = request.filters.rarity;
    let playthrough = request.filters.playthrough;
    let max_seeds = request.max_seeds;
    let mut constraints = Vec::new();
    if first_draw {
        // `primary_first_output_map` is only valid for the same compiled shape.
        match mapping {
            crate::maps::PrimaryMap::FirstDraw(map) => {
                if map.game_version != crate::maps::PRIMARY_MAP_GAME_VERSION
                    || u32::from(map.record_type) != u32::from(record_type_of(&request.template))
                    || map.rarity != request.filters.rarity
                    || Some(u32::from(map.category)) != playthrough
                    || map.draw_index != 1
                {
                    return Err(HostError::rejected(
                        "主词条 draw-1 映射与所选周目/稀有度不匹配",
                    ));
                }
                let runs = crate::maps::runs_for_effects(
                    &map.effects,
                    &request.filters.primary_effect_ids,
                    true,
                )?;
                constraints.push(DrawConstraint::new("primary", 1, runs)?);
            }
            crate::maps::PrimaryMap::Grace(_) => {
                return Err(HostError::rejected(
                    "the draw-1 primary path needs a draw-1 primary map",
                ))
            }
        }
    } else {
        // `primary_output_map`: rarity 5 with its Grace draw-1 already measured.
        let grace_id = request.filters.grace_effect_id.ok_or_else(|| {
            HostError::rejected("a joint primary map requires a selected special result")
        })?;
        let grace = request
            .acceleration
            .maps
            .grace()
            .cloned()
            .ok_or_else(|| HostError::rejected("a joint primary map requires the Grace map"))?;
        let mapping = match mapping {
            crate::maps::PrimaryMap::Grace(map) => map,
            crate::maps::PrimaryMap::FirstDraw(_) => {
                return Err(HostError::rejected(
                    "the joint primary path needs a Grace-conditioned primary map",
                ))
            }
        };
        let valid_context = mapping.playthrough == "current-loaded-state"
            || category_for_record_type(mapping.record_type)
                .map(|category| mapping.playthrough == format!("category-{category}-live-native"))
                .unwrap_or(false);
        if mapping.game_version != crate::maps::PRIMARY_MAP_GAME_VERSION
            || u32::from(mapping.record_type) != u32::from(record_type_of(&request.template))
            || mapping.rarity != rarity
            || grace.record_type != mapping.record_type
            || mapping.grace_effect_id != grace_id
            || mapping.grace_effect_slot != grace.effect_slot
            || mapping.draw_index != 2
            || !valid_context
        {
            return Err(HostError::rejected(
                "联立主词条映射与当前恩宠/周目原生生成上下文不匹配",
            ));
        }
        let grace_runs = crate::maps::first_u16_ranges_for_grace(grace_id, &grace)?;
        constraints.push(DrawConstraint::new(
            "grace",
            1,
            crate::maps::U16Runs::from_ranges(
                grace_runs.iter().map(|range| (range.start, range.end)),
            ),
        )?);
        let runs = crate::maps::runs_for_effects(
            &mapping.effects,
            &request.filters.primary_effect_ids,
            false,
        )?;
        constraints.push(DrawConstraint::new("primary", 2, runs)?);
    }

    let mut solver = crate::maps::ConstraintIntersection::new(
        constraints,
        request.acceleration.joint_start_after_trial,
        None,
    )?;
    let mut checked: u64 = 0;
    let mut filters = request.filters.clone();
    // The shipped primary paths decide the auxiliary half themselves, so the
    // per-record filter must not compose it a second time.
    filters.auxiliary = AuxiliaryCriteria::default();
    while checked < max_seeds {
        if cancelled() {
            return Ok(None);
        }
        let take = (oracle.max_batch_size() as u64).min(max_seeds - checked);
        let mut raw_batch = Vec::new();
        while (raw_batch.len() as u64) < take {
            match solver.next_solution() {
                Some(solution) => raw_batch.push(solution),
                None => break,
            }
        }
        if raw_batch.is_empty() {
            return Ok(None);
        }
        checked += raw_batch.len() as u64;
        let last_raw = raw_batch.last().copied();
        let (batch, auxiliary_by_seed) = {
            let auxiliary_ref: Option<&mut dyn AuxiliarySource> = match auxiliary_source.as_mut() {
                Some(source) => Some(&mut **source),
                None => None,
            };
            prefilter_auxiliary(
                raw_batch,
                |solution| solution.seed,
                playthrough,
                &request.filters.auxiliary,
                auxiliary_ref,
            )?
        };
        // The shipped progress report names the last *raw* trial of the page,
        // which is why it is captured before the auxiliary prefilter.
        let last =
            last_raw.ok_or_else(|| HostError::rejected("the joint solver yielded no trial"))?;
        if batch.is_empty() {
            progress(ScanProgress {
                scanned: checked,
                current_seed: last.seed,
                joint_trial: Some(last.pivot_trial),
            });
            continue;
        }
        let mut sources = Vec::with_capacity(batch.len());
        for solution in &batch {
            sources.push(
                nioh3_runtime::mutation::oracle::source_record(
                    &request.template,
                    solution.seed,
                    rarity,
                    request.level,
                    request.recommended_level,
                    request.transfer_count,
                )
                .map_err(HostError::from_runtime)?,
            );
        }
        let generated = oracle
            .generate(&sources, ORACLE_TIMEOUT_MS)
            .map_err(HostError::from_runtime)?;
        if generated.len() != batch.len() {
            return Err(HostError::rejected(
                "游戏原生生成器返回了错误数量的联立候选",
            ));
        }
        let finalize_native = should_finalize_native_record(rarity, playthrough);
        let finalized = if finalize_native {
            oracle
                .finalize_stage_records_batch(&generated, true, ORACLE_TIMEOUT_MS)
                .map_err(HostError::from_runtime)?
        } else {
            generated.clone()
        };
        for ((solution, stage_record), record) in
            batch.iter().zip(generated.iter()).zip(finalized.iter())
        {
            if seed_of(record) != solution.seed {
                return Err(HostError::rejected("游戏原生生成器改变了联立求解 Seed"));
            }
            let mut stage_filters = filters.clone();
            stage_filters.record_stage = Some(if finalize_native {
                RecordStage::FinalRecord
            } else {
                // The shipped scan passes `record_stage=None` here, which the
                // predicate resolves from the rarity.
                if rarity == 5 {
                    RecordStage::FinalRecord
                } else {
                    RecordStage::NativeStageOne
                }
            });
            let matched = candidate_matches_scan_filters(record, &stage_filters, None)?;
            let Some(mut matched) = matched else { continue };
            matched.joint_search_trial = Some(solution.pivot_trial);
            matched.auxiliary = auxiliary_by_seed.get(&solution.seed).cloned();
            if finalize_native {
                matched.installation_record = Some(stage_record.clone());
                matched.record = record.clone();
            }
            return Ok(Some(matched));
        }
        progress(ScanProgress {
            scanned: checked,
            current_seed: batch.last().map(|item| item.seed).unwrap_or(last.seed),
            joint_trial: Some(
                batch
                    .last()
                    .map(|item| item.pivot_trial)
                    .unwrap_or(last.pivot_trial),
            ),
        });
    }
    Ok(None)
}

/// The accelerated Grace branch of `native.scan_next_candidate`.
///
/// Rarity 4 and 5 enumerate the measured first-draw map directly and check the
/// resolved result's own slot. Rarity 3 stores a `0x0001` placeholder instead,
/// so only after it passes every real filter does the scan spend one same-seed
/// rarity-4 shadow call; a contradicted prediction is a refusal here, unlike
/// the non-accelerated path where it is a miss.
fn scan_grace_accelerated(
    oracle: &mut dyn BatchOracle,
    request: &ScanRequest,
    mut auxiliary_source: Option<&mut dyn AuxiliarySource>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<Option<ScanMatch>, HostError> {
    validate_scan_request(request)?;
    let grace_id = request
        .filters
        .grace_effect_id
        .ok_or_else(HostError::invalid_request)?;
    let rarity = request.filters.rarity;
    let playthrough = request.filters.playthrough;
    let mapping = request
        .acceleration
        .maps
        .grace()
        .cloned()
        .ok_or_else(|| HostError::rejected("the accelerated path requires the Grace map"))?;
    crate::scan::require_grace_acceleration_context(
        &request.template,
        rarity,
        playthrough,
        mapping.record_type,
        mapping.rarity,
        usize::from(mapping.effect_slot),
        true,
    )?;
    let mut cursor = crate::maps::GraceSeedCursor::new(
        grace_id,
        &mapping,
        request.acceleration.grace_start_after_seed,
    )?;
    let mut filters = request.filters.clone();
    let rarity3 = rarity == 3;
    if rarity3 {
        filters.grace_effect_id = None;
        filters.required_slot5_effect_id = Some(0x0001);
    }
    let mut scanned: u64 = 0;
    while scanned < request.max_seeds {
        if cancelled() {
            return Ok(None);
        }
        let take = (oracle.max_batch_size() as u64).min(request.max_seeds - scanned);
        let mut seed_batch = Vec::new();
        while (seed_batch.len() as u64) < take {
            match cursor.next_seed() {
                Some(seed) => seed_batch.push(seed),
                None => break,
            }
        }
        if seed_batch.is_empty() {
            return Ok(None);
        }
        let mut sources = Vec::with_capacity(seed_batch.len());
        for first_draw in &seed_batch {
            sources.push(
                nioh3_runtime::mutation::oracle::source_record(
                    &request.template,
                    first_draw.seed,
                    rarity,
                    request.level,
                    request.recommended_level,
                    request.transfer_count,
                )
                .map_err(HostError::from_runtime)?,
            );
        }
        let generated = oracle
            .generate(&sources, ORACLE_TIMEOUT_MS)
            .map_err(HostError::from_runtime)?;
        if generated.len() != seed_batch.len() {
            return Err(HostError::rejected(
                "Native batch oracle returned an unexpected record count",
            ));
        }
        if !rarity3 {
            let finalize_native = should_finalize_native_record(rarity, playthrough);
            let finalized = if finalize_native {
                oracle
                    .finalize_stage_records_batch(&generated, true, ORACLE_TIMEOUT_MS)
                    .map_err(HostError::from_runtime)?
            } else {
                generated.clone()
            };
            for ((stage_record, record), first_draw) in generated
                .iter()
                .zip(finalized.iter())
                .zip(seed_batch.iter())
            {
                if seed_of(record) != first_draw.seed {
                    return Err(HostError::rejected(format!(
                        "Native batch oracle changed an accelerated source seed: \
                         expected {:#x}, got {:#x}",
                        first_draw.seed,
                        seed_of(record)
                    )));
                }
                if finalize_native {
                    // The shipped check reads stage-one slot 5, which is the
                    // rarity-4 stage Grace; rarity 5 never finalizes here.
                    let stage_grace =
                        slot_effect_id(stage_record, 4).ok_or_else(HostError::invalid_request)?;
                    if stage_grace != grace_id {
                        return Err(HostError::rejected(format!(
                            "Native grace output contradicts the accelerated seed prediction: \
                             expected {grace_id:#x}, got {stage_grace:#x} in stage-one slot 5"
                        )));
                    }
                }
                let auxiliary_ref: Option<&mut dyn AuxiliarySource> =
                    match auxiliary_source.as_mut() {
                        Some(source) => Some(&mut **source),
                        None => None,
                    };
                let mut stage_filters = filters.clone();
                // Rarity 5 is never completed, so its generated record is
                // already final; everything else is a native stage-one result.
                stage_filters.record_stage = Some(if finalize_native || rarity == 5 {
                    RecordStage::FinalRecord
                } else {
                    RecordStage::NativeStageOne
                });
                stage_filters.strict_grace_prediction = !finalize_native;
                let matched =
                    candidate_matches_scan_filters(record, &stage_filters, auxiliary_ref)?;
                let Some(mut matched) = matched else { continue };
                if finalize_native {
                    matched.installation_record = Some(stage_record.clone());
                    matched.record = record.clone();
                }
                return Ok(Some(matched));
            }
        } else {
            for (record, first_draw) in generated.iter().zip(seed_batch.iter()) {
                if seed_of(record) != first_draw.seed {
                    return Err(HostError::rejected(format!(
                        "Native batch oracle changed an accelerated source seed: \
                         expected {:#x}, got {:#x}",
                        first_draw.seed,
                        seed_of(record)
                    )));
                }
                let auxiliary_ref: Option<&mut dyn AuxiliarySource> =
                    match auxiliary_source.as_mut() {
                        Some(source) => Some(&mut **source),
                        None => None,
                    };
                let matched = candidate_matches_scan_filters(record, &filters, auxiliary_ref)?;
                let Some(mut matched) = matched else { continue };
                let shadow = shadow_r4_grace_for_seed(
                    oracle,
                    &request.template,
                    matched.seed,
                    request.level,
                    request.recommended_level,
                    request.transfer_count,
                )?;
                if shadow != grace_id {
                    return Err(HostError::rejected(format!(
                        "Rarity-3 growing-effect prediction contradicts the same-seed \
                         rarity-4 native shadow: expected {grace_id:#x}, got {shadow:#x}"
                    )));
                }
                matched.predicted_growth_grace_id = Some(shadow);
                return Ok(Some(matched));
            }
        }
        scanned += seed_batch.len() as u64;
        progress(ScanProgress {
            scanned,
            current_seed: seed_batch
                .last()
                .map(|seed| seed.seed)
                .unwrap_or(request.start_seed),
            joint_trial: None,
        });
    }
    Ok(None)
}

fn slot_words(record: &[u8], index: usize) -> Option<(u32, u32, u32, u32, u32, u32)> {
    if record.len() != SCROLL_RECORD_SIZE || index >= EFFECT_COUNT {
        return None;
    }
    let start = EFFECT_START + index * EFFECT_STRIDE;
    let word = |offset: usize| {
        u32::from_le_bytes([
            record[start + offset],
            record[start + offset + 1],
            record[start + offset + 2],
            record[start + offset + 3],
        ])
    };
    Some((
        word(0),
        word(4),
        word(8),
        word(0x0C),
        word(0x10),
        word(0x14),
    ))
}

/// The raw effect id of one slot, or `None` when the record is not a scroll.
pub fn slot_effect_id(record: &[u8], index: usize) -> Option<u32> {
    slot_words(record, index).map(|(_, effect_id, _, _, _, _)| effect_id)
}

/// The payload-shaped effect entries of one native record.
///
/// `models.ScrollCandidate.from_record` decodes all seven slots positionally and
/// leaves `roll_percent` unset, because the native record's value and metadata
/// are the serialized truth rather than a recovered percentile.
pub fn record_effect_entries(
    record: &[u8],
) -> Result<Vec<nioh3_worker::model::CandidateEffect>, HostError> {
    let mut effects = Vec::with_capacity(EFFECT_COUNT);
    for index in 0..EFFECT_COUNT {
        let (prefix, effect_id, value, metadata, tail_0, tail_1) =
            slot_words(record, index).ok_or_else(HostError::invalid_request)?;
        effects.push(nioh3_worker::model::CandidateEffect {
            slot: (index + 1) as u32,
            effect_id,
            value: value as i32,
            metadata,
            prefix,
            tail_0,
            tail_1,
            roll_percent: None,
        });
    }
    Ok(effects)
}

/// `models.effective_required_secondary_ids`.
fn effective_required_secondary(
    primary_id: u32,
    primary_effect_ids: &BTreeSet<u32>,
    required_secondary_ids: &BTreeSet<u32>,
) -> BTreeSet<u32> {
    if required_secondary_ids.contains(&primary_id)
        && (primary_effect_ids.is_empty() || primary_effect_ids.contains(&primary_id))
    {
        return required_secondary_ids
            .iter()
            .copied()
            .filter(|id| *id != primary_id)
            .collect();
    }
    required_secondary_ids.clone()
}

/// `models.candidate_has_expected_effect_count`.
fn has_expected_effect_count(record: &[u8], rarity: u8) -> Result<bool, HostError> {
    let expected = usize::from(rarity).saturating_add(1).clamp(1, 6);
    for index in 0..expected {
        let (_, effect_id, _, _, _, _) =
            slot_words(record, index).ok_or_else(HostError::invalid_request)?;
        if effect_id == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `native._candidate_matches_scan_filters`.
pub fn candidate_matches_scan_filters(
    record: &[u8],
    filters: &ScanFilters,
    auxiliary_source: Option<&mut dyn AuxiliarySource>,
) -> Result<Option<ScanMatch>, HostError> {
    if record.len() != SCROLL_RECORD_SIZE {
        return Err(HostError::invalid_request());
    }
    if !has_expected_effect_count(record, filters.rarity)? {
        return Ok(None);
    }
    let primary_id = slot_effect_id(record, 0).ok_or_else(HostError::invalid_request)?;
    if !filters.primary_effect_ids.is_empty() && !filters.primary_effect_ids.contains(&primary_id) {
        return Ok(None);
    }
    if let Some(grace_effect_id) = filters.grace_effect_id {
        if !(1..=7).contains(&filters.grace_effect_slot) {
            return Err(HostError::rejected(
                "grace_effect_slot must be between 1 and 7",
            ));
        }
        let actual = slot_effect_id(record, filters.grace_effect_slot - 1)
            .ok_or_else(HostError::invalid_request)?;
        if actual != grace_effect_id {
            if filters.strict_grace_prediction {
                // A contradiction between an accelerated prediction and the
                // game's own output is a hard failure, not a miss.
                return Err(HostError::rejected(format!(
                    "Native grace output contradicts the accelerated seed prediction: \
                     expected {grace_effect_id:#x}, got {actual:#x} in slot {}",
                    filters.grace_effect_slot
                )));
            }
            return Ok(None);
        }
    }
    if let Some(required_slot5) = filters.required_slot5_effect_id {
        let actual = slot_effect_id(record, 4).ok_or_else(HostError::invalid_request)?;
        if actual != required_slot5 {
            return Ok(None);
        }
    }
    let effective = effective_required_secondary(
        primary_id,
        &filters.primary_effect_ids,
        &filters.required_secondary_ids,
    );
    if !effective.is_empty() || !filters.required_secondary_id_groups.is_empty() {
        let secondary_stop = if filters.required_slot5_effect_id.is_some()
            || (filters.grace_effect_slot == 5 && filters.grace_effect_id.is_some())
        {
            4
        } else {
            usize::from(filters.rarity).saturating_add(1).clamp(1, 5)
        };
        let mut secondary_ids = BTreeSet::new();
        for index in 1..secondary_stop {
            secondary_ids
                .insert(slot_effect_id(record, index).ok_or_else(HostError::invalid_request)?);
        }
        if !effective.is_subset(&secondary_ids) {
            return Ok(None);
        }
        let mut ordinary = secondary_ids.clone();
        if filters.primary_effect_ids.is_empty() {
            ordinary.insert(primary_id);
        }
        if filters
            .required_secondary_id_groups
            .iter()
            .any(|group| group.is_disjoint(&ordinary))
        {
            return Ok(None);
        }
    }
    let mut auxiliary = None;
    if !filters.auxiliary.is_empty() {
        let playthrough = filters.playthrough.ok_or_else(|| {
            HostError::rejected("auxiliary constraints require an explicit playthrough")
        })?;
        let source = auxiliary_source.ok_or_else(|| {
            HostError::rejected("auxiliary constraints require the auxiliary generator")
        })?;
        let seed = seed_of(record);
        let composed = source.compose(seed, playthrough)?;
        if !auxiliary_criteria_match(&filters.auxiliary, &composed) {
            return Ok(None);
        }
        auxiliary = Some(composed);
    }
    Ok(Some(ScanMatch {
        seed: seed_of(record),
        rarity: filters.rarity,
        playthrough: filters.playthrough,
        record_stage: filters.resolved_record_stage(),
        record: record.to_vec(),
        installation_record: None,
        joint_search_trial: None,
        auxiliary,
        predicted_growth_grace_id: None,
    }))
}

/// `record[0x20]`, the displayed seed every native result must preserve.
pub fn seed_of(record: &[u8]) -> u32 {
    if record.len() < 0x24 {
        return 0;
    }
    u32::from_le_bytes([record[0x20], record[0x21], record[0x22], record[0x23]])
}

/// `native._shadow_r4_grace_for_seed`: one same-seed rarity-4 probe whose slot-5
/// identifier predicts what a rarity-3 record's slot-5 `0x0001` will grow into.
pub fn shadow_r4_grace_for_seed(
    oracle: &mut dyn BatchOracle,
    template: &[u8],
    seed: u32,
    level: u16,
    recommended_level: u16,
    transfer_count: u32,
) -> Result<u32, HostError> {
    let source = nioh3_runtime::mutation::oracle::source_record(
        template,
        seed,
        4,
        level,
        recommended_level,
        transfer_count,
    )
    .map_err(HostError::from_runtime)?;
    let mut generated = oracle
        .generate(&[source], ORACLE_TIMEOUT_MS)
        .map_err(HostError::from_runtime)?;
    if generated.len() != 1 {
        return Err(HostError::rejected(
            "Native batch oracle returned an unexpected record count",
        ));
    }
    let shadow = generated.remove(0);
    if seed_of(&shadow) != seed {
        return Err(HostError::rejected(
            "Native batch oracle changed an experimental shadow seed",
        ));
    }
    slot_effect_id(&shadow, 4).ok_or_else(HostError::invalid_request)
}

/// `native._require_experimental_slot5_grace_context`.
pub fn require_experimental_slot5_grace_context(
    template: &[u8],
    rarity: u8,
    playthrough: Option<u32>,
) -> Result<(), HostError> {
    let record_type = record_type_of(template);
    if record_type != 0xE604 {
        return Err(HostError::rejected(
            "Experimental slot-5 grace filtering requires an E604 template",
        ));
    }
    if !matches!(rarity, 3 | 4) {
        return Err(HostError::rejected(
            "Experimental slot-5 grace filtering supports raw rarity 3 or 4",
        ));
    }
    if !matches!(playthrough, None | Some(3)) {
        return Err(HostError::rejected(
            "Experimental slot-5 Grace filtering requires category 3/E604",
        ));
    }
    Ok(())
}

/// The `category` argument's required record type.
///
/// `CATEGORY_TO_TYPE` from the shipped save layout, so a synthetic
/// template can never be mapped under the wrong category.
pub const CATEGORY_TO_TYPE: [u16; 6] = [0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523];

/// `record[0..2]`, the scroll category's record type.
pub fn record_type_of(record: &[u8]) -> u16 {
    if record.len() < 2 {
        return 0;
    }
    u16::from_le_bytes([record[0], record[1]])
}

/// `native._require_grace_acceleration_context`, given an already-loaded map.
pub fn require_grace_acceleration_context(
    template: &[u8],
    rarity: u8,
    playthrough: Option<u32>,
    map_record_type: u16,
    map_rarity: u8,
    map_effect_slot: usize,
    map_is_live: bool,
) -> Result<(), HostError> {
    let map_rarity_for = if rarity == 3 { 4 } else { rarity };
    if !matches!(map_rarity_for, 4 | 5) {
        return Err(HostError::rejected(
            "Grace-accelerated scanning currently supports rarity 3, 4, or 5",
        ));
    }
    let record_type = record_type_of(template);
    if record_type != map_record_type {
        return Err(HostError::rejected(format!(
            "Grace-accelerated scanning is verified only for record type \
             {map_record_type:#06X}; template is {record_type:#06X}"
        )));
    }
    if map_rarity != map_rarity_for {
        return Err(HostError::rejected(
            "特殊结果逆向映射的稀有度与当前生成条件不匹配",
        ));
    }
    let expected_slot = if map_rarity_for == 4 { 5 } else { 6 };
    if map_effect_slot != expected_slot {
        return Err(HostError::rejected(
            "特殊结果逆向映射的槽位与当前生成条件不匹配",
        ));
    }
    if !map_is_live && !matches!(playthrough, None | Some(3)) {
        return Err(HostError::rejected(
            "Grace inverse maps are available only for the category-3/E604 context",
        ));
    }
    if let Some(playthrough) = playthrough {
        let index = playthrough as usize;
        if index >= CATEGORY_TO_TYPE.len() || CATEGORY_TO_TYPE[index] != record_type {
            return Err(HostError::rejected(
                "特殊结果逆向映射的绘卷类型与所选周目不匹配",
            ));
        }
    }
    Ok(())
}

/// The seed-range scan.
///
/// This is the shared body of the shipped "non-accelerated Grace" and "ordinary
/// unfiltered" branches: both propose the same batches through
/// `generate_seed_range`, both run the completion pass only where the shipped
/// predicate says so, and both accept the first matching record. The Grace
/// branch additionally carries a grace filter and, for rarity 3, the same-seed
/// rarity-4 shadow validation.
pub fn scan_seed_range(
    oracle: &mut dyn BatchOracle,
    request: &ScanRequest,
    mut auxiliary_source: Option<&mut dyn AuxiliarySource>,
    cancelled: &mut dyn FnMut() -> bool,
    progress: &mut dyn FnMut(ScanProgress),
) -> Result<Option<ScanMatch>, HostError> {
    validate_scan_request(request)?;
    let grace_target = request.filters.grace_effect_id;
    if grace_target.is_some() {
        if matches!(request.filters.rarity, 3 | 4) {
            require_experimental_slot5_grace_context(
                &request.template,
                request.filters.rarity,
                request.filters.playthrough,
            )?;
        } else if request.filters.rarity == 5 {
            require_grace_acceleration_context(
                &request.template,
                5,
                request.filters.playthrough,
                0xE604,
                5,
                6,
                false,
            )?;
        } else {
            return Err(HostError::rejected(
                "Grace filtering currently supports rarity 3, 4, or 5",
            ));
        }
    }
    // The rarity-3 growing-effect path reads slot 5 rather than a Grace slot,
    // so its filter must not carry a grace target.
    let mut filters = request.filters.clone();
    let rarity3_shadow = grace_target.is_some() && filters.rarity == 3;
    if rarity3_shadow {
        filters.grace_effect_id = None;
        filters.required_slot5_effect_id = Some(0x0001);
    }

    let mut scanned: u64 = 0;
    while scanned < request.max_seeds {
        if cancelled() {
            return Ok(None);
        }
        let count = (oracle.max_batch_size() as u64)
            .min(request.max_seeds - scanned)
            .max(1) as u32;
        let batch_start = request
            .start_seed
            .wrapping_add((scanned as u32).wrapping_mul(request.seed_step));
        let source_template = nioh3_runtime::mutation::oracle::source_record(
            &request.template,
            batch_start,
            request.filters.rarity,
            request.level,
            request.recommended_level,
            request.transfer_count,
        )
        .map_err(HostError::from_runtime)?;
        let generated = oracle
            .generate_seed_range(
                &source_template,
                batch_start,
                request.seed_step,
                count,
                request.filters.playthrough,
                ORACLE_TIMEOUT_MS,
            )
            .map_err(HostError::from_runtime)?;
        if generated.len() != count as usize {
            return Err(HostError::rejected(
                "Native batch oracle returned an unexpected record count",
            ));
        }
        let finalize_native =
            should_finalize_native_record(request.filters.rarity, request.filters.playthrough);
        let finalized = if finalize_native {
            oracle
                .finalize_stage_records_batch(&generated, true, ORACLE_TIMEOUT_MS)
                .map_err(HostError::from_runtime)?
        } else {
            generated.clone()
        };
        if finalized.len() != generated.len() {
            return Err(HostError::rejected(
                "Native completion pass returned an unexpected record count",
            ));
        }
        for (stage_record, record) in generated.iter().zip(finalized.iter()) {
            if seed_of(record) != seed_of(stage_record) {
                return Err(HostError::rejected(
                    "Native batch oracle changed an accelerated source seed",
                ));
            }
            // The shipped accelerated path hard-fails when the game contradicts
            // a stage-one Grace prediction; the scan mirrors that by checking
            // the stage record before the completed one.
            if finalize_native {
                if let Some(target) = grace_target {
                    let stage_grace =
                        slot_effect_id(stage_record, 4).ok_or_else(HostError::invalid_request)?;
                    if stage_grace != target {
                        return Err(HostError::rejected(format!(
                            "Native grace output contradicts the accelerated seed prediction: \
                             expected {target:#x}, got {stage_grace:#x} in stage-one slot 5"
                        )));
                    }
                }
            }
            let mut stage_filters = filters.clone();
            stage_filters.record_stage = Some(if finalize_native {
                RecordStage::FinalRecord
            } else {
                // The shipped scan passes `record_stage=None` here, which the
                // predicate resolves from the rarity.
                if request.filters.rarity == 5 {
                    RecordStage::FinalRecord
                } else {
                    RecordStage::NativeStageOne
                }
            });
            stage_filters.strict_grace_prediction = !finalize_native;
            // Reborrow the generator for this iteration only, so the same
            // auxiliary cache serves every batch of the scan.
            let auxiliary_ref: Option<&mut dyn AuxiliarySource> = match auxiliary_source.as_mut() {
                Some(source) => Some(&mut **source),
                None => None,
            };
            let matched = candidate_matches_scan_filters(record, &stage_filters, auxiliary_ref)?;
            let Some(mut matched) = matched else { continue };
            if rarity3_shadow {
                let shadow = shadow_r4_grace_for_seed(
                    oracle,
                    &request.template,
                    matched.seed,
                    request.level,
                    request.recommended_level,
                    request.transfer_count,
                )?;
                if Some(shadow) != grace_target {
                    // The non-accelerated path treats a contradicted prediction
                    // as a miss rather than a failure.
                    continue;
                }
                matched.predicted_growth_grace_id = Some(shadow);
                return Ok(Some(matched));
            }
            if finalize_native {
                matched.installation_record = Some(stage_record.clone());
                matched.record = record.clone();
            }
            return Ok(Some(matched));
        }
        scanned += u64::from(count);
        progress(ScanProgress {
            scanned,
            current_seed: batch_start.wrapping_add((count - 1).wrapping_mul(request.seed_step)),
            joint_trial: None,
        });
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::oracle::scripted::ScriptedOracle;

    fn template(rarity: u8) -> Vec<u8> {
        let mut template = vec![0u8; SCROLL_RECORD_SIZE];
        template[0..2].copy_from_slice(&0xE604u16.to_le_bytes());
        template[0x30] = rarity;
        template
    }

    fn record_with_slots(seed: u32, rarity: u8, effects: &[(usize, u32)]) -> Vec<u8> {
        let mut record = template(rarity);
        record[0x20..0x24].copy_from_slice(&seed.to_le_bytes());
        for (index, effect_id) in effects {
            let start = EFFECT_START + index * EFFECT_STRIDE;
            record[start + 4..start + 8].copy_from_slice(&effect_id.to_le_bytes());
        }
        record
    }

    #[test]
    fn the_completion_predicate_matches_the_shipped_rule() {
        assert!(should_finalize_native_record(4, None));
        assert!(should_finalize_native_record(4, Some(1)));
        assert!(should_finalize_native_record(4, Some(3)));
        assert!(!should_finalize_native_record(4, Some(4)));
        assert!(should_finalize_native_record(3, Some(2)));
        assert!(!should_finalize_native_record(3, Some(3)));
        assert!(!should_finalize_native_record(5, Some(3)));
    }

    #[test]
    fn a_matching_record_is_accepted_and_a_short_one_is_not() {
        // Rarity 4 expects five resolved slots, so this record is acceptable.
        let full = record_with_slots(
            7,
            4,
            &[(0, 0x111), (1, 0x222), (2, 0x333), (3, 0x444), (4, 0x555)],
        );
        // The same primary with only two resolved slots is not.
        let short = record_with_slots(7, 4, &[(0, 0x111), (1, 0x222)]);
        let mut filters = ScanFilters::new(4, Some(3));
        filters.primary_effect_ids.insert(0x111);
        let matched = candidate_matches_scan_filters(&full, &filters, None)
            .expect("filter runs")
            .expect("primary and count match");
        assert_eq!(matched.seed, 7);
        assert_eq!(matched.record_stage, RecordStage::NativeStageOne);
        assert!(candidate_matches_scan_filters(&short, &filters, None)
            .expect("filter runs")
            .is_none());
        let mut wrong = filters.clone();
        wrong.primary_effect_ids.clear();
        wrong.primary_effect_ids.insert(0x999);
        assert!(candidate_matches_scan_filters(&full, &wrong, None)
            .expect("filter runs")
            .is_none());
    }

    #[test]
    fn the_ordinary_scan_publishes_the_installation_record_beside_the_completed_one() {
        let seed = 0x0000_1234;
        let stage = record_with_slots(
            seed,
            4,
            &[(0, 0x111), (1, 0x222), (2, 0x333), (3, 0x444), (4, 0x555)],
        );
        let mut completed = stage.clone();
        completed[EFFECT_START + 4 * EFFECT_STRIDE + 0x0E] = 0x04;
        completed[0x0C] = 1;
        let mut oracle = ScriptedOracle::new(
            vec![(seed, stage.clone(), completed.clone())],
            template(4),
            4,
            180,
            183,
            0,
        );
        let request = ScanRequest {
            template: template(4),
            start_seed: seed,
            seed_step: 1,
            max_seeds: 1,
            level: 180,
            recommended_level: 183,
            transfer_count: 0,
            filters: ScanFilters::new(4, Some(3)),
            acceleration: crate::scan::ScanAcceleration::default(),
        };
        let mut ticks = Vec::new();
        let matched = scan_seed_range(
            &mut oracle,
            &request,
            None,
            &mut || false,
            &mut |progress| ticks.push(progress),
        )
        .expect("scan runs")
        .expect("the scripted seed matches");
        assert_eq!(matched.seed, seed);
        assert_eq!(matched.record, completed);
        assert_eq!(
            matched.installation_record.as_deref(),
            Some(stage.as_slice())
        );
        assert_eq!(matched.record_stage, RecordStage::FinalRecord);
        // One seed range call plus one completion pass, exactly as shipped.
        assert_eq!(oracle.calls.len(), 2);
    }

    #[test]
    fn the_ordinary_scan_reports_progress_after_every_batch_and_stops_on_cancel() {
        let mut oracle = ScriptedOracle::new(vec![], template(5), 5, 180, 183, 0);
        oracle.set_max_batch_size(4);
        let request = ScanRequest {
            template: template(5),
            start_seed: 10,
            seed_step: 2,
            max_seeds: 10,
            level: 180,
            recommended_level: 183,
            transfer_count: 0,
            // Rarity 5 needs six resolved slots, so an empty script never matches.
            filters: ScanFilters::new(5, Some(3)),
            acceleration: crate::scan::ScanAcceleration::default(),
        };
        let mut ticks = Vec::new();
        let matched = scan_seed_range(
            &mut oracle,
            &request,
            None,
            &mut || false,
            &mut |progress| ticks.push(progress),
        )
        .expect("scan runs");
        assert!(matched.is_none());
        assert_eq!(ticks.len(), 3, "10 seeds in batches of 4");
        assert_eq!(ticks[0].scanned, 4);
        assert_eq!(ticks[0].current_seed, 16);
        assert_eq!(ticks[1].scanned, 8);
        assert_eq!(ticks[2].scanned, 10);
        assert_eq!(ticks[2].current_seed, 10 + 9 * 2);
        // Rarity 5 is never completed, so only the range calls are recorded.
        assert_eq!(oracle.calls.len(), 3);

        let mut oracle = ScriptedOracle::new(vec![], template(5), 5, 180, 183, 0);
        let caught = std::cell::Cell::new(false);
        let matched = scan_seed_range(
            &mut oracle,
            &request,
            None,
            &mut || {
                let seen = caught.get();
                caught.set(true);
                seen
            },
            &mut |_| {},
        )
        .expect("scan runs");
        assert!(matched.is_none());
        assert_eq!(
            oracle.calls.len(),
            1,
            "cancel is checked at the top of a batch"
        );
    }

    #[test]
    fn a_contradicted_grace_prediction_is_a_failure_when_armed_and_a_miss_otherwise() {
        let seed = 42u32;
        // Slot 5 carries 0x999, while the caller asked for 0x555.
        let record = record_with_slots(
            seed,
            4,
            &[(0, 0x111), (1, 0x222), (2, 0x333), (3, 0x444), (4, 0x999)],
        );
        let mut strict = ScanFilters::new(4, Some(3));
        strict.primary_effect_ids.insert(0x111);
        strict.grace_effect_id = Some(0x555);
        strict.grace_effect_slot = 5;
        strict.strict_grace_prediction = true;
        let failure = candidate_matches_scan_filters(&record, &strict, None)
            .expect_err("a contradiction must fail closed");
        assert!(failure
            .message
            .contains("contradicts the accelerated seed prediction"));

        strict.strict_grace_prediction = false;
        assert!(candidate_matches_scan_filters(&record, &strict, None)
            .expect("filter runs")
            .is_none());
    }
}
