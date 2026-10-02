//! A native-picked game path. Only its path is persisted; VERSIONINFO is always
//! read from the actual file. Selection takes effect on the next Studio launch.
use crate::game_version::{
    discover_game_executable, FileVersionReader, GameFileVersion, WindowsFileVersionReader,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn named_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || !path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Nioh3.exe"))
    {
        return Err("GAME_EXECUTABLE_SELECTION_INVALID: select the actual Nioh3.exe file".into());
    }
    Ok(())
}
pub fn selected(root: &Path) -> Result<Option<PathBuf>, String> {
    let file = root.join("game-install.json");
    match std::fs::metadata(&file) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("GAME_INSTALL_CONFIG_INVALID: {error}")),
        Ok(meta) if meta.len() > 16384 => {
            return Err("GAME_INSTALL_CONFIG_INVALID: record too large".into())
        }
        Ok(_) => {}
    }
    let raw = std::fs::read(&file).map_err(|e| format!("GAME_INSTALL_CONFIG_INVALID: {e}"))?;
    let value: Value =
        serde_json::from_slice(&raw).map_err(|e| format!("GAME_INSTALL_CONFIG_INVALID: {e}"))?;
    if value["schema"] != "nioh3-game-install/v1" {
        return Err("GAME_INSTALL_CONFIG_INVALID: unsupported record".into());
    }
    if value.get("executable").is_none() {
        return Err("GAME_INSTALL_CONFIG_INVALID: missing executable field".into());
    }
    if value["executable"].is_null() {
        return Ok(None);
    }
    let path = PathBuf::from(
        value["executable"]
            .as_str()
            .ok_or("GAME_INSTALL_CONFIG_INVALID: missing executable")?,
    );
    named_executable(&path)?;
    Ok(Some(path))
}
pub fn inspect(root: &Path) -> Result<Value, String> {
    inspect_with_reader(root, &WindowsFileVersionReader, discover_game_executable)
}

/// Host-only inspection deliberately does not construct a generation context or
/// start a worker. Unknown identities can therefore explain what is missing
/// without granting resource, runtime-profile, or mutation authority.
fn inspect_with_reader(
    root: &Path,
    reader: &impl FileVersionReader,
    discover: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<Value, String> {
    // A malformed explicit selection is an error, never permission to discover
    // a different installation and report it as the player's choice.
    let selected = selected(root)?;
    let source = if selected.is_some() {
        "selected"
    } else {
        "automatic"
    };
    let path = selected.map(Ok).unwrap_or_else(discover);
    let (executable, identity) = match path {
        Ok(path) => {
            let identity = reader.read(&path);
            (Some(path.to_string_lossy().into_owned()), identity)
        }
        Err(error) => (None, Err(error)),
    };
    let (version, error) = match identity {
        Ok(version) => (Some(version), Value::Null),
        Err(message) => {
            let code = message
                .split_once(':')
                .map(|(code, _)| code)
                .unwrap_or("GAME_INSTALL_INSPECTION_FAILED");
            (None, json!({"code":code,"message":message}))
        }
    };
    Ok(json!({
        "executable":executable,
        "file_version":version.map(|v|v.dotted()),
        "restart_required":false,
        "source":source,
        "identity_error":error,
        "compatibility":compatibility(version),
    }))
}

fn compatibility(version: Option<GameFileVersion>) -> Value {
    let Some(version) = version else {
        return unavailable_compatibility("unavailable", "unavailable",
            "The executable identity could not be read. The identity_error contains the exact discovery or version-resource failure.");
    };
    let tuple = version.tuple();
    let Ok(resource_directory) = nioh3_data::r4_resource_dir_for_file_version(tuple) else {
        return unavailable_compatibility("unknown", "unsupported",
            "No generation resource bundle or runtime layout/ABI profile is registered for this exact FILEVERSION. Inspection does not prove matching structure, and no other version's tables or native addresses are substituted.");
    };
    let (major, minor, patch, build) = tuple;
    let display_version = nioh3_runtime::supported_display_version(
        nioh3_runtime::FileVersion::new(major, minor, patch, build),
    )
    .map(|label| format!("PC v{label}"));
    // This is a description of the shipped per-feature evidence, not a second
    // approval gate. Every operation still resolves and validates its own
    // resources, process identity, layout, code sites, and recovery state.
    let (data_version, profile, character, scroll, equipment, reason) = match tuple {
        (2, 0, 0, 2) => (
            "PC v2.00.02", "pc_v2_00_02", "experimental", "unsupported", "unsupported",
            "Registered generation resources exist. The older character layout is experimental and lacks current-candidate real-game acceptance. Native scroll addition and native equipment generation have no verified bindings for this version.",
        ),
        (2, 0, 1, 0) => (
            "PC v2.00.02", "pc_v2_01", "experimental", "experimental", "unsupported",
            "Registered generation resources are shared with PC v2.00.02 after byte-equal comparison. Older character layouts and native scroll bindings retain experimental support; current-candidate real-game acceptance is missing. Native equipment generation has no verified old-version builder.",
        ),
        (2, 0, 2, 0) => (
            "PC v2.02", "pc_v2_02", "supported", "supported", "supported",
            "Version-specific generation resources and feature bindings are registered. This inspection does not validate installed resource bytes or establish acceptance for this executable variant or game session.",
        ),
        _ => return unavailable_compatibility("unknown", "unsupported",
            "The resource registry knows this version, but no host per-feature compatibility evidence is registered."),
    };
    json!({
        "status":"known",
        "display_version":display_version,
        "data_version":data_version,
        "resource_directory":resource_directory,
        "runtime_profile":profile,
        "features":{
            "offline_scroll_generation":"supported",
            "character_read_edit":character,
            "native_scroll_add":scroll,
            "native_equipment_add":equipment,
        },
        "reason":format!("{reason} Live features still require their process, code, layout, ownership and recovery checks."),
    })
}

fn unavailable_compatibility(status: &str, feature: &str, reason: &str) -> Value {
    json!({
        "status":status,
        "display_version":Value::Null,
        "data_version":Value::Null,
        "resource_directory":Value::Null,
        "runtime_profile":Value::Null,
        "features":{
            "offline_scroll_generation":feature,
            "character_read_edit":feature,
            "native_scroll_add":feature,
            "native_equipment_add":feature,
        },
        "reason":reason,
    })
}
pub fn choose(root: &Path, path: &Path) -> Result<Value, String> {
    named_executable(path)?;
    if !path.is_file() {
        return Err("GAME_EXECUTABLE_UNREADABLE: selected Nioh3.exe is missing".into());
    }
    let path = path
        .canonicalize()
        .map_err(|e| format!("GAME_EXECUTABLE_UNREADABLE: {e}"))?;
    named_executable(&path)?;
    WindowsFileVersionReader.read(&path)?;
    crate::storage::write_json(
        &root.join("game-install.json"),
        &json!({"schema":"nioh3-game-install/v1","executable":path.to_string_lossy()}),
    )?;
    let mut result = inspect(root)?;
    result["restart_required"] = json!(true);
    Ok(result)
}
pub fn reset(root: &Path) -> Result<Value, String> {
    crate::storage::write_json(
        &root.join("game-install.json"),
        &json!({"schema":"nioh3-game-install/v1","executable":Value::Null}),
    )?;
    let mut result = inspect(root)?;
    result["restart_required"] = json!(true);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nioh3-game-install-inspection-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&path).expect("create isolated test state");
            Self(path)
        }

        fn select(&self, executable: &Path) {
            std::fs::write(
                self.0.join("game-install.json"),
                serde_json::to_vec(&json!({
                    "schema":"nioh3-game-install/v1","executable":executable,
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("game-install.json"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    struct Reader(Result<GameFileVersion, String>);
    impl FileVersionReader for Reader {
        fn read(&self, _path: &Path) -> Result<GameFileVersion, String> {
            self.0.clone()
        }
    }

    #[test]
    fn unknown_version_is_inspectable_without_generation_authority() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Nioh3.exe");
        let version = GameFileVersion::parse("2.0.3.0").unwrap();
        let result =
            inspect_with_reader(&fixture.0, &Reader(Ok(version)), || Ok(path.clone())).unwrap();
        assert_eq!(result["source"], "automatic");
        assert_eq!(result["executable"], path.to_string_lossy().as_ref());
        assert_eq!(result["file_version"], "2.0.3.0");
        assert!(result["identity_error"].is_null());
        assert_eq!(result["compatibility"]["status"], "unknown");
        assert!(result["compatibility"]["data_version"].is_null());
        assert!(result["compatibility"]["resource_directory"].is_null());
        assert!(result["compatibility"]["runtime_profile"].is_null());
        assert!(
            version.ensure_supported().is_err(),
            "inspection never widens worker startup"
        );
        assert!(
            !fixture.0.join("game-install.json").exists(),
            "inspection is read-only"
        );
    }

    #[test]
    fn old_resource_alias_retains_exact_identity_and_feature_limits() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Nioh3.exe");
        fixture.select(&path);
        for raw in ["2.0.0.2", "2.0.1.0", "2.0.2.0"] {
            let version = GameFileVersion::parse(raw).unwrap();
            let result = inspect_with_reader(&fixture.0, &Reader(Ok(version)), || {
                panic!("an explicit selection must not use automatic discovery")
            })
            .unwrap();
            assert_eq!(result["source"], "selected");
            assert_eq!(result["file_version"], raw);
            let c = &result["compatibility"];
            assert_eq!(
                c["resource_directory"],
                nioh3_data::r4_resource_dir_for_file_version(version.tuple()).unwrap()
            );
            if raw != "2.0.2.0" {
                assert_eq!(c["data_version"], "PC v2.00.02");
                assert_eq!(c["features"]["character_read_edit"], "experimental");
                assert_eq!(c["features"]["native_equipment_add"], "unsupported");
            } else {
                assert_eq!(c["data_version"], "PC v2.02");
            }
        }
    }

    #[test]
    fn unreadable_selection_reports_exact_error_without_discovery_fallback() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Nioh3.exe");
        fixture.select(&path);
        let before = std::fs::read(fixture.0.join("game-install.json")).unwrap();
        let error = format!(
            "GAME_VERSION_UNREADABLE: no version resource on {}",
            path.display()
        );
        let result = inspect_with_reader(&fixture.0, &Reader(Err(error.clone())), || {
            panic!("an unreadable selection must not switch installations")
        })
        .unwrap();
        assert_eq!(result["executable"], path.to_string_lossy().as_ref());
        assert!(result["file_version"].is_null());
        assert_eq!(result["identity_error"]["code"], "GAME_VERSION_UNREADABLE");
        assert_eq!(result["identity_error"]["message"], error);
        assert_eq!(result["compatibility"]["status"], "unavailable");
        assert_eq!(
            std::fs::read(fixture.0.join("game-install.json")).unwrap(),
            before
        );
    }

    #[test]
    fn ambiguous_discovery_is_an_inspection_result_without_guessed_identity() {
        let fixture = Fixture::new();
        let result = inspect_with_reader(
            &fixture.0,
            &Reader(Ok(GameFileVersion::parse("2.0.2.0").unwrap())),
            || Err("GAME_EXECUTABLE_AMBIGUOUS: two candidate images".into()),
        )
        .unwrap();
        assert!(result["executable"].is_null());
        assert!(result["file_version"].is_null());
        assert_eq!(
            result["identity_error"]["code"],
            "GAME_EXECUTABLE_AMBIGUOUS"
        );
        assert_eq!(result["compatibility"]["status"], "unavailable");
    }

    #[test]
    fn invalid_selection_record_is_not_replaced_by_automatic_discovery() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("game-install.json"), b"{}").unwrap();
        let result = inspect_with_reader(
            &fixture.0,
            &Reader(Ok(GameFileVersion::parse("2.0.2.0").unwrap())),
            || panic!("invalid explicit configuration must remain visible"),
        );
        assert!(result
            .unwrap_err()
            .starts_with("GAME_INSTALL_CONFIG_INVALID:"));
    }
}
