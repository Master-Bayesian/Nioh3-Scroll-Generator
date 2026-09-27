//! Audit every owned equipment record of an encrypted save against the
//! natural-generation rules and print one JSON line per record.
//!
//! Usage: `cargo run --example equipment_audit -- <SAVEDATA.BIN> <data root>`

use std::path::Path;

use nioh3_protected::equipment_rules::audit_json;
use nioh3_save::character::{equipment_record, equipment_slot_is_empty};
use nioh3_save::crypto::decrypt_container;
use nioh3_save::EQUIPMENT_SLOT_COUNT;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let save = args
        .next()
        .ok_or("usage: equipment_audit <SAVEDATA.BIN> <data root>")?;
    let data_root = args.next().ok_or("missing data root")?;
    let plain = decrypt_container(&std::fs::read(save)?)?;
    for slot in 0..EQUIPMENT_SLOT_COUNT {
        let record = equipment_record(&plain, slot)?;
        if equipment_slot_is_empty(record) {
            continue;
        }
        let audit = audit_json(Path::new(&data_root), record);
        let hex: String = record.iter().map(|byte| format!("{byte:02x}")).collect();
        println!(
            "{}",
            serde_json::json!({ "slot": slot, "audit": audit, "record": hex })
        );
    }
    Ok(())
}
