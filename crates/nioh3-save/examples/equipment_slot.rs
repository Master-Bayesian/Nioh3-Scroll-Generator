//! Print one owned-equipment record of an encrypted save as hex.
//!
//! Usage: `cargo run --example equipment_slot -- <SAVEDATA.BIN> <slot>`

use nioh3_save::character::equipment_record;
use nioh3_save::crypto::decrypt_container;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: equipment_slot <SAVEDATA.BIN> <slot>")?;
    let slot: usize = args.next().ok_or("missing slot")?.parse()?;
    let plain = decrypt_container(&std::fs::read(&path)?)?;
    let record = equipment_record(&plain, slot)?;
    let hex: String = record.iter().map(|byte| format!("{byte:02x}")).collect();
    println!("{hex}");
    Ok(())
}
