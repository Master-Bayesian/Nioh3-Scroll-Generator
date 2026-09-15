//! Development-only batch preview for the cross-language migration gate.

use nioh3_data::{load_enemy_resources, parse_enemy_state_tables, EnemyResources};
use nioh3_domain::{
    context::{resolve_context, ResolvedContext},
    enemy::{EnemyError, MissionVariant, Possession},
    preview::generate_enemy_preview,
};
use serde_json::{json, Value};
use std::{error::Error, io, path::Path};

fn preview(request: &Value, resources: &EnemyResources) -> Result<Value, Box<dyn Error>> {
    let seed = u32::try_from(request["seed"].as_u64().ok_or("seed must be uint32")?)?;
    let playthrough = u8::try_from(
        request["playthrough"]
            .as_u64()
            .ok_or("invalid progression")?,
    )?;
    let variant = match request["variant"].as_str() {
        Some("solo") => MissionVariant::Solo,
        Some("expedition") => MissionVariant::Expedition,
        _ => return Err("invalid variant".into()),
    };
    let result = match generate_enemy_preview(
        seed,
        playthrough,
        variant,
        &resources.roster,
        &resources.context,
        &resources.states,
    ) {
        Ok(result) => result,
        Err(error @ EnemyError::Unsupported(_)) => {
            let context = resolve_context(seed, &resources.roster, &resources.context)?;
            if context.selector != 0 {
                return Ok(
                    json!({"status":"unsupported_selector", "context":context_json(&context)}),
                );
            }
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    };
    let context_json = context_json(&result.context);
    let roster = result.roster;
    let wraith = result.wraith;
    let waves: Vec<Vec<Value>> = roster
        .waves
        .iter()
        .map(|wave| {
            wave.iter().map(|x| json!({
        "wave_index":x.wave_index, "position":x.position,
        "spawn":x.native_spawn_key, "lookup":x.lookup_key, "role":x.role,
        "row":x.source_row_index, "selector":x.selector_class, "scratch":x.scratch_rule_key,
    })).collect()
        })
        .collect();
    let states: Vec<&str> = wraith
        .states
        .iter()
        .map(|state| match state {
            Possession::Yes => "yes",
            Possession::No => "no",
            Possession::Unknown => "unknown",
        })
        .collect();
    let trials: Vec<Value> = wraith
        .trials
        .iter()
        .map(|t| {
            json!({
                "selector":t.selector, "spawn":t.spawn, "ticket":t.ticket,
                "state":t.state, "draw":t.draw, "accepted":t.accepted,
            })
        })
        .collect();
    Ok(json!({
        "status":"ok", "context":context_json,
        "roster": {"terrain":roster.terrain, "branch_class":roster.branch_class,
            "state_after_roster":roster.state_after_roster,
            "parent_draws":roster.parent_draws, "waves":waves},
        "wraith": {"status":if wraith.exact {"exact"} else {"unknown"},
            "states":states, "source_entry_state":wraith.source_entry_state,
            "source_entry_draw":wraith.source_entry_draw, "final_state":wraith.final_state,
            "final_draws":wraith.final_draws, "trials":trials},
    }))
}

fn context_json(context: &ResolvedContext) -> Value {
    json!({
        "mode": context.auxiliary_mode,
        "mode_branch": context.mode_branch,
        "mode_draws": context.mode_draws,
        "mode_row_index": context.mode_row_index,
        "terrain_row_index": context.terrain_row_index,
        "terrain_value": context.terrain_value,
        "used_filtered_pool": context.used_filtered_pool,
        "selector": context.selector,
        "flags": context.flags,
        "descriptor_draws": context.descriptor_draws,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).ok_or("expected product data directory")?;
    let mut resources = load_enemy_resources(Path::new(root))?;
    if let Some(state_path) = args.get(2) {
        resources.states = parse_enemy_state_tables(&std::fs::read(state_path)?)?;
    }
    let requests: Vec<Value> = serde_json::from_reader(io::stdin().lock())?;
    let answers: Vec<Value> = requests
        .iter()
        .map(|request| {
            preview(request, &resources)
                .unwrap_or_else(|e| json!({"status":"error","message":e.to_string()}))
        })
        .collect();
    serde_json::to_writer(io::stdout().lock(), &answers)?;
    Ok(())
}
