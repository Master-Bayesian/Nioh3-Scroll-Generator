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
}
