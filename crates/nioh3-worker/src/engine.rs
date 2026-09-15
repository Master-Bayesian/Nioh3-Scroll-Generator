//! Application engine: context capture, contract identity, and the supported
//! read-only preview path.
//!
//! Resource loading and pure composition belong to `nioh3-data` and
//! `nioh3-domain`; this module only sequences them and binds the result to the
//! generation context.

use std::fs;
use std::path::{Path, PathBuf};

use nioh3_data::{load_effect_resource, load_preview_resources, PreviewResources};
use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::preview::{compose_ng3_preview, effect_previews, PreviewTables};
use nioh3_domain::record::{materialize_ng3_rarity4_final_record, ScrollRecord, ScrollRecordBytes};
use nioh3_domain::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_ng3_rarity5_effect_sequence, NG3_RECORD_TYPE,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::context::{capture_context, hex_lower, ContextError, GenerationContext};
use crate::model::{Candidate, CandidateEffect, RecordStage};
use crate::native::probe_seed_accelerator;
use crate::payload;
use crate::protocol::{Request, RequestError};

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

/// Lazily loaded resource set for the preview path.
struct LoadedResources {
    index: EffectTableIndex,
    effect: EffectResourceBytes,
    preview: PreviewResources,
}

/// Owns the generation identity and the supported request subset.
pub struct Engine {
    context: GenerationContext,
    contract_digest: String,
    data_root: PathBuf,
    resources: Option<LoadedResources>,
    negotiated: bool,
}

impl Engine {
    /// Capture the generation context and the shipped contract digest.
    pub fn load(
        data_root: &Path,
        contract_dir: &Path,
        accelerator_path: Option<PathBuf>,
    ) -> Result<Self, EngineError> {
        let contract_digest = contract_digest(contract_dir)?;
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
        Ok(Self {
            context,
            contract_digest,
            data_root: data_root.to_path_buf(),
            resources: None,
            negotiated: false,
        })
    }

    pub fn context(&self) -> &GenerationContext {
        &self.context
    }

    pub fn contract_digest(&self) -> &str {
        &self.contract_digest
    }

    /// Dispatch one validated request, mirroring `search_worker.main`.
    pub fn dispatch(&mut self, request: Request) -> Outcome {
        match request {
            Request::Handshake { id } => {
                self.negotiated = true;
                Outcome::Reply(payload::success_frame(
                    &Value::String(id),
                    payload::handshake_result(&self.contract_digest, &self.context),
                ))
            }
            Request::Shutdown { id } => Outcome::Stop(payload::success_frame(
                &Value::String(id),
                serde_json::json!({"stopped": true}),
            )),
            Request::Unsupported { id, method } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                self.failure(&id, RequestError::unsupported_method(&method))
            }
            Request::CandidatePreview {
                id,
                seed,
                rarity,
                level,
            } => {
                if !self.negotiated {
                    return self.failure(&id, RequestError::handshake_required());
                }
                match self.preview(seed, rarity, level) {
                    Ok(result) => {
                        Outcome::Reply(payload::success_frame(&Value::String(id), result))
                    }
                    Err(error) => Outcome::Reply(payload::error_frame(
                        &Value::String(id),
                        error.code,
                        &error.message,
                    )),
                }
            }
        }
    }

    /// Error reply for a request that never reached the engine.
    pub fn error_reply(&self, id: &Value, error: &RequestError) -> Value {
        payload::error_frame(id, error.code, &error.message)
    }

    fn failure(&self, id: &str, error: RequestError) -> Outcome {
        Outcome::Reply(payload::error_frame(
            &Value::String(id.to_string()),
            error.code,
            &error.message,
        ))
    }

    fn preview(&mut self, seed: u32, rarity: u8, level: u16) -> Result<Value, EngineError> {
        if self.resources.is_none() {
            self.resources = Some(self.load_resources()?);
        }
        let resources = self.resources.as_ref().expect("resources were just loaded");

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
        };

        let tables = PreviewTables {
            roster: &resources.preview.roster,
            context: &resources.preview.context,
            rules: &resources.preview.rules,
            states: &resources.preview.states,
        };
        let composition = compose_ng3_preview(seed, NG3_PLAYTHROUGH, &tables)
            .map_err(|error| EngineError::new("UNSUPPORTED_CONTEXT", format!("{error:?}")))?;

        Ok(payload::preview_result(
            &candidate,
            &self.context.context_digest,
            level,
            &composition,
        ))
    }

    fn load_resources(&self) -> Result<LoadedResources, EngineError> {
        let effect = load_effect_resource(&self.data_root)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        let index = EffectTableIndex::from_resource(&effect)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", format!("{error:?}")))?;
        let preview = load_preview_resources(&self.data_root)
            .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
        Ok(LoadedResources {
            index,
            effect,
            preview,
        })
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
}
