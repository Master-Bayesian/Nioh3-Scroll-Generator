//! The matched-workload surface the scan benchmark drives.
//!
//! `tests/migration/test_runtime_scan_performance_parity.py` compares this
//! crate's scan and map routes against the shipped Python functions over an
//! oracle with identical behaviour, so it needs one entry point per route that
//! takes exactly the arguments the shipped functions take and returns exactly
//! the facts both sides can compare: the route result, the oracle call sequence
//! and the elapsed time (timed by the caller).
//!
//! Built only with `--features test-fake`; the packaged host never includes it.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

use crate::error::HostError;
use crate::grace_capture::build_live_grace_output_map;
use crate::maps::{prepare_maps, PreparedMaps};
use crate::oracle::scripted::ScriptedOracle;
use crate::scan::{
    scan_next_candidate, scan_seed_range, AuxiliaryCriteria, ScanAcceleration, ScanFilters,
    ScanRequest,
};

/// The deterministic oracle both halves of the comparison build.
#[derive(Debug, Clone)]
pub struct OracleSpec {
    pub template: Vec<u8>,
    pub rarity: u8,
    pub level: u16,
    pub recommended_level: u16,
    pub transfer_count: u32,
    pub max_batch_size: usize,
    pub remote_call_pending: bool,
}

impl OracleSpec {
    /// Read the oracle half of one benchmark spec.
    pub fn from_json(spec: &Value) -> Result<Self, String> {
        let template_hex = spec
            .get("template_hex")
            .and_then(Value::as_str)
            .ok_or("the spec needs template_hex")?;
        let number = |name: &str, default: u64| -> u64 {
            spec.get(name).and_then(Value::as_u64).unwrap_or(default)
        };
        Ok(Self {
            template: decode_hex(template_hex)?,
            rarity: u8::try_from(number("rarity", 4)).map_err(|_| "rarity is out of range")?,
            level: u16::try_from(number("level", 180)).map_err(|_| "level is out of range")?,
            recommended_level: u16::try_from(number("recommended_level", 183))
                .map_err(|_| "recommended_level is out of range")?,
            transfer_count: u32::try_from(number("transfer_count", 0))
                .map_err(|_| "transfer_count is out of range")?,
            max_batch_size: number("max_batch_size", 128) as usize,
            remote_call_pending: spec
                .get("remote_call_pending")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    /// The scripted oracle: unscripted seeds come from the shipped emitter.
    pub fn build(&self) -> ScriptedOracle {
        let mut oracle = ScriptedOracle::new(
            Vec::new(),
            self.template.clone(),
            self.rarity,
            self.level,
            self.recommended_level,
            self.transfer_count,
        );
        oracle.set_max_batch_size(self.max_batch_size);
        oracle.set_remote_call_pending(self.remote_call_pending);
        oracle
    }
}

impl ScanRequest {
    /// Read the scan half of one benchmark spec.
    pub fn from_bench_spec(spec: &Value) -> Result<Self, String> {
        let oracle = OracleSpec::from_json(spec)?;
        let number = |name: &str, default: u64| -> u64 {
            spec.get(name).and_then(Value::as_u64).unwrap_or(default)
        };
        let rarity = oracle.rarity;
        let playthrough = spec
            .get("playthrough")
            .and_then(Value::as_u64)
            .map(|value| value as u32);
        let mut filters = ScanFilters::new(rarity, playthrough);
        if let Some(criteria) = spec.get("criteria") {
            filters.primary_effect_ids = key_set(criteria, "primary_effect_ids");
            filters.required_secondary_ids = key_set(criteria, "required_secondary_ids");
            filters.grace_effect_id = criteria
                .get("grace_effect_id")
                .and_then(Value::as_u64)
                .map(|value| value as u32);
            if filters.grace_effect_id.is_some() {
                filters.grace_effect_slot = if rarity == 4 { 5 } else { 6 };
            }
            if let Some(auxiliary) = criteria.get("auxiliary") {
                filters.auxiliary = AuxiliaryCriteria::default();
                let _ = auxiliary;
            }
        }
        Ok(ScanRequest {
            template: oracle.template,
            start_seed: number("start_seed", 0) as u32,
            seed_step: 1,
            max_seeds: number("max_seeds", 1),
            level: oracle.level,
            recommended_level: oracle.recommended_level,
            transfer_count: oracle.transfer_count,
            filters,
            acceleration: ScanAcceleration::default(),
        })
    }
}

fn key_set(value: &Value, name: &str) -> BTreeSet<u32> {
    value
        .get(name)
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_u64)
                .map(|key| key as u32)
                .collect()
        })
        .unwrap_or_default()
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("a hex field has an odd length".to_string());
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len() / 2);
    let mut index = 0;
    while index < bytes.len() {
        let high = (bytes[index] as char)
            .to_digit(16)
            .ok_or("a hex field has a non-hex digit")?;
        let low = (bytes[index + 1] as char)
            .to_digit(16)
            .ok_or("a hex field has a non-hex digit")?;
        out.push((high * 16 + low) as u8);
        index += 2;
    }
    Ok(out)
}

/// `grace_map.build_live_grace_output_map` over the bench request.
pub fn run_grace_capture(
    oracle: &mut ScriptedOracle,
    request: &ScanRequest,
) -> Result<Value, HostError> {
    let category = request
        .filters
        .playthrough
        .ok_or_else(|| HostError::rejected("the capture needs a playthrough"))?;
    let mapping = build_live_grace_output_map(
        oracle,
        &request.template,
        category as u8,
        request.filters.rarity,
        request.level,
        request.recommended_level,
        request.transfer_count,
        &mut || false,
        &mut |_| {},
    )?;
    Ok(json!({
        "captured": true,
        "rarity": mapping.rarity,
        "effect_slot": mapping.effect_slot,
        "ranges": mapping.ranges.len(),
    }))
}

/// `native_search_maps.prepare_maps` over one bench state root.
pub fn run_prepare_maps(
    oracle: &mut ScriptedOracle,
    state_root: &Path,
    request: &ScanRequest,
) -> Result<Value, HostError> {
    let maps = prepare_maps_for(oracle, state_root, request)?;
    Ok(describe_maps(&maps))
}

fn describe_maps(maps: &PreparedMaps) -> Value {
    match maps {
        PreparedMaps::None => json!({"maps": "none"}),
        PreparedMaps::Grace(grace) => json!({"maps": "grace", "grace_ranges": grace.ranges.len()}),
        PreparedMaps::PrimaryFirst(map) => {
            json!({"maps": "primary_first", "primary_effects": map.effects.len()})
        }
        PreparedMaps::Joint { grace, primary } => json!({
            "maps": "joint",
            "grace_ranges": grace.ranges.len(),
            "primary_effects": primary.effects.len(),
        }),
    }
}

/// Prepare the measured maps exactly as the shipped `search` would.
fn prepare_maps_for(
    oracle: &mut ScriptedOracle,
    state_root: &Path,
    request: &ScanRequest,
) -> Result<PreparedMaps, HostError> {
    let fingerprint = "a".repeat(64);
    let digest = "b".repeat(64);
    let playthrough = request.filters.playthrough.unwrap_or(0) as u8;
    prepare_maps(
        oracle,
        state_root,
        &request.template,
        &fingerprint,
        &digest,
        playthrough,
        request.filters.rarity,
        request.level,
        request.recommended_level,
        request.transfer_count,
        request.filters.grace_effect_id,
        &request.filters.primary_effect_ids,
        &mut || false,
        &mut |_, _| {},
    )
}

/// One scan route: prepare whatever the route needs, then run the shipped
/// branch dispatch over the same request.
pub fn run_search_route(
    oracle: &mut ScriptedOracle,
    state_root: &Path,
    request: &ScanRequest,
    route: &str,
) -> Result<Value, HostError> {
    let mut request = request.clone();
    match route {
        "plain" => {}
        "grace_accelerated" | "primary_first" | "joint" => {
            let maps = prepare_maps_for(oracle, state_root, &request)?;
            let uses_primary = maps.uses_primary();
            let empty = maps.is_empty();
            request.acceleration.maps = maps;
            if uses_primary {
                request.acceleration.joint_start_after_trial = 0;
            } else if !empty {
                request.acceleration.grace_start_after_seed = if request.start_seed != 0 {
                    Some(request.start_seed - 1)
                } else {
                    None
                };
            }
        }
        other => {
            return Err(HostError::rejected(format!("unknown bench route {other}")));
        }
    }
    let mut last: Option<crate::scan::ScanProgress> = None;
    let mut report = |progress: crate::scan::ScanProgress| last = Some(progress);
    let matched = if route == "plain" {
        scan_seed_range(oracle, &request, None, &mut || false, &mut report)?
    } else {
        scan_next_candidate(oracle, &request, None, &mut || false, &mut report)?
    };
    // `RuntimeApplication.search` derives the continuation from the last
    // progress report, so the bench publishes the same pair.
    let resume_seed = last
        .map(|progress| progress.current_seed.saturating_add(1))
        .unwrap_or_else(|| request.start_seed.saturating_add(1));
    let resume_trial = last.and_then(|progress| progress.joint_trial);
    Ok(match matched {
        Some(matched) => json!({
            "candidate": {
                "seed": matched.seed,
                "rarity": matched.rarity,
                "playthrough": matched.playthrough,
                "record_stage": matched.record_stage.as_str(),
                "has_installation_record": matched.installation_record.is_some(),
                "cursor": matched.joint_search_trial,
                "predicted_growth_grace_id": matched.predicted_growth_grace_id,
            },
            "resume_seed": resume_seed,
            "resume_trial": resume_trial,
        }),
        None => json!({
            "candidate": Value::Null,
            "resume_seed": resume_seed,
            "resume_trial": resume_trial,
        }),
    })
}
