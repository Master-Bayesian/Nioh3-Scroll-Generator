//! Read-only PC v2.02 equipment-record snapshot for bounded armor-remodel research.
//!
//! The example binds one running `Nioh3.exe` instance to the exact fixed file
//! version and executable digest already used by the read-only inventory
//! surface. It verifies the retained inventory signatures, then uses the
//! existing player-layout cross-check through [`read_character`]. It never
//! requests a write handle, calls game code, allocates in the target, or
//! emits a complete record.
//!
//! Usage:
//!
//! ```text
//! armor_remodel_snapshot [--item-id N] [--slot N] [--max-records N] [--out PATH]
//! ```
//!
//! `N` accepts decimal or `0x`-prefixed hexadecimal. The default output bound
//! is 64 occupied records; the hard bound is the 2,500-slot equipment array.
//! Use `--slot` for one record or `--item-id` to narrow a sample set.

use nioh3_runtime::character::{read_character, EQUIPMENT_RECORD_BYTES, EQUIPMENT_SLOTS};
use nioh3_runtime::inventory::{
    inventory_sites, ProcessInventoryMemory, INVENTORY_EXECUTABLE_SHA256, INVENTORY_GAME_VERSION,
};
use nioh3_runtime::platform::{
    file_sha256, process_creation_filetime, single_process_id, verify_game_executable, FileVersion,
    GameCompatibility, ReadOnlyProcess, GAME_IMAGE_NAME, GAME_MODULE_NAME,
};
use nioh3_runtime::RuntimeError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const EXPECTED_FILE_VERSION: FileVersion = FileVersion::new(2, 0, 2, 0);
const DEFAULT_MAX_RECORDS: usize = 64;
const PUBLIC_REGION_START: usize = 0x30;
const PUBLIC_REGION_END: usize = 0xDC;
const EFFECT_COUNT: usize = 7;
const EFFECT_STRIDE: usize = 0x18;
const EFFECT_ENTRY_START: usize = 0x34;

struct Options {
    item_id: Option<u16>,
    slot: Option<usize>,
    max_records: usize,
    output: Option<String>,
}

enum Failure {
    Refused(RuntimeError),
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        println!("{}", usage());
        return std::process::ExitCode::from(0);
    }

    let options = match parse_options(&args) {
        Ok(options) => options,
        Err(message) => {
            println!("{message}");
            return std::process::ExitCode::from(2);
        }
    };

    match capture(&options) {
        Ok(value) => match render_and_write(&value, options.output.as_deref()) {
            Ok(()) => std::process::ExitCode::from(0),
            Err(message) => {
                println!("{message}");
                std::process::ExitCode::from(2)
            }
        },
        Err(Failure::Refused(error)) => {
            println!("{}", refusal(&error));
            std::process::ExitCode::from(1)
        }
    }
}

fn usage() -> String {
    concat!(
        "usage: armor_remodel_snapshot [--item-id N] [--slot N] ",
        "[--max-records N] [--out PATH]\n",
        "  exact target: Nioh3.exe FILEVERSION 2.0.2.0 and the pinned inventory SHA-256\n",
        "  output: occupied 0xF0 records, capped at 64 by default; no full raw records\n",
    )
    .to_string()
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut item_id = None;
    let mut slot = None;
    let mut max_records = DEFAULT_MAX_RECORDS;
    let mut output = None;
    let mut index = 0usize;
    while index < args.len() {
        let option = args[index].as_str();
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{option} needs a value\n{}", usage()))?;
        match option {
            "--item-id" => {
                let parsed = parse_u64(value, option)?;
                item_id = Some(
                    u16::try_from(parsed)
                        .map_err(|_| format!("{option} must fit an unsigned 16-bit value"))?,
                );
            }
            "--slot" => {
                let parsed = parse_u64(value, option)?;
                let parsed = usize::try_from(parsed)
                    .map_err(|_| format!("{option} is too large for this platform"))?;
                if parsed >= EQUIPMENT_SLOTS {
                    return Err(format!(
                        "{option} must be from 0 to {}",
                        EQUIPMENT_SLOTS - 1
                    ));
                }
                slot = Some(parsed);
            }
            "--max-records" => {
                let parsed = parse_u64(value, option)?;
                let parsed = usize::try_from(parsed)
                    .map_err(|_| format!("{option} is too large for this platform"))?;
                if parsed == 0 || parsed > EQUIPMENT_SLOTS {
                    return Err(format!("{option} must be from 1 to {EQUIPMENT_SLOTS}"));
                }
                max_records = parsed;
            }
            "--out" => output = Some(value.clone()),
            other => return Err(format!("unknown option {other}\n{}", usage())),
        }
        index += 2;
    }
    Ok(Options {
        item_id,
        slot,
        max_records,
        output,
    })
}

fn parse_u64(text: &str, option: &str) -> Result<u64, String> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"));
    match digits {
        Some(hex) if !hex.is_empty() => u64::from_str_radix(hex, 16)
            .map_err(|error| format!("{option} must be an integer: {error}")),
        Some(_) => Err(format!("{option} must contain hexadecimal digits")),
        None => trimmed
            .parse::<u64>()
            .map_err(|error| format!("{option} must be an integer: {error}")),
    }
}

fn capture(options: &Options) -> Result<Value, Failure> {
    let pid = single_process_id(GAME_IMAGE_NAME).map_err(Failure::Refused)?;
    let expected_creation = process_creation_filetime(pid)
        .map_err(Failure::Refused)?
        .ok_or(Failure::Refused(RuntimeError::ProcessGone { pid }))?;
    let process = ReadOnlyProcess::open(pid, GAME_MODULE_NAME, Some(expected_creation))
        .map_err(Failure::Refused)?;

    // The path comes from the same read handle used for memory, then both the
    // fixed version and the pinned image digest are checked before any heap
    // record is read.
    let image_path = process.image_path().map_err(Failure::Refused)?;
    let executable = verify_game_executable(&image_path);
    let file_version = match (executable.state, executable.file_version) {
        (GameCompatibility::Supported, Some(version)) => version,
        (state, _) => {
            return Err(Failure::Refused(RuntimeError::GameExecutableUnsupported {
                path: image_path,
                state: state.as_str(),
            }))
        }
    };
    if file_version != EXPECTED_FILE_VERSION {
        return Err(Failure::Refused(RuntimeError::UnsupportedGameVersion {
            display: file_version.display(),
        }));
    }
    let executable_sha256 = file_sha256(&image_path).map_err(Failure::Refused)?;
    if executable_sha256 != INVENTORY_EXECUTABLE_SHA256 {
        return Err(Failure::Refused(RuntimeError::ExecutableDigestMismatch {
            path: image_path,
            expected: INVENTORY_EXECUTABLE_SHA256.to_string(),
            actual: executable_sha256,
        }));
    }

    let sites = inventory_sites();
    let verified_signatures = process.verify_sites(&sites).map_err(Failure::Refused)?;
    let memory = ProcessInventoryMemory::new(&process);
    let character = read_character(&memory).map_err(Failure::Refused)?;
    let expected_bytes = EQUIPMENT_SLOTS * EQUIPMENT_RECORD_BYTES;
    if character.equipment.len() != expected_bytes {
        return Err(Failure::Refused(RuntimeError::InventoryChain {
            detail: format!(
                "character equipment array is {} bytes, expected {expected_bytes}",
                character.equipment.len()
            ),
        }));
    }

    let mut records = Vec::new();
    let mut matched_occupied = 0usize;
    for (slot_index, record) in character
        .equipment
        .chunks_exact(EQUIPMENT_RECORD_BYTES)
        .enumerate()
    {
        let record_item_id = u16::from_le_bytes([record[0], record[1]]);
        if record_item_id == 0 || options.slot.is_some_and(|slot| slot != slot_index) {
            continue;
        }
        if options
            .item_id
            .is_some_and(|item_id| item_id != record_item_id)
        {
            continue;
        }
        matched_occupied += 1;
        if records.len() < options.max_records {
            records.push(record_json(slot_index, record));
        }
    }

    let process_identity = process.identity();
    Ok(json!({
        "status": "observed",
        "read_only": true,
        "game_version": INVENTORY_GAME_VERSION,
        "executable_sha256": INVENTORY_EXECUTABLE_SHA256,
        "verified_signature_count": verified_signatures,
        "process": {
            "pid": process_identity.pid,
            "creation_filetime": process_identity.creation_filetime.to_string(),
        },
        "module": {
            "base": format!("{:#x}", process.module_range().base),
            "size": format!("{:#x}", process.module_range().size),
        },
        "filters": {
            "item_id": options.item_id,
            "slot_index": options.slot,
            "max_records": options.max_records,
        },
        "matched_occupied_count": matched_occupied,
        "returned_record_count": records.len(),
        "truncated": records.len() < matched_occupied,
        "records": records,
    }))
}

fn record_json(slot_index: usize, record: &[u8]) -> Value {
    let effect_slots = (0..EFFECT_COUNT)
        .map(|effect_index| {
            let entry = EFFECT_ENTRY_START + effect_index * EFFECT_STRIDE;
            json!({
                "slot_index": effect_index,
                "effect_id": u32_at(record, entry + 4),
                "value": u32_at(record, entry + 8),
                "roll": record[entry + 0x0C],
                "marker": record[entry + 0x0E],
            })
        })
        .collect::<Vec<Value>>();

    json!({
        "slot_index": slot_index,
        "item_id": u16_at(record, 0x00),
        "record_0x31": record[0x31],
        "record_0x32": record[0x32],
        "level": u16_at(record, 0x06),
        "plus": u16_at(record, 0x0A),
        "rarity": record[0x30],
        "record_sha256": sha256_hex(record),
        "remodel_neutral_public_region_sha256": remodel_neutral_public_region_sha256(record),
        "compact_byte_diff_evidence": {
            // This public projection stops before the private serial/key words
            // and contains the rarity bytes plus the seven effect entries.
            "public_region_start": format!("0x{PUBLIC_REGION_START:02X}"),
            "public_region_end_exclusive": format!("0x{PUBLIC_REGION_END:02X}"),
            "public_region_sha256": sha256_hex(&record[PUBLIC_REGION_START..PUBLIC_REGION_END]),
            "nonzero_offset_ranges": nonzero_offset_ranges(record),
            "effect_slots": effect_slots,
        },
    })
}

fn u16_at(record: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([record[offset], record[offset + 1]])
}

fn u32_at(record: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        record[offset],
        record[offset + 1],
        record[offset + 2],
        record[offset + 3],
    ])
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn remodel_neutral_public_region_sha256(record: &[u8]) -> String {
    let mut region = record[PUBLIC_REGION_START..PUBLIC_REGION_END].to_vec();
    region[0x31 - PUBLIC_REGION_START] = 0;
    region[0x32 - PUBLIC_REGION_START] = 0;
    sha256_hex(&region)
}

fn nonzero_offset_ranges(record: &[u8]) -> Vec<Value> {
    let mut ranges = Vec::new();
    let mut start = None;
    for offset in PUBLIC_REGION_START..PUBLIC_REGION_END {
        if record[offset] != 0 {
            if start.is_none() {
                start = Some(offset);
            }
        } else if let Some(begin) = start.take() {
            ranges.push(json!([begin, offset]));
        }
    }
    if let Some(begin) = start {
        ranges.push(json!([begin, PUBLIC_REGION_END]));
    }
    ranges
}

fn render_and_write(value: &Value, output: Option<&str>) -> Result<(), String> {
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot render the snapshot: {error}"))?;
    println!("{rendered}");
    if let Some(path) = output {
        std::fs::write(path, format!("{rendered}\n"))
            .map_err(|error| format!("cannot write {path}: {error}"))?;
    }
    Ok(())
}

fn refusal(error: &RuntimeError) -> Value {
    json!({
        "refused": {
            "code": error.code(),
            "message": error.message(),
        }
    })
}
