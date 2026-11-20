//! Producer regressions bound to the supplied PC202 numeric table fixture.
//! These are host implementation tests, not a new live-game observation.

use super::*;

const ATTRS: [i32; 7] = [5, 45, 15, 200, 5, 60, 155];

fn ordinary_weapon(c: &mut Character, slot: usize) {
    let at = c.active * BANK + slot * ITEM;
    let item = &mut c.inv[at..at + ITEM];
    item.fill(0);
    item[..2].copy_from_slice(&0x00ABu16.to_le_bytes());
    item[6..8].copy_from_slice(&1u16.to_le_bytes());
    // No effect is supplied by this test item. Do not interpret zero as a
    // real effect identifier when checking the prepared-record producer.
    for i in 0..7 {
        put_u32(item, 0x38 + 24 * i, u32::MAX);
    }
}

#[test]
fn actual_ordinary_weapon_keeps_both_prepared_scalars_through_frame_writes() {
    let (t, f, mut c) = super::audit_tests::fixture();
    let row = t.items.row(0x00AB).unwrap();
    assert_eq!(row[0x182], 1);
    assert_eq!((u32_at(row, 0x10), u32_at(row, 0x14)), (59, 96));
    assert_eq!(u32_at(row, 0x58), 0x0D2F);
    ordinary_weapon(&mut c, 0);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    let prepared = (u32_at(&s.stat, 0x24), u32_at(&s.stat, 0x28));
    assert_ne!(
        prepared.0, prepared.1,
        "real table row must distinguish the channels"
    );
    t.frame_writes(&f, &c, &ATTRS, 37, 0, &mut s).unwrap();
    assert_eq!(
        (u32_at(&s.pred, 0x24), u32_at(&s.pred, 0x28)),
        (prepared.0 + 37, prepared.1 + 37)
    );
}

#[test]
fn conditional_weapon_additions_survive_alongside_the_generic_materializer() {
    let (t, f, mut c) = super::audit_tests::fixture();
    ordinary_weapon(&mut c, 0);
    ordinary_weapon(&mut c, 1);
    assert_eq!(t.class[0x8C47].0, STATEFUL);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[(0x6A7C, 3), (0x8C47, 7)], &mut s)
        .unwrap();
    assert_eq!((s.stat[0x61], s.stat[0xA1]), (0, 0));
    for at in [0x2C, 0x6C] {
        assert_eq!(u32_at(&s.stat, at), 10, "materialized STAT+{at:X}");
    }
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    for at in [0x2C, 0x6C] {
        assert_eq!(u32_at(&s.pred, at), 10, "final STAT+{at:X}");
    }
    // This source addresses only records0 and1.
    assert_eq!(u32_at(&s.pred, 0xAC), 3);

    // The second guard is independent of the first. Replacing just the
    // second item by the missing-weapon default sets its +3D byte to one.
    let second = c.active * BANK + ITEM;
    c.inv[second..second + 2].fill(0);
    t.pipeline(&f, &c, &ATTRS, &[(0x6A7C, 3), (0x8C47, 7)], &mut s)
        .unwrap();
    assert_eq!((s.stat[0x61], s.stat[0xA1]), (0, 1));
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!((u32_at(&s.pred, 0x2C), u32_at(&s.pred, 0x6C)), (10, 3));
}

#[test]
fn conditional_special_record_addition_is_not_replaced_by_generic_attack_sum() {
    let (t, f, c) = super::audit_tests::fixture();
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[(0x6A7C, 3)], &mut s).unwrap();
    assert_eq!(t.class[0x8703].0, STATEFUL);
    // Resolved precondition for this materializer, independent of inventory
    // admission: the fifth record is present and its +3D flag is zero.
    put_u32(&mut s.stat, 0x124, 41);
    s.stat[0x161] = 0;
    t.materialize_one(&c, &ATTRS, 0x8703, 9, &mut s.stat, &mut s.d)
        .unwrap();
    assert_eq!(u32_at(&s.stat, 0x12C), 12);
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!(u32_at(&s.pred, 0x12C), 12);

    // A false guard contributes nothing; it must not be turned into an
    // unconditional generic addition while preserving the first result.
    s.stat[0x161] = 1;
    t.materialize_one(&c, &ATTRS, 0x8703, 9, &mut s.stat, &mut s.d)
        .unwrap();
    assert_eq!(u32_at(&s.stat, 0x12C), 12);
}

#[test]
fn missing_fifth_item_does_not_reuse_captured_record_additions() {
    let (t, mut f, c) = super::audit_tests::fixture();
    for i in 0..5 {
        put_u32(&mut f.stat, 0x2C + 64 * i, 120 + i as u32);
    }
    assert_eq!(u16_at(&c.inv, c.active * BANK + 13 * ITEM), 0);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[(0x6A7C, 3)], &mut s).unwrap();
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    for i in 0..5 {
        assert_eq!(u32_at(&s.pred, 0x2C + 64 * i), 3, "record {i}");
    }
}

#[test]
fn signed_base_cancellation_preserves_real_max_duplicate_group_residue() {
    let (t, f, c) = super::audit_tests::fixture();
    let row = t.groups.row(0x2EB6).unwrap();
    assert_ne!(
        row[0x50] & 4,
        0,
        "bound group uses max-plus-duplicate aggregation"
    );
    let mut s = Scratch::new();
    s.source.mode = c.mode;
    for _ in 0..3 {
        t.aggregate(&mut s.source, c.mode, 0x2EB6, 10);
    }
    let bank = selected_map(c.mode, c.mode);
    let e = s.source.maps[bank][0];
    assert_eq!((e.key, e.base, e.dup), (0x2EB6, 10, 2));
    apply_operations(&mut s.source.maps[bank], &[(0x2EB6, -10)]).unwrap();
    assert_eq!(
        s.source.maps[bank].len(),
        1,
        "duplicate residue is still present"
    );
    let e = s.source.maps[bank][0];
    assert_eq!((e.base, e.dup), (0, 2));
    s.d.copy_from_slice(&f.d0);
    t.materialize_one(
        &c,
        &ATTRS,
        e.key,
        e.base.wrapping_add(e.dup as u32),
        &mut s.stat,
        &mut s.d,
    )
    .unwrap();
    assert_eq!(u32_at(&s.d, 0x2AC), 2);
}

#[test]
fn signed_overlay_keeps_existing_zero_entry_and_source_order_contract() {
    let mut entries = vec![
        Entry {
            key: 0x6A7C,
            base: 10,
            dup: 0,
        },
        Entry {
            key: 0x8C47,
            base: 7,
            dup: 0,
        },
    ];
    // Exact cancellation with no duplicate residue retains the established
    // removal behavior. A later explicit zero is still an entry, appended in
    // the supplied order rather than sorted or merged away as a no-op.
    apply_operations(
        &mut entries,
        &[(0x6A7C, -10), (0x6A7C, 0), (0x8703, 9), (0x8C47, -2)],
    )
    .unwrap();
    let observed: Vec<_> = entries.iter().map(|e| (e.key, e.base, e.dup)).collect();
    assert_eq!(observed, [(0x8C47, 5, 0), (0x6A7C, 0, 0), (0x8703, 9, 0)]);
}
