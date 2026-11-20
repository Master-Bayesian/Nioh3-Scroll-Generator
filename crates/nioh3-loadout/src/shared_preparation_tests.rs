//! The production equipment/frame pipeline with the existing native-table
//! fixture and a deliberately constant formula oracle. These regressions
//! require the originating checkout's tests/fixtures/audit_tables.bin; they
//! are not live-game or original-MAIN damage evidence.
use super::*;

fn baseline(engine: &Engine) -> (Backpack, Candidate) {
    (
        Backpack::from_slots(&[]).unwrap(),
        Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        },
    )
}

#[test]
fn admitted_sets_share_one_equipment_run_and_keep_every_prepared_byte() {
    let engine = Engine::audit_fixture();
    let (pack, candidate) = baseline(&engine);
    let mut old = engine.work();
    let set_state = engine
        .equipment_set_state(&candidate, &pack, &mut old)
        .unwrap();
    engine
        .prepare_eligible(&candidate, &pack, &Limits::NONE, &mut old)
        .unwrap();
    assert_eq!(old.preparation_stats().pipeline_runs, 2);

    let mut shared = engine.work();
    let prepared = engine
        .prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut shared,
            Some(|state: &SetState| state == &set_state),
        )
        .unwrap();
    assert_eq!(prepared.work.character.inv, old.character.inv);
    assert_eq!(prepared.work.scratch.stat, old.scratch.stat);
    assert_eq!(prepared.work.scratch.d, old.scratch.d);
    assert_eq!(prepared.work.scratch.pred, old.scratch.pred);
    assert_eq!(prepared.work.scratch.writes, old.scratch.writes);
    assert_eq!(
        prepared.evaluate(),
        engine.evaluate_prepared(&mut old, None)
    );
    assert_eq!(
        shared.preparation_stats(),
        PreparationStats {
            pipeline_runs: 1,
            frame_preparations: 1,
            set_checks: 1,
            shared_pipeline_uses: 1,
        }
    );
}

#[test]
fn the_same_admitted_state_builds_the_exact_prepared_device_packet() {
    let engine = Engine::audit_fixture();
    let (pack, candidate) = baseline(&engine);
    let mut old = engine.work();
    engine
        .equipment_set_state(&candidate, &pack, &mut old)
        .unwrap();
    engine
        .prepare_eligible(&candidate, &pack, &Limits::NONE, &mut old)
        .unwrap();
    let expected = engine.prepared_device_contacts(&mut old).unwrap();
    let mut shared = engine.work();
    let actual = engine
        .prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut shared,
            Some(|_: &SetState| true),
        )
        .unwrap()
        .device_contacts()
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 1);
    assert_eq!(shared.preparation_stats().pipeline_runs, 1);
}

#[test]
fn no_set_rule_needs_no_extra_equipment_preparation_or_predicate() {
    let engine = Engine::audit_fixture();
    let (pack, candidate) = baseline(&engine);
    let mut work = engine.work();
    let result = engine
        .prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut work,
            None::<fn(&SetState) -> bool>,
        )
        .unwrap()
        .evaluate();
    assert_eq!(result, Outcome::Damage(1.0f32.to_bits()));
    assert_eq!(
        work.preparation_stats(),
        PreparationStats {
            pipeline_runs: 1,
            frame_preparations: 1,
            set_checks: 0,
            shared_pipeline_uses: 0,
        }
    );
}

#[test]
fn rejected_sets_precede_invalid_profile_operations_and_frame_writes() {
    let mut engine = Engine::audit_fixture();
    // The admission pipeline has no profile deltas. A later scoring pipeline
    // with this many groups must refuse the native 256-entry capacity.
    engine.ops = (0..=256).map(|group| (group, 1)).collect();
    let (pack, candidate) = baseline(&engine);
    let mut work = engine.work();
    assert!(matches!(
        engine.prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut work,
            Some(|_: &SetState| false)
        ),
        Err(Outcome::OutsideLimits)
    ));
    assert_eq!(
        work.preparation_stats(),
        PreparationStats {
            pipeline_runs: 1,
            frame_preparations: 0,
            set_checks: 1,
            shared_pipeline_uses: 0,
        }
    );
    work.reset_preparation_stats();
    assert!(matches!(
        engine.prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut work,
            Some(|_: &SetState| true)
        ),
        Err(Outcome::Refused("aggregate exceeds its native capacity"))
    ));
    assert_eq!(work.preparation_stats().pipeline_runs, 2);
    assert_eq!(work.preparation_stats().frame_preparations, 0);
}

#[test]
fn legacy_profile_operations_keep_the_old_second_pipeline_and_exact_writes() {
    let mut engine = Engine::audit_fixture();
    engine.ops = vec![(0x7F69, 37)];
    let (pack, candidate) = baseline(&engine);
    let mut old = engine.work();
    let state = engine
        .equipment_set_state(&candidate, &pack, &mut old)
        .unwrap();
    engine
        .prepare_eligible(&candidate, &pack, &Limits::NONE, &mut old)
        .unwrap();
    let mut shared = engine.work();
    let prepared = engine
        .prepare_for_search(
            &candidate,
            &pack,
            &Limits::NONE,
            &mut shared,
            Some(|candidate: &SetState| candidate == &state),
        )
        .unwrap();
    assert_eq!(prepared.work.scratch.writes, old.scratch.writes);
    assert_eq!(prepared.work.scratch.stat, old.scratch.stat);
    assert_eq!(
        prepared.evaluate(),
        engine.evaluate_prepared(&mut old, None)
    );
    assert_eq!(
        shared.preparation_stats(),
        PreparationStats {
            pipeline_runs: 2,
            frame_preparations: 1,
            set_checks: 1,
            shared_pipeline_uses: 0,
        }
    );
}

#[test]
fn source_refusal_precedes_set_rejection_and_cannot_leave_a_usable_token() {
    let engine = Engine::audit_fixture();
    let (pack, good) = baseline(&engine);
    let mut bad = good;
    bad.attrs[0] += 1;
    let mut work = engine.work();
    assert!(matches!(
        engine.prepare_for_search(
            &bad,
            &pack,
            &Limits::NONE,
            &mut work,
            Some(|_: &SetState| panic!("source refusal must precede set admission"))
        ),
        Err(Outcome::Refused(
            crate::pipeline::source_dependency::ATTRIBUTES
        ))
    ));
    assert_eq!(work.preparation_stats(), PreparationStats::default());
    let result = engine
        .prepare_for_search(
            &good,
            &pack,
            &Limits::NONE,
            &mut work,
            Some(|_: &SetState| true),
        )
        .unwrap()
        .evaluate();
    assert_eq!(result, Outcome::Damage(1.0f32.to_bits()));
    assert_eq!(work.preparation_stats().pipeline_runs, 1);
}

#[test]
fn ordinary_limits_still_follow_complete_frame_preparation() {
    let engine = Engine::audit_fixture();
    let (pack, candidate) = baseline(&engine);
    let mut work = engine.work();
    let limits = Limits {
        min_health: u32::MAX,
        ..Limits::NONE
    };
    assert!(matches!(
        engine.prepare_for_search(
            &candidate,
            &pack,
            &limits,
            &mut work,
            Some(|_: &SetState| true)
        ),
        Err(Outcome::OutsideLimits)
    ));
    assert_eq!(
        work.preparation_stats(),
        PreparationStats {
            pipeline_runs: 1,
            frame_preparations: 1,
            set_checks: 1,
            shared_pipeline_uses: 1,
        }
    );
}
