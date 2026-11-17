//! Offline research only: evaluate the guarded descriptor-selector branch.
//! Seeded predictions remain distinct from independent native observations.
use std::error::Error;
use std::fs;
use std::path::Path;

use nioh3_data::load_preview_resources_for_file_version;
use nioh3_domain::context::{optional_threshold, resolve_context};
use nioh3_domain::enemy::{MissionVariant, Possession, RosterInput};
use nioh3_domain::preview::{compose_enemy_state_preview, PreviewTables};
use nioh3_domain::roster::generate_roster;
use nioh3_domain::wraith::{eligible, generate_wraith};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn possession_name(value: Possession) -> &'static str {
    match value {
        Possession::Yes => "yes",
        Possession::No => "no",
        Possession::Unknown => "unknown",
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err(
            "usage: selector_wraith_audit DATA_ROOT A.B.C.D OUTPUT_JSON [SEEDS_JSON]".into(),
        );
    }
    let components = args[1]
        .split('.')
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()?;
    let [a, b, c, d] = components.as_slice() else {
        return Err("game version must contain exactly four numeric components".into());
    };
    let data_root = Path::new(&args[0]);
    let resources = load_preview_resources_for_file_version(data_root, (*a, *b, *c, *d))?;
    let tables = PreviewTables {
        roster: &resources.roster,
        context: &resources.context,
        rules: &resources.rules,
        states: &resources.states,
    };
    let seeds = if let Some(path) = args.get(3) {
        let requested: Vec<u32> = serde_json::from_str(&fs::read_to_string(path)?)?;
        if requested.is_empty() || requested.len() > 256 {
            return Err("explicit seed list must contain 1..256 uint32 values".into());
        }
        requested
    } else {
        let mut selected = vec![141974638, 86872488, 156062997];
        for seed in 100_000_000..100_004_096 {
            if resolve_context(seed, tables.roster, tables.context)?.selector != 0 {
                selected.push(seed);
                if selected.len() == 27 {
                    break;
                }
            }
        }
        selected
    };
    let mut cases: Vec<Value> = Vec::new();
    for seed in seeds {
        let context = resolve_context(seed, tables.roster, tables.context)?;
        let threshold = if context.mode_branch == 0 {
            optional_threshold(tables.context, 0xCEFC)?
        } else {
            0
        };
        for (variant, name) in [
            (MissionVariant::Solo, "solo"),
            (MissionVariant::Expedition, "expedition"),
        ] {
            let roster = generate_roster(
                RosterInput {
                    seed,
                    playthrough: 3,
                    variant,
                    auxiliary_mode: context.auxiliary_mode,
                    terrain_row_index: context.terrain_row_index,
                    selector: context.selector,
                    flags: context.flags,
                },
                tables.roster,
                threshold,
            )?;
            let prediction = generate_wraith(&roster, tables.states);
            let production = compose_enemy_state_preview(seed, 3, variant, &tables)?;
            let occurrences: Vec<Value> = roster
                .occurrences()
                .zip(&prediction.states)
                .map(|(item, state)| {
                    json!({
                        "wave_index": item.wave_index, "position": item.position,
                        "spawn_key": item.native_spawn_key, "lookup_key": item.lookup_key,
                        "role": item.role, "source_row_index": item.source_row_index,
                        "occurrence_selector_class": item.selector_class,
                        "eligibility": format!("{:?}", tables.states.eligibility.get(&item.lookup_key)),
                        "source_eligible": eligible(tables.states, item.lookup_key).ok(),
                        "unverified_possessed_prediction": possession_name(*state),
                    })
                })
                .collect();
            let trials: Vec<Value> = prediction
                .trials
                .iter()
                .map(|trial| {
                    json!({"occurrence_selector_class":trial.selector,"spawn_key":trial.spawn,
                        "ticket":trial.ticket,"state":trial.state,"draw":trial.draw,
                        "accepted":trial.accepted})
                })
                .collect();
            cases.push(json!({
                "seed": seed, "playthrough":3,"variant":name,
                "descriptor_selector":context.selector,"descriptor_flags":context.flags,
                "auxiliary_mode":context.auxiliary_mode,"branch_class":context.mode_branch,
                "terrain":roster.terrain,"state_after_roster":roster.state_after_roster,
                "parent_draws_after_roster":roster.parent_draws,"occurrences":occurrences,
                "model_inputs_complete":prediction.exact,"missing_model_inputs":prediction.missing,
                "source_entry_state":prediction.source_entry_state,
                "source_entry_draw":prediction.source_entry_draw,
                "final_state":prediction.final_state,"final_draws":prediction.final_draws,
                "source_trials":trials,
                "production_occurrences":production.occurrences.len(),
                "production_possessed_complete":production.possessed_complete,
                "production_missing_inputs":production.missing_inputs,
                "evidence_state":if context.selector==0 {"existing_default_branch"} else {"prediction_only_awaiting_native_parity"},
            }));
        }
    }
    let state_bytes = fs::read(data_root.join(nioh3_data::ENEMY_STATE_TABLES_PATH))?;
    let output = json!({
        "schema":"nioh3-selector-wraith-offline-audit/v1",
        "game_resource_version":args[1],"captured_state_text_sha256":tables.states.text_sha256,
        "state_table_sha256":format!("{:x}",Sha256::digest(state_bytes)),
        "config_4543_hex":tables.states.config_4543.as_ref().map(|row| row.iter().map(|b| format!("{b:02x}")).collect::<String>()),
        "scope":"Offline predictions only; production support follows the version-bound native-parity policy. No process, game, debugger or save access.",
        "cases":cases,
    });
    let target = Path::new(&args[2]);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, serde_json::to_string_pretty(&output)? + "\n")?;
    println!("SELECTOR_WRAITH_RESEARCH_REPORT {}", target.display());
    Ok(())
}
