//! Production Engine/Search regressions over synthetic variants of the captured bundle.
use nioh3_loadout::engine::{
    position_of, Backpack, Candidate, Engine, Limits, Outcome, ITEM, SLOTS,
};
use nioh3_loadout::pipeline::Frame;
use nioh3_loadout::search::{Request, Search, SetOption, SetRule};
use std::path::PathBuf;

#[test]
fn constrained_partial_search_keeps_a_feasible_outfit() {
    std::env::set_var("NIOH3_LOADOUT_GPU", "0");
    let bundle = std::env::var_os("NIOH3_LOADOUT_TEST_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/loadout/katana-light")
        });
    let engine = Engine::open(&bundle).expect("captured katana bundle");
    let characters =
        Frame::characters(&std::fs::read(bundle.join("characters.bin")).unwrap()).unwrap();
    let c = &characters[0];
    let at = c.active * 0xF70 + 4 * ITEM;
    let original: [u8; ITEM] = c.inv[at..at + ITEM].try_into().unwrap();
    assert_eq!(
        u16::from_le_bytes(original[..2].try_into().unwrap()),
        0x401F
    );
    let mut low = original;
    low[10..12].copy_from_slice(&0u16.to_le_bytes());
    let mut high = low;
    high[10..12].copy_from_slice(&1u16.to_le_bytes());
    let mut output = String::from("Synthetic bundle regression; no game or user save access.\n");
    for (name, a, b, defense_constraint) in [
        ("armor_plus", low, high, true),
        (
            "partial_slot_weight",
            {
                let mut heavy = high;
                heavy[0x31] = 2;
                heavy[0x32] = 2;
                heavy
            },
            {
                let mut light = high;
                light[0x31] = 0;
                light[0x32] = 0;
                light
            },
            false,
        ),
        (
            "armor_familiarity",
            {
                let mut low_fam = low;
                low_fam[0x14..0x18].copy_from_slice(&0u32.to_le_bytes());
                low_fam
            },
            {
                let mut high_fam = low;
                high_fam[0x14..0x18].copy_from_slice(&999_999u32.to_le_bytes());
                high_fam
            },
            true,
        ),
    ] {
        let pack = Backpack::from_slots(&[(4, a), (4, b)]).unwrap();
        let mut first = Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        };
        first.items[position_of(4).unwrap()] = Some(0);
        let mut second = first;
        second.items[position_of(4).unwrap()] = Some(1);
        let mut work = engine.work();
        let af = engine.facts(&first, &pack, &mut work).unwrap();
        let bf = engine.facts(&second, &pack, &mut work).unwrap();
        let limits = if defense_constraint {
            assert!(bf.defense > af.defense, "{name}: {af:?} / {bf:?}");
            Limits {
                min_defense: af.defense + 1,
                ..Limits::NONE
            }
        } else {
            assert!(af.load > bf.load);
            Limits {
                max_load: (af.load + bf.load) / 2.0,
                ..Limits::NONE
            }
        };
        assert_eq!(
            engine.evaluate(&first, &pack, &limits, &mut work),
            Outcome::OutsideLimits
        );
        let direct = engine.evaluate(&second, &pack, &limits, &mut work);
        assert!(matches!(direct, Outcome::Damage(_)), "{name}: {direct:?}");
        let request = Request {
            sets: vec![],
            limits,
            move_points: false,
            threads: 1,
        };
        let search = Search::new(&engine, &pack);
        let report = search.run(&request);
        assert!(
            !report.best.is_empty(),
            "{name}: feasible candidate was pruned"
        );
        assert!(report
            .best
            .iter()
            .any(|scored| scored.0.candidate.items[0] == Some(1)));
        for scored in &report.best {
            assert!(matches!(
                engine.evaluate(&scored.0.candidate, &pack, &limits, &mut work),
                Outcome::Damage(_)
            ));
        }
        let unbounded = search.run(&Request {
            limits: Limits::NONE,
            ..request.clone()
        });
        assert!(!unbounded.best.is_empty(), "unbounded control");
        output.push_str(&format!("{name}: first={af:?}; second={bf:?}; limits={limits:?}; direct={direct:?}; planned={}; returned={}; unbounded={}\n", search.plan(&request).size(), report.best.len(), unbounded.best.len()));
    }
    // Unknown defense must not erase known set groups or silently discard the
    // candidate before the evaluator can report its unsupported input.
    let mut unknown = original;
    let flags = u32::from_le_bytes(unknown[0x18..0x1C].try_into().unwrap()) | 0x40000;
    unknown[0x18..0x1C].copy_from_slice(&flags.to_le_bytes());
    let known_pack = Backpack::from_slots(&[(4, original)]).unwrap();
    let known_sets = Search::new(&engine, &known_pack).sets();
    assert!(!known_sets.is_empty(), "captured helmet carries a set");
    let pack = Backpack::from_slots(&[(4, original), (4, unknown)]).unwrap();
    let search = Search::new(&engine, &pack);
    for (key, _) in &known_sets {
        assert_eq!(
            search.set_rows(*key),
            vec![0, 1],
            "known set membership survives"
        );
        assert!(search
            .sets()
            .iter()
            .any(|(found, by_slot)| found == key && by_slot == &vec![(4, 2)]));
    }
    let request = Request {
        sets: vec![SetRule {
            options: vec![SetOption {
                key: known_sets[0].0,
                pieces: 1,
            }],
            at_least: 1,
        }],
        limits: Limits {
            min_defense: 1,
            ..Limits::NONE
        },
        move_points: false,
        threads: 1,
    };
    let plan = search.plan(&request);
    assert_eq!(
        plan.size(),
        2,
        "unknown defense does not prove dominance or remove a set class"
    );
    assert!(plan
        .kept_counts()
        .iter()
        .any(|&(position, kept, all)| position == 0 && kept == 2 && all == 2));
    let report = search.run(&request);
    assert!(!report.best.is_empty(), "known feasible control remains");
    assert!(
        report.counts.refused >= 1,
        "unknown candidate reaches the evaluator"
    );
    output.push_str(&format!(
        "unknown_defense: sets={:?}; planned={}; kept={:?}; evaluator_refused={}; returned={}\n",
        search.sets(),
        plan.size(),
        plan.kept_counts(),
        report.counts.refused,
        report.best.len()
    ));
    let root = std::env::var_os("NIOH3_REGRESSION_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("NIOH3_BUILD_ROOT")
                .map(PathBuf::from)
                .expect("NIOH3_BUILD_ROOT or NIOH3_REGRESSION_ARTIFACT_DIR")
                .join("deliverables/loadout-regression")
        });
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("optimizer-feasibility.txt"), &output).unwrap();
    println!("{output}");
}
