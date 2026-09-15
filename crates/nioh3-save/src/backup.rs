//! Application-owned backup bundles: manifest, listing and recycle-bin move.
//!
//! Mirrors the shipped Python surfaces in `nioh3_scroll_editor/savegame.py`:
//! `write_backup_manifest`, `list_backup_entries` and
//! `move_backup_to_recycle_bin`, plus the `SaveApplication.backups` /
//! `recycle_backups` views that filter a bundle to one account and slot.
//!
//! A backup bundle is untrusted input. Its manifest is validated against the
//! same schema, profile, account and slot rules the restore path enforces, and a
//! directory that is a link, sits outside `backups/`, or declares a foreign
//! identity is refused rather than silently listed.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::SaveReadError;
use crate::save::sha256_hex;
use crate::transaction::is_safe_component;

/// Manifest schema every application backup carries.
pub const BACKUP_MANIFEST_SCHEMA: &str = "nioh3-scroll-backup/v2";
/// Save-layout profile the manifest must declare.
pub const SAVE_SCHEMA_PROFILE: &str = "nioh3-pc-v2.00.02-v2.01/save-layout-v1";

/// One backed-up file, mirroring the reference `backup_files` rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFileEntry {
    pub source_role: String,
    #[serde(default)]
    pub source_path: String,
    pub backup_file: String,
    pub size: u64,
    pub sha256: String,
}

/// The identity record written before any save write, mirroring
/// `write_backup_manifest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_manifest_schema: String,
    pub save_schema_profile: String,
    pub operation_id: String,
    pub created_at_utc: String,
    pub action: String,
    pub steam_account_id: u64,
    pub save_slot_index: u8,
    pub backup_files: Vec<BackupFileEntry>,
}

/// One listed backup bundle, mirroring the reference `BackupEntry` fields the
/// product surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub backup_id: String,
    pub action: String,
    pub account_id: Option<u64>,
    pub save_slot_index: Option<u8>,
    pub manifest_schema: Option<String>,
    pub file_count: usize,
    pub main_save_sha256: Option<String>,
}

/// Which relative name each role occupies inside a bundle.
pub const fn role_backup_file(role: crate::transaction::SaveRole) -> &'static str {
    match role {
        crate::transaction::SaveRole::Main => "SAVEDATA.BIN",
        crate::transaction::SaveRole::GameBackup => "BACKUP.BIN",
        crate::transaction::SaveRole::System => "SYSTEMSAVEDATA.BIN",
    }
}

/// The `backups` root under one state directory.
pub fn backups_root(state_root: &Path) -> PathBuf {
    state_root.join("backups")
}

/// Write one manifest, replacing the file only after its bytes reach disk.
pub fn write_backup_manifest(
    backup_directory: &Path,
    manifest: &BackupManifest,
) -> Result<PathBuf, SaveReadError> {
    fs::create_dir_all(backup_directory).map_err(|error| SaveReadError::Io {
        path: backup_directory.display().to_string(),
        message: error.to_string(),
    })?;
    let path = backup_directory.join("backup-manifest.json");
    let text = serde_json::to_string_pretty(manifest).map_err(|error| SaveReadError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    crate::transaction::write_durable(&path, text.as_bytes())?;
    Ok(path)
}

/// Read and validate one bundle's manifest.
///
/// The bundle must be a direct child of `backups/`, must not be a link, and must
/// declare the v2 schema and the current save-layout profile.
pub fn read_backup_manifest(
    state_root: &Path,
    backup_id: &str,
) -> Result<BackupManifest, SaveReadError> {
    let directory = validated_backup_directory(state_root, backup_id)?;
    let path = directory.join("backup-manifest.json");
    let text = fs::read_to_string(&path).map_err(|_| SaveReadError::TamperedRecord {
        kind: "backup manifest",
        message: format!(
            "{} has no readable backup-manifest.json",
            directory.display()
        ),
    })?;
    let manifest: BackupManifest =
        serde_json::from_str(&text).map_err(|error| SaveReadError::TamperedRecord {
            kind: "backup manifest",
            message: error.to_string(),
        })?;
    if manifest.backup_manifest_schema != BACKUP_MANIFEST_SCHEMA {
        return Err(SaveReadError::TamperedRecord {
            kind: "backup manifest",
            message: "the manifest does not declare the v2 schema".to_string(),
        });
    }
    if manifest.save_schema_profile != SAVE_SCHEMA_PROFILE {
        return Err(SaveReadError::TamperedRecord {
            kind: "backup manifest",
            message: "the manifest save-layout profile differs from this build".to_string(),
        });
    }
    Ok(manifest)
}

/// Resolve one bundle directory, refusing links and escaped identifiers.
fn validated_backup_directory(
    state_root: &Path,
    backup_id: &str,
) -> Result<PathBuf, SaveReadError> {
    if !is_safe_component(backup_id) {
        return Err(SaveReadError::TamperedRecord {
            kind: "backup",
            message: format!("backup id {backup_id:?} escapes the managed backups root"),
        });
    }
    let root = backups_root(state_root);
    let candidate = root.join(backup_id);
    let metadata = fs::symlink_metadata(&candidate).map_err(|error| SaveReadError::Io {
        path: candidate.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SaveReadError::TamperedRecord {
            kind: "backup",
            message: format!("{backup_id} is a link to another location"),
        });
    }
    if !metadata.is_dir() {
        return Err(SaveReadError::TamperedRecord {
            kind: "backup",
            message: format!("{backup_id} is not a directory"),
        });
    }
    Ok(candidate)
}

/// List the direct child bundles of `backups/`, newest name first.
///
/// Mirrors `list_backup_entries`: links and non-directories are skipped, an
/// unreadable manifest still yields an entry (with no identity), and the main
/// digest comes from the manifest when present, otherwise from the file.
pub fn list_backup_entries(state_root: &Path) -> Result<Vec<BackupEntry>, SaveReadError> {
    let root = backups_root(state_root);
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(&root).map_err(|error| SaveReadError::Io {
        path: root.display().to_string(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| SaveReadError::Io {
            path: root.display().to_string(),
            message: error.to_string(),
        })?;
        let metadata = entry.metadata().map_err(|error| SaveReadError::Io {
            path: entry.path().display().to_string(),
            message: error.to_string(),
        })?;
        if metadata.is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    names.reverse();
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let directory = root.join(&name);
        let manifest = read_backup_manifest(state_root, &name).ok();
        let file_count = fs::read_dir(&directory)
            .map(|iterator| {
                iterator
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_file())
                    .count()
            })
            .unwrap_or(0);
        let main_save_sha256 = match manifest.as_ref() {
            Some(manifest) => manifest
                .backup_files
                .iter()
                .find(|item| item.backup_file == "SAVEDATA.BIN")
                .map(|item| item.sha256.to_ascii_uppercase()),
            None => fs::read(directory.join("SAVEDATA.BIN"))
                .ok()
                .map(|bytes| sha256_hex(&bytes).to_ascii_uppercase()),
        };
        entries.push(BackupEntry {
            backup_id: name,
            action: manifest
                .as_ref()
                .map(|manifest| manifest.action.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            account_id: manifest.as_ref().map(|manifest| manifest.steam_account_id),
            save_slot_index: manifest.as_ref().map(|manifest| manifest.save_slot_index),
            manifest_schema: manifest
                .as_ref()
                .map(|manifest| manifest.backup_manifest_schema.clone()),
            file_count,
            main_save_sha256,
        });
    }
    Ok(entries)
}

/// Backups belonging to one account and character slot.
pub fn list_backups_for(
    state_root: &Path,
    account_id: u64,
    save_slot: u8,
) -> Result<Vec<BackupEntry>, SaveReadError> {
    Ok(list_backup_entries(state_root)?
        .into_iter()
        .filter(|entry| {
            entry.account_id == Some(account_id) && entry.save_slot_index == Some(save_slot)
        })
        .take(256)
        .collect())
}

/// Move one application-owned bundle to the recycle bin.
///
/// Mirrors `move_backup_to_recycle_bin`: only a validated direct child of
/// `backups/` is accepted, and on Windows the move is a shell delete with undo
/// so the bundle stays recoverable. On any other platform the operation refuses
/// rather than deleting.
pub fn move_backup_to_recycle_bin(state_root: &Path, backup_id: &str) -> Result<(), SaveReadError> {
    let directory = validated_backup_directory(state_root, backup_id)?;
    if !cfg!(windows) {
        return Err(SaveReadError::InvalidTransform {
            message: "Windows recycle-bin deletion is only available on Windows".to_string(),
        });
    }
    #[cfg(windows)]
    {
        windows_recycle(&directory)
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(windows)]
fn windows_recycle(directory: &Path) -> Result<(), SaveReadError> {
    use std::os::windows::ffi::OsStrExt;

    const FO_DELETE: u32 = 0x0003;
    const FOF_ALLOWUNDO: u16 = 0x0040;
    const FOF_NOCONFIRMATION: u16 = 0x0010;
    const FOF_SILENT: u16 = 0x0004;

    #[repr(C)]
    struct ShFileOpStructW {
        hwnd: *mut core::ffi::c_void,
        w_func: u32,
        p_from: *const u16,
        p_to: *const u16,
        f_flags: u16,
        f_any_operations_aborted: i32,
        h_name_mappings: *mut core::ffi::c_void,
        lpsz_progress_title: *const u16,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHFileOperationW(operation: *mut ShFileOpStructW) -> i32;
    }

    let mut source: Vec<u16> = directory.as_os_str().encode_wide().collect();
    source.push(0);
    source.push(0);
    let mut operation = ShFileOpStructW {
        hwnd: std::ptr::null_mut(),
        w_func: FO_DELETE,
        p_from: source.as_ptr(),
        p_to: std::ptr::null(),
        f_flags: FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT,
        f_any_operations_aborted: 0,
        h_name_mappings: std::ptr::null_mut(),
        lpsz_progress_title: std::ptr::null(),
    };
    // SAFETY: `source` is a double-null-terminated wide buffer that outlives the
    // call, and `operation` is a correctly sized, initialized structure.
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result != 0 {
        return Err(SaveReadError::Io {
            path: directory.display().to_string(),
            message: format!("Windows refused the recycle-bin move with code {result}"),
        });
    }
    if operation.f_any_operations_aborted != 0 {
        return Err(SaveReadError::Io {
            path: directory.display().to_string(),
            message: "the recycle-bin move was cancelled".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::transaction::SaveRole;

    fn temp_root(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("nioh3-backup-{label}-{nanos}"));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn manifest(action: &str) -> BackupManifest {
        BackupManifest {
            backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
            save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
            operation_id: "a".repeat(32),
            created_at_utc: "2026-01-01T00:00:00Z".to_string(),
            action: action.to_string(),
            steam_account_id: 76561198000000000,
            save_slot_index: 3,
            backup_files: vec![BackupFileEntry {
                source_role: "main_save".to_string(),
                source_path: "virt".to_string(),
                backup_file: "SAVEDATA.BIN".to_string(),
                size: 4,
                sha256: sha256_hex(b"main").to_ascii_uppercase(),
            }],
        }
    }

    #[test]
    fn listing_reports_identity_and_filters_by_slot() {
        let root = temp_root("list");
        let bundle = backups_root(&root).join("20260101-000000");
        fs::create_dir_all(&bundle).expect("bundle");
        fs::write(bundle.join("SAVEDATA.BIN"), b"main").expect("main");
        write_backup_manifest(&bundle, &manifest("v2-local-edit")).expect("manifest");
        let entries = list_backup_entries(&root).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "v2-local-edit");
        assert_eq!(entries[0].account_id, Some(76561198000000000));
        assert_eq!(entries[0].save_slot_index, Some(3));
        assert_eq!(
            list_backups_for(&root, 76561198000000000, 3).unwrap().len(),
            1
        );
        assert_eq!(
            list_backups_for(&root, 76561198000000000, 4).unwrap().len(),
            0
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_foreign_manifest_schema_is_refused() {
        let root = temp_root("schema");
        let bundle = backups_root(&root).join("old");
        fs::create_dir_all(&bundle).expect("bundle");
        let mut value = manifest("v1");
        value.backup_manifest_schema = "nioh3-scroll-backup/v1".to_string();
        write_backup_manifest(&bundle, &value).expect("manifest");
        assert!(read_backup_manifest(&root, "old").is_err());
        // The listing still reports the bundle, with no identity.
        let entries = list_backup_entries(&root).expect("list");
        assert_eq!(entries[0].account_id, None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_escaping_backup_id_is_refused() {
        let root = temp_root("escape");
        assert!(read_backup_manifest(&root, "../elsewhere").is_err());
        assert!(read_backup_manifest(&root, "..\\elsewhere").is_err());
        assert!(move_backup_to_recycle_bin(&root, "../elsewhere").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn role_names_match_the_reference_bundle_layout() {
        assert_eq!(role_backup_file(SaveRole::Main), "SAVEDATA.BIN");
        assert_eq!(role_backup_file(SaveRole::GameBackup), "BACKUP.BIN");
        assert_eq!(role_backup_file(SaveRole::System), "SYSTEMSAVEDATA.BIN");
    }
}
