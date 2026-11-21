//! Replay one retained outfit with the current scorer and original PC202 MAIN.
#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("Original PC202 MAIN replay requires Windows".into())
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use nioh3_loadout::{
        context::LoadoutContext,
        damage::{DamageDetails, DamageWords},
        engine::{Backpack, Candidate, Engine, SLOTS},
        native::Native,
    };
    use serde_json::json;
    use std::{ffi::OsString, path::Path};

    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let [bundle_path, context_path, original_path] = args.as_slice() else {
        return Err("usage: loadout_witness_replay BUNDLE CONTEXT ORIGINAL".into());
    };
    let context = LoadoutContext::load(Path::new(context_path))?;
    let engine = Engine::open_with_context(Path::new(bundle_path), &context)?;
    let mut original = Native::open(Path::new(original_path))?;
    let backpack = Backpack::from_slots(&[])?;
    let equipped = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let mut work = engine.work();
    let DamageDetails::Available { contacts, .. } =
        engine.damage_details(&equipped, &backpack, &mut work)
    else {
        return Err("The current scorer refused the retained outfit".into());
    };
    let (outfit_words, native_contacts) = engine.native_words(&equipped, &backpack, &mut work)?;
    if contacts.is_empty() || contacts.len() != native_contacts.len() {
        return Err("Scorer and original MAIN contact counts differ".into());
    }
    let mut replayed = Vec::with_capacity(contacts.len());
    for (index, (contact, words)) in contacts.iter().zip(native_contacts).enumerate() {
        let mut inputs = outfit_words.to_vec();
        inputs.extend(words);
        original.try_reset()?;
        original.write_words(&inputs);
        let oracle = original.run_components()?;
        let cpu = DamageWords {
            total: contact.total.to_bits(),
            physical: contact.physical.to_bits(),
            elemental: contact.elemental.to_bits(),
            reserved: 0,
        };
        replayed.push(json!({
            "index": index + 1,
            "cpu": {"total": contact.total, "physical": contact.physical, "elemental": contact.elemental},
            "original": {
                "total": f32::from_bits(oracle.total),
                "physical": f32::from_bits(oracle.physical),
                "elemental": f32::from_bits(oracle.elemental)
            },
            "bits_equal": cpu == oracle,
        }));
    }
    let receipt = json!({
        "schema": "nioh3-loadout-witness-replay/v1",
        "context_digest": engine.context_digest(),
        "input_digest": engine.bundle_input_digest(),
        "contacts": replayed,
    });
    println!("{}", receipt);
    Ok(())
}
