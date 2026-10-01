//! A native-picked game path. Only its path is persisted; VERSIONINFO is always
//! read from the actual file. Selection takes effect on the next Studio launch.
use crate::game_version::{FileVersionReader, WindowsFileVersionReader};
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
    let path = selected(root)?;
    let version = path
        .as_ref()
        .and_then(|p| WindowsFileVersionReader.read(p).ok())
        .map(|v| v.dotted());
    Ok(
        json!({"executable":path.map(|p|p.to_string_lossy().into_owned()),"file_version":version,"restart_required":false}),
    )
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
    let version = WindowsFileVersionReader.read(&path)?;
    crate::storage::write_json(
        &root.join("game-install.json"),
        &json!({"schema":"nioh3-game-install/v1","executable":path.to_string_lossy()}),
    )?;
    let mut result = inspect(root)?;
    result["file_version"] = json!(version.dotted());
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
