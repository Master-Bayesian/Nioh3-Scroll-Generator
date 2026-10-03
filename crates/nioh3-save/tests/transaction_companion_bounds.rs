#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Transaction boundary regressions over task-owned opaque save generations.
//! The transaction accepts short fixtures, but never an oversized role.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nioh3_save::crypto::{SYSTEM_CONTAINER_BYTES, USER_CONTAINER_BYTES};
use nioh3_save::transaction::{capture_related_fingerprints, related_save_paths, SaveRole};
use nioh3_save::SaveTransactionHost;
use sha2::{Digest, Sha256};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "nioh3-companion-{label}-{}-{}",
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

fn generation(root: &Path) -> PathBuf {
    let main = root.join("76561198000000000/SAVEDATA00/SAVEDATA.BIN");
    fs::create_dir_all(main.parent().unwrap()).unwrap();
    fs::create_dir_all(
        main.parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("SYSTEMSAVEDATA00"),
    )
    .unwrap();
    fs::write(&main, b"owned opaque main").unwrap();
    main
}

fn extended_fixture(path: &Path, length: usize) {
    let mut file = File::create(path).unwrap();
    file.set_len(length as u64).unwrap();
    file.write_all(b"owned-beginning").unwrap();
    file.seek(SeekFrom::End(-17)).unwrap();
    file.write_all(b"distinct-end-tail").unwrap();
}

fn streamed_digest(path: &Path) -> String {
    let mut file = File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut chunk = [0u8; 8192];
    loop {
        let count = file.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&chunk[..count]);
    }
    format!("{:x}", digest.finalize())
}

fn refuse_oversized_companion(role: SaveRole) {
    let root = TempRoot::new(role.label());
    let main = generation(&root.0);
    let paths = related_save_paths(&main);
    for (current, path) in &paths {
        if *current != SaveRole::Main {
            fs::write(path, b"owned opaque companion").unwrap();
        }
    }
    let target = &paths
        .iter()
        .find(|(current, _)| *current == role)
        .unwrap()
        .1;
    extended_fixture(target, 16 * 1024 * 1024);
    let before: Vec<_> = paths
        .iter()
        .map(|(_, path)| {
            let metadata = fs::metadata(path).unwrap();
            (
                metadata.len(),
                metadata.modified().unwrap(),
                streamed_digest(path),
            )
        })
        .collect();
    let state = root.0.join("state");
    let host = SaveTransactionHost::new(&state);
    let result = host.plan_delete(&main, &before[0].2, vec![1]);
    let error = result.expect_err("oversized companion must fail before a plan is prepared");
    let message = error.to_string();
    assert!(message.contains(role.label()), "{message}");
    assert!(message.contains("maximum"), "{message}");
    assert!(message.contains("16777216"), "{message}");
    for ((_, path), expected) in paths.iter().zip(&before) {
        let metadata = fs::metadata(path).unwrap();
        assert_eq!(
            (
                metadata.len(),
                metadata.modified().unwrap(),
                streamed_digest(path)
            ),
            *expected,
            "refusal must preserve every source file",
        );
    }
    assert!(
        !state.exists(),
        "refusal must leave no plan, receipt or backup"
    );
}

#[test]
fn sixteen_mib_game_backup_is_refused_without_writes() {
    refuse_oversized_companion(SaveRole::GameBackup);
}

#[test]
fn sixteen_mib_system_is_refused_without_writes() {
    refuse_oversized_companion(SaveRole::System);
}

#[test]
fn fingerprints_preserve_missing_empty_and_short_opaque_roles() {
    let root = TempRoot::new("opaque");
    let main = generation(&root.0);
    let paths = related_save_paths(&main);
    let missing = capture_related_fingerprints(&main).unwrap();
    for entry in missing.iter().skip(1) {
        assert!(!entry.exists);
        assert_eq!(entry.length, 0);
        assert_eq!(entry.modified_nanos, 0);
        assert_eq!(entry.sha256, "");
    }
    fs::write(&paths[1].1, b"").unwrap();
    fs::write(&paths[2].1, b"opaque-system-tail").unwrap();
    let observed = capture_related_fingerprints(&main).unwrap();
    assert!(observed.iter().all(|entry| entry.exists));
    for entry in &observed {
        let metadata = fs::metadata(&entry.path).unwrap();
        assert_eq!(entry.length, metadata.len());
        assert_eq!(
            entry.modified_nanos,
            metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        );
        assert_eq!(entry.sha256, streamed_digest(&entry.path));
        assert_eq!(entry.sha256, entry.sha256.to_ascii_lowercase());
    }
    assert_eq!(observed, capture_related_fingerprints(&main).unwrap());
    let host = SaveTransactionHost::new(&root.0.join("state"));
    let plan = host
        .plan_delete(&main, &observed[0].sha256, vec![1])
        .unwrap();
    assert_eq!(plan.baseline, observed);
}

#[test]
fn fingerprints_cover_full_legal_lengths_and_their_final_bytes() {
    let root = TempRoot::new("full-tail");
    let main = generation(&root.0);
    for (role, path) in related_save_paths(&main) {
        let length = match role {
            SaveRole::Main | SaveRole::GameBackup => USER_CONTAINER_BYTES,
            SaveRole::System => SYSTEM_CONTAINER_BYTES,
        };
        extended_fixture(&path, length);
    }
    for entry in capture_related_fingerprints(&main).unwrap() {
        assert!(entry.exists);
        assert_eq!(entry.sha256, streamed_digest(&entry.path));
        let mut file = File::open(&entry.path).unwrap();
        file.seek(SeekFrom::End(-17)).unwrap();
        let mut tail = Vec::new();
        file.read_to_end(&mut tail).unwrap();
        assert_eq!(tail, b"distinct-end-tail");
        assert_eq!(entry.length, fs::metadata(&entry.path).unwrap().len());
    }
}

#[test]
fn restore_authentication_bounds_declared_and_actual_companion_sizes() {
    use nioh3_save::backup::authenticate_restore_source;
    use nioh3_save::{
        role_backup_file, sha256_hex, write_backup_manifest, BackupFileEntry, BackupManifest,
        BACKUP_MANIFEST_SCHEMA, SAVE_SCHEMA_PROFILE, USER_SAVE_BYTES, USER_SAVE_MAGIC,
    };

    let root = TempRoot::new("restore-source");
    let main = generation(&root.0);
    let backup_root = root.0.join("state/backups");
    let bundle = backup_root.join("owned-source");
    fs::create_dir_all(&bundle).unwrap();
    let mut plaintext = vec![0u8; USER_SAVE_BYTES];
    plaintext[..USER_SAVE_MAGIC.len()].copy_from_slice(USER_SAVE_MAGIC);
    let container = nioh3_save::encrypt_container(&plaintext).unwrap();
    let mut manifest = BackupManifest {
        backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
        save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
        operation_id: "a".repeat(32),
        created_at_utc: "2026-10-03T00:00:00Z".to_string(),
        action: "owned-regression".to_string(),
        steam_account_id: 76561198000000000,
        save_slot_index: 0,
        backup_files: Vec::new(),
    };
    for role in [SaveRole::Main, SaveRole::GameBackup, SaveRole::System] {
        let bytes: &[u8] = if role == SaveRole::Main {
            &container
        } else {
            b"short opaque companion"
        };
        let name = role_backup_file(role);
        fs::write(bundle.join(name), bytes).unwrap();
        manifest.backup_files.push(BackupFileEntry {
            source_role: role.label().to_string(),
            source_path: "owned-fixture".to_string(),
            backup_file: name.to_string(),
            size: bytes.len() as u64,
            sha256: sha256_hex(bytes).to_ascii_uppercase(),
        });
    }
    write_backup_manifest(&bundle, &manifest).unwrap();
    let authenticated = authenticate_restore_source(&backup_root, "owned-source", &main).unwrap();
    assert_eq!(authenticated.files.len(), 3);
    for file in &authenticated.files {
        assert_eq!(file.bytes.len() as u64, file.identity.size);
        assert_eq!(sha256_hex(&file.bytes), file.identity.sha256);
    }

    for role in [SaveRole::GameBackup, SaveRole::System] {
        let index = manifest
            .backup_files
            .iter()
            .position(|entry| entry.source_role == role.label())
            .unwrap();
        let path = bundle.join(role_backup_file(role));
        let original_entry = manifest.backup_files[index].clone();
        let small_size = original_entry.size;
        // A dishonest recorded size is rejected before any role's content is read.
        manifest.backup_files[index].size = 16 * 1024 * 1024;
        write_backup_manifest(&bundle, &manifest).unwrap();
        let error = authenticate_restore_source(&backup_root, "owned-source", &main).unwrap_err();
        assert!(error.to_string().contains(role.label()), "{error}");
        assert!(error.to_string().contains("maximum"), "{error}");
        assert_eq!(fs::metadata(&path).unwrap().len(), small_size);

        // A dishonest small size cannot hide an oversized opened file.
        manifest.backup_files[index] = original_entry.clone();
        write_backup_manifest(&bundle, &manifest).unwrap();
        extended_fixture(&path, 16 * 1024 * 1024);
        let before = (
            fs::metadata(&path).unwrap().len(),
            fs::metadata(&path).unwrap().modified().unwrap(),
            streamed_digest(&path),
        );
        let error = authenticate_restore_source(&backup_root, "owned-source", &main).unwrap_err();
        assert!(error.to_string().contains(role.label()), "{error}");
        assert!(error.to_string().contains("maximum"), "{error}");
        assert_eq!(
            (
                fs::metadata(&path).unwrap().len(),
                fs::metadata(&path).unwrap().modified().unwrap(),
                streamed_digest(&path),
            ),
            before,
        );

        fs::write(&path, b"short opaque companion").unwrap();
        manifest.backup_files[index].size = small_size + 1;
        write_backup_manifest(&bundle, &manifest).unwrap();
        let error = authenticate_restore_source(&backup_root, "owned-source", &main).unwrap_err();
        assert!(matches!(
            error,
            nioh3_save::SaveReadError::IntegrityMismatch { .. }
        ));
        manifest.backup_files[index] = original_entry;
    }
    write_backup_manifest(&bundle, &manifest).unwrap();
    assert_eq!(fs::read(&main).unwrap(), b"owned opaque main");
    assert_eq!(
        streamed_digest(&bundle.join("SAVEDATA.BIN")),
        sha256_hex(&container)
    );
}

#[test]
fn transform_registration_refuses_oversized_main_before_codec_read() {
    let root = TempRoot::new("register");
    let main = generation(&root.0);
    extended_fixture(&main, 16 * 1024 * 1024);
    let before = (
        fs::metadata(&main).unwrap().len(),
        fs::metadata(&main).unwrap().modified().unwrap(),
        streamed_digest(&main),
    );
    let error = match nioh3_save::SaveTransformHost::register(&main) {
        Ok(_) => panic!("oversized registration must be refused"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("main_save"), "{error}");
    assert!(error.to_string().contains("maximum"), "{error}");
    assert_eq!(
        (
            fs::metadata(&main).unwrap().len(),
            fs::metadata(&main).unwrap().modified().unwrap(),
            streamed_digest(&main),
        ),
        before,
    );
}
