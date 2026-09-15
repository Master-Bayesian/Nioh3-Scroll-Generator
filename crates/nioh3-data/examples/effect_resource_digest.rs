//! Development parity emitter for the M2.1 effect resource loader.
//!
//! Prints per-table digests of the header-stripped rows the loader exposes plus
//! the Grace range tables, so `deliverables/v080-m21-data/parity_check.py` can
//! compare them against the Python reference reading the same files.

use std::{collections::BTreeMap, error::Error, path::Path};

use nioh3_data::load_effect_resource;
use sha2::{Digest, Sha256};

fn sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:X}", hasher.finalize())
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../nioh3_scroll_editor/data".to_string());
    let root = Path::new(&root);
    let resource = load_effect_resource(root)?;

    println!("{{");
    println!("  \"schema\": \"{}\",", resource.schema);
    let mut tables = BTreeMap::new();
    for (name, table) in [
        ("item", &resource.item),
        ("effect_group", &resource.effect_group),
        ("category", &resource.category),
        (
            "category_count_multiplier",
            &resource.category_count_multiplier,
        ),
        ("level_curve", &resource.level_curve),
        ("effect", &resource.effect),
        ("optional_multiplier", &resource.optional_multiplier),
        ("rarity_roll", &resource.rarity_roll),
        ("special_context", &resource.special_context),
    ] {
        tables.insert(
            name,
            (table.row_size, table.row_count(), sha256(&table.rows)),
        );
    }
    println!("  \"tables\": {{");
    let mut first = true;
    for (name, (row_size, row_count, digest)) in &tables {
        if !first {
            println!(",");
        }
        first = false;
        print!(
            "    \"{name}\": {{\"row_size\": {row_size}, \"row_count\": {row_count}, \"sha256\": \"{digest}\"}}"
        );
    }
    println!();
    println!("  }},");
    println!(
        "  \"bonus_curve_rows\": {{\"bytes\": {}, \"sha256\": \"{}\"}},",
        resource.bonus_curve_rows.len(),
        sha256(&resource.bonus_curve_rows)
    );
    println!(
        "  \"bonus_curve_index\": {{\"bytes\": {}, \"sha256\": \"{}\"}},",
        resource.bonus_curve_index.len(),
        sha256(&resource.bonus_curve_index)
    );
    println!(
        "  \"playthrough_progress\": {{\"bytes\": {}, \"sha256\": \"{}\"}},",
        resource.playthrough_progress.len(),
        sha256(&resource.playthrough_progress)
    );
    println!("  \"grace_maps\": [");
    for (index, map) in resource.grace_maps.iter().enumerate() {
        let ranges = map
            .ranges
            .iter()
            .map(|range| format!("[{}, {}, {}]", range.start, range.end, range.effect_id))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "    {{\"rarity\": {}, \"effect_slot\": {}, \"record_type\": {}, \"capture_state\": \"{}\", \"ranges\": [{}]}}{}",
            map.rarity,
            map.effect_slot,
            map.record_type,
            map.capture_state,
            ranges,
            if index + 1 == resource.grace_maps.len() { "" } else { "," }
        );
    }
    println!("  ],");
    // Full header-stripped rows for the tables the parity gate decodes, in
    // table order, so a reversed table assignment cannot pass.
    println!("  \"rows_hex\": {{");
    println!("    \"effect\": \"{}\",", hex(&resource.effect.rows));
    println!(
        "    \"level_curve\": \"{}\"",
        hex(&resource.level_curve.rows)
    );
    println!("  }}");
    println!("}}");
    Ok(())
}
