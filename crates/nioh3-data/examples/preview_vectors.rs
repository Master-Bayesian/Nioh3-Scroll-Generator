//! Development parity emitter for the M2.3a offline NG3 preview slice.
//!
//! Reads the shipped resources through the real production adapters
//! (`nioh3_data::load_preview_resources` and `nioh3_data::load_effect_resource`)
//! and prints deterministic preview rows that the Python gate reproduces from
//! the retained reference implementation. This example is not a product CLI.
//!
//! Floats travel as their exact IEEE bit patterns so a formatting difference
//! can never look like a numerical difference.
//!
//! Usage: `cargo run --example preview_vectors -- [data_root]`

use std::{env, path::Path};

use nioh3_data::{load_effect_resource, load_preview_resources};
use nioh3_domain::auxiliary::SpecialRuleEntry;
use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::enemy::{MissionVariant, Possession};
use nioh3_domain::preview::{
    compose_ng3_preview, effect_previews, CurseConditional, EnemyStatePreview,
    OccurrenceAvailability, PreviewTables,
};
use nioh3_domain::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
};

/// Level used by every emitted effect preview; the reference default is 180.
const LEVEL: u16 = 180;
/// Broad sweep size; both sides derive the same multiplicative sequence.
const SWEEP_SEEDS: u32 = 96;
const SWEEP_MULTIPLIER: u32 = 2_654_435_761;
/// Progressions the auxiliary composition is compared over.
const PLAYTHROUGH_SWEEP: [u8; 5] = [1, 2, 3, 4, 5];

/// Fixed edge and native-fixture seeds, in the order both sides emit them.
fn seed_sweep() -> Vec<u32> {
    let mut seeds = vec![
        0,
        1,
        2,
        2_965,
        240_348_265,
        6_096_970,
        74_063_692,
        82_212_268,
        183_696_634,
        241_719_428,
        // Nonzero descriptor selector: the auxiliary half still composes and
        // the enemy-state half falls back exactly like the shipped payload.
        53_432_590,
        0x0FFF_FFFF,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFE,
        0xFFFF_FFFF,
    ];
    for index in 0..SWEEP_SEEDS {
        seeds.push(index.wrapping_mul(SWEEP_MULTIPLIER));
    }
    seeds
}

/// Effect-preview seeds: the same fixed set plus a short deterministic stride.
fn effect_seed_sweep() -> Vec<u32> {
    let mut seeds = vec![0, 1, 2, 82_212_268, 183_696_634, 241_719_428, 0xFFFF_FFFF];
    for index in 0..16u32 {
        seeds.push(index.wrapping_mul(SWEEP_MULTIPLIER));
    }
    seeds
}

fn f32_bits(value: f32) -> String {
    format!("{:08X}", value.to_bits())
}

fn f64_bits(value: f64) -> String {
    format!("{:016X}", value.to_bits())
}

fn optional_f32_bits(value: Option<f32>) -> String {
    match value {
        Some(value) => f32_bits(value),
        None => "-".to_string(),
    }
}

fn optional_f64_bits(value: Option<f64>) -> String {
    match value {
        Some(value) => f64_bits(value),
        None => "-".to_string(),
    }
}

fn optional_text(value: Option<&str>) -> String {
    value.unwrap_or("-").to_string()
}

fn optional_usize(value: Option<usize>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

fn optional_u32(value: Option<u32>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "-".to_string(),
    }
}

fn possession_name(possessed: Possession) -> &'static str {
    match possessed {
        Possession::Yes => "yes",
        Possession::No => "no",
        Possession::Unknown => "unknown",
    }
}

fn availability_name(availability: OccurrenceAvailability) -> &'static str {
    match availability {
        OccurrenceAvailability::Base => "base",
        OccurrenceAvailability::ExpeditionOnly => "expedition_only",
    }
}

fn curse_name(curse: CurseConditional) -> &'static str {
    match curse {
        CurseConditional::Guaranteed => "guaranteed",
        CurseConditional::Never => "never",
        CurseConditional::Unknown => "unknown",
    }
}

fn describe_rule(seed: u32, playthrough: u8, slot: usize, entry: &SpecialRuleEntry) -> String {
    format!(
        "rule\t{seed}\t{playthrough}\t{slot}\t{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        entry.key,
        entry.row_index,
        optional_f32_bits(entry.raw_value),
        optional_f64_bits(entry.display_value),
        optional_text(entry.display_unit),
        optional_text(entry.display_grade),
        optional_usize(entry.value_source_offset),
        optional_text(entry.qualifier_kind),
        optional_u32(entry.qualifier_key)
    )
}

fn describe_state(seed: u32, state: &EnemyStatePreview) -> Vec<String> {
    let variant = match state.variant {
        MissionVariant::Solo => "solo",
        MissionVariant::Expedition => "expedition",
    };
    let mut lines = Vec::new();
    for occurrence in &state.occurrences {
        lines.push(format!(
            "state\t{seed}\t{variant}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            occurrence.wave_index,
            occurrence.position,
            occurrence.lookup_key,
            occurrence.role,
            occurrence.source_row_index,
            availability_name(occurrence.availability),
            occurrence.native_spawn_key,
            possession_name(occurrence.possessed),
            curse_name(occurrence.curse_if_fresh_null_source_selector_runs)
        ));
    }
    lines.push(format!(
        "statesummary\t{seed}\t{variant}\t{}\t{}\t{}\t{}\t{}\t{}",
        match state.terrain {
            Some(value) => value.to_string(),
            None => "-".to_string(),
        },
        state.occurrences.len(),
        state.possessed_complete,
        state.missing_inputs.len(),
        state.missing_inputs.join(" | "),
        state.curse_scope
    ));
    lines
}

/// Local error alias: domain errors carry structured data instead of `Display`.
type EmitResult<T> = Result<T, String>;

fn run() -> EmitResult<()> {
    let root = env::args()
        .nth(1)
        .unwrap_or_else(|| "../../nioh3_scroll_editor/data".to_string());
    let data_root = Path::new(&root);
    let resources = load_preview_resources(data_root).map_err(|error| error.to_string())?;
    let tables = PreviewTables {
        roster: &resources.roster,
        context: &resources.context,
        rules: &resources.rules,
        states: &resources.states,
    };

    let mut lines = Vec::new();
    for seed in seed_sweep() {
        for playthrough in PLAYTHROUGH_SWEEP {
            if playthrough == 3 {
                let composition = compose_ng3_preview(seed, playthrough, &tables)
                    .map_err(|error| error.to_string())?;
                lines.extend(describe_auxiliary(
                    seed,
                    playthrough,
                    &composition.auxiliary,
                ));
                lines.push(format!(
                    "capacity\t{seed}\t{}",
                    composition.initial_challenge_capacity
                ));
                for state in &composition.enemy_states {
                    lines.extend(describe_state(seed, state));
                }
                if compose_ng3_preview(seed, 4, &tables).is_ok() {
                    return Err(format!(
                        "seed {seed} unexpectedly produced an NG3 preview for playthrough 4"
                    ));
                }
                continue;
            }
            // Other supported progressions carry the auxiliary half only; the
            // shipped payload emits `enemy_states` as null for them.
            let auxiliary =
                nioh3_domain::preview::compose_auxiliary_preview(seed, playthrough, &tables)
                    .map_err(|error| error.to_string())?;
            lines.extend(describe_auxiliary(seed, playthrough, &auxiliary));
        }
    }

    // Effect payload mapping: the rarity-dependent half of the same payload.
    let resource = load_effect_resource(data_root).map_err(|error| error.to_string())?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;
    let stage_one_map = &resource.grace_maps[0];
    let grace_map = &resource.grace_maps[1];
    for seed in effect_seed_sweep() {
        let rarity3 = generate_ng3_rarity3_effect_sequence(&index, seed, LEVEL)
            .map_err(|error| format!("{error:?}"))?;
        let stage_one =
            generate_ng3_rarity4_stage_one_effect_sequence(&index, stage_one_map, seed, LEVEL)
                .map_err(|error| format!("{error:?}"))?;
        let rarity5 = generate_ng3_rarity5_effect_sequence(&index, grace_map, seed, LEVEL)
            .map_err(|error| format!("{error:?}"))?;
        for (path, record) in [
            ("r3", &rarity3),
            ("r4_stage_one", &stage_one),
            ("r5", &rarity5),
        ] {
            for effect in effect_previews(record) {
                lines.push(format!(
                    "effect\t{path}\t{seed}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    effect.slot,
                    effect.effect_id,
                    effect.value,
                    effect.metadata,
                    effect.prefix,
                    effect.tail_0,
                    effect.tail_1,
                    match effect.roll_percent {
                        Some(value) => value.to_string(),
                        None => "-".to_string(),
                    }
                ));
            }
        }
    }

    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn describe_auxiliary(
    seed: u32,
    playthrough: u8,
    auxiliary: &nioh3_domain::preview::AuxiliaryPreview,
) -> Vec<String> {
    let flags = auxiliary
        .descriptor_flags
        .iter()
        .map(|flag| if *flag { "1" } else { "0" })
        .collect::<Vec<_>>()
        .join(",");
    let display_keys = auxiliary
        .terrain
        .display_effect_keys
        .iter()
        .map(|key| format!("{key:04X}"))
        .collect::<Vec<_>>()
        .join(",");
    let rule_keys = auxiliary
        .special_rules
        .keys
        .iter()
        .map(|key| format!("{key:04X}"))
        .collect::<Vec<_>>()
        .join(",");
    let components = [
        format!("{:02X}", auxiliary.mode),
        auxiliary.mode_branch.to_string(),
        auxiliary.terrain.value.to_string(),
        auxiliary.terrain.selected_row_index.to_string(),
        auxiliary.terrain.used_filtered_pool.to_string(),
        auxiliary.terrain.scoped_seed.to_string(),
        auxiliary.descriptor_selector.to_string(),
        flags,
        display_keys,
        rule_keys,
        auxiliary.special_rules.target_budget.to_string(),
        auxiliary.special_rules.random_draws.to_string(),
        auxiliary.special_rules.scoped_seed.to_string(),
    ];
    let mut lines = vec![format!(
        "component\t{seed}\t{playthrough}\t{}",
        components.join("\t")
    )];
    for (wave, group) in auxiliary.enemy_groups.iter().enumerate() {
        lines.push(format!(
            "group\t{seed}\t{playthrough}\t{wave}\t{}\t{}",
            f32_bits(group.source_budget),
            group.entries.len()
        ));
        for (position, entry) in group.entries.iter().enumerate() {
            lines.push(format!(
                "entry\t{seed}\t{playthrough}\t{wave}\t{position}\t{}\t{:08X}\t{}\t{:04X}",
                entry.row_index, entry.lookup_key, entry.role, entry.scratch_rule_key
            ));
        }
    }
    for (slot, entry) in auxiliary.special_rules.entries.iter().enumerate() {
        lines.push(describe_rule(seed, playthrough, slot, entry));
    }
    lines
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PREVIEW_VECTORS_ERROR: {error}");
        std::process::exit(1);
    }
}
