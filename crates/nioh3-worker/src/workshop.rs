//! The workshop's frozen data files, rebuilt from the shipped tables.
//!
//! `apps/workshop/catalog.json`, `editor-values.json` and `ui-locales.json` are
//! bundled into the frontend. This module regenerates all three byte for byte
//! from `data/`, the workshop's own name sources and the live locale captures,
//! so a game-version update rebuilds them instead of editing them by hand.
//!
//! Object key order is part of the output, so the module needs serde_json's
//! `preserve_order` feature (the `workshop-export` crate feature).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use nioh3_data::{load_effect_resource, load_preview_resources, PreviewResources};
use nioh3_domain::auxiliary::describe_special_rule;
use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::preview::{compose_ng3_preview, PreviewTables};
use nioh3_domain::sequence::{compose_preview_sequence, generate_challenge_attempt_count};
use nioh3_domain::wraith;
use serde_json::{json, Map, Value};

use crate::catalog::{
    enemy_options, player_ready_effect_name, terrain_options, Catalog, R4_FINAL_GRACE_IDS,
};
use crate::recommended_level;

type Error = Box<dyn std::error::Error>;

const AUTHORS: [&str; 2] = ["MasterBayesian", "Saber_Li"];
const QQ_GROUP: &str = "1106302479";
const GITHUB_URL: &str = "https://github.com/Master-Bayesian/Nioh3-Scroll-Generator";
const SAMPLE_LEVEL: u16 = 180;
/// NG1/NG2 rarity 4 finalizes to the ten Graces of its stage-one map, not the
/// 21 NG3 final Graces.
const NG12_R4_GRACE_IDS: [u32; 10] = [
    9189, 10982, 20387, 20498, 22796, 31722, 36044, 45545, 45647, 59627,
];
/// Lookup keys whose enemy shares its name with another form or identity.
const ENEMY_VARIANT_QUALIFIERS: [(u32, [&str; 3]); 11] = [
    (0x0004B03B, ["人形", "人型", "Human form"]),
    (0x00092063, ["妖怪形态", "妖怪形態", "Yokai form"]),
    (0x00071ED1, ["人形", "人型", "Human form"]),
    (0x0000A5D2, ["妖怪形态", "妖怪形態", "Yokai form"]),
    (0x000B605B, ["人形", "人型", "Human form"]),
    (
        0x0004ACDF,
        ["古代妖怪形态", "古代妖怪形態", "Ancient-yokai form"],
    ),
    (
        0x00093F79,
        ["江户妖怪形态", "江戸妖怪形態", "Edo-yokai form"],
    ),
    (0x0000F9CA, ["人形", "人型", "Human form"]),
    (0x000179F7, ["妖怪形态", "妖怪形態", "Yokai form"]),
    (0x000D35E1, ["现任／子", "当代", "Current"]),
    (0x000202A7, ["前代", "先代", "Former"]),
];
/// The R4 finalizer resource whose effect rows give each effect its group.
const GROUP_TABLE: &str = "r4_finalizer/pc_v2_02/resource_v1/tables/effect.bin";
const SOUL_CORE_FORMAT_TEXT: &str = "0x010D29F1";

/// The three files, as the bytes the repository commits.
pub struct WorkshopFiles {
    pub catalog: String,
    pub editor_values: String,
    pub ui_locales: String,
}

/// Rebuild the workshop files from the repository at `root`.
pub fn export(root: &Path) -> Result<WorkshopFiles, Error> {
    let data = root.join("data");
    let workshop = root.join("apps/workshop");
    let effect = load_effect_resource(&data)?;
    let index = EffectTableIndex::from_resource(&effect).map_err(|e| format!("{e:?}"))?;
    let preview = load_preview_resources(&data)?;
    let catalog = Catalog::load(&data).map_err(|e| e.to_string())?;
    let source = workshop_catalog(&data, &catalog, &effect, &index, &preview)?;
    let ui = ui_messages(&workshop)?;
    let mut game = Map::new();
    for locale in ["en-US", "ja-JP"] {
        let names = game_names(root, &catalog, &preview, &source, &ui, locale)?;
        game.insert(locale.to_string(), Value::Object(names));
    }
    let locales = json!({"ui": Value::Object(ui), "game": Value::Object(game)});
    Ok(WorkshopFiles {
        catalog: serde_json::to_string(&source)?,
        editor_values: serde_json::to_string(&editor_values(&catalog, &index)?)?,
        ui_locales: serde_json::to_string_pretty(&locales)? + "\n",
    })
}

fn qualifier(lookup_key: u32, locale: &str) -> Option<&'static str> {
    let column = match locale {
        "zh-CN" => 0,
        "ja-JP" => 1,
        _ => 2,
    };
    ENEMY_VARIANT_QUALIFIERS
        .iter()
        .find(|(key, _)| *key == lookup_key)
        .map(|(_, names)| names[column])
}

/// Python's `f"{value:g}"`: six significant digits, trailing zeros removed.
fn general_format(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let exponent = value.abs().log10().floor() as i32;
    let trim = |text: String| -> String {
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            text
        }
    };
    if (-4..6).contains(&exponent) {
        let decimals = (5 - exponent).max(0) as usize;
        trim(format!("{value:.decimals$}"))
    } else {
        let text = format!("{value:.5e}");
        let (mantissa, power) = text.split_once('e').unwrap_or((&text, "0"));
        let power: i32 = power.parse().unwrap_or(0);
        let sign = if power < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mantissa.to_string()), power.abs())
    }
}

fn rule_value(value: Option<f64>, unit: Option<&str>, grade: Option<&str>) -> String {
    match value {
        None => grade.unwrap_or("固定变体").to_string(),
        Some(value) => {
            let suffix = match unit {
                Some("percent") => "%",
                Some("seconds") => " 秒",
                _ => "",
            };
            general_format(value) + suffix
        }
    }
}

fn workshop_catalog(
    data: &Path,
    catalog: &Catalog,
    effect: &EffectResourceBytes,
    index: &EffectTableIndex,
    preview: &PreviewResources,
) -> Result<Value, Error> {
    let names = catalog.auxiliary("zh-CN").map_err(|e| e.to_string())?;
    let roles: BTreeMap<u32, u8> = enemy_options(&preview.roster, &names)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|option| {
            (
                option["lookup_key"].as_u64().unwrap_or_default() as u32,
                option["role"].as_u64().unwrap_or_default() as u8,
            )
        })
        .collect();
    let mut possessed = BTreeSet::new();
    for key in roles.keys() {
        if wraith::eligible(&preview.states, *key).map_err(|e| e.to_string())? {
            possessed.insert(*key);
        }
    }

    // Native enemy names, grouped by name, then same-name forms split apart.
    let mut by_name: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    for (key, name) in &names.enemies {
        let name = name.trim();
        let Ok(key) = u32::from_str_radix(key.get(2..).unwrap_or_default(), 16) else {
            continue;
        };
        if !name.is_empty() && roles.contains_key(&key) {
            by_name.entry(name.to_string()).or_default().insert(key);
        }
    }
    let mut ordered: Vec<(String, BTreeSet<u32>)> = by_name.into_iter().collect();
    ordered.sort_by_key(|(name, _)| name.to_lowercase());
    let mut groups: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    for (name, keys) in ordered {
        let variants: Vec<u32> = keys
            .iter()
            .copied()
            .filter(|key| qualifier(*key, "zh-CN").is_some())
            .collect();
        for key in &variants {
            let qualified = format!("{name}（{}）", qualifier(*key, "zh-CN").unwrap_or_default());
            groups.insert(qualified, BTreeSet::from([*key]));
        }
        let remaining: BTreeSet<u32> = keys
            .difference(&variants.iter().copied().collect())
            .copied()
            .collect();
        if !remaining.is_empty() {
            groups.insert(name, remaining);
        }
    }
    let enemies: Vec<Value> = groups
        .iter()
        .map(|(name, keys)| {
            let native: BTreeSet<u8> = keys.iter().map(|key| roles[key]).collect();
            let tier = if native.iter().all(|role| *role <= 3) {
                "低手"
            } else if native == BTreeSet::from([4]) {
                "中手"
            } else if native == BTreeSet::from([5]) {
                "高手"
            } else {
                "中／高手"
            };
            json!({
                "id": keys.iter().min().map(u32::to_string),
                "name": name,
                "keys": keys,
                "tier": tier,
                "possessedKeys": keys.intersection(&possessed).collect::<Vec<_>>(),
            })
        })
        .collect();

    let allowed = nioh3_domain::auxiliary::legal_special_rule_keys(3, &preview.rules)
        .map_err(|e| e.to_string())?;
    let families = names
        .special_rule_key_groups(
            &allowed,
            &preview.rules,
            &catalog.effect_names,
            &catalog.item_names,
        )
        .map_err(|e| e.to_string())?;
    let mut rules = Vec::new();
    for (name, keys) in families {
        let keys: Vec<u16> = keys.into_iter().filter(|key| *key != 0).collect();
        let Some(first) = keys.iter().min() else {
            continue;
        };
        let category = if name.starts_with("造成的") && name.contains("属性伤害增加") {
            "造成的属性伤害增加".to_string()
        } else {
            name.split('（').next().unwrap_or_default().to_string()
        };
        let mut variants = Vec::new();
        for key in &keys {
            let entry = describe_special_rule(*key, &preview.rules).map_err(|e| e.to_string())?;
            let label = rule_value(entry.display_value, entry.display_unit, entry.display_grade);
            variants.push(json!({"key": key, "label": label}));
        }
        rules.push(json!({
            "id": first.to_string(),
            "name": name,
            "category": category,
            "keys": keys,
            "variants": variants,
        }));
    }

    let r4_map = effect
        .grace_maps
        .first()
        .ok_or("missing the rarity-4 Grace map")?;
    let r5_map = effect
        .grace_maps
        .get(1)
        .ok_or("missing the rarity-5 Grace map")?;
    let mut contexts = Map::new();
    for playthrough in 1..=5u8 {
        for rarity in 3..=5u8 {
            let effects: Vec<Value> = catalog
                .ordinary_effects(index, playthrough, rarity, "zh-CN")
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|row| json!({"id": row["effect_id"].to_string(), "name": row["name"]}))
                .collect();
            let grace_ids: Vec<u32> = if rarity == 4 && playthrough <= 2 {
                NG12_R4_GRACE_IDS.to_vec()
            } else if rarity == 4 && playthrough == 3 {
                let ids: BTreeSet<u32> = r4_map
                    .ranges
                    .iter()
                    .map(|range| range.effect_id)
                    .filter(|id| R4_FINAL_GRACE_IDS.contains(id))
                    .collect();
                ids.into_iter().collect()
            } else if rarity == 5 && playthrough >= 3 {
                let ids: BTreeSet<u32> =
                    r5_map.ranges.iter().map(|range| range.effect_id).collect();
                ids.into_iter().collect()
            } else {
                Vec::new()
            };
            let graces: Vec<Value> = grace_ids
                .iter()
                .map(|id| {
                    let name = catalog
                        .effect_names
                        .name(*id, "zh-CN")
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("0x{id:X}"));
                    json!({"id": id.to_string(), "name": name})
                })
                .collect();
            contexts.insert(
                format!("{playthrough}-{rarity}"),
                json!({"effects": effects, "graces": graces}),
            );
        }
    }

    let tables = PreviewTables {
        roster: &preview.roster,
        context: &preview.context,
        rules: &preview.rules,
        states: &preview.states,
    };
    let grace_names = load_grace_names(data, catalog);
    let mut seeds: Vec<u32> = vec![10030565, 43723117, 36526331];
    seeds.extend(12000..12197);
    let mut samples = Vec::new();
    for rarity in 3..=5u8 {
        let context_graces: BTreeSet<String> = contexts[&format!("3-{rarity}")]["graces"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| row["id"].as_str().unwrap_or_default().to_string())
            .collect();
        for seed in &seeds {
            let sequence = compose_preview_sequence(
                index,
                3,
                rarity,
                *seed,
                SAMPLE_LEVEL,
                Some(r4_map),
                Some(r5_map),
            )
            .map_err(|e| format!("{e:?}"))?;
            let composition =
                compose_ng3_preview(*seed, 3, &tables).map_err(|e| format!("{e:?}"))?;
            let auxiliary = &composition.auxiliary;
            let enemy_keys: Vec<u32> = auxiliary
                .enemy_groups
                .iter()
                .flat_map(|group| group.entries.iter().map(|entry| entry.lookup_key))
                .collect();
            let slot_keys: Vec<u32> = auxiliary
                .enemy_groups
                .iter()
                .filter_map(|group| group.entries.first().map(|entry| entry.lookup_key))
                .collect();
            let mut enemy_names: Vec<String> = Vec::new();
            for key in &enemy_keys {
                let name = names.enemy_name(*key);
                if !enemy_names.contains(&name) {
                    enemy_names.push(name);
                }
            }
            let slot_count = sequence.effects.len();
            let effects: Vec<Value> = sequence
                .effects
                .iter()
                .filter(|effect| effect.effect_id != 0xFFFF_FFFF)
                .map(|effect| {
                    let id = effect.effect_id.to_string();
                    let role = if effect.slot == 1 {
                        "主词条"
                    } else if context_graces.contains(&id) && usize::from(effect.slot) == slot_count {
                        "恩宠"
                    } else if rarity == 3 && effect.slot == 5 {
                        "成长词条"
                    } else {
                        "副词条"
                    };
                    json!({
                        "id": id,
                        "name": contextual_effect_name(catalog, &grace_names, effect.effect_id, rarity, effect.slot),
                        "roll": effect.roll_percent,
                        "raw": effect.resolved_value,
                        "role": role,
                    })
                })
                .collect();
            let rules: Vec<Value> = auxiliary
                .special_rules
                .entries
                .iter()
                .map(|entry| {
                    json!({
                        "key": entry.key,
                        "name": names.special_rule_name(entry.key, &preview.rules, &catalog.effect_names, &catalog.item_names),
                        "value": rule_value(entry.display_value, entry.display_unit, entry.display_grade),
                    })
                })
                .collect();
            samples.push(json!({
                "seed": seed.to_string(),
                "rarity": rarity,
                "effects": effects,
                "capacity": generate_challenge_attempt_count(*seed),
                "enemyKeys": enemy_keys,
                "enemySlotKeys": slot_keys,
                "enemies": enemy_names,
                "terrainKeys": auxiliary.terrain.display_effect_keys,
                "rules": rules,
            }));
        }
    }

    let curve = recommended_level::load(data).map_err(|e| e.to_string())?;
    let (minimum, maximum) = curve.displayed_level_bounds();
    let mut levels = Map::new();
    for level in minimum..=maximum {
        let selected = curve.resolve_displayed_level(level).selected_internal_level;
        levels.insert(level.to_string(), json!(selected));
    }

    let mut identities = Map::new();
    let mut by_row: Vec<_> = index.effects_by_id.values().collect();
    by_row.sort_by_key(|definition| definition.row_index);
    for definition in by_row {
        let category = index
            .group_for_effect(definition.effect_id)
            .map(|group| group.category_key)
            .ok_or_else(|| format!("effect {} has no group", definition.effect_id))?;
        identities.insert(
            definition.effect_id.to_string(),
            json!({
                "prefix": definition.group_key,
                "category": category,
                "star": definition.normalization_flags & 0x08 != 0,
            }),
        );
    }

    let terrains = terrain_options(&preview.roster, &names, "zh-CN");
    let mut runtime_terrains = Map::new();
    for choice in crate::terrain::terrain_choices(&preview.roster.terrains) {
        if choice.aggregate {
            continue;
        }
        let row = choice
            .rows
            .iter()
            .min()
            .ok_or("terrain option without rows")?;
        let value = preview.roster.terrains[*row as usize][crate::terrain::TERRAIN_VALUE_OFFSET];
        runtime_terrains.insert(choice.option_id, json!(value));
    }

    let mut enemy_roles = Map::new();
    for (key, role) in &roles {
        enemy_roles.insert(key.to_string(), json!(role));
    }
    let editor_effects: Vec<Value> = catalog
        .effect_names
        .names
        .keys()
        .map(|id| json!({"id": id.to_string(), "name": native_name(catalog, *id)}))
        .collect();
    let mut enemies = enemies;
    enemies.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

    Ok(json!({
        "runtimeTerrains": runtime_terrains,
        "editorIdentities": identities,
        "enemyRoles": enemy_roles,
        "editorEffects": editor_effects,
        "contexts": contexts,
        "enemies": enemies,
        "rules": rules,
        "terrains": terrains,
        "samples": samples,
        "levels": levels,
        "authors": AUTHORS,
        "qq": QQ_GROUP,
        "github": GITHUB_URL,
        "qqInvite": null,
    }))
}

/// `catalog.native_effect_name` in the preferred locale, or the unknown label.
fn native_name(catalog: &Catalog, effect_id: u32) -> String {
    catalog
        .effect_names
        .name(effect_id, &catalog.preferred_locale)
        .unwrap_or("未知词条")
        .to_string()
}

/// The shipped Grace names, native names first.
fn load_grace_names(data: &Path, catalog: &Catalog) -> BTreeMap<u32, String> {
    let mut names = BTreeMap::new();
    let Ok(text) = fs::read_to_string(data.join("grace_names_zh_cn.json")) else {
        return names;
    };
    let Ok(Value::Object(loaded)) = serde_json::from_str::<Value>(&text) else {
        return names;
    };
    for (identifier, name) in loaded {
        let name = name.as_str().unwrap_or_default().trim().to_string();
        let parsed = match identifier
            .strip_prefix("0x")
            .or_else(|| identifier.strip_prefix("0X"))
        {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => identifier.parse().ok(),
        };
        let Some(id) = parsed else {
            continue;
        };
        // A Grace listed without a curated name still takes its native name.
        match catalog.effect_names.name(id, &catalog.preferred_locale) {
            Some(native) => {
                names.insert(id, native.to_string());
            }
            None if !name.is_empty() => {
                names.insert(id, name);
            }
            None => {}
        }
    }
    names
}

/// `catalog.contextual_effect_name` for a final (not stage-one) record.
fn contextual_effect_name(
    catalog: &Catalog,
    graces: &BTreeMap<u32, String>,
    effect_id: u32,
    rarity: u8,
    slot: u8,
) -> String {
    let native = || {
        catalog
            .effect_names
            .name(effect_id, &catalog.preferred_locale)
    };
    if rarity == 3 && slot == 5 && effect_id == 1 {
        return "未完成的杰作（画龙点睛）".to_string();
    }
    if rarity == 4 && slot == 5 && R4_FINAL_GRACE_IDS.contains(&effect_id) {
        return native()
            .map(str::to_string)
            .or_else(|| graces.get(&effect_id).cloned())
            .unwrap_or_else(|| format!("R4恩宠 0x{effect_id:04X}（名称待验证）"));
    }
    if rarity == 5 && slot == 6 {
        if let Some(known) = graces.get(&effect_id) {
            return known.clone();
        }
        if effect_id == 0xFFFF_FFFF {
            return "空".to_string();
        }
        return format!("R5恩宠 0x{effect_id:04X}（名称待验证）");
    }
    if let Ok(id) = u16::try_from(effect_id) {
        if crate::catalog::is_curated_effect(id) {
            return player_ready_effect_name(&catalog.effect_names, &catalog.preferred_locale, id);
        }
    }
    if effect_id == 0xBABD {
        return graces
            .get(&effect_id)
            .cloned()
            .unwrap_or_else(|| "月读的恩宠".to_string());
    }
    if effect_id == 1 {
        return "未完成的杰作（画龙点睛；最终效果实验预测中）".to_string();
    }
    if effect_id == 0xFFFF_FFFF {
        return "空".to_string();
    }
    native()
        .map(str::to_string)
        .unwrap_or_else(|| format!("编号 0x{effect_id:08X}"))
}

/// Exact native raw-value sets per effect, level and roll floor.
fn editor_values(catalog: &Catalog, index: &EffectTableIndex) -> Result<Value, Error> {
    let mut sets: Vec<Vec<u32>> = Vec::new();
    let mut set_ids: BTreeMap<Vec<u32>, usize> = BTreeMap::new();
    let mut patterns: Vec<Vec<usize>> = Vec::new();
    let mut pattern_ids: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    let mut effects = Map::new();
    for effect_id in catalog.effect_names.names.keys() {
        let Ok(id) = u16::try_from(*effect_id) else {
            continue;
        };
        if !index.effects_by_id.contains_key(&id)
            || index.resolved_effect_value(*effect_id, 100, 180).is_err()
        {
            continue;
        }
        let mut pattern = Vec::with_capacity(181 * 3);
        for level in 0..=180u16 {
            let mut values = Vec::with_capacity(41);
            for roll in 60..=100u8 {
                let value = index
                    .resolved_effect_value(*effect_id, roll, level)
                    .map_err(|e| format!("{e:?}"))?;
                values.push(value as u32);
            }
            for start in [0usize, 20, 30] {
                let native: Vec<u32> = values[start..]
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let next = sets.len();
                let id = *set_ids.entry(native.clone()).or_insert(next);
                if id == next {
                    sets.push(native);
                }
                pattern.push(id);
            }
        }
        let next = patterns.len();
        let id = *pattern_ids.entry(pattern.clone()).or_insert(next);
        if id == next {
            patterns.push(pattern);
        }
        effects.insert(effect_id.to_string(), json!(id));
    }
    Ok(json!({"effects": effects, "patterns": patterns, "sets": sets}))
}

/// `ui-translations.tsv`: Chinese source key -> [English, Japanese].
fn ui_messages(workshop: &Path) -> Result<Map<String, Value>, Error> {
    let text = fs::read_to_string(workshop.join("ui-translations.tsv"))?;
    let mut ui = Map::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [key, english, japanese] = fields[..] else {
            return Err(format!("expected key, English and Japanese: {line}").into());
        };
        if ui
            .insert(key.to_string(), json!([english, japanese]))
            .is_some()
        {
            return Err(format!("duplicate UI source key: {key}").into());
        }
    }
    Ok(ui)
}

// --- Game text markup -------------------------------------------------------

/// Remove ruby and default-style markup (`^20~default~`, `^FE~RUBY~`,
/// `^FF~RUBY,…~`), case-insensitively.
fn strip_ruby(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let at = |i: usize, pattern: &str| {
        bytes
            .get(i..i + pattern.len())
            .is_some_and(|slice| slice.eq_ignore_ascii_case(pattern.as_bytes()))
    };
    while i < bytes.len() {
        if bytes[i] == b'^' {
            if let Some(pattern) = ["^20~default~", "^21~default~", "^FE~RUBY~"]
                .iter()
                .find(|pattern| at(i, pattern))
            {
                i += pattern.len();
                continue;
            }
            if at(i, "^FF~RUBY,") {
                let rest = i + "^FF~RUBY,".len();
                if let Some(end) = bytes[rest..].iter().position(|byte| *byte == b'~') {
                    i = rest + end + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).expect("only ASCII markup is removed")
}

fn has_slot(text: &str) -> bool {
    text.contains("^09~BUFF~") || text.contains("^09~DEBUFF~") || text.contains("{}")
}

/// `^09~BUFF~{name}^09~~[状态]` -> `name[状态]`.
fn fill_named_slots(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("^09~") {
        let after = &rest[start + 4..];
        let body = after
            .strip_prefix("BUFF~{")
            .or_else(|| after.strip_prefix("DEBUFF~{"));
        let matched = body.and_then(|body| {
            let end = body.find('}')?;
            let tail = body[end + 1..].strip_prefix("^09~~")?;
            let (state, tail) = match tail.strip_prefix("状态") {
                Some(tail) => ("状态", tail),
                None => ("", tail),
            };
            Some((format!("{}{state}", &body[..end]), tail))
        });
        match matched {
            Some((replacement, tail)) => {
                out.push_str(&rest[..start]);
                out.push_str(&replacement);
                rest = tail;
            }
            None => {
                out.push_str(&rest[..start + 1]);
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `{argument}` -> `argument`, except after `~` and for JSON-like content.
fn unwrap_bare_arguments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous: Option<char> = None;
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if ch == '{' && previous != Some('~') {
            let body = &rest[1..];
            let end = body.find(['{', '}', '"', ':']);
            if let Some(end) = end.filter(|end| *end > 0 && body[*end..].starts_with('}')) {
                out.push_str(&body[..end]);
                previous = Some('}');
                rest = &body[end + 1..];
                continue;
            }
        }
        out.push(ch);
        previous = Some(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

/// The editor's Chinese effect name, or `None` while it is still generic.
fn zh_display(name: &str) -> Option<String> {
    let name = strip_ruby(name);
    if name.contains("{}") {
        return None;
    }
    Some(unwrap_bare_arguments(&fill_named_slots(&name)))
}

fn collapse_whitespace(text: &str) -> String {
    let is_space = |c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c);
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for ch in text.chars() {
        if is_space(ch) {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

fn strip_closing_qualifier(name: &str) -> String {
    if let Some(open) = name.strip_suffix('）').and_then(|body| body.rfind('（')) {
        let inner = &name[open + '（'.len_utf8()..name.len() - '）'.len_utf8()];
        if !inner.contains(['（', '）']) {
            return name[..open].to_string();
        }
    }
    name.to_string()
}

/// Python truthiness of a JSON value.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64() != Some(0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

fn read_json(path: &Path) -> Result<Value, Error> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Chinese editor name -> game name for one locale.
fn game_names(
    root: &Path,
    catalog: &Catalog,
    preview: &PreviewResources,
    source: &Value,
    ui: &Map<String, Value>,
    locale: &str,
) -> Result<Map<String, Value>, Error> {
    let names = catalog.auxiliary(locale).map_err(|e| e.to_string())?;
    let contexts = source["contexts"].as_object().ok_or("contexts")?;
    let mut effects: Vec<&Value> = source["editorEffects"]
        .as_array()
        .ok_or("editorEffects")?
        .iter()
        .collect();
    for context in contexts.values() {
        effects.extend(context["effects"].as_array().into_iter().flatten());
        effects.extend(context["graces"].as_array().into_iter().flatten());
    }
    let row_id = |row: &Value| -> u32 {
        row["id"]
            .as_str()
            .and_then(|id| id.parse().ok())
            .unwrap_or_default()
    };
    let row_name = |row: &Value| -> String { row["name"].as_str().unwrap_or_default().to_string() };

    let mut translated = Map::new();
    for row in &effects {
        let id = row_id(row);
        let mut name = catalog.effect_names.name(id, locale).map(str::to_string);
        if id == 47147 {
            name = Some(
                if locale == "en-US" {
                    "Increased Damage Taken (Winded Enemy)"
                } else {
                    "敵の気力切れで被ダメージ増加"
                }
                .to_string(),
            );
        }
        if let Some(text) = &name {
            if ["~BUFF~", "~DEBUFF~", "{}", "^09"]
                .iter()
                .any(|token| text.contains(token))
            {
                name = Some(format!(
                    "{}0x{id:04X}",
                    if locale == "en-US" {
                        "Effect "
                    } else {
                        "効果 "
                    }
                ));
            }
        }
        if let Some(name) = name {
            translated.insert(row_name(row), json!(name));
        }
    }
    for row in source["enemies"].as_array().ok_or("enemies")? {
        let key = row["keys"][0].as_u64().unwrap_or_default() as u32;
        let mut name = names.enemy_name(key);
        let zh = row_name(row);
        if let (false, Some(qualifier)) = (name.is_empty(), qualifier(key, locale)) {
            let bare = strip_closing_qualifier(&zh);
            if !translated.contains_key(&bare) {
                translated.insert(bare, json!(name));
            }
            name = if locale == "en-US" {
                format!("{name} ({qualifier})")
            } else {
                format!("{name}（{qualifier}）")
            };
        }
        translated.insert(zh, json!(name));
    }
    for row in source["rules"].as_array().ok_or("rules")? {
        let key = row["keys"][0].as_u64().unwrap_or_default() as u16;
        let name = names.special_rule_name(
            key,
            &preview.rules,
            &catalog.effect_names,
            &catalog.item_names,
        );
        translated.insert(row_name(row), json!(name));
    }
    let originals = source["terrains"].as_array().ok_or("terrains")?;
    for row in terrain_options(&preview.roster, &names, locale) {
        if let Some(original) = originals
            .iter()
            .find(|original| original["option_id"] == row["option_id"])
        {
            translated.insert(row_name(original), row["name"].clone());
        }
    }
    for (key, value) in captured_names(root, ui, contexts, &effects, locale)? {
        if !translated.contains_key(&key) {
            translated.insert(key, json!(value));
        }
    }
    Ok(translated
        .into_iter()
        .map(|(key, value)| match value {
            Value::String(text) => (key, json!(strip_ruby(&text))),
            other => (key, other),
        })
        .collect())
}

/// Exact Chinese display name -> game name, from a live capture of the game in
/// `locale`. The capture maps text ids to the game's own strings; the item,
/// effect-group and skill tables give each name its text id.
fn captured_names(
    root: &Path,
    ui: &Map<String, Value>,
    contexts: &Map<String, Value>,
    effects: &[&Value],
    locale: &str,
) -> Result<Vec<(String, String)>, Error> {
    let workshop = root.join("apps/workshop");
    let path = workshop
        .join("locale-captures")
        .join(format!("{locale}.json"));
    if !path.exists() {
        return Ok(Vec::new());
    }
    let capture = read_json(&path)?;
    let texts = &capture["texts"];
    let text = |tid: &Value| -> Option<String> {
        let tid = tid.as_str()?;
        if tid.is_empty() || tid == "0x00000000" {
            return None;
        }
        texts[tid]
            .as_str()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let item_name = |item: &str| text(&capture["items"][item]);
    let arguments = read_json(&workshop.join("effect-arguments.json"))?;
    let unnamed: Vec<u64> = arguments["unnamed_in_game"]["ids"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect();
    let mut resolved: BTreeMap<String, String> = arguments["names"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, name)| (id.clone(), name.as_str().unwrap_or_default().to_string()))
        .collect();
    for id in &unnamed {
        resolved
            .entry(id.to_string())
            .or_insert_with(|| format!("未命名忍术（0x{id:04X}）"));
    }
    let item_names = read_json(&workshop.join("item-names.json"))?;
    let item_rows = item_names["items"].as_object().ok_or("item-names items")?;
    let mut zh_items: BTreeMap<String, String> = BTreeMap::new();
    for (item, row) in item_rows {
        let zh = row[0].as_str().unwrap_or_default().to_string();
        let item: u64 = item.parse()?;
        zh_items.entry(zh).or_insert_with(|| item.to_string());
    }
    let table = fs::read(root.join("data").join(GROUP_TABLE))?;
    let mut group_of: BTreeMap<u16, u16> = BTreeMap::new();
    let mut offset = 8;
    while offset + 4 <= table.len() {
        let effect_id = u16::from_le_bytes([table[offset], table[offset + 1]]);
        let group = u16::from_le_bytes([table[offset + 2], table[offset + 3]]);
        group_of.entry(effect_id).or_insert(group);
        offset += 0xD8;
    }

    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut add = |zh: Option<&str>, name: Option<String>| {
        let (Some(zh), Some(name)) = (zh, name) else {
            return;
        };
        if zh.is_empty() || name.contains("DUMMY") || name.contains("UNUSED") {
            return;
        }
        let key = collapse_whitespace(zh);
        if seen.insert(key.clone()) {
            pairs.push((key, strip_ruby(&name)));
        }
    };

    let mut curated: BTreeMap<String, String> = BTreeMap::new();
    for context in contexts.values() {
        for row in context["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(context["graces"].as_array().into_iter().flatten())
        {
            let name = row["name"].as_str().unwrap_or_default();
            if !has_slot(name) {
                curated.insert(
                    row["id"].as_str().unwrap_or_default().to_string(),
                    name.to_string(),
                );
            }
        }
    }
    let unnamed_label = |id: u32| {
        if locale == "en-US" {
            format!("Unnamed Ninjutsu (0x{id:04X})")
        } else {
            format!("名称のない忍術（0x{id:04X}）")
        }
    };
    for row in effects {
        let id_text = row["id"].as_str().unwrap_or_default();
        let effect_id: u32 = id_text.parse().unwrap_or_default();
        let mut zh = row["name"].as_str().unwrap_or_default().to_string();
        if has_slot(&zh) {
            if let Some(name) = curated.get(id_text) {
                zh = name.clone();
            }
        }
        if let Some(argument) = resolved.get(id_text) {
            zh = zh.replace("{}", &format!("{{{argument}}}"));
        }
        let Some(zh) = zh_display(&zh).filter(|zh| !zh.is_empty()) else {
            continue;
        };
        let group = u16::try_from(effect_id)
            .ok()
            .and_then(|id| group_of.get(&id))
            .map(|group| &capture["groups"][group.to_string()])
            .filter(|group| group.is_object());
        let Some(group) = group else {
            continue;
        };
        let Some(mut name) = text(&group["name"]) else {
            continue;
        };
        if has_slot(&name) {
            let argument = if unnamed.contains(&u64::from(effect_id)) {
                Some(unnamed_label(effect_id))
            } else if name.contains("^09~BUFF~") || name.contains("^09~DEBUFF~") {
                text(&group["arg"])
            } else {
                let hex = |field: &str| {
                    u32::from_str_radix(
                        group[field]
                            .as_str()
                            .unwrap_or("0")
                            .trim_start_matches("0x"),
                        16,
                    )
                    .unwrap_or_default()
                };
                let subject = capture["subjects"][hex("subject").to_string()]
                    .as_array()
                    .and_then(|subjects| subjects.first())
                    .and_then(text);
                subject
                    .or_else(|| item_name(&hex("item").to_string()))
                    .or_else(|| {
                        let zh_argument = resolved.get(id_text).cloned().unwrap_or_default();
                        zh_items
                            .get(&zh_argument)
                            .and_then(|item| item_name(item))
                            .or_else(|| {
                                ui.get(&zh_argument)
                                    .and_then(|row| row[usize::from(locale == "ja-JP")].as_str())
                                    .filter(|text| !text.is_empty())
                                    .map(str::to_string)
                            })
                    })
            };
            let Some(argument) = argument else {
                continue;
            };
            name = name
                .replace("^09~BUFF~{}^09~~", &argument)
                .replace("^09~DEBUFF~{}^09~~", &argument)
                .replace("{}", &argument);
        }
        add(Some(&zh), Some(name));
    }

    let cores = read_json(&root.join("test_fixtures/soul_core_yokai.json"))?;
    let core_format = text(&json!(SOUL_CORE_FORMAT_TEXT));
    for (item, row) in item_rows {
        let core = &cores["cores"][item.as_str()];
        let name = match (&core_format, truthy(core) && !truthy(&core["placeholder"])) {
            (Some(format), true) => {
                Some(format.replace("{}", core[locale].as_str().unwrap_or_default()))
            }
            _ => item_name(item),
        };
        add(row[0].as_str(), name);
    }
    let hell = read_json(&workshop.join("hell-skill-names.json"))?;
    for (skill, zh) in hell["skills"].as_object().into_iter().flatten() {
        add(zh.as_str(), text(&capture["skills"][skill.as_str()]));
    }
    Ok(pairs)
}
