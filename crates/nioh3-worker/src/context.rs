//! Generation identity ported from `nioh3_scroll_editor/core_services.py`.
//!
//! Two identities exist here, and only one of them may authorize work:
//!
//! * [`ResolvedGenerationContext`] is the production identity. It folds the
//!   exact installed game `file_version` and the version-bound selected-bundle
//!   digests from `nioh3_data::resolve_selected_generation_bundle`, so two data
//!   roots that only differ by which bundle they resolve no longer share an
//!   identity. Every canonical digest input is explicit and version-bound.
//! * [`LegacyGenerationContext`] is the pre-version identity: the fixed profile
//!   plus the whole-root resource digest. It survives only as an explicit
//!   proof/diagnostic field (`legacy_context_digest`) and an opt-in, visibly
//!   non-production test mode. It never authorizes candidate, cache, or resume
//!   reuse.
//!
//! Both payloads are canonically encoded as JSON with sorted keys and `,`/`:`
//! separators, then SHA-256 hashed. The digest itself is never part of its own
//! hashed payload, and neither is any private record pair.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::native::AcceleratorIdentity;
use nioh3_data::{resolve_selected_generation_bundle, SelectedGenerationBundle};

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
pub const PRODUCT_VERSION: &str = "0.8.0";

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

/// The exact installed game executable version a production context is bound to.
///
/// This is a four-part Windows file version such as `(2, 0, 2, 0)`. It is never
/// inferred or defaulted: the worker only resolves a context for a version the
/// caller supplied explicitly, so a missing or unknown version fails closed
/// instead of silently selecting the shipped `CURRENT_RESOURCE_VERSION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameFileVersion(pub u16, pub u16, pub u16, pub u16);

impl GameFileVersion {
    /// Dotted four-part spelling, matching the manifest and Python `str(version)`.
    pub fn dotted(&self) -> String {
        format!("{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

/// Generation identity bound to one exact installed game version.
///
/// This is the production identity. `context_digest` folds every field below in
/// canonical order, including the exact `game_file_version` and the selected
/// bundle digests, so contexts that resolve different bundles never collide.
/// `legacy_context_digest` is carried only as an explicit proof/diagnostic value;
/// no cache, candidate, or resume authority is derived from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGenerationContext {
    pub product_version: String,
    pub game_profile: String,
    /// Exact installed executable version this identity was resolved for.
    pub game_file_version: GameFileVersion,
    /// Version-selected offline resource directory the bundle resolved to.
    pub versioned_resource_dir: String,
    /// Identity over every selected input, including version-invariant ones.
    pub bundle_digest: String,
    /// Identity over only the files below the versioned resource directory.
    pub versioned_digest: String,
    /// Whole-root digest, retained as proof/diagnostic only.
    pub resources_digest: String,
    pub algorithm_version: String,
    pub policy_version: String,
    pub seed_accelerator_abi: Option<i64>,
    pub seed_accelerator_build_id: Option<String>,
    /// Pre-version identity digest, carried only as an explicit proof field.
    pub legacy_context_digest: String,
    /// The version-bound production digest that authorizes reuse.
    pub context_digest: String,
}

impl ResolvedGenerationContext {
    /// The handshake payload the worker publishes for a production launch.
    ///
    /// The proof fields (`game_file_version`, `versioned_resource_dir`,
    /// `bundle_digest`, `versioned_digest`, `legacy_context_digest`) are additive
    /// so an old reader can still recognize every legacy key.
    pub fn to_payload(&self) -> Value {
        let mut payload = self.base_payload();
        payload.insert(
            "game_file_version".to_string(),
            Value::String(self.game_file_version.dotted()),
        );
        payload.insert(
            "versioned_resource_dir".to_string(),
            Value::String(self.versioned_resource_dir.clone()),
        );
        payload.insert(
            "bundle_digest".to_string(),
            Value::String(self.bundle_digest.clone()),
        );
        payload.insert(
            "versioned_digest".to_string(),
            Value::String(self.versioned_digest.clone()),
        );
        payload.insert(
            "legacy_context_digest".to_string(),
            Value::String(self.legacy_context_digest.clone()),
        );
        payload.insert("production_authority".to_string(), Value::Bool(true));
        Value::Object(payload)
    }

    /// Shared legacy-compatible keys, before the version-bound proof fields.
    fn base_payload(&self) -> Map<String, Value> {
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
        payload
    }
}

/// Canonical digest inputs for the production identity, in sorted-key order.
///
/// Kept as one function so Python and Rust fold byte-identical JSON. Every value
/// that can change a generated result is present, and `context_digest` itself is
/// deliberately excluded.
///
/// The wide positional signature is deliberate: it mirrors the canonical field
/// order the Python mirror ticket must encode, so a caller cannot accidentally
/// drop an identity field while adapting to a narrower argument list.
#[allow(clippy::too_many_arguments)]
pub fn resolved_identity_payload(
    product_version: &str,
    game_profile: &str,
    game_file_version: GameFileVersion,
    versioned_resource_dir: &str,
    bundle_digest: &str,
    versioned_digest: &str,
    resources_digest: &str,
    algorithm_version: &str,
    policy_version: &str,
    seed_accelerator_abi: Option<i64>,
    seed_accelerator_build_id: Option<&str>,
) -> String {
    let mut payload = Map::new();
    payload.insert(
        "algorithm_version".to_string(),
        Value::String(algorithm_version.to_string()),
    );
    payload.insert(
        "bundle_digest".to_string(),
        Value::String(bundle_digest.to_string()),
    );
    payload.insert(
        "game_file_version".to_string(),
        Value::String(game_file_version.dotted()),
    );
    payload.insert(
        "game_profile".to_string(),
        Value::String(game_profile.to_string()),
    );
    payload.insert(
        "policy_version".to_string(),
        Value::String(policy_version.to_string()),
    );
    payload.insert(
        "product_version".to_string(),
        Value::String(product_version.to_string()),
    );
    payload.insert(
        "resources_digest".to_string(),
        Value::String(resources_digest.to_string()),
    );
    payload.insert(
        "seed_accelerator_abi".to_string(),
        match seed_accelerator_abi {
            Some(abi) => Value::Number(abi.into()),
            None => Value::Null,
        },
    );
    payload.insert(
        "seed_accelerator_build_id".to_string(),
        match seed_accelerator_build_id {
            Some(build_id) => Value::String(build_id.to_string()),
            None => Value::Null,
        },
    );
    payload.insert(
        "versioned_digest".to_string(),
        Value::String(versioned_digest.to_string()),
    );
    payload.insert(
        "versioned_resource_dir".to_string(),
        Value::String(versioned_resource_dir.to_string()),
    );
    // `serde_json::Map` is a `BTreeMap`, so this already matches Python's
    // `sort_keys=True`; the default compact separators match `(',', ':')`.
    Value::Object(payload).to_string()
}

/// Canonical digest inputs for the pre-version (legacy) identity.
///
/// This is the exact seven-key payload the shipped worker hashed. It remains
/// reachable so the legacy digest can be published as a proof field and so the
/// opt-in legacy test mode can reproduce historical values; it never authorizes
/// production reuse.
pub fn legacy_identity_payload(
    product_version: &str,
    game_profile: &str,
    resources_digest: &str,
    algorithm_version: &str,
    policy_version: &str,
    seed_accelerator_abi: Option<i64>,
    seed_accelerator_build_id: Option<&str>,
) -> String {
    let mut payload = Map::new();
    payload.insert(
        "algorithm_version".to_string(),
        Value::String(algorithm_version.to_string()),
    );
    payload.insert(
        "game_profile".to_string(),
        Value::String(game_profile.to_string()),
    );
    payload.insert(
        "policy_version".to_string(),
        Value::String(policy_version.to_string()),
    );
    payload.insert(
        "product_version".to_string(),
        Value::String(product_version.to_string()),
    );
    payload.insert(
        "resources_digest".to_string(),
        Value::String(resources_digest.to_string()),
    );
    payload.insert(
        "seed_accelerator_abi".to_string(),
        match seed_accelerator_abi {
            Some(abi) => Value::Number(abi.into()),
            None => Value::Null,
        },
    );
    payload.insert(
        "seed_accelerator_build_id".to_string(),
        match seed_accelerator_build_id {
            Some(build_id) => Value::String(build_id.to_string()),
            None => Value::Null,
        },
    );
    Value::Object(payload).to_string()
}

/// Capture the version-bound production identity for `data_root`.
///
/// `file_version` must be supplied explicitly by the caller. An unknown or
/// unregistered version fails closed before the data root is read; there is no
/// silent `CURRENT_RESOURCE_VERSION` fallback on this path.
pub fn capture_resolved_context(
    game_profile: &str,
    data_root: &Path,
    file_version: GameFileVersion,
    accelerator: Option<AcceleratorIdentity>,
) -> Result<ResolvedGenerationContext, ContextError> {
    let GameFileVersion(major, minor, patch, build) = file_version;
    let bundle = resolve_selected_generation_bundle(data_root, (major, minor, patch, build))
        .map_err(|error| ContextError::resource_mismatch(error.to_string()))?;
    capture_resolved_context_from_bundle(
        game_profile,
        data_root,
        file_version,
        accelerator,
        &bundle,
    )
}

/// Capture the production identity from an already-resolved selected bundle.
///
/// The caller must have resolved `bundle` for `file_version`; this split keeps
/// the fail-closed version check in the resolver and lets a caller that already
/// paid for resolution reuse the same snapshot for both the identity and the
/// loaders.
pub fn capture_resolved_context_from_bundle(
    game_profile: &str,
    data_root: &Path,
    file_version: GameFileVersion,
    accelerator: Option<AcceleratorIdentity>,
    bundle: &SelectedGenerationBundle,
) -> Result<ResolvedGenerationContext, ContextError> {
    let (major, minor, patch, build) = bundle.file_version;
    if GameFileVersion(major, minor, patch, build) != file_version {
        return Err(ContextError::resource_mismatch(format!(
            "selected bundle is for {} but the context was requested for {}",
            GameFileVersion(major, minor, patch, build).dotted(),
            file_version.dotted()
        )));
    }
    let resources_digest = runtime_resource_digest(data_root)?;
    let seed_accelerator_abi = accelerator.as_ref().map(|identity| i64::from(identity.abi));
    let seed_accelerator_build_id = accelerator.map(|identity| identity.build_id);

    let legacy_canonical = legacy_identity_payload(
        PRODUCT_VERSION,
        game_profile,
        &resources_digest,
        GENERATION_ALGORITHM_VERSION,
        OPERATION_POLICY_VERSION,
        seed_accelerator_abi,
        seed_accelerator_build_id.as_deref(),
    );
    let legacy_context_digest = hex_lower(&Sha256::digest(legacy_canonical.as_bytes()));

    let canonical = resolved_identity_payload(
        PRODUCT_VERSION,
        game_profile,
        file_version,
        bundle.versioned_resource_dir,
        &bundle.bundle_digest,
        &bundle.versioned_digest,
        &resources_digest,
        GENERATION_ALGORITHM_VERSION,
        OPERATION_POLICY_VERSION,
        seed_accelerator_abi,
        seed_accelerator_build_id.as_deref(),
    );
    let context_digest = hex_lower(&Sha256::digest(canonical.as_bytes()));

    Ok(ResolvedGenerationContext {
        product_version: PRODUCT_VERSION.to_string(),
        game_profile: game_profile.to_string(),
        game_file_version: file_version,
        versioned_resource_dir: bundle.versioned_resource_dir.to_string(),
        bundle_digest: bundle.bundle_digest.clone(),
        versioned_digest: bundle.versioned_digest.clone(),
        resources_digest,
        algorithm_version: GENERATION_ALGORITHM_VERSION.to_string(),
        policy_version: OPERATION_POLICY_VERSION.to_string(),
        seed_accelerator_abi,
        seed_accelerator_build_id,
        legacy_context_digest,
        context_digest,
    })
}

/// Opt-in, visibly non-production legacy identity.
///
/// This reproduces the pre-version digest for tests and diagnostics only. A
/// context captured this way is not a [`ResolvedGenerationContext`] and so can
/// never authorize a candidate, cache, or resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyGenerationContext {
    pub product_version: String,
    pub game_profile: String,
    pub resources_digest: String,
    pub algorithm_version: String,
    pub policy_version: String,
    pub seed_accelerator_abi: Option<i64>,
    pub seed_accelerator_build_id: Option<String>,
    pub context_digest: String,
}

impl LegacyGenerationContext {
    /// Legacy handshake payload, marked so a reader cannot mistake it for
    /// production authority.
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
            "legacy_context_digest".to_string(),
            Value::String(self.context_digest.clone()),
        );
        payload.insert(
            "context_digest".to_string(),
            Value::String(self.context_digest.clone()),
        );
        payload.insert("production_authority".to_string(), Value::Bool(false));
        Value::Object(payload)
    }
}

/// Capture the explicit legacy identity for `data_root`.
///
/// Reachable only from parity tests and diagnostics; `Engine::load` never calls
/// it, so a production launch cannot fall back to the pre-version digest.
pub fn capture_legacy_context(
    game_profile: &str,
    data_root: &Path,
    accelerator: Option<AcceleratorIdentity>,
) -> Result<LegacyGenerationContext, ContextError> {
    let resources_digest = runtime_resource_digest(data_root)?;
    let seed_accelerator_abi = accelerator.as_ref().map(|identity| i64::from(identity.abi));
    let seed_accelerator_build_id = accelerator.map(|identity| identity.build_id);
    let canonical = legacy_identity_payload(
        PRODUCT_VERSION,
        game_profile,
        &resources_digest,
        GENERATION_ALGORITHM_VERSION,
        OPERATION_POLICY_VERSION,
        seed_accelerator_abi,
        seed_accelerator_build_id.as_deref(),
    );
    let context_digest = hex_lower(&Sha256::digest(canonical.as_bytes()));
    Ok(LegacyGenerationContext {
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

    /// The explicit legacy capture still folds exactly the seven pre-version
    /// fields, and it is the only path that produces that digest.
    #[test]
    fn legacy_digest_covers_exactly_the_seven_pre_version_fields() {
        let context = capture_legacy_context(SUPPORTED_GAME_PROFILE, &product_data_root(), None)
            .expect("capture the legacy product context");
        assert_eq!(context.product_version, PRODUCT_VERSION);
        assert_eq!(context.resources_digest.len(), 64);
        assert_eq!(context.seed_accelerator_abi, None);
        assert_eq!(context.seed_accelerator_build_id, None);

        let canonical = legacy_identity_payload(
            PRODUCT_VERSION,
            SUPPORTED_GAME_PROFILE,
            &context.resources_digest,
            GENERATION_ALGORITHM_VERSION,
            OPERATION_POLICY_VERSION,
            None,
            None,
        );
        assert_eq!(
            context.context_digest,
            hex_lower(&Sha256::digest(canonical.as_bytes()))
        );
        assert_eq!(
            context.to_payload()["production_authority"],
            Value::Bool(false)
        );
    }

    /// The production digest is version-bound and folds the proof fields.
    #[test]
    fn resolved_digest_is_version_bound_and_folds_the_proof_fields() {
        let legacy_version = GameFileVersion(2, 0, 0, 2);
        let v202 = GameFileVersion(2, 0, 2, 0);
        let legacy = capture_resolved_context(
            SUPPORTED_GAME_PROFILE,
            &product_data_root(),
            legacy_version,
            None,
        )
        .expect("legacy version resolves a production context");
        let newer =
            capture_resolved_context(SUPPORTED_GAME_PROFILE, &product_data_root(), v202, None)
                .expect("v2.02 resolves a production context");

        // Same inputs, different exact game version: distinct production digests.
        assert_ne!(legacy.context_digest, newer.context_digest);
        assert_ne!(legacy.bundle_digest, newer.bundle_digest);
        assert_eq!(legacy.game_file_version, legacy_version);
        assert_eq!(newer.game_file_version, v202);
        assert_eq!(
            legacy.versioned_resource_dir,
            "r4_finalizer/pc_v2_00_02/resource_v1"
        );
        assert_eq!(
            newer.versioned_resource_dir,
            "r4_finalizer/pc_v2_02/resource_v1"
        );

        // The legacy digest is retained only as an explicit proof field.
        let proof = capture_legacy_context(SUPPORTED_GAME_PROFILE, &product_data_root(), None)
            .expect("legacy proof");
        assert_eq!(legacy.legacy_context_digest, proof.context_digest);
        assert_eq!(newer.legacy_context_digest, proof.context_digest);
        assert_ne!(legacy.context_digest, legacy.legacy_context_digest);

        // The published payload carries the proof fields and production authority.
        let payload = legacy.to_payload();
        for field in [
            "game_file_version",
            "versioned_resource_dir",
            "bundle_digest",
            "versioned_digest",
            "legacy_context_digest",
        ] {
            assert!(payload.get(field).is_some(), "payload is missing {field}");
        }
        assert_eq!(
            payload["game_file_version"],
            Value::String("2.0.0.2".into())
        );
        assert_eq!(payload["production_authority"], Value::Bool(true));
        assert_eq!(
            payload["context_digest"],
            Value::String(legacy.context_digest.clone())
        );
    }

    /// An unknown or missing explicit version must fail closed, never default.
    #[test]
    fn unknown_file_version_fails_closed_without_a_default() {
        let error = capture_resolved_context(
            SUPPORTED_GAME_PROFILE,
            &product_data_root(),
            GameFileVersion(9, 9, 9, 9),
            None,
        )
        .expect_err("an unregistered version must fail closed");
        assert_eq!(error.code, "RESOURCE_MISMATCH");
        // Fail closed before touching the data root: a missing root with an
        // unknown version still reports the version, not the root.
        let missing = capture_resolved_context(
            SUPPORTED_GAME_PROFILE,
            Path::new("definitely-not-present"),
            GameFileVersion(9, 9, 9, 9),
            None,
        )
        .expect_err("an unregistered version must fail closed before the root is read");
        assert_eq!(missing.code, "RESOURCE_MISMATCH");
    }

    /// A resolved context must be built from a bundle resolved for that version.
    #[test]
    fn resolved_context_rejects_a_bundle_for_a_different_version() {
        let bundle = resolve_selected_generation_bundle(&product_data_root(), (2, 0, 2, 0))
            .expect("resolve the v2.02 bundle");
        let error = capture_resolved_context_from_bundle(
            SUPPORTED_GAME_PROFILE,
            &product_data_root(),
            GameFileVersion(2, 0, 0, 2),
            None,
            &bundle,
        )
        .expect_err("a mismatched bundle must be refused");
        assert_eq!(error.code, "RESOURCE_MISMATCH");
    }

    /// The canonical JSON the digest folds has sorted keys and compact separators.
    #[test]
    fn resolved_payload_is_canonical_and_matches_python_field_order() {
        let canonical = resolved_identity_payload(
            PRODUCT_VERSION,
            SUPPORTED_GAME_PROFILE,
            GameFileVersion(2, 0, 2, 0),
            "r4_finalizer/pc_v2_02/resource_v1",
            "aa",
            "bb",
            "cc",
            GENERATION_ALGORITHM_VERSION,
            OPERATION_POLICY_VERSION,
            None,
            None,
        );
        assert_eq!(
            canonical,
            format!(
                "{{\"algorithm_version\":\"{GENERATION_ALGORITHM_VERSION}\",\
                 \"bundle_digest\":\"aa\",\
                 \"game_file_version\":\"2.0.2.0\",\
                 \"game_profile\":\"{SUPPORTED_GAME_PROFILE}\",\
                 \"policy_version\":\"{OPERATION_POLICY_VERSION}\",\
                 \"product_version\":\"{PRODUCT_VERSION}\",\
                 \"resources_digest\":\"cc\",\
                 \"seed_accelerator_abi\":null,\
                 \"seed_accelerator_build_id\":null,\
                 \"versioned_digest\":\"bb\",\
                 \"versioned_resource_dir\":\"r4_finalizer/pc_v2_02/resource_v1\"}}"
            )
        );
    }

    /// Pinned cross-language golden for the production canonical payload.
    ///
    /// These exact bytes and digests are the contract the Python mirror ticket
    /// must reproduce. The payload is the *only* thing hashed; the digest is not
    /// part of its own input. Field order is sorted, separators are `,`/`:`, and
    /// the version is an unambiguous dotted string rather than a list or object.
    #[test]
    fn production_canonical_payload_and_digest_are_pinned_goldens() {
        const V202_CANONICAL: &str = concat!(
            "{\"algorithm_version\":\"scroll-generation-v0.7-native-completion-1\",",
            "\"bundle_digest\":\"d1fb81af3bfc8e577239c2facdb097c320c9ab8b0a88e9fea792b1819c5584b2\",",
            "\"game_file_version\":\"2.0.2.0\",",
            "\"game_profile\":\"pc-v2.00.02-v2.01\",",
            "\"policy_version\":\"operation-policy-v1\",",
            "\"product_version\":\"0.8.0\",",
            "\"resources_digest\":\"411866d772e8e2450c1f4becd4c5e79761600f7766ab0659438bcbde21997048\",",
            "\"seed_accelerator_abi\":null,",
            "\"seed_accelerator_build_id\":null,",
            "\"versioned_digest\":\"09fa65803a0c058880f4d38900290b152eab89febb03615ce2576f4b020b358b\",",
            "\"versioned_resource_dir\":\"r4_finalizer/pc_v2_02/resource_v1\"}",
        );
        const V202_CONTEXT_DIGEST: &str =
            "6f1292895f25937005f736b3170ccfd11b295aa7c284d3744339f6bbfd1a8712";
        const LEGACY_CANONICAL: &str = concat!(
            "{\"algorithm_version\":\"scroll-generation-v0.7-native-completion-1\",",
            "\"game_profile\":\"pc-v2.00.02-v2.01\",",
            "\"policy_version\":\"operation-policy-v1\",",
            "\"product_version\":\"0.8.0\",",
            "\"resources_digest\":\"411866d772e8e2450c1f4becd4c5e79761600f7766ab0659438bcbde21997048\",",
            "\"seed_accelerator_abi\":null,",
            "\"seed_accelerator_build_id\":null}",
        );
        const LEGACY_DIGEST: &str =
            "4a38a6d3d14b3a2c24bbb946c9595d30b1098662b07299af2b617e927052d61f";

        let context = capture_resolved_context(
            SUPPORTED_GAME_PROFILE,
            &product_data_root(),
            GameFileVersion(2, 0, 2, 0),
            None,
        )
        .expect("v2.02 resolves a production context");

        let canonical = resolved_identity_payload(
            PRODUCT_VERSION,
            SUPPORTED_GAME_PROFILE,
            context.game_file_version,
            &context.versioned_resource_dir,
            &context.bundle_digest,
            &context.versioned_digest,
            &context.resources_digest,
            GENERATION_ALGORITHM_VERSION,
            OPERATION_POLICY_VERSION,
            context.seed_accelerator_abi,
            context.seed_accelerator_build_id.as_deref(),
        );
        assert_eq!(canonical, V202_CANONICAL);
        assert_eq!(
            context.context_digest,
            hex_lower(&Sha256::digest(V202_CANONICAL.as_bytes()))
        );
        assert_eq!(context.context_digest, V202_CONTEXT_DIGEST);

        let legacy_canonical = legacy_identity_payload(
            PRODUCT_VERSION,
            SUPPORTED_GAME_PROFILE,
            &context.resources_digest,
            GENERATION_ALGORITHM_VERSION,
            OPERATION_POLICY_VERSION,
            None,
            None,
        );
        assert_eq!(legacy_canonical, LEGACY_CANONICAL);
        assert_eq!(context.legacy_context_digest, LEGACY_DIGEST);
        assert_ne!(context.context_digest, context.legacy_context_digest);
    }

    /// Every resolved identity field is load-bearing: mutating any one of them
    /// changes the production digest.
    #[test]
    fn each_resolved_identity_field_mutation_changes_the_primary_digest() {
        let context = capture_resolved_context(
            SUPPORTED_GAME_PROFILE,
            &product_data_root(),
            GameFileVersion(2, 0, 2, 0),
            None,
        )
        .expect("v2.02 resolves a production context");
        let baseline = context.context_digest.clone();

        // The canonical encoder is the digest authority, so a field-level
        // mutation is expressed by re-encoding with one value changed. This
        // keeps the assertion independent of which bundle fixtures exist.
        let variants: Vec<(&str, String)> = vec![
            (
                "game_file_version",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    GameFileVersion(2, 0, 0, 2),
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "versioned_resource_dir",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    "r4_finalizer/pc_v2_00_02/resource_v1",
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "bundle_digest",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    "0".repeat(64).as_str(),
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "versioned_digest",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    "0".repeat(64).as_str(),
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "resources_digest",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &"0".repeat(64),
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "game_profile",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    "pc-v0.0.0-v0.0",
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "product_version",
                resolved_identity_payload(
                    "0.0.0",
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "algorithm_version",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    "scroll-generation-v0",
                    OPERATION_POLICY_VERSION,
                    None,
                    None,
                ),
            ),
            (
                "policy_version",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    "operation-policy-v0",
                    None,
                    None,
                ),
            ),
            (
                "seed_accelerator_abi",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    Some(1),
                    None,
                ),
            ),
            (
                "seed_accelerator_build_id",
                resolved_identity_payload(
                    PRODUCT_VERSION,
                    SUPPORTED_GAME_PROFILE,
                    context.game_file_version,
                    &context.versioned_resource_dir,
                    &context.bundle_digest,
                    &context.versioned_digest,
                    &context.resources_digest,
                    GENERATION_ALGORITHM_VERSION,
                    OPERATION_POLICY_VERSION,
                    None,
                    Some("mutant-build"),
                ),
            ),
        ];

        for (field, canonical) in variants {
            let digest = hex_lower(&Sha256::digest(canonical.as_bytes()));
            assert_ne!(
                digest, baseline,
                "mutating {field} must change the production digest"
            );
        }
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
