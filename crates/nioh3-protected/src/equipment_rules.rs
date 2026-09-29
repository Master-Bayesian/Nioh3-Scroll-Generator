//! Natural-generation rules for equipment edits (PC v2.02 tables).
//!
//! Read-only: the rules describe what the game can produce so the editor can
//! offer legal choices and label modded ones. They never refuse an edit.

use std::path::Path;
use std::sync::OnceLock;

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment::{EquipmentRules, Finding, SlotRole};
use serde_json::{json, Value};

use crate::HostError;

static RULES: OnceLock<Option<EquipmentRules<'static>>> = OnceLock::new();

fn load(data_root: &Path) -> Option<EquipmentRules<'static>> {
    let resource =
        nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0)).ok()?;
    let index: &'static EffectTableIndex =
        Box::leak(Box::new(EffectTableIndex::from_resource(&resource).ok()?));
    let graces = resource
        .grace_maps
        .iter()
        .flat_map(|map| map.ranges.iter().map(|range| range.effect_id))
        .collect::<Vec<_>>();
    Some(EquipmentRules::new(index, &resource.item, graces))
}

/// The shared rules, loaded on first use; `None` when the tables are missing.
pub fn rules(data_root: &Path) -> Option<&'static EquipmentRules<'static>> {
    RULES.get_or_init(|| load(data_root)).as_ref()
}

/// Give each written effect the group marker and category bits slot
/// normalization would, so the game shows the new effect's icon.
pub fn fill_effect_markers(data_root: &Path, patch: &mut nioh3_save::character::EquipmentPatch) {
    let Some(rules) = rules(data_root) else {
        return;
    };
    for effect in &mut patch.effects {
        if let Some((group, category)) = u16::try_from(effect.effect_id)
            .ok()
            .and_then(|id| rules.effect_marker(id))
        {
            effect.group = Some(group);
            effect.category = Some(category);
        }
    }
}

fn role_name(role: SlotRole) -> &'static str {
    match role {
        SlotRole::Innate => "innate",
        SlotRole::Hell => "hell",
        SlotRole::Random => "random",
        SlotRole::Set => "set",
        SlotRole::Grace => "grace",
    }
}

fn finding_json(finding: &Finding) -> Value {
    match finding {
        Finding::UnknownItem => json!({ "code": "unknown_item" }),
        Finding::UnsupportedRarity => json!({ "code": "unsupported_rarity" }),
        Finding::EffectCount { expected, actual } => {
            json!({ "code": "effect_count", "expected": expected, "actual": actual })
        }
        Finding::UnknownEffect { slot, .. } => json!({ "code": "unknown_effect", "slot": slot }),
        Finding::MissingInnate { .. } => json!({ "code": "missing_innate" }),
        Finding::MissingSet { .. } => json!({ "code": "missing_set" }),
        Finding::MissingGrace => json!({ "code": "missing_grace" }),
        Finding::UnexpectedFixed { slot } => json!({ "code": "unexpected_fixed", "slot": slot }),
        Finding::NotInPool { slot } => json!({ "code": "not_in_pool", "slot": slot }),
        Finding::HellEffectOnNormal { slot } => {
            json!({ "code": "hell_effect_on_normal", "slot": slot })
        }
        Finding::MissingHellEffect => json!({ "code": "missing_hell_effect" }),
        Finding::HellOnIneligibleItem => json!({ "code": "hell_on_ineligible_item" }),
        Finding::StarBelowRarity { slot } => json!({ "code": "star_below_rarity", "slot": slot }),
        Finding::MultipleStars => json!({ "code": "multiple_stars" }),
        Finding::StarFlagMismatch { slot } => json!({ "code": "star_flag_mismatch", "slot": slot }),
        Finding::GroupConflict { slot, other } => {
            json!({ "code": "group_conflict", "slot": slot, "other": other })
        }
        Finding::ValueNotNatural { slot, .. } => {
            json!({ "code": "value_not_natural", "slot": slot })
        }
        Finding::ValueAboveFormula { slot, .. } => {
            json!({ "code": "value_above_formula", "slot": slot })
        }
        Finding::RollOutOfRange { slot, .. } => {
            json!({ "code": "roll_out_of_range", "slot": slot })
        }
        Finding::ReplacedEffect { slot, original, .. } => {
            json!({ "code": "replaced_effect", "slot": slot, "original": original })
        }
    }
}
/// The audit a character row carries, or `null` when the tables are missing.
pub fn audit_json(data_root: &Path, record: &[u8]) -> Value {
    let Some(rules) = rules(data_root) else {
        return Value::Null;
    };
    let audit = rules.audit(record);
    json!({
        "natural": audit.natural(),
        "verdict": audit.verdict(),
        "findings": audit.findings.iter().map(finding_json).collect::<Vec<_>>(),
        "unverified": audit.unverified.iter().map(finding_json).collect::<Vec<_>>(),
    })
}

/// The audit of a saved record, natural when the game's generator reproduces
/// it from its own seed on a difficulty the character played (`replayed`).
/// The slot-layout rules cannot see every natural layout; the replay can.
pub fn audit_json_replayed(
    data_root: &Path,
    record: &[u8],
    progress: Option<&nioh3_save::character::GenerationProgress>,
) -> Value {
    let audit = audit_json(data_root, record);
    if audit["natural"] == Value::Bool(true)
        || !crate::equipment_seeds::replays_from_seed(data_root, record, progress)
    {
        return audit;
    }
    json!({
        "natural": true,
        "verdict": "natural",
        "findings": [],
        "unverified": [],
        "replayed": true,
    })
}

/// A new equipment record's content from an editor request.
///
/// Each requested effect takes the role natural generation gives its entry
/// (innate, random, grace, set, or the hell entry), its group marker and
/// category, and the roll its value comes from (the highest roll giving that
/// value, or 100 for a value natural generation never gives). The request is
/// not held to natural values: the preview audits the record like any edit.
pub fn new_equipment(
    data_root: &Path,
    params: &Value,
) -> Result<nioh3_save::character::NewEquipment, HostError> {
    use nioh3_save::character::{NewEntryRole, NewEquipment, NewEquipmentEffect, EMPTY_EFFECT_ID};
    let rules = loaded(data_root)?;
    let item_id = param_u64(params, "item_id", u64::from(u16::MAX))? as u16;
    let rarity = param_u64(params, "rarity", 5)? as u8;
    let level = param_u64(params, "level", u64::from(u16::MAX))? as u16;
    let plus = params
        .get("plus")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u16::MAX)) as u16;
    let hell = params.get("hell").and_then(Value::as_bool).unwrap_or(false);
    let hell_skill = params
        .get("hell_skill")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u16::MAX)) as u16;
    let item = rules.item(item_id).copied().ok_or_else(|| {
        HostError::rejected("This item cannot be added: it is not in the equipment tables")
    })?;
    if hell && !item.hell_capable() {
        return Err(HostError::rejected(
            "This item cannot be added as a hell weapon",
        ));
    }
    let roles = rules
        .slot_roles(&item, rarity, hell)
        .ok_or_else(|| HostError::rejected("This item cannot be added at this rarity"))?;
    let requested = params
        .get("effects")
        .and_then(Value::as_array)
        .ok_or_else(HostError::invalid_request)?;
    if requested.len() > nioh3_save::character::EQUIPMENT_EFFECT_COUNT {
        return Err(HostError::invalid_request());
    }
    let mut effects = Vec::new();
    for (index, effect) in requested.iter().enumerate() {
        let effect_id = effect
            .get("effect_id")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(HostError::invalid_request)?;
        if u32::from(effect_id) == EMPTY_EFFECT_ID {
            return Err(HostError::invalid_request());
        }
        let value = effect
            .get("value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(HostError::invalid_request)?;
        let (group, category) = rules
            .effect_marker(effect_id)
            .ok_or_else(|| HostError::rejected(format!("Unknown effect {effect_id:#06x}")))?;
        let role = match roles.get(index).copied().unwrap_or(SlotRole::Random) {
            SlotRole::Innate => NewEntryRole::Innate,
            SlotRole::Hell => NewEntryRole::Hell,
            SlotRole::Random => NewEntryRole::Random,
            SlotRole::Grace => NewEntryRole::Grace,
            SlotRole::Set => NewEntryRole::Set,
        };
        let roll = match role {
            NewEntryRole::Grace | NewEntryRole::Set => 0,
            _ => rules
                .legal_values(effect_id, rarity, level)
                .ok()
                .and_then(|values| {
                    values
                        .iter()
                        .find(|legal| i64::from(legal.value) == i64::from(value))
                        .map(|legal| legal.roll_max)
                })
                .unwrap_or(100),
        };
        let star = effect
            .get("star")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                rules
                    .effect(effect_id)
                    .is_some_and(|definition| definition.normalization_flags & 0x08 != 0)
            });
        effects.push(NewEquipmentEffect {
            role,
            effect_id: u32::from(effect_id),
            value,
            roll,
            star,
            group,
            category,
        });
    }
    Ok(NewEquipment {
        item_id,
        level,
        plus,
        rarity,
        hell,
        hell_skill,
        effects,
    })
}

fn param_u64(params: &Value, key: &str, max: u64) -> Result<u64, HostError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| *value <= max)
        .ok_or_else(|| HostError::rejected(format!("{key} is missing or out of range")))
}

fn loaded(data_root: &Path) -> Result<&'static EquipmentRules<'static>, HostError> {
    rules(data_root)
        .ok_or_else(|| HostError::rejected("Equipment rules are unavailable for this game version"))
}

/// `save.equipment_rules`: what natural generation places in each slot of one item.
pub fn equipment_rules_json(data_root: &Path, params: &Value) -> Result<Value, HostError> {
    let rules = loaded(data_root)?;
    let item_id = param_u64(params, "item_id", u64::from(u16::MAX))? as u16;
    let rarity = param_u64(params, "rarity", 255)? as u8;
    let level = param_u64(params, "level", u64::from(u16::MAX))? as u16;
    let hell = params.get("hell").and_then(Value::as_bool).unwrap_or(false);
    let Some(item) = rules.item(item_id).copied() else {
        return Ok(json!({ "item_id": item_id, "known": false }));
    };
    let roles = rules
        .slot_roles(&item, rarity, hell)
        .map(|roles| roles.into_iter().map(role_name).collect::<Vec<_>>());
    // The natural value range of each candidate at this rarity and level, so
    // same-named variants can be told apart.
    let range = |effect_id: u16| {
        rules
            .legal_values(effect_id, rarity.min(5), level)
            .ok()
            .and_then(|values| {
                let min = values.iter().map(|value| value.value).min()?;
                let max = values.iter().map(|value| value.value).max()?;
                Some((min, max))
            })
    };
    // Each drawn candidate carries its exclusion key so the editor can apply
    // the generator's group exclusion between slots.
    let candidate = |effect_id: u16, star: bool| {
        let (min, max) = range(effect_id).unwrap_or((0, 0));
        let (group, masks) = rules.conflict_key(effect_id).unwrap_or((0, [0, 0]));
        json!({
            "effect_id": effect_id,
            "star": star,
            "min": min,
            "max": max,
            "group": group,
            "masks": masks,
        })
    };
    let random = rules
        .random_pool(&item, rarity)
        .into_iter()
        .map(|effect| candidate(effect.effect_id, effect.star))
        .collect::<Vec<_>>();
    let hell_pool = rules
        .hell_pool(&item)
        .into_iter()
        .map(|effect_id| candidate(effect_id, false))
        .collect::<Vec<_>>();
    Ok(json!({
        "item_id": item_id,
        "known": true,
        "rarity": rarity,
        "level": level,
        "hell": hell,
        "hell_capable": item.hell_capable(),
        "soul_core": item.soul_core(),
        "roles": roles,
        "innate": item.innate_effects.iter().flatten().collect::<Vec<_>>(),
        "set_effect": item.set_effect,
        "graces": rules.graces().collect::<Vec<_>>(),
        "random_pool": random,
        "hell_pool": hell_pool,
        "hell_skills": rules.hell_skills(&item, level),
    }))
}

/// `save.effect_values`: every raw value an effect naturally takes, worst first.
pub fn effect_values_json(data_root: &Path, params: &Value) -> Result<Value, HostError> {
    let rules = loaded(data_root)?;
    let effect_id = param_u64(params, "effect_id", u64::from(u16::MAX))? as u16;
    let rarity = param_u64(params, "rarity", 5)? as u8;
    let level = param_u64(params, "level", u64::from(u16::MAX))? as u16;
    let star = rules
        .effect(effect_id)
        .map(|effect| effect.normalization_flags & 0x08 != 0);
    let values = rules
        .legal_values(effect_id, rarity, level)
        .map_err(|error| HostError::rejected(format!("{error:?}")))?;
    Ok(json!({
        "effect_id": effect_id,
        "rarity": rarity,
        "level": level,
        "star": star,
        "values": values
            .iter()
            .map(|value| json!({
                "value": value.value,
                "roll_min": value.roll_min,
                "roll_max": value.roll_max,
                "probability": value.probability,
                "top_fraction": value.top_fraction,
            }))
            .collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn data() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
    }

    #[test]
    fn a_katana_lists_its_slots_pools_and_hell_skills() {
        let rules = equipment_rules_json(
            &data(),
            &json!({ "item_id": 0x4BF7, "rarity": 4, "level": 170, "hell": true }),
        )
        .unwrap();
        assert_eq!(rules["known"], true);
        assert_eq!(rules["soul_core"], false);
        assert_eq!(
            rules["roles"],
            json!(["hell", "random", "random", "random", "grace"])
        );
        assert_eq!(rules["hell_skills"], json!([0xC0C1, 0x3435, 0xAC06]));
        assert!(rules["hell_pool"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["effect_id"] == 0x8641 && entry["max"].as_i64() > Some(0)));
        assert!(rules["random_pool"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["effect_id"] == 0x31D0 && entry["star"] == true));
        // Every drawn candidate carries its exclusion key; the star 武技精力伤害
        // shares group 0x9AE9 with its ordinary row.
        let pool = rules["random_pool"].as_array().unwrap();
        assert!(pool
            .iter()
            .all(|entry| entry["group"].is_u64() && entry["masks"].as_array().unwrap().len() == 2));
        assert!(pool
            .iter()
            .any(|entry| entry["effect_id"] == 0x31D0 && entry["group"] == 0x9AE9));
    }

    #[test]
    fn written_effects_get_the_marker_the_blacksmith_writes() {
        // Live PC v2.02: a blacksmith replacement to 火枪伤害 0xB3DB wrote
        // group 0x90D7 and category bits 0x1B into the entry.
        let mut patch = nioh3_save::character::EquipmentPatch {
            effects: vec![nioh3_save::character::EquipmentEffectPatch {
                index: 3,
                effect_id: 0xB3DB,
                value: 33,
                roll: Some(99),
                star: Some(false),
                group: None,
                category: None,
            }],
            ..Default::default()
        };
        fill_effect_markers(&data(), &mut patch);
        assert_eq!(patch.effects[0].group, Some(0x90D7));
        assert_eq!(patch.effects[0].category, Some(0x1B));
    }

    #[test]
    fn effect_values_carry_probabilities_best_last() {
        let values = effect_values_json(
            &data(),
            &json!({ "effect_id": 0x010A, "rarity": 4, "level": 170 }),
        )
        .unwrap();
        assert_eq!(values["star"], true);
        let list = values["values"].as_array().unwrap();
        assert_eq!(list.last().unwrap()["roll_max"], 100);
        assert!(list.iter().any(|value| value["value"] == 180));
    }
}
