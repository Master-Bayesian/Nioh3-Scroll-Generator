//! The versioned protected request/response contract, evaluated in process.
//!
//! `protected_worker.py` loads `packages/contracts/protected-request.schema.json`
//! and `protected-response.schema.json` with `jsonschema.Draft7Validator` and
//! validates both directions, so the Rust host uses the same files with the same
//! validator crate the Tauri broker uses. The contract digest is the SHA-256 of
//! the two schema files, exactly as the Python worker computes it.

use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Request schema file name.
pub const REQUEST_SCHEMA: &str = "protected-request.schema.json";
/// Response schema file name.
pub const RESPONSE_SCHEMA: &str = "protected-response.schema.json";

/// The loaded protected contract.
pub struct Contract {
    digest: String,
    request: jsonschema::Validator,
    response: jsonschema::Validator,
}

impl Contract {
    /// Load both schema files from `contract_dir` and compute the digest.
    pub fn load(contract_dir: &Path) -> Result<Self, String> {
        let request_bytes = fs::read(contract_dir.join(REQUEST_SCHEMA))
            .map_err(|error| format!("cannot read the protected request schema: {error}"))?;
        let response_bytes = fs::read(contract_dir.join(RESPONSE_SCHEMA))
            .map_err(|error| format!("cannot read the protected response schema: {error}"))?;
        let parse = |bytes: &[u8]| -> Result<jsonschema::Validator, String> {
            jsonschema::validator_for(
                &serde_json::from_slice::<Value>(bytes).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        };
        let request = parse(&request_bytes)?;
        let response = parse(&response_bytes)?;
        let mut digest = Sha256::new();
        digest.update(&request_bytes);
        digest.update(&response_bytes);
        Ok(Self {
            digest: format!("{:x}", digest.finalize()),
            request,
            response,
        })
    }

    /// The handshake `contract_digest`.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// `VALIDATOR.is_valid(payload)` for one request frame.
    pub fn request_valid(&self, payload: &Value) -> bool {
        self.request.is_valid(payload)
    }

    /// `RESPONSE_VALIDATOR.is_valid(response)` for one response frame.
    pub fn response_valid(&self, payload: &Value) -> bool {
        self.response.is_valid(payload)
    }
}
