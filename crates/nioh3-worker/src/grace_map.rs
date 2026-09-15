//! Save-bound measured grace/raw-output map cache: the `cache.register` payload.
//!
//! Ports `nioh3_scroll_editor/grace_map.py`'s cache half
//! (`grace_map_from_cache_payload` plus `_validate_complete_mapping`), including
//! every rejection message, so a stale or foreign save context is refused with
//! the same text the shipped worker uses. Nothing here reads a live game or a
//! save; the payload is supplied by the caller.

use serde_json::Value;

use nioh3_domain::effect::{GraceMap, GraceRange as DomainGraceRange};

/// `grace_map.GRACE_MAP_CACHE_SCHEMA`.
pub const GRACE_MAP_CACHE_SCHEMA: &str = "nioh3-grace-output-map-cache/v2";
/// `grace_map.GRACE_MAP_FORMAT`: the typed map file format the engine consumes.
pub const GRACE_MAP_FORMAT: &str = "nioh3-grace-first-u16-map-v2";
/// `grace_map._EXPECTED_VERSION`.
pub const EXPECTED_GAME_VERSION: &str = "2.00.02";
/// `emaki_exchange.CATEGORY_TO_TYPE`, indexed by playthrough.
pub const CATEGORY_TO_TYPE: [u16; 6] = [0x0000, 0x1E82, 0x516D, 0xE604, 0xDD82, 0xD523];

/// One contiguous draw-bucket range that resolves to one effect id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraceRange {
    pub start: u32,
    pub end: u32,
    pub grace_id: u32,
}

/// `grace_map.GraceOutputMap`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraceOutputMap {
    pub record_type: u16,
    pub rarity: u8,
    pub playthrough: String,
    pub effect_slot: u8,
    pub ranges: Vec<GraceRange>,
}

/// `_validate_complete_mapping`: the ranges must partition all 65,536 buckets.
///
/// `grace_id` stays a signed integer here so an out-of-range id reports the
/// shipped "does not fit in uint32" message instead of being clamped at parse.
fn validate_complete_mapping(ranges: &[(u32, u32, i64)]) -> Result<(), String> {
    let mut expected_start: u32 = 0;
    for (start, end, grace_id) in ranges {
        if *start != expected_start || *end < *start || *end > 0xFFFF {
            return Err("Grace output map is not a complete contiguous partition".to_string());
        }
        if !(0..=0xFFFF_FFFF).contains(grace_id) {
            return Err("Grace output map effect ID does not fit in uint32".to_string());
        }
        expected_start = *end + 1;
    }
    if expected_start != 0x1_0000 {
        return Err("Grace output map does not cover all 65,536 draw buckets".to_string());
    }
    Ok(())
}

/// Python `repr` for the JSON subset the schema guard prints.
fn py_repr(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "None".to_string(),
        Some(Value::Bool(flag)) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::String(text)) => format!("'{text}'"),
        Some(other) => other.to_string(),
    }
}

/// `int(value)` for the integer spellings the shipped cache uses, plus
/// `int(str(value), 0)` for the hexadecimal `grace_id` / `record_type` fields.
fn int_value(value: Option<&Value>, base_prefix_allowed: bool) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64().or_else(|| {
            number
                .as_f64()
                .filter(|float| float.fract() == 0.0)
                .map(|float| float as i64)
        }),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if base_prefix_allowed {
                parse_int_auto(trimmed)
            } else {
                trimmed.parse::<i64>().ok()
            }
        }
        _ => None,
    }
}

/// `int(text, 0)`: decimal, `0x` hex, `0o` octal or `0b` binary, sign aware.
fn parse_int_auto(text: &str) -> Option<i64> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let magnitude = if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(octal) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        i64::from_str_radix(octal, 8).ok()?
    } else if let Some(binary) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        i64::from_str_radix(binary, 2).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// `grace_map.grace_map_from_cache_payload`.
///
/// `Err` carries the shipped rejection message verbatim; the caller reports it
/// as `INVALID_REQUEST`, exactly like the Python worker's `ValueError`.
pub fn from_cache_payload(
    payload: &Value,
    expected_generation_context_digest: Option<&str>,
) -> Result<GraceOutputMap, String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "Grace-map cache root must be an object".to_string())?;

    let schema = object.get("schema");
    if schema.and_then(Value::as_str) != Some(GRACE_MAP_CACHE_SCHEMA) {
        return Err(format!(
            "unsupported Grace-map cache schema: {}",
            py_repr(schema)
        ));
    }
    if object.get("game_version").and_then(Value::as_str) != Some(EXPECTED_GAME_VERSION) {
        return Err("Grace-map cache belongs to another game version".to_string());
    }
    let generation_digest = object
        .get("generation_context_digest")
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
        .to_lowercase();
    if !is_sha256_hex(&generation_digest) {
        return Err("Grace-map cache has no valid generation context".to_string());
    }
    if let Some(expected) = expected_generation_context_digest {
        if generation_digest != expected.trim().to_lowercase() {
            return Err("Grace output map belongs to a different generation context".to_string());
        }
    }
    let draw_index = object
        .get("draw_index")
        .map_or(Some(0), |value| int_value(Some(value), false))
        .unwrap_or(0);
    if draw_index != 1 {
        return Err("Grace-map cache is not a draw-1 partition".to_string());
    }
    let raw_ranges = object
        .get("ranges")
        .and_then(Value::as_array)
        .ok_or_else(|| "Grace-map cache has no range partition".to_string())?;

    let mut ranges: Vec<(u32, u32, i64)> = Vec::with_capacity(raw_ranges.len());
    for item in raw_ranges {
        let entry = item
            .as_object()
            .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
        let start = int_value(entry.get("start"), false)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
        let end = int_value(entry.get("end"), false)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
        let grace_id = int_value(entry.get("grace_id"), true)
            .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
        ranges.push((start, end, grace_id));
    }
    validate_complete_mapping(&ranges)?;
    let ranges: Vec<GraceRange> = ranges
        .into_iter()
        .map(|(start, end, grace_id)| GraceRange {
            start,
            end,
            grace_id: grace_id as u32,
        })
        .collect();

    let record_type = int_value(object.get("record_type"), true)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
    let rarity = int_value(object.get("rarity"), false)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
    let playthrough = object
        .get("playthrough")
        .map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .ok_or_else(|| "invalid Grace-map cache range".to_string())?;
    let effect_slot = int_value(object.get("effect_slot"), false)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or_else(|| "invalid Grace-map cache range".to_string())?;

    let mapping = GraceOutputMap {
        record_type,
        rarity,
        playthrough,
        effect_slot,
        ranges,
    };
    Ok(mapping)
}

impl GraceOutputMap {
    /// The typed engine map this registered payload stands for.
    ///
    /// The cached payload and the bundled resource carry the same partition in
    /// different shapes; this is the bridge the NG4/NG5 route compiles against.
    /// Every range is re-validated as the dense `uint16` partition the engine's
    /// `GraceMap` contract requires, so a payload that only satisfied the cache
    /// schema cannot slip through with a truncated or inverted run.
    pub fn to_domain_map(&self) -> Result<GraceMap, String> {
        let mut ranges = Vec::with_capacity(self.ranges.len());
        for range in &self.ranges {
            if range.end > u32::from(u16::MAX) {
                return Err("Grace output map range does not fit in uint16".to_string());
            }
            ranges.push(DomainGraceRange {
                start: range.start as u16,
                end: range.end as u16,
                effect_id: range.grace_id,
            });
        }
        let map = GraceMap {
            format: GRACE_MAP_FORMAT.to_string(),
            game_version: EXPECTED_GAME_VERSION.to_string(),
            record_type: u32::from(self.record_type),
            rarity: self.rarity,
            capture_state: self.playthrough.clone(),
            effect_slot: self.effect_slot,
            ranges,
        };
        // `GraceMap::validate` pins the bundled NG3 metadata; a registered map
        // belongs to its own playthrough, so only the dense-partition contract
        // applies here. The certified composer re-checks the record type against
        // the playthrough it is asked to generate.
        map.validate_partition()
            .map_err(|error| format!("registered Grace output map is invalid: {error:?}"))?;
        Ok(map)
    }
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DIGEST: &str = "d0a71d9f1a6345b1f2b1d1e9b1b0a1f0b7d61e5a4e9f2c3a5b6c7d8e9f0a1b2c";

    fn valid_payload() -> Value {
        json!({
            "schema": GRACE_MAP_CACHE_SCHEMA,
            "context_fingerprint": "1".repeat(64),
            "generation_context_digest": DIGEST,
            "game_version": EXPECTED_GAME_VERSION,
            "record_type": "0xE604",
            "rarity": 5,
            "playthrough": "current-loaded-state",
            "effect_slot": 6,
            "draw_index": 1,
            "ranges": [
                {"start": 0, "end": 0x7FFF, "grace_id": "0x00001234"},
                {"start": 0x8000, "end": 0xFFFF, "grace_id": "0x00005678"},
            ],
        })
    }

    #[test]
    fn a_complete_partition_loads() {
        let mapping = from_cache_payload(&valid_payload(), Some(DIGEST)).expect("valid cache");
        assert_eq!(mapping.record_type, 0xE604);
        assert_eq!(mapping.rarity, 5);
        assert_eq!(mapping.effect_slot, 6);
        assert_eq!(mapping.ranges.len(), 2);
        assert_eq!(mapping.ranges[1].grace_id, 0x5678);
    }

    #[test]
    fn every_shipped_rejection_keeps_its_message() {
        let cases: Vec<(&str, Value, Option<&str>, &str)> = vec![
            (
                "schema",
                json!({"schema": "other"}),
                None,
                "unsupported Grace-map cache schema: 'other'",
            ),
            (
                "missing schema",
                json!({}),
                None,
                "unsupported Grace-map cache schema: None",
            ),
            (
                "game version",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": "1.00.00"}),
                None,
                "Grace-map cache belongs to another game version",
            ),
            (
                "generation context",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": EXPECTED_GAME_VERSION,
                       "generation_context_digest": "nope"}),
                None,
                "Grace-map cache has no valid generation context",
            ),
            (
                "stale generation context",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": EXPECTED_GAME_VERSION,
                       "generation_context_digest": "a".repeat(64), "draw_index": 1}),
                Some(DIGEST),
                "Grace output map belongs to a different generation context",
            ),
            (
                "draw index",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": EXPECTED_GAME_VERSION,
                       "generation_context_digest": DIGEST, "draw_index": 2}),
                None,
                "Grace-map cache is not a draw-1 partition",
            ),
            (
                "ranges",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": EXPECTED_GAME_VERSION,
                       "generation_context_digest": DIGEST, "draw_index": 1, "ranges": "x"}),
                None,
                "Grace-map cache has no range partition",
            ),
            (
                "range item",
                json!({"schema": GRACE_MAP_CACHE_SCHEMA, "game_version": EXPECTED_GAME_VERSION,
                       "generation_context_digest": DIGEST, "draw_index": 1, "ranges": [5]}),
                None,
                "invalid Grace-map cache range",
            ),
        ];
        for (label, payload, expected_digest, expected) in cases {
            let error = from_cache_payload(&payload, expected_digest).expect_err(label);
            assert_eq!(error, expected, "{label}");
        }
    }

    #[test]
    fn partition_errors_keep_their_messages() {
        let mut gapped = valid_payload();
        gapped["ranges"] = json!([{"start": 0, "end": 0x7FFF, "grace_id": "0x1"}]);
        assert_eq!(
            from_cache_payload(&gapped, None).expect_err("gap"),
            "Grace output map does not cover all 65,536 draw buckets"
        );

        let mut offset = valid_payload();
        offset["ranges"] = json!([
            {"start": 1, "end": 0x8000, "grace_id": "0x1"},
            {"start": 0x8001, "end": 0xFFFF, "grace_id": "0x2"},
        ]);
        assert_eq!(
            from_cache_payload(&offset, None).expect_err("offset"),
            "Grace output map is not a complete contiguous partition"
        );
    }
}
