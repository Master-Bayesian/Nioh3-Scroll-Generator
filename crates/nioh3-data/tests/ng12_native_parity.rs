//! NG1/NG2 certified materialization against live PC v2.02 native records.
//!
//! The fixture holds four native records per playthrough/rarity context from
//! the title-screen parity run (10000-seed reports in
//! `deliverables/v082-ce-research/ng12-parity`). Every byte must match except
//! the runtime header byte `+0x1B` and, for rarity 5, the documented header cap
//! (native `+0x30/+0x31` = 4/4 where the save record holds 5/5).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::install_materialize::{
    materialize_certified_install_record, materialize_certified_record,
};
use nioh3_domain::record::ScrollRecordBytes;

fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data")
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn assert_native(label: &str, native: &[u8], offline: &[u8], rarity: u8) {
    let differing: Vec<usize> = (0..native.len())
        .filter(|offset| native[*offset] != offline[*offset] && *offset != 0x1B)
        .collect();
    let header_cap = rarity == 5
        && differing == [0x30, 0x31]
        && native[0x30..0x32] == [4, 4]
        && offline[0x30..0x32] == [5, 5];
    assert!(
        differing.is_empty() || header_cap,
        "{label}: offline differs from the native record at {differing:02X?}"
    );
}

#[test]
fn ng1_and_ng2_certified_records_are_the_native_records() {
    let resource = nioh3_data::load_effect_resource_for_file_version(&data_root(), (2, 0, 2, 0))
        .expect("v2.02 resource");
    let index = EffectTableIndex::from_resource(&resource).expect("index");
    let fixture: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/ng12_native_records_v202.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut checked = 0;
    for context in fixture["contexts"].as_array().unwrap() {
        let playthrough = context["playthrough"].as_u64().unwrap() as u8;
        let rarity = context["rarity"].as_u64().unwrap() as u8;
        let level = context["level"].as_u64().unwrap() as u16;
        let recommended = context["recommended_level"].as_u64().unwrap() as u16;
        let template =
            ScrollRecordBytes::from_slice(&hex_bytes(context["template_hex"].as_str().unwrap()))
                .unwrap();
        for (seed, entry) in context["records"].as_object().unwrap() {
            let seed: u32 = seed.parse().unwrap();
            let label = format!("NG{playthrough} R{rarity} seed {seed}");
            let (install_native, final_native) = match entry.as_str() {
                Some(record) => (hex_bytes(record), hex_bytes(record)),
                None => (
                    hex_bytes(entry["stage"].as_str().unwrap()),
                    hex_bytes(entry["final"].as_str().unwrap()),
                ),
            };
            let serial = u32_at(&install_native, 0x28);
            let transfer = u32_at(&install_native, 0xDC);
            let (install, _) = materialize_certified_install_record(
                &index,
                playthrough,
                None,
                &template,
                rarity,
                seed,
                level,
                recommended,
                serial,
                transfer,
            )
            .expect("the install record materializes");
            assert_native(
                &format!("{label} install"),
                &install_native,
                install.as_bytes(),
                rarity,
            );
            let (finalized, _) = materialize_certified_record(
                &index,
                playthrough,
                None,
                &template,
                rarity,
                seed,
                level,
                recommended,
                serial,
                transfer,
            )
            .expect("the certified record materializes");
            assert_native(
                &format!("{label} final"),
                &final_native,
                finalized.as_bytes(),
                rarity,
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 24, "four records for each of the six contexts");
}
