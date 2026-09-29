//! Write `apps/workshop/effect-sources.json`: where natural generation can put
//! each effect, so the editor can tell apart effects that share a name.
//!
//! One effect group has a row per context (a weapon's random slot, an armor
//! or accessory random slot, a star tier, an item's innate effect, ...), and
//! every row carries the group's name. The tag names the equipment kinds and
//! slot roles a row can take, plus its value at rarity 4, level 180.
//!
//! Usage: `cargo run --example effect_sources -- <repository root>`

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment::EquipmentRules;
use serde_json::{json, Value};

/// Display order of the kinds a tag names.
const KINDS: [&str; 4] = ["武器", "防具", "饰品", "魂核"];

/// Slot roles in display order, with the text each adds to a tag.
const ROLES: [(&str, &str); 6] = [
    ("innate", "固有"),
    ("hell", "地狱"),
    ("random", "随机"),
    ("star", "星级"),
    ("set", "套装"),
    ("grace", "恩宠"),
];

/// The equipment kinds of a catalog kind; empty for rows that are not
/// equipment (scrolls draw their effects with a different generator).
fn kinds_of(catalog_kind: &str) -> &'static [&'static str] {
    match catalog_kind {
        "武器" => &["武器"],
        "防具" => &["防具"],
        "饰品" => &["饰品"],
        "防具或饰品" => &["防具", "饰品"],
        "魂核" => &["魂核"],
        _ => &[],
    }
}

fn tag(roles: &BTreeMap<&str, BTreeSet<&str>>, range: Option<(i32, i32)>) -> String {
    let mut parts = Vec::new();
    for (role, text) in ROLES {
        let Some(kinds) = roles.get(role) else {
            continue;
        };
        let named: Vec<&str> = if KINDS.iter().all(|kind| kinds.contains(kind)) {
            vec!["全部装备"]
        } else {
            KINDS.iter().copied().filter(|kind| kinds.contains(kind)).collect()
        };
        parts.push(format!("{}{}", named.join("·"), text));
    }
    let mut out = if parts.is_empty() {
        "非装备掉落".to_string()
    } else {
        parts.join("；")
    };
    match range {
        Some((low, high)) if low == high && low != 0 => out.push_str(&format!(" {low}")),
        Some((low, high)) if low != high => out.push_str(&format!(" {low}–{high}")),
        _ => {}
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: effect_sources <repository root>")?;
    let root = Path::new(&root);
    let resource = nioh3_data::load_effect_resource_for_file_version(
        &root.join("nioh3_scroll_editor").join("data"),
        (2, 0, 2, 0),
    )?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;
    let graces: Vec<u32> = resource
        .grace_maps
        .iter()
        .flat_map(|map| map.ranges.iter().map(|range| range.effect_id))
        .collect();
    let rules = EquipmentRules::new(&index, &resource.item, graces.clone());
    let names: Value = serde_json::from_slice(&std::fs::read(
        root.join("apps").join("workshop").join("item-names.json"),
    )?)?;
    let catalog = names["items"].as_object().ok_or("item-names.json has no items")?;

    let mut usage: BTreeMap<u16, BTreeMap<&str, BTreeSet<&str>>> = BTreeMap::new();
    for item_id in 0..=u16::MAX {
        let Some(item) = rules.item(item_id) else {
            continue;
        };
        let kind = catalog
            .get(&item_id.to_string())
            .and_then(|entry| entry[1].as_str())
            .unwrap_or("");
        let kinds = kinds_of(kind);
        if kinds.is_empty() {
            continue;
        }
        let mut add = |effect: u16, role: &'static str| {
            usage
                .entry(effect)
                .or_default()
                .entry(role)
                .or_default()
                .extend(kinds.iter().copied());
        };
        for rarity in 1..=5u8 {
            for pool in rules.random_pool(item, rarity) {
                add(pool.effect_id, if pool.star { "star" } else { "random" });
            }
        }
        for effect in rules.hell_pool(item) {
            add(effect, "hell");
        }
        for effect in item.innate_effects.iter().flatten() {
            add(*effect, "innate");
        }
        if let Some(effect) = item.set_effect {
            add(effect, "set");
        }
    }
    for grace in graces.iter().filter_map(|id| u16::try_from(*id).ok()) {
        usage
            .entry(grace)
            .or_default()
            .entry("grace")
            .or_default()
            .extend(["武器", "防具", "饰品"]);
    }

    let mut sources = serde_json::Map::new();
    for effect in &index.effects_in_row_order {
        let range = rules
            .legal_values(effect.effect_id, 4, 180)
            .ok()
            .and_then(|values| Some((values.first()?.value, values.last()?.value)));
        let empty = BTreeMap::new();
        sources.insert(
            effect.effect_id.to_string(),
            json!(tag(usage.get(&effect.effect_id).unwrap_or(&empty), range)),
        );
    }
    let out = json!({
        "schema": "nioh3-effect-sources/v1",
        "game_version": "2.02",
        "provenance": "crates/nioh3-protected/examples/effect_sources.rs over the PC v2.02 tables; values at rarity 4, level 180",
        "sources": sources,
    });
    let path = root.join("apps").join("workshop").join("effect-sources.json");
    std::fs::write(&path, serde_json::to_string(&out)? + "\n")?;
    println!("{} effects -> {}", sources_len(&out), path.display());
    Ok(())
}

fn sources_len(out: &Value) -> usize {
    out["sources"].as_object().map_or(0, |sources| sources.len())
}
