//! Seeded equipment generation against the game's own code (PC v2.02).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment_generation::{EquipmentGenerator, PlayerState, EMPTY_ENTRY};

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn retained_native_hell_records_match_the_supported_conversion() {
    let resource =
        nioh3_data::load_effect_resource_for_file_version(&data_root(), (2, 0, 2, 0)).unwrap();
    let index = EffectTableIndex::from_resource(&resource).unwrap();
    let generator = EquipmentGenerator::new(&resource, &index);
    let source_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/equipment_natural_v202.json");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&source_path).unwrap()).unwrap();
    let state = PlayerState {
        type_class: 1,
        progress: [6510, 7710, 0, 7710],
    };
    let semantics = |record: &[u8]| -> Vec<(u16, u32, i32, u8, u8, u8)> {
        (0..7)
            .filter_map(|position| {
                let slot = &record[0x34 + position * 0x18..0x4C + position * 0x18];
                let id = u32::from_le_bytes(slot[4..8].try_into().unwrap());
                (id != EMPTY_ENTRY).then(|| {
                    (
                        u16::from_le_bytes(slot[..2].try_into().unwrap()),
                        id,
                        i32::from_le_bytes(slot[8..12].try_into().unwrap()),
                        slot[0x0C],
                        slot[0x0D] & 0x3F,
                        slot[0x0E] & 7,
                    )
                })
            })
            .collect()
    };
    let mut cases = Vec::new();
    let mut kinds = std::collections::BTreeSet::new();
    let mut identities = std::collections::BTreeSet::new();
    let mut traced_kinds = std::collections::BTreeSet::new();
    let mut traced_identities = std::collections::BTreeSet::new();
    for captured in document["records"].as_array().unwrap() {
        let native = unhex(captured["record"].as_str().unwrap());
        if native[0x1A] & 0x10 == 0 {
            continue;
        }
        let word = |at| u16::from_le_bytes([native[at], native[at + 1]]);
        let (item, seed, level, plus, skill) =
            (word(0), word(0x22), word(8), word(0x0A), word(0x10));
        let prepared = generator.prepare_hell(item, native[0x30], state).unwrap();
        let entries = prepared.generate(seed);
        let built = prepared
            .build_hell_record(&entries, seed, level, plus, skill)
            .unwrap();
        let matched = semantics(&built) == semantics(&native);
        // d21ee3ac records zero converter flags for hell3 and the native
        // insertion. Earlier captures omit inputs and remain diagnostic.
        let traced_context = matches!(
            captured["source"].as_str(),
            Some("hell3" | "hell-conversion-trace" | "inserted-hell")
        );
        let item_row = resource
            .item
            .rows
            .chunks(resource.item.row_size)
            .find(|row| u16::from_le_bytes([row[0x152], row[0x153]]) == item)
            .unwrap();
        let kind = u16::from_le_bytes([item_row[0x58], item_row[0x59]]);
        kinds.insert(kind);
        identities.insert((item, seed));
        if traced_context {
            traced_kinds.insert(kind);
            traced_identities.insert((item, seed));
        }
        cases.push(
            serde_json::json!({"source":captured["source"],"item_id":item,"seed":seed,
            "level":level,"plus":plus,"rarity":native[0x30],"skill":skill,"weapon_type":kind,
            "matched":matched,"traced_zero_flag_context":traced_context,
            "native":semantics(&native),"generated":semantics(&built)}),
        );
    }
    let base = std::env::var_os("NIOH3_BUILD_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("D:/Nioh3_v080_deliverables"));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = base
        .join("deliverables/hell-weapon-seed-search-20261116/native-replay")
        .join(format!("run-{}-{nonce}.json", std::process::id()));
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let all_matched = cases
        .iter()
        .filter(|case| case["traced_zero_flag_context"] == true)
        .all(|case| case["matched"] == true);
    std::fs::write(&output,serde_json::to_vec_pretty(&serde_json::json!({
        "pass":all_matched,"game_version":"2.0.2.0","source":source_path,
        "scope":"retained native effect order, group, value, roll, category and set/grace/star semantics; zero drop flags",
        "provenance":document["provenance"],"difficulty":state.type_class,"progress":state.progress,
        "cases":cases,"unique_items_and_seeds":identities.len(),"weapon_types":kinds.len(),
        "traced_items_and_seeds":traced_identities.len(),"traced_weapon_types":traced_kinds.len(),
        "assertion_scope":"Only samples with zero converter flags recorded in d21ee3ac are strict oracles; untraced inputs remain unknown.",
        "game_or_real_save_accessed":false,
    })).unwrap()).unwrap();
    println!("Retained native hell replay artifact: {}", output.display());
    assert!(cases.len() >= 9 && identities.len() >= 9 && kinds.len() >= 3);
    assert!(traced_identities.len() >= 3 && traced_kinds.len() >= 3);
    assert!(
        all_matched,
        "retained native comparison failed; see {}",
        output.display()
    );
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
