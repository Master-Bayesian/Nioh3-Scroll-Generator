use super::*;

fn fixture() -> (ActorInputs, u64, Vec<Vec<Word>>) {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/event_input_binding.json")).unwrap();
    let source = SourceIdentity {
        owner: v["source"]["owner"].as_u64().unwrap(),
        incarnation: v["source"]["incarnation"].as_u64().unwrap(),
    };
    let words = |key: &str| -> Vec<Word> {
        v[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| (w[0].as_u64().unwrap(), w[1].as_u64().unwrap() as u32))
            .collect()
    };
    let pair = |key: &str| {
        [
            v[key][0].as_u64().unwrap() as u32,
            v[key][1].as_u64().unwrap() as u32,
        ]
    };
    let status = SingleUseBinding {
        source,
        scalar_addresses: [
            v["scalar_addresses"][0].as_u64().unwrap(),
            v["scalar_addresses"][1].as_u64().unwrap(),
        ],
        active_bits: pair("active_bits"),
        rebuilt_bits: pair("rebuilt_bits"),
        active_membership: words("active_membership"),
        removed_membership: words("removed_membership"),
    };
    assert_eq!(status.active_bits, [1.06f32.to_bits(); 2]);
    assert_eq!(status.rebuilt_bits, [1.0f32.to_bits(); 2]);
    assert_eq!(v["active_status_keys"], serde_json::json!([363]));
    assert_eq!(v["current_status_keys"], serde_json::json!([]));
    let main = v["main_address"].as_u64().unwrap();
    let plans = v["plan_words"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            a.as_array()
                .unwrap()
                .iter()
                .map(|w| (w[0].as_u64().unwrap(), w[1].as_u64().unwrap() as u32))
                .collect()
        })
        .collect();
    (
        ActorInputs {
            source,
            epoch: 0,
            candidate_words: vec![
                (main + 8, 2179),
                (main + 0x14, 2),
                (main + 0x18, 456),
                (main + 0x298, 0x0001_0000),
            ],
            single_use: Some(status),
        },
        main,
        plans,
    )
}

fn event<'a>(
    actor: &ActorInputs,
    patches: &'a [WordPatch],
    packet: PacketPolicy,
) -> EventInput<'a> {
    EventInput {
        occurrence: "one",
        source: actor.source,
        epoch: actor.epoch,
        patches,
        packet,
        landed: Some(true),
        guard: Some(true),
        runtime_extensions_closed: true,
    }
}
fn last(words: &[Word], at: u64) -> u32 {
    words.iter().rev().find(|w| w.0 == at).unwrap().1
}
fn settled(elemental: i16) -> Settlement {
    Settlement {
        landed: true,
        source_present: true,
        hp_owner_present: true,
        packet_flags_296_298: [0; 3],
        actor_event_241: false,
        physical_motion: 0,
        elemental_motion: elemental,
    }
}
fn no_read(_: u64) -> Result<u32> {
    Err("unknown baseline")
}

#[test]
fn retained_six_percent_is_removed_from_membership_before_scalar_rebuild() {
    let (actor, _, plans) = fixture();
    let binding = actor.single_use.as_ref().unwrap();
    let patches: Vec<_> = plans[0]
        .iter()
        .map(|&(a, v)| WordPatch::full(a, v))
        .collect();
    let active = assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        NextHitPhase::Active,
        no_read,
    )
    .unwrap()
    .unwrap();
    let phase = NextHitPhase::Active.after_settlement(settled(12));
    assert_eq!(phase, NextHitPhase::RemovedPendingRebuild);
    let pending = assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        phase,
        no_read,
    )
    .unwrap()
    .unwrap();
    for at in binding.scalar_addresses {
        assert_eq!(last(&active.words, at), 1.06f32.to_bits());
        assert_eq!(last(&pending.words, at), 1.06f32.to_bits());
    }
    for &(at, value) in &binding.removed_membership {
        assert_eq!(last(&pending.words, at), value);
    }
    let rebuilt = actor.rebuilt(1, actor.candidate_words.clone()).unwrap();
    let consumed = assemble_event_inputs(
        &rebuilt,
        event(&rebuilt, &patches, PacketPolicy::PreparedActor),
        phase.after_rebuild(),
        no_read,
    )
    .unwrap()
    .unwrap();
    for at in binding.scalar_addresses {
        assert_eq!(last(&consumed.words, at), 1.0f32.to_bits());
    }
}

#[test]
fn current_consumed_snapshot_overrides_the_old_frozen_entry_words() {
    let (actor, _, plans) = fixture();
    let patches: Vec<_> = plans[0]
        .iter()
        .map(|&(a, v)| WordPatch::full(a, v))
        .collect();
    let current = assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        NextHitPhase::Inactive,
        no_read,
    )
    .unwrap()
    .unwrap();
    for at in actor.single_use.as_ref().unwrap().scalar_addresses {
        assert_eq!(last(&current.words, at), 1.0f32.to_bits());
    }
    assert!(actor
        .single_use
        .as_ref()
        .unwrap()
        .active_membership
        .iter()
        .any(|&(a, v)| last(&current.words, a) != v));
}

#[test]
fn each_contact_uses_fresh_candidate_and_its_own_source_packet() {
    let (actor, main, _) = fixture();
    let projectile = PacketPolicy::UnimbuedOwnerProjectile {
        main,
        descriptor_kind: 0,
        projectile: 0x90000,
        projectile_incarnation: 19,
        source_tag: 7,
        power: 100,
        main_flags: 0,
    };
    let a = assemble_event_inputs(
        &actor,
        event(&actor, &[], projectile),
        NextHitPhase::Inactive,
        no_read,
    )
    .unwrap()
    .unwrap();
    let b = assemble_event_inputs(
        &actor,
        event(&actor, &[], PacketPolicy::PreparedActor),
        NextHitPhase::Inactive,
        no_read,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        (
            last(&a.words, main + 0x14),
            last(&a.words, main + 0x18),
            last(&a.words, main + 0x298)
        ),
        (u32::MAX, 100, 0)
    );
    assert_eq!(
        (
            last(&b.words, main + 0x14),
            last(&b.words, main + 0x18),
            last(&b.words, main + 0x298)
        ),
        (2, 456, 0x10000)
    );
    assert_eq!(last(&a.words, main + 8), 2179);
    assert_eq!(last(&b.words, main + 8), 2179);
    assert_eq!(a.projectile, Some(0x90000));
    assert_eq!(b.projectile, None);
}

#[test]
fn mask_respects_last_writer_bytes_and_requires_known_baseline() {
    let source = SourceIdentity {
        owner: 10,
        incarnation: 1,
    };
    let actor = ActorInputs {
        source,
        epoch: 0,
        candidate_words: vec![(100, 0x11223344), (102, 0xAABBCCDD)],
        single_use: None,
    };
    let patches = [WordPatch {
        address: 100,
        value: 0x5500,
        mask: 0xFF00,
    }];
    let output = assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        NextHitPhase::Inactive,
        no_read,
    )
    .unwrap()
    .unwrap();
    assert_eq!(last(&output.words, 100), 0xCCDD5544);
    let patches = [WordPatch {
        address: 200,
        value: 0x6600,
        mask: 0xFF00,
    }];
    assert!(assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        NextHitPhase::Inactive,
        no_read
    )
    .is_err());
    let output = assemble_event_inputs(
        &actor,
        event(&actor, &patches, PacketPolicy::PreparedActor),
        NextHitPhase::Inactive,
        |_| Ok(0x12345678),
    )
    .unwrap()
    .unwrap();
    assert_eq!(last(&output.words, 200), 0x12346678);
}

#[test]
fn unknown_missed_wrong_epoch_and_recycled_owner_are_distinct() {
    let (actor, main, _) = fixture();
    let base = event(&actor, &[], PacketPolicy::PreparedActor);
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            landed: None,
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            guard: None,
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            runtime_extensions_closed: false,
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            epoch: 1,
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
    let source = SourceIdentity {
        incarnation: actor.source.incarnation + 1,
        ..actor.source
    };
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            source,
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
    assert!(assemble_event_inputs(
        &actor,
        EventInput {
            landed: Some(false),
            guard: Some(false),
            ..base.clone()
        },
        NextHitPhase::Active,
        no_read
    )
    .unwrap()
    .is_none());
    let packet = PacketPolicy::UnimbuedOwnerProjectile {
        main,
        descriptor_kind: 1,
        projectile: 20,
        projectile_incarnation: 19,
        source_tag: 0,
        power: 100,
        main_flags: 0,
    };
    assert!(assemble_event_inputs(
        &actor,
        EventInput { packet, ..base },
        NextHitPhase::Active,
        no_read
    )
    .is_err());
}

#[test]
fn settlement_conditions_include_elemental_and_do_not_consume_on_a_miss() {
    let active = NextHitPhase::Active;
    let good = settled(1);
    assert_eq!(
        active.after_settlement(good),
        NextHitPhase::RemovedPendingRebuild
    );
    for bad in [
        Settlement {
            landed: false,
            ..good
        },
        Settlement {
            source_present: false,
            ..good
        },
        Settlement {
            hp_owner_present: false,
            ..good
        },
        Settlement {
            actor_event_241: true,
            ..good
        },
        Settlement {
            packet_flags_296_298: [0, 0, 1],
            ..good
        },
        Settlement {
            elemental_motion: 0,
            ..good
        },
    ] {
        assert_eq!(active.after_settlement(bad), active);
    }
}
