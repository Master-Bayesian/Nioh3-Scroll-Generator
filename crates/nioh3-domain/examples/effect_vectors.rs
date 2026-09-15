//! Development-only parity emitter for the M2.1 effect-table slice.
//!
//! Reads the shipped, header-prefixed resource tables from the data root and
//! writes deterministic TSV rows that the Python gate reproduces through the
//! retained reference implementation. This example is not a product CLI and is
//! the only place in the crate family that touches the filesystem for M2.1.
//!
//! Usage: `cargo run --example effect_vectors -- [data_root]`

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use nioh3_domain::effect::{
    EffectResourceBytes, EffectRow, EffectTableBytes, EffectTableIndex,
    CATEGORY_COUNT_MULTIPLIER_ROW_BYTES, CATEGORY_ROW_BYTES, EFFECT_GROUP_ROW_BYTES,
    EFFECT_ROW_BYTES, ITEM_ROW_BYTES, SCROLL_RECORD_TYPES,
};
use nioh3_domain::sequence::generate_challenge_attempt_count;

const TABLE_HEADER_BYTES: usize = 8;
const TABLE_HEADER_MAGIC: [u8; 4] = [0x00, 0x22, 0x04, 0x20];

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Strip the 8-byte table header and assert the embedded row count.
fn load_table(path: &Path, row_size: usize) -> Result<EffectTableBytes, String> {
    let blob = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if blob.len() < TABLE_HEADER_BYTES {
        return Err(format!("{}: truncated table header", path.display()));
    }
    if blob[..4] != TABLE_HEADER_MAGIC {
        return Err(format!("{}: unexpected table header magic", path.display()));
    }
    let declared = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) as usize;
    let rows = blob[TABLE_HEADER_BYTES..].to_vec();
    if rows.len() != declared * row_size {
        return Err(format!(
            "{}: header count {declared} does not match {} payload bytes at stride {row_size}",
            path.display(),
            rows.len()
        ));
    }
    EffectTableBytes::new("table", row_size, rows).map_err(|error| format!("{error:?}"))
}

fn seed_sweep() -> Vec<u32> {
    let mut seeds = vec![
        0u32,
        1,
        2,
        241_719_428,
        0x0FFF_FFFF,
        82_212_268,
        183_696_634,
        0xFFFF_FFFF,
        0xFFFF_FFFE,
        0x8000_0000,
    ];
    for index in 0..512u32 {
        seeds.push(index.wrapping_mul(2_654_435_761));
    }
    seeds
}

fn run() -> Result<(), String> {
    let root = env::args().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data"),
        PathBuf::from,
    );
    let resource = root
        .join("r4_finalizer")
        .join("pc_v2_00_02")
        .join("resource_v1");
    let table_path = |name: &str| resource.join("tables").join(format!("{name}.bin"));

    let built = EffectResourceBytes {
        schema: "nioh3-r4-finalizer-resource/v1".to_string(),
        item: load_table(&table_path("item"), ITEM_ROW_BYTES)?,
        effect_group: load_table(&table_path("effect_group"), EFFECT_GROUP_ROW_BYTES)?,
        category: load_table(&table_path("category"), CATEGORY_ROW_BYTES)?,
        category_count_multiplier: load_table(
            &table_path("category_count_multiplier"),
            CATEGORY_COUNT_MULTIPLIER_ROW_BYTES,
        )?,
        effect: load_table(&table_path("effect"), EFFECT_ROW_BYTES)?,
        level_curve: load_table(&table_path("level_curve"), 10)?,
        optional_multiplier: load_table(&table_path("optional_multiplier"), 32)?,
        rarity_roll: load_table(&table_path("rarity_roll"), 248)?,
        special_context: load_table(&table_path("special_context"), 48)?,
        bonus_curve_rows: Vec::new(),
        bonus_curve_index: Vec::new(),
        playthrough_progress: Vec::new(),
        grace_maps: Vec::new(),
    };
    let index = EffectTableIndex::from_resource(&built).map_err(|error| format!("{error:?}"))?;

    let mut lines = Vec::new();
    for (name, table) in [
        ("item", (ITEM_ROW_BYTES, built.item.row_count())),
        (
            "effect_group",
            (EFFECT_GROUP_ROW_BYTES, built.effect_group.row_count()),
        ),
        ("category", (CATEGORY_ROW_BYTES, built.category.row_count())),
        (
            "category_count_multiplier",
            (
                CATEGORY_COUNT_MULTIPLIER_ROW_BYTES,
                built.category_count_multiplier.row_count(),
            ),
        ),
        ("effect", (EFFECT_ROW_BYTES, built.effect.row_count())),
    ] {
        lines.push(format!("table\t{name}\t{}\t{}", table.0, table.1));
    }

    for record_type in SCROLL_RECORD_TYPES {
        let item = index
            .items_by_record_type
            .get(&record_type)
            .ok_or_else(|| format!("missing scroll item 0x{record_type:04X}"))?;
        lines.push(format!(
            "item\t{record_type}\t{}\t{}\t{}\t{}",
            item.field_154, item.field_15c, item.mode, item.candidate_item_flags
        ));
    }
    for group in index.groups_by_key.values() {
        lines.push(format!(
            "group\t{}\t{}\t{}\t{}",
            group.group_key, group.category_key, group.conflict_mask_0, group.conflict_mask_1
        ));
    }
    for category in index.categories_by_key.values() {
        let capacities: Vec<String> = category
            .rarity_capacities
            .iter()
            .map(u16::to_string)
            .collect();
        lines.push(format!(
            "category\t{}\t{}\t{}\t{}\t{}",
            category.category_key,
            capacities.join(","),
            category.mode12_lottery_weight,
            category.mode12_capacity,
            category.mode12_count_multiplier_key
        ));
    }
    for multiplier in index.count_multipliers_by_key.values() {
        let bits: Vec<String> = multiplier
            .multipliers
            .iter()
            .map(|value| format!("{:08x}", value.to_bits()))
            .collect();
        lines.push(format!(
            "ccmult\t{}\t{}",
            multiplier.lookup_key,
            bits.join(",")
        ));
    }
    for effect in index.effects_by_id.values() {
        let mut weights = Vec::with_capacity(128);
        for weight in effect.lottery_weights {
            weights.extend_from_slice(&weight.to_le_bytes());
        }
        lines.push(format!(
            "effect\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}",
            effect.effect_id,
            effect.group_key,
            effect.flags,
            effect.normalization_flags,
            effect.progress_threshold,
            effect.alternate_threshold,
            fnv1a64(&weights)
        ));
    }
    for seed in seed_sweep() {
        lines.push(format!(
            "challenge\t{seed}\t{}",
            generate_challenge_attempt_count(seed)
        ));
    }

    // Row-level rechecks that depend on the decoded rows themselves.
    for record_type in SCROLL_RECORD_TYPES {
        let _ = index.items_by_record_type.get(&record_type);
    }
    if let Some(row) = built.effect.row(0) {
        let parsed = EffectRow::new(row).map_err(|error| format!("{error:?}"))?;
        lines.push(format!("probe\teffect0\t{}", parsed.effect_id()));
    }
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("EFFECT_VECTORS_ERROR: {error}");
        std::process::exit(1);
    }
}
