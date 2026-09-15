//! Drive the guarded save-transaction host from the command line.
//!
//! Every command takes `--state-root`; product commands also take `--save-path`.
//! Writes happen only at the paths the caller supplies, so the parity gate can
//! run the whole lifecycle against task-local fixture copies.
//!
//! Commands
//! - `plan` / `plan-edit` / `plan-delete` / `plan-install` prepare operations,
//! - `commit --plan-id` commits a stored plan in a fresh process,
//! - `discard` drops a stored plan without writing,
//! - `restore` checkpoints then restores a recorded backup,
//! - `receipt` / `operations` / `reconcile` read the ledger,
//! - `backups` / `recycle` view and recycle application-owned bundles,
//! - `fault --point <stage>` commits a product plan with an injected fault at
//!   one commit stage, which the fault gate uses to prove recovery semantics,
//! - `quiescence-probe` runs one guarded plan with an optional writer that
//!   rewrites a related file inside the window, which the gate uses to prove
//!   the timed multi-file guard,
//! - `bench` / `bench-commit` / `bench-install` measure the read, guarded-edit
//!   and guarded-batch-install surfaces with every guard window intact,
//! - `decrypt-container` / `encrypt-container` move bytes through the ported
//!   Rust codec and write them exactly as transformed, with no field rewritten,
//!   so a gate can check the stored checksum before any oracle normalization.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use nioh3_save::backup::{list_backup_entries, move_backup_to_recycle_bin};
use nioh3_save::error::SaveReadError;
use nioh3_save::transaction::{
    is_operation_id, FaultPoint, PlanCommand, PlanKind, SaveTransactionHost, TransactionFaults,
};
use nioh3_save::transform::{
    patch_local_scroll_header, patch_local_scroll_record, EffectPatch, HeaderPatch, InstallRequest,
    SaveTransformHost, SlotEdit,
};
use nioh3_save::{sha256_hex, SCROLL_RECORD_BYTES};

fn parse_kind(value: &str) -> Result<PlanKind, String> {
    match value {
        "edit" => Ok(PlanKind::Edit),
        "delete" => Ok(PlanKind::Delete),
        "install" => Ok(PlanKind::Install),
        "restore" => Ok(PlanKind::Restore),
        other => Err(format!("unknown kind {other}")),
    }
}

fn parse_slot(value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("bad slot {value}"))
}

fn parse_fault(value: &str) -> Result<FaultPoint, String> {
    FaultPoint::ALL
        .into_iter()
        .find(|point| point.label() == value)
        .ok_or_else(|| format!("unknown fault point {value}"))
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let trimmed = value.trim();
    if !trimmed.len().is_multiple_of(2) {
        return Err("hex text must have an even length".to_string());
    }
    let mut bytes = Vec::with_capacity(trimmed.len() / 2);
    let raw = trimmed.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        let pair = std::str::from_utf8(&raw[index..index + 2]).map_err(|_| "bad hex")?;
        bytes.push(u8::from_str_radix(pair, 16).map_err(|_| "bad hex")?);
        index += 2;
    }
    Ok(bytes)
}

fn record_from_hex(value: &str) -> Result<[u8; SCROLL_RECORD_BYTES], String> {
    let bytes = hex_bytes(value)?;
    if bytes.len() != SCROLL_RECORD_BYTES {
        return Err(format!(
            "a record must be {SCROLL_RECORD_BYTES:#x} bytes, got {:#x}",
            bytes.len()
        ));
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(&bytes);
    Ok(owned)
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// Build one replacement record from an edit specification file.
///
/// The spec is `{"header": {...}, "effects": [{...}]}` where every field is
/// optional. The header is patched first, then the effect slots, exactly as
/// `prepare_edit` composes `patch_local_scroll_header` and
/// `patch_local_scroll_record`.
fn build_replacement(
    current: &[u8],
    spec: &serde_json::Value,
) -> Result<[u8; SCROLL_RECORD_BYTES], String> {
    let mut record = current.to_vec();
    if let Some(header) = spec.get("header") {
        let patch = HeaderPatch {
            playthrough: number(header, "playthrough")?.unwrap_or(3) as u8,
            level: number(header, "level")?.unwrap_or(u64::from(u16::MAX)) as u16,
            recommended_level: number(header, "recommended_level")?.unwrap_or(0) as u16,
            seed: number(header, "seed")?.unwrap_or(0) as u32,
            rarity: number(header, "rarity")?.unwrap_or(5) as u8,
            transfer_count: number(header, "transfer_count")?.unwrap_or(0) as u32,
        };
        record = patch_local_scroll_header(&record, &patch)
            .map_err(|error| error.to_string())?
            .to_vec();
    }
    if let Some(effects) = spec.get("effects") {
        let entries = effects
            .as_array()
            .ok_or_else(|| "effects must be an array".to_string())?;
        if !entries.is_empty() {
            let mut patches = Vec::with_capacity(entries.len());
            for entry in entries {
                patches.push(EffectPatch {
                    slot_index: number(entry, "slot_index")?
                        .ok_or_else(|| "an effect edit needs slot_index".to_string())?
                        as usize,
                    prefix: number(entry, "prefix")?.map(|value| value as u32),
                    effect_id: number(entry, "effect_id")?.map(|value| value as u32),
                    value: number(entry, "value")?.map(|value| value as u32),
                    metadata: number(entry, "metadata")?.map(|value| value as u32),
                    tail_0: number(entry, "tail_0")?.map(|value| value as u32),
                    tail_1: number(entry, "tail_1")?.map(|value| value as u32),
                });
            }
            record = patch_local_scroll_record(&record, &patches)
                .map_err(|error| error.to_string())?
                .to_vec();
        }
    }
    let mut owned = [0u8; SCROLL_RECORD_BYTES];
    owned.copy_from_slice(&record);
    Ok(owned)
}

fn number(value: &serde_json::Value, field: &str) -> Result<Option<u64>, String> {
    match value.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{field} must be a non-negative integer")),
        Some(_) => Err(format!("{field} must be a number")),
    }
}

struct Arguments {
    command: String,
    state_root: PathBuf,
    save_path: Option<PathBuf>,
    kind: Option<String>,
    source_sha256: Option<String>,
    write_file: Option<PathBuf>,
    plan_id: Option<String>,
    backup_id: Option<String>,
    account_id: Option<String>,
    save_slot: Option<String>,
    record_hex: Option<String>,
    record_file: Option<PathBuf>,
    spec_file: Option<PathBuf>,
    container_file: Option<PathBuf>,
    output_file: Option<PathBuf>,
    transfer_count: Option<u32>,
    quiescence_ms: Option<u64>,
    writer_role: Option<String>,
    writer_delay_ms: Option<u64>,
    slots: Vec<usize>,
    fault: Option<FaultPoint>,
    crash: bool,
    repeat: usize,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut command = String::new();
    let mut state_root: Option<PathBuf> = None;
    let mut save_path: Option<PathBuf> = None;
    let mut kind = None;
    let mut source_sha256 = None;
    let mut write_file = None;
    let mut plan_id = None;
    let mut backup_id = None;
    let mut account_id = None;
    let mut save_slot = None;
    let mut record_hex = None;
    let mut record_file = None;
    let mut spec_file = None;
    let mut container_file = None;
    let mut output_file = None;
    let mut transfer_count = None;
    let mut quiescence_ms = None;
    let mut writer_role = None;
    let mut writer_delay_ms = None;
    let mut slots = Vec::new();
    let mut fault = None;
    let mut crash = false;
    let mut repeat = 1usize;

    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "plan" | "plan-edit" | "plan-delete" | "plan-install" | "prepare-edit" | "commit"
            | "discard" | "restore" | "receipt" | "operations" | "reconcile" | "backups"
            | "recycle" | "fault" | "bench" | "bench-commit" | "bench-install"
            | "quiescence-probe" | "decrypt-container" | "encrypt-container" => command = argument,
            "--state-root" => state_root = arguments.next().map(PathBuf::from),
            "--save-path" => save_path = arguments.next().map(PathBuf::from),
            "--kind" => kind = arguments.next(),
            "--source-sha256" => source_sha256 = arguments.next(),
            "--write-file" => write_file = arguments.next().map(PathBuf::from),
            "--container-file" => container_file = arguments.next().map(PathBuf::from),
            "--output-file" => output_file = arguments.next().map(PathBuf::from),
            "--plan-id" => plan_id = arguments.next(),
            "--backup-id" => backup_id = arguments.next(),
            "--account-id" => account_id = arguments.next(),
            "--save-slot" => save_slot = arguments.next(),
            "--record-hex" => record_hex = arguments.next(),
            "--record-file" => record_file = arguments.next().map(PathBuf::from),
            "--spec-file" => spec_file = arguments.next().map(PathBuf::from),
            "--quiescence-ms" => {
                quiescence_ms = Some(
                    arguments
                        .next()
                        .ok_or("--quiescence-ms needs a value")?
                        .parse::<u64>()
                        .map_err(|_| "--quiescence-ms must be a millisecond count")?,
                )
            }
            "--writer-role" => writer_role = arguments.next(),
            "--writer-delay-ms" => {
                writer_delay_ms = Some(
                    arguments
                        .next()
                        .ok_or("--writer-delay-ms needs a value")?
                        .parse::<u64>()
                        .map_err(|_| "--writer-delay-ms must be a millisecond count")?,
                )
            }
            "--transfer-count" => {
                transfer_count = Some(
                    arguments
                        .next()
                        .ok_or("--transfer-count needs a value")?
                        .parse::<u32>()
                        .map_err(|_| "--transfer-count must be a u32")?,
                )
            }
            "--slot" => slots.push(parse_slot(
                arguments.next().ok_or("--slot needs a value")?.as_str(),
            )?),
            "--point" => {
                fault = Some(parse_fault(
                    arguments.next().ok_or("--point needs a value")?.as_str(),
                )?)
            }
            "--crash" => crash = true,
            "--repeat" => {
                repeat = arguments
                    .next()
                    .ok_or("--repeat needs a value")?
                    .parse::<usize>()
                    .map_err(|_| "--repeat must be a count")?
                    .max(1)
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if command.is_empty() {
        return Err("a command is required".to_string());
    }
    Ok(Arguments {
        command,
        state_root: state_root.ok_or("--state-root is required")?,
        save_path,
        kind,
        source_sha256,
        write_file,
        plan_id,
        backup_id,
        account_id,
        save_slot,
        record_hex,
        record_file,
        spec_file,
        container_file,
        output_file,
        transfer_count,
        quiescence_ms,
        writer_role,
        writer_delay_ms,
        slots,
        fault,
        crash,
        repeat,
    })
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    let host = SaveTransactionHost::new(&arguments.state_root);
    // The shipped window is the default; a probe may widen or zero it, but the
    // two-pass comparison behind it never turns off.
    let host = match arguments.quiescence_ms {
        Some(millis) => host.with_quiescence_interval(Duration::from_millis(millis)),
        None => host,
    };
    let save_path = arguments
        .save_path
        .clone()
        .ok_or("--save-path is required for this command")?;

    match arguments.command.as_str() {
        "plan" => {
            let kind = parse_kind(&arguments.kind.clone().ok_or("--kind is required")?)?;
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            let bytes = fs::read(
                arguments
                    .write_file
                    .clone()
                    .ok_or("--write-file is required")?,
            )
            .map_err(|error| error.to_string())?;
            let plan = host
                .plan(kind, &save_path, &source, PlanCommand::WriteMain { bytes })
                .map_err(|error| error.to_string())?;
            host.store_plan(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.plan_id);
            Ok(())
        }
        "plan-edit" => {
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            let spec = read_text(
                arguments
                    .spec_file
                    .as_ref()
                    .ok_or("--spec-file is required")?,
            )?;
            let parsed: serde_json::Value =
                serde_json::from_str(&spec).map_err(|error| error.to_string())?;
            let edits = parsed
                .get("edits")
                .and_then(serde_json::Value::as_array)
                .ok_or("the spec needs an edits array")?;
            let transform = SaveTransformHost::register(&save_path).map_err(|e| e.to_string())?;
            let mut planned = Vec::with_capacity(edits.len());
            for entry in edits {
                let slot_index =
                    number(entry, "slot_index")?.ok_or("an edit needs slot_index")? as usize;
                let offset = slot_index
                    .checked_mul(SCROLL_RECORD_BYTES)
                    .and_then(|value| value.checked_add(0x17_6CCE))
                    .ok_or("slot index overflow")?;
                let current = transform
                    .plaintext()
                    .get(offset..offset + SCROLL_RECORD_BYTES)
                    .ok_or("slot outside the inventory region")?;
                let mut original = [0u8; SCROLL_RECORD_BYTES];
                original.copy_from_slice(current);
                let replacement = build_replacement(current, entry)?;
                planned.push(SlotEdit {
                    slot_index,
                    expected_original: original,
                    replacement,
                });
            }
            let plan = host
                .plan_edit(&save_path, &source, planned)
                .map_err(|error| error.to_string())?;
            host.store_plan(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.plan_id);
            Ok(())
        }
        "prepare-edit" => {
            // Measure the prepare path without leaving a stored plan behind: the
            // full transform and encryption run, then the plan is not persisted.
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            let spec = read_text(
                arguments
                    .spec_file
                    .as_ref()
                    .ok_or("--spec-file is required")?,
            )?;
            let parsed: serde_json::Value =
                serde_json::from_str(&spec).map_err(|error| error.to_string())?;
            let entries = parsed
                .get("edits")
                .and_then(serde_json::Value::as_array)
                .ok_or("the spec needs an edits array")?;
            let transform = SaveTransformHost::register(&save_path).map_err(|e| e.to_string())?;
            let mut planned = Vec::with_capacity(entries.len());
            for entry in entries {
                let slot_index =
                    number(entry, "slot_index")?.ok_or("an edit needs slot_index")? as usize;
                let offset = slot_index
                    .checked_mul(SCROLL_RECORD_BYTES)
                    .and_then(|value| value.checked_add(0x17_6CCE))
                    .ok_or("slot index overflow")?;
                let current = transform
                    .plaintext()
                    .get(offset..offset + SCROLL_RECORD_BYTES)
                    .ok_or("slot outside the inventory region")?;
                let mut original = [0u8; SCROLL_RECORD_BYTES];
                original.copy_from_slice(current);
                planned.push(SlotEdit {
                    slot_index,
                    expected_original: original,
                    replacement: build_replacement(current, entry)?,
                });
            }
            // Validate the source identity exactly as `plan_edit` does, then run
            // the real transform. The plan is not persisted, so the measurement
            // covers read + decrypt + transform + encrypt only.
            let plan = host
                .plan_edit(&save_path, &source, planned)
                .map_err(|error| error.to_string())?;
            let edits = match plan.product.as_ref() {
                Some(nioh3_save::ProductPlanData::Edit { edits }) => edits.clone(),
                _ => return Err("the prepared plan is not an edit".to_string()),
            };
            let plaintext = SaveTransformHost::register(&save_path)
                .map_err(|error| error.to_string())?
                .edit(&edits)
                .map_err(|error| error.to_string())?;
            let container = nioh3_save::crypto::encrypt_container(&plaintext.plaintext)
                .map_err(|error| error.to_string())?;
            println!(
                "prepared\t{}\t{}",
                plan.baseline.len(),
                sha256_hex(&container)
            );
            Ok(())
        }
        "plan-delete" => {
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            if arguments.slots.is_empty() {
                return Err("--slot is required".to_string());
            }
            let plan = host
                .plan_delete(&save_path, &source, arguments.slots.clone())
                .map_err(|error| error.to_string())?;
            host.store_plan(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.plan_id);
            Ok(())
        }
        "plan-install" => {
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            let records = install_records(&arguments)?;
            let transfer_count = arguments.transfer_count.unwrap_or(0);
            let requests: Vec<InstallRequest> = records
                .into_iter()
                .map(|candidate_record| InstallRequest {
                    candidate_record,
                    transfer_count,
                })
                .collect();
            let plan = if requests.len() == 1 {
                host.plan_install(&save_path, &source, requests[0].clone())
            } else {
                host.plan_install_many(&save_path, &source, requests)
            }
            .map_err(|error| error.to_string())?;
            host.store_plan(&plan).map_err(|error| error.to_string())?;
            // Report the realized identity of every installed record so a parity
            // gate can compare slots, keys, serials and installed bytes without
            // decrypting the result again.
            let transform = SaveTransformHost::register(&save_path).map_err(|e| e.to_string())?;
            let planned = match plan.product.as_ref() {
                Some(nioh3_save::ProductPlanData::Install { request }) => transform
                    .install(request)
                    .map_err(|error| error.to_string())?,
                Some(nioh3_save::ProductPlanData::InstallMany { requests }) => transform
                    .install_many(requests)
                    .map_err(|error| error.to_string())?,
                _ => return Err("the prepared plan is not an install".to_string()),
            };
            println!("{}", plan.plan_id);
            println!(
                "slots\t{}",
                planned
                    .slot_indices
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!(
                "keys\t{}",
                planned
                    .inventory_keys
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!(
                "serials\t{}",
                planned
                    .generation_serials
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!(
                "records\t{}",
                planned
                    .installed_records
                    .iter()
                    .map(|record| hex_encode(record))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            println!("checksum\t{}\t{}", planned.checksum.0, planned.checksum.1);
            Ok(())
        }
        "commit" => {
            let plan_id = arguments.plan_id.clone().ok_or("--plan-id is required")?;
            if !is_operation_id(&plan_id) {
                return Err(format!("{plan_id:?} is not a valid operation id"));
            }
            let plan = host
                .load_plan_untrusted(&plan_id)
                .map_err(|error| error.to_string())?;
            let receipt = host.commit(&plan).map_err(|error| error.to_string())?;
            println!("{}", receipt.outcome);
            Ok(())
        }
        "fault" => {
            let plan_id = arguments.plan_id.clone().ok_or("--plan-id is required")?;
            let point = arguments.fault.ok_or("--point is required")?;
            let plan = host
                .load_plan_untrusted(&plan_id)
                .map_err(|error| error.to_string())?;
            let faults = TransactionFaults::default();
            faults.arm(point);
            let fault_host = SaveTransactionHost::with_faults(&arguments.state_root, faults);
            let outcome = fault_host.commit(&plan);
            if arguments.crash {
                // A crash point is reached only when the armed stage fired, so
                // a stage that never runs surfaces as an ordinary error rather
                // than a silent success.
                match &outcome {
                    Err(error) if error.to_string().contains("injected fault") => {
                        std::process::exit(9);
                    }
                    other => return other.as_ref().map(|_| ()).map_err(ToString::to_string),
                }
            }
            println!("{}", outcome.map_err(|error| error.to_string())?.outcome);
            Ok(())
        }
        "discard" => {
            let plan_id = arguments.plan_id.clone().ok_or("--plan-id is required")?;
            let plan = host
                .load_plan_untrusted(&plan_id)
                .map_err(|error| error.to_string())?;
            println!("{}", host.discard(plan).outcome);
            Ok(())
        }
        "restore" => {
            let backup_id = arguments
                .backup_id
                .clone()
                .ok_or("--backup-id is required")?;
            let source = sha256_hex(&fs::read(&save_path).map_err(|error| error.to_string())?);
            let plan = host
                .plan(
                    PlanKind::Restore,
                    &save_path,
                    &source,
                    PlanCommand::RestoreFromBackup {
                        backup_id: backup_id.clone(),
                    },
                )
                .map_err(|error| error.to_string())?;
            let receipt = host.commit(&plan).map_err(|error| error.to_string())?;
            println!("{}", receipt.outcome);
            Ok(())
        }
        "receipt" => {
            let plan_id = arguments.plan_id.clone().ok_or("--plan-id is required")?;
            match host.receipt(&plan_id).map_err(|error| error.to_string())? {
                Some(receipt) => println!(
                    "{}\t{}\t{}",
                    receipt.outcome,
                    receipt.installed_sha256.unwrap_or_else(|| "-".to_string()),
                    receipt.backup_id.unwrap_or_else(|| "-".to_string())
                ),
                None => println!("none"),
            }
            Ok(())
        }
        "operations" => {
            for receipt in host.operations().map_err(|error| error.to_string())? {
                println!("{}\t{}", receipt.operation_id, receipt.outcome);
            }
            Ok(())
        }
        "reconcile" => {
            let plan_id = arguments.plan_id.clone().ok_or("--plan-id is required")?;
            match host
                .reconcile(&plan_id)
                .map_err(|error| error.to_string())?
            {
                Some(receipt) => println!("{}", receipt.outcome),
                None => println!("none"),
            }
            Ok(())
        }
        "backups" => {
            if let (Some(account), Some(slot)) = (&arguments.account_id, &arguments.save_slot) {
                let account_id = account.parse::<u64>().map_err(|_| "bad account id")?;
                let save_slot = slot.parse::<u8>().map_err(|_| "bad save slot")?;
                for entry in
                    nioh3_save::list_backups_for(&arguments.state_root, account_id, save_slot)
                        .map_err(|error| error.to_string())?
                {
                    println!(
                        "{}\t{}\t{}\t{}",
                        entry.backup_id,
                        entry.action,
                        entry.file_count,
                        entry.manifest_schema.unwrap_or_else(|| "-".to_string())
                    );
                }
            } else {
                for entry in
                    list_backup_entries(&arguments.state_root).map_err(|error| error.to_string())?
                {
                    println!(
                        "{}\t{}\t{}",
                        entry.backup_id,
                        entry.action,
                        entry.main_save_sha256.unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            Ok(())
        }
        "bench" => {
            // Steady-state measurement inside one process: decrypt the container
            // and then run read + transform + encrypt, repeated `--repeat`
            // times. Printed as tab-separated microseconds so the harness can
            // tell cold-spawn cost apart from per-call cost.
            let container = fs::read(&save_path).map_err(|error| error.to_string())?;
            let iterations = arguments.repeat;
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let decrypted =
                    nioh3_save::decrypt_container(&container).map_err(|error| error.to_string())?;
                let _ =
                    nioh3_save::DecryptedSave::new(decrypted).map_err(|error| error.to_string())?;
            }
            let decrypt_elapsed = start.elapsed();

            let spec = read_text(
                arguments
                    .spec_file
                    .as_ref()
                    .ok_or("--spec-file is required")?,
            )?;
            let parsed: serde_json::Value =
                serde_json::from_str(&spec).map_err(|error| error.to_string())?;
            let entries = parsed
                .get("edits")
                .and_then(serde_json::Value::as_array)
                .ok_or("the spec needs an edits array")?;
            let source = arguments
                .source_sha256
                .clone()
                .ok_or("--source-sha256 is required")?;
            let transform = SaveTransformHost::register(&save_path).map_err(|e| e.to_string())?;
            let mut planned = Vec::with_capacity(entries.len());
            for entry in entries {
                let slot_index =
                    number(entry, "slot_index")?.ok_or("an edit needs slot_index")? as usize;
                let offset = slot_index
                    .checked_mul(SCROLL_RECORD_BYTES)
                    .and_then(|value| value.checked_add(0x17_6CCE))
                    .ok_or("slot index overflow")?;
                let current = transform
                    .plaintext()
                    .get(offset..offset + SCROLL_RECORD_BYTES)
                    .ok_or("slot outside the inventory region")?;
                let mut original = [0u8; SCROLL_RECORD_BYTES];
                original.copy_from_slice(current);
                planned.push(SlotEdit {
                    slot_index,
                    expected_original: original,
                    replacement: build_replacement(current, entry)?,
                });
            }
            // The prepare case measures the same unguarded cycle as the shipped
            // `python_prepare` (read + decrypt + transform + re-encrypt), so the
            // plan here runs without the quiescence window; the guarded windows
            // are measured by `bench-commit` / `bench-install`.
            let prepare_host =
                SaveTransactionHost::new(&arguments.state_root).without_quiescence_delay();
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let plan = prepare_host
                    .plan_edit(&save_path, &source, planned.clone())
                    .map_err(|error| error.to_string())?;
                let edits = match plan.product.as_ref() {
                    Some(nioh3_save::ProductPlanData::Edit { edits }) => edits.clone(),
                    _ => return Err("the prepared plan is not an edit".to_string()),
                };
                let plaintext = SaveTransformHost::register(&save_path)
                    .map_err(|error| error.to_string())?
                    .edit(&edits)
                    .map_err(|error| error.to_string())?;
                let _ = nioh3_save::crypto::encrypt_container(&plaintext.plaintext)
                    .map_err(|error| error.to_string())?;
            }
            let prepare_elapsed = start.elapsed();
            let to_micros = |duration: std::time::Duration| duration.as_micros();
            println!(
                "bench\t{iterations}\t{}\t{}",
                to_micros(decrypt_elapsed),
                to_micros(prepare_elapsed)
            );
            Ok(())
        }
        "recycle" => {
            let backup_id = arguments
                .backup_id
                .clone()
                .ok_or("--backup-id is required")?;
            move_backup_to_recycle_bin(&arguments.state_root, &backup_id)
                .map_err(|error| error.to_string())?;
            println!("recycled");
            Ok(())
        }
        "bench-commit" => {
            // Guarded commit benchmark with every product safeguard intact:
            // quiescent baseline, durable pending receipt, checkpoint, atomic
            // replacement and readback. Each iteration starts from the caller's
            // current save, so the harness restores the quiet generation between
            // iterations exactly as a fresh user operation would.
            let checksum_document = read_text(
                arguments
                    .spec_file
                    .as_ref()
                    .ok_or("--spec-file is required")?,
            )?;
            let parsed: serde_json::Value =
                serde_json::from_str(&checksum_document).map_err(|error| error.to_string())?;
            let entries = parsed
                .get("edits")
                .and_then(serde_json::Value::as_array)
                .ok_or("the spec needs an edits array")?;
            let transform = SaveTransformHost::register(&save_path).map_err(|e| e.to_string())?;
            let mut edits = Vec::with_capacity(entries.len());
            for entry in entries {
                let slot_index =
                    number(entry, "slot_index")?.ok_or("an edit needs slot_index")? as usize;
                let offset = slot_index
                    .checked_mul(SCROLL_RECORD_BYTES)
                    .and_then(|value| value.checked_add(0x17_6CCE))
                    .ok_or("slot index overflow")?;
                let current = transform
                    .plaintext()
                    .get(offset..offset + SCROLL_RECORD_BYTES)
                    .ok_or("slot outside the inventory region")?;
                let mut original = [0u8; SCROLL_RECORD_BYTES];
                original.copy_from_slice(current);
                edits.push(SlotEdit {
                    slot_index,
                    expected_original: original,
                    replacement: build_replacement(current, entry)?,
                });
            }
            let mut iterations = 0usize;
            let mut total_micros: u128 = 0;
            for _ in 0..arguments.repeat {
                let source = sha256_hex(&fs::read(&save_path).map_err(|error| error.to_string())?);
                // Time the whole operation the product performs: prepare the
                // plan (quiescent baseline + identity gate) and commit it
                // (backup, durable receipt, atomic replace, readback), with
                // every guard window intact.
                let start = std::time::Instant::now();
                let plan = host
                    .plan_edit(&save_path, &source, edits.clone())
                    .map_err(|error| error.to_string())?;
                host.commit(&plan).map_err(|error| error.to_string())?;
                total_micros += start.elapsed().as_micros();
                iterations += 1;
            }
            println!("bench-commit\t{iterations}\t{total_micros}");
            Ok(())
        }
        "bench-install" => {
            // The same end-to-end measurement for the batch-install surface,
            // which is the shipped operation that actually carries the 0.20 s
            // quiescence windows (`SaveInstaller.install_many`).
            let records = install_records(&arguments)?;
            let transfer_count = arguments.transfer_count.unwrap_or(0);
            let requests: Vec<InstallRequest> = records
                .into_iter()
                .map(|candidate_record| InstallRequest {
                    candidate_record,
                    transfer_count,
                })
                .collect();
            let mut iterations = 0usize;
            let mut total_micros: u128 = 0;
            for _ in 0..arguments.repeat {
                let source = sha256_hex(&fs::read(&save_path).map_err(|error| error.to_string())?);
                let start = std::time::Instant::now();
                let plan = host
                    .plan_install_many(&save_path, &source, requests.clone())
                    .map_err(|error| error.to_string())?;
                host.commit(&plan).map_err(|error| error.to_string())?;
                total_micros += start.elapsed().as_micros();
                iterations += 1;
            }
            println!("bench-install\t{iterations}\t{total_micros}");
            Ok(())
        }
        "decrypt-container" => {
            // Byte-preserving decode with the ported Rust codec: no shipped
            // process runs, and no field is rewritten, so a caller can assert
            // the exact bytes the Rust writer encrypted (including the user
            // checksum) before any oracle normalization touches them.
            let container = arguments
                .container_file
                .clone()
                .ok_or("--container-file is required")?;
            let output = arguments
                .output_file
                .clone()
                .ok_or("--output-file is required")?;
            let bytes = fs::read(&container).map_err(|error| error.to_string())?;
            let clear = nioh3_save::decrypt_container(&bytes).map_err(|error| error.to_string())?;
            fs::write(&output, &clear).map_err(|error| error.to_string())?;
            println!("decrypted\t{}\t{}", clear.len(), sha256_hex(&clear));
            Ok(())
        }
        "encrypt-container" => {
            // The mirror of `decrypt-container`: encode `--write-file` into
            // `--output-file` with the ported Rust codec, rewriting nothing.
            let clear_file = arguments
                .write_file
                .clone()
                .ok_or("--write-file is required")?;
            let output = arguments
                .output_file
                .clone()
                .ok_or("--output-file is required")?;
            let clear = fs::read(&clear_file).map_err(|error| error.to_string())?;
            let container =
                nioh3_save::encrypt_container(&clear).map_err(|error| error.to_string())?;
            fs::write(&output, &container).map_err(|error| error.to_string())?;
            println!("encrypted\t{}\t{}", container.len(), sha256_hex(&container));
            Ok(())
        }
        "quiescence-probe" => {
            // Deterministic external-writer probe for the timed multi-file
            // guard. The writer lands inside the window on a related file that
            // is not the write target, so only the generation guard can catch
            // it. The outcome is reported instead of exiting non-zero, so a
            // caller can tell "the guard refused" from "something else broke".
            let parent = save_path
                .parent()
                .ok_or("the save path has no parent directory")?;
            let bytes = fs::read(&save_path).map_err(|error| error.to_string())?;
            let source = sha256_hex(&bytes);

            // One warm-up pass measures what a fingerprint pass costs here. The
            // writer is then scheduled half a window after that cost, which is
            // always past the guard's first pass and always before its second —
            // a real write inside the window rather than a race with it.
            let warm_started = std::time::Instant::now();
            let _ = nioh3_save::capture_related_fingerprints(&save_path)
                .map_err(|error| error.to_string())?;
            let warm_millis = warm_started.elapsed().as_millis() as u64;
            let window_millis = host.quiescence_interval().as_millis() as u64;
            let default_delay = warm_millis + window_millis / 2;

            let mut writer = None;
            let mut writer_delay = None;
            if let Some(role) = arguments.writer_role.clone() {
                let target = match role.as_str() {
                    "main" => save_path.clone(),
                    "backup" => parent.join("BACKUP.BIN"),
                    "system" => parent
                        .parent()
                        .ok_or("the save path has no account directory")?
                        .join("SYSTEMSAVEDATA00")
                        .join("SAVEDATA.BIN"),
                    other => return Err(format!("unknown writer role {other}")),
                };
                let delay = arguments.writer_delay_ms.unwrap_or(default_delay);
                writer_delay = Some(delay);
                if delay == 0 {
                    fs::write(&target, b"external writer outside any window")
                        .map_err(|error| error.to_string())?;
                } else {
                    let target = target.clone();
                    writer = Some(std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(delay));
                        let _ = fs::write(&target, b"external writer inside the quiescence window");
                    }));
                }
            }

            let started = std::time::Instant::now();
            let outcome = host.plan(
                PlanKind::Install,
                &save_path,
                &source,
                PlanCommand::WriteMain { bytes },
            );
            let elapsed = started.elapsed().as_millis();
            if let Some(writer) = writer {
                let _ = writer.join();
            }
            let writer_field = writer_delay
                .map(|delay| format!("writer_delay_ms={delay}"))
                .unwrap_or_else(|| "-".to_string());
            match outcome {
                Ok(_) => {
                    println!("guarded\t{elapsed}\t{writer_field}\twarm_ms={warm_millis}");
                    Ok(())
                }
                Err(SaveReadError::SaveChanged { .. }) => {
                    println!("refused\t{elapsed}\t{writer_field}\twarm_ms={warm_millis}");
                    Ok(())
                }
                Err(error) => Err(error.to_string()),
            }
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn install_records(arguments: &Arguments) -> Result<Vec<[u8; SCROLL_RECORD_BYTES]>, String> {
    if let Some(hex) = &arguments.record_hex {
        return Ok(vec![record_from_hex(hex)?]);
    }
    if let Some(path) = &arguments.record_file {
        let text = read_text(path)?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let entries = value
            .get("records")
            .and_then(serde_json::Value::as_array)
            .ok_or("the record file needs a records array".to_string())?;
        let mut records = Vec::with_capacity(entries.len());
        for entry in entries {
            let text = entry
                .as_str()
                .ok_or_else(|| "every record entry must be a hex string".to_string())?;
            records.push(record_from_hex(text)?);
        }
        return Ok(records);
    }
    Err("--record-hex or --record-file is required".to_string())
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
