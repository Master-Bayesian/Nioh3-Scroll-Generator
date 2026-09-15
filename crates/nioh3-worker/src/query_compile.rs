//! Table-derived compilation of one validated query into a native pivot query.
//!
//! This is the portable port of the configuration builders the shipped Python
//! worker uses before it calls the accelerator:
//! `auxiliary_generation._terrain_batch_configuration`,
//! `_enemy_batch_configuration`, `_special_rule_batch_configuration`,
//! `effect_sequence._ng3_r4_multi_primary_configuration`, and
//! `joint_solver.choose_pivot` / `permuted_pivot_values`.
//!
//! The compiler never reads Python and never reads a captured fixture: every
//! packed row is rebuilt from the verified product resource under the data
//! root, and every filter the caller asked for must be representable or the
//! compile fails by name. Unsupported routes are rejected explicitly rather
//! than silently widened.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use nioh3_data::{load_effect_resource, load_preview_resources};
use nioh3_domain::auxiliary::terrain_display_effect_keys_for_row;
use nioh3_domain::effect::{
    CandidatePoolRequest, EffectTableIndex, GraceMap, NativeWeightContext, EFFECT_FLAG_PROMOTED,
};
use nioh3_domain::rng::{cvtt_i32, f32_of};

use crate::collector::{
    BatchRequest, CollectorError, IntersectionReport, IntersectionStageCount, SearchBatch,
    SearchCollector, SearchFactory,
};
use crate::native_search::{
    Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, NativeSearchError, R4PrimaryPivotSpec,
};
use crate::query::SearchQuery;
use crate::search_backend::{NativePivotQuery, PageRequest, SearchBackend};

/// NG3 scroll record type (`NG3_RECORD_TYPE`).
pub const NG3_RECORD_TYPE: u16 = 0xE604;
/// The rarity whose stage-one mapping the R4 primary lookups use.
const RARITY_FINALIZABLE: u8 = 4;
/// The rarity-4 Grace map's effect slot (`_validate_rarity4_stage_mapping`).
const R4_GRACE_SLOT: u8 = 5;

const AUXILIARY_MODE_THRESHOLD_KEY: u32 = 0x1E7D;
const AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS: [u32; 3] = [0x3903, 0x779F, 0x0275];
const AUXILIARY_DESCRIPTOR_SELECTOR_KEY: u32 = 0xDA38;
const ROLE_FIVE_THRESHOLD_KEY: u32 = 0xCEFC;

const OPTIONAL_MULTIPLIER_KEY_OFFSET: usize = 0x14;
const OPTIONAL_MULTIPLIER_BASE_OFFSET: usize = 0x10;
const OPTIONAL_MULTIPLIER_SCALE_OFFSET: usize = 0x18;

/// `permuted_pivot_values`' bucket stride (`PIVOT_BUCKET_STRIDE`).
const PIVOT_BUCKET_STRIDE: u32 = 0x9E37;
/// One native chunk of the fused auxiliary route
/// (`collect_auxiliary_only_seed_page`'s `chunk_trials`).
const AUXILIARY_CHUNK_TRIALS: u64 = 8_000_000;
/// One native chunk of the R4 primary route
/// (`pivot_seed_collector_chunk_trials`).
const R4_PRIMARY_CHUNK_TRIALS: u64 = 50_000_000;

/// Which native route a compiled query uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Fused auxiliary scan: terrain, enemy, scratch and rule filters.
    Auxiliary,
    /// Rarity-4 primary-effect pivot scan.
    R4Primary,
}

/// One report stage: the declared kind and the user's values, in native order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub kind: &'static str,
    pub values: Vec<u64>,
}

/// A query compiled into exactly one native pivot query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledQuery {
    pub route: Route,
    pub native: NativePivotQuery,
    pub digest: String,
    pub playthrough: u8,
    pub rarity: u8,
    pub has_terrain_constraint: bool,
    /// Report stages in native order; `stage_counts[1..]` lines up with these.
    pub stage_specs: Vec<StageSpec>,
    pub chunk_trials: u64,
    /// The per-seed auxiliary predicate the collector must apply before it
    /// returns matches whose pivot does not already pack the caller's
    /// auxiliary criteria (the rarity-4 primary route).
    ///
    /// `None` means the route already packs them natively, so a second check
    /// would only cost time.
    pub auxiliary_predicate: Option<AuxiliaryPivotSpec>,
    /// Filters the job layer must apply to accepted candidates, because the
    /// shipped worker applies them after materialization rather than as pivot
    /// constraints. Listed so they can never be silently dropped.
    pub post_acceptance_filters: Vec<&'static str>,
}

/// Why a query cannot be compiled into a native search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The query asks for a filter this build cannot pack faithfully.
    Unsupported(String),
    /// A product table is missing, unverified or inconsistent.
    Data(String),
    /// A criterion is internally inconsistent with the shipped tables.
    Rejected(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Unsupported(message)
            | CompileError::Data(message)
            | CompileError::Rejected(message) => formatter.write_str(message),
        }
    }
}

impl CompileError {
    fn unsupported(message: impl Into<String>) -> Self {
        CompileError::Unsupported(message.into())
    }

    fn data(message: impl Into<String>) -> Self {
        CompileError::Data(message.into())
    }
}

/// Packed enemy matcher inputs (`_enemy_batch_configuration`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnemyBatch {
    pub mode_threshold: i32,
    pub descriptor_thresholds: [i32; 3],
    pub selector_threshold: i32,
    pub role_five_threshold: i32,
    pub selector_value: u8,
    pub enemy_rows: Vec<u8>,
    pub terrains: Vec<u8>,
    pub contexts: Vec<u8>,
}

/// Packed special-rule inputs (`_special_rule_batch_configuration`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialRuleBatch {
    pub scratch_groups: Vec<Vec<u32>>,
    pub rule_rows: Vec<u8>,
}

/// The verified product tables one compiler needs.
pub struct QueryCompiler {
    enemies: Vec<[u8; 28]>,
    auxiliary_contexts: Vec<[u8; 48]>,
    terrains: Vec<[u8; 52]>,
    terrain_keys: Vec<u16>,
    parameter_types: BTreeMap<u32, u32>,
    optional_multipliers: Vec<[u8; 32]>,
    rule_keys: Vec<u16>,
    rules: Vec<[u8; 56]>,
    conflict_keys: Vec<u16>,
    conflicts: Vec<[u8; 24]>,
    effect_index: EffectTableIndex,
    r4_grace: Option<GraceMap>,
}

impl QueryCompiler {
    /// Load and verify every table the compiler consumes.
    pub fn load(data_root: &Path) -> Result<Self, CompileError> {
        let preview = load_preview_resources(data_root)
            .map_err(|error| CompileError::data(format!("preview resources: {error}")))?;
        let effect_bytes = load_effect_resource(data_root)
            .map_err(|error| CompileError::data(format!("effect resource: {error}")))?;
        let effect_index = EffectTableIndex::from_resource(&effect_bytes)
            .map_err(|error| CompileError::data(format!("effect tables: {error:?}")))?;
        let r4_grace = effect_bytes
            .grace_maps
            .iter()
            .find(|map| map.rarity == RARITY_FINALIZABLE)
            .cloned();
        Ok(Self {
            enemies: preview.roster.enemies,
            auxiliary_contexts: preview.roster.contexts,
            terrains: preview.roster.terrains,
            terrain_keys: preview.roster.terrain_keys,
            parameter_types: preview.roster.parameter_types,
            optional_multipliers: preview.context.optional_multipliers,
            rule_keys: preview.rules.rule_keys,
            rules: preview.rules.rules,
            conflict_keys: preview.rules.conflict_keys,
            conflicts: preview.rules.conflicts,
            effect_index,
            r4_grace,
        })
    }

    /// Compile one validated query, or explain why it cannot be compiled.
    pub fn compile(
        &self,
        query: &SearchQuery,
        accelerator: &Accelerator,
    ) -> Result<CompiledQuery, CompileError> {
        if query.playthrough != 3 {
            return Err(CompileError::unsupported(format!(
                "offline NG3 search requires playthrough 3, not {}",
                query.playthrough
            )));
        }
        if !(3..=5).contains(&query.rarity) {
            return Err(CompileError::unsupported(format!(
                "offline NG3 search requires rarity 3, 4 or 5, not {}",
                query.rarity
            )));
        }
        if query.grace_effect_id.is_some() {
            return Err(CompileError::unsupported(
                "Grace-filtered pivots need the certified Grace run inversion, which this \
                 development worker does not compile yet",
            ));
        }
        if !query.terrain_selection_ids.is_empty() {
            return Err(CompileError::unsupported(
                "terrain option ids have no shipped reference implementation to compile against \
                 yet; this development worker rejects them instead of guessing a row mapping",
            ));
        }

        let auxiliary_only =
            !auxiliary_is_empty(&query.auxiliary) && !query_has_effect_constraints(query);
        if auxiliary_only {
            return self.compile_auxiliary(query);
        }
        if query.rarity == RARITY_FINALIZABLE && !query.primary_effect_ids.is_empty() {
            return self.compile_r4_primary(query, accelerator);
        }
        if query.rarity == 5 {
            return Err(CompileError::unsupported(
                "rarity-5 effect searches run through the effect-preimage accelerator, which this \
                 development worker does not implement yet",
            ));
        }
        // Everything left is a generic effect search the shipped worker resolves
        // without a pivot this worker can compile, and the three shapes do not
        // share one missing implementation: a rarity-3 primary search walks the
        // full seed family with its batched primary generator, a remaining
        // effect-constraint search runs the fixed-draw replay with the
        // DirectCompute effect filters, and an unconstrained sweep runs the
        // plain replay with no filter at all. Naming the route each arm misses
        // keeps a rarity-3 primary search from being reported as the rarity-5
        // effect-preimage route, which the native evidence shows is a different
        // implementation.
        if !query.primary_effect_ids.is_empty() {
            return Err(CompileError::unsupported(format!(
                "a rarity-{} primary search needs the shipped batched primary/replay route \
                 over the full seed family, which this development worker does not compile yet",
                query.rarity
            )));
        }
        if query_has_effect_constraints(query) {
            return Err(CompileError::unsupported(
                "this effect-constraint search needs the shipped DirectCompute effect route (the \
                 effect-preimage accelerator's forward filter, or its complete-composition \
                 inverse), which this development worker does not implement yet",
            ));
        }
        Err(CompileError::unsupported(format!(
            "an unconstrained rarity-{} effect search needs the shipped fixed-draw replay over \
             the full seed family, which this development worker does not compile yet",
            query.rarity
        )))
    }

    fn compile_auxiliary(&self, query: &SearchQuery) -> Result<CompiledQuery, CompileError> {
        let (spec, stage_specs, has_terrain_constraint) = self.auxiliary_packing(query)?;
        let native = NativePivotQuery::Auxiliary {
            values: self.full_family_values(),
            spec,
        };
        Ok(CompiledQuery {
            route: Route::Auxiliary,
            digest: query.digest.clone(),
            native,
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs,
            chunk_trials: AUXILIARY_CHUNK_TRIALS,
            auxiliary_predicate: None,
            post_acceptance_filters: post_acceptance_filters(query),
        })
    }

    /// Pack the caller's auxiliary criteria exactly as the fused route does.
    ///
    /// Returns the pivot spec, the report stages and whether a terrain
    /// constraint is present. The fused auxiliary scan and the per-seed
    /// predicate share this one packing path, so both consume identical rows,
    /// thresholds and criterion groups.
    fn auxiliary_packing(
        &self,
        query: &SearchQuery,
    ) -> Result<(AuxiliaryPivotSpec, Vec<StageSpec>, bool), CompileError> {
        let criteria = &query.auxiliary;
        let terrain = self.terrain_batch();
        let enemy = self.enemy_batch()?;

        let mut rule_groups: Vec<Vec<u16>> = Vec::new();
        for key in sorted_unique(&criteria.required_special_rule_keys) {
            rule_groups.push(vec![u16::try_from(key).map_err(|_| {
                CompileError::Rejected(format!("special-rule key {key} does not fit in uint16"))
            })?]);
        }
        for group in &criteria.required_special_rule_key_groups {
            let mut converted = Vec::with_capacity(group.len());
            for key in sorted_unique(group) {
                converted.push(u16::try_from(key).map_err(|_| {
                    CompileError::Rejected(format!("special-rule key {key} does not fit in uint16"))
                })?);
            }
            rule_groups.push(converted);
        }
        // The shipped route packs the rule rows and their scratch enemy groups
        // only when the request actually has a rule criterion.
        let rules = if rule_groups.is_empty() {
            SpecialRuleBatch {
                scratch_groups: Vec::new(),
                rule_rows: Vec::new(),
            }
        } else {
            self.special_rule_batch(query.playthrough)?
        };

        let has_terrain_constraint = auxiliary_has_terrain(criteria);
        let allowed_terrain_rows = self.allowed_terrain_rows(criteria, has_terrain_constraint)?;

        let mut enemy_groups: Vec<Vec<u32>> = Vec::new();
        for key in sorted_unique(&criteria.required_enemy_lookup_keys) {
            enemy_groups.push(vec![key]);
        }
        for group in &criteria.required_enemy_lookup_key_groups {
            enemy_groups.push(sorted_unique(group));
        }
        let enemy_group_count = enemy_groups.len() as u32;
        let scratch_group_count = rules.scratch_groups.len() as u32;
        enemy_groups.extend(rules.scratch_groups.iter().cloned());

        let spec = AuxiliaryPivotSpec {
            draw_index: 1,
            playthrough: query.playthrough,
            mode_threshold: enemy.mode_threshold,
            filtered_terrain_rows: terrain.filtered_rows,
            terrain_row_count: terrain.terrain_row_count,
            allowed_terrain_rows,
            has_terrain_constraint,
            descriptor_thresholds: enemy.descriptor_thresholds,
            selector_threshold: enemy.selector_threshold,
            role_five_threshold: enemy.role_five_threshold,
            selector_value: enemy.selector_value,
            enemy_rows: enemy.enemy_rows,
            terrains: enemy.terrains,
            contexts: enemy.contexts,
            enemy_criterion_groups: enemy_groups,
            enemy_group_count,
            scratch_group_count,
            rule_rows: rules.rule_rows,
            rule_criterion_groups: rule_groups,
        };
        let stage_specs = stage_specs(criteria, has_terrain_constraint);
        Ok((spec, stage_specs, has_terrain_constraint))
    }

    fn compile_r4_primary(
        &self,
        query: &SearchQuery,
        accelerator: &Accelerator,
    ) -> Result<CompiledQuery, CompileError> {
        if !query.required_secondary_ids.is_empty()
            || !query.required_secondary_id_groups.is_empty()
            || !query.minimum_roll_percent_by_effect_id.is_empty()
            || !query.grouped_rolls.is_empty()
        {
            return Err(CompileError::unsupported(
                "secondary and roll constraints are replayed by the job layer, not pivots, and \
                 this development worker does not compile that route yet",
            ));
        }
        let spec = self.r4_primary_spec(accelerator, &query.primary_effect_ids)?;
        // The rarity-4 pivot narrows on the primary effect only, so the caller's
        // auxiliary criteria are applied by the native per-seed predicate after
        // the page is collected, exactly where the shipped worker applies them.
        let auxiliary_predicate = if auxiliary_is_empty(&query.auxiliary) {
            None
        } else {
            Some(self.auxiliary_packing(query)?.0)
        };
        let native = NativePivotQuery::R4Primary {
            values: self.full_family_values(),
            spec,
        };
        Ok(CompiledQuery {
            route: Route::R4Primary,
            digest: query.digest.clone(),
            native,
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint: false,
            stage_specs: Vec::new(),
            chunk_trials: R4_PRIMARY_CHUNK_TRIALS,
            auxiliary_predicate,
            post_acceptance_filters: r4_primary_post_acceptance_filters(query),
        })
    }

    /// The permuted full seed family, i.e. `permuted_pivot_values` over
    /// `0x0000..=0xFFFF` (`choose_pivot` selects it whenever the request adds no
    /// effect constraint of its own).
    pub fn full_family_values(&self) -> Vec<u16> {
        let count = 0x1_0000u32;
        let stride = PIVOT_BUCKET_STRIDE % count;
        (0..count)
            .map(|index| (index * stride % count) as u16)
            .collect()
    }

    /// `_terrain_batch_configuration`.
    pub fn terrain_batch(&self) -> TerrainBatch {
        let filtered_rows: Vec<u32> = self
            .terrains
            .iter()
            .enumerate()
            .filter(|(_, row)| row[0x2E] & 0x02 == 0)
            .map(|(index, _)| index as u32)
            .collect();
        let mode_threshold = self
            .optional_multiplier_threshold(AUXILIARY_MODE_THRESHOLD_KEY)
            .unwrap_or(0);
        TerrainBatch {
            mode_threshold,
            filtered_rows,
            terrain_row_count: self.terrains.len() as u32,
        }
    }

    /// `_enemy_batch_configuration`.
    pub fn enemy_batch(&self) -> Result<EnemyBatch, CompileError> {
        let mut selectors = self
            .enemies
            .iter()
            .map(|row| row[0x19])
            .filter(|value| *value != 0)
            .collect::<Vec<u8>>();
        selectors.sort_unstable();
        selectors.dedup();
        if selectors.len() != 1 {
            return Err(CompileError::data(
                "native enemy matcher requires one stable descriptor selector",
            ));
        }

        let mut enemy_rows = Vec::with_capacity(self.enemies.len() * 18);
        for row in &self.enemies {
            let lookup_key = u32_at(row, 0x04);
            enemy_rows.extend_from_slice(&lookup_key.to_le_bytes());
            enemy_rows.extend_from_slice(&row[0x0C..0x10]);
            enemy_rows.extend_from_slice(&row[0x12..0x14]);
            enemy_rows.extend_from_slice(&row[0x14..0x16]);
            enemy_rows.push(row[0x16]);
            enemy_rows.push(row[0x18]);
            enemy_rows.push(row[0x19]);
            enemy_rows.push(row[0x1A]);
            enemy_rows.push(row[0x1B]);
            enemy_rows.push(
                self.parameter_types
                    .get(&lookup_key)
                    .map(|value| *value as u8)
                    .unwrap_or(0xFF),
            );
        }

        let mut terrains = Vec::with_capacity(self.terrains.len() * 5);
        for row in &self.terrains {
            terrains.extend_from_slice(&row[0x2C..0x2E]);
            terrains.extend_from_slice(&row[0x2E..0x30]);
            terrains.push(row[0x31]);
        }

        let mut contexts = Vec::with_capacity(self.auxiliary_contexts.len() * 22);
        for row in &self.auxiliary_contexts {
            contexts.push(row[0x28]);
            contexts.push(row[0x29]);
            contexts.extend_from_slice(&row[0x04..0x18]);
        }

        Ok(EnemyBatch {
            mode_threshold: self.optional_multiplier_threshold(AUXILIARY_MODE_THRESHOLD_KEY)?,
            descriptor_thresholds: [
                self.optional_multiplier_threshold(AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[0])?,
                self.optional_multiplier_threshold(AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[1])?,
                self.optional_multiplier_threshold(AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS[2])?,
            ],
            selector_threshold: self
                .optional_multiplier_threshold(AUXILIARY_DESCRIPTOR_SELECTOR_KEY)?,
            role_five_threshold: self.optional_multiplier_threshold(ROLE_FIVE_THRESHOLD_KEY)?,
            selector_value: selectors[0],
            enemy_rows,
            terrains,
            contexts,
        })
    }

    /// `_special_rule_batch_configuration`.
    pub fn special_rule_batch(&self, playthrough: u8) -> Result<SpecialRuleBatch, CompileError> {
        if !(1..=5).contains(&playthrough) {
            return Err(CompileError::Rejected(
                "playthrough must be in 1..5".to_string(),
            ));
        }
        let mut scratch_keys: Vec<u16> = self
            .enemies
            .iter()
            .map(|row| u16::from_le_bytes([row[0x12], row[0x13]]))
            .filter(|key| *key != 0xFFFF)
            .collect();
        scratch_keys.sort_unstable();
        scratch_keys.dedup();
        if scratch_keys.len() > 32 {
            return Err(CompileError::data(
                "native special-rule matcher supports at most 32 scratch keys",
            ));
        }
        let scratch_groups: Vec<Vec<u32>> = scratch_keys
            .iter()
            .map(|scratch_key| {
                self.enemies
                    .iter()
                    .filter(|row| u16::from_le_bytes([row[0x12], row[0x13]]) == *scratch_key)
                    .map(|row| u32_at(row, 0x04))
                    .collect()
            })
            .collect();

        let conflicts: BTreeMap<u16, &[u8; 24]> = self
            .conflict_keys
            .iter()
            .copied()
            .zip(self.conflicts.iter())
            .collect();
        let scratch_bit_by_key: BTreeMap<u16, u8> = scratch_keys
            .iter()
            .enumerate()
            .map(|(bit, key)| (*key, bit as u8))
            .collect();

        let mut rule_rows = Vec::with_capacity(self.rules.len() * 16);
        for (key, row) in self.rule_keys.iter().zip(self.rules.iter()) {
            let mut identities = [0xFFFFu16; 2];
            let mut active_mask = 0u8;
            for (index, group_key) in [
                u16::from_le_bytes([row[0x2C], row[0x2D]]),
                u16::from_le_bytes([row[0x2E], row[0x2F]]),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(conflict) = conflicts.get(&group_key) else {
                    continue;
                };
                identities[index] = u16::from_le_bytes([conflict[0x08], conflict[0x09]]);
                if conflict[0x0C] & 0x01 != 0 {
                    active_mask |= 1 << index;
                }
            }
            let weight = if row[0x36] & 0x01 != 0 {
                u16::from_le_bytes([
                    row[0x20 + playthrough as usize * 2],
                    row[0x21 + playthrough as usize * 2],
                ])
            } else {
                0
            };
            rule_rows.extend_from_slice(&key.to_le_bytes());
            rule_rows.extend_from_slice(&weight.to_le_bytes());
            rule_rows.extend_from_slice(&row[0x14..0x18]);
            rule_rows.extend_from_slice(&identities[0].to_le_bytes());
            rule_rows.extend_from_slice(&identities[1].to_le_bytes());
            rule_rows.push(active_mask);
            rule_rows.push(scratch_bit_by_key.get(key).copied().unwrap_or(0xFF));
            rule_rows.extend_from_slice(&0u16.to_le_bytes());
        }
        Ok(SpecialRuleBatch {
            scratch_groups,
            rule_rows,
        })
    }

    fn allowed_terrain_rows(
        &self,
        criteria: &crate::query::AuxiliaryCriteria,
        has_terrain_constraint: bool,
    ) -> Result<Vec<u8>, CompileError> {
        let mut allowed = vec![0u8; self.terrains.len()];
        for (index, slot) in allowed.iter_mut().enumerate() {
            let value = !has_terrain_constraint || self.terrain_row_matches(criteria, index)?;
            *slot = u8::from(value);
        }
        Ok(allowed)
    }

    fn terrain_row_matches(
        &self,
        criteria: &crate::query::AuxiliaryCriteria,
        row_index: usize,
    ) -> Result<bool, CompileError> {
        let actual =
            terrain_display_effect_keys_for_row(row_index, &self.terrains, &self.terrain_keys)
                .map_err(|error| {
                    CompileError::data(format!("terrain row {row_index}: {error:?}"))
                })?;
        for key in &criteria.required_terrain_effect_keys {
            if !actual.contains(&u16::try_from(*key).unwrap_or(u16::MAX)) {
                return Ok(false);
            }
        }
        for group in &criteria.required_terrain_effect_key_groups {
            let matched = group
                .iter()
                .any(|key| actual.contains(&u16::try_from(*key).unwrap_or(u16::MAX)));
            if !matched {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn optional_multiplier_threshold(&self, key: u32) -> Result<i32, CompileError> {
        let matches: Vec<&[u8; 32]> = self
            .optional_multipliers
            .iter()
            .filter(|row| u32_at(row.as_slice(), OPTIONAL_MULTIPLIER_KEY_OFFSET) == key)
            .collect();
        if matches.len() != 1 {
            return Err(CompileError::data(format!(
                "optional multiplier key 0x{key:04X} resolved to {} rows",
                matches.len()
            )));
        }
        let row = *matches[0];
        let base = i32::from_le_bytes([
            row[OPTIONAL_MULTIPLIER_BASE_OFFSET],
            row[OPTIONAL_MULTIPLIER_BASE_OFFSET + 1],
            row[OPTIONAL_MULTIPLIER_BASE_OFFSET + 2],
            row[OPTIONAL_MULTIPLIER_BASE_OFFSET + 3],
        ]);
        let scale = f32::from_le_bytes([
            row[OPTIONAL_MULTIPLIER_SCALE_OFFSET],
            row[OPTIONAL_MULTIPLIER_SCALE_OFFSET + 1],
            row[OPTIONAL_MULTIPLIER_SCALE_OFFSET + 2],
            row[OPTIONAL_MULTIPLIER_SCALE_OFFSET + 3],
        ]);
        let product = f32_of(f32_of(f64::from(base)) as f64 * f64::from(scale));
        Ok(cvtt_i32(f64::from(product)))
    }

    /// `_ng3_r4_multi_primary_configuration`.
    pub fn r4_primary_spec(
        &self,
        accelerator: &Accelerator,
        primary_effect_ids: &[u32],
    ) -> Result<R4PrimaryPivotSpec, CompileError> {
        let Some(grace) = self.r4_grace.as_ref() else {
            return Err(CompileError::data(
                "the shipped rarity-4 Grace map is missing",
            ));
        };
        if grace.effect_slot != R4_GRACE_SLOT {
            return Err(CompileError::data(format!(
                "unsupported rarity-4 stage-one mapping context: rarity {} slot {}",
                grace.rarity, grace.effect_slot
            )));
        }
        let mut special_ids: Vec<u32> = Vec::new();
        for range in &grace.ranges {
            if !special_ids.contains(&range.effect_id) {
                special_ids.push(range.effect_id);
            }
        }
        if special_ids.is_empty() || special_ids.len() > 0x100 {
            return Err(CompileError::data("R4 special context count is invalid"));
        }
        let mut context_by_first_u16 = vec![0u8; 0x1_0000];
        for range in &grace.ranges {
            let index = special_ids
                .iter()
                .position(|id| *id == range.effect_id)
                .ok_or_else(|| CompileError::data("R4 context index is inconsistent"))?;
            for value in range.start..=range.end {
                context_by_first_u16[value as usize] = index as u8;
            }
        }

        let mut normal = Vec::with_capacity(special_ids.len() * 0x1_0000);
        let mut promoted = Vec::with_capacity(special_ids.len() * 0x1_0000);
        for special_id in &special_ids {
            for is_promoted in [false, true] {
                let pool = self.primary_pool(*special_id, is_promoted)?;
                let lookup = accelerator
                    .build_weighted_effect_lookup(&pool)
                    .map_err(|error| {
                        CompileError::data(format!(
                            "native weighted lookup for special 0x{special_id:08X}: {error:?}"
                        ))
                    })?;
                let target = if is_promoted {
                    &mut promoted
                } else {
                    &mut normal
                };
                target.extend(lookup);
            }
        }

        let mut allowed = sorted_unique(primary_effect_ids);
        if allowed.is_empty() {
            return Err(CompileError::Rejected(
                "at least one R4 primary effect must be selected".to_string(),
            ));
        }
        Ok(R4PrimaryPivotSpec {
            allowed_effect_ids: std::mem::take(&mut allowed),
            context_by_first_u16,
            context_count: special_ids.len() as u32,
            normal_lookups: u32_bytes(&normal),
            promoted_lookups: u32_bytes(&promoted),
            promotion_success_lookup: promotion_success_lookup(30),
            random7_lookup: random_int_u8_lookup(7),
        })
    }

    fn primary_pool(
        &self,
        special_id: u32,
        promoted: bool,
    ) -> Result<Vec<(u32, u32)>, CompileError> {
        let request = CandidatePoolRequest {
            context: NativeWeightContext {
                record_type: NG3_RECORD_TYPE,
                rarity: RARITY_FINALIZABLE,
                playthrough: 3,
                restricted_destination_slot: false,
                extra_selector: 0,
                rarity5_type_floor: 0,
            },
            destination_category_and_flags: 0x40,
            destination_effect_flags: if promoted { EFFECT_FLAG_PROMOTED } else { 0 },
            remaining_category_capacities: self
                .effect_index
                .category_capacities(NG3_RECORD_TYPE, RARITY_FINALIZABLE)
                .map_err(|error| CompileError::data(format!("category capacities: {error:?}")))?,
            special_effect_id: Some(special_id),
            alternate_runtime_context: false,
        };
        let pool = self
            .effect_index
            .weighted_candidate_pool(&request, &[])
            .map_err(|error| {
                CompileError::data(format!(
                    "primary pool for special 0x{special_id:08X}: {error:?}"
                ))
            })?;
        if pool.is_empty() {
            return Err(CompileError::data(
                "native rarity-3/4 primary pool is empty",
            ));
        }
        Ok(pool
            .into_iter()
            .map(|candidate| (u32::from(candidate.effect_id), candidate.weight as u32))
            .collect())
    }
}

/// `_terrain_batch_configuration` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainBatch {
    pub mode_threshold: i32,
    pub filtered_rows: Vec<u32>,
    pub terrain_row_count: u32,
}

fn stage_specs(
    criteria: &crate::query::AuxiliaryCriteria,
    has_terrain_constraint: bool,
) -> Vec<StageSpec> {
    let mut specs = Vec::new();
    if has_terrain_constraint {
        let mut keys: Vec<u64> = criteria
            .required_terrain_effect_keys
            .iter()
            .chain(criteria.required_terrain_effect_key_groups.iter().flatten())
            .map(|key| u64::from(*key))
            .collect();
        keys.sort_unstable();
        keys.dedup();
        specs.push(StageSpec {
            kind: "terrain",
            values: keys,
        });
    }
    for key in sorted_unique(&criteria.required_enemy_lookup_keys) {
        specs.push(StageSpec {
            kind: "enemy",
            values: vec![u64::from(key)],
        });
    }
    for group in &criteria.required_enemy_lookup_key_groups {
        specs.push(StageSpec {
            kind: "enemy",
            values: sorted_unique(group).into_iter().map(u64::from).collect(),
        });
    }
    for key in sorted_unique(&criteria.required_special_rule_keys) {
        specs.push(StageSpec {
            kind: "rule",
            values: vec![u64::from(key)],
        });
    }
    for group in &criteria.required_special_rule_key_groups {
        specs.push(StageSpec {
            kind: "rule",
            values: sorted_unique(group).into_iter().map(u64::from).collect(),
        });
    }
    specs
}

fn sorted_unique<T: Ord + Copy>(values: &[T]) -> Vec<T> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
}

/// `AuxiliarySearchCriteria.is_empty`.
pub fn auxiliary_is_empty(criteria: &crate::query::AuxiliaryCriteria) -> bool {
    criteria.required_terrain_effect_keys.is_empty()
        && criteria.required_terrain_effect_key_groups.is_empty()
        && criteria.required_special_rule_keys.is_empty()
        && criteria.required_special_rule_key_groups.is_empty()
        && criteria.required_enemy_lookup_keys.is_empty()
        && criteria.required_enemy_lookup_key_groups.is_empty()
}

/// `_has_terrain_constraints`.
pub fn auxiliary_has_terrain(criteria: &crate::query::AuxiliaryCriteria) -> bool {
    !criteria.required_terrain_effect_keys.is_empty()
        || !criteria.required_terrain_effect_key_groups.is_empty()
}

/// Whether the query carries any effect constraint of its own.
///
/// Mirrors `search_application.request_is_auxiliary_only`'s negative: a request
/// with a primary, secondary, roll or Grace constraint is not auxiliary-only,
/// so it must not take the fused auxiliary route.
pub fn query_has_effect_constraints(query: &SearchQuery) -> bool {
    !query.primary_effect_ids.is_empty()
        || !query.required_secondary_ids.is_empty()
        || !query.required_secondary_id_groups.is_empty()
        || query.grace_effect_id.is_some()
        || !query.grace_effect_ids.is_empty()
        || !query.minimum_roll_percent_by_effect_id.is_empty()
        || !query.grouped_rolls.is_empty()
        || !query.effect_occurrences.is_empty()
}

/// Filters the shipped worker applies after materialization, in its own order.
///
/// `search_jobs` filters accepted candidates on `enemy_occurrence_groups`
/// (via `enemy_state_search`) and on explicit effect occurrences, so neither
/// may narrow the native pivot; listing them here keeps that fact visible to the
/// job layer instead of letting a criterion disappear.
pub fn post_acceptance_filters(query: &SearchQuery) -> Vec<&'static str> {
    let mut filters = Vec::new();
    if !query.enemy_occurrence_groups.is_empty() {
        filters.push("enemy_occurrence_groups");
    }
    if !query.effect_occurrences.is_empty() {
        filters.push("effect_occurrences");
    }
    if !query.initial_challenge_counts.is_empty() {
        filters.push("initial_challenge_counts");
    }
    filters
}

/// Post-acceptance filters for the rarity-4 primary route.
///
/// This route narrows the native pivot on the primary effect only, so any
/// auxiliary criterion must be verified per accepted candidate by the job
/// layer. It is listed first because dropping it returns candidates that
/// violate the user's terrain/enemy/rule request.
pub fn r4_primary_post_acceptance_filters(query: &SearchQuery) -> Vec<&'static str> {
    let mut filters = Vec::new();
    if !auxiliary_is_empty(&query.auxiliary) {
        filters.push("auxiliary_criteria");
    }
    filters.extend(post_acceptance_filters(query));
    filters
}

fn u32_at(row: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        row[offset],
        row[offset + 1],
        row[offset + 2],
        row[offset + 3],
    ])
}

fn u32_bytes(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// `game_random_int_from_u16`: two binary32 roundings then truncation.
fn game_random_int_from_u16(value: u16, count: u32) -> u32 {
    let unit = f32_of(f64::from(value) * (1.0 / 65536.0));
    let scaled = f32_of(f64::from(unit) * f64::from(f32_of(f64::from(count))));
    let result = cvtt_i32(f64::from(scaled));
    (result.max(0) as u32).min(count.saturating_sub(1))
}

/// `_random_int_u8_lookup`.
pub fn random_int_u8_lookup(count: u32) -> Vec<u8> {
    (0..0x1_0000u32)
        .map(|value| game_random_int_from_u16(value as u16, count) as u8)
        .collect()
}

/// `_promotion_success_lookup`.
pub fn promotion_success_lookup(probability_percent: u32) -> Vec<u8> {
    let threshold = probability_percent * 100;
    (0..0x1_0000u32)
        .map(|value| u8::from(game_random_int_from_u16(value as u16, 10_000) < threshold))
        .collect()
}

/// The native-search factory the job layer mounts once at startup.
///
/// Returns `None` only when the caller supplies no application root, so the
/// job layer always gets a factory; a missing accelerator or an unusable table
/// is reported per query as a typed [`CollectorError`] instead of a silent
/// fallback.
pub fn native_factory(
    application_root: &Path,
    accelerator_override: Option<&Path>,
    data_root: &Path,
) -> Option<Arc<dyn SearchFactory>> {
    Some(Arc::new(NativeSearchFactory::new(
        application_root,
        accelerator_override,
        data_root,
    )))
}

/// Compiles queries and hands the job layer a bounded native collector.
pub struct NativeSearchFactory {
    accelerator: Option<Arc<Accelerator>>,
    compiler: Result<QueryCompiler, CompileError>,
}

impl NativeSearchFactory {
    pub fn new(
        application_root: &Path,
        accelerator_override: Option<&Path>,
        data_root: &Path,
    ) -> Self {
        Self {
            accelerator: Accelerator::load(application_root, accelerator_override).map(Arc::new),
            compiler: QueryCompiler::load(data_root),
        }
    }
}

impl SearchFactory for NativeSearchFactory {
    fn collector(&self, query: &SearchQuery) -> Result<Arc<dyn SearchCollector>, CollectorError> {
        let compiler = self
            .compiler
            .as_ref()
            .map_err(|error| CollectorError::new("SEARCH_FAILED", error.to_string()))?;
        let accelerator = self.accelerator.as_ref().ok_or_else(|| {
            CollectorError::unavailable(
                "the native seed accelerator is unavailable, so bounded search cannot run",
            )
        })?;
        let compiled = compiler
            .compile(query, accelerator)
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error.to_string()))?;
        let backend = SearchBackend::from_shared(Arc::clone(accelerator));
        Ok(Arc::new(NativeCollector { backend, compiled }))
    }
}

/// The bounded collector for one compiled query.
struct NativeCollector {
    backend: SearchBackend,
    compiled: CompiledQuery,
}

impl SearchCollector for NativeCollector {
    fn collect(
        &self,
        request: &BatchRequest<'_>,
        progress: &mut dyn FnMut(&IntersectionReport),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SearchBatch, CollectorError> {
        if request.query.digest != self.compiled.digest {
            return Err(CollectorError::new(
                "SEARCH_FAILED",
                "compiled query does not match the running request",
            ));
        }
        let policy = ExecutionPolicy::from_allow_cpu_fallback(request.allow_cpu_fallback);
        let _pin = self
            .backend
            .pin_policy(policy)
            .map_err(|error| map_collector_error("pin execution policy", &error))?;

        let page_request = PageRequest::chunk(
            request.start_after_trial,
            request.max_trials_per_batch,
            self.compiled.chunk_trials,
            request.result_count,
        );
        let family_size = self.compiled.native.family_size();
        let mut latest = report_for(
            &self.compiled,
            request,
            request.start_after_trial,
            0,
            0,
            &[],
            false,
        );
        // The page budget counts matches the caller actually accepts, exactly
        // like the shipped solver: the native per-seed predicate runs inside the
        // page loop, so a page whose candidates are all rejected still covers its
        // window in one scan instead of being truncated on raw native matches.
        let page = self
            .backend
            .collect_page_filtered(
                &self.compiled.native,
                &page_request,
                cancelled,
                &mut |chunk| {
                    latest = report_for(
                        &self.compiled,
                        request,
                        chunk.inspected_through_trial,
                        chunk.matches,
                        chunk.stage_counts.first().copied().unwrap_or(0),
                        &chunk.stage_counts,
                        false,
                    );
                    progress(&latest);
                },
                self.compiled.auxiliary_predicate.as_ref(),
            )
            .map_err(|error| map_collector_error("collect page", &error))?;

        let staged = report_for(
            &self.compiled,
            request,
            page.next_cursor,
            page.matches.len(),
            page.fixed_seed_count,
            &page.stage_counts,
            page.exhausted && !page.cancelled,
        );
        if page.cancelled {
            progress(&staged);
        }
        debug_assert!(page.next_cursor <= family_size || page.exhausted);
        Ok(SearchBatch {
            matches: page.matches,
            next_start_after_trial: Some(page.next_cursor),
            intersection_report: Some(staged),
            streamed: false,
        })
    }
}

fn report_for(
    compiled: &CompiledQuery,
    request: &BatchRequest<'_>,
    inspected_through_trial: u64,
    complete_match_count: usize,
    fixed_seed_count: u64,
    stage_counts: &[u64],
    exhausted_family: bool,
) -> IntersectionReport {
    IntersectionReport {
        start_after_trial: request.start_after_trial,
        inspected_through_trial,
        family_size: compiled.native.family_size(),
        fixed_seed_count,
        stages: compiled
            .stage_specs
            .iter()
            .enumerate()
            .map(|(index, spec)| IntersectionStageCount {
                kind: spec.kind.to_string(),
                values: spec.values.clone(),
                count: stage_counts.get(index + 1).copied().unwrap_or(0),
            })
            .collect(),
        complete_match_count: complete_match_count as u64,
        exhausted_family,
    }
}

fn map_collector_error(what: &str, error: &NativeSearchError) -> CollectorError {
    match error {
        NativeSearchError::Unavailable
        | NativeSearchError::CudaUnavailable { .. }
        | NativeSearchError::PolicyRejected => CollectorError::unavailable(format!(
            "{what}: the native seed accelerator refused to run ({error:?}); CPU fallback is disabled"
        )),
        NativeSearchError::InvalidInput(message) => {
            CollectorError::new("SEARCH_FAILED", format!("{what}: {message}"))
        }
        NativeSearchError::Rejected { call } => {
            CollectorError::new("SEARCH_FAILED", format!("{what}: {call} rejected valid input"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;
    use crate::collector::CandidateSource;
    use crate::engine::Materializer;
    use crate::query::SearchQuery;

    /// The workspace root, resolved the same way the native-search tests do.
    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn combined_query(primary: u32, rules: &[u32]) -> SearchQuery {
        SearchQuery::from_payload(&json!({
            "playthrough": 3,
            "rarity": 4,
            "level": 180,
            "primary_effect_ids": [primary],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": rules,
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        }))
        .expect("the combined query shape is valid")
    }

    /// The shipped worker decides the auxiliary criteria of this route with the
    /// native per-seed matchers (`_iter_solution_prefetch`), and the job layer
    /// then re-checks each accepted candidate against the composed auxiliary
    /// output. If the two ever disagreed, a native false negative would silently
    /// drop a candidate the product returns.
    ///
    /// This drives the real R4-primary pivot over a bounded window and compares
    /// the native predicate with the composed acceptance for every match: both
    /// verdicts must be identical, and the window must contain accepted and
    /// rejected seeds so the comparison is not vacuous.
    #[test]
    fn the_native_auxiliary_predicate_agrees_with_the_composed_acceptance() {
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let compiler = QueryCompiler::load(&data_root).expect("the shipped tables load");
        let materializer = Materializer::new(&data_root, "query-compile-predicate-test");
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let backend = SearchBackend::from_shared(Arc::new(
            Accelerator::load(&root, None).expect("the shipped accelerator loads twice"),
        ));

        // One combination the product returns candidates for, and one whose
        // three rules no candidate satisfies inside the same window.
        let cases = [
            (0x774Fu32, vec![113u32], true),
            (0xAE5Au32, vec![64956, 113, 20893], false),
        ];
        for (primary, rules, expect_accepted) in cases {
            let query = combined_query(primary, &rules);
            let compiled = compiler
                .compile(&query, &accelerator)
                .expect("the combined route compiles");
            let predicate = compiled
                .auxiliary_predicate
                .as_ref()
                .expect("the combined route carries the native predicate");
            let page = backend
                .collect_page(
                    &compiled.native,
                    // A page size above the window's match count keeps the page
                    // from truncating, so every match of the window is compared
                    // instead of only the first few.
                    &PageRequest::chunk(0, 100_000, compiled.chunk_trials, 1_000_000),
                    &|| false,
                )
                .expect("a bounded R4 primary page");
            assert!(
                !page.matches.is_empty(),
                "primary 0x{primary:04X} must match inside the window"
            );
            let seeds: Vec<u32> = page.matches.iter().map(|matched| matched.seed).collect();
            let selected = backend
                .auxiliary_criteria_selected(predicate, &seeds)
                .expect("the native predicate runs");
            assert_eq!(selected.len(), page.matches.len());

            let mut accepted = 0usize;
            for (matched, keep) in page.matches.iter().zip(&selected) {
                let materialized = materializer
                    .materialize(&query, matched.seed, matched.trial)
                    .expect("every R4 primary match composes");
                assert_eq!(
                    *keep,
                    materialized.auxiliary_match == Some(true),
                    "primary 0x{primary:04X} seed {} trial {}",
                    matched.seed,
                    matched.trial
                );
                accepted += usize::from(*keep);
            }
            if expect_accepted {
                assert!(
                    accepted > 0,
                    "primary 0x{primary:04X} must accept at least one match in the window"
                );
            } else {
                assert_eq!(
                    accepted, 0,
                    "primary 0x{primary:04X} must reject every match in the window"
                );
            }
        }
    }
}
