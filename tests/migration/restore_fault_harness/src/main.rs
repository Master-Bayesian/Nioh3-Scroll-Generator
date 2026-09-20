//! Test-only driver for Restore plans through the save crate's public API.
//!
//! It drives a plan to a named fault point and, when asked, exits before any
//! in-process rollback can run so a parent test observes a real crash-cut. A
//! separate `classify` command reads the durable restore journal and the current
//! role bytes so the parent can confirm that a restart distinguishes an
//! untouched target, source A, checkpoint B and an external C.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use nioh3_save::sha256_hex;
use nioh3_save::transaction::{
    related_save_paths, CrashHook, FaultPoint, PlanCommand, PlanKind, RestoreCut, SaveRole,
    SaveTransactionHost, TransactionFaults,
};

struct Arguments {
    command: String,
    state_root: PathBuf,
    backup_root: Option<PathBuf>,
    save_path: PathBuf,
    backup_id: Option<String>,
    plan_id: Option<String>,
    fault: Option<FaultPoint>,
    crash: bool,
    crash_cut: Option<RestoreCut>,
}

fn parse_crash_cut(value: &str) -> Result<RestoreCut, String> {
    match value {
        "journal-create" => return Ok(RestoreCut::JournalCreate),
        "journal-write" => return Ok(RestoreCut::JournalWrite),
        "journal-flush" => return Ok(RestoreCut::JournalFlush),
        "journal-replace" => return Ok(RestoreCut::JournalReplace),
        _ => {}
    }
    let (kind, role_label) = value.split_once(':').ok_or_else(|| {
        format!(
            "crash cut {value} must be a journal stage or \
             <before-replace|after-replace>:<role>"
        )
    })?;
    let role = SaveRole::from_label(role_label)
        .ok_or_else(|| format!("unknown role in crash cut {value}"))?;
    match kind {
        "before-replace" => Ok(RestoreCut::BeforeReplace(role)),
        "after-replace" => Ok(RestoreCut::AfterReplace(role)),
        other => Err(format!("unknown crash cut kind {other}")),
    }
}

fn parse_fault(value: &str) -> Result<FaultPoint, String> {
    FaultPoint::ALL
        .into_iter()
        .find(|point| point.label() == value)
        .ok_or_else(|| format!("unknown fault point {value}"))
}

fn parse() -> Result<Arguments, String> {
    let mut values = env::args().skip(1);
    let command = values.next().ok_or("prepare or commit is required")?;
    let mut state_root = None;
    let mut backup_root = None;
    let mut save_path = None;
    let mut backup_id = None;
    let mut plan_id = None;
    let mut fault = None;
    let mut crash = false;
    let mut crash_cut = None;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--state-root" => state_root = values.next().map(PathBuf::from),
            "--backup-root" => backup_root = values.next().map(PathBuf::from),
            "--save-path" => save_path = values.next().map(PathBuf::from),
            "--backup-id" => backup_id = values.next(),
            "--plan-id" => plan_id = values.next(),
            "--point" => fault = Some(parse_fault(&values.next().ok_or("--point needs a value")?)?),
            "--crash" => crash = true,
            "--crash-cut" => {
                crash_cut = Some(parse_crash_cut(
                    &values.next().ok_or("--crash-cut needs a value")?,
                )?)
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Arguments {
        command,
        state_root: state_root.ok_or("--state-root is required")?,
        backup_root,
        save_path: save_path.ok_or("--save-path is required")?,
        backup_id,
        plan_id,
        fault,
        crash,
        crash_cut,
    })
}

fn run() -> Result<(), String> {
    let arguments = parse()?;
    match arguments.command.as_str() {
        "prepare" => {
            let backup_id = arguments.backup_id.ok_or("--backup-id is required")?;
            let host = SaveTransactionHost::new(&arguments.state_root).without_quiescence_delay();
            let host = match arguments.backup_root.clone() {
                Some(root) => host.with_backup_root(root),
                None => host,
            };
            let bytes = fs::read(&arguments.save_path).map_err(|error| error.to_string())?;
            let plan = host
                .plan(
                    PlanKind::Restore,
                    &arguments.save_path,
                    &sha256_hex(&bytes),
                    PlanCommand::RestoreFromBackup { backup_id },
                )
                .map_err(|error| error.to_string())?;
            host.store_plan(&plan).map_err(|error| error.to_string())?;
            println!("{}", plan.plan_id);
            Ok(())
        }
        "commit" => {
            let plan_id = arguments.plan_id.ok_or("--plan-id is required")?;
            let faults = TransactionFaults::default();
            if let Some(point) = arguments.fault {
                faults.arm(point);
            }
            let host = SaveTransactionHost::with_faults(&arguments.state_root, faults);
            let host = match arguments.backup_root.clone() {
                Some(root) => host.with_backup_root(root),
                None => host,
            };
            let host = match arguments.crash_cut {
                Some(cut) => host.with_crash_hook(CrashHook::Exit(cut)),
                None => host,
            };
            let plan = host
                .load_plan_untrusted(&plan_id)
                .map_err(|error| error.to_string())?;
            let outcome = host.commit(&plan);
            // A crash cut exits inside `commit_restore`, so reaching here means
            // the cut never fired; surface that as an error rather than a
            // silent success. `--crash` is the older post-return cut retained
            // for the existing injected-fault cases.
            if arguments.crash_cut.is_some() {
                return Err("the deterministic crash cut did not fire".to_string());
            }
            if arguments.crash {
                // A crash point is reached only when the armed stage fired.
                // Exiting here means the parent sees a real process death, with
                // no in-process rollback or finish_restore_failure having run.
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
        "commit-by-id" => {
            let plan_id = arguments.plan_id.ok_or("--plan-id is required")?;
            let host = SaveTransactionHost::new(&arguments.state_root).without_quiescence_delay();
            let host = match arguments.backup_root.clone() {
                Some(root) => host.with_backup_root(root),
                None => host,
            };
            let outcome = host
                .commit_by_id(&plan_id)
                .map_err(|error| error.to_string())?;
            println!("{}", outcome.outcome);
            Ok(())
        }
        "classify" => {
            // Read-only: print the durable journal state plus, per role, the
            // current target digest and which of B (recorded before/checkpoint),
            // A (recorded source) or an external C the bytes match. It never
            // writes to a target.
            let plan_id = arguments.plan_id.ok_or("--plan-id is required")?;
            let host = SaveTransactionHost::new(&arguments.state_root);
            let journal = read_restore_journal(&arguments.state_root)
                .ok_or("no restore journal was found under the state root")?;
            let operation = journal
                .get("operation_id")
                .and_then(|value| value.as_str())
                .unwrap_or("<missing>");
            println!(
                "journal_state={}",
                journal
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<missing>")
            );
            println!("journal_operation_id={operation}");
            let roles = journal
                .get("role_results")
                .and_then(|value| value.as_array())
                .ok_or("restore journal has no role_results array")?;
            let targets = related_save_paths(&arguments.save_path);
            for entry in roles {
                let role_label = entry
                    .get("role")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<missing>");
                let role = role_from_label(role_label)
                    .ok_or_else(|| format!("unknown role {role_label}"))?;
                let target = targets
                    .iter()
                    .find(|(target_role, _)| *target_role == role)
                    .map(|(_, path)| path.clone())
                    .ok_or_else(|| format!("no target path for role {role_label}"))?;
                let current = fs::read(&target).ok().map(|bytes| sha256_hex(&bytes));
                let before = entry.get("target_before_sha256").and_then(|v| v.as_str());
                let source = entry.get("source_sha256").and_then(|v| v.as_str());
                // A role the journal never dispatched keeps its checkpoint
                // classification even if its bytes are the same as the source.
                let dispatched = entry
                    .get("replacement_started")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let class = match current.as_deref() {
                    Some(digest) if dispatched && Some(digest) == source => "A_source",
                    Some(digest) if Some(digest) == before => "B_checkpoint",
                    Some(_) => "external_C",
                    None => "missing",
                };
                // One `key=value` line per field keeps the parent parser trivial
                // and lets it assert an exact per-role classification.
                println!("{role_label}.class={class}");
                println!(
                    "{role_label}.replacement_started={}",
                    entry
                        .get("replacement_started")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                );
                println!(
                    "{role_label}.replacement_completed={}",
                    entry
                        .get("replacement_completed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                );
            }
            // The receipt projection is separate from the journal on purpose: a
            // restart must not depend on a successfully persisted terminal receipt.
            match host.reconcile(&plan_id) {
                Ok(Some(receipt)) => println!("receipt_outcome={}", receipt.outcome),
                Ok(None) => println!("receipt_outcome=<none>"),
                Err(error) => println!("receipt_outcome=<error:{error}>"),
            }
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn role_from_label(label: &str) -> Option<SaveRole> {
    [SaveRole::System, SaveRole::GameBackup, SaveRole::Main]
        .into_iter()
        .find(|role| role.label() == label)
}

fn read_restore_journal(state_root: &std::path::Path) -> Option<serde_json::Value> {
    let backups = state_root.join("backups");
    let entries = fs::read_dir(backups).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join("restore-journal.json");
        if candidate.is_file() {
            let text = fs::read_to_string(&candidate).ok()?;
            return serde_json::from_str(&text).ok();
        }
    }
    None
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
