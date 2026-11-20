//! Real production pipeline tests; controlled records are labelled explicitly.
use super::*;
use source_rebuild::{
    Binding, CharacterBinding, CoreInput, EvidenceScope, GuardianSubmission, Program, Submission,
    SCHEMA,
};

const ATTRS: [i32; 7] = [5, 45, 15, 200, 5, 60, 155];

fn item(key: u16, mask: u8, effect: Option<(u32, u32)>) -> Vec<u8> {
    let mut record = vec![0; ITEM];
    record[..2].copy_from_slice(&key.to_le_bytes());
    record[6..8].copy_from_slice(&1u16.to_le_bytes());
    record[0x33] = mask;
    for ordinal in 0..7 {
        put_u32(&mut record, 0x38 + 24 * ordinal, u32::MAX);
    }
    if let Some((key, value)) = effect {
        put_u32(&mut record, 0x38, key);
        put_u32(&mut record, 0x3c, value);
    }
    record
}

pub(super) fn fixture() -> (Tables, Frame, Character, Program) {
    let (t, f, mut c) = super::audit_tests::fixture();
    c.core_mode = c.mode;
    c.active = 1;
    put_u32(&mut c.inv, 0x1ee0, c.active as u32);
    c.inv[c.active * BANK + 0xf68] = 1;
    c.inv[..ITEM].copy_from_slice(&item(0x00f1, 2, None));
    c.inv[BANK..BANK + ITEM].copy_from_slice(&item(0x00f1, 1, None));
    let program = Program {
        schema: SCHEMA.into(),
        binding: Binding {
            scope: EvidenceScope::OfflineConditional,
            input_sha256: "a".repeat(64),
            tables_sha256: t.source_digest.clone(),
            generation: "explicit-controlled-source-generation".into(),
            form: c.mode,
            native_item_category: 0x33c9,
            compiler_return_rva: 0x3774ad,
            owner_kind: 0,
            item_sync: false,
            second_weapon_pass: false,
            runtime_input_sha256: None,
            epoch_receipt_sha256: None,
            character: CharacterBinding {
                active: c.active,
                cap: c.cap,
                plus_cap: c.plus_cap,
                style: c.style,
                unarmed: c.unarmed,
                level: c.level,
            },
            source_scalar_words: [0; 9],
        },
        prefix: vec![],
        active_guardian: 0,
        guardian_requirement: 100,
        guardians: vec![],
        cores: (0..8)
            .map(|_| CoreInput {
                record: item(0, 0, None),
                unlocked: false,
                effect_mask: 0,
            })
            .collect(),
        late_clan: std::array::from_fn(|_| Submission { group: 0, value: 0 }),
        post_inventory_projection: true,
        reference_inventory: c.inv.clone(),
        post_record_present: [[false; 4]; 2],
        baseline: None,
    };
    program.validate().unwrap();
    (t, f, c, program)
}

fn total(source: &Source, group: u16) -> Option<u32> {
    source.maps[1]
        .iter()
        .find(|e| e.key == group)
        .map(|e| e.base.wrapping_add(u32::from(e.dup)))
}

#[test]
fn source_rebuild_uses_fresh_core_then_selected_not_retained_aggregate() {
    let (t, mut f, mut c, program) = fixture();
    c.core_maps[1].push(Entry {
        key: 0xcb4b,
        base: 0,
        dup: 0,
    });
    let mut legacy = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut legacy).unwrap();
    f.source_rebuild = Some(program);
    let mut fresh = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut fresh).unwrap();
    assert_ne!(
        u32_at(&fresh.stat, 0x24),
        u32_at(&legacy.stat, 0x24),
        "stale inherited mask changes attack"
    );
    assert!(fresh.source_rebuild_trace.as_ref().unwrap().prefix[1].is_empty());
    assert_eq!(total(&fresh.source, 0xcb4b), None);

    // A newly owned body of the SAME category supplies zero-valued presence.
    c.inv[BANK..BANK + ITEM].copy_from_slice(&item(0x00f1, 1, Some((52036, 0))));
    t.pipeline(&f, &c, &ATTRS, &[], &mut fresh).unwrap();
    let trace = fresh.source_rebuild_trace.as_ref().unwrap();
    assert!(trace.prefix[1].is_empty());
    assert!(trace.completed_core[1]
        .iter()
        .any(|e| e.key == 0xcb4b && e.base == 0));
    assert_eq!(
        u32_at(&fresh.stat, 0x24),
        u32_at(&legacy.stat, 0x24),
        "selected must consume this candidate's completed core"
    );
}

#[test]
fn source_rebuild_guardian_gate_is_live_and_rechecked_within_stage() {
    let (t, mut f, c, mut p) = fixture();
    p.guardians = vec![
        GuardianSubmission {
            slot: 0,
            ordinal: 0,
            group: 0x8508,
            value: 0,
            requirement: 0,
        },
        GuardianSubmission {
            slot: 1,
            ordinal: 0,
            group: 0x7f69,
            value: 37,
            requirement: 0,
        },
    ];
    f.source_rebuild = Some(p.clone());
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(total(&s.source, 0x7f69), Some(37));
    p.active_guardian = 1;
    p.guardians[0].group = 0x7f69;
    p.guardians[0].value = 71;
    f.source_rebuild = Some(p.clone());
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(
        total(&s.source, 0x7f69),
        Some(37),
        "inactive guardian must not be flattened into fixed sources"
    );
    p.prefix.push(Submission {
        group: 0x8508,
        value: 0,
    });
    f.source_rebuild = Some(p);
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(total(&s.source, 0x7f69), Some(108));
}

#[test]
fn source_rebuild_all_eight_core_records_use_actual_collector_masks() {
    let (t, mut f, c, mut p) = fixture();
    for (slot, core) in p.cores.iter_mut().enumerate() {
        core.record = item(0x00ab, 0, Some((2610, 1 << slot)));
        core.unlocked = true;
        core.effect_mask = 1;
    }
    f.source_rebuild = Some(p.clone());
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(total(&s.source, 0x7f69), Some(255));
    p.cores[7].unlocked = false;
    p.cores[6].effect_mask = 0;
    f.source_rebuild = Some(p);
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(total(&s.source, 0x7f69), Some(63));
}

#[test]
fn source_rebuild_late_core_sides_cannot_qualify_the_earlier_set_stage() {
    let (t, mut f, c, mut p) = fixture();
    let (effect, side) = t
        .effects
        .direct
        .iter()
        .enumerate()
        .filter(|(_, index)| **index != ABSENT)
        .find_map(|(effect, _)| {
            let row = t.effects.row(effect as u32)?;
            let group = t.groups.row(u32::from(u16_at(row, 2)))?;
            let side = u32_at(group, 8);
            t.sides
                .row(side)
                .filter(|row| u32_at(row, 0x10) == 0)
                .map(|_| (effect as u32, side))
        })
        .expect("real fixture has a free set-side effect");
    p.cores[0] = CoreInput {
        record: item(0x00ab, 0, Some((effect, 1))),
        unlocked: true,
        effect_mask: 0,
    };
    f.source_rebuild = Some(p);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert!(s.source.sides[1]
        .iter()
        .any(|entry| u32_at(entry, 0) == side && u16_at(entry, 4) == 1));
    assert!(
        s.source.set_state().pieces.is_empty(),
        "post-set cores must not qualify earlier set rewards"
    );
}

#[test]
fn source_rebuild_late_groups_do_not_reenter_current_materialization() {
    let (t, mut f, c, mut p) = fixture();
    let mut baseline = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut baseline).unwrap();
    p.late_clan[0] = Submission {
        group: 0x1ee2,
        value: 7,
    };
    f.source_rebuild = Some(p);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(total(&s.source, 0x1ee2), Some(7));
    assert_eq!(s.stat, baseline.stat);
    assert_eq!(
        s.d, baseline.d,
        "late3F9FC0 source addition must not write current D+7C"
    );
}

#[test]
fn source_rebuild_late_inventory_sums_each_bank_then_takes_signed_max() {
    let (t, mut f, mut c, mut p) = fixture();
    for (bank, slot, value) in [(0, 9, 7), (0, 10, 5), (1, 9, 9), (1, 10, 2)] {
        let at = bank * BANK + slot * ITEM;
        c.inv[at..at + ITEM].copy_from_slice(&item(0, 0, Some((4039, value))));
    }
    p.reference_inventory = c.inv.clone();
    p.post_record_present = [[false, true, true, false]; 2];
    f.source_rebuild = Some(p);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert_eq!(
        total(&s.source, 0x1ee2),
        Some(12),
        "do not sum both banks or drop an unqualified raw item"
    );
}

#[test]
fn source_rebuild_refuses_mixed_unknown_or_different_generation_inputs() {
    let (t, mut f, mut c, p) = fixture();
    assert_eq!(p.binding.scope, EvidenceScope::OfflineConditional);
    f.source_rebuild = Some(p.clone());
    assert!(t
        .pipeline(&f, &c, &ATTRS, &[(0x7f69, 1)], &mut Scratch::new())
        .is_err());
    c.inv[0x14] ^= 1;
    assert!(t
        .pipeline(&f, &c, &ATTRS, &[], &mut Scratch::new())
        .is_err());
    c.inv[0x14] ^= 1;
    let mut changed = p.clone();
    changed.binding.tables_sha256 = "b".repeat(64);
    f.source_rebuild = Some(changed);
    assert!(t
        .pipeline(&f, &c, &ATTRS, &[], &mut Scratch::new())
        .is_err());
    let mut changed = p.clone();
    changed.cores[7].record.pop();
    assert!(changed.validate().is_err());
    let mut changed = p.clone();
    changed.binding.scope = EvidenceScope::RebuildEpoch;
    assert!(changed.validate().is_err());
    changed.binding.runtime_input_sha256 = Some("a".repeat(64));
    changed.binding.epoch_receipt_sha256 = Some("b".repeat(64));
    assert!(
        changed.validate().is_err(),
        "digest-shaped strings cannot authorize runtime ingestion"
    );
    let mut changed = p.clone();
    changed.binding.source_scalar_words[8] = 1;
    assert!(changed.validate().is_err());
    let mut changed = p;
    changed.post_inventory_projection = false;
    f.source_rebuild = Some(changed);
    assert!(t
        .pipeline(&f, &c, &ATTRS, &[], &mut Scratch::new())
        .is_err());
}

#[test]
fn source_rebuild_late_context_uses_inventory_bank_separately_from_owner_mode() {
    let (t, mut f, mut c, mut p) = fixture();
    c.mode = 0;
    c.core_mode = 0;
    p.binding.form = 0;
    p.late_clan[0] = Submission {
        group: 0x1ee2,
        value: 7,
    };
    f.source_rebuild = Some(p);
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert!(s.source.maps[0].iter().all(|e| e.key != 0x1ee2));
    assert_eq!(total(&s.source, 0x1ee2), Some(7));
}

#[test]
fn source_rebuild_baseline_is_rebuilt_before_frame_scoring() {
    let (t, mut f, mut c, mut p) = fixture();
    const WORD: usize = 0x218;
    const GROUP: u16 = 0xecc1;
    // Actual table recipe ECC1 adds u32 to STAT+218, which uses Add32 and is
    // not overwritten later by frame_writes. Weapon+2C uses Rebuilt and Ki+1C
    // uses an absolute identity write; neither tests predictor delta merging.
    assert_eq!(
        crate::stat_words::kind(WORD),
        crate::stat_words::StatWord::Add32
    );
    let (kind, start, count) = t.class[usize::from(GROUP)];
    assert_eq!(kind, RECIPES);
    assert!(t.recipes[start as usize..(start + count) as usize]
        .iter()
        .any(|recipe| recipe.kind == STAT_ADD_U32 && recipe.offset == WORD));
    // Explicit controlled observed frame, not a fitted runtime result. Both
    // predictors start from this same seed; the old predictor lacks the new
    // source program's37. The residual merge must preserve the observation.
    let observed_word = 1000;
    put_u32(&mut f.stat, WORD, observed_word);
    for (index, value) in ATTRS.iter().enumerate() {
        put_u32(&mut f.stat, 0x844 + 4 * index, *value as u32);
    }
    let mut legacy = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut legacy).unwrap();
    let legacy_word = u32_at(&legacy.stat, WORD);
    f.s_base = legacy.stat;
    use sha2::{Digest, Sha256};
    p.baseline = Some(source_rebuild::BaselineBinding {
        stat_sha256: format!("{:x}", Sha256::digest(&f.stat)),
        reset_d_sha256: format!("{:x}", Sha256::digest(&f.d0)),
        attrs: ATTRS,
        health_seed: f.hp_base,
        defense_sources: f.baseline_defense_sources,
        main_base: f.main8,
    });
    p.prefix.push(Submission {
        group: GROUP,
        value: 37,
    });
    f.source_rebuild = Some(p);
    f.source_baseline_ready = false;
    let mut s = Scratch::new();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    assert!(t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).is_err());
    f.rebase_source(&t, &c, &ATTRS).unwrap();
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!(
        u32_at(&s.stat, WORD),
        legacy_word + 37,
        "the actual source materializer supplies the test contribution"
    );
    assert_eq!(
        u32_at(&s.pred, WORD),
        observed_word,
        "the matching baseline must prevent counting a source twice"
    );
    // A same-category owned weapon supplies another13 through real effectF4
    // -> groupECC1. Its actual pipeline delta must survive the frame merge.
    c.inv[BANK..BANK + ITEM].copy_from_slice(&item(0x00f1, 1, Some((0xf4, 13))));
    t.pipeline(&f, &c, &ATTRS, &[], &mut s).unwrap();
    t.frame_writes(&f, &c, &ATTRS, 0, 0, &mut s).unwrap();
    assert_eq!(u32_at(&s.stat, WORD), legacy_word + 37 + 13);
    assert_eq!(
        u32_at(&s.pred, WORD),
        observed_word + 13,
        "the candidate delta must not be frozen to the baseline"
    );
    f.source_baseline_ready = false;
    put_u32(&mut f.stat, 0x844, 25);
    assert!(
        f.rebase_source(&t, &c, &ATTRS).is_err(),
        "old canonical STAT cannot become a current baseline"
    );
}

#[test]
fn source_rebuild_refuses_incomplete_or_extended_candidate_inventory() {
    let (t, mut f, mut c, p) = fixture();
    f.source_rebuild = Some(p);
    let inventory = c.inv.clone();
    for length in [0, 0x1ee0, 0x2017, 0x2019] {
        c.inv = inventory.clone();
        c.inv.resize(length, 0);
        let mut scratch = Scratch::new();
        assert!(
            t.pipeline(&f, &c, &ATTRS, &[], &mut scratch).is_err(),
            "invalid inventory extent must fail before reading selectors"
        );
    }
}
