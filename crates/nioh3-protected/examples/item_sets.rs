//! Write `apps/workshop/item-sets.json`: the set effect each piece of
//! equipment carries in the item table, so the equipment lists can group the
//! pieces of one set together.
//!
//! The set is the item record's own set-effect id (the last fixed effect of a
//! set piece). Items without one are left out; nothing is inferred from names.
//!
//! Usage: `cargo run --example item_sets -- <repository root>`

use std::collections::BTreeMap;
use std::path::Path;

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment::EquipmentRules;
use serde_json::{json, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: item_sets <repository root>")?;
    let root = Path::new(&root);
    let resource = nioh3_data::load_effect_resource_for_file_version(
        &root.join("nioh3_scroll_editor").join("data"),
        (2, 0, 2, 0),
    )?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;
    let rules = EquipmentRules::new(&index, &resource.item, Vec::new());
    let names: Value = serde_json::from_slice(&std::fs::read(
        root.join("apps").join("workshop").join("item-names.json"),
    )?)?;
    let catalog = names["items"]
        .as_object()
        .ok_or("item-names.json has no items")?;

    let mut items = BTreeMap::new();
    for item_id in 0..=u16::MAX {
        let Some(item) = rules.item(item_id) else {
            continue;
        };
        let Some(set) = item.set_effect else {
            continue;
        };
        // Only equipment the catalog names; unnamed rows are not offered anywhere.
        let named = catalog
            .get(&item_id.to_string())
            .and_then(|entry| entry[0].as_str())
            .is_some_and(|name| !name.is_empty());
        if named {
            items.insert(item_id.to_string(), json!(set));
        }
    }
    let out = json!({
        "schema": "nioh3-item-sets/v1",
        "game_version": "2.02",
        "provenance": "crates/nioh3-protected/examples/item_sets.rs: item table set-effect id over the PC v2.02 tables",
        "items": items,
    });
    let path = root.join("apps").join("workshop").join("item-sets.json");
    std::fs::write(&path, serde_json::to_string(&out)? + "\n")?;
    println!("{} set pieces -> {}", items.len(), path.display());
    Ok(())
}
