//! Response assembly for the read-only preview worker.
//!
//! Mirrors `worker_contracts.candidate_payload`, `candidate_transfer.export_candidate`
//! and `search_worker.handshake` field for field. Private record bytes only ever
//! appear in the broker-facing `transfer` block, and only for the fields the
//! shipped preview actually carries.

use nioh3_domain::enemy::{MissionVariant, Possession};
use nioh3_domain::preview::{
    AuxiliaryPreview, CurseConditional, EnemyStatePreview, OccurrenceAvailability,
};
use serde_json::{json, Map, Value};

use crate::capabilities::Capabilities;
use crate::context::GenerationContext;
use crate::jobs::JobView;
use crate::model::{candidate_identity, Candidate, CandidateEffect};

/// Evidence label the offline preview path reports.
pub const CERTIFIED_OFFLINE_REPLAY: &str = "certified_offline_replay";

/// `cached_rarity5_playthroughs`: the save-bound cache route serves NG4 and NG5.
///
/// Mirrors the shipped worker's handshake value. It is published because the
/// route is ported (`SearchFactory::cached_collector` compiles the registered
/// map into the native collector), not merely because a map can be registered.
pub const CACHED_RARITY5_PLAYTHROUGHS: [u8; 2] = [4, 5];

/// The handshake result the development worker publishes.
///
/// The capability block states the real subset: exact CPU replay of the
/// offline preview, no GPU helper, no search orchestration, no cache and no
/// write path. Absent optional fields are omitted rather than faked.
pub fn handshake_result(
    contract_digest: &str,
    context: &GenerationContext,
    capabilities: Capabilities,
) -> Value {
    json!({
        "protocol": crate::protocol::PROTOCOL_VERSION,
        "contract_digest": contract_digest,
        "role": "offline_search",
        "context": context.to_payload(),
        "capabilities": {
            "playthroughs": [3],
            "rarities": [3, 4, 5],
            // Real probes: the shipped seed-accelerator and effect-preimage
            // helpers are loaded and asked, exactly like the Python worker.
            "cuda_pivot_and_auxiliary": capabilities.cuda_pivot_and_auxiliary,
            "directcompute_effect_filter": capabilities.directcompute_effect_filter,
            // The save-bound NG4/NG5 cached route is ported, so the playthroughs
            // it serves are advertised exactly like the shipped worker's. NG3
            // keeps using its certified bundled map and never takes a cache.
            "cached_rarity5_playthroughs": CACHED_RARITY5_PLAYTHROUGHS,
            "cpu_exact_replay": true,
            "bulk_cpu_requires_opt_in": capabilities.bulk_cpu_requires_opt_in,
            "save_write": false,
            "runtime_calls": false,
        },
    })
}

/// A success frame: `{"protocol":1,"id":<id>,"ok":true,"result":<result>}`.
pub fn success_frame(id: &Value, result: Value) -> Value {
    json!({
        "protocol": crate::protocol::PROTOCOL_VERSION,
        "id": id,
        "ok": true,
        "result": result,
    })
}

/// An error frame: `{"protocol":1,"id":<id>,"ok":false,"error":{"code","message"}}`.
pub fn error_frame(id: &Value, code: &str, message: &str) -> Value {
    json!({
        "protocol": crate::protocol::PROTOCOL_VERSION,
        "id": id,
        "ok": false,
        "error": {"code": code, "message": message},
    })
}

/// Build the `candidate.preview` result for one composed candidate.
pub fn preview_result(
    candidate: &Candidate,
    context_digest: &str,
    level: u16,
    composition: &crate::engine::ComposedPreview,
) -> Value {
    json!({
        "candidate": candidate_payload_json(candidate, context_digest, composition),
        "transfer": transfer_json(candidate, context_digest, level),
    })
}

/// `worker_contracts.candidate_payload`: one job candidate entry.
///
/// `cursor` is the 1-based solver trial the candidate came from, matching
/// `candidate.joint_search_trial`; the standalone preview path leaves it null.
pub fn candidate_payload_json(
    candidate: &Candidate,
    context_digest: &str,
    composition: &crate::engine::ComposedPreview,
) -> Value {
    let candidate_id = candidate_identity(candidate, context_digest);
    let blocker = candidate.install_blocker();
    let installable = blocker.is_none();
    let effects: Vec<Value> = candidate.effects.iter().map(effect_json).collect();

    json!({
        "candidate_id": candidate_id,
        "context_digest": context_digest,
        "seed": candidate.seed,
        "playthrough": candidate.playthrough,
        "rarity": candidate.rarity,
        "record_stage": candidate.record_stage.as_str(),
        "installable": installable,
        "install_blocker": blocker,
        "effects": effects,
        "auxiliary": auxiliary_json(&composition.auxiliary),
        "enemy_states": match &composition.enemy_states {
            Some(states) => enemy_states_json(states),
            None => Value::Null,
        },
        "cursor": candidate.joint_search_trial,
        "evidence": CERTIFIED_OFFLINE_REPLAY,
        "installation_available": installable,
        "initial_challenge_capacity": composition.initial_challenge_capacity,
    })
}

/// `candidate_transfer.export_candidate`: the broker-only transfer block.
pub fn transfer_json(candidate: &Candidate, context_digest: &str, level: u16) -> Value {
    let candidate_id = candidate_identity(candidate, context_digest);
    let effects: Vec<Value> = candidate.effects.iter().map(effect_json).collect();
    json!({
        "candidate_id": candidate_id,
        "context_digest": context_digest,
        "level": level,
        "seed": candidate.seed,
        "playthrough": candidate.playthrough,
        "rarity": candidate.rarity,
        "record_stage": candidate.record_stage.as_str(),
        "record_hex": hex_encode(&candidate.record),
        "installation_record_hex": candidate
            .installation_record
            .as_deref()
            .map(hex_encode)
            .map(Value::String)
            .unwrap_or(Value::Null),
        "effects": effects,
    })
}

/// One detached `JobSnapshot`, exactly the shipped field set.
pub fn job_snapshot_json(view: &JobView) -> Value {
    let progress = match &view.progress {
        None => Value::Null,
        Some(report) => json!({
            "start_after_trial": report.start_after_trial,
            "inspected_through_trial": report.inspected_through_trial,
            "family_size": report.family_size,
            "fixed_seed_count": report.fixed_seed_count,
            "stages": report
                .stages
                .iter()
                .map(|stage| json!({
                    "kind": stage.kind,
                    "values": stage.values,
                    "count": stage.count,
                }))
                .collect::<Vec<Value>>(),
            "complete_match_count": report.complete_match_count,
            "exhausted_family": report.exhausted_family,
        }),
    };
    json!({
        "job_id": view.job_id,
        "state": view.state,
        "sequence": view.sequence,
        "query_digest": view.query_digest,
        "context_digest": view.context_digest,
        "cursor": view.cursor,
        "start_cursor": view.start_cursor,
        "candidates": view.candidates,
        "progress": progress,
        "stop_reason": view.stop_reason,
        "error": match &view.error {
            None => Value::Null,
            Some((code, message)) => json!({"code": code, "message": message}),
        },
        "resume_token": view.resume_token,
        "elapsed_ms": view.elapsed_ms,
    })
}

/// `job.current`: the latest job, or null before the first search.
pub fn current_job_json(view: Option<&JobView>) -> Value {
    json!({
        "job": match view {
            None => Value::Null,
            Some(view) => job_snapshot_json(view),
        },
    })
}

fn effect_json(effect: &CandidateEffect) -> Value {
    json!({
        "slot": effect.slot,
        "effect_id": effect.effect_id,
        "value": effect.value,
        "metadata": effect.metadata,
        "prefix": effect.prefix,
        "tail_0": effect.tail_0,
        "tail_1": effect.tail_1,
        "roll_percent": effect.roll_percent,
    })
}

fn auxiliary_json(auxiliary: &AuxiliaryPreview) -> Value {
    let enemy_groups: Vec<Value> = auxiliary
        .enemy_groups
        .iter()
        .map(|group| {
            Value::Array(
                group
                    .entries
                    .iter()
                    .map(|entry| {
                        json!({
                            "lookup_key": entry.lookup_key,
                            "role": entry.role,
                        })
                    })
                    .collect(),
            )
        })
        .collect();
    let special_rules: Vec<Value> = auxiliary
        .special_rules
        .entries
        .iter()
        .map(|entry| {
            // `raw_value` is a binary32 in the reference and is serialized
            // through its exact `f64` widening, which is what Python's
            // `json.dumps(float(f32))` prints.
            let raw_value = entry.raw_value.map(f64::from);
            json!({
                "key": entry.key,
                "raw_value": raw_value,
                "display_value": entry.display_value,
                "display_unit": entry.display_unit,
                "display_grade": entry.display_grade,
                "qualifier_kind": entry.qualifier_kind,
                "qualifier_key": entry.qualifier_key,
            })
        })
        .collect();
    json!({
        "terrain": {
            "value": auxiliary.terrain.value,
            "display_effect_keys": auxiliary.terrain.display_effect_keys,
        },
        "enemy_groups": enemy_groups,
        "special_rules": special_rules,
    })
}

fn enemy_states_json(states: &[EnemyStatePreview; 2]) -> Value {
    let mut object = Map::new();
    for state in states {
        let key = match state.variant {
            MissionVariant::Solo => "solo",
            MissionVariant::Expedition => "expedition",
        };
        object.insert(key.to_string(), enemy_state_json(state));
    }
    Value::Object(object)
}

fn enemy_state_json(state: &EnemyStatePreview) -> Value {
    let occurrences: Vec<Value> = state
        .occurrences
        .iter()
        .map(|occurrence| {
            json!({
                "wave_index": occurrence.wave_index,
                "position": occurrence.position,
                "lookup_key": occurrence.lookup_key,
                "role": occurrence.role,
                "source_row_index": occurrence.source_row_index,
                "availability": match occurrence.availability {
                    OccurrenceAvailability::Base => "base",
                    OccurrenceAvailability::ExpeditionOnly => "expedition_only",
                },
                "native_spawn_key": occurrence.native_spawn_key,
                "possessed": match occurrence.possessed {
                    Possession::Yes => "yes",
                    Possession::No => "no",
                    Possession::Unknown => "unknown",
                },
                // The shipped preview dataclass leaves these at their defaults.
                "curse": "unknown",
                "curse_probability": Value::Null,
                "curse_if_fresh_null_source_selector_runs": match occurrence
                    .curse_if_fresh_null_source_selector_runs
                {
                    CurseConditional::Guaranteed => "guaranteed",
                    CurseConditional::Never => "never",
                    CurseConditional::Unknown => "unknown",
                },
                "evidence_grade": "static_replay",
                "curse_evidence": "unknown",
            })
        })
        .collect();
    json!({
        "seed": state.seed,
        "playthrough": state.playthrough,
        "variant": match state.variant {
            MissionVariant::Solo => "solo",
            MissionVariant::Expedition => "expedition",
        },
        "terrain": state.terrain,
        "occurrences": occurrences,
        "possessed_complete": state.possessed_complete,
        "missing_inputs": state.missing_inputs,
        "curse_scope": state.curse_scope,
        "curse_count_range": Value::Null,
        "curse_count_domain": Value::Null,
    })
}

/// Lower-case hex, matching Python's `bytes.hex()`.
fn hex_encode(bytes: &[u8]) -> String {
    crate::context::hex_lower(bytes)
}
