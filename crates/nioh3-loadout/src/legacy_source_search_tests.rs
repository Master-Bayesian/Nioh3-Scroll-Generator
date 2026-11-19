//! Guarded search production paths; score fixture is explicitly constant.
use super::*;
use crate::engine::ITEM;

fn request() -> Request {
    Request { sets: Vec::new(), limits: Limits::NONE, move_points: false, threads: 1 }
}

fn changed_head() -> [u8; ITEM] {
    let inventory = include_bytes!("../tests/fixtures/legacy_source_set5.bin");
    inventory[0xF70 + 4 * ITEM..0xF70 + 5 * ITEM].try_into().unwrap()
}

#[test]
fn risky_legacy_domains_preserve_rows_bounds_and_cpu_admission() {
    let engine = Engine::audit_legacy_source_fixture();
    let pack = Backpack::from_slots(&[(4, changed_head()), (4, changed_head())]).unwrap();
    let search = Search::new(&engine, &pack);
    assert!(search.legacy_pool_risk);
    let screened = search.build_plan(&request(), None);
    assert_eq!(screened.size(), 2, "unproved rows cannot disappear through local dominance");
    assert!(screened.prefers_bounded_cpu(), "risky large/screened plans cannot reach the old GPU source ABI");
    let mut constrained = request();
    constrained.sets.push(SetRule { options: vec![SetOption { key: 0x024C, pieces: 7 }], at_least: 1 });
    assert_eq!(search.plan(&constrained).size(), 2, "unproved source inputs cannot be pruned as set-infeasible");
    let report = search.run(&constrained);
    assert_eq!((report.searched, report.counts.refused, report.counts.outside_limits), (2, 2, 0));
    assert_eq!((report.counts.answered, report.source_uncomputed), (0, 0));
    assert!(report.best.is_empty() && !report.complete);
    assert!(matches!(report.backend, Backend::Cpu(ref reason) if reason.contains("shared-source")));
    assert!(search.run_complete_on_card(&request(), 10, 10).err().unwrap()
        .contains("legacy shared-source dependencies require guarded CPU evaluation"));
}

#[test]
fn mixed_safe_and_refused_outfits_never_publish_complete_or_zero_ranked_rows() {
    let engine = Engine::audit_legacy_source_fixture();
    let current: [u8; ITEM] = engine.worn_item(0).try_into().unwrap();
    let pack = Backpack::from_slots(&[(4, current), (4, changed_head())]).unwrap();
    let report = Search::new(&engine, &pack).run(&request());
    assert_eq!((report.full_size, report.searched, report.counts.answered, report.counts.refused), (2, 2, 1, 1));
    assert!(!report.complete, "enumerating a refused outfit is not complete answered coverage");
    assert_eq!(report.source_uncomputed, 0, "weapon-only source counts retain their declared meaning");
    assert_eq!(report.best.len(), 1);
    assert_eq!(report.best[0].0.damage(), 1.0);
    assert_eq!(report.best[0].0.candidate.items[0], Some(0));
}

#[test]
fn legacy_attribute_search_is_explicitly_unsupported_for_owned_type_too() {
    let engine = Engine::audit_legacy_source_fixture();
    let pack = Backpack::from_slots(&[]).unwrap();
    let mut moving = request();
    moving.move_points = true;
    let report = Search::new(&engine, &pack).run(&moving);
    assert_eq!(report.failure, Some(SearchFailure::SourceAttributesUnproven));
    assert_eq!(report.searched, 0);
    assert_eq!(report.counts, Counts::default());
    assert!(report.best.is_empty() && !report.complete);
    assert_eq!(engine.source_model_scope(), "legacy_unverified");
    let mut conditional = Engine::audit_fixture();
    conditional.audit_source_program(crate::pipeline::audit_source_program());
    assert_eq!(conditional.source_model_scope(), "conditional_rebuild");
}
