//! Replay every owned hell weapon of encrypted saves through the offline hell
//! conversion and print one JSON line per record, then a summary.
//!
//! Usage: `cargo run --example hell_replay -- <data root> <SAVEDATA.BIN>...`

use std::path::Path;

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::equipment_generation::{EquipmentGenerator, PlayerState, EMPTY_ENTRY};
use nioh3_protected::equipment_seeds::replays_from_seed;
use nioh3_save::character::{equipment_record, equipment_slot_is_empty, generation_progress};
use nioh3_save::crypto::decrypt_container;
use nioh3_save::EQUIPMENT_SLOT_COUNT;

fn effects(record: &[u8]) -> Vec<(u32, u8)> {
    (0..7)
        .map(|index| {
            let at = 0x34 + index * 0x18;
            let id = u32::from_le_bytes(record[at + 4..at + 8].try_into().unwrap_or([0xFF; 4]));
            (id, record[at + 0x0C])
        })
        .filter(|(id, _)| *id != EMPTY_ENTRY)
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let data_root = args
        .next()
        .ok_or("usage: hell_replay <data root> <SAVEDATA.BIN>...")?;
    let data_root = Path::new(&data_root);
    let resource = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0))
        .map_err(|error| format!("{error:?}"))?;
    let index = EffectTableIndex::from_resource(&resource).map_err(|error| format!("{error:?}"))?;
    let generator = EquipmentGenerator::new(&resource, &index);
    let (mut hell, mut hell_matched, mut normal, mut normal_matched) = (0, 0, 0, 0);
    for save in args {
        let plain = decrypt_container(&std::fs::read(&save)?)?;
        let progress = generation_progress(&plain);
        if let Some(progress) = &progress {
            println!(
                "{}",
                serde_json::json!({
                    "save": save,
                    "difficulty": progress.difficulty,
                    "progress": progress.difficulties().into_iter()
                        .map(|difficulty| (difficulty, progress.progress(difficulty)))
                        .collect::<Vec<_>>(),
                })
            );
        }
        for slot in 0..EQUIPMENT_SLOT_COUNT {
            let record = equipment_record(&plain, slot)?;
            if equipment_slot_is_empty(record) {
                continue;
            }
            let replayed = replays_from_seed(data_root, record, progress.as_ref());
            if record[0x1A] & 0x10 == 0 {
                normal += 1;
                normal_matched += usize::from(replayed);
                continue;
            }
            hell += 1;
            hell_matched += usize::from(replayed);
            let item_id = u16::from_le_bytes([record[0], record[1]]);
            let seed = u16::from_le_bytes([record[0x22], record[0x23]]);
            let rarity = record[0x30];
            let level = u16::from_le_bytes([record[6], record[7]]);
            // What each played difficulty generates, for reading a mismatch.
            let generated: Vec<_> = progress
                .iter()
                .flat_map(|progress| {
                    progress.difficulties().into_iter().map(|difficulty| {
                        let state = PlayerState {
                            type_class: difficulty,
                            progress: progress.progress(difficulty),
                        };
                        let build = |hell: bool| {
                            if hell {
                                generator.prepare_hell(item_id, rarity, state)
                            } else {
                                generator.prepare(item_id, rarity, state)
                            }
                            .map_err(|error| format!("{error:?}"))
                            .and_then(|prepared| {
                                let entries = prepared.generate(seed);
                                prepared
                                    .build_record(&entries, seed, level, 0)
                                    .map_err(|error| format!("{error:?}"))
                            })
                            .map(|built| effects(&built))
                            .unwrap_or_default()
                        };
                        serde_json::json!({
                            "difficulty": difficulty,
                            "effects": build(true),
                            "normal_route": build(false),
                        })
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "save": save,
                    "slot": slot,
                    "item_id": item_id,
                    "seed": seed,
                    "rarity": rarity,
                    "level": level,
                    "skill": u16::from_le_bytes([record[0x10], record[0x11]]),
                    "replayed": replayed,
                    "stored": effects(record),
                    "generated": generated,
                })
            );
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "summary": {
                "hell": hell, "hell_replayed": hell_matched,
                "normal": normal, "normal_replayed": normal_matched,
            }
        })
    );
    Ok(())
}
