//! Minimal Draft-7 subset validator for the shipped request schema.
//!
//! The shipped contract is `packages/contracts/request.schema.json` and the
//! shipped worker validates it with `jsonschema.Draft7Validator`. Re-deriving a
//! hand-written per-method parameter check drifts; this module instead evaluates
//! the shipped file directly, for the keyword subset that file actually uses.
//! Loading asserts that subset, so a schema that grows an unsupported keyword
//! fails loudly instead of silently weakening validation.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

/// Keywords this evaluator implements. Anything else in the shipped schema is a
/// hard load error rather than an ignored constraint.
const SUPPORTED_KEYWORDS: [&str; 21] = [
    "$schema",
    "additionalProperties",
    "anyOf",
    "const",
    "default",
    "description",
    "enum",
    "items",
    "maxItems",
    "maxLength",
    "maximum",
    "minItems",
    "minLength",
    "minimum",
    "oneOf",
    "pattern",
    "properties",
    "required",
    "title",
    "type",
    "uniqueItems",
];

/// The only `pattern` the shipped request schema uses.
const HEX_64_PATTERN: &str = "^[0-9a-f]{64}$";

/// The shipped request schema, evaluated in process.
#[derive(Debug, Clone)]
pub struct RequestSchema {
    root: Value,
}

impl RequestSchema {
    /// Read and check `request.schema.json` from the contract directory.
    pub fn load(contract_dir: &Path) -> Result<Self, String> {
        let bytes = fs::read(contract_dir.join("request.schema.json"))
            .map_err(|error| format!("cannot read the request schema: {error}"))?;
        let root: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("request schema is not JSON: {error}"))?;
        assert_supported(&root)?;
        Ok(Self { root })
    }

    /// `jsonschema.Draft7Validator(schema).is_valid(payload)` for one `oneOf`.
    ///
    /// The shipped schema is a `oneOf` over the protocol methods, so exactly one
    /// branch must accept the payload, matching the shipped validator.
    pub fn accepts(&self, payload: &Value) -> bool {
        let Value::Object(object) = &self.root else {
            return false;
        };
        let Some(Value::Array(branches)) = object.get("oneOf") else {
            return false;
        };
        branches
            .iter()
            .filter(|branch| matches_schema(branch, payload))
            .count()
            == 1
    }
}

fn assert_supported(node: &Value) -> Result<(), String> {
    match node {
        Value::Object(object) => {
            for (key, value) in object {
                if !SUPPORTED_KEYWORDS.contains(&key.as_str()) {
                    return Err(format!("unsupported request-schema keyword: {key}"));
                }
                if key == "pattern" && value.as_str() != Some(HEX_64_PATTERN) {
                    return Err(format!("unsupported request-schema pattern: {value}"));
                }
                if key == "properties" {
                    // Property names are not schema keywords.
                    for sub_schema in value.as_object().into_iter().flatten().map(|(_, v)| v) {
                        assert_supported(sub_schema)?;
                    }
                } else {
                    assert_supported(value)?;
                }
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                assert_supported(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Evaluate one sub-schema against one value.
fn matches_schema(schema: &Value, value: &Value) -> bool {
    let Value::Object(schema) = schema else {
        return true;
    };

    if let Some(constant) = schema.get("const") {
        if value != constant {
            return false;
        }
    }
    if let Some(Value::Array(variants)) = schema.get("enum") {
        if !variants.iter().any(|variant| variant == value) {
            return false;
        }
    }
    if let Some(types) = schema.get("type") {
        if !matches_type(types, value) {
            return false;
        }
    }
    if let Some(Value::Array(branches)) = schema.get("anyOf") {
        if !branches.iter().any(|branch| matches_schema(branch, value)) {
            return false;
        }
    }
    if let Some(Value::Array(branches)) = schema.get("oneOf") {
        if branches
            .iter()
            .filter(|branch| matches_schema(branch, value))
            .count()
            != 1
        {
            return false;
        }
    }

    match value {
        Value::Object(object) => matches_object(schema, object),
        Value::Array(items) => matches_array(schema, items),
        Value::String(text) => matches_string(schema, text),
        Value::Number(_) => matches_number(schema, value),
        _ => true,
    }
}

fn matches_type(types: &Value, value: &Value) -> bool {
    match types {
        Value::String(name) => type_matches(name, value),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .any(|name| type_matches(name, value)),
        _ => true,
    }
}

/// Draft 4+ `integer` accepts integral JSON numbers such as `180.0`; JSON
/// booleans are never numbers.
fn type_matches(name: &str, value: &Value) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        "integer" => integral(value).is_some(),
        _ => false,
    }
}

fn matches_object(schema: &Map<String, Value>, object: &Map<String, Value>) -> bool {
    if let Some(Value::Array(required)) = schema.get("required") {
        for key in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(key) {
                return false;
            }
        }
    }
    if let Some(Value::Object(properties)) = schema.get("properties") {
        for (key, sub_schema) in properties {
            if let Some(value) = object.get(key) {
                if !matches_schema(sub_schema, value) {
                    return false;
                }
            }
        }
    }
    if let Some(additional) = schema.get("additionalProperties") {
        if additional == &Value::Bool(false) {
            let known = schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            if object.keys().any(|key| !known.contains(key)) {
                return false;
            }
        }
    }
    true
}

fn matches_array(schema: &Map<String, Value>, items: &[Value]) -> bool {
    if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64) {
        if (items.len() as u64) < minimum {
            return false;
        }
    }
    if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64) {
        if (items.len() as u64) > maximum {
            return false;
        }
    }
    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        for (index, item) in items.iter().enumerate() {
            if items[index + 1..].contains(item) {
                return false;
            }
        }
    }
    if let Some(sub_schema) = schema.get("items") {
        // The shipped schema only uses a single schema (or a tuple schema whose
        // entries are all `anyOf`) for `items`; evaluate both shapes.
        match sub_schema {
            Value::Array(tuple) => {
                if items.len() != tuple.len() {
                    return false;
                }
                if !tuple
                    .iter()
                    .zip(items)
                    .all(|(branch, item)| matches_schema(branch, item))
                {
                    return false;
                }
            }
            other => {
                if !items.iter().all(|item| matches_schema(other, item)) {
                    return false;
                }
            }
        }
    }
    true
}

fn matches_string(schema: &Map<String, Value>, text: &str) -> bool {
    let length = text.chars().count() as u64;
    if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
        if length < minimum {
            return false;
        }
    }
    if let Some(maximum) = schema.get("maxLength").and_then(Value::as_u64) {
        if length > maximum {
            return false;
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        if pattern == HEX_64_PATTERN {
            if text.len() != 64
                || !text
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

fn matches_number(schema: &Map<String, Value>, value: &Value) -> bool {
    let Some(number) = value.as_f64() else {
        return false;
    };
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if number < minimum {
            return false;
        }
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64) {
        if number > maximum {
            return false;
        }
    }
    true
}

/// JSON `integer`, including integral floats and excluding booleans.
pub fn integral(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                Some(value)
            } else if let Some(value) = number.as_u64() {
                i64::try_from(value).ok()
            } else {
                let value = number.as_f64()?;
                if value.is_finite() && value.fract() == 0.0 {
                    Some(value as i64)
                } else {
                    None
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> RequestSchema {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
        RequestSchema::load(&dir).expect("load the shipped request schema")
    }

    #[test]
    fn the_shipped_schema_only_uses_supported_keywords() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
        assert!(RequestSchema::load(&dir).is_ok());
    }

    #[test]
    fn accepts_a_well_formed_start_and_rejects_shape_faults() {
        let schema = schema();
        let valid = json!({
            "protocol": 1, "id": "a", "method": "job.current", "params": {}
        });
        assert!(schema.accepts(&valid));
        assert!(!schema.accepts(&json!({"protocol": 1, "id": "a", "method": "job.current"})));
        assert!(!schema.accepts(
            &json!({"protocol": 1, "id": "a", "method": "job.current", "params": {"x": 1}})
        ));
        assert!(!schema
            .accepts(&json!({"protocol": 1, "id": "", "method": "job.current", "params": {}})));
        assert!(!schema
            .accepts(&json!({"protocol": 1, "id": "a", "method": "not.a.method", "params": {}})));
    }

    #[test]
    fn job_id_and_digest_bounds_come_from_the_schema() {
        let schema = schema();
        assert!(schema.accepts(&json!({
            "protocol": 1, "id": "a", "method": "job.snapshot",
            "params": {"job_id": "00000000-0000-4000-8000-000000000000"}
        })));
        assert!(!schema.accepts(&json!({
            "protocol": 1, "id": "a", "method": "job.snapshot", "params": {"job_id": ""}
        })));
        assert!(schema.accepts(&json!({
            "protocol": 1, "id": "a", "method": "candidate.export",
            "params": {"job_id": "j", "candidate_id": "a".repeat(64)}
        })));
        assert!(!schema.accepts(&json!({
            "protocol": 1, "id": "a", "method": "candidate.export",
            "params": {"job_id": "j", "candidate_id": "A".repeat(64)}
        })));
    }

    #[test]
    fn integral_numbers_are_accepted_like_draft_seven() {
        assert_eq!(integral(&json!(180)), Some(180));
        assert_eq!(integral(&json!(180.0)), Some(180));
        assert_eq!(integral(&json!(180.5)), None);
        assert_eq!(integral(&json!(true)), None);
    }
}
