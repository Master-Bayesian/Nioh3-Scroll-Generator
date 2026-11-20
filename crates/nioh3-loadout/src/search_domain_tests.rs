//! Production planner/source-admission regressions. Item tables and equipment
//! qualification are real; the score fixture is deliberately constant and does
//! not claim native MAIN or runtime source-generation proof.
use super::*;
use crate::engine::ITEM;

const FREE_SET: u32 = 0x024C;
const FREE_EFFECT: u32 = 0xB24F;

fn record(key: u16, effects: &[(u32, u32)]) -> [u8; ITEM] {
    let mut item = [0; ITEM];
    item[..2].copy_from_slice(&key.to_le_bytes());
    item[6..8].copy_from_slice(&1u16.to_le_bytes());
    for line in item[0x38..0xE0].as_chunks_mut::<24>().0 {
        line[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    for (index, &(effect, value)) in effects.iter().enumerate() {
        let at = 0x38 + index * 24;
        item[at..at + 4].copy_from_slice(&effect.to_le_bytes());
        item[at + 4..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    item
}

fn engine() -> Engine {
    let mut engine = Engine::audit_fixture();
    let character = engine.audit_character();
    character.active = 1;
    // This fixture defines known empty positions, not unknown runtime bytes.
    character.inv.fill(0);
    wear(&mut engine, 0, record(0x00AB, &[]));
    engine
}

fn wear(engine: &mut Engine, slot: usize, item: [u8; ITEM]) {
    let character = engine.audit_character();
    let at = character.active * 0xF70 + slot * ITEM;
    character.inv[at..at + ITEM].copy_from_slice(&item);
}

fn request() -> Request {
    Request {
        sets: Vec::new(),
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    }
}

#[test]
fn type_source_partition_counts_uncomputed_before_set_pruning() {
    let engine = engine();
    let current = record(0x00AB, &[]);
    let mut changed = current;
    changed[0xA..0xC].copy_from_slice(&1u16.to_le_bytes());
    let pack = Backpack::from_slots(&[
        (0, current),
        (0, changed),
        (4, record(0x0404, &[])),
        (4, record(0x0404, &[])),
        (9, record(0x117E, &[])),
        (9, record(0x117E, &[])),
        (9, record(0x117E, &[])),
    ])
    .unwrap();
    let search = Search::new(&engine, &pack)
        .with_source_admission(SourceAdmissionPolicy::CapturedWeaponOnly);
    let plan = search.plan(&request());
    assert_eq!(
        (plan.full_size, plan.size(), plan.source_uncomputed),
        (24, 12, 12)
    );
    assert!(!plan.is_complete());
    assert!(plan
        .candidates(engine.start_attrs)
        .all(|candidate| candidate.items[9] == Some(0)));
    let report = search.run(&request());
    assert_eq!(
        (
            report.searched,
            report.counts.answered,
            report.source_uncomputed
        ),
        (12, 12, 12)
    );
    assert!(!report.complete);
    assert!(report.best.iter().all(|row| row.0.damage() == 1.0));

    let mut constrained = request();
    constrained.sets.push(SetRule {
        options: vec![SetOption {
            key: FREE_SET,
            pieces: 7,
        }],
        at_least: 1,
    });
    let pruned = search.plan(&constrained);
    assert_eq!(
        pruned.size(),
        0,
        "known admitted outfits cannot satisfy this rule"
    );
    assert_eq!(
        pruned.source_uncomputed, 12,
        "a set bound cannot erase unknown source candidates"
    );
    let report = search.run(&constrained);
    assert!(!report.complete && report.best.is_empty());
    assert_eq!((report.searched, report.source_uncomputed), (0, 12));

    let unrestricted = Search::new(&engine, &pack).plan(&request());
    assert_eq!(
        (unrestricted.size(), unrestricted.source_uncomputed),
        (24, 0)
    );
    assert!(
        unrestricted.is_complete(),
        "the existing admitted TYPE path is unchanged"
    );
}

#[test]
fn an_unproved_type_replacement_never_becomes_zero_damage_or_an_empty_complete_result() {
    let engine = engine();
    let mut changed = record(0x00AB, &[]);
    changed[0xA..0xC].copy_from_slice(&1u16.to_le_bytes());
    let pack = Backpack::from_slots(&[(0, changed)]).unwrap();
    let search = Search::new(&engine, &pack)
        .with_source_admission(SourceAdmissionPolicy::CapturedWeaponOnly);
    let report = search.run(&request());
    assert_eq!(
        (report.full_size, report.searched, report.source_uncomputed),
        (1, 0, 1)
    );
    assert!(report.best.is_empty() && !report.complete);
    assert_eq!(report.counts, Counts::default());

    let search = search.with_current_choices(CurrentChoicePolicy::Captured);
    let report = search.run(&request());
    assert_eq!(
        (report.full_size, report.searched, report.source_uncomputed),
        (2, 1, 1)
    );
    assert_eq!(report.counts.answered, 1);
    assert_eq!(report.best[0].0.candidate.items[9], None);
    assert_eq!(report.best[0].0.damage(), 1.0);
    assert!(!report.complete);
    assert!(search
        .run_complete_on_card(&request(), 100, 100)
        .err()
        .unwrap()
        .contains("uncomputed weapon sources"));
}

#[test]
fn hypothetical_enhanced_weapon_does_not_acquire_captured_source_admission() {
    let mut engine = engine();
    let mut enhanced = record(0x00AB, &[]);
    enhanced[6..8].copy_from_slice(&2u16.to_le_bytes());
    wear(&mut engine, 0, enhanced);
    engine.enhance(&mut enhanced);
    assert_ne!(enhanced.as_slice(), engine.worn_item(9));
    let pack = Backpack::from_slots(&[(0, enhanced)]).unwrap();
    let search = Search::new(&engine, &pack)
        .with_source_admission(SourceAdmissionPolicy::CapturedWeaponOnly)
        .with_current_choices(CurrentChoicePolicy::Enhanced);
    let plan = search.plan(&request());
    assert_eq!(
        (plan.full_size, plan.size(), plan.source_uncomputed),
        (2, 1, 1)
    );
    assert!(plan
        .candidates(engine.start_attrs)
        .all(|candidate| candidate.items[9].is_none()));
}

#[test]
fn one_byte_identical_pool_accessory_cannot_represent_two_distinct_current_items() {
    let mut engine = engine();
    let identical = record(0x117E, &[(FREE_EFFECT, 1)]);
    for slot in [9, 10] {
        wear(&mut engine, slot, identical);
    }
    let other = record(0xF5BB, &[]);
    let pack = Backpack::from_slots(&[(9, identical), (9, other)]).unwrap();
    let search = Search::new(&engine, &pack).with_current_choices(CurrentChoicePolicy::Captured);
    assert_eq!(search.current_accessories_missing(&[0, 1]), [false, true]);
    let plan = search.plan(&request());
    assert_eq!((plan.full_size, plan.size()), (4, 4));
    let [one, two] = ACCESSORY_SLOTS.map(|slot| position_of(slot).unwrap());
    let candidates: Vec<_> = plan.candidates(engine.start_attrs).collect();
    assert!(
        candidates
            .iter()
            .any(|c| c.items[one] == Some(0) && c.items[two].is_none()),
        "the current two owned items must remain representable"
    );
    let report = search.run(&request());
    assert_eq!(
        (report.counts.answered, report.counts.refused),
        (1, 3),
        "the current pair remains scored; changed unbound side state is now explicit refusal"
    );
    let current = candidates
        .iter()
        .find(|c| c.items[one] == Some(0) && c.items[two].is_none())
        .unwrap();
    let state = engine
        .equipment_set_state(current, &pack, &mut engine.work())
        .unwrap();
    assert!(state.pieces.contains(&(FREE_SET, 2)));
}

#[test]
fn retained_accessory_slots_are_not_collapsed_as_exchangeable_orientations() {
    let mut engine = engine();
    wear(&mut engine, 9, record(0x117E, &[(FREE_EFFECT, 1)]));
    wear(&mut engine, 10, record(0xF5BB, &[]));
    let mut alternative = record(0x117E, &[]);
    alternative[0xA..0xC].copy_from_slice(&1u16.to_le_bytes());
    let pack = Backpack::from_slots(&[(9, alternative)]).unwrap();
    let search = Search::new(&engine, &pack).with_current_choices(CurrentChoicePolicy::Captured);
    let report = search.run(&request());
    assert_eq!(
        (
            report.full_size,
            report.searched,
            report.counts.answered,
            report.counts.refused
        ),
        (3, 3, 2, 1)
    );
    // Exercise ranking canonicalization independently of the new source guard.
    // These are declared constant-oracle scores, not computed damage for the
    // one now-refused source state. None must retain its own captured slot.
    let ranked = search.ranked_facts(
        search
            .plan(&request())
            .candidates(engine.start_attrs)
            .map(|candidate| Scored {
                candidate,
                bits: 1.0f32.to_bits(),
            })
            .collect(),
        true,
    );
    assert_eq!(
        ranked.len(),
        3,
        "None in slot9 retains a different item from None in slot10"
    );
    let [one, two] = ACCESSORY_SLOTS.map(|slot| position_of(slot).unwrap());
    let retained_first = ranked
        .iter()
        .find(|r| r.0.candidate.items[one].is_none() && r.0.candidate.items[two] == Some(0))
        .unwrap();
    let retained_second = ranked
        .iter()
        .find(|r| r.0.candidate.items[one] == Some(0) && r.0.candidate.items[two].is_none())
        .unwrap();
    let a = engine.equipment_set_state(&retained_first.0.candidate, &pack, &mut engine.work());
    let b = engine.equipment_set_state(&retained_second.0.candidate, &pack, &mut engine.work());
    assert!(a.is_ok());
    assert_eq!(
        b.err(),
        Some(crate::pipeline::source_dependency::CHANGED),
        "native source production distinguishes the two retained-slot assignments"
    );
}

#[test]
fn limited_source_policy_cannot_be_bypassed_by_library_attribute_polish() {
    let engine = engine();
    let pack = Backpack::from_slots(&[(0, record(0x00AB, &[]))]).unwrap();
    let search = Search::new(&engine, &pack)
        .with_source_admission(SourceAdmissionPolicy::CapturedWeaponOnly);
    let mut request = request();
    request.move_points = true;
    let report = search.run(&request);
    assert_eq!(
        report.failure,
        Some(SearchFailure::SourceAttributesUnproven)
    );
    assert_eq!(report.searched, 0);
    assert!(report.best.is_empty() && !report.complete);
}

#[test]
fn uncomputed_cartesian_counts_cannot_overflow_or_round_through_a_tiny_admitted_plan() {
    let engine = engine();
    for (per_slot, accessories, saturates_u64) in [(100usize, 200usize, false), (350, 400, true)] {
        let mut changed = record(0x00AB, &[]);
        changed[0xA..0xC].copy_from_slice(&1u16.to_le_bytes());
        let mut items = Vec::new();
        for _ in 0..per_slot {
            items.push((0, changed));
        }
        for slot in 4..=8 {
            for _ in 0..per_slot {
                items.push((slot, record(0x0404, &[])));
            }
        }
        for _ in 0..accessories {
            items.push((9, record(0x117E, &[])));
        }
        assert!(
            items.len() <= 2500,
            "the ordinary request item limit permits this product"
        );
        let pack = Backpack::from_slots(&items).unwrap();
        let search = Search::new(&engine, &pack)
            .with_source_admission(SourceAdmissionPolicy::CapturedWeaponOnly);
        let plan = search.plan(&request());
        assert_eq!(
            plan.size(),
            0,
            "all requested weapon sources are uncomputed"
        );
        assert!(plan.full_size > MAX_EXACT_COVERAGE);
        assert_eq!(plan.full_size == u64::MAX, saturates_u64);
        let (_, report) = search
            .run_limited(&request(), 0)
            .expect("explicit count failure precedes execution budget");
        assert_eq!(report.failure, Some(SearchFailure::DomainCountTooLarge));
        assert_eq!(report.searched, 0);
        assert_eq!(report.counts, Counts::default());
        assert!(report.best.is_empty() && !report.complete);

        let admitted = Search::new(&engine, &pack);
        let plan = admitted.plan(&request());
        assert!(plan.full_size > MAX_EXACT_COVERAGE);
        assert_eq!(plan.source_uncomputed, 0);
        assert_eq!(
            plan.spaces(),
            0,
            "oversized admitted products must be refused before class traversal"
        );
        let (_, report) = admitted.run_limited(&request(), 0).unwrap();
        assert_eq!(report.failure, Some(SearchFailure::DomainCountTooLarge));
        assert_eq!(report.searched, 0);
        assert_eq!(report.counts, Counts::default());
    }
}

#[test]
fn dynamic_source_programs_preserve_rows_and_defer_unproved_set_bounds() {
    let mut engine = engine();
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[])), (4, record(0x0404, &[]))]).unwrap();
    let moving = request();
    assert_eq!(
        Search::new(&engine, &pack).build_plan(&moving, None).size(),
        1,
        "the legacy profile screen merges these equivalent local profiles"
    );
    let mut constrained = request();
    constrained.sets.push(SetRule {
        options: vec![SetOption {
            key: FREE_SET,
            pieces: 7,
        }],
        at_least: 1,
    });
    assert_eq!(Search::new(&engine, &pack).plan(&constrained).size(), 0);

    // A real Program activates the production capability branch. We only
    // plan here: the constant score fixture has no attested source baseline.
    let mut program = crate::pipeline::audit_source_program();
    // Bind the conditional program to this controlled inventory. The earlier
    // fixture had a different compiler category/selector; its pipeline refused
    // before producing retained state, which could hide a removed guard.
    *engine.audit_character() = crate::pipeline::audit_source_character();
    engine.audit_character().inv.fill(0);
    let mut weapon = record(0x00F1, &[]);
    weapon[0x33] = 1;
    wear(&mut engine, 0, weapon);
    let character = engine.audit_character();
    character.inv[0x1EE0..0x1EE4].copy_from_slice(&(character.active as u32).to_le_bytes());
    character.inv[character.active * 0xF70 + 0xF68] = 1;
    program.reference_inventory = character.inv.clone();
    program.validate().unwrap();
    engine.audit_source_program(program);
    assert!(engine.has_dynamic_sources());
    let retained = engine
        .retained_set_state(&pack, &engine.start_attrs, &mut engine.work())
        .expect(
            "the source pipeline must produce the actual retained-state input to the faulty bound",
        );
    assert!(!constrained.sets[0].holds(
        |key| retained
            .pieces
            .iter()
            .find(|entry| entry.0 == key)
            .map(|entry| entry.1),
        retained.reduction
    ));
    let search = Search::new(&engine, &pack);
    assert_eq!(
        search.build_plan(&moving, None).size(),
        2,
        "local dominance cannot order candidate guardian/core source effects"
    );
    assert_eq!(
        search.plan(&constrained).size(),
        2,
        "set constraints must use the exact per-candidate native predicate"
    );
}
