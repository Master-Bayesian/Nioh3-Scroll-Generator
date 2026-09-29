//! Seeded equipment generation against the game's own code (PC v2.02).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment_generation::{EquipmentGenerator, PlayerState, EMPTY_ENTRY};

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn generated_records_match_the_game_byte_for_byte() {
    let resource = nioh3_data::load_effect_resource_for_file_version(&data_root(), (2, 0, 2, 0))
        .expect("v2.02 resource");
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let generator = EquipmentGenerator::new(&resource, &index);
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/equipment_generation_v202.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 900);
    let mut wiped = 0;
    let mut soul_cores = 0;
    for case in cases {
        let number = |key: &str| case[key].as_u64().unwrap();
        let progress: Vec<u32> = case["progress"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect();
        let state = PlayerState {
            type_class: number("type_class") as u8,
            progress: [progress[0], progress[1], progress[2], progress[3]],
        };
        let item = number("item") as u16;
        let rarity = number("rarity") as u8;
        let seed = number("seed") as u16;
        let prepared = generator.prepare(item, rarity, state).unwrap();
        let entries = prepared.generate(seed);
        let record = prepared
            .build_record(
                &entries,
                seed,
                number("level") as u16,
                number("plus") as u16,
            )
            .unwrap();
        let game = unhex(case["record"].as_str().unwrap());
        assert_eq!(
            record.as_slice(),
            game.as_slice(),
            "item {item:#x} rarity {rarity} seed {seed:#x} state {state:?}"
        );
        if entries.iter().all(|entry| entry.effect_id == EMPTY_ENTRY) {
            wiped += 1;
        }
        if resource
            .item
            .rows
            .chunks(resource.item.row_size)
            .any(|row| u16::from_le_bytes([row[0x152], row[0x153]]) == item && row[0x182] == 9)
        {
            soul_cores += 1;
        }
    }
    // The fixture covers items the game empties and soul cores.
    assert!(wiped > 0, "no emptied item in the fixture");
    assert!(soul_cores > 0, "no soul core in the fixture");
}
