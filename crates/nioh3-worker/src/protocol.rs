//! Request validation and the supported method subset.
//!
//! The shipped request schema is the contract, evaluated verbatim by
//! [`crate::schema::RequestSchema`], so every method the worker serves gets the
//! same strict parameter validation the shipped `jsonschema.Draft7Validator`
//! gives it and answers with the same `INVALID_REQUEST` code. Methods the schema
//! knows but this slice does not serve would answer `UNSUPPORTED_METHOD` after
//! that validation, never before it; the shipped contract's eleven methods are
//! all served, so that path is currently unreachable. Methods the schema does
//! not know at all stay `INVALID_REQUEST`, exactly like the Python worker.

use serde_json::Value;

use crate::schema::RequestSchema;

/// `PROTOCOL_VERSION` from the shipped contracts.
pub const PROTOCOL_VERSION: i64 = 1;
/// Methods this development worker actually serves. Every method the shipped
/// contract can express is served, so [`UNIMPLEMENTED_METHODS`] is empty.
pub const SUPPORTED_METHODS: [&str; 12] = [
    "handshake",
    "recommended_level.resolve",
    "cache.register",
    "candidate.preview",
    "search.start",
    "search.feasibility",
    "search.catalog",
    "job.current",
    "job.snapshot",
    "job.cancel",
    "candidate.export",
    "shutdown",
];
/// Methods the shipped schema knows but this slice does not implement.
pub const UNIMPLEMENTED_METHODS: [&str; 0] = [];

/// A rejected request, carrying the shipped error code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    pub code: &'static str,
    pub message: String,
}

impl RequestError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

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

    /// A schema-valid request the shipped worker still rejects on semantics.
    pub fn invalid_request_message(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_REQUEST",
            message: message.into(),
        }
    }

    pub fn handshake_required() -> Self {
        Self {
            code: "HANDSHAKE_REQUIRED",
            message: "Negotiate before sending commands".to_string(),
        }
    }

    pub fn job_not_found() -> Self {
        Self {
            code: "JOB_NOT_FOUND",
            message: "Job is unavailable; only the latest job is retained".to_string(),
        }
    }

    pub fn invalid_resume_token() -> Self {
        Self {
            code: "INVALID_RESUME_TOKEN",
            message: "Resume token belongs to a different query, context, execution policy, or \
                      worker session"
                .to_string(),
        }
    }

    pub fn unsupported_method(method: &str) -> Self {
        Self {
            code: "UNSUPPORTED_METHOD",
            message: format!(
                "This development worker does not implement {method}; the versioned \
                 contract's methods are served by {}",
                SUPPORTED_METHODS.join(", ")
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
    /// Carries the schema-checked `search.start` params verbatim; the job layer
    /// runs the cross-field query validation in the shipped order, after its
    /// BUSY and context checks.
    SearchStart {
        id: String,
        params: Value,
    },
    JobCurrent {
        id: String,
    },
    JobSnapshot {
        id: String,
        job_id: String,
    },
    JobCancel {
        id: String,
        job_id: String,
    },
    CandidateExport {
        id: String,
        job_id: String,
        candidate_id: String,
    },
    /// `recommended_level.resolve`: exact inverse of the captured native curve.
    RecommendedLevelResolve {
        id: String,
        displayed_level: i32,
    },
    /// `cache.register`: validate a save-bound measured map and return its id.
    CacheRegister {
        id: String,
        cache_json: String,
    },
    /// `search.feasibility`: the read-only structural preflight of one query.
    SearchFeasibility {
        id: String,
        query: Value,
    },
    /// `search.catalog`: the context-bound option catalog for one rarity.
    SearchCatalog {
        id: String,
        rarity: u8,
        locale: String,
    },
    Shutdown {
        id: String,
    },
    /// Schema-known method this slice does not serve.
    Unimplemented {
        id: String,
        method: String,
    },
}

impl Request {
    pub fn id(&self) -> &str {
        match self {
            Request::Handshake { id }
            | Request::CandidatePreview { id, .. }
            | Request::SearchStart { id, .. }
            | Request::SearchFeasibility { id, .. }
            | Request::JobCurrent { id }
            | Request::JobSnapshot { id, .. }
            | Request::JobCancel { id, .. }
            | Request::CandidateExport { id, .. }
            | Request::RecommendedLevelResolve { id, .. }
            | Request::CacheRegister { id, .. }
            | Request::SearchCatalog { id, .. }
            | Request::Shutdown { id }
            | Request::Unimplemented { id, .. } => id,
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

/// Validate one request frame. The order mirrors `worker_contracts.validate_request`:
/// protocol version, then the shipped schema, then dispatch.
pub fn parse_request(payload: &Value, schema: &RequestSchema) -> Result<Request, RequestError> {
    let object = payload
        .as_object()
        .ok_or_else(RequestError::protocol_mismatch)?;
    match object.get("protocol") {
        Some(Value::Number(number)) if number.as_i64() == Some(PROTOCOL_VERSION) => {}
        _ => return Err(RequestError::protocol_mismatch()),
    }
    if !schema.accepts(payload) {
        return Err(RequestError::invalid_request());
    }
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .expect("schema requires id")
        .to_string();
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .expect("schema requires method");
    let params = object.get("params").expect("schema requires params");

    match method {
        "handshake" => Ok(Request::Handshake { id }),
        "shutdown" => Ok(Request::Shutdown { id }),
        "candidate.preview" => Ok(Request::CandidatePreview {
            id,
            seed: integer(params, "seed") as u32,
            rarity: integer(params, "rarity") as u8,
            level: integer(params, "level") as u16,
        }),
        "search.start" => Ok(Request::SearchStart {
            id,
            params: params.clone(),
        }),
        "search.feasibility" => Ok(Request::SearchFeasibility {
            id,
            query: params.get("query").cloned().expect("schema requires the query"),
        }),
        "job.current" => Ok(Request::JobCurrent { id }),
        "job.snapshot" => Ok(Request::JobSnapshot {
            id,
            job_id: string(params, "job_id").to_string(),
        }),
        "job.cancel" => Ok(Request::JobCancel {
            id,
            job_id: string(params, "job_id").to_string(),
        }),
        "candidate.export" => Ok(Request::CandidateExport {
            id,
            job_id: string(params, "job_id").to_string(),
            candidate_id: string(params, "candidate_id").to_string(),
        }),
        "recommended_level.resolve" => Ok(Request::RecommendedLevelResolve {
            id,
            displayed_level: integer(params, "displayed_level") as i32,
        }),
        "cache.register" => Ok(Request::CacheRegister {
            id,
            cache_json: string(params, "cache_json").to_string(),
        }),
        "search.catalog" => Ok(Request::SearchCatalog {
            id,
            rarity: integer(params, "rarity") as u8,
            locale: string(params, "locale").to_string(),
        }),
        other => Ok(Request::Unimplemented {
            id,
            method: other.to_string(),
        }),
    }
}

/// A schema-required integer parameter.
fn integer(params: &Value, key: &str) -> i64 {
    crate::schema::integral(params.get(key).expect("schema requires the key"))
        .expect("schema requires an integer")
}

/// A schema-required string parameter.
fn string<'a>(params: &'a Value, key: &str) -> &'a str {
    params
        .get(key)
        .and_then(Value::as_str)
        .expect("schema requires a string")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request_schema() -> RequestSchema {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts");
        RequestSchema::load(&dir).expect("load the shipped request schema")
    }

    #[test]
    fn accepts_the_supported_methods() {
        let schema = request_schema();
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"handshake","params":{}}),
                &schema
            ),
            Ok(Request::Handshake {
                id: "a".to_string()
            })
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"b","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":4,"level":180}}),
                &schema
            ),
            Ok(Request::CandidatePreview {
                id: "b".to_string(),
                seed: 1,
                rarity: 4,
                level: 180
            })
        );
        // JSON Schema integer accepts an integral JSON number.
        assert!(matches!(
            parse_request(
                &json!({"protocol":1,"id":"c","method":"candidate.preview",
                                  "params":{"seed":1.0,"rarity":4,"level":180.0}}),
                &schema
            ),
            Ok(Request::CandidatePreview { .. })
        ));
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"d","method":"job.current","params":{}}),
                &schema
            ),
            Ok(Request::JobCurrent {
                id: "d".to_string()
            })
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"e","method":"recommended_level.resolve",
                        "params":{"displayed_level":200}}),
                &schema
            ),
            Ok(Request::RecommendedLevelResolve {
                id: "e".to_string(),
                displayed_level: 200
            })
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"f","method":"cache.register",
                        "params":{"cache_json":"{}"}}),
                &schema
            ),
            Ok(Request::CacheRegister {
                id: "f".to_string(),
                cache_json: "{}".to_string()
            })
        );
    }

    #[test]
    fn every_contract_method_parses_and_shape_faults_validate_first() {
        let schema = request_schema();
        // Serving `search.catalog` retired the last contract method this slice
        // refused, so nothing may fall through to UNSUPPORTED_METHOD any more.
        assert_eq!(UNIMPLEMENTED_METHODS.len(), 0);
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"search.catalog",
                        "params":{"playthrough":3,"rarity":4,"locale":"zh-CN"}}),
                &schema
            )
            .expect("the catalog method is served"),
            Request::SearchCatalog {
                id: "a".to_string(),
                rarity: 4,
                locale: "zh-CN".to_string()
            }
        );
        // The shipped code stays available for a method a later schema adds.
        assert_eq!(
            RequestError::unsupported_method("not.a.method").code,
            "UNSUPPORTED_METHOD"
        );
        // Malformed params answer INVALID_REQUEST before any routing decision.
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"search.catalog",
                        "params":{"playthrough":4,"rarity":4,"locale":"zh-CN"}}),
                &schema
            )
            .expect_err("playthrough 4 is outside the enum")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"search.catalog",
                        "params":{"playthrough":3,"rarity":4,"locale":"fr-FR"}}),
                &schema
            )
            .expect_err("locale outside the enum")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"search.start","params":{}}),
                &schema
            )
            .expect_err("search.start requires its full param block")
            .code,
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn protocol_and_shape_faults_match_the_shipped_codes() {
        let schema = request_schema();
        assert_eq!(
            parse_request(
                &json!({"protocol":2,"id":"a","method":"handshake","params":{}}),
                &schema
            )
            .expect_err("protocol 2 must fail")
            .code,
            "PROTOCOL_MISMATCH"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"handshake","params":{},
                                  "unexpected":true}),
                &schema
            )
            .expect_err("additional properties must fail")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":4,"level":180,"extra":1}}),
                &schema
            )
            .expect_err("unknown params must fail")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"not.a.method","params":{}}),
                &schema
            )
            .expect_err("unknown methods are not in the schema")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"","method":"handshake","params":{}}),
                &schema
            )
            .expect_err("empty id must fail")
            .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            parse_request(
                &json!({"protocol":1,"id":"a","method":"candidate.preview",
                                  "params":{"seed":1,"rarity":6,"level":180}}),
                &schema
            )
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
