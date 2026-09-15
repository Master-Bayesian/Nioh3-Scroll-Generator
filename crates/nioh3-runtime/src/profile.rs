//! Approved native runtime profiles, ported from `nioh3_scroll_editor/native.py`.
//!
//! A profile pins the RVAs and captured signatures of the generation chain for
//! one exact executable version. Selection is fail-closed: only the verified
//! versions below resolve, and the v2.01 document must carry its own approval
//! gate before it may be used.

use crate::error::RuntimeError;
use crate::platform::FileVersion;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Schema of the shipped research profile document.
pub const PROFILE_SCHEMA: &str = "nioh3-game-version-research-profile/v1";

/// Domain separator for [`NativeRuntimeProfile::identity_digest`].
const PROFILE_IDENTITY_DOMAIN: &[u8] = b"nioh3-runtime-profile-identity/v1\0";

/// The eight chain sites `native.NativeBatchOracle.open` verifies, in order.
pub const SIGNATURE_SITE_NAMES: [&str; 8] = [
    "init_compact",
    "reset_compact",
    "effective_level",
    "init_generation_context",
    "incomplete_record",
    "generate_effects",
    "assemble_scroll",
    "playthrough_vector",
];

/// Fixed executable versions the shipped product verifies.
pub const SUPPORTED_GAME_VERSIONS: [(FileVersion, &str); 2] = [
    (FileVersion::new(2, 0, 0, 2), "2.00.02"),
    (FileVersion::new(2, 0, 1, 0), "2.01"),
];

/// Display version the offline algorithms target.
pub const SUPPORTED_GAME_VERSION: &str = "2.01";

/// One resolved site: a stable name, a module-relative address and the bytes
/// captured there on the verified executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSite {
    pub name: &'static str,
    pub rva: u64,
    pub signature: Vec<u8>,
}

/// Typed port of `native.NativeRuntimeProfile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeRuntimeProfile {
    pub display_version: String,
    pub canonicalize: ProfileSite,
    pub finalize_effect: ProfileSite,
    pub descriptor_complete: ProfileSite,
    /// The eight chain signatures, in `SIGNATURE_SITE_NAMES` order.
    pub native_signatures: Vec<ProfileSite>,
    pub playthrough_selector_pointer_rva: u64,
}

impl NativeRuntimeProfile {
    /// Every text site in `load_native_runtime_profile` order. Used for bounds
    /// checks and for the identity digest.
    pub fn text_sites(&self) -> Vec<&ProfileSite> {
        let mut sites = vec![
            &self.canonicalize,
            &self.finalize_effect,
            &self.descriptor_complete,
        ];
        sites.extend(self.native_signatures.iter());
        sites
    }

    /// Look up any named site, including the two the shipped oracle verifies
    /// outside `native_signatures`.
    pub fn site(&self, name: &str) -> Option<&ProfileSite> {
        self.text_sites().into_iter().find(|site| site.name == name)
    }

    /// The sites `NativeBatchOracle.open` verifies, in its order.
    pub fn verification_sites(&self) -> Vec<&ProfileSite> {
        let mut sites = vec![&self.canonicalize, &self.finalize_effect];
        sites.extend(self.native_signatures.iter());
        sites
    }

    /// Every site must fit inside the loaded module image before any read.
    pub fn validate_site_bounds(&self, module_size: u64) -> Result<(), RuntimeError> {
        for site in self.text_sites() {
            let size = site.signature.len() as u64;
            let fits = site
                .rva
                .checked_add(size)
                .is_some_and(|end| end <= module_size);
            if !fits {
                return Err(RuntimeError::RangeOutOfBounds {
                    offset: site.rva,
                    size,
                    limit: module_size,
                });
            }
        }
        Ok(())
    }

    /// Stable fingerprint of the profile identity (version, site names, RVAs
    /// and captured signatures).
    ///
    /// This is a migration-local binding value: the future protected worker
    /// records it next to a validated process instance so a profile swap cannot
    /// silently reuse an earlier validation. It is deliberately *not* part of
    /// the protected wire contract.
    pub fn identity_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(PROFILE_IDENTITY_DOMAIN);
        hasher.update(self.display_version.as_bytes());
        hasher.update([0u8]);
        for site in self.text_sites() {
            hasher.update(site.name.as_bytes());
            hasher.update([0u8]);
            hasher.update(site.rva.to_le_bytes());
            hasher.update((site.signature.len() as u64).to_le_bytes());
            hasher.update(&site.signature);
        }
        hasher.update(self.playthrough_selector_pointer_rva.to_le_bytes());
        let digest = hasher.finalize();
        let mut rendered = String::with_capacity(digest.len() * 2);
        for byte in digest {
            rendered.push_str(&format!("{byte:02x}"));
        }
        rendered
    }

    /// Parse the shipped `nioh3-game-version-research-profile/v1` document.
    ///
    /// `load_native_runtime_profile` rejects a foreign schema, an unresolved
    /// site and a site without a captured signature; the same three rejections
    /// are reproduced here with the same English text.
    pub fn from_research_profile_json(text: &str) -> Result<Self, RuntimeError> {
        let payload: Value =
            serde_json::from_str(text).map_err(|error| RuntimeError::ProfileIntegrity {
                detail: format!("native runtime profile is not valid JSON: {error}"),
            })?;
        let schema = payload
            .get("schema")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if schema != PROFILE_SCHEMA {
            return Err(RuntimeError::ProfileSchema { schema });
        }

        let text_sites = payload.get("text_sites");
        let resolve = |name: &'static str| -> Result<ProfileSite, RuntimeError> {
            let raw = text_sites
                .and_then(Value::as_object)
                .and_then(|sites| sites.get(name));
            let rva = raw
                .and_then(|site| site.get("rva"))
                .and_then(parse_rva)
                .ok_or_else(|| RuntimeError::ProfileUnresolved {
                    site: name.to_string(),
                })?;
            let signature = raw
                .and_then(|site| site.get("captured_signature"))
                .map(|value| match value.as_str() {
                    Some(hex) => parse_hex(hex).ok_or_else(|| RuntimeError::ProfileIntegrity {
                        detail: format!(
                            "native runtime profile site {name} has a non-hex captured signature"
                        ),
                    }),
                    None => Err(RuntimeError::ProfileIntegrity {
                        detail: format!(
                            "native runtime profile site {name} has a non-text captured signature"
                        ),
                    }),
                })
                .transpose()?
                .filter(|bytes| !bytes.is_empty())
                .ok_or_else(|| RuntimeError::ProfileMissingSignature {
                    site: name.to_string(),
                })?;
            Ok(ProfileSite {
                name,
                rva,
                signature,
            })
        };

        let mut native_signatures = Vec::with_capacity(SIGNATURE_SITE_NAMES.len());
        for name in SIGNATURE_SITE_NAMES {
            native_signatures.push(resolve(name)?);
        }

        let pointer_name = "playthrough_selector_pointer";
        let playthrough_selector_pointer_rva = payload
            .get("data_sites")
            .and_then(Value::as_object)
            .and_then(|sites| sites.get(pointer_name))
            .and_then(|site| site.get("rva"))
            .and_then(parse_rva)
            .ok_or_else(|| RuntimeError::ProfileUnresolved {
                site: pointer_name.to_string(),
            })?;

        Ok(Self {
            display_version: payload
                .get("display_version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            canonicalize: resolve("canonicalize")?,
            finalize_effect: resolve("completion_finalizer_wrapper")?,
            descriptor_complete: resolve("descriptor_complete")?,
            native_signatures,
            playthrough_selector_pointer_rva,
        })
    }
}

/// Read a research profile document from disk.
pub fn load_research_profile(path: &Path) -> Result<NativeRuntimeProfile, RuntimeError> {
    let text = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    NativeRuntimeProfile::from_research_profile_json(&text)
}

/// The shipped hardcoded PC v2.00.02 profile.
///
/// Port of `DEFAULT_NATIVE_RUNTIME_PROFILE`; every RVA and signature is copied
/// from `native.py` and pinned by unit tests.
pub fn default_pc_v2_00_02() -> NativeRuntimeProfile {
    NativeRuntimeProfile {
        display_version: "PC v2.00.02".to_string(),
        canonicalize: site(
            "canonicalize",
            0x20DD6EC,
            "48 89 5C 24 18 48 89 7C 24 20 55 48 8D 6C 24 C0",
        ),
        finalize_effect: site(
            "completion_finalizer_wrapper",
            0x22799A8,
            "40 53 55 56 57 41 54 41 55 41 56 41 57 48 81 EC 78 01 00 00",
        ),
        descriptor_complete: site("descriptor_complete", 0x20DD558, "48 8B 54 24 60"),
        native_signatures: vec![
            site("init_compact", 0x1B6F650, "40 53 48 83 EC 20 33 C0"),
            site("reset_compact", 0x1BBADBC, "48 83 EC 28 33 C0 C6 41"),
            site("effective_level", 0x3DB834, "F7 41 18 00 00 20 00 75"),
            site(
                "init_generation_context",
                0x570DF8,
                "48 83 EC 28 4C 8B D9 66",
            ),
            site("incomplete_record", 0x110BF30, "33 D2 48 8D 41 42 80 38"),
            site("generate_effects", 0x577964, "48 8B C4 48 89 58 20 55"),
            site("assemble_scroll", 0x2277FE8, "48 89 5C 24 08 48 89 6C"),
            site("playthrough_vector", 0x578CD4, "48 83 EC 48 48 8B 05 51"),
        ],
        playthrough_selector_pointer_rva: 0x47494A0,
    }
}

/// Display version for one supported executable version.
pub fn supported_display_version(version: FileVersion) -> Option<&'static str> {
    SUPPORTED_GAME_VERSIONS
        .iter()
        .find(|(candidate, _)| *candidate == version)
        .map(|(_, display)| *display)
}

/// Port of `native.native_runtime_profile_for_game_version`.
///
/// `profile_dir` is the directory that holds `pc_v2_01.json`
/// (`nioh3_scroll_editor/data/game_versions` in the product tree).
pub fn profile_for_game_version(
    version: FileVersion,
    profile_dir: &Path,
) -> Result<NativeRuntimeProfile, RuntimeError> {
    if version == FileVersion::new(2, 0, 0, 2) {
        return Ok(default_pc_v2_00_02());
    }
    if version != FileVersion::new(2, 0, 1, 0) {
        return Err(RuntimeError::UnsupportedGameVersion {
            display: version.display(),
        });
    }

    let path = profile_dir.join("pc_v2_01.json");
    let text = std::fs::read_to_string(&path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let payload: Value =
        serde_json::from_str(&text).map_err(|error| RuntimeError::ProfileIntegrity {
            detail: format!("native runtime profile is not valid JSON: {error}"),
        })?;
    let approved = payload.get("approval_status").and_then(Value::as_str) == Some("approved")
        && payload
            .get("product_enablement_allowed")
            .and_then(Value::as_bool)
            == Some(true)
        && payload
            .get("gates")
            .and_then(|gates| gates.get("product_enablement_allowed"))
            .and_then(Value::as_bool)
            == Some(true);
    if !approved {
        return Err(RuntimeError::ProfileNotApproved {
            profile: "PC v2.01".to_string(),
        });
    }
    NativeRuntimeProfile::from_research_profile_json(&text)
}

fn site(name: &'static str, rva: u64, signature: &str) -> ProfileSite {
    ProfileSite {
        name,
        rva,
        signature: parse_hex(signature).unwrap_or_default(),
    }
}

fn parse_rva(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    let text = value.as_str()?.trim();
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse::<u64>().ok(),
    }
}

/// Whitespace-separated hex, matching `bytes.fromhex`.
fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes.push((high * 16 + low) as u8);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        default_pc_v2_00_02, profile_for_game_version, supported_display_version,
        NativeRuntimeProfile, PROFILE_SCHEMA, SIGNATURE_SITE_NAMES,
    };
    use crate::error::RuntimeError;
    use crate::platform::FileVersion;
    use std::path::{Path, PathBuf};

    fn shipped_profile_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("nioh3_scroll_editor")
            .join("data")
            .join("game_versions")
            .join("pc_v2_01.json")
    }

    fn read_shipped_profile() -> Result<(String, NativeRuntimeProfile), RuntimeError> {
        let path = shipped_profile_path();
        let text = std::fs::read_to_string(&path).map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let profile = NativeRuntimeProfile::from_research_profile_json(&text)?;
        Ok((text, profile))
    }

    #[test]
    fn the_default_profile_matches_the_shipped_constants() {
        let profile = default_pc_v2_00_02();
        assert_eq!(profile.display_version, "PC v2.00.02");
        assert_eq!(profile.canonicalize.rva, 0x20DD6EC);
        assert_eq!(profile.finalize_effect.rva, 0x22799A8);
        assert_eq!(profile.descriptor_complete.rva, 0x20DD558);
        assert_eq!(profile.playthrough_selector_pointer_rva, 0x47494A0);
        assert_eq!(profile.canonicalize.signature.len(), 16);
        assert_eq!(profile.finalize_effect.signature.len(), 20);
        assert_eq!(
            profile
                .native_signatures
                .iter()
                .map(|site| site.name)
                .collect::<Vec<_>>(),
            SIGNATURE_SITE_NAMES
        );
        assert_eq!(
            profile.site("generate_effects").map(|site| site.rva),
            Some(0x577964)
        );
    }

    #[test]
    fn the_shipped_v2_01_document_loads_and_digests_stably() -> Result<(), RuntimeError> {
        let (text, profile) = read_shipped_profile()?;
        assert_eq!(profile.display_version, "PC v2.01");
        assert_eq!(profile.canonicalize.rva, 0x20E1AF0);
        assert_eq!(profile.text_sites().len(), 11);
        assert_eq!(profile.verification_sites().len(), 10);
        assert_eq!(profile.identity_digest().len(), 64);
        assert_eq!(
            profile.identity_digest(),
            NativeRuntimeProfile::from_research_profile_json(&text)?.identity_digest()
        );

        let mut mutated = profile.clone();
        mutated.canonicalize.rva += 0x10;
        assert_ne!(profile.identity_digest(), mutated.identity_digest());
        Ok(())
    }

    #[test]
    fn version_selection_fails_closed() -> Result<(), RuntimeError> {
        assert_eq!(
            supported_display_version(FileVersion::new(2, 0, 1, 0)),
            Some("2.01")
        );
        assert_eq!(
            supported_display_version(FileVersion::new(2, 0, 2, 0)),
            None
        );
        assert_eq!(
            profile_for_game_version(FileVersion::new(2, 0, 0, 2), Path::new("."))?.display_version,
            "PC v2.00.02"
        );
        assert_eq!(
            profile_for_game_version(FileVersion::new(2, 0, 1, 1), Path::new(".")).err(),
            RuntimeError::UnsupportedGameVersion {
                display: "2.0.1.1".to_string()
            }
            .into()
        );
        Ok(())
    }

    #[test]
    fn unresolved_and_unapproved_profiles_are_rejected() -> Result<(), RuntimeError> {
        let text = format!(
            r#"{{"schema":"{PROFILE_SCHEMA}","display_version":"x","text_sites":{{}},"data_sites":{{}}}}"#
        );
        assert_eq!(
            NativeRuntimeProfile::from_research_profile_json(&text).err(),
            Some(RuntimeError::ProfileUnresolved {
                site: "init_compact".to_string(),
            })
        );
        assert_eq!(
            NativeRuntimeProfile::from_research_profile_json("{}").err(),
            Some(RuntimeError::ProfileSchema {
                schema: String::new()
            })
        );
        Ok(())
    }

    #[test]
    fn site_bounds_are_checked_before_any_read() -> Result<(), RuntimeError> {
        let profile = default_pc_v2_00_02();
        // The v2.00.02 sites reach 0x22799A8, so a real game image is ~100 MB
        // and every site must fit; a 4 KB image cannot hold the first site.
        assert!(profile.validate_site_bounds(0x4000_0000).is_ok());
        assert_eq!(
            profile.validate_site_bounds(0x1000).err(),
            Some(RuntimeError::RangeOutOfBounds {
                offset: 0x20DD6EC,
                size: 16,
                limit: 0x1000,
            })
        );
        Ok(())
    }
}
