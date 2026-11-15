//! Synthetic encrypted saves for packaged acceptance; never a playable save.
//!
//! `synthetic_save save <new directory>` writes one account with a Main save,
//! the game's per-slot backup and a System save, then prints `{"path": ...}`.
//!
//! `synthetic_save restore <existing directory>` writes that save under
//! `local/KoeiTecmo/NIOH3/Savedata`, backs it up into `profile/backups` the way
//! a save edit does, then changes the save so a no-op restore cannot pass. It
//! prints the save, the backup and both hashes.
//!
//! `synthetic_save decrypt <container> <output>` writes the clear bytes.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nioh3_save::codec::patch_user_checksum;
use nioh3_save::crypto::{decrypt_container, encrypt_container};
use nioh3_save::{
    backups_root, capture_related_fingerprints, role_backup_file, sha256_hex,
    write_backup_manifest, BackupFileEntry, BackupManifest, BACKUP_MANIFEST_SCHEMA,
    SAVE_SCHEMA_PROFILE, SCROLL_GROUP_OFFSET, SCROLL_RECORD_BYTES, USER_SAVE_BYTES,
};
use serde_json::json;

const ACCOUNT: u64 = 0x1111_2222_3333_4444;

type Result<T> = std::result::Result<T, String>;

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn scroll_record() -> Vec<u8> {
    let mut record = vec![0u8; SCROLL_RECORD_BYTES];
    put_u16(&mut record, 0, 0xE604);
    for (offset, value) in [(6, 180), (8, 180), (0x10, 183), (0x12, 183)] {
        put_u16(&mut record, offset, value);
    }
    put_u32(&mut record, 0x20, 36_526_331);
    record[0x30] = 4;
    record[0x31] = 4;
    record[0x33] = 3;
    put_u16(&mut record, 0x02, (ACCOUNT >> 48) as u16);
    put_u16(&mut record, 0x04, (ACCOUNT >> 32) as u16);
    put_u32(&mut record, 0x14, ACCOUNT as u32);
    for slot in 0..7 {
        put_u32(&mut record, 0x34 + slot * 0x18 + 4, u32::MAX);
    }
    record
}

fn encrypted(mut clear: Vec<u8>) -> Result<Vec<u8>> {
    patch_user_checksum(&mut clear).map_err(|error| error.to_string())?;
    encrypt_container(&clear).map_err(|error| error.to_string())
}

fn clear_save() -> Vec<u8> {
    let mut data = vec![0u8; USER_SAVE_BYTES];
    data[..6].copy_from_slice(b"RNNUSR");
    let record = scroll_record();
    data[SCROLL_GROUP_OFFSET..SCROLL_GROUP_OFFSET + record.len()].copy_from_slice(&record);
    data
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn create_save(target: &Path) -> Result<PathBuf> {
    if target.exists() {
        return Err("the fixture output must be new".into());
    }
    let folder = target.join(ACCOUNT.to_string()).join("SAVEDATA00");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let save = folder.join("SAVEDATA.BIN");
    let bytes = encrypted(clear_save())?;
    write(&save, &bytes)?;
    // Main, the game's per-slot backup and the System save form one generation.
    write(&folder.join("BACKUP.BIN"), &bytes)?;
    let system = target.join(ACCOUNT.to_string()).join("SYSTEMSAVEDATA00");
    std::fs::create_dir_all(&system).map_err(|error| error.to_string())?;
    write(&system.join("SAVEDATA.BIN"), b"RNN-SYNTHETIC-SYSTEM-SAVE")?;
    Ok(save)
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn back_up(state: &Path, save: &Path) -> Result<(String, PathBuf)> {
    let nanos = now_nanos();
    let operation_id =
        sha256_hex(format!("{nanos}-{}", std::process::id()).as_bytes())[..32].to_ascii_lowercase();
    let backup_id = format!("{nanos:020}-{operation_id}");
    let directory = backups_root(state).join(&backup_id);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for entry in capture_related_fingerprints(save).map_err(|error| error.to_string())? {
        if !entry.exists {
            continue;
        }
        let name = role_backup_file(entry.role);
        let bytes = std::fs::read(&entry.path).map_err(|error| error.to_string())?;
        write(&directory.join(name), &bytes)?;
        files.push(BackupFileEntry {
            source_role: entry.role.label().to_string(),
            source_path: entry.path.display().to_string(),
            backup_file: name.to_string(),
            size: entry.length,
            sha256: sha256_hex(&bytes).to_ascii_uppercase(),
        });
    }
    let manifest = BackupManifest {
        backup_manifest_schema: BACKUP_MANIFEST_SCHEMA.to_string(),
        save_schema_profile: SAVE_SCHEMA_PROFILE.to_string(),
        operation_id,
        created_at_utc: format!("{nanos:020}"),
        action: "v2-local-edit".to_string(),
        steam_account_id: ACCOUNT,
        save_slot_index: 0,
        backup_files: files,
    };
    write_backup_manifest(&directory, &manifest).map_err(|error| error.to_string())?;
    Ok((backup_id, directory.join("SAVEDATA.BIN")))
}

fn create_restore(root: &Path) -> Result<serde_json::Value> {
    let root = std::path::absolute(root).map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err(format!("{}: not a directory", root.display()));
    }
    let target = root.join("local/KoeiTecmo/NIOH3/Savedata");
    let state = root.join("profile");
    if target.exists() || state.exists() {
        return Err("the restore fixture needs new save and profile directories".into());
    }
    let save = create_save(&target)?;
    let (backup_id, backup) = back_up(&state, &save)?;
    let mut changed = clear_save();
    for offset in [0x10, 0x12] {
        put_u16(&mut changed, SCROLL_GROUP_OFFSET + offset, 585);
    }
    write(&save, &encrypted(changed)?)?;
    let hash = |path: &Path| -> Result<String> {
        Ok(sha256_hex(
            &std::fs::read(path).map_err(|error| error.to_string())?,
        ))
    };
    let (before, backed_up) = (hash(&save)?, hash(&backup)?);
    if before == backed_up {
        return Err("the synthetic generations must differ".into());
    }
    Ok(json!({
        "path": save, "backup_id": backup_id, "backup_path": backup,
        "before_sha256": before, "backup_sha256": backed_up,
    }))
}

fn decrypt(container: &str, output: &str) -> Result<serde_json::Value> {
    let bytes = std::fs::read(container).map_err(|error| format!("{container}: {error}"))?;
    let clear = decrypt_container(&bytes).map_err(|error| error.to_string())?;
    write(Path::new(output), &clear)?;
    Ok(json!({ "path": output, "sha256": sha256_hex(&clear) }))
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [mode, target] if mode == "save" => {
            create_save(Path::new(target)).map(|path| json!({ "path": path }))
        }
        [mode, root] if mode == "restore" => create_restore(Path::new(root)),
        [mode, container, output] if mode == "decrypt" => decrypt(container, output),
        _ => Err(
            "usage: synthetic_save (save <new directory> | restore <directory> | decrypt <container> <output>)"
                .into(),
        ),
    };
    match result {
        Ok(value) => println!("{value}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}
