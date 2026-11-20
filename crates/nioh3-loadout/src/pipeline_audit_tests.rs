use super::*;

const ATTRS: [i32; 7] = [5, 45, 15, 200, 5, 60, 155];

#[test]
fn first_read_preserves_each_action_layout_when_replacing_player_values() -> R<()> {
    let projectile: Vec<u8> = [0u32, 0, 0, 1, 100, 0, 1, 100, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for policies in [
        vec![("packet_kind", &[2u8, 0][..])],
        vec![("aad_packet_kind", &[1][..]), ("hit_packets", &projectile)],
        vec![("held_reaper", &[1][..])],
    ] {
        let mut layout = audit_frame_bytes();
        for (name, value) in policies {
            layout.extend((name.len() as u32).to_le_bytes());
            layout.extend(name.as_bytes());
            layout.extend((value.len() as u32).to_le_bytes());
            layout.extend(value);
        }
        let declared = Frame::load(&layout)?;
        let private = Frame::load(&Frame::first_use_frame(
            &layout,
            &[1; 0x900],
            &[2; 0x900],
            &[3; 0xb00],
            &[4; 0xb00],
            [100, 200, 300],
            [4, 5],
        )?)?;
        assert!(declared.same_layout(&private));
        assert_eq!(
            (
                private.main8,
                private.hp_base,
                private.baseline_defense_sources
            ),
            (100, 200, 300)
        );
        assert_eq!(
            (
                private.stat[0],
                private.s_base[0],
                private.d_base[0],
                private.d0[0]
            ),
            (1, 2, 3, 4)
        );
    }
    Ok(())
}

pub(super) fn fixture() -> (Tables, Frame, Character) {
    // Losslessly reconstructed from the supplied NUMERIC_TABLES.json.gz.
    // SHA256: 922ba8fa5c1089538c1f075e5600cc65a8310ee4150d12eb4aa34610f3f804c2.
    let tables = Tables::load(include_bytes!("../tests/fixtures/audit_tables.bin")).unwrap();
    let c = Character {
        inv: vec![0; 0x2018],
        mode: 1,
        active: 0,
        cap: 180,
        plus_cap: 20,
        style: 1,
        unarmed: 0,
        level: 100,
        core_mode: 0,
        core_maps: [Vec::new(), Vec::new()],
    };
    let mut f = Frame {
        stat: vec![0; 0x900],
        d0: vec![0; 0xB00],
        s_base: vec![0; 0x900],
        d_base: vec![0; 0xB00],
        d_base_offset: 0,
        main8: 0,
        packet_kind: PacketKind::Weapon,
        hit_packets: vec![],
        runtime_hit_inputs: None,
        weapon_element_packet: true,
        stat_addr: 0x100000,
        d_addr: 0x200000,
        main_addr: 0x300000,
        ext_addr: 0x400000,
        owner: 0x500000,
        owner_resource: None,
        actor_agility: None,
        native_before: vec![],
        native_after: vec![],
        source_rebuild: None,
        source_baseline_ready: true,
        attr: 0x600000,
        dest: 0x700000,
        hp_base: 0,
        baseline_defense_sources: 0,
        held_reaper: true,
        identity_ki_seed: true,
    };
    put_u32(&mut f.stat, 0x848, 45);
    put_u32(&mut f.d0, 0x608, u32::MAX);
    put_u32(&mut f.d0, 0x2AC, u32::MAX);
    let mut s = Scratch::new();
    tables.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    f.s_base = s.stat.clone();
    f.stat = s.stat;
    f.d_base = s.d;
    (tables, f, c)
}

#[test]
fn heart_table_seed_precedes_additive_ki_in_the_actual_pipeline() {
    let (t, f, c) = fixture();
    // These row90 values come directly from the static resource, not 1220 scaling.
    for (heart, seed) in [
        (1, 920),
        (5, 960),
        (10, 1040),
        (20, 1125),
        (44, 1217),
        (45, 1220),
        (46, 1223),
        (50, 1237),
        (99, 1350),
        (200, 1390),
        (999, 1390),
    ] {
        let mut attrs = ATTRS;
        attrs[1] = heart;
        for contribution in [0, 1, 37, 400] {
            let mut s = Scratch::new();
            t.pipeline(&f, &c, &attrs, &[(0x7F69, contribution)], &mut s)
                .unwrap();
            t.frame_writes(&f, &c, &attrs, 0, 0, &mut s).unwrap();
            assert_eq!(u32_at(&s.pred, 0x1C), seed + contribution as u32);
        }
    }
}

#[test]
fn materialized_capacity_recovery_and_integer_sources_survive_frame_writes() {
    let (t, f, c) = fixture();
    let mut s = Scratch::new();
    t.pipeline(
        &f,
        &c,
        &ATTRS,
        &[(0xB3FE, 100), (0xB148, 30), (0x7FAC, 7), (0x3AF1, 9)],
        &mut s,
    )
    .unwrap();
    assert_eq!(u32_at(&s.stat, 0x7B4), 190); // 173 + trunc32(173 * 100 * .001).
    assert_eq!(u32_at(&s.stat, 0x1BC), 17); // row90(5)+16 = 10, then +7.
    assert_eq!(u32_at(&s.stat, 0x1C0), 19);
    assert_ne!(u32_at(&s.stat, 0x7A4), u32_at(&f.s_base, 0x7A4));
    let expected: Vec<_> = [0x7B4, 0x7A4, 0x7A8, 0x1BC, 0x1C0]
        .into_iter()
        .map(|at| (at, u32_at(&s.stat, at)))
        .collect();
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    for (at, bits) in expected {
        assert_eq!(u32_at(&s.pred, at), bits, "STAT+{at:X}");
    }
    put_u32(&mut s.stat, 0x7B8, 95);
    assert_eq!(t.load_of(&c, &ATTRS, &s.stat), 0.5);
}

#[test]
fn attribute_world_contract_is_explicit_and_part_of_context_compatibility() {
    let (t, mut f, c) = fixture();
    f.identity_ki_seed = false;
    let mut attrs = ATTRS;
    attrs[1] += 1;
    assert!(t
        .pipeline(&f, &c, &attrs, &[], &mut Scratch::new())
        .is_err());
    let (_, g, _) = fixture();
    assert!(!f.same_layout(&g));
    f.identity_ki_seed = true;
    f.held_reaper = false;
    assert!(!f.same_layout(&g));
}

#[test]
fn supplied_current_capture_keeps_distinct_primary_and_secondary_attack() {
    let captured = Frame::load(include_bytes!("../tests/fixtures/audit_frame.bin")).unwrap();
    assert_eq!(
        (u32_at(&captured.stat, 0x24), u32_at(&captured.stat, 0x28)),
        (2179, 2034)
    );
    assert_eq!(
        (
            u32_at(&captured.s_base, 0x24),
            u32_at(&captured.s_base, 0x28)
        ),
        (1831, 1686)
    );
    let (t, f, c) = fixture();
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    put_u32(&mut s.stat, 0x24, 1831);
    put_u32(&mut s.stat, 0x28, 1686);
    t.frame_writes(&f, &c, &ATTRS, 348, 0, &mut s).unwrap();
    assert_eq!((u32_at(&s.pred, 0x24), u32_at(&s.pred, 0x28)), (2179, 2034));
}

#[test]
fn post_materializer_bonus_is_applied_once_and_unknown_owner_curve_is_refused() {
    let (t, f, c) = fixture();
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    put_f32(&mut s.d, 0xA8, 0.25);
    put_f32(&mut s.d, 0x64, 0.5);
    let value = f32_at(&s.stat, 0x1AC);
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!(f32_at(&s.pred, 0x1AC), value + 0.25);
    assert_eq!(u32_at(&s.pred, 0x1C4), 936); // row90(155)+18 = 624; then +312.
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!(f32_at(&s.pred, 0x1AC), value + 0.25);
    put_u32(&mut s.d, 0x608, 0);
    assert!(t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).is_err());
}

#[test]
fn resource_curve_uses_the_observed_source_owner_and_signed_maximum() {
    let (t, mut f, c) = fixture();
    // The supplied resource01119 has 7A62 grade0 endpoints (0,0), (1000,21),
    // and grade17 endpoints (0,0), (1000,43). Integers truncate after sampling.
    for (current, maximum, grade, increment) in [
        (0.0f32, 100, 0, 0),
        (50.0, 100, 0, 10),
        (100.0, 100, 0, 21),
        (-3.0, 100, 0, 0),
        (140.0, 100, 0, 21),
        (1.0, 0, 0, 0),
        (1.0, -1, 0, 0),
        (50.0, 100, 17, 21),
        (100.0, 100, 999, 43),
    ] {
        f.owner_resource = Some((current.to_bits(), maximum));
        let mut s = Scratch::new();
        t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
        put_u32(&mut s.d, 0x608, grade);
        t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
        assert_eq!(u32_at(&s.pred, 0x1C4), 624 + increment);
    }
    f.owner_resource = Some((f32::NAN.to_bits(), 100));
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    put_u32(&mut s.d, 0x608, 0);
    assert!(t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).is_err());
}

#[test]
fn native_source_submissions_recheck_candidate_threshold_and_keep_duplicate_laws() {
    let (t, mut f, c) = fixture();
    f.native_before = vec![(0x7F69, 10), (0xF8B5, 6), (0x38BA, 7)];
    f.native_after = vec![(0x7F69, 11), (0x38BA, 3)];
    for (constitution, admitted) in [(5, false), (6, true)] {
        let mut attrs = ATTRS;
        attrs[0] = constitution;
        let mut s = Scratch::new();
        t.pipeline(&f, &c, &attrs, &[], &mut s).unwrap();
        let entries = &s.source.maps[1];
        assert_eq!(entries.iter().any(|e| e.key == 0xF8B5), admitted);
        let max = entries.iter().find(|e| e.key == 0x38BA).unwrap();
        assert_eq!((max.base, max.dup), (7, 0));
        assert_eq!(u32_at(&s.stat, 0x1C), 1241);
    }
    // Native max+duplicate is deliberately not addition of observed totals.
    let mut s = Scratch::new();
    s.source.mode = 1;
    t.aggregate(&mut s.source, 1, 0x037D, 10);
    t.aggregate(&mut s.source, 1, 0x037D, 3);
    let e = &s.source.maps[1][0];
    assert_eq!((e.base, e.dup), (10, 1));
}

#[test]
fn native_source_frame_program_replaces_prior_program_without_stale_sources() {
    let raw = include_bytes!("../tests/fixtures/audit_frame.bin");
    let first = Frame::with_native_sources(raw, &[(0x7F69, 0)], &[(0x38BA, 9)]).unwrap();
    let parsed = Frame::load(&first).unwrap();
    assert_eq!(parsed.native_before, [(0x7F69, 0)]);
    assert_eq!(parsed.native_after, [(0x38BA, 9)]);
    assert_eq!(Frame::with_native_sources(&first, &[], &[]).unwrap(), raw);
    assert!(Frame::with_native_sources(raw, &[(0, 1)], &[]).is_err());
}

#[test]
fn actor_agility_gate_reads_only_the_executed_native_branch() {
    let (t, mut f, c) = fixture();
    for (words, result) in [
        ([0, 0, 0, 0], None),
        ([1, 0, 0, 1], Some(false)),
        ([0, 0, 0, 1], None),
        ([0, 0, 0, 3], Some(false)),
        ([0, 1, 0, 3], None),
        ([0, 1, 1, 7], Some(false)),
        ([0x40, 1, 0, 7], Some(true)),
    ] {
        f.actor_agility = Some(words);
        assert_eq!(f.actor_minimum_agility().ok(), result);
        let mut s = Scratch::new();
        t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
        // Over-capacity is zero without the actor gate; both agility fields
        // are clamped to twenty when the actual native branch admits it.
        put_u32(&mut s.stat, 0x7B8, 10_000);
        let output = t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s);
        if let Some(active) = result {
            output.unwrap();
            let expected = if active { 20 } else { 0 };
            assert_eq!(
                (u32_at(&s.pred, 0x798), u32_at(&s.pred, 0x79C)),
                (expected, expected)
            );
        } else {
            assert!(output.is_err());
        }
    }
}

#[test]
fn empty_active_slot15_preserves_independent_reserve_zero_presence() {
    let (t, f, mut c) = fixture();
    c.active = 1;
    let item = &mut c.inv[15 * ITEM..16 * ITEM];
    item[..2].copy_from_slice(&0xE604u16.to_le_bytes());
    for ordinal in 0..7 {
        put_u32(item, 0x38 + 24 * ordinal, u32::MAX);
    }
    put_u32(item, 0x38 + 24 * 3, 0xF9BE);
    put_u32(item, 0x3C + 24 * 3, 0);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    let entry = s.source.maps[1].iter().find(|e| e.key == 0xF79F).unwrap();
    assert_eq!((entry.base, entry.dup), (0, 0));
}

#[test]
#[ignore = "invoked by validation/cpu_gpu_equivalence.py with explicit output files"]
fn export_cpu_gpu_vectors() {
    let (t, f, c) = fixture();
    let input = std::fs::read_to_string(std::env::var("NIOH3_AUDIT_CASES").unwrap()).unwrap();
    let mut output = Vec::new();
    for line in input.lines() {
        let v: Vec<u32> = line
            .split_whitespace()
            .map(|x| x.parse().unwrap())
            .collect();
        assert_eq!(v.len(), 7);
        let mut c = c.clone();
        if v[6] == 1 {
            for slot in 0..2 {
                let item = &mut c.inv[slot * ITEM..(slot + 1) * ITEM];
                item[..2].copy_from_slice(&0x00ABu16.to_le_bytes());
                item[6..8].copy_from_slice(&1u16.to_le_bytes());
                for effect in 0..7 {
                    put_u32(item, 0x38 + 24 * effect, u32::MAX);
                }
            }
        }
        let mut attrs = ATTRS;
        attrs[1] = v[0] as i32;
        let mut s = Scratch::new();
        let mut ops = vec![
            (0x7F69, v[1] as i64),
            (0xB3FE, v[2] as i64),
            (0xB148, v[3] as i64),
            (0x7FAC, 7),
            (0x3AF1, 9),
        ];
        if v[6] != 0 {
            ops.extend_from_slice(&[(0x6A7C, 3), (0x8C47, 7)]);
        }
        t.pipeline(&f, &c, &attrs, &ops, &mut s).unwrap();
        if v[6] == 2 {
            put_u32(&mut s.stat, 0x124, 41);
            s.stat[0x161] = 0;
            t.materialize_one(&c, &attrs, 0x8703, 9, &mut s.stat, &mut s.d)
                .unwrap();
        }
        put_u32(&mut s.d, 0x2AC, if v[4] != 0 { 0 } else { u32::MAX });
        put_u32(&mut s.d, 0x160, v[5]);
        t.frame_writes(&f, &c, &attrs, if v[6] != 0 { 37 } else { 0 }, 0, &mut s)
            .unwrap();
        output.extend_from_slice(&s.stat);
        output.extend_from_slice(&s.d);
        output.extend_from_slice(&s.pred);
        output.extend_from_slice(
            &crate::sustained::reaper_steps(&s.pred, &s.d)
                .unwrap()
                .to_le_bytes(),
        );
    }
    std::fs::write(std::env::var("NIOH3_AUDIT_RUST_OUTPUT").unwrap(), output).unwrap();
}
