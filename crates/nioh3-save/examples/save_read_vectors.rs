//! Emit the read model of one save as tab-separated rows.
//!
//! Modes (all consumed by `tests/migration/test_save_read_parity.py`):
//! - default: `--fixture <path>` (decrypted `RNNUSR` blob) and `--save-path
//!   <path>` (on-disk identity for the account/slot rules);
//! - `--decrypt <encrypted path> [--save-path <path>]`: decrypt a shipped
//!   container and emit its identity rows (encrypted and decrypted digests are
//!   both reported);
//! - `--encrypt <plain path> [--output-file <path>]`: encode that plaintext with
//!   the ported codec and report the container digest, so a gate can compare the
//!   raw container bytes against the shipped tool's own output;
//! - `--discover <root>`: list the saves below a supplied save root.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use nioh3_save::{
    account_id_from_save_path, discover_save_paths, save_slot_index_from_path, DecryptedSave,
    SaveInventory, SCROLL_SLOT_COUNT,
};

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

fn run() -> Result<(), String> {
    let mut fixture: Option<PathBuf> = None;
    let mut encrypted: Option<PathBuf> = None;
    let mut encrypt: Option<PathBuf> = None;
    let mut output_file: Option<PathBuf> = None;
    let mut discover: Option<PathBuf> = None;
    let mut save_path: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--fixture" => fixture = arguments.next().map(PathBuf::from),
            "--decrypt" => encrypted = arguments.next().map(PathBuf::from),
            "--encrypt" => encrypt = arguments.next().map(PathBuf::from),
            "--output-file" => output_file = arguments.next().map(PathBuf::from),
            "--discover" => discover = arguments.next().map(PathBuf::from),
            "--header-keys" => {
                let (key_1, iv_1, key_2, iv_2) = nioh3_save::crypto::header_session_keys();
                println!("k1\t{}", hex(&key_1));
                println!("iv1\t{}", hex(&iv_1));
                println!("k2\t{}", hex(&key_2));
                println!("iv2\t{}", hex(&iv_2));
                return Ok(());
            }
            "--save-path" => save_path = arguments.next().map(PathBuf::from),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if let Some(path) = encrypt {
        let bytes = fs::read(&path).map_err(|error| format!("{path:?}: {error}"))?;
        let container = nioh3_save::encrypt_container(&bytes).map_err(|error| error.to_string())?;
        if let Some(output) = output_file {
            fs::write(&output, &container).map_err(|error| format!("{output:?}: {error}"))?;
        }
        println!("container\t{}", nioh3_save::sha256_hex(&container));
        return Ok(());
    }
    if let Some(root) = discover {
        let paths = discover_save_paths(&root).map_err(|error| error.to_string())?;
        for path in paths {
            let account = account_id_from_save_path(&path).map_err(|error| error.to_string())?;
            let slot = save_slot_index_from_path(&path).map_err(|error| error.to_string())?;
            println!("discovered\t{account}\t{slot}\t{}", path.display());
        }
        return Ok(());
    }

    let save_path = match (save_path, &encrypted, &fixture) {
        (Some(path), _, _) => path,
        (None, Some(path), _) | (None, _, Some(path)) => path.clone(),
        (None, None, None) => {
            return Err("one of --fixture, --decrypt or --discover is required".to_string())
        }
    };
    let (save, encrypted_sha256) = match (&encrypted, &fixture) {
        (Some(path), _) => {
            let container = fs::read(path).map_err(|error| format!("{path:?}: {error}"))?;
            let digest = nioh3_save::sha256_hex(&container);
            let decrypted = DecryptedSave::from_container(&container).map_err(|error| {
                format!(
                    "{error} (container first 16: {})",
                    hex(&container[..16.min(container.len())])
                )
            })?;
            (decrypted, Some(digest))
        }
        (None, Some(path)) => {
            let bytes = fs::read(path).map_err(|error| format!("{path:?}: {error}"))?;
            (
                DecryptedSave::new(bytes).map_err(|error| error.to_string())?,
                None,
            )
        }
        (None, None) => {
            return Err("one of --fixture, --decrypt or --discover is required".to_string())
        }
    };
    let inventory =
        SaveInventory::load(&save_path, save, true).map_err(|error| error.to_string())?;

    let mut rows = Vec::new();
    if let Some(digest) = encrypted_sha256 {
        rows.push(format!("container\t{digest}"));
    }
    rows.push(format!(
        "save\t{}\t{}\t{}",
        save_path.display(),
        inventory.account_id,
        inventory.source_sha256()
    ));
    rows.push(format!(
        "path\t{}\t{}",
        account_id_from_save_path(&save_path).map_err(|error| error.to_string())?,
        save_slot_index_from_path(&save_path).map_err(|error| error.to_string())?
    ));

    match inventory.template_record.as_ref() {
        Some(record) => rows.push(format!(
            "template\t{:04x}\t{}",
            record.record_type(),
            hex(record.as_bytes())
        )),
        None => rows.push("template\tnone\t-".to_string()),
    }

    for playthrough in [3u8, 4, 5] {
        match inventory.template_record_for_playthrough(playthrough) {
            Ok(record) => rows.push(format!(
                "playthrough\t{playthrough}\t{:04x}\t{}",
                record.record_type(),
                hex(record.as_bytes())
            )),
            Err(error) => rows.push(format!("playthrough\t{playthrough}\tnone\t{error}")),
        }
    }

    for slot_index in 0..SCROLL_SLOT_COUNT {
        if inventory.empty_slots.contains(&slot_index) {
            rows.push(format!("empty\t{slot_index}"));
        }
    }
    if let Some(next) = inventory.next_slot_index {
        rows.push(format!("next-slot\t{next}"));
    } else {
        rows.push("next-slot\tnone".to_string());
    }
    let serials = inventory.occupied_generation_serials();
    rows.push(format!(
        "serials\t{}\t{}",
        serials.len(),
        serials
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
    rows.push(format!(
        "next-serial\t{}",
        inventory.next_generation_serial()
    ));
    let keys = inventory.occupied_inventory_keys();
    rows.push(format!(
        "keys\t{}\t{}",
        keys.len(),
        keys.iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));

    for entry in inventory.scroll_entries(true) {
        let category = match entry.playthrough() {
            Some(playthrough) => playthrough.to_string(),
            None => "unmapped".to_string(),
        };
        rows.push(format!(
            "entry\t{}\t{}\t{:04x}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.slot_index,
            entry.record_offset,
            entry.record_type(),
            category,
            entry.seed(),
            entry.rarity(),
            entry.transfer_count(),
            entry.generation_serial(),
            entry.inventory_key()
        ));
    }
    for entry in inventory.scroll_entries(false) {
        if entry.playthrough().is_none() {
            return Err("a mapped-only read produced an unmapped entry".to_string());
        }
    }
    for row in rows {
        println!("{row}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
