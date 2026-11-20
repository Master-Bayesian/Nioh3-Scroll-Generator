//! Actual native-table/pipeline constraints with a deliberately constant score.
//! These synthetic records test search contracts, not an observed game loadout.
use super::*;
use crate::engine::ITEM;

const FREE_SET: u32 = 0x024C;
const FREE_EFFECT: u32 = 0xB24F;
const FIXED_SET: u32 = 0x0EF0;
const FIXED_EFFECT: u32 = 0xA7A1;
const REDUCTION_EFFECT: u32 = 0x53D4;

fn engine() -> Engine {
    let mut e = Engine::audit_fixture();
    // The fixture's mode is one; put the searched equipment in that bank.
    e.audit_character().active = 1;
    e
}

fn record(key: u16, effects: &[(u32, u32)]) -> [u8; ITEM] {
    let mut item = [0; ITEM];
    item[..2].copy_from_slice(&key.to_le_bytes());
    item[6..8].copy_from_slice(&1u16.to_le_bytes());
    for i in 0..7 {
        item[0x38 + 24 * i..0x3C + 24 * i].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    for (i, (effect, value)) in effects.iter().enumerate() {
        item[0x38 + 24 * i..0x3C + 24 * i].copy_from_slice(&effect.to_le_bytes());
        item[0x3C + 24 * i..0x40 + 24 * i].copy_from_slice(&value.to_le_bytes());
    }
    item
}

fn wear_context(e: &mut Engine, slot: usize, item: [u8; ITEM]) {
    let c = e.audit_character();
    let at = c.active * 0xF70 + slot * ITEM;
    c.inv[at..at + ITEM].copy_from_slice(&item);
}

fn request(key: u32, pieces: usize) -> Request {
    Request {
        sets: vec![SetRule {
            options: vec![SetOption { key, pieces }],
            at_least: 1,
        }],
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    }
}

fn candidate(e: &Engine, rows: &[(u32, u32)]) -> Candidate {
    let mut c = Candidate {
        attrs: e.start_attrs,
        items: [None; SLOTS],
    };
    for &(slot, row) in rows {
        c.items[position_of(slot).unwrap()] = Some(row);
    }
    c
}

fn native_count(e: &Engine, pack: &Backpack, c: &Candidate, key: u32) -> usize {
    // Changed attributes are a native collector control, not a legacy source
    // license. Production score/search assertions below retain the guard.
    let state = if c.attrs == e.start_attrs {
        e.equipment_set_state(c, pack, &mut e.work())
    } else {
        e.audit_native_set_state(c, pack)
    };
    state
        .unwrap()
        .pieces
        .into_iter()
        .find(|entry| entry.0 == key)
        .map_or(0, |entry| entry.1)
}

#[test]
fn partial_slot_search_counts_retained_native_equipment_before_pruning() {
    let mut e = engine();
    wear_context(&mut e, 9, record(0x117E, &[(FREE_EFFECT, 1)]));
    wear_context(&mut e, 4, record(0x0404, &[(FREE_EFFECT, 1)]));
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[(FREE_EFFECT, 1)]))]).unwrap();
    let search = Search::new(&e, &pack);
    let req = request(FREE_SET, 2);
    let c = candidate(&e, &[(4, 0)]);
    assert_eq!(native_count(&e, &pack, &c, FREE_SET), 2);
    let retained = e
        .retained_set_state(&pack, &e.start_attrs, &mut e.work())
        .unwrap();
    assert!(retained.pieces.contains(&(FREE_SET, 1)));
    let plan = search.plan(&req);
    assert_eq!(
        (plan.size(), plan.full_size),
        (1, 1),
        "a retained piece must not be pruned"
    );
    let report = search.run(&req);
    assert_eq!(
        (report.counts.answered, report.counts.outside_limits),
        (1, 0)
    );
    assert_eq!(report.best[0].0.candidate, c);
}

#[test]
fn a_single_accessory_alternative_keeps_both_context_slots_in_set_bounds() {
    let mut e = engine();
    for slot in [9, 10] {
        wear_context(&mut e, slot, record(0x117E, &[(FREE_EFFECT, 1)]));
    }
    let pack = Backpack::from_slots(&[(9, record(0x117E, &[]))]).unwrap();
    let search = Search::new(&e, &pack);
    let req = request(FREE_SET, 2);
    let plan = search.plan(&req);
    assert_eq!(plan.size(), 1);
    let report = search.run(&req);
    assert_eq!(report.counts.answered, 1);
    assert_eq!(report.best[0].0.candidate.items, [None; SLOTS]);
    assert_eq!(
        native_count(&e, &pack, &report.best[0].0.candidate, FREE_SET),
        2
    );
}

#[test]
fn duplicate_fixed_piece_does_not_satisfy_two_native_set_pieces() {
    let mut e = engine();
    let item = record(0xF5BB, &[(FIXED_EFFECT, 1)]);
    for slot in [9, 10] {
        wear_context(&mut e, slot, item);
    }
    let pack = Backpack::from_slots(&[(9, item), (9, item)]).unwrap();
    let c = candidate(&e, &[(9, 0), (10, 1)]);
    assert_eq!(native_count(&e, &pack, &c, FIXED_SET), 1);
    let search = Search::new(&e, &pack);
    let req = request(FIXED_SET, 2);
    assert_eq!(
        search.plan(&req).size(),
        2,
        "the planner is deliberately optimistic"
    );
    let report = search.run(&req);
    assert!(report.complete);
    assert!(report.best.is_empty());
    assert_eq!(
        (report.counts.answered, report.counts.outside_limits),
        (0, 2)
    );
    assert!(
        matches!(report.backend, Backend::Cpu(ref why) if why.contains("native set constraints"))
    );
}

#[test]
fn repeated_free_side_appearances_are_an_upper_bound_not_a_single_item_bit() {
    let mut e = engine();
    let item = record(
        0x0404,
        &[(FREE_EFFECT, 1), (FREE_EFFECT, 1), (FREE_EFFECT, 1)],
    );
    wear_context(&mut e, 4, item);
    let pack = Backpack::from_slots(&[(4, item)]).unwrap();
    let c = candidate(&e, &[(4, 0)]);
    assert_eq!(native_count(&e, &pack, &c, FREE_SET), 3);
    let search = Search::new(&e, &pack);
    let req = request(FREE_SET, 3);
    assert_eq!(search.plan(&req).size(), 1);
    assert_eq!(search.run(&req).counts.answered, 1);
}

#[test]
fn zero_valued_reduction_presence_matches_native_set_reward_admission() {
    let mut e = engine();
    wear_context(
        &mut e,
        4,
        record(0x0404, &[(FREE_EFFECT, 1), (REDUCTION_EFFECT, 0)]),
    );
    let pack = Backpack::from_slots(&[(
        4,
        record(0x0404, &[(FREE_EFFECT, 1), (REDUCTION_EFFECT, 0)]),
    )])
    .unwrap();
    let c = candidate(&e, &[(4, 0)]);
    let native = e.equipment_set_state(&c, &pack, &mut e.work()).unwrap();
    assert!(native.reduction);
    assert!(native.pieces.contains(&(FREE_SET, 1)));
    let search = Search::new(&e, &pack);
    let req = request(FREE_SET, 2);
    assert_eq!(search.plan(&req).size(), 1);
    assert_eq!(search.run(&req).counts.answered, 1);
}

#[test]
fn point_moves_recheck_native_qualification_without_licensing_legacy_source_scores() {
    let mut e = engine();
    // The actual table row F5BB requires attribute index5 >= 9.
    let pack = Backpack::from_slots(&[
        (9, record(0xF5BB, &[(FIXED_EFFECT, 1)])),
        (9, record(0x117E, &[])),
    ])
    .unwrap();
    wear_context(&mut e, 9, pack.pool[0]);
    wear_context(&mut e, 10, pack.pool[1]);
    let base = candidate(&e, &[(9, 0), (10, 1)]);
    let mut moved = base;
    moved.attrs[1] += moved.attrs[5] - 5;
    moved.attrs[5] = 5;
    assert_eq!(
        moved.attrs.iter().sum::<i32>(),
        base.attrs.iter().sum::<i32>()
    );
    assert_eq!(native_count(&e, &pack, &base, FIXED_SET), 1);
    assert_eq!(native_count(&e, &pack, &moved, FIXED_SET), 0);
    assert_eq!(
        e.evaluate(&moved, &pack, &Limits::NONE, &mut e.work()),
        Outcome::Refused(crate::pipeline::source_dependency::ATTRIBUTES)
    );
    let mut req = request(FIXED_SET, 1);
    req.move_points = true;
    let search = Search::new(&e, &pack);
    let (scores, counts) = search.evaluate_all(&[base, moved], &req, None, 0);
    assert_eq!(scores, [Some(1.0f32.to_bits()), None]);
    assert_eq!(
        (counts.answered, counts.refused, counts.outside_limits),
        (1, 1, 0)
    );
    assert!(!search.plan(&req).is_complete());
}

#[test]
fn unknown_retained_state_disables_pruning_and_reports_refusal_not_infeasibility() {
    let mut e = engine();
    let mut refused = record(0x0024, &[(FREE_EFFECT, 1)]);
    refused[0x18..0x1C].copy_from_slice(&0x40000u32.to_le_bytes());
    wear_context(&mut e, 6, refused);
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[]))]).unwrap();
    assert!(e
        .retained_set_state(&pack, &e.start_attrs, &mut e.work())
        .is_err());
    let search = Search::new(&e, &pack);
    let req = request(FREE_SET, 7);
    assert_eq!(
        search.plan(&req).size(),
        1,
        "unknown retained state is not a proof of infeasibility"
    );
    let report = search.run(&req);
    assert!(report.best.is_empty());
    assert_eq!(
        (report.counts.refused, report.counts.outside_limits),
        (1, 0)
    );
}

#[test]
fn constrained_card_entrypoint_refuses_before_kernel_and_budget_cannot_claim_completion() {
    let e = engine();
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[(FREE_EFFECT, 1)]))]).unwrap();
    let req = request(FREE_SET, 1);
    let search = Search::new(&e, &pack);
    assert!(search.plan(&req).prefers_bounded_cpu());
    assert!(search
        .run_complete_on_card(&req, 100, 100)
        .err()
        .unwrap()
        .contains("card ABI has no set predicate"));
    assert!(search.run_limited_by(&req, |_| 0).is_none());
}

#[test]
fn admitted_weapon_set_metadata_has_the_same_rows_as_its_piece_counts() {
    let e = engine();
    let pack = Backpack::from_slots(&[(0, record(0x00AB, &[(FREE_EFFECT, 1)]))]).unwrap();
    let search = Search::new(&e, &pack);
    assert_eq!(search.sets(), [(FREE_SET, vec![(0, 1)])]);
    assert_eq!(search.set_rows(FREE_SET), [0]);
    assert!(e
        .tables()
        .effect_lines(&pack.pool[search.set_rows(FREE_SET)[0] as usize])
        .iter()
        .any(|line| line.0 == FREE_EFFECT && line.2 == FREE_SET));
}

#[test]
fn moving_points_keeps_candidates_that_gain_native_set_qualification() {
    let mut e = engine();
    let transfer = e.start_attrs[5] - 5;
    e.start_attrs[1] += transfer;
    e.start_attrs[5] = 5;
    let pack = Backpack::from_slots(&[
        (9, record(0xF5BB, &[(FIXED_EFFECT, 1)])),
        (9, record(0x117E, &[])),
    ])
    .unwrap();
    wear_context(&mut e, 9, pack.pool[0]);
    wear_context(&mut e, 10, pack.pool[1]);
    let unqualified = candidate(&e, &[(9, 0), (10, 1)]);
    let mut qualified = unqualified;
    qualified.attrs[5] += 4;
    qualified.attrs[1] -= 4;
    assert_eq!(native_count(&e, &pack, &unqualified, FIXED_SET), 0);
    assert_eq!(native_count(&e, &pack, &qualified, FIXED_SET), 1);
    let mut req = request(FIXED_SET, 1);
    req.move_points = true;
    let search = Search::new(&e, &pack);
    let plan = search.plan(&req);
    assert!(
        plan.size() > 0,
        "start attributes cannot prove future set infeasibility"
    );
    let (scores, counts) = search.evaluate_all(&[unqualified, qualified], &req, None, 0);
    assert_eq!(scores, [None, None]);
    assert_eq!(
        (counts.answered, counts.refused, counts.outside_limits),
        (0, 1, 1),
        "unknown changed-attribute sources are refused before claiming eligibility or a score"
    );
    assert!(
        !plan.is_complete(),
        "attribute screening still is not a global search"
    );
}

#[test]
fn a_card_sized_admission_does_not_bypass_the_cpu_fallback_budget() {
    let e = engine();
    let pack = Backpack::from_slots(&[(4, record(0x0404, &[(FREE_EFFECT, 1)]))]).unwrap();
    let req = request(FREE_SET, 1);
    let search = Search::new(&e, &pack).with_cpu_fallback_limit(0);
    let (_, report) = search.run_limited_by(&req, |_| 100).unwrap();
    assert!(matches!(
        report.failure,
        Some(SearchFailure::CpuFallbackBudget {
            planned: 1,
            limit: 0,
            ..
        })
    ));
    assert_eq!(report.searched, 0);
    assert_eq!(
        report.counts.answered
            + report.counts.refused
            + report.counts.no_path
            + report.counts.outside_limits,
        0
    );
    assert!(
        !report.complete && report.best.is_empty(),
        "failure is neither an optimum nor infeasibility"
    );
}
