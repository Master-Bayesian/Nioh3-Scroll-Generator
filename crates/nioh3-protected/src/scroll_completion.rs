//! Read-only, generation-context-bound ordinary scroll completion prediction.

use crate::HostError;
use nioh3_domain::{
    effect::EffectTableIndex,
    record::ScrollRecordBytes,
    scroll_completion::{predict, Replacement},
};
use nioh3_worker::{EngineContext, GameFileVersion};
use serde_json::{json, Value};
use std::path::Path;

fn effect(value: &Replacement) -> Value {
    json!({"slot":value.slot,"effect_id":value.effect_id,"roll":value.roll,"value":value.value})
}

pub fn prediction_json(
    data_root: &Path,
    context: &EngineContext,
    params: &Value,
) -> Result<Value, HostError> {
    let supported = matches!(context,EngineContext::Production(c) if c.game_file_version==GameFileVersion(2,0,2,0));
    if !supported || params["context_digest"].as_str() != Some(context.digest()) {
        return Err(HostError::rejected(
            "Scroll prediction requires the matching PC 2.0.2.0 context",
        ));
    }
    let text = params["record_hex"]
        .as_str()
        .ok_or_else(HostError::invalid_request)?;
    if text.len() != 0xE8 * 2 {
        return Err(HostError::invalid_request());
    }
    let bytes: Result<Vec<u8>, _> = text
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|v| u8::from_str_radix(v, 16).ok())
                .ok_or_else(HostError::invalid_request)
        })
        .collect();
    let record =
        ScrollRecordBytes::from_slice(&bytes?).map_err(|_| HostError::invalid_request())?;
    let resource = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0))
        .map_err(|e| HostError::rejected(format!("Scroll prediction tables unavailable: {e:?}")))?;
    let tables = EffectTableIndex::from_resource(&resource)
        .map_err(|e| HostError::rejected(format!("{e:?}")))?;
    let prediction = predict(&tables, &record).map_err(|e| {
        HostError::rejected(format!("Scroll completion prediction rejected: {e:?}"))
    })?;
    Ok(json!({"completion_prediction":{
        "ordinary_completion_only":true,"reveal_status":"unknown",
        "seed":record.displayed_seed(),"counter":record.completion_salt(),"pity":record.as_bytes()[50],"attempts":record.as_bytes()[51],
        "candidates":prediction.candidates.iter().map(effect).collect::<Vec<_>>(),
        "painting":{"eligible":prediction.painting.eligible,"draw":prediction.painting.draw,
            "threshold":prediction.painting.threshold,"success":prediction.painting.success},
        "branches":prediction.branches.iter().map(|branch|json!({"choice":branch.choice,
            "painting_effect":branch.painting_effect.as_ref().map(effect),
            "record_hex":branch.record.as_bytes().iter().map(|b|format!("{b:02x}")).collect::<String>()
        })).collect::<Vec<_>>()
    }}))
}
