//! Offline-versus-native parity for one playthrough/rarity context.
//!
//! `playthrough_parity <data-root> <native-dump.json> [grace-map-cache.json]`
//!
//! The native dump holds records the running game generated in isolated
//! buffers for one template, playthrough, rarity and level (see the research
//! handoff). Each record is regenerated offline from the same template and
//! seed and compared byte for byte; `+0x1B` is a runtime header byte the native
//! assembly path clears and is reported separately, as in the NG3 gates.
//! Rarity 4 compares both the stage-one record and the finalized preview.
//! Rarity 4 and 5 need the context's captured Grace map (a runtime cache file).

use std::path::Path;

use serde_json::{json, Value};

use nioh3_domain::effect::{EffectTableIndex, GraceMap, GraceRange};
use nioh3_domain::install_materialize::{
    materialize_rarity3_record, materialize_rarity5_plain_record, materialize_rarity5_record,
};
use nioh3_domain::record::ScrollRecordBytes;
use nioh3_domain::sequence::{materialize_rarity4_final_record, materialize_rarity4_stage_one_record};

const RUNTIME_HEADER_OFFSET: usize = 0x1B;

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex"))
        .collect()
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn differences(native: &[u8], offline: &[u8]) -> (Vec<usize>, bool) {
    let offsets: Vec<usize> = (0..native.len())
        .filter(|offset| native[*offset] != offline[*offset])
        .collect();
    let runtime = offsets.contains(&RUNTIME_HEADER_OFFSET);
    (
        offsets
            .into_iter()
            .filter(|offset| *offset != RUNTIME_HEADER_OFFSET)
            .collect(),
        runtime,
    )
}

/// A research capture (`record_type`, `rarity`, `effect_slot`, `ranges`).
fn load_map(path: &Path, rarity: u8) -> GraceMap {
    let payload: Value = serde_json::from_slice(&std::fs::read(path).expect("map file")).expect("map json");
    let map = GraceMap {
        format: "nioh3-grace-first-u16-map-v2".to_string(),
        game_version: "2.02".to_string(),
        record_type: payload["record_type"].as_u64().expect("record_type") as u32,
        rarity: payload["rarity"].as_u64().expect("rarity") as u8,
        capture_state: format!("playthrough-{}-research", payload["playthrough"]),
        effect_slot: payload["effect_slot"].as_u64().expect("effect_slot") as u8,
        ranges: payload["ranges"]
            .as_array()
            .expect("ranges")
            .iter()
            .map(|range| GraceRange {
                start: range["start"].as_u64().expect("start") as u16,
                end: range["end"].as_u64().expect("end") as u16,
                effect_id: range["effect_id"].as_u64().expect("effect_id") as u32,
            })
            .collect(),
    };
    map.validate_partition().expect("dense map");
    assert_eq!(map.rarity, rarity, "the map belongs to another rarity");
    map
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data_root = Path::new(&args[1]);
    let dump: Value = serde_json::from_slice(&std::fs::read(&args[2]).expect("dump")).expect("dump json");
    let playthrough = dump["playthrough"].as_u64().expect("playthrough") as u8;
    let rarity = dump["rarity"].as_u64().expect("rarity") as u8;
    let level = dump["level"].as_u64().expect("level") as u16;
    let recommended = dump["recommended_level"].as_u64().expect("recommended") as u16;
    let template = ScrollRecordBytes::from_slice(&hex_bytes(dump["template_hex"].as_str().expect("template")))
        .expect("template record");
    let map = args.get(3).map(|path| load_map(Path::new(path), rarity));

    let resource = nioh3_data::load_effect_resource_for_file_version(data_root, (2, 0, 2, 0)).expect("v2.02 resource");
    let index = EffectTableIndex::from_resource(&resource).expect("index");

    let mut mismatches = 0usize;
    let mut runtime_only = 0usize;
    let mut header_cap = 0usize;
    let mut adopted = 0usize;
    let mut errors = 0usize;
    let mut samples = Vec::new();
    let records = dump["records"].as_object().expect("records");
    for (seed_text, entry) in records {
        let seed: u32 = seed_text.parse().expect("seed");
        // The native generator writes its own recommended level at +0x10; take
        // it so a level pair the game rewrites cannot mask the effect bytes.
        let first_native = hex_bytes(
            if rarity == 4 { entry["stage"].as_str() } else { entry.as_str() }.expect("record"),
        );
        let native_recommended = u16::from_le_bytes([first_native[0x10], first_native[0x11]]);
        if native_recommended != recommended {
            adopted += 1;
        }
        let recommended = native_recommended;
        let mut compare = |label: &str, native: &[u8], offline: Result<Vec<u8>, String>| match offline {
            Err(error) => {
                errors += 1;
                if samples.len() < 24 {
                    samples.push(json!({ "seed": seed, "stage": label, "error": error }));
                }
            }
            Ok(offline) => {
                let (mut offsets, runtime) = differences(native, &offline);
                // The documented rarity-5 header cap: the isolated native path
                // stores 4/4 at +0x30/+0x31 where the save record holds 5/5.
                if rarity == 5
                    && offsets == [0x30, 0x31]
                    && native[0x30..0x32] == [4, 4]
                    && offline[0x30..0x32] == [5, 5]
                {
                    header_cap += 1;
                    offsets.clear();
                }
                if runtime && offsets.is_empty() {
                    runtime_only += 1;
                }
                if !offsets.is_empty() {
                    mismatches += 1;
                    if samples.len() < 24 {
                        samples.push(json!({
                            "seed": seed,
                            "stage": label,
                            "offsets": offsets.iter().map(|offset| format!("0x{offset:02X}")).collect::<Vec<_>>(),
                            "native": hex(native),
                            "offline": hex(&offline),
                        }));
                    }
                }
            }
        };
        match rarity {
            3 => {
                let native = hex_bytes(entry.as_str().expect("record"));
                let offline = materialize_rarity3_record(
                    &index, playthrough, &template, seed, level, recommended,
                    u32_at(&native, 0x28), u32_at(&native, 0xDC),
                )
                .map(|(record, _)| record.as_bytes().to_vec())
                .map_err(|error| error.to_string());
                compare("record", &native, offline);
            }
            4 => {
                let stage = hex_bytes(entry["stage"].as_str().expect("stage"));
                let final_record = hex_bytes(entry["final"].as_str().expect("final"));
                let map = map.as_ref().expect("rarity 4 needs the stage-one map");
                let (serial, transfer) = (u32_at(&stage, 0x28), u32_at(&stage, 0xDC));
                let offline_stage = materialize_rarity4_stage_one_record(
                    &index, map, playthrough, &template, seed, level, recommended, serial, transfer,
                )
                .map(|(record, _)| record.as_bytes().to_vec())
                .map_err(|error| format!("{error:?}"));
                compare("stage", &stage, offline_stage);
                let offline_final = materialize_rarity4_final_record(
                    &index, map, playthrough, &template, seed, level, recommended, serial, transfer,
                )
                .map(|pair| pair.preview_record().as_bytes().to_vec())
                .map_err(|error| format!("{error:?}"));
                compare("final", &final_record, offline_final);
            }
            5 => {
                let native = hex_bytes(entry.as_str().expect("record"));
                let (serial, transfer) = (u32_at(&native, 0x28), u32_at(&native, 0xDC));
                // Without a map the context is the Grace-less NG1/NG2 layout.
                let offline = match map.as_ref() {
                    Some(map) => materialize_rarity5_record(
                        &index, map, playthrough, &template, seed, level, recommended, serial, transfer,
                    ),
                    None => materialize_rarity5_plain_record(
                        &index, playthrough, &template, seed, level, recommended, serial, transfer,
                    ),
                }
                .map(|(record, _)| record.as_bytes().to_vec())
                .map_err(|error| error.to_string());
                compare("record", &native, offline);
            }
            other => panic!("unsupported rarity {other}"),
        }
    }
    let report = json!({
        "schema": "claude-playthrough-parity/v1",
        "playthrough": playthrough,
        "record_type": dump["record_type"],
        "rarity": rarity,
        "level": level,
        "recommended_level": recommended,
        "seed_count": records.len(),
        "seed_set_sha256": dump["seed_set_sha256"],
        "native_process_id": dump["process_id"],
        "mismatch_count": mismatches,
        "error_count": errors,
        "runtime_header_only_count": runtime_only,
        "known_rarity5_header_cap_count": header_cap,
        "native_recommended_level_adopted_count": adopted,
        "parity_pass": mismatches == 0 && errors == 0,
        "samples": samples,
    });
    println!("{}", serde_json::to_string_pretty(&report).expect("report"));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}
