//! Port of `nioh3_scroll_editor/core_services.py` `GenerationContext`.
//!
//! The identity payload is exactly seven fields, canonically encoded as JSON
//! with sorted keys and `,`/`:` separators, then SHA-256 hashed. `context_digest`
//! itself is not part of the hashed payload, and neither is any private record
//! pair.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::native::AcceleratorIdentity;

/// `GENERATION_ALGORITHM_VERSION` from the shipped core services.
pub const GENERATION_ALGORITHM_VERSION: &str = "scroll-generation-v0.7-native-completion-1";
/// `OPERATION_POLICY_VERSION` from the shipped core services.
pub const OPERATION_POLICY_VERSION: &str = "operation-policy-v1";
/// `SUPPORTED_GAME_PROFILE` from the shipped core services.
pub const SUPPORTED_GAME_PROFILE: &str = "pc-v2.00.02-v2.01";
/// `APP_VERSION` reported by the shipped product.
///
/// This value mirrors `nioh3_scroll_editor/version.py`, which the Python worker
/// imports directly. A unit test re-reads that file so the two cannot drift
/// silently; the value is never inferred or defaulted at runtime.
pub const PRODUCT_VERSION: &str = "0.7.5";

/// Fail-closed context errors, mirroring `CoreErrorCode.RESOURCE_MISMATCH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextError {
    pub code: &'static str,
    pub message: String,
}

impl ContextError {
    fn resource_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: "RESOURCE_MISMATCH",
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ContextError {}

/// Generation identity shared with the shipped worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationContext {
    pub product_version: String,
    pub game_profile: String,
    pub resources_digest: String,
    pub algorithm_version: String,
    pub policy_version: String,
    pub seed_accelerator_abi: Option<i64>,
    pub seed_accelerator_build_id: Option<String>,
    pub context_digest: String,
}

impl GenerationContext {
    /// The eight-key payload the handshake publishes.
    pub fn to_payload(&self) -> Value {
        let mut payload = Map::new();
        payload.insert(
            "product_version".to_string(),
            Value::String(self.product_version.clone()),
        );
        payload.insert(
            "game_profile".to_string(),
            Value::String(self.game_profile.clone()),
        );
        payload.insert(
            "resources_digest".to_string(),
            Value::String(self.resources_digest.clone()),
        );
        payload.insert(
            "algorithm_version".to_string(),
            Value::String(self.algorithm_version.clone()),
        );
        payload.insert(
            "policy_version".to_string(),
            Value::String(self.policy_version.clone()),
        );
        payload.insert(
            "seed_accelerator_abi".to_string(),
            match self.seed_accelerator_abi {
                Some(abi) => Value::Number(abi.into()),
                None => Value::Null,
            },
        );
        payload.insert(
            "seed_accelerator_build_id".to_string(),
            match &self.seed_accelerator_build_id {
                Some(build_id) => Value::String(build_id.clone()),
                None => Value::Null,
            },
        );
        payload.insert(
            "context_digest".to_string(),
            Value::String(self.context_digest.clone()),
        );
        Value::Object(payload)
    }
}

/// Capture the generation identity for `data_root`.
pub fn capture_context(
    game_profile: &str,
    data_root: &Path,
    accelerator: Option<AcceleratorIdentity>,
) -> Result<GenerationContext, ContextError> {
    let resources_digest = runtime_resource_digest(data_root)?;
    let seed_accelerator_abi = accelerator.as_ref().map(|identity| i64::from(identity.abi));
    let seed_accelerator_build_id = accelerator.map(|identity| identity.build_id);

    let mut identity_payload = Map::new();
    identity_payload.insert(
        "product_version".to_string(),
        Value::String(PRODUCT_VERSION.to_string()),
    );
    identity_payload.insert(
        "game_profile".to_string(),
        Value::String(game_profile.to_string()),
    );
    identity_payload.insert(
        "resources_digest".to_string(),
        Value::String(resources_digest.clone()),
    );
    identity_payload.insert(
        "algorithm_version".to_string(),
        Value::String(GENERATION_ALGORITHM_VERSION.to_string()),
    );
    identity_payload.insert(
        "policy_version".to_string(),
        Value::String(OPERATION_POLICY_VERSION.to_string()),
    );
    identity_payload.insert(
        "seed_accelerator_abi".to_string(),
        match seed_accelerator_abi {
            Some(abi) => Value::Number(abi.into()),
            None => Value::Null,
        },
    );
    identity_payload.insert(
        "seed_accelerator_build_id".to_string(),
        match &seed_accelerator_build_id {
            Some(build_id) => Value::String(build_id.clone()),
            None => Value::Null,
        },
    );

    // `serde_json::Map` is a `BTreeMap`, so this already matches Python's
    // `sort_keys=True`; the default compact separators match `(',', ':')`.
    let canonical = Value::Object(identity_payload).to_string();
    let context_digest = hex_lower(&Sha256::digest(canonical.as_bytes()));

    Ok(GenerationContext {
        product_version: PRODUCT_VERSION.to_string(),
        game_profile: game_profile.to_string(),
        resources_digest,
        algorithm_version: GENERATION_ALGORITHM_VERSION.to_string(),
        policy_version: OPERATION_POLICY_VERSION.to_string(),
        seed_accelerator_abi,
        seed_accelerator_build_id,
        context_digest,
    })
}

/// Hash every packaged runtime data file with its stable relative path.
///
/// Mirrors `runtime_resource_digest`: files are ordered the way Python's
/// `sorted(Path)` orders them on Windows (case-normalised path strings), and
/// each entry contributes `<len u32 le> || relative POSIX path || file SHA-256`
/// with the *original* relative spelling.
pub fn runtime_resource_digest(data_root: &Path) -> Result<String, ContextError> {
    let root = data_root.canonicalize().map_err(|error| {
        ContextError::resource_mismatch(format!(
            "runtime data directory is missing: {} ({error})",
            data_root.display()
        ))
    })?;
    if !root.is_dir() {
        return Err(ContextError::resource_mismatch(format!(
            "runtime data directory is missing: {}",
            root.display()
        )));
    }

    let mut files = Vec::new();
    collect_files(&root, &mut files)?;
    if files.is_empty() {
        return Err(ContextError::resource_mismatch(
            "runtime data directory is empty",
        ));
    }

    let mut ordered: Vec<(String, String, PathBuf)> = Vec::with_capacity(files.len());
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("collected paths stay below the data root")
            .to_string_lossy()
            .replace('\\', "/");
        let sort_key = relative.to_ascii_lowercase();
        ordered.push((sort_key, relative, path));
    }
    ordered.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let mut digest = Sha256::new();
    for (_, relative, path) in &ordered {
        let bytes = relative.as_bytes();
        digest.update((bytes.len() as u32).to_le_bytes());
        digest.update(bytes);
        digest.update(file_sha256(path)?);
    }
    Ok(hex_lower(&digest.finalize()))
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), ContextError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ContextError::resource_mismatch(format!(
            "runtime data directory is unreadable: {} ({error})",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ContextError::resource_mismatch(format!(
                "runtime data entry is unreadable: {} ({error})",
                directory.display()
            ))
        })?;
        let path = entry.path();
        // `fs::metadata` follows symlinks, matching `Path.is_file()`.
        let metadata = fs::metadata(&path).map_err(|error| {
            ContextError::resource_mismatch(format!(
                "runtime data entry is unreadable: {} ({error})",
                path.display()
            ))
        })?;
        if metadata.is_dir() {
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<[u8; 32], ContextError> {
    let bytes = fs::read(path).map_err(|error| {
        ContextError::resource_mismatch(format!(
            "runtime data file is unreadable: {} ({error})",
            path.display()
        ))
    })?;
    Ok(Sha256::digest(&bytes).into())
}

/// Lower-case hex, matching `hashlib.hexdigest()`.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
        output.push(char::from_digit(u32::from(byte & 0x0F), 16).expect("nibble"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The product version this crate compiles against must be the shipped one.
    #[test]
    fn product_version_matches_shipped_version_module() {
        let source = include_str!("../../../nioh3_scroll_editor/version.py");
        let expected = source
            .lines()
            .find_map(|line| line.strip_prefix("APP_VERSION = \""))
            .and_then(|rest| rest.split('"').next())
            .expect("version.py declares APP_VERSION");
        assert_eq!(PRODUCT_VERSION, expected);
    }

    /// The ported constants must equal the shipped module's literals.
    #[test]
    fn identity_constants_match_shipped_core_services() {
        let source = include_str!("../../../nioh3_scroll_editor/core_services.py");
        for (name, value) in [
            ("GENERATION_ALGORITHM_VERSION", GENERATION_ALGORITHM_VERSION),
            ("OPERATION_POLICY_VERSION", OPERATION_POLICY_VERSION),
            ("SUPPORTED_GAME_PROFILE", SUPPORTED_GAME_PROFILE),
        ] {
            let needle = format!("{name} = \"{value}\"");
            assert!(
                source.contains(&needle),
                "core_services.py no longer declares {needle}"
            );
        }
    }

    #[test]
    fn context_digest_covers_exactly_the_seven_identity_fields() {
        let context = capture_context(SUPPORTED_GAME_PROFILE, &product_data_root(), None)
            .expect("capture the product context");
        assert_eq!(context.product_version, PRODUCT_VERSION);
        assert_eq!(context.resources_digest.len(), 64);
        assert_eq!(context.seed_accelerator_abi, None);
        assert_eq!(context.seed_accelerator_build_id, None);

        let canonical = format!(
            "{{\"algorithm_version\":\"{GENERATION_ALGORITHM_VERSION}\",\
             \"game_profile\":\"{SUPPORTED_GAME_PROFILE}\",\
             \"policy_version\":\"{OPERATION_POLICY_VERSION}\",\
             \"product_version\":\"{PRODUCT_VERSION}\",\
             \"resources_digest\":\"{}\",\
             \"seed_accelerator_abi\":null,\
             \"seed_accelerator_build_id\":null}}",
            context.resources_digest
        );
        assert_eq!(
            context.context_digest,
            hex_lower(&Sha256::digest(canonical.as_bytes()))
        );
    }

    #[test]
    fn missing_product_data_fails_closed() {
        let error = runtime_resource_digest(Path::new("definitely-not-present"))
            .expect_err("a missing data root must fail closed");
        assert_eq!(error.code, "RESOURCE_MISMATCH");
    }

    fn product_data_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }
}
