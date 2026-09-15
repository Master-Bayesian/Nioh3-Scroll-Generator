//! Response assembly for the read-only preview worker.
//!
//! Mirrors `worker_contracts.candidate_payload`, `candidate_transfer.export_candidate`
//! and `search_worker.handshake` field for field. Private record bytes only ever
//! appear in the broker-facing `transfer` block, and only for the fields the
//! shipped preview actually carries.

use nioh3_domain::enemy::{MissionVariant, Possession};
use nioh3_domain::preview::{
    AuxiliaryPreview, CurseConditional, EnemyStatePreview, Ng3PreviewComposition,
    OccurrenceAvailability,
};
use serde_json::{json, Map, Value};

use crate::context::GenerationContext;
use crate::model::{candidate_identity, Candidate, CandidateEffect};

/// Evidence label the offline preview path reports.
pub const CERTIFIED_OFFLINE_REPLAY: &str = "certified_offline_replay";

/// The handshake result the development worker publishes.
///
/// The capability block states the real subset: exact CPU replay of the
/// offline preview, no GPU helper, no search orchestration, no cache and no
/// write path. Absent optional fields are omitted rather than faked.
pub fn handshake_result(contract_digest: &str, context: &GenerationContext) -> Value {
    json!({
        "protocol": crate::protocol::PROTOCOL_VERSION,
        "contract_digest": contract_digest,
        "role": "offline_search",
        "context": context.to_payload(),
        "capabilities": {
            "playthroughs": [3],
            "rarities": [3, 4, 5],
            "cuda_pivot_and_auxiliary": false,
            "directcompute_effect_filter": false,
            "cpu_exact_replay": true,
            "bulk_cpu_requires_opt_in": false,
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
    composition: &Ng3PreviewComposition,
) -> Value {
    let candidate_id = candidate_identity(candidate, context_digest);
    let blocker = candidate.install_blocker();
    let installable = blocker.is_none();
    let effects: Vec<Value> = candidate.effects.iter().map(effect_json).collect();

    let candidate_payload = json!({
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
        "enemy_states": enemy_states_json(&composition.enemy_states),
        "cursor": Value::Null,
        "evidence": CERTIFIED_OFFLINE_REPLAY,
        "installation_available": installable,
        "initial_challenge_capacity": composition.initial_challenge_capacity,
    });

    let transfer = json!({
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
    });

    json!({"candidate": candidate_payload, "transfer": transfer})
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
