//! Print the currencies and occupied equipment count of one decrypted save.
//!
//! Usage: `cargo run --example character_report -- <decrypted SAVEDATA.BIN>`

use nioh3_save::character::{equipment_record, equipment_slot_is_empty, read_currency};
use nioh3_save::{Currency, EQUIPMENT_SLOT_COUNT};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: character_report <decrypted SAVEDATA.BIN>")?;
    let plain = std::fs::read(&path)?;
    for currency in Currency::ALL {
        match read_currency(&plain, currency) {
            Ok(value) => println!("{}: {value}", currency.label()),
            Err(error) => println!("{}: {error}", currency.label()),
        }
    }
    let mut occupied = 0;
    for slot in 0..EQUIPMENT_SLOT_COUNT {
        if !equipment_slot_is_empty(equipment_record(&plain, slot)?) {
            occupied += 1;
        }
    }
    println!("equipment: {occupied} occupied of {EQUIPMENT_SLOT_COUNT}");
    Ok(())
}
