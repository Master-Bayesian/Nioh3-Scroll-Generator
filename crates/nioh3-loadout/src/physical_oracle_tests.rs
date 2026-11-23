use super::*;
use crate::engine::{position_of, Limits};
use crate::search::{CurrentChoicePolicy, Report, Search, SetOption, SetRule};
use serde_json::{json, Value};

const ACTIVE: usize = 1;
const ONE: u32 = 1.0f32.to_bits();
const FREE_SIDE: u32 = 0x024C;
const FREE_EFFECT: u32 = 0xB24F;
const REDUCTION_EFFECT: u32 = 0x53D4;
const CROSS_SIDE: u32 = 41164;
const CROSS_EFFECT: u32 = 31299;
const CROSS_REWARD: u16 = 37872;

fn record(key: u16, effects: &[u32], special: bool) -> [u8; ITEM] {
    let mut item = [0; ITEM];
    item[..2].copy_from_slice(&key.to_le_bytes());
    item[6..8].copy_from_slice(&1u16.to_le_bytes());
    item[0x18..0x1C].copy_from_slice(&(u32::from(special) << 19).to_le_bytes());
    for (index, line) in item[0x38..0xE0]
        .as_chunks_mut::<24>()
        .0
        .iter_mut()
        .enumerate()
    {
        let effect = effects.get(index).copied().unwrap_or(u32::MAX);
        line[..4].copy_from_slice(&effect.to_le_bytes());
    }
    item
}

struct Fixture {
    engine: Engine,
    owned: Owned,
    before: PhysicalLayout,
}

impl Fixture {
    fn new() -> Self {
        let mut engine = Engine::audit_fixture();
        let character = engine.audit_character();
        character.active = ACTIVE;
        character.inv[ACTIVE * BANK + 0xF68] = 1;
        Self {
            engine,
            owned: Owned::new(),
            before: PhysicalLayout::default(),
        }
    }

    fn add(&mut self, id: OwnedId, item: [u8; ITEM]) {
        assert!(self.owned.insert(id, item).is_none());
    }

    fn wear(&mut self, id: OwnedId, bank: usize, slot: usize, item: [u8; ITEM]) {
        self.add(id, item);
        self.before.0[bank][slot] = Some(id);
    }

    fn bind(&mut self) {
        let character = self.engine.audit_character();
        character.inv = self.before.inventory(&character.inv, &self.owned);
    }

    fn run(&self, domain: &Domain, request: &Request, top_k: usize) -> Receipt {
        let receipt = run(
            &self.engine,
            &self.owned,
            self.before,
            domain,
            request,
            top_k,
        );
        let covered: Vec<_> = [
            &receipt.coverage.answered,
            &receipt.coverage.refused,
            &receipt.coverage.no_path,
            &receipt.coverage.outside_limits,
        ]
        .into_iter()
        .flatten()
        .copied()
        .collect();
        let unique: BTreeSet<_> = covered.iter().copied().collect();
        assert_eq!(covered.len(), unique.len());
        assert_eq!(unique, (0..receipt.rows.len()).collect());
        receipt
    }
}

fn request() -> Request {
    Request {
        sets: Vec::new(),
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    }
}

fn require(side: u32, pieces: usize) -> Request {
    Request {
        sets: vec![SetRule {
            options: vec![SetOption { key: side, pieces }],
            at_least: 1,
        }],
        ..request()
    }
}

fn baseline_report(report: &Report) -> Value {
    json!({
        "full_size": report.full_size,
        "searched": report.searched,
        "complete": report.complete,
        "source_uncomputed": report.source_uncomputed,
        "answered": report.counts.answered,
        "refused": report.counts.refused,
        "no_path": report.counts.no_path,
        "outside_limits": report.counts.outside_limits,
        "top": report.best.iter().map(|(score, _)| {
            json!({"items": score.candidate.items, "score_bits": score.bits})
        }).collect::<Vec<_>>(),
    })
}

fn write_receipt(name: &str, receipt: &Receipt, baseline: Value) {
    if let Some(dir) = std::env::var_os("NIOH3_PHYSICAL_ORACLE_RECEIPTS") {
        let data = json!({
            "schema": "nioh3-physical-oracle/v1",
            "fixture": name,
            "baseline_commit": "d1fc4ddd",
            "oracle": receipt,
            "baseline": baseline,
        });
        let path = std::path::PathBuf::from(dir).join(format!("{name}.json"));
        std::fs::write(path, serde_json::to_vec_pretty(&data).unwrap()).unwrap();
    }
}

#[test]
fn identical_bytes_keep_distinct_owned_identity_and_tie_order() {
    let mut fixture = Fixture::new();
    let [current, loose] = [OwnedId(10), OwnedId(20)];
    let head = record(0x0404, &[], false);
    fixture.wear(current, ACTIVE, 4, head);
    fixture.add(loose, head);
    fixture.bind();
    let mut domain = Domain::default();
    domain.axes[4] = vec![Equip(loose), Keep];

    let oracle = fixture.run(&domain, &request(), 2);
    assert_eq!(oracle.coverage.answered, [0, 1]);
    assert_eq!(oracle.top, [1, 0]);
    let [replaced, kept] = oracle.rows.as_slice() else {
        panic!("two owned copies")
    };
    assert_eq!(replaced.layout.0[ACTIVE][4], Some(loose));
    assert_eq!(kept.layout.0[ACTIVE][4], Some(current));
    assert_eq!(replaced.after_words, kept.after_words);
    assert_eq!(replaced.score_bits, Some(ONE));
    assert_eq!(kept.score_bits, Some(ONE));
    assert_eq!(fixture.run(&domain, &request(), 1).top, [1]);

    let pack = Backpack::from_slots(&[(4, head)]).unwrap();
    let baseline = Search::new(&fixture.engine, &pack)
        .with_current_choices(CurrentChoicePolicy::Captured)
        .run(&request());
    assert_eq!(
        (
            baseline.full_size,
            baseline.counts.answered,
            baseline.best.len()
        ),
        (1, 1, 1)
    );
    assert_eq!(
        baseline.best[0].0.candidate.items[position_of(4).unwrap()],
        Some(0)
    );
    assert_eq!(baseline.best[0].0.bits, ONE);
    write_receipt(
        "identical-owned-copies",
        &oracle,
        baseline_report(&baseline),
    );

    // Numeric owned IDs break ties between two Equip choices independently
    // of discovery order, even when both records have identical bytes.
    domain.axes[4] = vec![Equip(loose), Equip(current)];
    assert_eq!(fixture.run(&domain, &request(), 2).top, [1, 0]);
}

#[test]
fn spare_swap_crosses_the_native_threshold_without_duplicating_a_weapon() {
    for spare_slot in 1..=3 {
        let mut fixture = Fixture::new();
        *fixture.engine.audit_character() = crate::pipeline::audit_source_character();
        fixture.engine.audit_character().inv[..ITEM].fill(0);
        let [held, spare, head] = [OwnedId(1), OwnedId(2), OwnedId(3)];
        let mut plain = record(0x00F1, &[], false);
        plain[0x33] = 1;
        let mut reducer = record(0x00F1, &[REDUCTION_EFFECT], false);
        reducer[0x33] = 1;
        fixture.wear(held, ACTIVE, 0, plain);
        fixture.wear(spare, ACTIVE, spare_slot, reducer);
        fixture.wear(head, ACTIVE, 4, record(0x0404, &[FREE_EFFECT], false));
        fixture.bind();
        let mut program = crate::pipeline::audit_source_program();
        program.reference_inventory = fixture.engine.character().inv.clone();
        program.validate().unwrap();
        fixture.engine.audit_source_program(program);

        let mut domain = Domain::default();
        domain.axes[0] = vec![Keep, Equip(spare)];
        let request = require(FREE_SIDE, 2);
        let oracle = fixture.run(&domain, &request, 10);
        assert_eq!(oracle.coverage.outside_limits, [0]);
        assert_eq!(oracle.coverage.answered, [1]);
        assert_eq!(oracle.top, [1]);
        let [kept, swapped] = oracle.rows.as_slice() else {
            panic!("keep and swap")
        };
        assert_eq!(kept.layout, oracle.before);
        assert!(kept.activations.is_empty());
        assert_eq!(swapped.layout.0[ACTIVE][0], Some(spare));
        assert_eq!(swapped.layout.0[ACTIVE][spare_slot], Some(held));
        assert_eq!(
            swapped.after_words[ACTIVE][0],
            oracle.before_words[ACTIVE][spare_slot]
        );
        assert_eq!(
            swapped.after_words[ACTIVE][spare_slot],
            oracle.before_words[ACTIVE][0]
        );
        assert_eq!(
            swapped.activations,
            [SetRowActivation {
                side: FREE_SIDE,
                bank: ACTIVE,
                row_index: 0,
                threshold: 2,
                reward_key: 45494,
                flags: 0,
            }]
        );

        let pack = Backpack::from_slots(&[(0, reducer)]).unwrap();
        let baseline = Search::new(&fixture.engine, &pack)
            .with_current_choices(CurrentChoicePolicy::Captured)
            .run(&request);
        assert_eq!(
            (
                baseline.full_size,
                baseline.counts.outside_limits,
                baseline.counts.answered
            ),
            (2, 1, 1)
        );
        let scored = baseline.best[0].0;
        assert_eq!(Some(scored.bits), swapped.score_bits);
        let mut work = fixture.engine.work();
        fixture.engine.wear(&scored.candidate, &pack, &mut work);
        let without_swap = equipped_words(&work.character.inv);
        assert_eq!(without_swap[ACTIVE][spare_slot], without_swap[ACTIVE][0]);

        // Exercise the baseline wear primitive used by ExactSweep. Its public
        // swap admission additionally requires an encoded slot-zero scene.
        work.weapon_swaps.push((0, spare_slot));
        assert_eq!(
            fixture
                .engine
                .evaluate(&scored.candidate, &pack, &Limits::NONE, &mut work),
            Outcome::Damage(ONE)
        );
        assert_eq!(equipped_words(&work.character.inv), swapped.after_words);
        assert_eq!(work.scratch.source.set_row_activations, swapped.activations);
        write_receipt(
            &format!("equipped-spare-{spare_slot}"),
            &oracle,
            json!({
                "search": baseline_report(&baseline), "search_after_words": without_swap,
                "swap_primitive_after_words": equipped_words(&work.character.inv),
            }),
        );
    }
}

fn accessory_layouts(receipt: &Receipt) -> BTreeSet<[Option<OwnedId>; 2]> {
    receipt
        .rows
        .iter()
        .map(|row| ACCESSORIES.map(|slot| row.layout.0[ACTIVE][slot]))
        .collect()
}

#[test]
fn accessory_cardinality_and_ranked_orientation_characterize_baseline() {
    let [a, b] = [OwnedId(101), OwnedId(102)];
    let mut fixture = Fixture::new();
    let item = record(0x117E, &[], false);
    fixture.add(a, item);
    fixture.add(b, item);
    for (eligible, layouts, baseline_counts) in [
        (&[][..], vec![[None, None]], (1, 1, 1)),
        (&[a][..], vec![[Some(a), None], [None, Some(a)]], (1, 1, 1)),
        (
            &[a, b][..],
            vec![[Some(a), Some(b)], [Some(b), Some(a)]],
            (2, 2, 1),
        ),
    ] {
        let domain = Domain::default().accessories(eligible, [None; 2], AccessoryPolicy::default());
        let oracle = fixture.run(&domain, &request(), 10);
        assert_eq!(oracle.rows.len(), layouts.len());
        assert_eq!(accessory_layouts(&oracle), layouts.into_iter().collect());
        assert_eq!(oracle.coverage.answered.len(), oracle.rows.len());
        assert_eq!(
            oracle.top,
            if eligible.len() == 1 {
                vec![1, 0]
            } else {
                (0..oracle.rows.len()).collect()
            }
        );
        assert!(oracle.rows.iter().all(|row| row.score_bits == Some(ONE)));

        let pack = Backpack::from_slots(&vec![(9, item); eligible.len()]).unwrap();
        let search = Search::new(&fixture.engine, &pack);
        let baseline = search.run(&request());
        assert_eq!(
            (
                baseline.full_size,
                baseline.counts.answered,
                baseline.best.len()
            ),
            baseline_counts
        );
        let [one, two] = ACCESSORIES.map(|slot| position_of(slot as u32).unwrap());
        let best = baseline.best[0].0;
        assert_eq!(best.bits, ONE);
        assert_eq!(
            [best.candidate.items[one], best.candidate.items[two]],
            if eligible.len() == 2 {
                [Some(0), Some(1)]
            } else {
                [None, None]
            }
        );
        let current = search
            .with_current_choices(CurrentChoicePolicy::Captured)
            .run(&request());
        if eligible.len() == 1 {
            let ranked: Vec<_> = current
                .best
                .iter()
                .map(|(score, _)| [score.candidate.items[one], score.candidate.items[two]])
                .collect();
            assert_eq!(ranked, [[None, None], [None, Some(0)], [Some(0), None]]);
        }
        write_receipt(
            &format!("accessories-{}", eligible.len()),
            &oracle,
            json!({
                "default": baseline_report(&baseline), "with_current_choices": baseline_report(&current),
            }),
        );
    }
}

#[test]
fn optional_accessory_choices_reject_duplicates_across_banks_and_kept_slots() {
    let [current, reserve, loose] = [OwnedId(1), OwnedId(2), OwnedId(3)];
    let item = record(0x117E, &[], false);
    let mut fixture = Fixture::new();
    fixture.wear(current, ACTIVE, 9, item);
    fixture.wear(reserve, ACTIVE ^ 1, 10, item);
    fixture.add(loose, item);
    fixture.bind();

    let domain = Domain::default().accessories(
        &[loose],
        [Some(current), None],
        AccessoryPolicy {
            allow_empty: true,
            allow_keep: true,
        },
    );
    let oracle = fixture.run(&domain, &request(), 10);
    assert_eq!(
        accessory_layouts(&oracle),
        BTreeSet::from([
            [Some(current), Some(loose)],
            [Some(current), None],
            [Some(loose), None],
            [None, Some(loose)],
            [None, None],
        ])
    );
    assert_eq!(oracle.rows.len(), 5);
    let represented = Domain::default().accessories(
        &[current, loose],
        [Some(current), None],
        AccessoryPolicy {
            allow_empty: true,
            allow_keep: true,
        },
    );
    let represented = fixture.run(&represented, &request(), 10);
    assert_eq!(represented.rows.len(), 7);
    assert_eq!(accessory_layouts(&represented).len(), 7);
    let mut choices = [Keep; PHYSICAL_SLOTS];
    choices[10] = Equip(current);
    assert_eq!(fixture.before.resolve(ACTIVE, &choices), None);
    choices[9] = Empty;
    let moved = fixture.before.resolve(ACTIVE, &choices).unwrap();
    assert_eq!(moved.0[ACTIVE][10], Some(current));
    choices[10] = Equip(reserve);
    assert_eq!(fixture.before.resolve(ACTIVE, &choices), None);
    write_receipt("accessory-empty-keep", &oracle, Value::Null);
}

fn fixed_members(bank: usize, special: [bool; 2]) -> Fixture {
    let mut fixture = Fixture::new();
    // Both packaged armor keys require 12 points in attribute columns 0 and 4.
    fixture.engine.start_attrs[0] = 12;
    fixture.engine.start_attrs[4] = 12;
    for (i, (key, special)) in [0x6FD6, 0xCCD4].into_iter().zip(special).enumerate() {
        fixture.wear(
            OwnedId((i + 1) as u32),
            bank,
            4 + i,
            record(key, &[CROSS_EFFECT], special),
        );
    }
    fixture.bind();
    fixture
}

#[test]
fn packaged_pc202_row_activates_from_either_bank_with_cross_bank_flag() {
    use sha2::{Digest, Sha256};
    // The committed audit table and katana-light packaged tables.bin have the
    // same digest, so the tiny fixture needs no installed game or DLL.
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!("../tests/fixtures/audit_tables.bin"))
        ),
        "922ba8fa5c1089538c1f075e5600cc65a8310ee4150d12eb4aa34610f3f804c2"
    );
    for bank in [0, 1] {
        let mut fixture = fixed_members(bank, [false; 2]);
        let oracle = fixture.run(&Domain::default(), &request(), 1);
        assert_eq!(
            oracle.rows[0].activations,
            [SetRowActivation {
                side: CROSS_SIDE,
                bank,
                row_index: 0,
                threshold: 2,
                reward_key: CROSS_REWARD,
                flags: 2,
            }]
        );
        assert_eq!(oracle.rows[0].score_bits, Some(ONE));
        let pack = Backpack::from_slots(&[]).unwrap();
        let candidate = Candidate {
            attrs: fixture.engine.start_attrs,
            items: [None; SLOTS],
        };
        let groups = fixture
            .engine
            .equipment_groups(&candidate, &pack, &mut fixture.engine.work())
            .unwrap();
        assert!(groups[ACTIVE].iter().any(|entry| entry.key == CROSS_REWARD));
        assert!(!groups[ACTIVE ^ 1]
            .iter()
            .any(|entry| entry.key == CROSS_REWARD));
        let baseline = Search::new(&fixture.engine, &pack).run(&request());
        assert_eq!(
            (baseline.counts.answered, baseline.best[0].0.bits),
            (1, ONE)
        );
        write_receipt(
            &format!("pc202-cross-bank-{bank}"),
            &oracle,
            baseline_report(&baseline),
        );

        fixture
            .engine
            .tables
            .audit_set_reward_flags(CROSS_SIDE, 0, 0);
        let control = fixture.run(&Domain::default(), &request(), 1);
        assert_eq!(
            control.rows[0].activations.len(),
            usize::from(bank == ACTIVE)
        );
    }
}

#[test]
fn synthetic_flag_one_counts_special_members_instead_of_all_members() {
    for (special, activated) in [
        ([false, false], false),
        ([true, false], false),
        ([true, true], true),
    ] {
        let bank = ACTIVE;
        let mut fixture = fixed_members(bank, special);
        fixture
            .engine
            .tables
            .audit_set_reward_flags(CROSS_SIDE, 0, 1);
        let oracle = fixture.run(&Domain::default(), &require(CROSS_SIDE, 2), 1);
        assert_eq!(oracle.rows[0].score_bits, Some(ONE)); // Both qualify for the piece constraint.
        let expected = activated.then_some(SetRowActivation {
            side: CROSS_SIDE,
            bank,
            row_index: 0,
            threshold: 2,
            reward_key: CROSS_REWARD,
            flags: 1,
        });
        assert_eq!(
            oracle.rows[0].activations,
            expected.into_iter().collect::<Vec<_>>()
        );
        let pack = Backpack::from_slots(&[]).unwrap();
        let baseline = Search::new(&fixture.engine, &pack).run(&require(CROSS_SIDE, 2));
        assert_eq!(
            (baseline.counts.answered, baseline.best[0].0.bits),
            (1, ONE)
        );
        write_receipt(
            &format!(
                "synthetic-special-count-{}",
                special.into_iter().filter(|&yes| yes).count()
            ),
            &oracle,
            baseline_report(&baseline),
        );
    }
}

#[test]
fn canonical_scores_and_disjoint_partition_include_unanswered_rows() {
    let outcomes = [
        Outcome::Damage((-0.0f32).to_bits()),
        Outcome::Damage(0),
        Outcome::Damage(ONE),
        Outcome::Damage(f32::INFINITY.to_bits()),
        Outcome::Damage(f32::NAN.to_bits()),
        Outcome::Damage((-1.0f32).to_bits()),
        Outcome::Refused("controlled refusal"),
        Outcome::NoPath,
        Outcome::OutsideLimits,
    ];
    let mut coverage = Coverage::default();
    let scores = outcomes
        .map(NativeOutcome::from)
        .map(NativeOutcome::score_bits);
    assert_eq!(
        scores,
        [
            Some(0),
            Some(0),
            Some(ONE),
            None,
            None,
            None,
            None,
            None,
            None
        ]
    );
    for (index, outcome) in outcomes.into_iter().enumerate() {
        coverage.record(index, outcome.into());
    }
    assert_eq!(
        coverage,
        Coverage {
            answered: vec![0, 1, 2],
            refused: vec![3, 4, 5, 6],
            no_path: vec![7],
            outside_limits: vec![8],
        }
    );
}

#[test]
#[ignore = "Requires the expanded katana-light fixture used by constrained_search"]
fn packaged_formula_scores_both_owned_copies() {
    let path = std::path::PathBuf::from(std::env::var_os("NIOH3_LOADOUT_TEST_BUNDLE").unwrap());
    let engine = Engine::open(&path).unwrap();
    let character = engine.character();
    let mut before = PhysicalLayout::default();
    let mut owned = Owned::new();
    for (bank, slots) in before.0.iter_mut().enumerate() {
        for (slot, id) in slots.iter_mut().enumerate() {
            let at = bank * BANK + slot * ITEM;
            let item: [u8; ITEM] = character.inv[at..at + ITEM].try_into().unwrap();
            if item[..2] != [0, 0] {
                let owned_id = OwnedId((bank * PHYSICAL_SLOTS + slot) as u32);
                owned.insert(owned_id, item);
                *id = Some(owned_id);
            }
        }
    }
    let loose = OwnedId(32);
    let head = owned[&before.0[character.active][4].unwrap()];
    owned.insert(loose, head);
    let fixture = Fixture {
        engine,
        owned,
        before,
    };
    let mut domain = Domain::default();
    domain.axes[4] = vec![Equip(loose), Keep];
    let oracle = fixture.run(&domain, &request(), 2);
    assert_eq!(oracle.coverage.answered, [0, 1]);
    assert_eq!(oracle.top, [1, 0]);
    assert!(oracle
        .rows
        .iter()
        .all(|row| row.after_words == oracle.before_words));
    let pack = Backpack::from_slots(&[(4, head)]).unwrap();
    let baseline = Search::new(&fixture.engine, &pack)
        .with_current_choices(CurrentChoicePolicy::Captured)
        .run(&request());
    assert_eq!((baseline.counts.answered, baseline.best.len()), (1, 1));
    let bits = 1038.0f32.to_bits();
    assert_eq!(baseline.best[0].0.bits, bits);
    for row in &oracle.rows {
        assert_eq!(row.outcome, NativeOutcome::Damage(bits));
    }
    write_receipt(
        "packaged-formula-identical-copies",
        &oracle,
        baseline_report(&baseline),
    );
}
