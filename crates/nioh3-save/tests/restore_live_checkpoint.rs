#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Restoring a live (in-game) checkpoint replaces the files it holds.
//!
//! Live checkpoints (`nioh3-protected` `runtime_backup.rs`) hold only the main
//! save and record the live operation's id (`scroll-<millis>`, `count-<millis>`
//! or a UUID). Restore once refused them as a malformed manifest; these
//! regressions pin that they now restore the main save, leave the companions
//! alone, keep a pre-restore checkpoint, and still refuse what is not
//! authentic.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nioh3_save::transaction::{related_save_paths, PlanCommand, PlanKind, SaveRole};
use nioh3_save::{
    role_backup_file, sha256_hex, write_backup_manifest, BackupFileEntry, BackupManifest,
    SaveTransactionHost, BACKUP_MANIFEST_SCHEMA, SAVE_SCHEMA_PROFILE, USER_SAVE_BYTES,
    USER_SAVE_MAGIC,
};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "nioh3-live-restore-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const ACCOUNT: u64 = 76561198000000000;

/// A save generation whose three roles hold distinct current bytes.
fn generation(root: &Path) -> PathBuf {
    let main = root.join("76561198000000000/SAVEDATA00/SAVEDATA.BIN");
    fs::create_dir_all(main.parent().unwrap()).unwrap();
    for (role, path) in related_save_paths(&main) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes: &[u8] = match role {
            SaveRole::Main => b"current main after a live write",
            SaveRole::GameBackup => b"current game backup",
            SaveRole::System => b"current system save",
        };
        fs::write(&path, bytes).unwrap();
    }
    main
}

fn container(fill: u8) -> Vec<u8> {
    let mut plaintext = vec![fill; USER_SAVE_BYTES];
    plaintext[..USER_SAVE_MAGIC.len()].copy_from_slice(USER_SAVE_MAGIC);
    nioh3_save::encrypt_container(&plaintext).unwrap()
}

/// Write a bundle exactly as the live checkpoint writer does: the main save,
/// its decrypted copy, and a manifest declaring only the main role with an
/// upper-case digest.
fn live_checkpoint(backups: &Path, name: &str, operation_id: &str, main: &Path, raw: &[u8]) {
    let directory = backups.join(name);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("SAVEDATA.BIN"), raw).unwrap();
    fs::write(directory.join("decrypted.bin"), b"plaintext copy").unwrap();
    let manifest = BackupManifest {
        backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
        save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
        operation_id: operation_id.to_string(),
        created_at_utc: "2026-10-11T00:00:00.000000+00:00".to_string(),
        action: "v2-scroll-edit".to_string(),
        steam_account_id: ACCOUNT,
        save_slot_index: 0,
        backup_files: vec![BackupFileEntry {
            source_role: "main_save".to_string(),
            source_path: main.display().to_string(),
            backup_file: "SAVEDATA.BIN".to_string(),
            size: raw.len() as u64,
            sha256: sha256_hex(raw).to_uppercase(),
        }],
    };
    write_backup_manifest(&directory, &manifest).unwrap();
}

fn host(root: &Path) -> SaveTransactionHost {
    SaveTransactionHost::new(&root.join("state")).with_quiescence_interval(Duration::ZERO)
}

fn restore(host: &SaveTransactionHost, main: &Path, backup_id: &str) -> Result<String, String> {
    let current = sha256_hex(&fs::read(main).unwrap());
    let plan = host
        .plan(
            PlanKind::Restore,
            main,
            &current,
            PlanCommand::RestoreFromBackup {
                backup_id: backup_id.to_string(),
            },
        )
        .map_err(|error| error.to_string())?;
    host.commit(&plan)
        .map(|receipt| receipt.outcome)
        .map_err(|error| error.to_string())
}

#[test]
fn every_recorded_live_checkpoint_shape_restores_the_main_save_only() {
    for (index, operation_id) in [
        "scroll-1791154905906",
        "count-1791153339997",
        "6f1c2a4e-8b3d-4c5e-9f60-1a2b3c4d5e6f",
    ]
    .into_iter()
    .enumerate()
    {
        let root = TempRoot::new(&format!("shape-{index}"));
        let main = generation(&root.0);
        let companions: Vec<(PathBuf, Vec<u8>)> = related_save_paths(&main)
            .into_iter()
            .filter(|(role, _)| *role != SaveRole::Main)
            .map(|(_, path)| {
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        let raw = container(index as u8 + 1);
        let backups = root.0.join("state/backups");
        let name = format!("{operation_id}-001");
        live_checkpoint(&backups, &name, operation_id, &main, &raw);

        let host = host(&root.0);
        let outcome = restore(&host, &main, &name).unwrap();
        assert_eq!(outcome, "committed", "{operation_id}");
        assert_eq!(fs::read(&main).unwrap(), raw, "{operation_id}");
        for (path, bytes) in &companions {
            assert_eq!(&fs::read(path).unwrap(), bytes, "companions stay untouched");
        }

        // The replaced bytes are kept as an undoable pre-restore checkpoint.
        let checkpoint = fs::read_dir(&backups)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.join("restore-journal.json").is_file())
            .expect("a pre-restore checkpoint");
        assert_eq!(
            fs::read(checkpoint.join("SAVEDATA.BIN")).unwrap(),
            b"current main after a live write"
        );
        let journal: serde_json::Value =
            serde_json::from_slice(&fs::read(checkpoint.join("restore-journal.json")).unwrap())
                .unwrap();
        assert_eq!(journal["state"], "committed");
        let roles = journal["role_results"].as_array().unwrap();
        assert_eq!(roles.len(), 1, "only the restored role is a target");
        assert_eq!(roles[0]["role"], "main_save");
        assert_eq!(roles[0]["final_state"], "source_installed");
    }
}

#[test]
fn a_live_checkpoint_restore_rolls_back_on_failure() {
    let root = TempRoot::new("rollback");
    let main = generation(&root.0);
    let before = fs::read(&main).unwrap();
    let raw = container(7);
    let backups = root.0.join("state/backups");
    live_checkpoint(&backups, "scroll-1-001", "scroll-1", &main, &raw);
    let host =
        SaveTransactionHost::new(&root.0.join("state")).with_quiescence_interval(Duration::ZERO);
    host.faults()
        .arm(nioh3_save::transaction::FaultPoint::AfterReplace);
    let error = restore(&host, &main, "scroll-1-001").unwrap_err();
    assert!(error.contains("after-replace"), "{error}");
    assert_eq!(
        fs::read(&main).unwrap(),
        before,
        "the checkpoint is restored"
    );
}

#[test]
fn a_restore_still_refuses_what_is_not_authentic() {
    let root = TempRoot::new("refusals");
    let main = generation(&root.0);
    let raw = container(3);
    let backups = root.0.join("state/backups");
    let host = host(&root.0);
    let before = fs::read(&main).unwrap();

    // An operation id this product never records.
    live_checkpoint(&backups, "foreign", "../escape", &main, &raw);
    let error = restore(&host, &main, "foreign").unwrap_err();
    assert!(error.contains("operation id"), "{error}");

    // Tampered bytes.
    live_checkpoint(&backups, "tampered", "scroll-2", &main, &raw);
    let mut changed = raw.clone();
    let last = changed.len() - 1;
    changed[last] ^= 0xFF;
    fs::write(backups.join("tampered/SAVEDATA.BIN"), &changed).unwrap();
    assert!(restore(&host, &main, "tampered").is_err());

    // A bundle without the main save.
    let directory = backups.join("no-main");
    fs::create_dir_all(&directory).unwrap();
    let companion = b"companion only";
    fs::write(
        directory.join(role_backup_file(SaveRole::GameBackup)),
        companion,
    )
    .unwrap();
    write_backup_manifest(
        &directory,
        &BackupManifest {
            backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
            save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
            operation_id: "a".repeat(32),
            created_at_utc: "2026-10-11T00:00:00Z".to_string(),
            action: "owned-regression".to_string(),
            steam_account_id: ACCOUNT,
            save_slot_index: 0,
            backup_files: vec![BackupFileEntry {
                source_role: "game_backup".to_string(),
                source_path: String::new(),
                backup_file: role_backup_file(SaveRole::GameBackup).to_string(),
                size: companion.len() as u64,
                sha256: sha256_hex(companion),
            }],
        },
    )
    .unwrap();
    let error = restore(&host, &main, "no-main").unwrap_err();
    assert!(error.contains("missing role main"), "{error}");

    // Another account's checkpoint.
    let other = root.0.join("76561198000000001/SAVEDATA00/SAVEDATA.BIN");
    live_checkpoint(&backups, "other-account", "scroll-3", &other, &raw);
    let manifest_path = backups.join("other-account/backup-manifest.json");
    let mut manifest: BackupManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest.steam_account_id = 76561198000000001;
    write_backup_manifest(&backups.join("other-account"), &manifest).unwrap();
    assert!(restore(&host, &main, "other-account").is_err());

    assert_eq!(
        fs::read(&main).unwrap(),
        before,
        "no refusal writes the save"
    );
}
