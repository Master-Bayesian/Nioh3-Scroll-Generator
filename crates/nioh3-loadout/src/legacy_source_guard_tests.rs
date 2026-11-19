//! Actual Engine admission with original native tables and controlled inputs.
//! The score oracle is explicitly constant; no original MAIN/GPU claim is made.
use super::*;
use crate::pipeline::source_dependency::{ATTRIBUTES, CHANGED};

fn current(engine: &Engine) -> Candidate {
    Candidate { attrs: engine.start_attrs, items: [None; SLOTS] }
}

fn case(engine: &Engine, inventory: &[u8]) -> (Backpack, Candidate) {
    let mut rows = Vec::new();
    for slot in 4..=10 {
        let at = engine.character.active * BANK + slot * ITEM;
        if inventory[at..at + ITEM] != engine.character.inv[at..at + ITEM] {
            rows.push((slot as u32, inventory[at..at + ITEM].try_into().unwrap()));
        }
    }
    let pack = Backpack::from_slots(&rows).unwrap();
    let mut candidate = current(engine);
    for &(slot, first, count) in &pack.ranges {
        assert_eq!(count, 1);
        candidate.items[position_of(slot).unwrap()] = Some(first);
    }
    (pack, candidate)
}

fn record(key: u16, effects: &[(u32, u32)]) -> [u8; ITEM] {
    let mut item = [0; ITEM];
    item[..2].copy_from_slice(&key.to_le_bytes());
    item[6..8].copy_from_slice(&1u16.to_le_bytes());
    item[0x30] = 5;
    for line in item[0x38..0xE0].as_chunks_mut::<24>().0 {
        line[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    for (index, &(effect, value)) in effects.iter().enumerate() {
        item[0x38 + 24 * index..0x3C + 24 * index].copy_from_slice(&effect.to_le_bytes());
        item[0x3C + 24 * index..0x40 + 24 * index].copy_from_slice(&value.to_le_bytes());
    }
    item
}

fn simple_engine() -> Engine {
    let mut engine = Engine::audit_fixture();
    engine.character.active = 1;
    engine.character.inv.fill(0);
    engine.character.inv[BANK..BANK + ITEM].copy_from_slice(&record(0x00AB, &[]));
    engine.character.inv[BANK + 0xF68] = 1;
    engine
}

#[test]
fn same_weapon_cd36_tiers_are_refused_before_scoring_or_set_admission() {
    let engine = Engine::audit_legacy_source_fixture();
    assert_eq!(engine.character.inv, include_bytes!("../tests/fixtures/legacy_source_current.bin"));
    let empty = Backpack::from_slots(&[]).unwrap();
    let baseline = current(&engine);
    assert_eq!(engine.evaluate(&baseline, &empty, &Limits::NONE, &mut engine.work()),
        Outcome::Damage(1.0f32.to_bits()));
    for (inventory, reduction) in [
        (include_bytes!("../tests/fixtures/legacy_source_set5.bin").as_slice(), false),
        (include_bytes!("../tests/fixtures/legacy_source_set5_reduction.bin").as_slice(), true),
    ] {
        let (pack, candidate) = case(&engine, inventory);
        assert_eq!(candidate.items[9], None, "the captured weapon is byte-identical");
        let mut work = engine.work();
        engine.wear(&candidate, &pack, &mut work);
        assert_eq!(&work.character.inv[BANK..BANK + ITEM], engine.worn_item(9));
        // Read the real set producer independently of the admission decision.
        engine.tables.pipeline(&engine.frame, &work.character, &candidate.attrs, &[], &mut work.scratch).unwrap();
        let entries = &work.scratch.source.maps[1];
        assert!(entries.iter().any(|e| e.key == 0x8508 && e.base == 0));
        assert_eq!(entries.iter().find(|e| e.key == 0xCB4B).map(|e| e.base), reduction.then_some(120));
        assert!(work.scratch.source.set_state().pieces.contains(&(0xCD36, 5)));
        assert_eq!(engine.evaluate(&candidate, &pack, &Limits::NONE, &mut work), Outcome::Refused(CHANGED));
        assert_eq!(engine.equipment_set_state(&candidate, &pack, &mut work), Err(CHANGED));
        assert_eq!(engine.equipment_groups(&candidate, &pack, &mut work), Err(CHANGED));
        assert_eq!(engine.native_words(&candidate, &pack, &mut work).err(), Some(CHANGED));

        // A successful constant-oracle read witness would otherwise match any
        // candidate. Source refusal must precede that cache lookup.
        let mut bounds = ReadBounds::new();
        assert!(matches!(engine.evaluate_with_bound(&baseline, &empty, &Limits::NONE,
            &mut work, &mut bounds), SweepOutcome::Executed(Outcome::Damage(_))));
        assert!(matches!(engine.evaluate_with_bound(&candidate, &pack, &Limits::NONE,
            &mut work, &mut bounds), SweepOutcome::Executed(Outcome::Refused(CHANGED))));
    }
}

#[test]
fn ordinary_existing_group_value_changes_keep_the_legacy_model_available() {
    let mut engine = simple_engine();
    let at = BANK + 4 * ITEM;
    engine.character.inv[at..at + ITEM].copy_from_slice(&record(0x0404, &[(0xF4, 13)]));
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[(0xF4, 20)]))]).unwrap();
    assert!(!engine.legacy_source_pool_risk(&pack));
    let baseline = current(&engine);
    let mut candidate = baseline;
    candidate.items[0] = Some(0);
    let word = |candidate: &Candidate| {
        let mut work = engine.work();
        let (words, _) = engine.native_words(candidate, &pack, &mut work).unwrap();
        words.iter().find(|(address, _)| *address == 0x100000 + 0x218).unwrap().1
    };
    assert_eq!(word(&candidate).wrapping_sub(word(&baseline)), 7,
        "real effectF4/groupECC1 changes the additive STAT218 recipe");
    assert_eq!(engine.evaluate(&candidate, &pack, &Limits::NONE, &mut engine.work()),
        Outcome::Damage(1.0f32.to_bits()));
    assert_eq!(engine.source_model_scope(), "legacy_unverified");
}

#[test]
fn zero_gate_presence_and_unqualified_raw_late_lines_are_guarded() {
    for (slot, key, effect) in [(4u32, 0x0404, 0xCB44), (4, 0x0404, 0x3779),
        (9, 0xAF66, 0x0FC7), (9, 0xAF66, 0x1667), (9, 0xAF66, 0x127C)] {
        let mut engine = simple_engine();
        if slot == 9 { engine.start_attrs[5] = 1; }
        let pack = Backpack::from_slots(&[(slot, record(key, &[(effect, 0)]))]).unwrap();
        let mut candidate = current(&engine);
        candidate.items[position_of(slot).unwrap()] = Some(0);
        if slot == 9 {
            let profile = engine.tables.profile(&engine.character, &candidate.attrs,
                &pack.pool[0], &HashSet::from([0x1EE2, 0xC437, 0xD12B])).unwrap();
            assert!(profile.values.is_empty(), "Skill1 fails the native accessory requirement9");
        }
        assert!(engine.legacy_source_pool_risk(&pack));
        assert_eq!(engine.evaluate(&candidate, &pack, &Limits::NONE, &mut engine.work()), Outcome::Refused(CHANGED));
    }
    let engine = simple_engine();
    let empty = Backpack::from_slots(&[]).unwrap();
    let mut changed = current(&engine);
    changed.attrs[0] += 1;
    assert_eq!(engine.evaluate(&changed, &empty, &Limits::NONE, &mut engine.work()), Outcome::Refused(ATTRIBUTES));
    assert_eq!(engine.equipment_set_state(&changed, &empty, &mut engine.work()), Err(ATTRIBUTES));
}

#[test]
fn unchanged_sensitive_values_do_not_license_changed_unbound_side_state() {
    let engine = simple_engine();
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[(0xB24F, 1)]))]).unwrap();
    let mut candidate = current(&engine);
    candidate.items[0] = Some(0);
    let mut work = engine.work();
    engine.wear(&candidate, &pack, &mut work);
    engine.tables.pipeline(&engine.frame, &work.character, &candidate.attrs, &[], &mut work.scratch).unwrap();
    assert!(!work.scratch.source.maps.iter().flatten().any(|e| matches!(e.key, 0x8508 | 0xCB4B | 0xD7BF)));
    assert!(work.scratch.source.set_state().pieces.contains(&(0x024C, 1)));
    assert_eq!(engine.evaluate(&candidate, &pack, &Limits::NONE, &mut work), Outcome::Refused(CHANGED),
        "unbound late core sides share native side ordering/capacity even below a visible gate tier");
}
