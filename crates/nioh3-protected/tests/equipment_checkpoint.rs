//! Real checkpoint adapter over an owned, entirely synthetic encrypted save.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use nioh3_protected::runtime_backup::prepare_equipment_checkpoint;
use nioh3_save::layout::{USER_SAVE_BYTES, USER_SAVE_MAGIC};
use std::path::PathBuf;

const ID: &str = "10000000-0000-4000-8000-000000000003";

struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nioh3-equipment-backup-{}-{stamp}",
            std::process::id()
        ));
        let directory = root.join("123456/SAVEDATA00");
        std::fs::create_dir_all(&directory).unwrap();
        Self {
            source: directory.join("SAVEDATA.BIN"),
            root,
        }
    }
    fn write_save(&self, marker: u8) -> Vec<u8> {
        let mut plaintext = vec![0; USER_SAVE_BYTES];
        plaintext[..USER_SAVE_MAGIC.len()].copy_from_slice(USER_SAVE_MAGIC);
        plaintext[0x200] = marker;
        let encrypted = nioh3_save::crypto::encrypt_container(&plaintext).unwrap();
        std::fs::write(&self.source, &encrypted).unwrap();
        encrypted
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn equipment_checkpoint_verifies_current_bytes_and_preserves_prior_retry_backup() {
    let fixture = Fixture::new();
    let first = fixture.write_save(3);
    let checkpoint = prepare_equipment_checkpoint(&fixture.root, &fixture.source, ID).unwrap();
    let backup = PathBuf::from(checkpoint["backup_path"].as_str().unwrap());
    assert_eq!(std::fs::read(&backup).unwrap(), first);
    assert_eq!(
        checkpoint["source_save_sha256"],
        nioh3_save::save::sha256_hex(&first)
    );
    assert_eq!(checkpoint["operation_id"], ID);
    assert!(backup
        .parent()
        .unwrap()
        .join("backup-manifest.json")
        .is_file());

    let second = fixture.write_save(7);
    let retry = prepare_equipment_checkpoint(&fixture.root, &fixture.source, ID).unwrap();
    let refreshed = PathBuf::from(retry["backup_path"].as_str().unwrap());
    assert_ne!(backup, refreshed);
    assert_eq!(std::fs::read(&backup).unwrap(), first);
    assert_eq!(std::fs::read(&refreshed).unwrap(), second);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), second);
}

#[test]
fn malformed_save_or_operation_never_returns_checkpoint_evidence() {
    let fixture = Fixture::new();
    std::fs::write(&fixture.source, b"not a save container").unwrap();
    assert!(prepare_equipment_checkpoint(&fixture.root, &fixture.source, ID).is_err());
    assert!(prepare_equipment_checkpoint(&fixture.root, &fixture.source, "../foreign").is_err());
    assert_eq!(
        std::fs::read(&fixture.source).unwrap(),
        b"not a save container"
    );
}
