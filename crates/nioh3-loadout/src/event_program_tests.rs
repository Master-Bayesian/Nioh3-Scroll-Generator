//! Discriminating production Engine controls. The score oracle in these tests
//! is synthetic; source/resource/state execution is the actual production path.
use super::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn document(engine: &Engine) -> (Value, Vec<u8>) {
    let raw = b"owned test hit templates".to_vec();
    let layout: Vec<_> = engine
        .hits
        .addresses
        .iter()
        .flat_map(|a| a.to_le_bytes())
        .collect();
    let templates: Vec<_> = engine.hits.rows.iter().enumerate().map(|(i,_)| json!({
        "id":format!("fixture:{i}"),"landed":true,"guard":true,"packet":{"kind":"prepared_actor"},
        "rebuild_before":false,"scored":true,"identity":null,"settlement":null
    })).collect();
    (
        json!({"schema":"nioh3-production-event-plan/v1",
        "source":{"owner":engine.frame.source_owner_address(),"incarnation":17},"initial_epoch":4,
        "initial_phase":"inactive","main_address":engine.frame.main_address(),
        "template_layout_sha256":format!("{:x}",Sha256::digest(layout)),
        "hit_values_sha256":format!("{:x}",Sha256::digest(&raw)),"runtime_extensions_closed":true,
        "mode":{"kind":"fixed_contacts"},"templates":templates,"single_use":null,
        "provenance":{"kind":"synthetic production contract control"}}),
        raw,
    )
}

fn compile(engine: &Engine, document: &Value, raw: &[u8]) -> Result<EventProgram, String> {
    EventProgram::compile(
        &serde_json::to_vec(document).unwrap(),
        &engine.frame,
        &engine.hits.addresses,
        &engine.hits.rows,
        raw,
        engine.context.as_ref(),
    )
}

fn action(id: u16, cost: i16, contacts: &[usize], before: Value) -> Value {
    let mut parameter = [0u8; 336];
    parameter[0xC..0xE].copy_from_slice(&id.to_le_bytes());
    parameter[0x16..0x18].copy_from_slice(&cost.to_le_bytes());
    json!({"resource":"fixture-resource","action":id,
        "parameter_hex":parameter.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        "parameter_sha256":format!("{:x}",Sha256::digest(parameter)),"contacts":contacts,
        "cost":{"kind":"resolved_multiplier","bits":1.0f32.to_bits()},"before":before})
}

#[test]
fn production_sequence_scores_after_prerequisite_recovery_final_overspend_and_release() {
    let mut engine = Engine::audit_fixture();
    engine.hits.rows = vec![vec![0]; 3];
    let (mut doc, raw) = document(&engine);
    doc["templates"][0]["scored"] = json!(false);
    doc["mode"] = json!({"kind":"finite_sequence","initial_ki_bits":10.0f32.to_bits(),
        "ki_address":0x830000u64,"debit_address":0x830004u64,"actions":[
            action(100,6,&[0],json!([])),action(101,6,&[1,1],json!([])),
            action(102,6,&[2],json!([{"kind":"recovery","amount_bits":4.0f32.to_bits()},{"kind":"rebuild"}])),
            action(103,6,&[2],json!([{"kind":"stop","reason":"released"}]))]});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = engine.work();
    work.scratch.pred[0x1C..0x20].copy_from_slice(&100u32.to_le_bytes());
    let events = engine.execution_events(&work).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (
                e.template,
                e.scored,
                f32::from_bits(e.ki.unwrap().1),
                e.epoch
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, false, 4.0, 4),
            (1, true, -2.0, 4),
            (1, true, -2.0, 4),
            (2, true, -4.0, 5)
        ]
    );
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(3.0f32.to_bits())
    );
    assert!(!engine.event_program.as_ref().unwrap().gpu_compatible);
    assert!(!engine.supports_read_bounds());
    // Removing the resolved recovery cannot admit the third positive-cost
    // action merely because the preceding action's final overspend was legal.
    doc["mode"]["actions"][2]["before"] = json!([]);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(2.0f32.to_bits())
    );
}

#[test]
fn production_single_use_separates_zero_motion_elemental_removal_and_rebuild() {
    let mut engine = Engine::audit_fixture();
    engine.hits.addresses = vec![0x810000, 0x810004, 0x820000];
    let active = 1.06f32.to_bits();
    let inactive = 1.0f32.to_bits();
    engine.hits.rows = vec![
        vec![active, active, 1],
        vec![active, active, 1],
        vec![active, active, 0],
        vec![inactive, inactive, 0],
    ];
    engine.formula = Formula::audit_word(0x810000, active);
    let (mut doc, raw) = document(&engine);
    doc["initial_phase"] = json!("active");
    doc["single_use"] = json!({"scalar_addresses":[0x810000,0x810004],"active_bits":[active,active],
        "rebuilt_bits":[inactive,inactive],"active_membership":[[0x820000,1]],"removed_membership":[[0x820000,0]]});
    for i in 0..4 {
        doc["templates"][i]["settlement"] = json!({"source_present":true,"hp_owner_present":true,
        "packet_flags_296_298":[0,0,0],"actor_event_241":false,"physical_motion":0,"elemental_motion":if i==1{30}else{0}});
    }
    doc["templates"][3]["rebuild_before"] = json!(true);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = engine.work();
    let events = engine.execution_events(&work).unwrap();
    use crate::sequence::inputs::NextHitPhase::*;
    assert_eq!(
        events.iter().map(|e| e.phase).collect::<Vec<_>>(),
        vec![Active, Active, RemovedPendingRebuild, Inactive]
    );
    let expected = [1.06f32, 1.06, 1.06, 1.0]
        .into_iter()
        .fold(0.0f32, |sum, x| sum + x);
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(expected.to_bits())
    );
    assert!(engine.event_program.as_ref().unwrap().gpu_compatible);
    // A missing rebuild must preserve the old aggregate, although membership
    // was already removed. It cannot silently divide the total by 1.06.
    doc["templates"][3]["rebuild_before"] = json!(false);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_ne!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(expected.to_bits())
    );
    assert!(!engine.event_program.as_ref().unwrap().gpu_compatible);

    // A current consumed context is authoritative over the same frozen Active
    // plan. The entry snapshot cannot be resurrected after runtime refresh.
    engine.hits.rows[0] = vec![inactive, inactive, 0];
    doc["templates"][3]["rebuild_before"] = json!(true);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(engine.execution_events(&work).unwrap()[0].phase, Inactive);
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(4.0f32.to_bits())
    );
    assert_eq!(
        engine.event_plan_diagnostics().unwrap()["initial_phase"],
        "Inactive"
    );

    // Membership removed before aggregate rebuild is a distinct, valid start.
    engine.hits.rows[0] = vec![active, active, 0];
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(
        engine.execution_events(&work).unwrap()[0].phase,
        RemovedPendingRebuild
    );
    // An unknown aggregate is not fitted by dividing the old active snapshot.
    engine.hits.rows[0][0] = 1.08f32.to_bits();
    assert!(compile(&engine, &doc, &raw).is_err());
}

#[test]
fn production_packet_builder_restores_actor_after_owned_projectile() {
    let mut engine = Engine::audit_fixture();
    let main = engine.frame.main_address();
    engine.formula = Formula::audit_word(main + 0x18, 1.0f32.to_bits());
    // The descriptor overwrites both halves of an earlier object pointer.
    engine.hits.addresses = vec![main + 0x198, main + 0x19c, main + 0x198, main + 0x19c];
    engine.hits.rows = vec![vec![0x90004, 1, 0x90000, 0]; 3];
    engine.frame.hit_packets = vec![
        crate::pipeline::HitPacket::Prepared,
        crate::pipeline::HitPacket::UnimbuedOwnerProjectile {
            power: 2.0f32.to_bits(),
            main_flags: 0,
        },
        crate::pipeline::HitPacket::Prepared,
    ];
    let (mut doc, raw) = document(&engine);
    doc["templates"][1]["packet"] = json!({"kind":"unimbued_owner_projectile","object":0x90000,
        "incarnation":18,"owner":engine.frame.source_owner_address(),"descriptor_kind":0,
        "source_tag":7,"power":2.0f32.to_bits(),"main_flags":0});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = engine.work();
    work.scratch.writes = vec![(main + 0x18, 1.0f32.to_bits())];
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(4.0f32.to_bits())
    );
    for (field, value) in [
        ("incarnation", json!(0)),
        ("object", json!(0x90004)),
        ("object", json!(0x1_0009_0004u64)),
        ("descriptor_kind", json!(1)),
        ("owner", json!(7)),
    ] {
        let mut bad = doc.clone();
        bad["templates"][1]["packet"][field] = value;
        assert!(compile(&engine, &bad, &raw).is_err(), "{field}");
    }
}

#[test]
fn production_admission_rejects_unknown_scope_foreign_bytes_and_unbound_costs() {
    let engine = Engine::audit_fixture();
    let (doc, raw) = document(&engine);
    assert!(compile(&engine, &doc, &raw).is_ok());
    for (field, value) in [
        ("runtime_extensions_closed", json!(false)),
        ("source", json!({"owner":7,"incarnation":17})),
        ("hit_values_sha256", json!("00")),
        ("template_layout_sha256", json!("00")),
    ] {
        let mut bad = doc.clone();
        bad[field] = value;
        assert!(compile(&engine, &bad, &raw).is_err(), "{field}");
    }
    let mut bad = doc.clone();
    bad["templates"][0]["landed"] = Value::Null;
    assert!(compile(&engine, &bad, &raw).is_err());
    bad = doc.clone();
    bad["mode"] = json!({"kind":"finite_sequence","actions":[action(100,1,&[0],json!([]))]});
    bad["mode"]["actions"][0]["parameter_sha256"] = json!("00");
    assert!(compile(&engine, &bad, &raw).is_err());
}

fn held_work(engine: &Engine) -> Work {
    let mut work = engine.work();
    work.scratch.pred[0x1C..0x20].copy_from_slice(&1000u32.to_le_bytes());
    work.scratch.pred[0x79C..0x7A0].copy_from_slice(&75u32.to_le_bytes());
    work.scratch.d.fill(0);
    work.scratch.d[0x2AC..0x2B0].copy_from_slice(&(-1i32).to_le_bytes());
    assert_eq!(
        crate::sustained::reaper_steps(&work.scratch.pred, &work.scratch.d),
        Ok(12)
    );
    work
}

#[test]
fn held_late_cycle_consumption_is_rebuilt_for_every_actual_occurrence() {
    let mut engine = Engine::audit_fixture();
    engine.frame.held_reaper = true;
    let active = 1.06f32.to_bits();
    let inactive = 1.0f32.to_bits();
    engine.hits.addresses = vec![0x810000, 0x810004, 0x820000];
    engine.hits.rows = vec![vec![active, active, 1]; 3];
    engine.formula = Formula::audit_word(0x810000, active);
    let (mut doc, raw) = document(&engine);
    doc["mode"] = json!({"kind":"held_reaper"});
    doc["initial_phase"] = json!("active");
    doc["single_use"] = json!({"scalar_addresses":[0x810000,0x810004],"active_bits":[active,active],
        "rebuilt_bits":[inactive,inactive],"active_membership":[[0x820000,1]],"removed_membership":[[0x820000,0]]});
    for i in 0..3 {
        doc["templates"][i]["rebuild_before"] = json!(i == 1);
        doc["templates"][i]["settlement"] = json!({"source_present":true,"hp_owner_present":true,
            "packet_flags_296_298":[0,0,0],"actor_event_241":false,
            "physical_motion":0,"elemental_motion":if i == 2 { 30 } else { 0 }});
    }
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert!(!engine.reuses_held_templates());
    assert!(!engine.event_program.as_ref().unwrap().gpu_compatible);
    let mut work = held_work(&engine);
    let events = engine.execution_events(&work).unwrap();
    use crate::sequence::inputs::NextHitPhase::*;
    assert_eq!(events.len(), 12);
    assert_eq!(
        events[..5]
            .iter()
            .map(|e| (e.template, e.phase, e.epoch))
            .collect::<Vec<_>>(),
        vec![
            (0, Active, 4),
            (1, Active, 5),
            (2, Active, 5),
            (1, Inactive, 6),
            (2, Inactive, 6)
        ]
    );
    let expected = (0..12).fold(0.0f32, |sum, i| sum + if i < 3 { 1.06 } else { 1.0 });
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(expected.to_bits())
    );
    // Deliberately restore the old production optimization. The actual Engine
    // path now yields the wrong answer, rather than a vacuous assertion fault.
    engine
        .event_program
        .as_mut()
        .unwrap()
        .audit_force_held_template_reuse();
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(crate::sustained::sum_reaper([active; 3], 12))
    );
    assert_ne!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(expected.to_bits())
    );
}

#[test]
fn held_misses_and_unscored_prerequisites_are_not_cached_as_scored_contacts() {
    let mut engine = Engine::audit_fixture();
    engine.frame.held_reaper = true;
    engine.hits.rows = vec![vec![0]; 3];
    let (mut doc, raw) = document(&engine);
    doc["mode"] = json!({"kind":"held_reaper"});
    doc["templates"][0]["scored"] = json!(false);
    doc["templates"][1]["landed"] = json!(false);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = held_work(&engine);
    assert!(!engine.reuses_held_templates());
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(5.0f32.to_bits())
    );
    // Old reuse incorrectly counts the unscored entry once.
    engine
        .event_program
        .as_mut()
        .unwrap()
        .audit_force_held_template_reuse();
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(6.0f32.to_bits())
    );
}

#[test]
fn stationary_held_contract_retains_dense_gpu_eligibility_and_exact_cpu_sum() {
    let mut engine = Engine::audit_fixture();
    engine.frame.held_reaper = true;
    engine.hits.rows = vec![vec![0]; 3];
    let (mut doc, raw) = document(&engine);
    doc["mode"] = json!({"kind":"held_reaper"});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert!(engine.reuses_held_templates());
    assert!(engine.event_program.as_ref().unwrap().gpu_compatible);
    let mut work = held_work(&engine);
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(12.0f32.to_bits())
    );
    doc["initial_epoch"] = json!(u64::MAX - 3);
    doc["templates"][1]["rebuild_before"] = json!(true);
    doc["templates"][2]["rebuild_before"] = json!(true);
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Refused("event rebuild epoch overflow")
    );
}

#[test]
fn production_contact_boundary_targets_occurrence_and_stops_inside_a_multihit_action() {
    let mut engine = Engine::audit_fixture();
    let (mut doc, raw) = document(&engine);
    let mut repeated = action(100, 6, &[0, 0, 0], json!([]));
    repeated["contact_boundaries"] = json!([
        {"before_contact":1,"boundaries":[{"kind":"recovery","amount_bits":4.0f32.to_bits()},{"kind":"rebuild"}]},
        {"before_contact":2,"boundaries":[{"kind":"stop","reason":"interrupted"}]}]);
    doc["mode"] = json!({"kind":"finite_sequence","actions":[repeated],
        "initial_ki_bits":10.0f32.to_bits(),"ki_address":0x830000u64,"debit_address":0x830004u64});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = engine.work();
    work.scratch.pred[0x1C..0x20].copy_from_slice(&100u32.to_le_bytes());
    let events = engine.execution_events(&work).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (
                e.ordinal,
                e.template,
                e.epoch,
                f32::from_bits(e.ki.unwrap().1),
                f32::from_bits(e.debit.unwrap().1)
            ))
            .collect::<Vec<_>>(),
        vec![(0, 0, 4, 4.0, 6.0), (1, 0, 5, 8.0, 6.0)]
    );
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(2.0f32.to_bits())
    );
    doc["mode"]["actions"][0]["contact_boundaries"][1]["before_contact"] = json!(3);
    assert!(compile(&engine, &doc, &raw).is_err());
}

#[test]
fn production_generic_cycle_admits_last_overspend_and_preserves_repeated_templates() {
    let mut engine = Engine::audit_fixture();
    engine.hits.rows = vec![vec![0]; 3];
    let (mut doc, raw) = document(&engine);
    doc["mode"] = json!({"kind":"cyclic_sequence","entry":[action(100,2,&[0,0],json!([]))],
        "cycle":[action(101,2,&[1],json!([])),action(102,3,&[2,2],json!([]))],
        "max_cycles":null,"initial_ki_bits":6.0f32.to_bits(),"ki_address":0x830000u64});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    let mut work = engine.work();
    work.scratch.pred[0x1C..0x20].copy_from_slice(&100u32.to_le_bytes());
    let events = engine.execution_events(&work).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.ordinal, e.template, f32::from_bits(e.ki.unwrap().1)))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, 4.0),
            (1, 0, 4.0),
            (2, 1, 2.0),
            (3, 2, -1.0),
            (4, 2, -1.0)
        ]
    );
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(5.0f32.to_bits())
    );
    assert!(!engine.event_program.as_ref().unwrap().gpu_compatible);
    assert!(!engine.supports_read_bounds());
    // Explicit finite release limits admit zero-cost loops; an unbounded
    // nondepleting loop is unknown, never silently capped or scored as zero.
    doc["mode"] = json!({"kind":"cyclic_sequence","entry":[],
        "cycle":[action(103,0,&[0],json!([]))],"max_cycles":7});
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Damage(7.0f32.to_bits())
    );
    doc["mode"]["max_cycles"] = Value::Null;
    engine.event_program = Some(compile(&engine, &doc, &raw).unwrap());
    assert_eq!(
        engine.evaluate_prepared(&mut work, None),
        Outcome::Refused("unbounded sequence has no proven finite Ki depletion")
    );
}

#[test]
fn production_sequence_compiles_cost_and_identity_once_without_defaulting_unknowns() {
    let engine = Engine::audit_fixture();
    let (doc, raw) = document(&engine);
    let mut bad = doc.clone();
    bad["templates"][0]["identity"] =
        json!({"resource":"fixture","action":1,"parameter_sha256":42});
    assert!(compile(&engine, &bad, &raw).is_err());
    bad["templates"][0]["identity"]["parameter_sha256"] = json!("z".repeat(64));
    assert!(compile(&engine, &bad, &raw).is_err());
    let mut sequence = doc;
    sequence["mode"] = json!({"kind":"finite_sequence","actions":[action(100,6,&[0],json!([]))]});
    let mut wrong_action = sequence.clone();
    wrong_action["mode"]["actions"][0]["action"] = json!(101);
    assert!(compile(&engine, &wrong_action, &raw).is_err());
    let mut unknown_cost = sequence.clone();
    unknown_cost["mode"]["actions"][0]["cost"]["bits"] = json!(f32::NAN.to_bits());
    assert!(compile(&engine, &unknown_cost, &raw).is_err());
    let mut overlapping = sequence;
    overlapping["mode"]["ki_address"] = json!(0x830000u64);
    overlapping["mode"]["debit_address"] = json!(0x830002u64);
    assert!(compile(&engine, &overlapping, &raw).is_err());
}
