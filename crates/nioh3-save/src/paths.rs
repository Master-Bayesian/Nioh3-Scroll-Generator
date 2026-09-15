//! Save-path identity rules shared with the shipped discovery code.

use std::path::Path;
use std::path::PathBuf;

use crate::error::SaveReadError;

/// Match `SAVEDATA??` directory names (`SAVE_SLOT_DIRECTORY_PATTERN`).
fn slot_directory_name(path: &Path) -> Option<&str> {
    let name = path.file_name()?.to_str()?;
    let suffix = name.strip_prefix("SAVEDATA")?;
    if suffix.len() == 2 && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(suffix)
    } else {
        None
    }
}

/// Zero-based in-game character-save slot for one `SAVEDATA??/SAVEDATA.BIN` path.
pub fn save_slot_index_from_path(path: &Path) -> Result<u8, SaveReadError> {
    let parent = path.parent().ok_or_else(|| SaveReadError::SlotDirectory {
        path: path.display().to_string(),
    })?;
    let suffix = slot_directory_name(parent).ok_or_else(|| SaveReadError::SlotDirectory {
        path: parent.display().to_string(),
    })?;
    suffix
        .parse::<u8>()
        .map_err(|_| SaveReadError::SlotDirectory {
            path: parent.display().to_string(),
        })
}

/// Steam account id encoded in the directory above `SAVEDATA??`.
pub fn account_id_from_save_path(path: &Path) -> Result<u64, SaveReadError> {
    let directory = || SaveReadError::AccountDirectory {
        path: path.display().to_string(),
    };
    let slot_directory = path.parent().ok_or_else(directory)?;
    let account_directory = slot_directory.parent().ok_or_else(directory)?;
    let name = account_directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(directory)?;
    name.parse::<u64>().map_err(|_| directory())
}

/// Discover character saves below a supplied save root.
///
/// The shipped product reads its root from `LOCALAPPDATA`, but the caller
/// supplies the root here so the product path and tests can share one
/// implementation and no user save is touched. Only
/// `<root>/<account>/SAVEDATA<nn>/SAVEDATA.BIN` entries whose account directory
/// is numeric are returned, sorted by `(account, slot)` exactly like the shipped
/// `discover_save_paths`.
pub fn discover_save_paths(root: &Path) -> Result<Vec<PathBuf>, SaveReadError> {
    let root_error = || SaveReadError::DiscoveryRoot {
        path: root.display().to_string(),
    };
    if !root.is_dir() {
        return Err(root_error());
    }
    let accounts = std::fs::read_dir(root).map_err(|_| root_error())?;
    let mut discovered: Vec<(u64, u8, PathBuf)> = Vec::new();
    for account_entry in accounts.flatten() {
        let account_path = account_entry.path();
        let account_name = account_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let Ok(account_id) = account_name.parse::<u64>() else {
            continue;
        };
        if !account_path.is_dir() {
            continue;
        }
        let Ok(slots) = std::fs::read_dir(&account_path) else {
            continue;
        };
        for slot_entry in slots.flatten() {
            let slot_path = slot_entry.path();
            let Some(suffix) = slot_path
                .file_name()
                .and_then(|value| value.to_str())
                .and_then(|name| name.strip_prefix("SAVEDATA"))
            else {
                continue;
            };
            if suffix.len() != 2 || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            let Ok(slot_index) = suffix.parse::<u8>() else {
                continue;
            };
            let save = slot_path.join("SAVEDATA.BIN");
            if !save.is_file() {
                continue;
            }
            discovered.push((account_id, slot_index, save));
        }
    }
    discovered.sort_by_key(|(account_id, slot_index, _)| (*account_id, *slot_index));
    Ok(discovered.into_iter().map(|(_, _, path)| path).collect())
}
