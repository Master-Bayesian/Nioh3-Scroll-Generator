//! Application engine: context capture, contract identity, candidate
//! materialization and request dispatch.
//!
//! Resource loading and pure composition belong to `nioh3-data` and
//! `nioh3-domain`; this module sequences them, binds the result to the
//! generation context and owns the single background search job.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nioh3_data::{load_effect_resource, load_preview_resources, PreviewResources};
use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::enemy::MissionVariant;
use nioh3_domain::preview::{
    compose_auxiliary_preview, compose_enemy_state_preview, effect_previews, AuxiliaryPreview,
    EnemyStatePreview, PreviewTables,
};
use nioh3_domain::record::{materialize_ng3_rarity4_final_record, ScrollRecord, ScrollRecordBytes};
use nioh3_domain::sequence::{
    generate_challenge_attempt_count, generate_ng3_rarity3_effect_sequence,
    generate_rarity5_grace_effect_sequence, NG3_RECORD_TYPE,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::capabilities::{self, Capabilities};
use crate::catalog::{self, Catalog};
use crate::collector::{self, CandidateSource, CollectorError, MaterializedCandidate};
use crate::context::{
    capture_legacy_context, capture_resolved_context, hex_lower, ContextError, GameFileVersion,
    LegacyGenerationContext, ResolvedGenerationContext,
};
use crate::jobs::{JobStore, StartParams};
use crate::model::{Candidate, CandidateEffect, RecordStage};
use crate::native::probe_seed_accelerator;
use crate::payload;
use crate::protocol::{Request, RequestError};
use crate::recommended_level::{self, RecommendedLevelCurve};
use crate::schema::RequestSchema;

/// Product playthrough the offline preview path is certified for.
pub const NG3_PLAYTHROUGH: u8 = 3;

/// Engine failures, carrying a shipped-compatible error code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

impl EngineError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EngineError {}

/// What one dispatch produced.
#[derive(Debug)]
pub enum Outcome {
    /// A frame to write back.
    Reply(Value),
    /// A reply that must be flushed, after which the process stops.
    Stop(Value),
}

/// Lazily loaded resource set for the offline path.
struct LoadedResources {
    index: EffectTableIndex,
    effect: EffectResourceBytes,
    preview: PreviewResources,
}

/// Candidate materialization shared by the preview method and the search job.
///
/// Only seeds the collector already accepted are composed, so a page that
/// inspects 100M trials still builds at most `result_count` candidates.
pub struct Materializer {
    data_root: PathBuf,
    context_digest: String,
    resource_version: Option<(u16, u16, u16, u16)>,
    resources: Mutex<Option<Arc<LoadedResources>>>,
}

impl Materializer {
    pub fn new(data_root: &Path, context_digest: &str) -> Self {
        Self::with_resource_version(
            data_root,
            context_digest,
            Some(nioh3_data::CURRENT_RESOURCE_VERSION),
        )
    }

    /// ``None`` selects the shipped legacy v2.00.02 payload for BOTH halves -
    /// the effect tables behind record composition and the preview tables - so
    /// legacy fixtures keep their historical bytes.
    pub fn with_resource_version(
        data_root: &Path,
        context_digest: &str,
        resource_version: Option<(u16, u16, u16, u16)>,
    ) -> Self {
        Self {
            data_root: data_root.to_path_buf(),
            context_digest: context_digest.to_string(),
            resource_version,
            resources: Mutex::new(None),
        }
    }

    pub fn resource_version(&self) -> Option<(u16, u16, u16, u16)> {
        self.resource_version
    }

    fn resources(&self) -> Result<Arc<LoadedResources>, EngineError> {
        let mut slot = self.resources.lock().expect("resource cache");
        if let Some(resources) = slot.as_ref() {
            return Ok(Arc::clone(resources));
        }
        // The version selects both halves; an unregistered version fails closed
        // here rather than silently reusing the shipped tables.
        let effect = match self.resource_version {
            Some(version) => {
                nioh3_data::load_effect_resource_for_file_version(&self.data_root, version)
            }
            None => load_effect_resource(&self.data_root),
        }
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        let index = EffectTableIndex::from_resource(&effect)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", format!("{error:?}")))?;
        let preview = match self.resource_version {
            Some(version) => {
                nioh3_data::load_preview_resources_for_file_version(&self.data_root, version)
            }
            None => load_preview_resources(&self.data_root),
        }
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        let resources = Arc::new(LoadedResources {
            index,
            effect,
            preview,
        });
        *slot = Some(Arc::clone(&resources));
        Ok(resources)
    }

    /// Run `f` over the loaded effect resource and preview tables.
    ///
    /// The catalog needs the decoded effect index, the raw resource (for the
    /// measured Grace maps) and the preview tables. Handing them over keeps the
    /// lazy cache in one place instead of making the catalog load its own copy
    /// of tables the worker already holds.
    pub(crate) fn inspect<T>(
        &self,
        f: impl FnOnce(
            &EffectTableIndex,
            &EffectResourceBytes,
            &PreviewResources,
        ) -> Result<T, EngineError>,
    ) -> Result<T, EngineError> {
        let resources = self.resources()?;
        f(&resources.index, &resources.effect, &resources.preview)
    }

    /// Compose the certified candidate and the preview payload its result
    /// carries, mirroring `candidate_payload` plus `export_candidate`.
    ///
    /// The preview is composed in the reference order - auxiliary half first,
    /// then the two enemy-state variants and the challenge capacity - so the
    /// payload is byte-identical to `compose_ng3_preview`.
    pub fn compose(
        &self,
        seed: u32,
        rarity: u8,
        level: u16,
        joint_search_trial: Option<u64>,
    ) -> Result<(Candidate, Value, Value, ComposedPreview), EngineError> {
        let resources = self.resources()?;
        let tables = preview_tables(&resources);
        let auxiliary = compose_auxiliary_preview(seed, NG3_PLAYTHROUGH, &tables)
            .map_err(unsupported_preview)?;
        self.compose_from_auxiliary(
            &resources,
            &tables,
            seed,
            rarity,
            level,
            joint_search_trial,
            auxiliary,
            NG3_PLAYTHROUGH,
            None,
        )
    }

    /// Compose the record, the enemy-state half and the payload from an
    /// already-computed auxiliary half.
    ///
    /// Splitting the auxiliary stage out lets [`Materializer::materialize`]
    /// decide the caller's auxiliary criteria before the remaining, far more
    /// expensive composition runs, without composing the auxiliary half twice.
    #[allow(clippy::too_many_arguments)]
    fn compose_from_auxiliary(
        &self,
        resources: &LoadedResources,
        tables: &PreviewTables<'_>,
        seed: u32,
        rarity: u8,
        level: u16,
        joint_search_trial: Option<u64>,
        auxiliary: AuxiliaryPreview,
        playthrough: u8,
        cached_grace: Option<&nioh3_domain::effect::GraceMap>,
    ) -> Result<(Candidate, Value, Value, ComposedPreview), EngineError> {
        // The shipped payload publishes the enemy-state half for the NG3
        // playthrough only (`worker_contracts.candidate_payload` emits `null`
        // otherwise and never generates the previews), so a cached NG4/NG5
        // candidate carries `null` and pays for no enemy composition.
        let enemy_states = if playthrough == NG3_PLAYTHROUGH {
            Some([
                compose_enemy_state_preview(seed, playthrough, MissionVariant::Solo, tables)
                    .map_err(unsupported_preview)?,
                compose_enemy_state_preview(seed, playthrough, MissionVariant::Expedition, tables)
                    .map_err(unsupported_preview)?,
            ])
        } else {
            None
        };
        let composition = ComposedPreview {
            auxiliary,
            enemy_states,
            initial_challenge_capacity: generate_challenge_attempt_count(seed),
        };

        let record = build_sequence(
            rarity,
            seed,
            level,
            &resources.index,
            &resources.effect,
            playthrough,
            cached_grace,
        )?;
        let effects: Vec<CandidateEffect> = effect_previews(&record)
            .into_iter()
            .map(|effect| CandidateEffect {
                slot: u32::from(effect.slot),
                effect_id: effect.effect_id,
                value: effect.value,
                metadata: effect.metadata,
                prefix: u32::from(effect.prefix),
                tail_0: effect.tail_0,
                tail_1: effect.tail_1,
                roll_percent: effect.roll_percent.map(u32::from),
            })
            .collect();
        let candidate = Candidate {
            seed,
            playthrough: Some(playthrough),
            rarity,
            record_stage: RecordStage::EffectSequenceOnly,
            record: Vec::new(),
            installation_record: None,
            effects,
            joint_search_trial,
        };

        let payload =
            payload::candidate_payload_json(&candidate, &self.context_digest, &composition);
        let transfer = payload::transfer_json(&candidate, &self.context_digest, level);
        Ok((candidate, payload, transfer, composition))
    }

    /// `candidate.preview`: `{"candidate": ..., "transfer": ...}`.
    pub fn preview(&self, seed: u32, rarity: u8, level: u16) -> Result<Value, EngineError> {
        let (_, candidate, transfer, _) = self.compose(seed, rarity, level, None)?;
        Ok(serde_json::json!({
            "candidate": candidate,
            "transfer": transfer,
        }))
    }
}

impl CandidateSource for Materializer {
    /// Resolve terrain options and check Grace choices against this context's
    /// own tables, exactly as `SearchQuery.from_payload` does with the shipped
    /// resolver and Grace maps.
    fn resolve_query(&self, query: &mut crate::query::SearchQuery) -> Result<(), CollectorError> {
        let resources = self
            .resources()
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        query.auxiliary.terrain_row_indices = crate::terrain::resolve_terrain_selections(
            &resources.preview.roster.terrains,
            &query.terrain_selection_ids,
        )
        .map_err(|message| CollectorError::new("INVALID_REQUEST", message))?;
        if !query.grace_effect_ids.is_empty() {
            // Rarity 4 offers the verified final Grace ids, never the raw
            // stage-one output codes; rarity 5 offers its measured map's ids.
            let allowed: Vec<u32> = match query.rarity {
                4 => catalog::R4_FINAL_GRACE_IDS.to_vec(),
                5 => resources
                    .effect
                    .grace_maps
                    .get(1)
                    .map(|map| map.ranges.iter().map(|range| range.effect_id).collect())
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            if query
                .grace_effect_ids
                .iter()
                .any(|effect_id| !allowed.contains(effect_id))
            {
                return Err(CollectorError::new(
                    "INVALID_REQUEST",
                    "Grace choices do not belong to this rarity",
                ));
            }
        }
        Ok(())
    }

    fn materialize(
        &self,
        query: &crate::query::SearchQuery,
        seed: u32,
        trial: u64,
        grace: Option<&nioh3_domain::effect::GraceMap>,
    ) -> Result<MaterializedCandidate, CollectorError> {
        let resources = self
            .resources()
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        let tables = preview_tables(&resources);
        let playthrough = query.playthrough;

        // Cheap pre-acceptance stage. The caller's terrain / special-rule /
        // enemy criteria are decided by the auxiliary half alone, so they are
        // evaluated before the record, the enemy-state half and the payload are
        // composed. A match those criteria already exclude is rejected without
        // paying for the full preview, exactly as the shipped worker never
        // composes a payload for a candidate its own filter drops. Anything the
        // auxiliary half itself cannot compose still fails the job closed.
        let auxiliary = compose_auxiliary_preview(seed, playthrough, &tables)
            .map_err(unsupported_preview)
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        let auxiliary_match = auxiliary_criteria_match(query, &auxiliary);
        if !auxiliary_match {
            return Ok(MaterializedCandidate {
                candidate: rejected_candidate(seed, query.rarity, trial, playthrough),
                payload: Value::Null,
                auxiliary_match: Some(false),
                enemy_occurrence_match: None,
            });
        }

        let (candidate, payload, _, composition) = self
            .compose_from_auxiliary(
                &resources,
                &tables,
                seed,
                query.rarity,
                query.level,
                Some(trial),
                auxiliary,
                playthrough,
                grace,
            )
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        let enemy_occurrence_match = Some(enemy_occurrence_groups_match(query, &composition));
        Ok(MaterializedCandidate {
            candidate,
            payload,
            auxiliary_match: Some(auxiliary_match),
            enemy_occurrence_match,
        })
    }
}

/// The typed preview tables a loaded resource set exposes.
fn preview_tables(resources: &LoadedResources) -> PreviewTables<'_> {
    PreviewTables {
        roster: &resources.preview.roster,
        context: &resources.preview.context,
        rules: &resources.preview.rules,
        states: &resources.preview.states,
    }
}

/// A candidate the ported auxiliary composition cannot represent (for example
/// a data row outside the captured tables) fails closed with a named limit
/// instead of being dropped from the result list.
fn unsupported_preview(error: impl std::fmt::Debug) -> EngineError {
    EngineError::new(
        "UNSUPPORTED_CONTEXT",
        format!(
            "the ported offline preview cannot compose this candidate, so the \
             search fails closed rather than dropping it: {error:?}"
        ),
    )
}

/// The candidate a match carries when the required auxiliary criteria already
/// reject it. It is never published: `jobs::accepts` refuses any candidate whose
/// `auxiliary_match` is not `Some(true)`, so the placeholder only has to be
/// cheap, not complete.
fn rejected_candidate(seed: u32, rarity: u8, trial: u64, playthrough: u8) -> Candidate {
    Candidate {
        seed,
        playthrough: Some(playthrough),
        rarity,
        record_stage: RecordStage::EffectSequenceOnly,
        record: Vec::new(),
        installation_record: None,
        effects: Vec::new(),
        joint_search_trial: Some(trial),
    }
}

/// `auxiliary_generation.AuxiliarySearchCriteria.matches(...)` over the composed
/// auxiliary output.
///
/// Ports the three sub-checks exactly: terrain display keys, non-zero special
/// rule keys and the enemy lookup keys of every group, each requiring the
/// requested key set as a subset plus one hit per any-of group.
///
/// When `enemy_occurrence_groups` is present the shipped `SearchQuery.from_payload`
/// *replaces* the caller's enemy keys and groups with compiled prefilters, so the
/// caller's own enemy keys are deliberately not re-checked here; the real
/// occurrence semantics are enforced by [`enemy_occurrence_groups_match`].
fn auxiliary_criteria_match(
    query: &crate::query::SearchQuery,
    auxiliary_preview: &AuxiliaryPreview,
) -> bool {
    let auxiliary = auxiliary_preview;

    let terrain: Vec<u32> = auxiliary
        .terrain
        .display_effect_keys
        .iter()
        .map(|key| u32::from(*key))
        .collect();
    if !contains_all(&terrain, &query.auxiliary.required_terrain_effect_keys)
        || !hits_every_group(
            &terrain,
            &query.auxiliary.required_terrain_effect_key_groups,
        )
    {
        return false;
    }
    // `AuxiliarySearchCriteria.matches_terrain`: a resolved option selection
    // is an exact row union on top of the key requirements above.
    if !query.auxiliary.terrain_row_indices.is_empty()
        && !u32::try_from(auxiliary.terrain.selected_row_index)
            .is_ok_and(|row| query.auxiliary.terrain_row_indices.contains(&row))
    {
        return false;
    }

    let rules: Vec<u32> = auxiliary
        .special_rules
        .keys
        .iter()
        .filter(|key| **key != 0)
        .map(|key| u32::from(*key))
        .collect();
    if !contains_all(&rules, &query.auxiliary.required_special_rule_keys)
        || !hits_every_group(&rules, &query.auxiliary.required_special_rule_key_groups)
    {
        return false;
    }

    if !query.enemy_occurrence_groups.is_empty() {
        return true;
    }
    let enemies: Vec<u32> = auxiliary
        .enemy_groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .map(|entry| entry.lookup_key)
        .collect();
    contains_all(&enemies, &query.auxiliary.required_enemy_lookup_keys)
        && hits_every_group(&enemies, &query.auxiliary.required_enemy_lookup_key_groups)
}

/// `required.issubset(actual)`.
fn contains_all(actual: &[u32], required: &[u32]) -> bool {
    required.iter().all(|key| actual.contains(key))
}

/// Every group must intersect the actual key set.
fn hits_every_group(actual: &[u32], groups: &[Vec<u32>]) -> bool {
    groups
        .iter()
        .all(|group| group.iter().any(|key| actual.contains(key)))
}

/// `enemy_state_search.enemy_occurrence_groups_status(...) == "match"`.
///
/// The status is `no_match` if any group is `no_match`, else `unknown` if any
/// group is `unknown`, else `match`; only `match` accepts a candidate, so an
/// unknown Possessed/Curse state never hides behind a false positive.
fn enemy_occurrence_groups_match(
    query: &crate::query::SearchQuery,
    composition: &ComposedPreview,
) -> bool {
    use crate::query::{Availability, EnemyStateFilter, EnemyVariant};
    use nioh3_domain::enemy::Possession;
    use nioh3_domain::preview::OccurrenceAvailability;

    if query.enemy_occurrence_groups.is_empty() {
        return true;
    }
    // Only the NG3 playthrough publishes an enemy-state half, so a cached
    // NG4/NG5 candidate cannot be decided against occurrence groups. The
    // shipped worker raises in the same situation (it generates the preview for
    // the request's own playthrough), so this stays a rejection rather than an
    // invented answer.
    let Some(enemy_states) = composition.enemy_states.as_ref() else {
        return false;
    };
    let state = match query.enemy_variant {
        EnemyVariant::Solo => &enemy_states[0],
        EnemyVariant::Expedition => &enemy_states[1],
    };

    let mut any_no_match = false;
    let mut any_unknown = false;
    for group in &query.enemy_occurrence_groups {
        let mut group_match = false;
        let mut group_unknown = false;
        for requirement in group {
            let mut matched = false;
            let mut unknown = false;
            for occurrence in &state.occurrences {
                if !requirement.lookup_keys.contains(&occurrence.lookup_key) {
                    continue;
                }
                let availability_ok = match requirement.availability {
                    Availability::Any => true,
                    Availability::Base => occurrence.availability == OccurrenceAvailability::Base,
                    Availability::ExpeditionOnly => {
                        occurrence.availability == OccurrenceAvailability::ExpeditionOnly
                    }
                };
                if !availability_ok {
                    continue;
                }
                match requirement.state {
                    EnemyStateFilter::Any => matched = true,
                    EnemyStateFilter::Possessed => {
                        matched = occurrence.possessed == Possession::Yes;
                        unknown = unknown || occurrence.possessed == Possession::Unknown;
                    }
                    // Curse is rejected before a job starts; it is never
                    // Seed-exact, so it can only ever be unknown here.
                    EnemyStateFilter::Curse => unknown = true,
                }
                if matched {
                    break;
                }
            }
            group_match = group_match || matched;
            group_unknown = group_unknown || unknown;
            if matched {
                break;
            }
        }
        if !group_match {
            if group_unknown {
                any_unknown = true;
            } else {
                any_no_match = true;
            }
        }
    }
    !any_no_match && !any_unknown
}

/// The composed halves one candidate payload and its occurrence filter need.
///
/// The enemy-state half exists only for the NG3 playthrough: the shipped
/// `worker_contracts.candidate_payload` emits `enemy_states: null` for every
/// other playthrough and never even generates the previews, so a cached NG4/NG5
/// payload must carry `null` rather than a composition the product does not
/// publish for that playthrough.
pub struct ComposedPreview {
    pub auxiliary: AuxiliaryPreview,
    pub enemy_states: Option<[EnemyStatePreview; 2]>,
    pub initial_challenge_capacity: i32,
}

/// Which identity a launch resolves, and whether it may authorize work.
///
/// A production launch selects one exact installed game version. The legacy
/// variant is opt-in, explicitly labelled non-production, and is never reachable
/// from the packaged or development acknowledgement path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSelection {
    /// Production: bind the identity to this exact installed executable version.
    Production(GameFileVersion),
    /// Opt-in test/dev mode: reproduce the pre-version identity, no authority.
    LegacyTest,
}

/// The identity an `Engine` resolved, boxed so the enum stays cheap to move.
///
/// The two variants are deliberately different types: only the production
/// variant is a [`ResolvedGenerationContext`], so a code path that needs
/// production authority cannot accidentally accept the opt-in legacy identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineContext {
    Production(Box<ResolvedGenerationContext>),
    LegacyTest(Box<LegacyGenerationContext>),
}

impl EngineContext {
    /// The digest that authorizes candidate, cache, and resume reuse.
    pub fn digest(&self) -> &str {
        match self {
            EngineContext::Production(context) => &context.context_digest,
            EngineContext::LegacyTest(context) => &context.context_digest,
        }
    }

    /// Whether this identity may authorize production reuse.
    pub fn production_authority(&self) -> bool {
        matches!(self, EngineContext::Production(_))
    }

    /// The handshake payload for this identity.
    pub fn to_payload(&self) -> Value {
        match self {
            EngineContext::Production(context) => context.to_payload(),
            EngineContext::LegacyTest(context) => context.to_payload(),
        }
    }

    /// The pre-version digest, published only as an explicit proof field.
    pub fn legacy_context_digest(&self) -> &str {
        match self {
            EngineContext::Production(context) => &context.legacy_context_digest,
            EngineContext::LegacyTest(context) => &context.context_digest,
        }
    }
}

/// Owns the generation identity, the shipped schema and the single search job.
pub struct Engine {
    context: EngineContext,
    contract_digest: String,
    capabilities: Capabilities,
    schema: RequestSchema,
    materializer: Arc<Materializer>,
    jobs: Arc<JobStore>,
    /// The captured native recommended-level curve for `recommended_level.resolve`.
    recommended_level: RecommendedLevelCurve,
    /// The shipped name catalogs `search.catalog` renders from, loaded on first
    /// use so the other methods never pay for them.
    catalog: Mutex<Option<Arc<Catalog>>>,
    /// Product data root, kept for the lazily loaded catalog.
    data_root: PathBuf,
    negotiated: bool,
}

impl Engine {
    /// Resolve the generation context, the shipped contract digest and the real
    /// capability probes for one explicit [`ContextSelection`].
    ///
    /// A production selection binds the identity to the exact installed version
    /// and resolves the matching resource bundle; a missing or unknown version
    /// fails closed with `RESOURCE_MISMATCH` and never falls back to
    /// `CURRENT_RESOURCE_VERSION`. The legacy variant is the only path that may
    /// reproduce the pre-version identity, and it is non-production.
    pub fn load(
        data_root: &Path,
        contract_dir: &Path,
        accelerator_path: Option<PathBuf>,
        selection: ContextSelection,
    ) -> Result<Self, EngineError> {
        let contract_digest = contract_digest(contract_dir)?;
        let schema = RequestSchema::load(contract_dir)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error))?;
        let application_root = data_root
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        let accelerator = probe_seed_accelerator(application_root, accelerator_path.as_deref());
        let (context, resource_version) = match selection {
            ContextSelection::Production(file_version) => {
                let resolved = capture_resolved_context(
                    crate::context::SUPPORTED_GAME_PROFILE,
                    data_root,
                    file_version,
                    accelerator,
                )
                .map_err(from_context_error)?;
                let version = (
                    file_version.0,
                    file_version.1,
                    file_version.2,
                    file_version.3,
                );
                (EngineContext::Production(Box::new(resolved)), Some(version))
            }
            ContextSelection::LegacyTest => {
                let legacy = capture_legacy_context(
                    crate::context::SUPPORTED_GAME_PROFILE,
                    data_root,
                    accelerator,
                )
                .map_err(from_context_error)?;
                // The shipped legacy payload is the historical v2.00.02 tables.
                (
                    EngineContext::LegacyTest(Box::new(legacy)),
                    Some(nioh3_data::CURRENT_RESOURCE_VERSION),
                )
            }
        };
        let context_digest = context.digest().to_string();
        let capabilities = capabilities::probe(application_root, accelerator_path.as_deref(), None);
        let recommended_level = recommended_level::load(data_root)?;
        let materializer = Arc::new(Materializer::with_resource_version(
            data_root,
            &context_digest,
            resource_version,
        ));
        let jobs = JobStore::new(
            &context_digest,
            // The compiler reads the same resource bundle the materializer
            // composes from, so a search never filters on one version's
            // tables and publishes candidates built from another's.
            collector::native_factory(
                application_root,
                accelerator_path.as_deref(),
                data_root,
                materializer.resource_version(),
            ),
            Arc::clone(&materializer) as Arc<dyn CandidateSource>,
        )
        .ok_or_else(|| {
            EngineError::new(
                "RESOURCE_MISMATCH",
                "the operating system CSPRNG is unavailable; refusing to mint resume tokens",
            )
        })?;
        Ok(Self {
            context,
            contract_digest,
            capabilities,
            schema,
            materializer,
            jobs: Arc::new(jobs),
            recommended_level,
            catalog: Mutex::new(None),
            data_root: data_root.to_path_buf(),
            negotiated: false,
        })
    }

    pub fn context(&self) -> &EngineContext {
        &self.context
    }

    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }

    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    pub fn schema(&self) -> &RequestSchema {
        &self.schema
    }

    /// The single search job, for shutdown ordering and tests.
    pub fn jobs(&self) -> &Arc<JobStore> {
        &self.jobs
    }

    /// Dispatch one validated request, mirroring `search_worker.main`.
    pub fn dispatch(&mut self, request: Request) -> Outcome {
        let id = Value::String(request.id().to_string());
        match request {
            Request::Handshake { .. } => {
                self.negotiated = true;
                Outcome::Reply(payload::success_frame(
                    &id,
                    payload::handshake_result(
                        &self.contract_digest,
                        &self.context,
                        self.capabilities,
                    ),
                ))
            }
            Request::Shutdown { .. } => {
                self.jobs.shutdown();
                Outcome::Stop(payload::success_frame(
                    &id,
                    serde_json::json!({"stopped": true}),
                ))
            }
            Request::CandidatePreview {
                seed,
                rarity,
                level,
                ..
            } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.materializer.preview(seed, rarity, level) {
                    Ok(result) => Outcome::Reply(payload::success_frame(&id, result)),
                    Err(error) => {
                        Outcome::Reply(payload::error_frame(&id, error.code, &error.message))
                    }
                }
            }
            Request::SearchStart { params, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                let start = self.start_params(&params);
                let query = params.get("query").expect("schema requires the query");
                match self.jobs.start(query, &start) {
                    Ok(job) => Outcome::Reply(payload::success_frame(
                        &id,
                        payload::job_snapshot_json(&job),
                    )),
                    Err(error) => self.failure(&id, error),
                }
            }
            Request::JobCurrent { .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                let job = self.jobs.current();
                Outcome::Reply(payload::success_frame(
                    &id,
                    payload::current_job_json(job.as_ref()),
                ))
            }
            Request::JobSnapshot { job_id, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.jobs.snapshot(&job_id) {
                    Ok(job) => Outcome::Reply(payload::success_frame(
                        &id,
                        payload::job_snapshot_json(&job),
                    )),
                    Err(error) => self.failure(&id, error),
                }
            }
            Request::JobCancel { job_id, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.jobs.cancel(&job_id) {
                    Ok(job) => Outcome::Reply(payload::success_frame(
                        &id,
                        payload::job_snapshot_json(&job),
                    )),
                    Err(error) => self.failure(&id, error),
                }
            }
            Request::CandidateExport {
                job_id,
                candidate_id,
                ..
            } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.jobs.export(&job_id, &candidate_id) {
                    Ok(transfer) => Outcome::Reply(payload::success_frame(&id, transfer)),
                    Err(error) => self.failure(&id, error),
                }
            }
            Request::RecommendedLevelResolve {
                displayed_level, ..
            } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                Outcome::Reply(payload::success_frame(
                    &id,
                    self.recommended_level.resolve_payload(displayed_level),
                ))
            }
            Request::CacheRegister { cache_json, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.jobs.register_cache(&cache_json) {
                    Ok(result) => Outcome::Reply(payload::success_frame(&id, result)),
                    Err(error) => self.failure(&id, error),
                }
            }
            Request::SearchCatalog { rarity, locale, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.catalog_payload(rarity, &locale) {
                    Ok(result) => Outcome::Reply(payload::success_frame(&id, result)),
                    Err(error) => {
                        Outcome::Reply(payload::error_frame(&id, error.code, &error.message))
                    }
                }
            }
            Request::Unimplemented { method, .. } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                self.failure(&id, RequestError::unsupported_method(&method))
            }
        }
    }

    /// Error reply for a request that never reached the engine.
    pub fn error_reply(&self, id: &Value, error: &RequestError) -> Value {
        payload::error_frame(id, error.code, &error.message)
    }

    /// Stop the job thread before the process exits.
    pub fn shutdown(&self) {
        self.jobs.shutdown();
    }

    fn failure(&self, id: &Value, error: RequestError) -> Outcome {
        Outcome::Reply(payload::error_frame(id, error.code, &error.message))
    }

    /// The bundled name catalogs, loaded once on first use.
    fn catalog(&self) -> Result<Arc<Catalog>, EngineError> {
        let mut slot = self.catalog.lock().expect("catalog cache");
        if let Some(catalog) = slot.as_ref() {
            return Ok(Arc::clone(catalog));
        }
        let catalog = Arc::new(Catalog::load(&self.data_root)?);
        *slot = Some(Arc::clone(&catalog));
        Ok(catalog)
    }

    /// `search.catalog`, composed from the loaded tables and name catalogs.
    fn catalog_payload(&self, rarity: u8, locale: &str) -> Result<Value, EngineError> {
        let catalog = self.catalog()?;
        self.materializer.inspect(|index, effect, preview| {
            catalog.payload(catalog::CatalogInputs {
                context_digest: self.context.digest(),
                rarity,
                locale,
                index,
                effect,
                preview,
                recommended_level: &self.recommended_level,
            })
        })
    }

    /// Read the schema-checked `search.start` params into typed job arguments.
    fn start_params(&self, params: &Value) -> StartParams {
        let integer = |key: &str| -> u64 {
            crate::schema::integral(params.get(key).expect("schema requires the key"))
                .expect("schema requires an integer") as u64
        };
        StartParams {
            context_digest: params
                .get("context_digest")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            result_count: integer("result_count") as usize,
            page_trials: integer("page_trials"),
            job_trials: integer("job_trials"),
            continue_until_complete: params
                .get("continue_until_complete")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            allow_cpu_fallback: params
                .get("allow_cpu_fallback")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            resume_token: params
                .get("resume_token")
                .and_then(Value::as_str)
                .map(str::to_string),
            cache_id: params
                .get("cache_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

/// Build the certified sequence the preview reports for `rarity`.
fn build_sequence(
    rarity: u8,
    seed: u32,
    level: u16,
    index: &EffectTableIndex,
    effect: &EffectResourceBytes,
    playthrough: u8,
    cached_grace: Option<&nioh3_domain::effect::GraceMap>,
) -> Result<ScrollRecord, EngineError> {
    match rarity {
        3 => generate_ng3_rarity3_effect_sequence(index, seed, level)
            .map_err(|error| EngineError::new("INVALID_REQUEST", format!("{error:?}"))),
        4 => {
            let mut template = ScrollRecordBytes::zeroed();
            template
                .write_u16(0x00, NG3_RECORD_TYPE)
                .map_err(|error| EngineError::new("INVALID_REQUEST", format!("{error:?}")))?;
            let pair = materialize_ng3_rarity4_final_record(
                index,
                &effect.grace_maps[0],
                &template,
                seed,
                level,
                0,
                0,
                0,
            )
            .map_err(|error| EngineError::new("INVALID_REQUEST", format!("{error:?}")))?;
            Ok(pair.preview_sequence().clone())
        }
        5 => generate_rarity5_grace_effect_sequence(
            index,
            cached_grace.unwrap_or(&effect.grace_maps[1]),
            playthrough,
            seed,
            level,
        )
        .map_err(|error| EngineError::new("INVALID_REQUEST", format!("{error:?}"))),
        other => Err(EngineError::new(
            "INVALID_REQUEST",
            format!("certified offline preview supports rarity 3, 4 or 5, not {other}"),
        )),
    }
}

/// `CONTRACT_DIGEST` = SHA-256 of the request schema bytes then the response
/// schema bytes, read verbatim from the shipped contract files.
pub fn contract_digest(contract_dir: &Path) -> Result<String, EngineError> {
    let request = fs::read(contract_dir.join("request.schema.json"))
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
    let response = fs::read(contract_dir.join("response.schema.json"))
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(&request);
    digest.update(&response);
    Ok(hex_lower(&digest.finalize()))
}

fn from_context_error(error: ContextError) -> EngineError {
    EngineError::new(error.code, error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_digest_matches_the_shipped_contract_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
        let digest = contract_digest(&dir).expect("read shipped contract files");
        let mut expected = Sha256::new();
        expected.update(fs::read(dir.join("request.schema.json")).expect("request schema"));
        expected.update(fs::read(dir.join("response.schema.json")).expect("response schema"));
        assert_eq!(digest, hex_lower(&expected.finalize()));
        assert_eq!(digest.len(), 64);
    }

    /// The auxiliary half decides the caller's auxiliary criteria, so a match
    /// those criteria reject is refused before the record, the enemy-state half
    /// and the payload are composed. A matching criterion still composes the
    /// whole candidate, so nothing is skipped for an accepted match.
    #[test]
    fn an_auxiliary_rejection_skips_the_rest_of_the_composition() {
        use crate::query::SearchQuery;

        let data_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        let materializer = Materializer::new(&data_root, "engine-test-context");

        let payload = |rule_keys: Value, rule_groups: Value| {
            serde_json::json!({
                "playthrough": 3,
                "rarity": 4,
                "level": 180,
                "primary_effect_ids": [],
                "required_secondary_ids": [],
                "required_secondary_id_groups": [],
                "grace_effect_id": null,
                "minimum_roll_percent_by_effect_id": [],
                "auxiliary": {
                    "required_terrain_effect_keys": [],
                    "required_terrain_effect_key_groups": [],
                    "required_special_rule_keys": rule_keys,
                    "required_special_rule_key_groups": rule_groups,
                    "required_enemy_lookup_keys": [],
                    "required_enemy_lookup_key_groups": [],
                },
            })
        };

        // Seed 226061463 is the shipped canonical three-rule match (rules
        // 64956 / 113 / 20893), so it satisfies those keys exactly.
        let seed = 226061463u32;
        let accepted = SearchQuery::from_payload(&payload(
            serde_json::json!([64956, 113, 20893]),
            serde_json::json!([]),
        ))
        .expect("accepted query");
        let rejected =
            SearchQuery::from_payload(&payload(serde_json::json!([]), serde_json::json!([[1]])))
                .expect("rejected query");

        let accepted_match = materializer
            .materialize(&accepted, seed, 158614759, None)
            .expect("the shipped canonical match composes");
        assert_eq!(accepted_match.auxiliary_match, Some(true));
        assert!(
            !accepted_match.payload.is_null(),
            "an accepted match must still carry its composed payload"
        );
        assert!(
            !accepted_match.candidate.effects.is_empty(),
            "an accepted match must still carry its effect sequence"
        );

        let rejected_match = materializer
            .materialize(&rejected, seed, 158614759, None)
            .expect("an auxiliary rejection is not an error");
        assert_eq!(rejected_match.auxiliary_match, Some(false));
        assert!(
            rejected_match.payload.is_null(),
            "a rejected match must not pay for the payload composition"
        );
        assert!(
            rejected_match.candidate.effects.is_empty(),
            "a rejected match must not pay for the effect sequence"
        );
    }

    /// The release default composes a completed preview from the versioned
    /// PC v2.02 tables, while an explicit legacy selector still uses the
    /// shipped PC v2.00.02 payload.
    #[test]
    fn completed_preview_uses_the_release_version_and_legacy_stays_explicit() {
        let data_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        let current = Materializer::new(&data_root, "engine-test-context");
        assert_eq!(
            current.resource_version(),
            Some(nioh3_data::CURRENT_RESOURCE_VERSION)
        );
        let payload = current
            .preview(10030700, 4, 180)
            .expect("completed preview");
        assert!(
            payload.is_object(),
            "the release preview must produce a composed record: {payload}"
        );
        assert_eq!(
            nioh3_data::r4_resource_dir_for_file_version(nioh3_data::CURRENT_RESOURCE_VERSION)
                .expect("release resource"),
            nioh3_data::R4_RESOURCE_DIR_V202
        );

        let legacy = Materializer::with_resource_version(&data_root, "engine-test-context", None);
        assert_eq!(legacy.resource_version(), None);
        let legacy_payload = legacy.preview(10030700, 4, 180).expect("legacy preview");
        assert!(
            legacy_payload.is_object(),
            "the explicit legacy selector must still compose a record"
        );
    }

    /// The version selects the ENGINE's effect tables, not only the preview
    /// tables.  PC v2.02 owns a 2954-row `optional_multiplier` table while the
    /// shipped PC v2.00.02 payload has 2951 rows, so the two loads are
    /// distinguishable from the rows the executor actually holds.
    #[test]
    fn resource_version_selects_the_engine_effect_tables() {
        use std::hash::{Hash, Hasher};

        let data_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");

        let observed = |materializer: &Materializer| {
            materializer
                .inspect(|_index, effect, _preview| {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    effect.item.rows.hash(&mut hasher);
                    Ok((
                        effect.optional_multiplier.row_count(),
                        effect.item.row_count(),
                        hasher.finish(),
                    ))
                })
                .expect("resource loads")
        };

        let (current_optional, current_item, current_item_hash) =
            observed(&Materializer::new(&data_root, "engine-test-context"));
        assert_eq!(
            current_optional, 2954,
            "PC v2.02 optional_multiplier row count"
        );

        let (legacy_optional, legacy_item, legacy_item_hash) = observed(
            &Materializer::with_resource_version(&data_root, "engine-test-context", None),
        );
        assert_eq!(
            legacy_optional, 2951,
            "shipped PC v2.00.02 optional_multiplier row count"
        );

        assert_eq!(
            current_item, legacy_item,
            "the item table keeps its row count"
        );
        assert_ne!(
            current_item_hash, legacy_item_hash,
            "PC v2.02 changed the item payload, so the loaded rows must differ"
        );
    }

    /// Both shipped identities stay pinned, and the version-bound digest is the
    /// value that authorizes reuse: two resolutions that select different
    /// bundles never share a `context_digest`, so a candidate minted for one
    /// version is refused by a job store opened for the other. The same explicit
    /// context still composes byte-identical output, so determinism is preserved
    /// per identity rather than weakened.
    #[test]
    fn selected_versions_do_not_share_production_authority() {
        let data_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        let v2_00_02 = capture_resolved_context(
            crate::context::SUPPORTED_GAME_PROFILE,
            &data_root,
            GameFileVersion(2, 0, 0, 2),
            None,
        )
        .expect("the shipped v2.00.02 bundle resolves");
        let v2_02 = capture_resolved_context(
            crate::context::SUPPORTED_GAME_PROFILE,
            &data_root,
            GameFileVersion(2, 0, 2, 0),
            None,
        )
        .expect("the shipped v2.02 bundle resolves");

        // Both identities are pinned to the shipped selected-bundle digests.
        assert_eq!(v2_00_02.versioned_resource_dir, nioh3_data::R4_RESOURCE_DIR);
        assert_eq!(
            v2_02.versioned_resource_dir,
            nioh3_data::R4_RESOURCE_DIR_V202
        );
        assert_eq!(
            v2_00_02.context_digest,
            "d866b3445427d264dfd58b4da29681075d8b1d8d227c67c3b4e409bc10f1174c"
        );
        assert_eq!(
            v2_02.context_digest,
            "6f1292895f25937005f736b3170ccfd11b295aa7c284d3744339f6bbfd1a8712"
        );
        // The pre-version proof field is shared; only the primary digest differs,
        // which is exactly why the legacy digest cannot authorize reuse.
        assert_eq!(v2_00_02.legacy_context_digest, v2_02.legacy_context_digest);
        assert_ne!(v2_00_02.context_digest, v2_02.context_digest);

        // A job store opened for one version refuses the other's digest. Both
        // stores are fed the same in-process collector, so only the identity
        // differs.
        let open = |digest: &str| -> Arc<JobStore> {
            Arc::new(
                JobStore::new(
                    digest,
                    None,
                    Arc::new(Materializer::with_resource_version(
                        &data_root,
                        digest,
                        Some(nioh3_data::CURRENT_RESOURCE_VERSION),
                    )) as Arc<dyn CandidateSource>,
                )
                .expect("an OS CSPRNG is available"),
            )
        };
        let store = open(&v2_02.context_digest);
        let query = serde_json::json!({
            "playthrough": 3,
            "rarity": 4,
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
        });
        let mismatched = StartParams {
            context_digest: v2_00_02.context_digest.clone(),
            result_count: 1,
            page_trials: 1000,
            job_trials: 1000,
            continue_until_complete: false,
            allow_cpu_fallback: false,
            resume_token: None,
            cache_id: None,
        };
        assert_eq!(
            store
                .start(&query, &mismatched)
                .expect_err("a foreign context digest must be refused")
                .code,
            "CONTEXT_MISMATCH"
        );

        // Determinism per identity: the same explicit context composes the same
        // record bytes twice, so binding the version did not perturb RNG output.
        let materializer = |digest: &str, version: (u16, u16, u16, u16)| {
            Materializer::with_resource_version(&data_root, digest, Some(version))
        };
        let v2_02_materializer = materializer(&v2_02.context_digest, (2, 0, 2, 0));
        let first = v2_02_materializer
            .preview(10030700, 4, 180)
            .expect("v2.02 preview");
        let second = v2_02_materializer
            .preview(10030700, 4, 180)
            .expect("v2.02 preview again");
        assert_eq!(
            first, second,
            "the same explicit context must stay deterministic"
        );
        let v2_00_02_first = materializer(&v2_00_02.context_digest, (2, 0, 0, 2))
            .preview(10030700, 4, 180)
            .expect("v2.00.02 preview");
        assert_ne!(
            first, v2_00_02_first,
            "the two selected versions must not compose identical bytes"
        );
    }
}
