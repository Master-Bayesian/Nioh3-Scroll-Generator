//! Equipment natural-generation rules against captured PC v2.02 records.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use nioh3_domain::effect::{EffectResourceBytes, EffectTableIndex};
use nioh3_domain::equipment::{
    EquipmentRules, Finding, SlotRole, EFFECT_ENTRY_BYTES, EFFECT_ENTRY_OFFSET,
};

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
}

fn resource() -> EffectResourceBytes {
    nioh3_data::load_effect_resource_for_file_version(&data_root(), (2, 0, 2, 0))
        .expect("v2.02 resource")
}

fn graces(resource: &EffectResourceBytes) -> Vec<u32> {
    resource
        .grace_maps
        .iter()
        .flat_map(|map| map.ranges.iter().map(|range| range.effect_id))
        .collect()
}

fn records() -> Vec<(String, Vec<u8>)> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/equipment_natural_v202.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    doc["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            let hex = record["record"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (
                record["name"].as_str().unwrap_or_default().to_string(),
                bytes,
            )
        })
        .collect()
}

fn entry_mut(record: &mut [u8], slot: usize) -> &mut [u8] {
    &mut record[EFFECT_ENTRY_OFFSET + slot * EFFECT_ENTRY_BYTES..][..EFFECT_ENTRY_BYTES]
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn every_captured_natural_record_audits_natural() {
    let resource = resource();
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let rules = EquipmentRules::new(&index, &resource.item, graces(&resource));
    let records = records();
    assert!(records.len() >= 100, "fixture shrank to {}", records.len());
    let failures: Vec<String> = records
        .iter()
        .filter_map(|(name, record)| {
            let audit = rules.audit(record);
            (!audit.natural())
                .then(|| format!("{name} {:02x?}: {:?}", &record[..2], audit.findings))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} unnatural:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn legal_values_cover_the_roll_range_and_sum_to_one() {
    let resource = resource();
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let rules = EquipmentRules::new(&index, &resource.item, graces(&resource));
    // 体力恢复量（地狱）: 48..=60 at level 170 across rarities 3..=4 in the export.
    let values = rules.legal_values(0x00E1, 4, 170).unwrap();
    let total: f64 = values.iter().map(|value| value.probability).sum();
    assert!((total - 1.0).abs() < 1e-9, "probabilities sum to {total}");
    assert!((values[0].top_fraction - 1.0).abs() < 1e-9);
    assert_eq!(values.last().unwrap().roll_max, 100);
    assert!(values
        .windows(2)
        .all(|pair| pair[0].roll_max < pair[1].roll_min));
}

#[test]
fn edits_that_natural_generation_cannot_produce_are_reported() {
    let resource = resource();
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let rules = EquipmentRules::new(&index, &resource.item, graces(&resource));
    let records = records();
    let (_, natural) = records
        .iter()
        .find(|(_, record)| {
            let audit = rules.audit(record);
            audit.natural()
                && audit.roles.len() == 5
                && audit.roles[1] == Some(SlotRole::Random)
                && audit.roles[2] == Some(SlotRole::Random)
                && record[EFFECT_ENTRY_OFFSET + EFFECT_ENTRY_BYTES + 0xE] & 0x04 == 0
        })
        .expect("a natural rarity-4 record");

    let mut inflated = natural.clone();
    set_u32(entry_mut(&mut inflated, 1), 8, 9999);
    assert!(rules
        .audit(&inflated)
        .findings
        .contains(&Finding::ValueNotNatural {
            slot: 1,
            value: 9999
        }));

    let mut duplicated = natural.clone();
    let first_random = u32::from_le_bytes(entry_mut(&mut duplicated, 1)[4..8].try_into().unwrap());
    set_u32(entry_mut(&mut duplicated, 2), 4, first_random);
    assert!(rules
        .audit(&duplicated)
        .findings
        .iter()
        .any(|finding| matches!(finding, Finding::GroupConflict { slot: 2, other: 1 })));

    let mut unknown = natural.clone();
    set_u32(entry_mut(&mut unknown, 1), 4, 0xFFFE);
    assert!(!rules.audit(&unknown).natural());

    let mut fake_hell = natural.clone();
    fake_hell[0x1A] = 0x10;
    assert!(!rules.audit(&fake_hell).natural());

    // A slot whose id was swapped keeps the old effect's group marker.
    let mut replaced = natural.clone();
    let other_group: [u8; 2] = entry_mut(&mut replaced, 2)[0..2].try_into().unwrap();
    entry_mut(&mut replaced, 1)[0..2].copy_from_slice(&other_group);
    let recorded_group = u16::from_le_bytes(other_group);
    assert!(rules
        .audit(&replaced)
        .findings
        .iter()
        .any(|finding| matches!(
            finding,
            Finding::ReplacedEffect { slot: 1, recorded_group: group, original: Some(_) }
                if *group == recorded_group
        )));
}

#[test]
fn a_soul_core_re_rolled_to_a_second_star_is_not_unnatural() {
    let resource = resource();
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let rules = EquipmentRules::new(&index, &resource.item, graces(&resource));
    // 姑获鸟魂核 at rarity 3: two innate effects, then two random slots. A
    // player re-rolled the second random slot to the star 水属性伤害 0x4AE3
    // (16.0%) while the first already held the star 对伤害的反映（心） 0xE021.
    let item = *rules.item(31705).expect("姑获鸟魂核");
    assert!(item.soul_core());
    let (rarity, level) = (3u8, 170u16);
    let mut record = vec![0u8; 0xF0];
    record[0..2].copy_from_slice(&31705u16.to_le_bytes());
    record[6..8].copy_from_slice(&level.to_le_bytes());
    record[0x18] = 0x84;
    record[0x30] = rarity;
    for slot in 0..7 {
        set_u32(entry_mut(&mut record, slot), 4, u32::MAX);
    }
    let innate: Vec<u16> = item.innate_effects.iter().flatten().copied().collect();
    let effects = [innate[0], innate[1], 0xE021, 0x4AE3];
    for (slot, effect_id) in effects.iter().copied().enumerate() {
        let best = *rules
            .legal_values(effect_id, rarity, level)
            .unwrap()
            .last()
            .unwrap();
        let (group, category) = rules.effect_marker(effect_id).unwrap();
        let star = rules.effect(effect_id).unwrap().normalization_flags & 0x08 != 0;
        let entry = entry_mut(&mut record, slot);
        entry[0..2].copy_from_slice(&group.to_le_bytes());
        set_u32(entry, 4, u32::from(effect_id));
        set_u32(entry, 8, best.value as u32);
        entry[0xC] = best.roll_max;
        entry[0xD] = category;
        entry[0xE] = if star { 0x04 } else { 0 };
    }
    let audit = rules.audit(&record);
    assert!(
        audit.natural(),
        "{:?} {:?}",
        audit.findings,
        audit.unverified
    );

    // Other equipment still carries at most one star.
    let (_, natural) = records()
        .into_iter()
        .find(|(_, record)| {
            let item = u16::from_le_bytes([record[0], record[1]]);
            let audit = rules.audit(record);
            audit.natural()
                && !rules.item(item).unwrap().soul_core()
                && audit.roles.len() >= 3
                && audit.roles[1] == Some(SlotRole::Random)
                && audit.roles[2] == Some(SlotRole::Random)
                && record[EFFECT_ENTRY_OFFSET + EFFECT_ENTRY_BYTES + 0xE] & 0x04 != 0
        })
        .expect("a natural record with a star in its first random slot");
    let mut doubled = natural.clone();
    let star = entry_mut(&mut doubled, 1).to_vec();
    entry_mut(&mut doubled, 2).copy_from_slice(&star);
    assert!(rules
        .audit(&doubled)
        .findings
        .contains(&Finding::MultipleStars));
}
