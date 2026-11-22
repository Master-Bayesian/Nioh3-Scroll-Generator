//! Run the exact planner and scoring engine independently from the desktop app.
use nioh3_loadout::{
    context::{sha256, LoadoutContext},
    engine::{slot_of, Backpack, Candidate, Engine, Limits, ITEM},
    search::{
        EnumerationOrder, ExactCheckpoint, ExactSweep, Request, ScorerKind, SetOption, SetPruning,
        SetRule,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct OwnedRow {
    slot: u32,
    slot_index: usize,
    record_sha256: String,
    assignment_bytes: [u8; 8],
    record_bytes: Vec<u8>,
}

#[derive(Deserialize)]
struct Inventory {
    schema: String,
    only_reads_save: bool,
    searchable_items: usize,
    source_sha256: String,
    records: Vec<OwnedRow>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RunCheckpoint {
    program_sha256: String,
    progress: ExactCheckpoint,
}

fn write_atomic(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let pending = path.with_extension("next");
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&pending)?;
        serde_json::to_writer(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(pending, path)?;
    Ok(())
}

fn assignments(
    candidate: &Candidate,
    pack: &Backpack,
    input: &Inventory,
) -> Vec<serde_json::Value> {
    candidate
        .items
        .iter()
        .enumerate()
        .filter_map(|(position, row)| {
            let row = row.as_ref()?;
            let owned = &input.records[pack.origin[*row as usize]];
            Some(
                json!({"slot":slot_of(position),"save_slot":owned.slot_index,
            "record_sha256":owned.record_sha256}),
            )
        })
        .collect()
}

fn program_hash(bundle: &Path, kind: ScorerKind) -> Result<String, Box<dyn Error>> {
    // Reopening this binary and the same scoring libraries is required to
    // interpret a persisted cursor as part of the same exact solve.
    let mut hash = Sha256::new();
    hash.update(fs::read(std::env::current_exe()?)?);
    let main = if cfg!(windows) {
        "main_cpu.dll"
    } else {
        "main_cpu.so"
    };
    let host = nioh3_loadout::kernel_asset::resolve(bundle, main)?;
    if host.is_file() {
        hash.update(fs::read(host)?);
    } else if cfg!(windows) && bundle.join("kernel.dll").is_file() {
        hash.update(fs::read(bundle.join("kernel.dll"))?);
    }
    if kind == ScorerKind::Cuda {
        let name = if cfg!(windows) {
            "main_device.dll"
        } else {
            "main_device.so"
        };
        let device = nioh3_loadout::kernel_asset::resolve(bundle, name)?;
        hash.update(fs::read(device)?);
    }
    Ok(format!("{:x}", hash.finalize()))
}

const USAGE: &str = concat!(
    "standalone_search <bundle> <inventory.json> <result-dir> <cpu|cuda> ",
    "[outfits-this-run] [linear|spread] --set <id> <minimum-tier> ",
    "[--set <id> <minimum-tier> ...] [--verify-unpruned] ",
    "[--profile] [--no-reuse] [--cuda-reuse] [--batch <1..4096>] [--audit] [--cancel-file <path>] ",
    "[--cancel-after-checks <n>] [--audit-native-inputs] [--context <directory>]"
);

struct Diagnostics {
    profile: bool,
    reuse: bool,
    cuda_reuse: bool,
    batch: usize,
    audit: bool,
    native_inputs: bool,
    context: Option<PathBuf>,
    cancel_file: Option<PathBuf>,
    cancel_after_checks: Option<u64>,
}

fn parse_sets(
    mut flags: &[std::ffi::OsString],
) -> Result<(Vec<SetOption>, SetPruning, Diagnostics), Box<dyn Error>> {
    let mut selected = Vec::new();
    let mut pruning = SetPruning::Proven;
    let mut diagnostics = Diagnostics {
        profile: false,
        reuse: true,
        cuda_reuse: false,
        batch: 2048,
        audit: false,
        native_inputs: false,
        context: None,
        cancel_file: None,
        cancel_after_checks: None,
    };
    while !flags.is_empty() {
        match flags {
            [flag, path, rest @ ..] if flag == "--context" && diagnostics.context.is_none() => {
                diagnostics.context = Some(PathBuf::from(path));
                flags = rest;
            }
            [flag, key, tier, rest @ ..] if flag == "--set" => {
                let key = key.to_str().ok_or(USAGE)?;
                let key = match key.strip_prefix("0x").or_else(|| key.strip_prefix("0X")) {
                    Some(hex) => u32::from_str_radix(hex, 16)?,
                    None => key.parse()?,
                };
                let pieces = tier.to_str().ok_or(USAGE)?.parse()?;
                selected.push(SetOption { key, pieces });
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--verify-unpruned" => {
                pruning = SetPruning::Unpruned;
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--profile" => {
                diagnostics.profile = true;
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--no-reuse" => {
                diagnostics.reuse = false;
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--cuda-reuse" => {
                diagnostics.cuda_reuse = true;
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--audit" => {
                diagnostics.audit = true;
                flags = rest;
            }
            [flag, rest @ ..] if flag == "--audit-native-inputs" => {
                diagnostics.audit = true;
                diagnostics.native_inputs = true;
                flags = rest;
            }
            [flag, batch, rest @ ..] if flag == "--batch" => {
                diagnostics.batch = batch.to_str().ok_or(USAGE)?.parse()?;
                if !(1..=4096).contains(&diagnostics.batch) {
                    return Err(USAGE.into());
                }
                flags = rest;
            }
            [flag, path, rest @ ..] if flag == "--cancel-file" => {
                diagnostics.cancel_file = Some(PathBuf::from(path));
                flags = rest;
            }
            [flag, count, rest @ ..] if flag == "--cancel-after-checks" => {
                let count = count.to_str().ok_or(USAGE)?.parse()?;
                if count == 0 {
                    return Err(USAGE.into());
                }
                diagnostics.cancel_after_checks = Some(count);
                flags = rest;
            }
            _ => return Err(USAGE.into()),
        }
    }
    if selected.is_empty() {
        return Err(format!("Choose at least one set. {USAGE}").into());
    }
    Ok((selected, pruning, diagnostics))
}

fn main() -> Result<(), Box<dyn Error>> {
    let invocation_started = Instant::now();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [bundle, inventory, output, kind, tail @ ..] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let bundle = Path::new(bundle);
    let output = PathBuf::from(output);
    let kind = match kind.to_str() {
        Some("cpu") => ScorerKind::Cpu,
        Some("cuda") => ScorerKind::Cuda,
        _ => return Err(USAGE.into()),
    };
    let flag_start = tail
        .iter()
        .position(|arg| arg.to_string_lossy().starts_with("--"))
        .unwrap_or(tail.len());
    let (positional, flags) = tail.split_at(flag_start);
    let (selected, pruning, diagnostics) = parse_sets(flags)?;
    let (budget, order) = match positional {
        [] => (u64::MAX, EnumerationOrder::Linear),
        [budget] => (
            budget.to_str().ok_or(USAGE)?.parse()?,
            EnumerationOrder::Linear,
        ),
        [budget, order] => {
            let order = match order.to_str() {
                Some("linear") => EnumerationOrder::Linear,
                Some("spread") => EnumerationOrder::Spread,
                _ => return Err(USAGE.into()),
            };
            (budget.to_str().ok_or(USAGE)?.parse()?, order)
        }
        _ => return Err(USAGE.into()),
    };
    let inventory_bytes = fs::read(inventory)?;
    let inventory_file_sha256 = sha256(&inventory_bytes);
    let input: Inventory = serde_json::from_slice(&inventory_bytes)?;
    if input.schema != "nioh3-current-whole-backpack/v1"
        || !input.only_reads_save
        || input.searchable_items != input.records.len()
    {
        return Err("expected a saved inventory export".into());
    }
    let context_dir = diagnostics
        .context
        .clone()
        .unwrap_or_else(|| bundle.join("context"));
    let capture = LoadoutContext::load(&context_dir)?;
    let engine = Engine::open_with_context(bundle, &capture)?;
    let mut rows = Vec::with_capacity(input.records.len());
    let mut seen = HashSet::new();
    for equipment in &input.records {
        let body: [u8; ITEM] = equipment.record_bytes.as_slice().try_into()?;
        if sha256(&body) != equipment.record_sha256 || !seen.insert(equipment.slot_index) {
            return Err("inventory contains a changed or duplicate owned instance".into());
        }
        rows.push((equipment.slot, body));
    }
    let pack = Backpack::from_slots(&rows)?;
    let bank = capture.manifest().character.active as usize;
    let swaps: Vec<_> = pack
        .origin
        .iter()
        .enumerate()
        .filter_map(|(row, &origin)| {
            let equipment = &input.records[origin];
            let offset = bank * 4;
            let position = u32::from_le_bytes(
                equipment.assignment_bytes[offset..offset + 4]
                    .try_into()
                    .expect("one bank word"),
            );
            (equipment.slot == 0 && (1..=3).contains(&position))
                .then_some((row as u32, position as usize))
        })
        .collect();
    let request = Request {
        limits: Limits::NONE,
        sets: selected
            .iter()
            .map(|&option| SetRule {
                options: vec![option],
                at_least: 1,
            })
            .collect(),
        move_points: false,
        threads: 1,
    };
    let mut sweep = ExactSweep::open(&engine, &pack, request, kind, order, &swaps, pruning)?;
    let device_reuse =
        matches!(kind, ScorerKind::Cuda) && diagnostics.reuse && diagnostics.cuda_reuse;
    let score_reuse = if matches!(kind, ScorerKind::Cpu) {
        diagnostics.reuse
    } else {
        device_reuse
    };
    sweep.set_reuse_enabled(score_reuse);
    sweep.enable_profiling(diagnostics.profile)?;
    let required_sets: Vec<_> = selected
        .iter()
        .map(|option| json!({"key":option.key,"minimum_tier":option.pieces}))
        .collect();
    fs::create_dir_all(&output)?;
    let checkpoint = output.join("CHECKPOINT.json");
    let program_sha256 = program_hash(bundle, kind)?;
    let run_owner = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(output.join("RUNNING.lock"))?;
    run_owner.try_lock()?;
    let run = (|| -> Result<(), Box<dyn Error>> {
        let mut state = if checkpoint.is_file() {
            let previous: RunCheckpoint = serde_json::from_slice(&fs::read(&checkpoint)?)?;
            if previous.program_sha256 != program_sha256 {
                return Err("cannot resume under a different scoring binary".into());
            }
            previous
        } else {
            RunCheckpoint {
                program_sha256,
                progress: sweep.start(),
            }
        };
        sweep.validate(&state.progress)?;
        let startup_seconds = invocation_started.elapsed().as_secs_f64();
        let started = Instant::now();
        let from = state.progress.completed;
        let end = from.saturating_add(budget).min(state.progress.planned);
        let mut last_saved = started;
        let mut checkpoint_io_seconds = 0.0;
        let cancel_checks = std::cell::Cell::new(0u64);
        while state.progress.completed < end {
            let batch = (end - state.progress.completed).min(diagnostics.batch as u64) as usize;
            let before = (diagnostics.cancel_after_checks.is_some()
                || diagnostics.cancel_file.is_some())
            .then(|| state.progress.clone());
            if let Err(error) = sweep.advance_controlled(&mut state.progress, batch, &|| {
                let injected = diagnostics.cancel_after_checks.is_some_and(|limit| {
                    cancel_checks.set(cancel_checks.get() + 1);
                    cancel_checks.get() >= limit
                });
                injected
                    || diagnostics
                        .cancel_file
                        .as_ref()
                        .is_some_and(|path| path.exists())
            }) {
                write_atomic(
                    &output.join("ABORTED.json"),
                    &json!({
                        "schema":"nioh3-standalone-aborted/v1", "accepted":false,
                        "error":error, "ranking_published_this_invocation":false,
                        "progress":state.progress, "cancel_checks_observed":cancel_checks.get(),
                        "cancelled_batch_left_checkpoint_unchanged":before.as_ref().map(|saved| *saved == state.progress),
                        "performance":sweep.performance(),
                    }),
                )?;
                return Err(error.into());
            }
            if last_saved.elapsed() >= Duration::from_secs(3) {
                let checkpoint_started = Instant::now();
                write_atomic(&checkpoint, &state)?;
                checkpoint_io_seconds += checkpoint_started.elapsed().as_secs_f64();
                eprintln!(
                    "Examined {} of {} assignments",
                    state.progress.completed, state.progress.planned
                );
                last_saved = Instant::now();
            }
        }
        let sweep_seconds = started.elapsed().as_secs_f64();
        let finalized = Instant::now();
        let checkpoint_started = Instant::now();
        write_atomic(&checkpoint, &state)?;
        checkpoint_io_seconds += checkpoint_started.elapsed().as_secs_f64();
        let best = sweep.best(&state.progress)?;
        let mut work = sweep.work()?;
        let mut ranked = Vec::with_capacity(best.len());
        for score in best {
            let assignments = assignments(&score.candidate, &pack, &input);
            ranked.push(json!({
                "score_bits":score.bits,"damage":f32::from_bits(score.bits),
                "assignments":assignments,
                "move_previous_weapon_to_slot":sweep.displaced_weapon_slot(&score.candidate),
            }));
        }
        let verified_top = if let Some(first) = sweep.best(&state.progress)?.first() {
            let (words, hits) = engine.native_words(&first.candidate, &pack, &mut work)?;
            Some(json!({
                "components":engine.damage_details(&first.candidate, &pack, &mut sweep.work()?),
                "native_outfit_words":words,"native_hit_words":hits,
            }))
        } else {
            None
        };
        let mut report = json!({"complete":state.progress.complete(),
            "save_source_sha256":input.source_sha256,"context_sha256":capture.digest(),
            "source_scope":engine.source_model_scope(),"scorer":kind,"progress":state.progress,"owned_spare_weapons":swaps.len(),
            "required_sets":required_sets,"ranked":ranked,"top_case":verified_top,
            "from_this_invocation":from,
            "inventory_file_sha256":inventory_file_sha256,
            "bundle_input_sha256":engine.bundle_input_digest(),
            "settings":{"batch":diagnostics.batch,"reuse":score_reuse,
                "profile":diagnostics.profile,"order":order,"set_pruning":pruning},
            "performance":sweep.performance(),
            "startup_seconds":startup_seconds,"sweep_seconds":sweep_seconds,
            "checkpoint_io_seconds":checkpoint_io_seconds,
            "finalize_seconds":finalized.elapsed().as_secs_f64(),
            "elapsed_seconds":started.elapsed().as_secs_f64()});
        if !state.progress.unanswered_samples.is_empty() {
            let cases: Vec<_> = state
                .progress
                .unanswered_samples
                .iter()
                .map(|&position| {
                    let candidate = sweep
                        .candidate_at(position)
                        .expect("validated checkpoint position");
                    let placement = assignments(&candidate, &pack, &input);
                    match engine.native_words(&candidate, &pack, &mut work) {
                        Ok((words, hits)) => json!({"case_id":position, "assignments":placement,
                        "state":"prepared","native_outfit_words":words,"native_hit_words":hits}),
                        Err(reason) => json!({"case_id":position,"assignments":placement,
                        "state":"refused","reason":reason}),
                    }
                })
                .collect();
            write_atomic(
                &output.join("UNANSWERED_CASES.json"),
                &json!({
                    "schema":"nioh3-standalone-unanswered/v1","rows":cases,
                    "context_sha256":capture.digest(),
                }),
            )?;
        }
        // Never include this verification work in performance measurements.
        if diagnostics.audit {
            if diagnostics.native_inputs {
                let equipped = Candidate {
                    attrs: engine.start_attrs,
                    items: [None; nioh3_loadout::engine::SLOTS],
                };
                let details = engine.damage_details(&equipped, &pack, &mut work);
                let (outfit, contacts) = engine.native_words(&equipped, &pack, &mut work)?;
                write_atomic(
                    &output.join("EQUIPPED.json"),
                    &json!({"top_case":{
                        "components":details,"native_outfit_words":outfit,"native_hit_words":contacts
                    }}),
                )?;
            }
            let audit_started = Instant::now();
            let trace_path = output.join("AUDIT_ROWS.jsonl");
            let mut writer = BufWriter::new(fs::File::create(&trace_path)?);
            let mut native_inputs = diagnostics
                .native_inputs
                .then(|| fs::File::create(output.join("NATIVE_INPUTS.jsonl")).map(BufWriter::new))
                .transpose()?;
            let mut audit = sweep.audit_session(kind == ScorerKind::Cuda)?;
            audit.set_device_reuse_enabled(device_reuse);
            let mut compared = 0;
            let mut answered = 0;
            let mut outside_limits = 0;
            let mut unanswered = 0;
            while compared < state.progress.completed {
                let batch = audit.batch(
                    compared,
                    (state.progress.completed - compared).min(diagnostics.batch as u64) as usize,
                )?;
                if batch.compared == 0 {
                    return Err("audit made no progress".into());
                }
                compared += batch.compared;
                answered += batch.answered;
                outside_limits += batch.outside_limits;
                unanswered += batch.unanswered;
                for row in batch.rows {
                    if let Some(file) = native_inputs.as_mut() {
                        if let Some(expected) = &row.cpu_components {
                            let candidate = Candidate {
                                attrs: row.attrs,
                                items: row.items,
                            };
                            let details = engine.damage_details(&candidate, &pack, &mut work);
                            let (outfit, contacts) =
                                engine.native_words(&candidate, &pack, &mut work)?;
                            serde_json::to_writer(
                                &mut *file,
                                &json!({
                                    "ordinal": row.ordinal, "outfit": outfit, "contacts": contacts,
                                    "expected": expected, "details": details,
                                }),
                            )?;
                            file.write_all(b"\n")?;
                        }
                    }
                    serde_json::to_writer(&mut writer, &row)?;
                    writer.write_all(b"\n")?;
                }
            }
            writer.flush()?;
            if let Some(file) = native_inputs.as_mut() {
                file.flush()?;
            }
            drop(writer);
            if compared != state.progress.completed
                || answered != state.progress.answered
                || outside_limits != state.progress.outside_limits
                || unanswered != state.progress.refused + state.progress.no_path
            {
                return Err("audit coverage disagrees with timed search".into());
            }
            report["audit"] = json!({
                "schema":"nioh3-prepared-search-audit/v1",
                "passed":unanswered == 0,
                "complete_domain":state.progress.complete(),
                "compared":compared,"answered":answered,"outside_limits":outside_limits,
                "unanswered":unanswered,"compared_cuda":kind == ScorerKind::Cuda,
                "device_reuse":device_reuse,
                "trace_sha256":sha256(&fs::read(trace_path)?),
                "seconds_excluded_from_benchmark":audit_started.elapsed().as_secs_f64(),
                "checks":["reference_vs_shared_outcome","reference_vs_shared_ordered_inputs",
                    "cpu_certificate_vs_direct","strict_native_component_bits"],
                "original_game_MAIN":"requires separate retained-host witness replay",
            });
        }
        if state.progress.no_path != 0 {
            eprintln!("CPU scoring diagnostics: {:?}", engine.cpu_diagnostics());
        }
        write_atomic(&output.join("SEARCH_RESULT.json"), &report)?;
        println!(
            "{}",
            json!({"covered":state.progress.completed,"outfits":state.progress.requested,
            "planned":state.progress.planned,"excluded_by_set_bound":state.progress.excluded_by_set_bound,
            "complete":state.progress.complete(),"output":output,"best":report["ranked"].get(0)})
        );
        Ok(())
    })();
    if let Err(error) = &run {
        // No ranking accompanies a cancelled/failed invocation. Periodically
        // saved checkpoints describe only earlier fully committed batches.
        if !output.join("ABORTED.json").exists() {
            let _ = write_atomic(
                &output.join("ABORTED.json"),
                &json!({
                "schema":"nioh3-standalone-aborted/v1", "accepted":false,
                "error":error.to_string(), "ranking_published_this_invocation":false,
                "checkpoint_policy":"last fully committed periodically saved prefix",
                }),
            );
        }
    }
    drop(run_owner);
    run
}
