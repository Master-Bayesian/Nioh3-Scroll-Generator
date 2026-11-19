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
    io::Write,
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
    let host = bundle.join(main);
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
        hash.update(fs::read(bundle.join(name))?);
    }
    Ok(format!("{:x}", hash.finalize()))
}

const USAGE: &str = concat!(
    "standalone_search <bundle> <inventory.json> <result-dir> <cpu|cuda> ",
    "[outfits-this-run] [linear|spread] --set <id> <minimum-tier> ",
    "[--set <id> <minimum-tier> ...] [--verify-unpruned]"
);

fn parse_sets(
    mut flags: &[std::ffi::OsString],
) -> Result<(Vec<SetOption>, SetPruning), Box<dyn Error>> {
    let mut selected = Vec::new();
    let mut pruning = SetPruning::Proven;
    while !flags.is_empty() {
        match flags {
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
            _ => return Err(USAGE.into()),
        }
    }
    if selected.is_empty() {
        return Err(format!("Choose at least one set. {USAGE}").into());
    }
    Ok((selected, pruning))
}

fn main() -> Result<(), Box<dyn Error>> {
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
    let (selected, pruning) = parse_sets(flags)?;
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
    let input: Inventory = serde_json::from_slice(&fs::read(inventory)?)?;
    if input.schema != "nioh3-current-whole-backpack/v1"
        || !input.only_reads_save
        || input.searchable_items != input.records.len()
    {
        return Err("expected a saved inventory export".into());
    }
    let capture = LoadoutContext::load(&bundle.join("context"))?;
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
        let started = Instant::now();
        let from = state.progress.completed;
        let end = from.saturating_add(budget).min(state.progress.planned);
        let mut last_saved = started;
        while state.progress.completed < end {
            let batch = (end - state.progress.completed).min(2048) as usize;
            sweep.advance(&mut state.progress, batch)?;
            if last_saved.elapsed() >= Duration::from_secs(3) {
                write_atomic(&checkpoint, &state)?;
                eprintln!(
                    "Examined {} of {} assignments",
                    state.progress.completed, state.progress.planned
                );
                last_saved = Instant::now();
            }
        }
        write_atomic(&checkpoint, &state)?;
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
        let report = json!({"complete":state.progress.complete(),
            "save_source_sha256":input.source_sha256,"context_sha256":capture.digest(),
            "source_scope":engine.source_model_scope(),"scorer":kind,"progress":state.progress,"owned_spare_weapons":swaps.len(),
            "required_sets":required_sets,"ranked":ranked,"top_case":verified_top,
            "from_this_invocation":from,
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
        write_atomic(&output.join("SEARCH_RESULT.json"), &report)?;
        println!(
            "{}",
            json!({"covered":state.progress.completed,"outfits":state.progress.requested,
            "planned":state.progress.planned,"excluded_by_set_bound":state.progress.excluded_by_set_bound,
            "complete":state.progress.complete(),"output":output,"best":report["ranked"].get(0)})
        );
        Ok(())
    })();
    drop(run_owner);
    run
}
