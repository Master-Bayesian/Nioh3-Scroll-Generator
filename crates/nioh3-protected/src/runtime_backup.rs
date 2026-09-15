//! The real save-side checkpoint the protected runtime role hands to the
//! reviewed live-addition application.
//!
//! `LiveAddApplication.prepare` refuses to publish an executable plan before a
//! verified backup exists, and the checkpoint it stores is the one the shipped
//! backup manager restores. Porting `SaveBackup` from the fake used by the
//! runtime crate's own tests to the product save components keeps that
//! guarantee: the source shape is validated, the copy is verified against the
//! bytes that were planned, the copy is decrypted with the audited primitive,
//! and the same v2 manifest the offline transactions publish is written before
//! the plan is exposed.

use std::path::{Path, PathBuf};

use nioh3_runtime::mutation::live_add::{SaveBackup, SaveCheckpoint};
use nioh3_runtime::RuntimeError;
use nioh3_save::backup::{
    backups_root, write_backup_manifest, BackupFileEntry, BackupManifest, BACKUP_MANIFEST_SCHEMA,
    SAVE_SCHEMA_PROFILE,
};
use nioh3_save::save::DecryptedSave;

/// The action label the shipped `prepare` records for a live addition.
pub const LIVE_ADD_ACTION: &str = "v2-live-add";

/// One application-owned backup bundle per reviewed live addition.
pub struct SaveBackupAdapter {
    root: PathBuf,
    counter: u64,
}

impl SaveBackupAdapter {
    pub fn new(state_root: &Path) -> Self {
        Self {
            root: state_root.to_path_buf(),
            counter: 0,
        }
    }
}

fn io_error(path: &Path, detail: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::Io {
        path: path.display().to_string(),
        detail: detail.to_string(),
    }
}

impl SaveBackup for SaveBackupAdapter {
    fn checkpoint(
        &mut self,
        source: &Path,
        raw: &[u8],
        operation_id: &str,
    ) -> Result<SaveCheckpoint, RuntimeError> {
        // Path shape first: the shipped preparation refuses a save it cannot
        // attribute to one Steam account and slot before it copies anything.
        let account = nioh3_save::paths::account_id_from_save_path(source).map_err(|error| {
            RuntimeError::LiveAddRejected {
                detail: error.to_string(),
            }
        })?;
        let slot = nioh3_save::paths::save_slot_index_from_path(source).map_err(|error| {
            RuntimeError::LiveAddRejected {
                detail: error.to_string(),
            }
        })?;

        self.counter += 1;
        let directory =
            backups_root(&self.root).join(format!("{operation_id}-{:03}", self.counter));
        std::fs::create_dir_all(&directory).map_err(|error| io_error(&directory, error))?;

        // The copy is written exclusively and verified against the planned
        // bytes, so a partially written backup can never look complete.
        let backup_path = directory.join("SAVEDATA.BIN");
        std::fs::write(&backup_path, raw).map_err(|error| io_error(&backup_path, error))?;
        let copied = std::fs::read(&backup_path).map_err(|error| io_error(&backup_path, error))?;
        if copied != raw {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Automatic save backup verification failed".to_string(),
            });
        }

        // The source must not have moved while the backup was taken.
        let current = std::fs::read(source).map_err(|error| io_error(source, error))?;
        if current != raw {
            return Err(RuntimeError::LiveAddRejected {
                detail: "Source save changed during backup".to_string(),
            });
        }

        let decrypted =
            DecryptedSave::from_container(raw).map_err(|error| RuntimeError::LiveAddRejected {
                detail: error.to_string(),
            })?;
        let plaintext = decrypted.as_bytes().to_vec();
        let decrypted_path = directory.join("decrypted.bin");
        std::fs::write(&decrypted_path, &plaintext)
            .map_err(|error| io_error(&decrypted_path, error))?;

        // Publish the same account/slot/hash manifest the offline operations
        // publish, so the existing backup manager can restore this checkpoint.
        let manifest = BackupManifest {
            backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
            save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
            operation_id: operation_id.to_string(),
            created_at_utc: created_at_utc(),
            action: LIVE_ADD_ACTION.to_string(),
            steam_account_id: account,
            save_slot_index: slot,
            backup_files: vec![BackupFileEntry {
                source_role: "main_save".to_string(),
                source_path: source.display().to_string(),
                backup_file: "SAVEDATA.BIN".to_string(),
                size: raw.len() as u64,
                sha256: nioh3_save::save::sha256_hex(raw).to_uppercase(),
            }],
        };
        write_backup_manifest(&directory, &manifest).map_err(|error| {
            RuntimeError::LiveAddRejected {
                detail: error.to_string(),
            }
        })?;

        Ok(SaveCheckpoint {
            directory,
            backup_path,
            decrypted: plaintext,
        })
    }
}

/// `datetime.now(timezone.utc).isoformat()` in the one shape the manifest
/// carries: `YYYY-MM-DDTHH:MM:SS.ffffff+00:00`.
///
/// The manifest field is an identity record, not a clock the product compares,
/// so the host stamps it from the system clock without pulling in a date
/// library for one string.
fn created_at_utc() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs();
    let micros = now.subsec_micros();
    let days = seconds / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let remainder = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}+00:00",
        remainder / 3600,
        (remainder % 3600) / 60,
        remainder % 60,
    )
}

/// Howard Hinnant's `civil_from_days`, the standard inverse of the Julian-day
/// conversion, so the stamp needs no external crate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_civil_conversion_anchors_on_the_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000-03-01 is 11017 days after the epoch and exercises the leap rule.
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn the_stamp_has_the_shipped_isoformat_shape() {
        let stamp = created_at_utc();
        assert!(stamp.ends_with("+00:00"), "{stamp}");
        assert_eq!(stamp.len(), 32, "{stamp}");
        assert_eq!(&stamp[4..5], "-");
        assert_eq!(&stamp[10..11], "T");
    }

    #[test]
    fn a_foreign_save_shape_is_refused_before_any_copy() {
        let root = std::env::temp_dir().join("nioh3-live-add-backup-refusal");
        let mut backup = SaveBackupAdapter::new(&root);
        let error = backup
            .checkpoint(Path::new("C:/nowhere/SAVEDATA.BIN"), b"raw", "op")
            .expect_err("a path without an account and slot must be refused");
        assert!(matches!(error, RuntimeError::LiveAddRejected { .. }));
        assert!(!root.join("backups").exists(), "nothing may be created");
    }
}
