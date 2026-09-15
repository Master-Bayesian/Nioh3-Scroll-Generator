//! Development parity emitter for the M2.1 NG3 effect-sequence slice.
//!
//! Reads the shipped resource through the real production adapter
//! (`nioh3_data::load_effect_resource`) and prints deterministic sequence rows
//! that the Python gate reproduces through the retained reference
//! implementation. This example is not a product CLI.
//!
//! Usage: `cargo run --example effect_sequence_vectors -- [data_root]`

use std::{env, path::Path};

use nioh3_data::load_effect_resource;
use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::record::ScrollRecord;
use nioh3_domain::sequence::{
    generate_ng3_rarity3_effect_sequence, generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence, NG3_RECORD_TYPE, RARITY_DIVINE, RARITY_FINALIZABLE,
    RARITY_GROWING,
};

/// Level used by every emitted sequence; the reference default is 180.
const LEVEL: u16 = 180;
/// Broader sweep size; both sides derive the same multiplicative sequence.
const SWEEP_SEEDS: u32 = 96;
const SWEEP_MULTIPLIER: u32 = 2_654_435_761;
/// Levels swept by the second block.
///
/// `resolved_base_value` clamps the level to 500 through
/// `MAX_CURVE_LEVEL` before reading the 501-row curve, so every level above
/// 500 (`700`, `65535`) must reduce to the level-500 result. The reference
/// rejects nothing in this range; both sides report the level verbatim in the
/// row while only the resolved values follow the clamped curve.
const LEVEL_SWEEP: [u16; 9] = [1, 30, 90, 150, 180, 300, 500, 700, 65_535];
/// Level-sweep stride indices used to derive two extra seeds.
const LEVEL_SWEEP_STRIDES: [u32; 2] = [5, 97];

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

/// Fixed edge seeds plus two stride seeds for the level sweep.
fn level_seed_sweep() -> Vec<u32> {
    let mut seeds = vec![0, 1, 2, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF];
    for index in LEVEL_SWEEP_STRIDES {
        seeds.push(index.wrapping_mul(SWEEP_MULTIPLIER));
    }
    seeds
}

/// One sequence row: identity, promotion, draw accounting and every effect.
fn describe(path: &str, seed: u32, record: &ScrollRecord) -> String {
    let effects = record
        .effects
        .iter()
        .map(|effect| {
            format!(
                "{}:{}:{:04X}:{}:{:02X}:{:02X}:{}:{}:{:04X}",
                effect.slot,
                effect.source_index,
                effect.effect_id,
                effect.roll_percent,
                effect.category_and_flags,
                effect.effect_flags,
                effect.candidate_count,
                effect.resolved_value,
                effect.prefix_word
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let promoted = record
        .promoted_source_indexes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "seq\t{path}\t{seed}\t{}\t{:04X}\t{}\t{}\t{}\t{}\t{}\t{:08X}\t{effects}",
        record.level,
        record.record_type,
        record.rarity,
        record.playthrough,
        record.terminal_is_special,
        promoted,
        record.random_draws,
        record.final_rng_state
    )
}

/// Local error alias: domain errors carry structured data instead of `Display`.
type EmitResult<T> = Result<T, String>;

fn run() -> EmitResult<()> {
    let root = env::args()
        .nth(1)
        .unwrap_or_else(|| "../../nioh3_scroll_editor/data".to_string());
    let resource = load_effect_resource(Path::new(&root)).map_err(|error| error.to_string())?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;

    let mut lines = Vec::new();
    for rarity in [RARITY_GROWING, RARITY_FINALIZABLE, RARITY_DIVINE] {
        let capacities = index
            .category_capacities(NG3_RECORD_TYPE, rarity)
            .map_err(|error| format!("{error:?}"))?;
        let values = capacities
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!(
            "capacities\t{:04X}\t{rarity}\t{values}",
            NG3_RECORD_TYPE
        ));
    }

    let stage_one_map = &resource.grace_maps[0];
    let grace_map = &resource.grace_maps[1];
    for seed in seed_sweep() {
        let rarity3 = generate_ng3_rarity3_effect_sequence(&index, seed, LEVEL)
            .map_err(|error| format!("{error:?}"))?;
        lines.push(describe("r3", seed, &rarity3));
        let stage_one =
            generate_ng3_rarity4_stage_one_effect_sequence(&index, stage_one_map, seed, LEVEL)
                .map_err(|error| format!("{error:?}"))?;
        lines.push(describe("r4_stage_one", seed, &stage_one));
        let rarity5 = generate_ng3_rarity5_effect_sequence(&index, grace_map, seed, LEVEL)
            .map_err(|error| format!("{error:?}"))?;
        lines.push(describe("r5", seed, &rarity5));
    }

    // Level block: outer loop over levels, then seeds, then the three paths.
    // `tests/migration/test_effect_parity.py` builds the reference rows in the
    // same order, so a reordered sweep fails the comparison instead of passing
    // silently.
    for level in LEVEL_SWEEP {
        for seed in level_seed_sweep() {
            let rarity3 = generate_ng3_rarity3_effect_sequence(&index, seed, level)
                .map_err(|error| format!("{error:?}"))?;
            lines.push(describe("r3", seed, &rarity3));
            let stage_one =
                generate_ng3_rarity4_stage_one_effect_sequence(&index, stage_one_map, seed, level)
                    .map_err(|error| format!("{error:?}"))?;
            lines.push(describe("r4_stage_one", seed, &stage_one));
            let rarity5 = generate_ng3_rarity5_effect_sequence(&index, grace_map, seed, level)
                .map_err(|error| format!("{error:?}"))?;
            lines.push(describe("r5", seed, &rarity5));
        }
    }

    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("EFFECT_SEQUENCE_VECTORS_ERROR: {error}");
        std::process::exit(1);
    }
}
