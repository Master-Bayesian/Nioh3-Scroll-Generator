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
    Ng3PreviewComposition, PreviewTables,
};
use nioh3_domain::record::{materialize_ng3_rarity4_final_record, ScrollRecord, ScrollRecordBytes};
use nioh3_domain::sequence::{
    generate_challenge_attempt_count, generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity5_effect_sequence, NG3_RECORD_TYPE,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::capabilities::{self, Capabilities};
use crate::collector::{self, CandidateSource, CollectorError, MaterializedCandidate};
use crate::context::{capture_context, hex_lower, ContextError, GenerationContext};
use crate::jobs::{JobStore, StartParams};
use crate::model::{Candidate, CandidateEffect, RecordStage};
use crate::native::probe_seed_accelerator;
use crate::payload;
use crate::protocol::{Request, RequestError};
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
    resources: Mutex<Option<Arc<LoadedResources>>>,
}

impl Materializer {
    pub fn new(data_root: &Path, context_digest: &str) -> Self {
        Self {
            data_root: data_root.to_path_buf(),
            context_digest: context_digest.to_string(),
            resources: Mutex::new(None),
        }
    }

    fn resources(&self) -> Result<Arc<LoadedResources>, EngineError> {
        let mut slot = self.resources.lock().expect("resource cache");
        if let Some(resources) = slot.as_ref() {
            return Ok(Arc::clone(resources));
        }
        let effect = load_effect_resource(&self.data_root)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        let index = EffectTableIndex::from_resource(&effect)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", format!("{error:?}")))?;
        let preview = load_preview_resources(&self.data_root)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        let resources = Arc::new(LoadedResources {
            index,
            effect,
            preview,
        });
        *slot = Some(Arc::clone(&resources));
        Ok(resources)
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
    ) -> Result<(Candidate, Value, Value, Ng3PreviewComposition), EngineError> {
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
    ) -> Result<(Candidate, Value, Value, Ng3PreviewComposition), EngineError> {
        let composition = Ng3PreviewComposition {
            auxiliary,
            enemy_states: [
                compose_enemy_state_preview(seed, NG3_PLAYTHROUGH, MissionVariant::Solo, tables)
                    .map_err(unsupported_preview)?,
                compose_enemy_state_preview(
                    seed,
                    NG3_PLAYTHROUGH,
                    MissionVariant::Expedition,
                    tables,
                )
                .map_err(unsupported_preview)?,
            ],
            initial_challenge_capacity: generate_challenge_attempt_count(seed),
        };

        let record = build_sequence(rarity, seed, level, &resources.index, &resources.effect)?;
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
            playthrough: Some(NG3_PLAYTHROUGH),
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
    fn materialize(
        &self,
        query: &crate::query::SearchQuery,
        seed: u32,
        trial: u64,
    ) -> Result<MaterializedCandidate, CollectorError> {
        let resources = self
            .resources()
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        let tables = preview_tables(&resources);

        // Cheap pre-acceptance stage. The caller's terrain / special-rule /
        // enemy criteria are decided by the auxiliary half alone, so they are
        // evaluated before the record, the enemy-state half and the payload are
        // composed. A match those criteria already exclude is rejected without
        // paying for the full preview, exactly as the shipped worker never
        // composes a payload for a candidate its own filter drops. Anything the
        // auxiliary half itself cannot compose still fails the job closed.
        let auxiliary = compose_auxiliary_preview(seed, NG3_PLAYTHROUGH, &tables)
            .map_err(unsupported_preview)
            .map_err(|error| CollectorError::new(error.code, error.message))?;
        let auxiliary_match = auxiliary_criteria_match(query, &auxiliary);
        if !auxiliary_match {
            return Ok(MaterializedCandidate {
                candidate: rejected_candidate(seed, query.rarity, trial),
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
fn rejected_candidate(seed: u32, rarity: u8, trial: u64) -> Candidate {
    Candidate {
        seed,
        playthrough: Some(NG3_PLAYTHROUGH),
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
    composition: &nioh3_domain::preview::Ng3PreviewComposition,
) -> bool {
    use crate::query::{Availability, EnemyStateFilter, EnemyVariant};
    use nioh3_domain::enemy::Possession;
    use nioh3_domain::preview::OccurrenceAvailability;

    if query.enemy_occurrence_groups.is_empty() {
        return true;
    }
    let state = match query.enemy_variant {
        EnemyVariant::Solo => &composition.enemy_states[0],
        EnemyVariant::Expedition => &composition.enemy_states[1],
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

/// Owns the generation identity, the shipped schema and the single search job.
pub struct Engine {
    context: GenerationContext,
    contract_digest: String,
    capabilities: Capabilities,
    schema: RequestSchema,
    materializer: Arc<Materializer>,
    jobs: Arc<JobStore>,
    negotiated: bool,
}

impl Engine {
    /// Capture the generation context, the shipped contract digest and the real
    /// capability probes.
    pub fn load(
        data_root: &Path,
        contract_dir: &Path,
        accelerator_path: Option<PathBuf>,
    ) -> Result<Self, EngineError> {
        let contract_digest = contract_digest(contract_dir)?;
        let schema = RequestSchema::load(contract_dir)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error))?;
        let application_root = data_root
            .parent()
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."));
        let accelerator = probe_seed_accelerator(application_root, accelerator_path.as_deref());
        let context = capture_context(
            crate::context::SUPPORTED_GAME_PROFILE,
            data_root,
            accelerator,
        )
        .map_err(from_context_error)?;
        let capabilities = capabilities::probe(application_root, accelerator_path.as_deref(), None);
        let materializer = Arc::new(Materializer::new(data_root, &context.context_digest));
        let jobs = JobStore::new(
            &context.context_digest,
            collector::native_factory(application_root, accelerator_path.as_deref(), data_root),
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
            negotiated: false,
        })
    }

    pub fn context(&self) -> &GenerationContext {
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
        5 => generate_ng3_rarity5_effect_sequence(index, &effect.grace_maps[1], seed, level)
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
            .materialize(&accepted, seed, 158614759)
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
            .materialize(&rejected, seed, 158614759)
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
}
