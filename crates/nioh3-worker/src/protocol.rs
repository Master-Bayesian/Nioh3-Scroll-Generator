//! Request validation and the supported method subset.
//!
//! The shipped request schema is the contract. This module reproduces the
//! parts the development worker serves (`handshake`, `candidate.preview`,
//! `shutdown`), keeps `additionalProperties: false` at every level it checks,
//! and answers every other schema-known method with `UNSUPPORTED_METHOD`
//! instead of pretending to implement it. Methods the schema does not know at
//! all stay `INVALID_REQUEST`, exactly like the Python worker.

use serde_json::{Map, Value};

/// `PROTOCOL_VERSION` from the shipped contracts.
pub const PROTOCOL_VERSION: i64 = 1;
/// Methods this development worker actually serves.
pub const SUPPORTED_METHODS: [&str; 3] = ["handshake", "candidate.preview", "shutdown"];
/// Methods the shipped schema knows but this slice does not implement.
pub const UNIMPLEMENTED_METHODS: [&str; 8] = [
    "search.catalog",
    "recommended_level.resolve",
    "search.start",
    "cache.register",
    "candidate.export",
    "job.snapshot",
    "job.current",
    "job.cancel",
];

/// A rejected request, carrying the shipped error code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    pub code: &'static str,
    pub message: String,
}

impl RequestError {
    pub fn protocol_mismatch() -> Self {
        Self {
            code: "PROTOCOL_MISMATCH",
            message: "Expected protocol version 1".to_string(),
        }
    }

    pub fn invalid_request() -> Self {
        Self {
            code: "INVALID_REQUEST",
            message: "Request does not match the versioned contract".to_string(),
        }
    }

    pub fn handshake_required() -> Self {
        Self {
            code: "HANDSHAKE_REQUIRED",
            message: "Negotiate before sending commands".to_string(),
        }
    }

    pub fn unsupported_method(method: &str) -> Self {
        Self {
            code: "UNSUPPORTED_METHOD",
            message: format!(
                "This development worker does not implement {method}; supported methods are \
                 handshake, candidate.preview and shutdown"
            ),
        }
    }
}

/// A validated request in the order the Python worker dispatches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Handshake {
        id: String,
    },
    CandidatePreview {
        id: String,
        seed: u32,
        rarity: u8,
        level: u16,
    },
    Shutdown {
        id: String,
    },
    Unsupported {
        id: String,
        method: String,
    },
}

impl Request {
    pub fn id(&self) -> &str {
        match self {
            Request::Handshake { id }
            | Request::CandidatePreview { id, .. }
            | Request::Shutdown { id }
            | Request::Unsupported { id, .. } => id,
        }
    }
}

/// Echoed `id` for a response, or `null` when the request carried none.
pub fn request_id(payload: &Value) -> Value {
    payload
        .as_object()
        .and_then(|object| object.get("id"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// Validate one request frame. The order mirrors `worker_contracts.validate_request`.
pub fn parse_request(payload: &Value) -> Result<Request, RequestError> {
    let object = payload
        .as_object()
        .ok_or_else(RequestError::protocol_mismatch)?;
    match object.get("protocol") {
        Some(Value::Number(number)) if number.as_i64() == Some(PROTOCOL_VERSION) => {}
        _ => return Err(RequestError::protocol_mismatch()),
    }
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "protocol" | "id" | "method" | "params"))
    {
        return Err(RequestError::invalid_request());
    }

    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(RequestError::invalid_request)?;
    let id_chars = id.chars().count();
    if id_chars == 0 || id_chars > 64 {
        return Err(RequestError::invalid_request());
    }

    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(RequestError::invalid_request)?;
    let params = object
        .get("params")
        .ok_or_else(RequestError::invalid_request)?;
    if !params.is_object() {
        return Err(RequestError::invalid_request());
    }
    if !SUPPORTED_METHODS.contains(&method) && !UNIMPLEMENTED_METHODS.contains(&method) {
        return Err(RequestError::invalid_request());
    }

    match method {
        "handshake" => {
            require_empty_object(params)?;
            Ok(Request::Handshake { id: id.to_string() })
        }
        "shutdown" => {
            require_empty_object(params)?;
            Ok(Request::Shutdown { id: id.to_string() })
        }
        "candidate.preview" => {
            let params = params.as_object().expect("checked above");
            require_exact_keys(params, &["seed", "rarity", "level"])?;
            let seed = integral(params.get("seed"))
                .filter(|value| (0..=u32::MAX as i64).contains(value))
                .ok_or_else(RequestError::invalid_request)?;
            let rarity = integral(params.get("rarity"))
                .filter(|value| matches!(value, 3..=5))
                .ok_or_else(RequestError::invalid_request)?;
            let level = integral(params.get("level"))
                .filter(|value| (1..=180).contains(value))
                .ok_or_else(RequestError::invalid_request)?;
            Ok(Request::CandidatePreview {
                id: id.to_string(),
                seed: seed as u32,
                rarity: rarity as u8,
                level: level as u16,
            })
        }
        other => Ok(Request::Unsupported {
            id: id.to_string(),
            method: other.to_string(),
        }),
    }
}

fn require_empty_object(params: &Value) -> Result<(), RequestError> {
    let object = params
        .as_object()
        .ok_or_else(RequestError::invalid_request)?;
    if object.is_empty() {
        Ok(())
    } else {
        Err(RequestError::invalid_request())
    }
}

fn require_exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), RequestError> {
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(RequestError::invalid_request());
    }
    Ok(())
}

/// JSON Schema `integer`: integral JSON numbers, including `180.0`.
fn integral(value: Option<&Value>) -> Option<i64> {
    match value? {
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

    #[test]
    fn accepts_the_supported_methods() {
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"a","method":"handshake","params":{}})),
            Ok(Request::Handshake {
                id: "a".to_string()
            })
        );
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"b","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":4,"level":180}})),
            Ok(Request::CandidatePreview {
                id: "b".to_string(),
                seed: 1,
                rarity: 4,
                level: 180
            })
        );
        // JSON Schema integer accepts an integral JSON number.
        assert!(matches!(
            parse_request(&json!({"protocol":1,"id":"c","method":"candidate.preview",
                                  "params":{"seed":1.0,"rarity":4,"level":180.0}})),
            Ok(Request::CandidatePreview { .. })
        ));
    }

    #[test]
    fn unimplemented_methods_are_named_not_dropped() {
        let parsed =
            parse_request(&json!({"protocol":1,"id":"a","method":"search.start","params":{}}))
                .expect("schema-known method parses");
        assert_eq!(
            parsed,
            Request::Unsupported {
                id: "a".to_string(),
                method: "search.start".to_string()
            }
        );
        assert_eq!(
            RequestError::unsupported_method("search.start").code,
            "UNSUPPORTED_METHOD"
        );
    }

    #[test]
    fn protocol_and_shape_faults_match_the_shipped_codes() {
        assert_eq!(
            parse_request(&json!({"protocol":2,"id":"a","method":"handshake","params":{}}))
                .expect_err("protocol 2 must fail")
                .code,
            "PROTOCOL_MISMATCH"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"handshake","params":{},
                                  "unexpected":true})
            )
            .expect_err("additional properties must fail")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"a","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":4,"level":180,"extra":1}}))
            .expect_err("unknown params must fail")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"a","method":"not.a.method","params":{}}))
                .expect_err("unknown methods are not in the schema")
                .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"","method":"handshake","params":{}}))
                .expect_err("empty id must fail")
                .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(&json!({"protocol":1,"id":"a","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":6,"level":180}}))
            .expect_err("rarity outside the enum must fail")
            .code,
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn request_id_is_echoed_or_nulled() {
        assert_eq!(request_id(&json!({"id":"x"})), json!("x"));
        assert_eq!(request_id(&json!([])), Value::Null);
        assert_eq!(request_id(&json!({})), Value::Null);
    }
}
