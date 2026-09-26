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

use nioh3_data::{
    load_effect_resource, load_effect_resource_for_file_version, load_preview_resources,
    load_preview_resources_for_file_version,
};
use nioh3_domain::auxiliary::terrain_display_effect_keys_for_row;
use nioh3_domain::effect::{
    CandidatePoolRequest, EffectTableIndex, GraceMap, NativeWeightContext, EFFECT_FLAG_PROMOTED,
};
use nioh3_domain::rng::{cvtt_i32, f32_of};
use nioh3_domain::sequence::RARITY5_GRACE_SLOT;

use crate::collector::{
    BatchRequest, CollectorError, IntersectionReport, IntersectionStageCount, SearchBatch,
    SearchCollector, SearchFactory,
};
use crate::effect_batch::{EffectMaskSpec, PartialEffectVerifier};
use crate::effect_path::{
    compile_full_composition_plans, first_u16_ranges_for_grace, CompiledEffectPlan,
    FullCompositionRequest, PreimageVerifier,
};
use crate::grace_map::CATEGORY_TO_TYPE;
use crate::native_search::{
    Accelerator, AuxiliaryPivotSpec, ExecutionPolicy, NativeSearchError, PrimaryEffectSpec,
    R4PrimaryPivotSpec,
};
use crate::preimage::{PreimagePolicy, MAX_PREIMAGE_TRIALS};
use crate::query::SearchQuery;
use crate::search_backend::{
    MatchFilter, NativePivotQuery, PageRequest, PrimaryPivotFamilySpec, SearchBackend,
};

/// NG3 scroll record type (`NG3_RECORD_TYPE`).
pub const NG3_RECORD_TYPE: u16 = 0xE604;
/// The rarity whose stage-one mapping the R4 primary lookups use.
const RARITY_FINALIZABLE: u8 = 4;
/// The rarity whose primary lottery is rolled from the full seed family.
const RARITY_GROWING: u8 = 3;
/// The rarity whose complete composition terminates in a Grace (`RARITY_DIVINE`).
const RARITY_DIVINE: u8 = 5;
/// The single special group a rarity-3 primary lottery is conditioned on.
const RARITY3_PRIMARY_SPECIAL_ID: u32 = 0x0001;
/// `_promotion_success_lookup(10)` for the rarity-3 primary lottery.
const RARITY3_PRIMARY_PROMOTION_PERCENT: u32 = 10;
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
/// One native chunk of the full-family replay (`MAX_NATURAL_TRIALS`).
const FULL_FAMILY_CHUNK_TRIALS: u64 = 1_000_000;

/// Which native route a compiled query uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Fused auxiliary scan: terrain, enemy, scratch and rule filters.
    Auxiliary,
    /// Rarity-4 primary-effect pivot scan.
    R4Primary,
    /// Complete-composition effect-preimage sweep over compiled plan families.
    CompletePreimage,
    /// One-wildcard effect-preimage sweep: the requested ordinary effects must be
    /// present and the fifth ordinary slot is free.
    OneWildcardPreimage,
    /// Plain natural pivot over the full family with the shipped batched
    /// predicates: a rarity-3 primary search or an unconstrained search.
    FullFamily,
    /// NG3 rarity-3 named-primary pivot families: the shipped finite cursor
    /// space of the legal promotion outcomes, with the partial-effect
    /// acceptance deciding the caller's own criteria.
    R3PrimaryPivot,
    /// Partial ordinary-effect search: the full seed family swept with the
    /// accelerator's batched constraint mask, then certified per Seed by the
    /// ported forward composition (`partial_effect_batch_generator`).
    PartialEffectFilter,
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
    /// The shipped predicates the page must apply before a raw match counts,
    /// for routes whose pivot does not already pack the caller's criteria.
    ///
    /// `None` means the route already decides them natively, so a second check
    /// would only cost time.
    pub page_filter: Option<PageFilter>,
    /// Filters the job layer must apply to accepted candidates, because the
    /// shipped worker applies them after materialization rather than as pivot
    /// constraints. Listed so they can never be silently dropped.
    pub post_acceptance_filters: Vec<&'static str>,
}

/// The shipped predicates one page applies before a raw match counts.
///
/// Mirrors the shipped prefetch order: the batched primary-effect predicate
/// first (rarity-3 primary), then the auxiliary criteria for its survivors.
///
/// The partial-effect route adds the accelerator's batched constraint mask and
/// the certified recomposition that decides the request's own effect criteria;
/// both stay `None` on every route that packs its criteria natively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFilter {
    pub primary: Option<PrimaryEffectSpec>,
    pub auxiliary: Option<AuxiliaryPivotSpec>,
    /// `match_effect_constraints_d3d11` for a partial ordinary-effect request.
    pub effect_mask: Option<Arc<EffectMaskSpec>>,
    /// The certified composition gate for that same request.
    pub effect_verifier: Option<Arc<PartialEffectVerifier>>,
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

/// Map one plan-compile failure onto the error the query compiler reports.
fn map_effect_path_compile_error(error: crate::effect_path::EffectPathError) -> CompileError {
    match error {
        crate::effect_path::EffectPathError::Unsupported(message) => {
            CompileError::unsupported(message)
        }
        crate::effect_path::EffectPathError::Rejected(message) => CompileError::Rejected(message),
        crate::effect_path::EffectPathError::Data(message) => CompileError::data(message),
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
    /// Shared with the effect-preimage verifier, which re-composes every
    /// accelerator hit on the same verified tables.
    effect_index: Arc<EffectTableIndex>,
    r4_grace: Option<GraceMap>,
    r5_grace: Option<GraceMap>,
    /// Set only on the derived compiler a save-bound cached job uses: the
    /// playthrough whose registered Grace map replaced the bundled rarity-5 map.
    cached_playthrough: Option<u8>,
}

impl QueryCompiler {
    /// Load and verify every table the compiler consumes, from the shipped
    /// legacy v2.00.02 payload.
    ///
    /// Kept for the historical fixtures; a production worker compiles with
    /// [`QueryCompiler::load_for_resource_version`] so its searches read the
    /// same tables its candidates are composed from.
    pub fn load(data_root: &Path) -> Result<Self, CompileError> {
        Self::load_for_resource_version(data_root, None)
    }

    /// Load the tables bound to one exact executable version.
    ///
    /// `resource_version` is the value the worker hands its `Materializer`, so
    /// the compiler, the page filters and the composed candidates all consume
    /// one resource bundle. `None` selects the shipped legacy payload for both
    /// halves, exactly like `Materializer::with_resource_version(.., None)`;
    /// an unregistered version fails closed and never falls back to it.
    pub fn load_for_resource_version(
        data_root: &Path,
        resource_version: Option<(u16, u16, u16, u16)>,
    ) -> Result<Self, CompileError> {
        let preview = match resource_version {
            Some(version) => load_preview_resources_for_file_version(data_root, version),
            None => load_preview_resources(data_root),
        }
        .map_err(|error| CompileError::data(format!("preview resources: {error}")))?;
        let effect_bytes = match resource_version {
            Some(version) => load_effect_resource_for_file_version(data_root, version),
            None => load_effect_resource(data_root),
        }
        .map_err(|error| CompileError::data(format!("effect resource: {error}")))?;
        let effect_index = EffectTableIndex::from_resource(&effect_bytes)
            .map_err(|error| CompileError::data(format!("effect tables: {error:?}")))?;
        let r4_grace = effect_bytes
            .grace_maps
            .iter()
            .find(|map| map.rarity == RARITY_FINALIZABLE)
            .cloned();
        let r5_grace = effect_bytes
            .grace_maps
            .iter()
            .find(|map| map.rarity == RARITY_DIVINE)
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
            effect_index: Arc::new(effect_index),
            r4_grace,
            r5_grace,
            cached_playthrough: None,
        })
    }

    /// Bind a derived compiler to one registered NG4/NG5 Grace map.
    ///
    /// The save-bound route compiles against the measured map of the player's
    /// own playthrough instead of the bundled NG3 map, and only rarity 5 with
    /// that playthrough's record type and Grace slot is accepted. The returned
    /// compiler carries the same verified tables; only the rarity-5 map and the
    /// playthrough gate differ.
    pub fn for_cached_grace(
        &self,
        playthrough: u8,
        map: GraceMap,
        record_type: u16,
        effect_slot: u8,
    ) -> Result<Self, CompileError> {
        let expected_record_type = CATEGORY_TO_TYPE
            .get(usize::from(playthrough))
            .copied()
            .filter(|_| matches!(playthrough, 4 | 5))
            .ok_or_else(|| {
                CompileError::unsupported(format!(
                    "the save-bound cache route serves NG4 and NG5, not playthrough {playthrough}"
                ))
            })?;
        if record_type != expected_record_type {
            return Err(CompileError::Rejected(format!(
                "the registered map belongs to record type 0x{record_type:04X}, not 0x{expected_record_type:04X}"
            )));
        }
        if effect_slot != RARITY5_GRACE_SLOT {
            return Err(CompileError::Rejected(format!(
                "the registered map reports Grace slot {effect_slot}, not {RARITY5_GRACE_SLOT}"
            )));
        }
        if map.rarity != RARITY_DIVINE || map.effect_slot != effect_slot {
            return Err(CompileError::Rejected(
                "the registered map is not a rarity-5 Grace map".to_string(),
            ));
        }
        if map.record_type != u32::from(record_type) {
            return Err(CompileError::Rejected(format!(
                "the registered map record type 0x{:04X} does not match playthrough {playthrough}",
                map.record_type
            )));
        }
        Ok(Self {
            enemies: self.enemies.clone(),
            auxiliary_contexts: self.auxiliary_contexts.clone(),
            terrains: self.terrains.clone(),
            terrain_keys: self.terrain_keys.clone(),
            parameter_types: self.parameter_types.clone(),
            optional_multipliers: self.optional_multipliers.clone(),
            rule_keys: self.rule_keys.clone(),
            rules: self.rules.clone(),
            conflict_keys: self.conflict_keys.clone(),
            conflicts: self.conflicts.clone(),
            effect_index: Arc::clone(&self.effect_index),
            r4_grace: self.r4_grace.clone(),
            r5_grace: Some(map),
            cached_playthrough: Some(playthrough),
        })
    }

    /// The verified effect tables every route composes and filters against.
    pub fn effect_index(&self) -> &EffectTableIndex {
        &self.effect_index
    }

    /// The same tables as a shared handle, for the certified verifiers.
    pub fn effect_index_arc(&self) -> Arc<EffectTableIndex> {
        Arc::clone(&self.effect_index)
    }

    /// The captured rarity-4 draw-1 Grace map, when the resource carries it.
    pub fn r4_grace(&self) -> Option<&GraceMap> {
        self.r4_grace.as_ref()
    }

    /// The captured rarity-5 draw-1 Grace map, when the resource carries it.
    pub fn r5_grace(&self) -> Option<&GraceMap> {
        self.r5_grace.as_ref()
    }

    /// Compile one validated query, or explain why it cannot be compiled.
    ///
    /// `effect_preimage_available` is the real probe of the verified
    /// effect-preimage helper (`d3d11_effect_acceleration_available`). Routes
    /// the shipped worker gates on that probe use it the same way, so a machine
    /// without a DirectCompute backend keeps the shipped fallback instead of
    /// being promised a route it cannot run.
    pub fn compile(
        &self,
        query: &SearchQuery,
        accelerator: &Accelerator,
        effect_preimage_available: bool,
    ) -> Result<CompiledQuery, CompileError> {
        match self.cached_playthrough {
            // A cache-bound compiler serves exactly the playthrough its
            // registered map belongs to.
            Some(bound) if query.playthrough != bound => {
                return Err(CompileError::Rejected(format!(
                    "this cached search is bound to playthrough {bound}, not {}",
                    query.playthrough
                )))
            }
            None if query.playthrough != 3 => {
                return Err(CompileError::unsupported(format!(
                    "offline NG3 search requires playthrough 3, not {}; NG4/NG5 needs its \
                     save-bound rarity-5 cache",
                    query.playthrough
                )))
            }
            _ => {}
        }
        if !(3..=5).contains(&query.rarity) {
            return Err(CompileError::unsupported(format!(
                "offline NG3 search requires rarity 3, 4 or 5, not {}",
                query.rarity
            )));
        }
        // A selected Grace is a draw-1 pivot constraint for every route that
        // serves it: the rarity-5 preimage routes build it in
        // `compile_complete_preimage` / `compile_one_wildcard_preimage`, and the
        // partial-effect route builds the same run inversion in
        // `compile_partial_effect_filter`. Rarities without a captured map are
        // refused by name there.
        if query.grace_effect_id.is_some()
            && !matches!(query.rarity, RARITY_FINALIZABLE | RARITY_DIVINE)
        {
            return Err(CompileError::unsupported(
                "Grace choices do not belong to this rarity",
            ));
        }
        // Terrain option ids arrive resolved: the job layer turned them into
        // `auxiliary.terrain_row_indices` with the shared `crate::terrain`
        // resolver. A selection that was never resolved must not be dropped.
        if !query.terrain_selection_ids.is_empty() && query.auxiliary.terrain_row_indices.is_empty()
        {
            return Err(CompileError::Rejected(
                "terrain options were not resolved against the context-bound terrain table"
                    .to_string(),
            ));
        }

        let auxiliary_only =
            !auxiliary_is_empty(&query.auxiliary) && !query_has_effect_constraints(query);
        if auxiliary_only {
            return self.compile_auxiliary(query);
        }
        // Shipped order: the complete-composition preimage is tried before the
        // one-wildcard family and before the fixed-draw replay, so a request
        // that names every ordinary slot runs the GPU inverse instead of
        // sweeping the whole seed family.
        if self.complete_preimage_eligible(query) {
            return self.compile_complete_preimage(query);
        }
        if self.one_wildcard_eligible(query) {
            return self.compile_one_wildcard_preimage(query);
        }
        // The shipped NG3 rarity-3 named-primary pivot is tried next: a
        // primary that is drawable only before or only behind the promotion
        // shuffle compiles into a finite family set, and the shipped
        // partial-effect acceptance then decides the caller's own criteria.
        // The method returns `None` for every other shape and for every
        // fallback the shipped guard keeps, so the arms below stay unchanged.
        if let Some(compiled) = self.compile_primary_pivot(query, effect_preimage_available)? {
            return Ok(compiled);
        }
        // The rarity-4 primary pivot serves a primary-only request; a request
        // that also names secondaries or rolls is the shipped partial-effect
        // fixed-draw replay instead (the rarity-4 finalizer-aware forward
        // filter), so it falls through to the partial arm below.
        if query.rarity == RARITY_FINALIZABLE
            && !query.primary_effect_ids.is_empty()
            && query.required_secondary_ids.is_empty()
            && query.required_secondary_id_groups.is_empty()
            && query.minimum_roll_percent_by_effect_id.is_empty()
            && query.grouped_rolls.is_empty()
        {
            return self.compile_r4_primary(query, accelerator);
        }
        // Rarity-3 primary and unconstrained searches are the shipped fixed-draw
        // replay over the full seed family: the same family the plain natural
        // pivot walks, with the caller's criteria decided by the shipped batched
        // predicates before anything is composed. A rarity-3 primary search that
        // also names secondaries, rolls or occurrences is the partial-effect
        // forward filter instead, so it falls through to that arm.
        if query.rarity == RARITY_GROWING
            && !query.primary_effect_ids.is_empty()
            && query.required_secondary_ids.is_empty()
            && query.required_secondary_id_groups.is_empty()
            && query.minimum_roll_percent_by_effect_id.is_empty()
            && query.grouped_rolls.is_empty()
            && query.effect_occurrences.is_empty()
        {
            return self.compile_full_family(query, accelerator);
        }
        // An unconstrained search - or one whose only criteria are job-layer
        // filters such as a challenge count - is the shipped fixed-draw replay
        // over the full seed family at every rarity. The rarity-5 record and its
        // Grace are composed per accepted Seed from the context-bound map (or the
        // registered save-bound map on the cached NG4/NG5 route).
        if !query_has_effect_constraints(query) {
            return self.compile_full_family(query, accelerator);
        }
        // Everything left names at least one ordinary-effect criterion, so it is
        // the shipped partial-effect fixed-draw replay: the full seed family,
        // the accelerator's batched constraint mask when ordinary slots are
        // named, and the certified recomposition of every survivor.
        if query_has_effect_constraints(query) {
            return self.compile_partial_effect_filter(query, accelerator);
        }
        // The arms above cover every shape this worker compiles; anything that
        // reaches here is refused by the arm that owns its missing route rather
        // than silently widened into another route.
        Err(CompileError::unsupported(format!(
            "this rarity-{} search does not match any route this development worker compiles",
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
            page_filter: None,
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

    /// Whether the shipped complete-composition preimage route serves this
    /// query (`_complete_preimage_requests` plus this worker's own gates).
    ///
    /// The shipped dispatch tries this route first whenever every ordinary slot
    /// is named, so a rarity-3 request with a primary and three secondaries is
    /// inverted on the GPU instead of sweeping the whole seed family.
    fn complete_preimage_eligible(&self, query: &SearchQuery) -> bool {
        if !matches!(query.rarity, RARITY_GROWING | RARITY_DIVINE)
            || !query.required_secondary_id_groups.is_empty()
        {
            return false;
        }
        // A rarity-5 complete composition is only defined with its selected
        // Grace; without one the shipped layer leaves this route unclaimed.
        if query.rarity == RARITY_DIVINE && query.grace_effect_id.is_none() {
            return false;
        }
        let expected_ordinary = expected_ordinary_count(query.rarity);
        if query.primary_effect_ids.is_empty()
            && query.required_secondary_ids.len() != expected_ordinary
        {
            return false;
        }
        self.complete_preimage_requests(query).is_some()
    }

    /// Every exact-primary request for one complete ordinary set.
    ///
    /// Returns `None` when the shape is not a complete composition. Roll
    /// percentages, secondary any-of groups and explicit effect occurrences are
    /// replayed per candidate by the shipped job layer, which this worker does
    /// not implement yet, so a query carrying them is not claimed by this route
    /// and falls through to a route that names its own missing verification.
    fn complete_preimage_requests(
        &self,
        query: &SearchQuery,
    ) -> Option<Vec<FullCompositionRequest>> {
        // The shipped complete-composition route is only defined for a fully
        // named ordinary set, so secondary any-of groups make it ineligible.
        // Plain roll minimums are enforced by this route's own acceptance and
        // occurrences by the job layer, so neither makes it ineligible.
        if !query.required_secondary_id_groups.is_empty() {
            return None;
        }
        let expected_ordinary = expected_ordinary_count(query.rarity);
        let primary_options: Vec<u32> = if !query.primary_effect_ids.is_empty() {
            sorted_unique(&query.primary_effect_ids)
        } else if query.required_secondary_ids.len() == expected_ordinary {
            sorted_unique(&query.required_secondary_ids)
        } else {
            return None;
        };
        let mut requests: Vec<FullCompositionRequest> = Vec::new();
        for primary_effect_id in primary_options {
            let mut secondaries: Vec<u32> = sorted_unique(&query.required_secondary_ids);
            secondaries.retain(|effect_id| *effect_id != primary_effect_id);
            if secondaries.len() != expected_ordinary - 1 {
                continue;
            }
            let request = FullCompositionRequest {
                rarity: query.rarity,
                primary_effect_id,
                secondary_effect_ids: secondaries,
                stage_special_effect_id: query.grace_effect_id,
                natural_only: true,
                playthrough: query.playthrough,
            };
            if request.validate().is_err() {
                continue;
            }
            if !requests.contains(&request) {
                requests.push(request);
            }
        }
        if requests.is_empty() {
            None
        } else {
            Some(requests)
        }
    }

    /// Whether the shipped one-wildcard route serves this query.
    ///
    /// `_one_wildcard_preimage_request`: rarity 5, no primary restriction, no
    /// secondary any-of groups, its Grace selected, and exactly four required
    /// ordinary effects (the fifth ordinary slot is the wildcard).
    fn one_wildcard_eligible(&self, query: &SearchQuery) -> bool {
        query.rarity == RARITY_DIVINE
            && query.primary_effect_ids.is_empty()
            && query.required_secondary_id_groups.is_empty()
            && query.grace_effect_id.is_some()
            && query.required_secondary_ids.len() == expected_ordinary_count(query.rarity) - 1
    }

    /// Compile the one-wildcard route.
    ///
    /// The plan family is the same shape as the complete-composition route; the
    /// difference is that acceptance requires the requested ordinary effects to
    /// be present rather than forming one exact set, which is what
    /// `verify_one_wildcard_matches` does.
    fn compile_one_wildcard_preimage(
        &self,
        query: &SearchQuery,
    ) -> Result<CompiledQuery, CompileError> {
        let grace = query.grace_effect_id.ok_or_else(|| {
            CompileError::Rejected("a rarity-5 one-wildcard search requires its Grace".to_string())
        })?;
        let map = self.r5_grace.as_ref().ok_or_else(|| {
            CompileError::data("the rarity-5 Grace map is not present in the product resource")
        })?;
        let special_runs = first_u16_ranges_for_grace(grace, map).map_err(|error| {
            CompileError::Rejected(format!(
                "rarity-5 Grace 0x{grace:04X} has no draw-1 preimage: {error}"
            ))
        })?;
        let request = crate::effect_path::OneWildcardCompositionRequest {
            rarity: query.rarity,
            required_effect_ids: sorted_unique(&query.required_secondary_ids),
            stage_special_effect_id: Some(grace),
            natural_only: true,
            playthrough: query.playthrough,
        };
        let plans = crate::effect_path::compile_one_wildcard_composition_plans(
            &request,
            &self.effect_index,
            &special_runs,
        )
        .map_err(|error| {
            CompileError::Rejected(format!(
                "the one-wildcard rarity-5 composition has no legal native path: {error}"
            ))
        })?;
        let verifier = PreimageVerifier::build(crate::effect_path::PreimageVerifierSpec {
            tables: Arc::clone(&self.effect_index),
            rarity: query.rarity,
            accepted: Vec::new(),
            stage_special_effect_id: Some(grace),
            grace_map: Some(map.clone()),
            playthrough: query.playthrough,
            level: query.level,
            natural_only: true,
            minimum_rolls: query.minimum_roll_percent_by_effect_id.clone(),
            wildcard_required: Some(request.required_effect_ids.clone()),
        });
        let page_filter = PageFilter {
            primary: None,
            auxiliary: if auxiliary_is_empty(&query.auxiliary) {
                None
            } else {
                Some(self.auxiliary_packing(query)?.0)
            },
            effect_mask: None,
            effect_verifier: None,
        };
        let has_terrain_constraint = page_filter
            .auxiliary
            .as_ref()
            .is_some_and(|spec| spec.has_terrain_constraint);
        Ok(CompiledQuery {
            route: Route::OneWildcardPreimage,
            digest: query.digest.clone(),
            native: NativePivotQuery::EffectPreimage {
                plans: Arc::new(plans),
                verifier: Arc::new(verifier),
            },
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials: MAX_PREIMAGE_TRIALS,
            page_filter: Some(page_filter),
            post_acceptance_filters: post_acceptance_filters(query),
        })
    }

    /// Compile the complete-composition preimage route.
    ///
    /// Every request's plan families are concatenated in request order, which is
    /// exactly the cursor layout the shipped page walks: plan offsets accumulate
    /// `pivot_state_count`, and the accelerator's own local trial becomes the
    /// one-based cursor `plan_offset + local trial + 1`.
    fn compile_complete_preimage(
        &self,
        query: &SearchQuery,
    ) -> Result<CompiledQuery, CompileError> {
        if !matches!(query.rarity, RARITY_GROWING | RARITY_DIVINE) {
            return Err(CompileError::unsupported(format!(
                "the complete-composition preimage route is compiled for rarities 3 and 5; rarity \
                 {} needs the captured draw-1 Grace preimage this worker does not have",
                query.rarity
            )));
        }
        // Rarity 5 terminates in the selected Grace, so its draw-1 preimage is
        // the route's shared constraint and has to come from the captured map.
        let special_runs = if query.rarity == RARITY_DIVINE {
            let grace = query.grace_effect_id.ok_or_else(|| {
                CompileError::Rejected(
                    "a rarity-5 complete composition requires its selected Grace".to_string(),
                )
            })?;
            let map = self.r5_grace.as_ref().ok_or_else(|| {
                CompileError::data("the rarity-5 Grace map is not present in the product resource")
            })?;
            first_u16_ranges_for_grace(grace, map).map_err(|error| {
                CompileError::Rejected(format!(
                    "rarity-5 Grace 0x{grace:04X} has no draw-1 preimage: {error}"
                ))
            })?
        } else {
            Vec::new()
        };
        let requests = self.complete_preimage_requests(query).ok_or_else(|| {
            CompileError::Rejected(
                "a complete composition needs a primary and every remaining distinct secondary"
                    .to_string(),
            )
        })?;
        let mut plans: Vec<CompiledEffectPlan> = Vec::new();
        let mut accepted: Vec<(u32, Vec<u32>)> = Vec::new();
        for request in &requests {
            let request_plans =
                compile_full_composition_plans(request, &self.effect_index, &special_runs)
                    .map_err(|error| {
                        CompileError::Rejected(format!(
                            "the complete rarity-{} composition has no legal native path: {error}",
                            query.rarity
                        ))
                    })?;
            plans.extend(request_plans);
            accepted.push((
                request.primary_effect_id,
                request.secondary_effect_ids.clone(),
            ));
        }
        if plans.is_empty() {
            return Err(CompileError::Rejected(
                "the complete composition compiled no plan family".to_string(),
            ));
        }
        let verifier = PreimageVerifier::build(crate::effect_path::PreimageVerifierSpec {
            tables: Arc::clone(&self.effect_index),
            rarity: query.rarity,
            accepted,
            stage_special_effect_id: query.grace_effect_id,
            grace_map: if query.rarity == RARITY_DIVINE {
                self.r5_grace.clone()
            } else {
                None
            },
            playthrough: query.playthrough,
            level: query.level,
            natural_only: true,
            minimum_rolls: query.minimum_roll_percent_by_effect_id.clone(),
            wildcard_required: None,
        });
        let page_filter = PageFilter {
            primary: None,
            auxiliary: if auxiliary_is_empty(&query.auxiliary) {
                None
            } else {
                Some(self.auxiliary_packing(query)?.0)
            },
            effect_mask: None,
            effect_verifier: None,
        };
        let has_terrain_constraint = page_filter
            .auxiliary
            .as_ref()
            .is_some_and(|spec| spec.has_terrain_constraint);
        Ok(CompiledQuery {
            route: Route::CompletePreimage,
            digest: query.digest.clone(),
            native: NativePivotQuery::EffectPreimage {
                plans: Arc::new(plans),
                verifier: Arc::new(verifier),
            },
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials: MAX_PREIMAGE_TRIALS,
            page_filter: Some(page_filter),
            post_acceptance_filters: post_acceptance_filters(query),
        })
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
        let page_filter = PageFilter {
            primary: None,
            auxiliary: if auxiliary_is_empty(&query.auxiliary) {
                None
            } else {
                Some(self.auxiliary_packing(query)?.0)
            },
            effect_mask: None,
            effect_verifier: None,
        };
        let native = NativePivotQuery::R4Primary {
            values: self.full_family_values(),
            spec,
        };
        let has_terrain_constraint = page_filter
            .auxiliary
            .as_ref()
            .is_some_and(|spec| spec.has_terrain_constraint);
        Ok(CompiledQuery {
            route: Route::R4Primary,
            digest: query.digest.clone(),
            native,
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials: R4_PRIMARY_CHUNK_TRIALS,
            page_filter: Some(page_filter),
            post_acceptance_filters: r4_primary_post_acceptance_filters(query),
        })
    }

    /// The shipped fixed-draw replay over the full seed family.
    ///
    /// `fixed_draw_constraints` reduces a rarity-3 primary search and an
    /// unconstrained search to the same `seed_space` constraint at draw 1, so
    /// both walk the family the plain natural pivot enumerates. The criteria are
    /// decided afterwards by the shipped batched predicates
    /// (`effect_sequence.generate_ng3_rarity34_primary_effect_ids` for the
    /// primary ids, then the auxiliary masks), which is why no candidate has to
    /// be composed to reject a seed.
    fn compile_full_family(
        &self,
        query: &SearchQuery,
        accelerator: &Accelerator,
    ) -> Result<CompiledQuery, CompileError> {
        if !query.required_secondary_ids.is_empty()
            || !query.required_secondary_id_groups.is_empty()
            || !query.minimum_roll_percent_by_effect_id.is_empty()
            || !query.grouped_rolls.is_empty()
            || !query.effect_occurrences.is_empty()
        {
            return Err(CompileError::unsupported(
                "secondary, roll and occurrence constraints are replayed per candidate and this \
                 development worker does not compile that verification yet",
            ));
        }
        let primary = if query.primary_effect_ids.is_empty() {
            None
        } else {
            Some(self.primary_effect_spec(accelerator, query.rarity, &query.primary_effect_ids)?)
        };
        let auxiliary = if auxiliary_is_empty(&query.auxiliary) {
            None
        } else {
            Some(self.auxiliary_packing(query)?.0)
        };
        let has_terrain_constraint = auxiliary
            .as_ref()
            .is_some_and(|spec| spec.has_terrain_constraint);
        Ok(CompiledQuery {
            route: Route::FullFamily,
            digest: query.digest.clone(),
            native: NativePivotQuery::Natural {
                values: self.full_family_values(),
            },
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials: FULL_FAMILY_CHUNK_TRIALS,
            page_filter: Some(PageFilter {
                primary,
                auxiliary,
                effect_mask: None,
                effect_verifier: None,
            }),
            post_acceptance_filters: post_acceptance_filters(query),
        })
    }

    /// The shipped partial-effect fixed-draw replay
    /// (`partial_effect_batch_generator` feeding the full-family sweep).
    ///
    /// A request that names only some ordinary slots has no complete-composition
    /// preimage, so the shipped worker sweeps the whole seed family and decides
    /// each Seed with two filters: the accelerator's batched constraint mask
    /// (`match_effect_constraints_d3d11`) and then the certified forward
    /// composition (`effect_seed_solver._verify_effect_sequence`). The mask is
    /// acceleration, never the decision, so this route always carries the
    /// certified verifier.
    fn compile_partial_effect_filter(
        &self,
        query: &SearchQuery,
        accelerator: &Accelerator,
    ) -> Result<CompiledQuery, CompileError> {
        let special_mapping = match query.rarity {
            RARITY_FINALIZABLE => self.r4_grace.as_ref(),
            RARITY_DIVINE => self.r5_grace.as_ref(),
            _ => None,
        };
        // A selected Grace is the pivot: the shipped solver inverts its draw-1
        // runs and the cursor walks that permuted bucket table, never the whole
        // seed family.
        let pivot_values = match query.grace_effect_id {
            Some(grace) => {
                let map = special_mapping.ok_or_else(|| {
                    CompileError::data(format!(
                        "the rarity-{} Grace map is not present in the product resource",
                        query.rarity
                    ))
                })?;
                crate::effect_path::grace_pivot_values(grace, map).map_err(|error| {
                    CompileError::Rejected(format!(
                        "rarity-{} Grace 0x{grace:04X} has no draw-1 preimage: {error}",
                        query.rarity
                    ))
                })?
            }
            // Several selected Graces are an OR: the pivot is the union of each
            // Grace's draw-1 preimage, walked in the permuted family order so the
            // cursor space is deterministic. The exact final Grace is decided by
            // the job layer's `grace_effect_ids` acceptance.
            None if !query.grace_effect_ids.is_empty() => {
                let map = special_mapping.ok_or_else(|| {
                    CompileError::data(format!(
                        "the rarity-{} Grace map is not present in the product resource",
                        query.rarity
                    ))
                })?;
                let mut allowed = std::collections::BTreeSet::new();
                for grace in sorted_unique(&query.grace_effect_ids) {
                    // A Grace this map can never produce contributes no Seed.
                    if let Ok(values) = crate::effect_path::grace_pivot_values(grace, map) {
                        allowed.extend(values);
                    }
                }
                if allowed.is_empty() {
                    return Err(CompileError::Rejected(format!(
                        "none of the selected rarity-{} Graces has a draw-1 preimage",
                        query.rarity
                    )));
                }
                self.full_family_values()
                    .into_iter()
                    .filter(|value| allowed.contains(value))
                    .collect()
            }
            None => self.full_family_values(),
        };
        let (page_filter, has_terrain_constraint) =
            self.partial_effect_page_filter(query, special_mapping)?;
        // The shipped dispatch picks the pivot from the platform: with CUDA the
        // rarity-4 primary-conditioned collector enumerates the family, and
        // without it the DirectCompute fixed-draw collector walks the whole
        // seed family. Mirroring that choice keeps the cursor space identical.
        let (native, chunk_trials) = if query.rarity == RARITY_FINALIZABLE
            && !query.primary_effect_ids.is_empty()
            && accelerator.capabilities().cuda_seed_acceleration
        {
            let spec = self.r4_primary_spec(accelerator, &query.primary_effect_ids)?;
            (
                NativePivotQuery::R4Primary {
                    values: pivot_values,
                    spec,
                },
                R4_PRIMARY_CHUNK_TRIALS,
            )
        } else {
            (
                NativePivotQuery::Natural {
                    values: pivot_values,
                },
                FULL_FAMILY_CHUNK_TRIALS,
            )
        };
        Ok(CompiledQuery {
            route: Route::PartialEffectFilter,
            digest: query.digest.clone(),
            native,
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials,
            page_filter: Some(page_filter),
            post_acceptance_filters: partial_effect_post_acceptance_filters(query),
        })
    }

    /// The page-time acceptance every route whose pivot does not pack the
    /// caller's own effect criteria shares with the shipped solver
    /// (`partial_effect_batch_generator` plus `_iter_solution_prefetch`).
    ///
    /// The batched constraint mask is acceleration, never the decision, so the
    /// certified recomposition is always carried. A request whose only
    /// constraint is a job-layer filter composes nothing.
    fn partial_effect_page_filter(
        &self,
        query: &SearchQuery,
        special_mapping: Option<&GraceMap>,
    ) -> Result<(PageFilter, bool), CompileError> {
        let mask = crate::effect_batch::plan_effect_mask(
            &self.effect_index,
            special_mapping,
            query.playthrough,
            query.rarity,
            query.level,
            &query.primary_effect_ids,
            &query.required_secondary_ids,
            &query.required_secondary_id_groups,
        )
        .map_err(|error| match error {
            crate::effect_path::EffectPathError::Unsupported(message) => {
                CompileError::unsupported(message)
            }
            crate::effect_path::EffectPathError::Rejected(message) => {
                CompileError::Rejected(message)
            }
            crate::effect_path::EffectPathError::Data(message) => CompileError::data(message),
        })?;
        let criteria = crate::effect_batch::PartialEffectCriteria {
            primary_effect_ids: sorted_unique(&query.primary_effect_ids),
            required_secondary_ids: sorted_unique(&query.required_secondary_ids),
            required_secondary_id_groups: query
                .required_secondary_id_groups
                .iter()
                .map(|group| sorted_unique(group))
                .collect(),
            grace_effect_id: query.grace_effect_id,
            minimum_roll_percent_by_effect_id: query.minimum_roll_percent_by_effect_id.clone(),
        };
        // The shipped solver only runs the certified composition when the
        // request actually carries an ordinary-effect criterion; a request whose
        // only constraint is a job-layer filter (explicit effect occurrences, a
        // challenge count) is the plain full-family replay, and composing it
        // would decide nothing.
        let has_composition_criteria = !criteria.primary_effect_ids.is_empty()
            || !criteria.required_secondary_ids.is_empty()
            || !criteria.required_secondary_id_groups.is_empty()
            || !criteria.minimum_roll_percent_by_effect_id.is_empty()
            || criteria.grace_effect_id.is_some();
        let verifier = has_composition_criteria.then(|| {
            crate::effect_batch::PartialEffectVerifier::build(
                Arc::clone(&self.effect_index),
                query.rarity,
                query.playthrough,
                query.level,
                criteria,
                self.r4_grace.clone(),
                self.r5_grace.clone(),
            )
        });
        let auxiliary = if auxiliary_is_empty(&query.auxiliary) {
            None
        } else {
            Some(self.auxiliary_packing(query)?.0)
        };
        let has_terrain_constraint = auxiliary
            .as_ref()
            .is_some_and(|spec| spec.has_terrain_constraint);
        Ok((
            PageFilter {
                primary: None,
                auxiliary,
                effect_mask: mask.map(Arc::new),
                effect_verifier: verifier.map(Arc::new),
            },
            has_terrain_constraint,
        ))
    }

    /// The shipped NG3 rarity-3 named-primary pivot
    /// (`search_application.collect_offline_ng3_rarity3_primary_pivot_search_batch`).
    ///
    /// A rarity-3 request that names a primary and no Grace compiles its legal
    /// promotion-state families into one finite cursor space; the shipped
    /// exact-solver page then runs it unchanged, so the certified constraints
    /// stay page-side filters and the certified replay still decides every
    /// published Seed. Every shape the shipped guard keeps, a missing
    /// DirectCompute backend, and a cursor space that is not strictly smaller
    /// than the full 2**32 Seed family all return `None` so the caller
    /// continues down the shipped dispatch.
    fn compile_primary_pivot(
        &self,
        query: &SearchQuery,
        effect_preimage_available: bool,
    ) -> Result<Option<CompiledQuery>, CompileError> {
        if !effect_preimage_available
            || query.rarity != RARITY_GROWING
            || query.playthrough != 3
            || query.grace_effect_id.is_some()
            || query.primary_effect_ids.is_empty()
        {
            return Ok(None);
        }
        let families = crate::effect_path::compile_ng3_rarity3_primary_pivot_families(
            &query.primary_effect_ids,
            &self.effect_index,
        )
        .map_err(map_effect_path_compile_error)?;
        let state_count: u64 = families
            .iter()
            .map(|family| {
                family
                    .pivot_allowed_u16
                    .iter()
                    .map(|run| u64::from(run.bucket_count()))
                    .sum::<u64>()
            })
            .sum();
        if families.is_empty() || state_count >= 0x1_0000 {
            return Ok(None);
        }
        let mut specs: Vec<PrimaryPivotFamilySpec> = Vec::with_capacity(families.len());
        for family in &families {
            let plan = crate::effect_path::primary_pivot_native_plan(family)
                .map_err(map_effect_path_compile_error)?;
            specs.push(PrimaryPivotFamilySpec {
                values: plan.values,
                descriptors: plan.descriptors,
                params: plan.params,
                promotion_u16_runs: family.promotion_u16_runs.clone(),
            });
        }
        // Rarity 3 has no capture-backed special mapping, so the shared
        // partial-effect acceptance is built exactly as the full-family route
        // builds it.
        let (page_filter, has_terrain_constraint) = self.partial_effect_page_filter(query, None)?;
        Ok(Some(CompiledQuery {
            route: Route::R3PrimaryPivot,
            digest: query.digest.clone(),
            native: NativePivotQuery::PrimaryPivot {
                families: Arc::new(specs),
            },
            playthrough: query.playthrough,
            rarity: query.rarity,
            has_terrain_constraint,
            stage_specs: Vec::new(),
            chunk_trials: crate::search_backend::MAX_PRIMARY_PIVOT_TRIALS,
            page_filter: Some(page_filter),
            post_acceptance_filters: partial_effect_post_acceptance_filters(query),
        }))
    }

    /// `_ng3_rarity34_primary_lookup` plus the shipped rarity-3 draw parameters.
    ///
    /// Rarity 3 rolls its primary from the single special group `0x0001` with no
    /// promotion draw before it; rarity 4 uses the Grace-context matrix and is
    /// served by [`QueryCompiler::r4_primary_spec`] instead.
    fn primary_effect_spec(
        &self,
        accelerator: &Accelerator,
        rarity: u8,
        primary_effect_ids: &[u32],
    ) -> Result<PrimaryEffectSpec, CompileError> {
        let mut allowed = sorted_unique(primary_effect_ids);
        if allowed.is_empty() {
            return Err(CompileError::Rejected(
                "at least one primary effect must be selected".to_string(),
            ));
        }
        let normal = self.primary_pool_for(rarity, RARITY3_PRIMARY_SPECIAL_ID, false)?;
        let promoted = self.primary_pool_for(rarity, RARITY3_PRIMARY_SPECIAL_ID, true)?;
        let lookup = |pool: &[(u32, u32)]| {
            accelerator
                .build_weighted_effect_lookup(pool)
                .map_err(|error| {
                    CompileError::data(format!(
                        "native primary lottery for rarity {rarity}: {error:?}"
                    ))
                })
        };
        Ok(PrimaryEffectSpec {
            allowed_effect_ids: std::mem::take(&mut allowed),
            normal_lookup: lookup(&normal)?,
            promoted_lookup: lookup(&promoted)?,
            promotion_success_lookup: promotion_success_lookup(RARITY3_PRIMARY_PROMOTION_PERCENT),
            random7_lookup: random_int_u8_lookup(7),
            pre_promotion_draws: 0,
            slot_limit: 4,
            excluded_slot_mask: 0,
            primary_source_index: 0,
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
        // `terrain_row_matches_criteria`: the resolved option rows are an exact
        // row union on top of the key requirements.
        Ok(criteria.terrain_row_indices.is_empty()
            || u32::try_from(row_index)
                .is_ok_and(|row| criteria.terrain_row_indices.contains(&row)))
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
                let pool = self.primary_pool_for(RARITY_FINALIZABLE, *special_id, is_promoted)?;
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

    /// `_ng3_rarity34_primary_pool`: the seed-invariant primary lottery for one
    /// rarity and special group.
    fn primary_pool_for(
        &self,
        rarity: u8,
        special_id: u32,
        promoted: bool,
    ) -> Result<Vec<(u32, u32)>, CompileError> {
        let request = CandidatePoolRequest {
            context: NativeWeightContext {
                record_type: NG3_RECORD_TYPE,
                rarity,
                playthrough: 3,
                restricted_destination_slot: false,
                extra_selector: 0,
                rarity5_type_floor: 0,
            },
            destination_category_and_flags: 0x40,
            destination_effect_flags: if promoted { EFFECT_FLAG_PROMOTED } else { 0 },
            remaining_category_capacities: self
                .effect_index
                .category_capacities(NG3_RECORD_TYPE, rarity)
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
        && criteria.terrain_row_indices.is_empty()
}

/// `_has_terrain_constraints`.
pub fn auxiliary_has_terrain(criteria: &crate::query::AuxiliaryCriteria) -> bool {
    !criteria.required_terrain_effect_keys.is_empty()
        || !criteria.required_terrain_effect_key_groups.is_empty()
        || !criteria.terrain_row_indices.is_empty()
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
    if !query.grace_effect_ids.is_empty() {
        filters.push("grace_effect_ids");
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

/// Post-acceptance filters for the partial-effect forward-filter route.
///
/// The route's own acceptance already decides the ordinary-effect criteria
/// (primary, secondaries, any-of groups and plain roll minimums) from the
/// certified composition, so only the shipped job-layer filters remain: the
/// auxiliary composition, the grouped-roll thresholds, the explicit effect
/// occurrences and the challenge-count filter.
pub fn partial_effect_post_acceptance_filters(query: &SearchQuery) -> Vec<&'static str> {
    let mut filters = Vec::new();
    if !auxiliary_is_empty(&query.auxiliary) {
        filters.push("auxiliary_criteria");
    }
    if !query.grouped_rolls.is_empty() {
        filters.push("grouped_rolls");
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

/// `{3: 4, 4: 4, 5: 5}`: the ordinary slots one complete composition needs.
fn expected_ordinary_count(rarity: u8) -> usize {
    if rarity == RARITY_DIVINE {
        5
    } else {
        4
    }
}

fn u32_bytes(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// `game_random_int_from_u16`: two binary32 roundings then truncation.
pub(crate) fn game_random_int_from_u16(value: u16, count: u32) -> u32 {
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
    resource_version: Option<(u16, u16, u16, u16)>,
) -> Option<Arc<dyn SearchFactory>> {
    Some(Arc::new(NativeSearchFactory::new(
        application_root,
        accelerator_override,
        data_root,
        resource_version,
    )))
}

/// Compiles queries and hands the job layer a bounded native collector.
pub struct NativeSearchFactory {
    accelerator: Option<Arc<Accelerator>>,
    preimage: Result<Arc<crate::preimage::PreimageAccelerator>, crate::preimage::PreimageError>,
    compiler: Result<QueryCompiler, CompileError>,
}

impl NativeSearchFactory {
    /// `resource_version` must be the version the worker's `Materializer`
    /// composes candidates with; see [`QueryCompiler::load_for_resource_version`].
    pub fn new(
        application_root: &Path,
        accelerator_override: Option<&Path>,
        data_root: &Path,
        resource_version: Option<(u16, u16, u16, u16)>,
    ) -> Self {
        Self {
            accelerator: Accelerator::load(application_root, accelerator_override).map(Arc::new),
            preimage: {
                let path = crate::capabilities::effect_preimage_path(application_root, None);
                crate::preimage::PreimageAccelerator::load(application_root, Some(&path))
                    .map(Arc::new)
            },
            compiler: QueryCompiler::load_for_resource_version(data_root, resource_version),
        }
    }

    /// The real DirectCompute probe the shipped dispatch gates routes on
    /// (`d3d11_effect_acceleration_available`).
    ///
    /// The helper is hash-verified at load, so a substituted artifact reports
    /// `false` here exactly as it refuses every other route.
    fn effect_preimage_available(&self) -> bool {
        self.preimage
            .as_ref()
            .map(|accelerator| accelerator.available())
            .unwrap_or(false)
    }
}

impl SearchFactory for NativeSearchFactory {
    fn feasibility(&self, query: &SearchQuery) -> Option<Result<(), String>> {
        let compiler = self.compiler.as_ref().ok()?;
        Some(crate::feasibility::validate_query_feasibility(
            query,
            &compiler.effect_index,
        ))
    }

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
        // The shipped structural preflight, applied once here for every route:
        // a request the product refuses must be refused whatever collector it
        // would have reached, and no route may answer a structurally impossible
        // combination.
        crate::feasibility::validate_query_feasibility(query, &compiler.effect_index)
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error))?;
        let compiled = compiler
            .compile(query, accelerator, self.effect_preimage_available())
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error.to_string()))?;
        let backend = SearchBackend::new(Some(Arc::clone(accelerator)), self.preimage.clone());
        Ok(Arc::new(NativeCollector { backend, compiled }))
    }

    /// Compile the save-bound NG4/NG5 cached rarity-5 route.
    ///
    /// The job layer has already resolved `cache_id` to this registered map and
    /// checked that it belongs to the query's playthrough; this hook binds the
    /// compiler to that map so every downstream stage (the Grace draw-1
    /// inversion, the complete-composition plans and the certified
    /// recomposition) composes the player's own playthrough instead of the
    /// bundled NG3 tables.
    fn cached_collector(
        &self,
        query: &SearchQuery,
        cache: &crate::grace_map::GraceOutputMap,
    ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
        let compiler = self
            .compiler
            .as_ref()
            .map_err(|error| CollectorError::new("SEARCH_FAILED", error.to_string()))?;
        let accelerator = self.accelerator.as_ref().ok_or_else(|| {
            CollectorError::unavailable(
                "the native seed accelerator is unavailable, so bounded search cannot run",
            )
        })?;
        let mapping = cache
            .to_domain_map()
            .map_err(|message| CollectorError::new("INVALID_REQUEST", message))?;
        let bound = compiler
            .for_cached_grace(
                query.playthrough,
                mapping,
                cache.record_type,
                cache.effect_slot,
            )
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error.to_string()))?;
        crate::feasibility::validate_query_feasibility(query, bound.effect_index())
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error))?;
        let compiled = bound
            .compile(query, accelerator, self.effect_preimage_available())
            .map_err(|error| CollectorError::new("INVALID_REQUEST", error.to_string()))?;
        let backend = SearchBackend::new(Some(Arc::clone(accelerator)), self.preimage.clone());
        Ok(Arc::new(NativeCollector { backend, compiled }))
    }
}

/// The bounded collector for one compiled query.
struct NativeCollector {
    backend: SearchBackend,
    compiled: CompiledQuery,
}

impl SearchCollector for NativeCollector {
    fn native_unit_trials(&self) -> Option<u64> {
        // The window each native call scans (`PageRequest::effective_chunk`).
        Some(
            self.compiled
                .chunk_trials
                .min(self.compiled.native.max_chunk_trials()),
        )
        .filter(|trials| *trials > 0)
    }

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
        // The effect-preimage routes run on a different helper with its own
        // policy; pinning it for the job keeps a strict request strict even if a
        // previous job opted into a non-accelerated path. Drop restores it.
        let _preimage_pin =
            self.backend
                .pin_preimage_policy(PreimagePolicy::from_allow_cpu_fallback(
                    request.allow_cpu_fallback,
                ));

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
        // like the shipped solver: the native predicates run inside the page
        // loop, so a page whose candidates are all rejected still covers its
        // window in one scan instead of being truncated on raw native matches.
        let page_filter = self
            .compiled
            .page_filter
            .as_ref()
            .map(|filter| MatchFilter {
                primary: filter.primary.as_ref(),
                auxiliary: filter.auxiliary.as_ref(),
                effect_mask: filter.effect_mask.as_deref(),
                effect_verifier: filter.effect_verifier.as_deref(),
            });
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
                page_filter.as_ref(),
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
        NativeSearchError::PreimageUnavailable(message) => {
            CollectorError::unavailable(format!("{what}: {message}"))
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

    /// A clearly synthetic but structurally valid NG4 Grace-map payload.
    ///
    /// No genuine NG4/NG5 capture exists in the tree, so the cache route is
    /// proven with a labeled fixture: a dense two-range partition of the draw-1
    /// buckets whose ids are real effect rows. `generation_context_digest` is
    /// whatever the registering worker's own context reports.
    fn synthetic_ng4_cache_payload(generation_digest: &str) -> serde_json::Value {
        json!({
            "schema": "nioh3-grace-output-map-cache/v2",
            "game_version": "2.00.02",
            "generation_context_digest": generation_digest,
            "draw_index": 1,
            "record_type": "0xDD82",
            "rarity": 5,
            "playthrough": "synthetic-ng4",
            "effect_slot": 6,
            "ranges": [
                {"start": 0, "end": 32767, "grace_id": 5858},
                {"start": 32768, "end": 65535, "grace_id": 25939},
            ],
        })
    }

    /// The cached NG4 route compiles against the registered map and its page
    /// reproduces the shipped solver's own cursor for that map.
    ///
    /// The expected Seed and cursor come from running the shipped Python layer
    /// on this exact synthetic map returned (seed 182,147,323 at trial 11).
    #[test]
    fn the_cached_ng4_route_pages_like_the_shipped_solver() {
        let _accelerator = crate::accelerator_test_lock();
        use crate::native_search::Accelerator;
        use crate::search_backend::{MatchFilter, NativePivotQuery, PageRequest, SearchBackend};

        let root = repo_root();
        let compiler = QueryCompiler::load(&root.join("nioh3_scroll_editor").join("data"))
            .expect("the product tables load");
        let registered = crate::grace_map::from_cache_payload(
            &synthetic_ng4_cache_payload(&"a".repeat(64)),
            None,
        )
        .expect("the synthetic map satisfies the cache contract");
        let map = registered.to_domain_map().expect("the typed map is valid");
        let bound = compiler
            .for_cached_grace(4, map, registered.record_type, registered.effect_slot)
            .expect("the cached compiler binds");
        assert_eq!(CATEGORY_TO_TYPE[4], registered.record_type);

        let accelerator =
            Arc::new(Accelerator::load(&root, None).expect("the shipped seed accelerator loads"));
        let query = SearchQuery::from_payload(&json!({
            "playthrough": 4,
            "rarity": 5,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": 5858,
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
        .expect("the NG4 rarity-5 request is valid");
        let compiled = bound
            // This test drives the save-bound rarity-5 cache route, which never
            // uses the DirectCompute-gated rarity-3 pivot, so the probe is not
            // part of what it measures.
            .compile(&query, &accelerator, false)
            .expect("the cached route compiles");
        assert_eq!(compiled.route, Route::PartialEffectFilter);
        match &compiled.native {
            NativePivotQuery::Natural { values } => {
                assert_eq!(
                    values.len(),
                    32_768,
                    "the registered Grace run is the pivot"
                );
            }
            other => panic!("the NG4 Grace route must walk the Grace pivot: {other:?}"),
        }

        let preimage_path = crate::capabilities::effect_preimage_path(&root, None);
        let preimage =
            crate::preimage::PreimageAccelerator::load(&root, Some(&preimage_path)).map(Arc::new);
        let backend = SearchBackend::new(Some(Arc::clone(&accelerator)), preimage);
        let filter = compiled.page_filter.as_ref().map(|filter| MatchFilter {
            primary: filter.primary.as_ref(),
            auxiliary: filter.auxiliary.as_ref(),
            effect_mask: filter.effect_mask.as_deref(),
            effect_verifier: filter.effect_verifier.as_deref(),
        });
        // This page must also run on a host without a CUDA device, so the test
        // pins its own explicit bulk-CPU policy. The production default stays
        // StrictGpu and the strict-refusal tests keep asserting that a strict
        // request refuses.
        let _policy = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("the test's explicit bulk-CPU policy is accepted");
        let page = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 200_000, compiled.chunk_trials, 1),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the cached page runs");
        assert_eq!(page.matches.len(), 1);
        assert_eq!(page.matches[0].seed, 182_147_323);
        assert_eq!(page.matches[0].trial, 11);
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
        let _accelerator = crate::accelerator_test_lock();
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
                // The combined auxiliary query is rarity 4, so the
                // DirectCompute-gated rarity-3 pivot never applies.
                .compile(&query, &accelerator, false)
                .expect("the combined route compiles");
            let predicate = compiled
                .page_filter
                .as_ref()
                .and_then(|filter| filter.auxiliary.as_ref())
                .expect("the combined route carries the native predicate");
            // The page must also run on a host without a CUDA device, so the test
            // pins its own explicit bulk-CPU policy; the production default stays
            // StrictGpu.
            let _policy = backend
                .pin_policy(ExecutionPolicy::AllowBulkCpu)
                .expect("the test's explicit bulk-CPU policy is accepted");
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
                    .materialize(&query, matched.seed, matched.trial, None)
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

    /// The parity gate's partial-effect fixture
    /// (`tests/migration/test_search_worker_parity.py`).
    /// The shipped `PRIMARY_ROUTE_EFFECT` the parity gate searches by name.
    const PRIMARY_ROUTE_EFFECT: u32 = 30543;
    const PARITY_PRIMARY: u32 = 60020;
    const PARITY_SECONDARY: u32 = 12028;
    const PARITY_SEED: u32 = 226_727_520;
    const PARITY_TRIAL: u64 = 18_266;
    const PARITY_FAMILY_SIZE: u64 = 429_457_408;

    fn pivot_query(primary: &[u32], secondary: &[u32]) -> SearchQuery {
        SearchQuery::from_payload(&json!({
            "playthrough": 3,
            "rarity": 3,
            "level": 180,
            "primary_effect_ids": primary,
            "required_secondary_ids": secondary,
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
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
        .expect("the rarity-3 primary query is valid")
    }

    fn pivot_backend(root: &std::path::Path) -> SearchBackend {
        let preimage_path = crate::capabilities::effect_preimage_path(root, None);
        let preimage =
            crate::preimage::PreimageAccelerator::load(root, Some(&preimage_path)).map(Arc::new);
        SearchBackend::new(
            Some(Arc::new(
                Accelerator::load(root, None).expect("the shipped seed accelerator loads"),
            )),
            preimage,
        )
    }

    fn page_filter_of(compiled: &CompiledQuery) -> Option<MatchFilter<'_>> {
        compiled.page_filter.as_ref().map(|filter| MatchFilter {
            primary: filter.primary.as_ref(),
            auxiliary: filter.auxiliary.as_ref(),
            effect_mask: filter.effect_mask.as_deref(),
            effect_verifier: filter.effect_verifier.as_deref(),
        })
    }

    /// The compiled rarity-3 families of one named primary must reproduce the
    /// shipped `compile_ng3_rarity3_primary_pivot_families` pair exactly: the
    /// promotion interval, the pivot draw, the bucket interval and the state
    /// count of the parity fixture's own effect.
    #[test]
    fn the_rarity3_primary_pivot_family_matches_the_shipped_pair() {
        let root = repo_root();
        let compiler = QueryCompiler::load(&root.join("nioh3_scroll_editor").join("data"))
            .expect("the shipped tables load");
        let families = crate::effect_path::compile_ng3_rarity3_primary_pivot_families(
            &[PARITY_PRIMARY],
            &compiler.effect_index,
        )
        .expect("the parity family compiles");
        assert_eq!(families.len(), 1);
        let family = &families[0];
        assert_eq!(family.promoted_states, vec![Some(0)]);
        assert!(family.requires_promotion());
        assert_eq!(family.pivot_draw_index, 9);
        assert_eq!(
            family.pivot_allowed_u16,
            vec![crate::effect_path::U16Run {
                start: 52430,
                end: 58982
            }]
        );
        assert_eq!(family.promotion_draw_index, 1);
        assert_eq!(
            family.promotion_u16_runs,
            vec![crate::effect_path::U16Run {
                start: 0,
                end: 6553
            }]
        );
        assert_eq!(family.pivot_state_count(), PARITY_FAMILY_SIZE);
        assert!(family.pivot_state_count() < 0x1_0000_0000);
    }

    /// The two-family case partitions the draw-1 promotion states: the
    /// un-promoted family comes first (draw 2, promotion failed) and the
    /// promoted family second (draw 9, promotion succeeded), exactly like the
    /// shipped family order the cursor concatenates.
    #[test]
    fn the_rarity3_primary_pivot_families_partition_the_promotion_states() {
        let root = repo_root();
        let compiler = QueryCompiler::load(&root.join("nioh3_scroll_editor").join("data"))
            .expect("the shipped tables load");
        let families = crate::effect_path::compile_ng3_rarity3_primary_pivot_families(
            &[PRIMARY_ROUTE_EFFECT],
            &compiler.effect_index,
        )
        .expect("the two-family primary compiles");
        assert_eq!(families.len(), 2);
        let (unpromoted, promoted) = (&families[0], &families[1]);
        assert!(!unpromoted.requires_promotion());
        assert!(promoted.requires_promotion());
        assert_eq!(unpromoted.pivot_draw_index, 2);
        assert_eq!(promoted.pivot_draw_index, 9);
        assert_eq!(unpromoted.pivot_allowed_u16, promoted.pivot_allowed_u16);
        assert_eq!(unpromoted.promotion_draw_index, 1);
        assert_eq!(promoted.promotion_draw_index, 1);
        assert_eq!(
            unpromoted.promotion_u16_runs,
            vec![crate::effect_path::U16Run {
                start: 6554,
                end: 0xFFFF
            }]
        );
        assert_eq!(
            promoted.promotion_u16_runs,
            vec![crate::effect_path::U16Run {
                start: 0,
                end: 6553
            }]
        );
        // The two intervals partition the draw-1 states with no overlap.
        assert_eq!(
            promoted.promotion_u16_runs[0].end + 1,
            unpromoted.promotion_u16_runs[0].start
        );
        let state_count: u64 = families.iter().map(|item| item.pivot_state_count()).sum();
        assert_eq!(state_count, 220_332_032);
        assert!(state_count < 0x1_0000_0000);
    }

    /// The route must select the compiled families for a named rarity-3
    /// primary, and a page over its cursor space must publish the shipped
    /// fixture's Seed at the shipped fixture's trial.
    #[test]
    fn the_rarity3_primary_pivot_page_publishes_the_shipped_fixture() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let compiler = QueryCompiler::load(&data_root).expect("the shipped tables load");
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let query = pivot_query(&[PARITY_PRIMARY], &[PARITY_SECONDARY]);
        let compiled = compiler
            .compile(&query, &accelerator, true)
            .expect("the rarity-3 primary pivot compiles");
        assert_eq!(compiled.route, Route::R3PrimaryPivot);
        assert_eq!(compiled.native.family_size(), PARITY_FAMILY_SIZE);
        assert_eq!(
            compiled.chunk_trials,
            crate::search_backend::MAX_PRIMARY_PIVOT_TRIALS
        );
        match &compiled.native {
            NativePivotQuery::PrimaryPivot { families } => {
                assert_eq!(families.len(), 1);
                assert_eq!(families[0].values.len(), 6553);
                assert_eq!(families[0].values[0], 52430);
                assert_eq!(families[0].values[6552], 58982);
            }
            other => panic!("the rarity-3 primary pivot must walk its families: {other:?}"),
        }
        // Without the DirectCompute probe the shipped full-family route stays.
        assert_eq!(
            compiler
                .compile(&query, &accelerator, false)
                .expect("the fallback compiles")
                .route,
            Route::PartialEffectFilter
        );

        let backend = pivot_backend(&root);
        let filter = page_filter_of(&compiled);
        let page = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 10_000_000, compiled.chunk_trials, 1),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the pivot page runs");
        assert_eq!(page.matches.len(), 1);
        assert_eq!(page.matches[0].seed, PARITY_SEED);
        assert_eq!(page.matches[0].trial, PARITY_TRIAL);
        assert_eq!(page.next_cursor, PARITY_TRIAL);
        assert!(!page.exhausted);
        assert_eq!(page.fixed_seed_count, 0);
        assert!(page.stage_counts.is_empty());
    }

    /// The exact verifier: a trial's Seed must re-derive from the family
    /// cursor alone, and a wrong pair must be refused.
    #[test]
    fn the_rarity3_primary_pivot_cursor_replays_from_the_families() {
        let root = repo_root();
        let compiler = QueryCompiler::load(&root.join("nioh3_scroll_editor").join("data"))
            .expect("the shipped tables load");
        let families = crate::effect_path::compile_ng3_rarity3_primary_pivot_families(
            &[PARITY_PRIMARY],
            &compiler.effect_index,
        )
        .expect("the parity family compiles");
        let mut specs: Vec<PrimaryPivotFamilySpec> = Vec::new();
        for family in &families {
            let plan = crate::effect_path::primary_pivot_native_plan(family)
                .expect("the native plan builds");
            specs.push(PrimaryPivotFamilySpec {
                values: plan.values,
                descriptors: plan.descriptors,
                params: plan.params,
                promotion_u16_runs: family.promotion_u16_runs.clone(),
            });
        }
        assert_eq!(
            crate::search_backend::replay_primary_pivot_seed(&specs, PARITY_TRIAL)
                .expect("the fixture trial replays"),
            PARITY_SEED
        );
        assert!(crate::search_backend::replay_primary_pivot_seed(&specs, 0).is_err());
        assert!(
            crate::search_backend::replay_primary_pivot_seed(&specs, PARITY_FAMILY_SIZE + 1)
                .is_err(),
            "a trial outside the family has no Seed"
        );

        // The second family pair: the two-family primary's first two shipped
        // matches sit at trials 1 and 18 of the concatenated cursor.
        let two = crate::effect_path::compile_ng3_rarity3_primary_pivot_families(
            &[PRIMARY_ROUTE_EFFECT],
            &compiler.effect_index,
        )
        .expect("the two-family primary compiles");
        let mut two_specs: Vec<PrimaryPivotFamilySpec> = Vec::new();
        for family in &two {
            let plan = crate::effect_path::primary_pivot_native_plan(family)
                .expect("the native plan builds");
            two_specs.push(PrimaryPivotFamilySpec {
                values: plan.values,
                descriptors: plan.descriptors,
                params: plan.params,
                promotion_u16_runs: family.promotion_u16_runs.clone(),
            });
        }
        assert_eq!(
            crate::search_backend::replay_primary_pivot_seed(&two_specs, 1)
                .expect("trial 1 replays"),
            67_687_138
        );
        assert_eq!(
            crate::search_backend::replay_primary_pivot_seed(&two_specs, 18)
                .expect("trial 18 replays"),
            76_331_659
        );
    }

    /// A cancelled page keeps its checkpoint and the resume continues without
    /// replaying the candidate it already passed.
    #[test]
    fn the_rarity3_primary_pivot_page_resumes_without_replay() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let compiler = QueryCompiler::load(&data_root).expect("the shipped tables load");
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let query = pivot_query(&[PARITY_PRIMARY], &[PARITY_SECONDARY]);
        let compiled = compiler
            .compile(&query, &accelerator, true)
            .expect("the rarity-3 primary pivot compiles");
        let backend = pivot_backend(&root);
        let filter = page_filter_of(&compiled);

        // A first page that stops short of the fixture's trial publishes
        // nothing and reports the scan boundary as its resume cursor.
        let prefix = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 4096, compiled.chunk_trials, 1),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the prefix page runs");
        assert!(prefix.matches.is_empty());
        assert_eq!(prefix.next_cursor, 4096);

        let resumed = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(prefix.next_cursor, 10_000_000, compiled.chunk_trials, 1),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the resumed page runs");
        assert_eq!(resumed.matches.len(), 1);
        assert_eq!(resumed.matches[0].seed, PARITY_SEED);
        assert_eq!(resumed.matches[0].trial, PARITY_TRIAL);
        assert_eq!(resumed.next_cursor, PARITY_TRIAL);

        // Resuming at the published trial skips it rather than replaying it.
        let after = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(PARITY_TRIAL, 10_000_000, compiled.chunk_trials, 1),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the follow-up page runs");
        assert!(
            after
                .matches
                .iter()
                .all(|matched| matched.trial > PARITY_TRIAL),
            "a resumed page must not replay the published candidate"
        );

        // A cancel inside a long page keeps the accepted matches and the exact
        // checkpoint it already reached.
        let cancelled = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 10_000_000, compiled.chunk_trials, 25),
                &|| true,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("a cancelled page reports its checkpoint");
        assert!(cancelled.cancelled);
        assert_eq!(cancelled.next_cursor, 0);
        assert!(cancelled.matches.is_empty());
    }

    /// A cancellation observed while a long window is being decided stops the
    /// page inside that window: the checkpoint is the exclusive cursor the page
    /// really decided through, and the resume re-decides the interrupted raw
    /// match rather than replaying a published candidate or skipping one.
    #[test]
    fn the_rarity3_primary_pivot_cancels_inside_a_window_without_replay() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let compiler = QueryCompiler::load(&data_root).expect("the shipped tables load");
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let query = pivot_query(&[PARITY_PRIMARY], &[PARITY_SECONDARY]);
        let compiled = compiler
            .compile(&query, &accelerator, true)
            .expect("the rarity-3 primary pivot compiles");
        let backend = pivot_backend(&root);
        let filter = page_filter_of(&compiled);

        // The uninterrupted page is the reference candidate stream.
        let full = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 10_000_000, compiled.chunk_trials, 5),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the reference page runs");
        assert_eq!(full.matches.len(), 5);

        // The decision loop polls cancellation before each survivor, so the
        // third poll lands after the window was swept and its first batch masked
        // but before the first survivor is decided.
        let calls = std::cell::Cell::new(0usize);
        let cancelled = || {
            let seen = calls.get();
            calls.set(seen + 1);
            seen >= 2
        };
        let interrupted = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(0, 10_000_000, compiled.chunk_trials, 5),
                &cancelled,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the interrupted page runs");
        assert!(
            interrupted.cancelled,
            "the decision loop must observe the cancellation"
        );
        assert!(
            interrupted.next_cursor < PARITY_TRIAL,
            "the checkpoint must stay inside the interrupted window: {}",
            interrupted.next_cursor
        );
        assert_eq!(interrupted.matches, full.matches[..0]);

        // The resume re-decides the interrupted raw match and publishes the
        // reference stream's head, with nothing replayed and nothing skipped.
        let remaining = 5 - interrupted.matches.len();
        let resumed = backend
            .collect_page_filtered(
                &compiled.native,
                &PageRequest::chunk(
                    interrupted.next_cursor,
                    10_000_000,
                    compiled.chunk_trials,
                    remaining,
                ),
                &|| false,
                &mut |_| {},
                filter.as_ref(),
            )
            .expect("the resumed page runs");
        let union: Vec<crate::native_search::PivotMatch> = interrupted
            .matches
            .iter()
            .chain(resumed.matches.iter())
            .copied()
            .collect();
        assert_eq!(
            union, full.matches,
            "cancel plus resume must reproduce the uninterrupted stream"
        );
    }

    /// The production worker hands its compiler the exact version its
    /// materializer composes with. PC v2.02 changed the `optional_multiplier`
    /// table, so a compiler that silently kept the legacy payload would pack
    /// different thresholds than the candidates it publishes were built from.
    #[test]
    fn the_compiler_reads_the_materializer_resource_version() {
        let data_root = repo_root().join("nioh3_scroll_editor").join("data");
        let legacy = QueryCompiler::load(&data_root).expect("the legacy tables load");
        let current = QueryCompiler::load_for_resource_version(
            &data_root,
            Some(nioh3_data::CURRENT_RESOURCE_VERSION),
        )
        .expect("the current tables load");
        let current_resource = nioh3_data::load_preview_resources_for_file_version(
            &data_root,
            nioh3_data::CURRENT_RESOURCE_VERSION,
        )
        .expect("the current preview tables load");
        assert_eq!(
            current.optional_multipliers, current_resource.context.optional_multipliers,
            "the compiler consumes the selected version's optional multipliers"
        );
        assert_ne!(
            current.optional_multipliers, legacy.optional_multipliers,
            "PC v2.02 owns a different optional_multiplier table"
        );
        // An unregistered version fails closed instead of reusing the legacy
        // payload.
        assert!(QueryCompiler::load_for_resource_version(&data_root, Some((9, 9, 9, 9))).is_err());
    }

    fn resolved(materializer: &Materializer, payload: serde_json::Value) -> SearchQuery {
        let mut query = SearchQuery::from_payload(&payload).expect("the query shape is valid");
        materializer
            .resolve_query(&mut query)
            .expect("the query resolves against the context tables");
        query
    }

    fn base_payload(rarity: u8) -> serde_json::Value {
        json!({
            "playthrough": 3,
            "rarity": rarity,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        })
    }

    /// Terrain option ids used to be refused outright. They now resolve to the
    /// reference row union, the fused native mask carries exactly those rows,
    /// and on the R4 primary route (which does not pivot on the auxiliary
    /// criteria) the native predicate and the composed final acceptance agree
    /// on every match of a bounded window.
    #[test]
    fn terrain_options_compile_to_their_row_union_and_are_enforced() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let version = Some(nioh3_data::CURRENT_RESOURCE_VERSION);
        let compiler = QueryCompiler::load_for_resource_version(&data_root, version)
            .expect("the current tables load");
        let materializer =
            Materializer::with_resource_version(&data_root, "terrain-option-test", version);
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let backend = SearchBackend::from_shared(Arc::new(
            Accelerator::load(&root, None).expect("the shipped accelerator loads twice"),
        ));

        let choices = crate::terrain::terrain_choices(&compiler.terrains);
        let aggregate = choices
            .iter()
            .find(|choice| choice.aggregate)
            .expect("the shipped table publishes a contains: option");
        let none = choices
            .iter()
            .find(|choice| choice.option_id == "exact:")
            .expect("the shipped table publishes the no-effect option");

        // Auxiliary-only: the fused route's mask is exactly the row union.
        let mut payload = base_payload(4);
        payload["terrain_selection_ids"] = json!([aggregate.option_id, none.option_id]);
        let query = resolved(&materializer, payload);
        let expected: Vec<u32> = aggregate.rows.union(&none.rows).copied().collect();
        assert_eq!(query.auxiliary.terrain_row_indices, expected);
        let compiled = compiler
            .compile(&query, &accelerator, false)
            .expect("a terrain-only query compiles");
        assert_eq!(compiled.route, Route::Auxiliary);
        assert!(compiled.has_terrain_constraint);
        let NativePivotQuery::Auxiliary { spec, .. } = &compiled.native else {
            panic!("a terrain-only query takes the fused auxiliary route");
        };
        let masked: Vec<u32> = spec
            .allowed_terrain_rows
            .iter()
            .enumerate()
            .filter(|(_, allowed)| **allowed != 0)
            .map(|(row, _)| row as u32)
            .collect();
        assert_eq!(masked, expected);

        // An unknown option is refused, never guessed.
        let mut unknown = SearchQuery::from_payload(&{
            let mut payload = base_payload(4);
            payload["terrain_selection_ids"] = json!(["exact:1"]);
            payload
        })
        .expect("the shape is valid");
        let error = materializer
            .resolve_query(&mut unknown)
            .expect_err("an unknown option id is refused");
        assert!(error.message.contains("Unknown terrain option"));

        // R4 primary plus terrain: the page filter decides the terrain and the
        // composed acceptance agrees with it seed for seed.
        let mut payload = base_payload(4);
        payload["primary_effect_ids"] = json!([0x774F]);
        payload["terrain_selection_ids"] = json!([aggregate.option_id]);
        let query = resolved(&materializer, payload);
        let compiled = compiler
            .compile(&query, &accelerator, false)
            .expect("the R4 primary route compiles with a terrain option");
        assert_eq!(compiled.route, Route::R4Primary);
        assert!(compiled.has_terrain_constraint);
        let predicate = compiled
            .page_filter
            .as_ref()
            .and_then(|filter| filter.auxiliary.as_ref())
            .expect("the R4 primary route carries the terrain predicate");
        let _policy = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("the test's explicit bulk-CPU policy is accepted");
        let page = backend
            .collect_page(
                &compiled.native,
                &PageRequest::chunk(0, 100_000, compiled.chunk_trials, 1_000_000),
                &|| false,
            )
            .expect("a bounded R4 primary page");
        let seeds: Vec<u32> = page.matches.iter().map(|matched| matched.seed).collect();
        let selected = backend
            .auxiliary_criteria_selected(predicate, &seeds)
            .expect("the native predicate runs");
        let (mut accepted, mut rejected) = (0usize, 0usize);
        for (matched, keep) in page.matches.iter().zip(&selected) {
            let materialized = materializer
                .materialize(&query, matched.seed, matched.trial, None)
                .expect("every R4 primary match composes");
            assert_eq!(*keep, materialized.auxiliary_match == Some(true));
            if *keep {
                let preview = materializer
                    .compose(matched.seed, 4, 180, None)
                    .expect("an accepted seed composes")
                    .3;
                assert!(aggregate
                    .rows
                    .contains(&(preview.auxiliary.terrain.selected_row_index as u32)));
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
        assert!(
            accepted > 0 && rejected > 0,
            "the window must exercise both verdicts ({accepted} accepted, {rejected} rejected)"
        );
    }

    /// Several selected Graces are an OR. The partial-effect route pivots on
    /// the union of their draw-1 preimages, and the job layer's final Grace
    /// acceptance keeps exactly the candidates whose actual Grace is selected.
    #[test]
    fn several_selected_graces_pivot_on_their_union_and_filter_the_final_grace() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let version = Some(nioh3_data::CURRENT_RESOURCE_VERSION);
        let compiler = QueryCompiler::load_for_resource_version(&data_root, version)
            .expect("the current tables load");
        let materializer =
            Materializer::with_resource_version(&data_root, "grace-union-test", version);
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let backend = SearchBackend::from_shared(Arc::new(
            Accelerator::load(&root, None).expect("the shipped accelerator loads twice"),
        ));
        let _policy = backend
            .pin_policy(ExecutionPolicy::AllowBulkCpu)
            .expect("the test's explicit bulk-CPU policy is accepted");

        for (rarity, map) in [
            (5u8, compiler.r5_grace.clone()),
            (4u8, compiler.r4_grace.clone()),
        ] {
            let map = map.expect("the product resource carries the Grace map");
            let mut offered: Vec<u32> = map.ranges.iter().map(|range| range.effect_id).collect();
            offered.sort_unstable();
            offered.dedup();
            if rarity == 4 {
                offered.retain(|id| crate::catalog::R4_FINAL_GRACE_IDS.contains(id));
            }
            let selected = vec![offered[0], offered[1]];
            let mut payload = base_payload(rarity);
            payload["grace_effect_ids"] = json!(selected);
            let query = resolved(&materializer, payload);
            let compiled = compiler
                .compile(&query, &accelerator, false)
                .expect("a Grace-only multi-selection compiles");
            assert_eq!(compiled.route, Route::PartialEffectFilter);
            assert!(compiled
                .post_acceptance_filters
                .contains(&"grace_effect_ids"));
            let NativePivotQuery::Natural { values } = &compiled.native else {
                panic!("the Grace union is a natural pivot without CUDA primaries");
            };
            let mut expected = std::collections::BTreeSet::new();
            for grace in &selected {
                expected.extend(
                    crate::effect_path::grace_pivot_values(*grace, &map).expect("a preimage"),
                );
            }
            assert_eq!(values.len(), expected.len());
            assert!(values.iter().all(|value| expected.contains(value)));

            let page = backend
                .collect_page(
                    &compiled.native,
                    &PageRequest::chunk(0, 20_000, compiled.chunk_trials, 1_000_000),
                    &|| false,
                )
                .expect("a bounded Grace page");
            assert!(!page.matches.is_empty());
            let mut kept = 0usize;
            for matched in &page.matches {
                let materialized = materializer
                    .materialize(&query, matched.seed, matched.trial, None)
                    .expect("every Grace match composes");
                let grace = crate::jobs::candidate_grace(&materialized.candidate);
                if grace.is_some_and(|id| selected.contains(&id)) {
                    kept += 1;
                }
            }
            assert!(kept > 0, "rarity {rarity}: the union keeps real matches");
        }
    }

    /// An unconstrained rarity-5 request is the full-family replay like every
    /// other rarity, instead of an "unimplemented" refusal.
    #[test]
    fn an_unconstrained_rarity5_search_takes_the_full_family() {
        let _accelerator = crate::accelerator_test_lock();
        let root = repo_root();
        let data_root = root.join("nioh3_scroll_editor").join("data");
        let compiler = QueryCompiler::load_for_resource_version(
            &data_root,
            Some(nioh3_data::CURRENT_RESOURCE_VERSION),
        )
        .expect("the current tables load");
        let accelerator = Accelerator::load(&root, None).expect("the shipped accelerator loads");
        let query = SearchQuery::from_payload(&base_payload(5)).expect("valid");
        let compiled = compiler
            .compile(&query, &accelerator, false)
            .expect("an unconstrained rarity-5 query compiles");
        assert_eq!(compiled.route, Route::FullFamily);
    }
}
