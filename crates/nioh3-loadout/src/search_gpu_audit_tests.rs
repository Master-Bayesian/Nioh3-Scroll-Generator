//! Host-only hardware gate; its controlled duplicate-value/zero fixture is not a game scenario.
use super::*;
use crate::context::LoadoutContext;

#[test]
#[ignore = "Requires an explicit current-context CUDA hardware fixture"]
fn actual_device_zero_ties_partition_and_keep_all_top_identities() {
    let root =
        std::path::PathBuf::from(std::env::var_os("NIOH3_GPU_AUDIT_ROOT").expect("hardware root"));
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("CURRENT_CANDIDATE_FIELDS.json")).unwrap())
            .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(source["pool_rows"][0].clone()).unwrap();
    let row: [u8; crate::engine::ITEM] = bytes.try_into().unwrap();
    let context = LoadoutContext::load(&root.join("held-context")).unwrap();
    let engine = Engine::open_with_context(&root.join("zero-bundle"), &context).unwrap();
    let pack = Backpack::from_slots(&(0..801).map(|_| (4, row)).collect::<Vec<_>>()).unwrap();
    let request = Request {
        sets: Vec::new(),
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    };
    let search = Search::new(&engine, &pack);
    let plan = search.build_plan(&request, Some(1000));
    assert!(plan.complete);
    let mut kernel = engine.kernel();
    let card = match &mut *kernel {
        Kernel::Ready(card) => card,
        _ => panic!("actual GPU unavailable"),
    };
    card.use_pool(&pack.pool).unwrap();
    search
        .verify(card, &plan, engine.start_attrs, &request)
        .unwrap();
    let (top, counts, missed) = search
        .sweep_card(card, &plan, engine.start_attrs, &request, 73)
        .unwrap();
    assert_eq!(top.len(), KEEP);
    assert_eq!(counts.answered, 801);
    assert_eq!(counts.refused + counts.no_path + counts.outside_limits, 0);
    assert!(counts.card_overflows > 0);
    assert!(missed.is_empty());
    let identities: Vec<_> = top
        .iter()
        .map(|score| {
            assert_eq!(score.bits, 0);
            score.candidate.items[0].unwrap()
        })
        .collect();
    assert_eq!(identities, (0..KEEP as u32).collect::<Vec<_>>());
    let receipt = serde_json::json!({"passed":true,"actual_device":true,"candidates":801,"capacity":73,"top":KEEP,"overflows":counts.card_overflows,"batches":counts.card_batches,"scope":"Actual CUDA MAIN and production partition/merge, controlled zero-motion and duplicate-value options; not skill legality or live-game acceptance."});
    std::fs::write(
        root.join("ACTUAL_GPU_ZERO_RANKING.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
const GPU_SET_FREE: u32 = 0x024C;
const GPU_SET_FIXED: u32 = 0x0EF0;
const GPU_SET_FREE_EFFECT: u32 = 0xB24F;
const GPU_SET_FIXED_EFFECT: u32 = 0xA7A1;
const GPU_SET_REDUCTION_EFFECT: u32 = 0x53D4;

fn gpu_set_record(key: u16, effects: &[(u32, u32)]) -> [u8; crate::engine::ITEM] {
    let mut row = [0; crate::engine::ITEM];
    row[..2].copy_from_slice(&key.to_le_bytes());
    row[6..8].copy_from_slice(&1u16.to_le_bytes());
    for index in 0..7 {
        row[0x38 + 24 * index..0x3C + 24 * index].copy_from_slice(&u32::MAX.to_le_bytes());
    }
    for (index, &(effect, value)) in effects.iter().enumerate() {
        row[0x38 + 24 * index..0x3C + 24 * index].copy_from_slice(&effect.to_le_bytes());
        row[0x3C + 24 * index..0x40 + 24 * index].copy_from_slice(&value.to_le_bytes());
    }
    row
}

fn gpu_set_request(sets: Vec<SetRule>) -> Request {
    Request {
        sets,
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    }
}

fn gpu_set_rule(options: &[(u32, usize)], at_least: usize) -> SetRule {
    SetRule {
        options: options
            .iter()
            .map(|&(key, pieces)| SetOption { key, pieces })
            .collect(),
        at_least,
    }
}

fn gpu_set_active(state: &crate::pipeline::SetState, key: u32) -> usize {
    active_pieces(
        state
            .pieces
            .iter()
            .find(|entry| entry.0 == key)
            .map_or(0, |entry| entry.1),
        state.reduction,
    )
}

fn gpu_set_candidate(engine: &Engine, pack: &Backpack, selected: &[(u32, usize)]) -> Candidate {
    let mut candidate = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    for &(slot, origin) in selected {
        candidate.items[position_of(slot).unwrap()] = Some(
            pack.origin
                .iter()
                .position(|&entry| entry == origin)
                .unwrap() as u32,
        );
    }
    candidate
}

fn gpu_set_identity(candidate: &Candidate) -> serde_json::Value {
    serde_json::json!({"attrs":candidate.attrs,"items":candidate.items})
}

fn gpu_set_counts(counts: &Counts) -> [u64; 4] {
    [
        counts.refused,
        counts.answered,
        counts.no_path,
        counts.outside_limits,
    ]
}

fn gpu_set_state_json(state: Result<crate::pipeline::SetState, &'static str>) -> serde_json::Value {
    match state {
        Ok(state) => serde_json::json!({"pieces":state.pieces,"reduction":state.reduction}),
        Err(reason) => serde_json::json!({"refused":reason}),
    }
}

fn gpu_set_outcome_json(outcome: &Outcome) -> serde_json::Value {
    match outcome {
        Outcome::Damage(bits) => serde_json::json!({"outcome":"damage","bits":bits}),
        Outcome::Refused(reason) => serde_json::json!({"outcome":"refused","reason":reason}),
        Outcome::NoPath => serde_json::json!({"outcome":"no_path"}),
        Outcome::OutsideLimits => serde_json::json!({"outcome":"outside_limits"}),
    }
}

// Construct complete, independently loadable context copies. The captured
// scene files are immutable hard links (or copies when cross-volume); only
// owned inventory/manifest files change. Both evaluators consume one encoding.
fn gpu_set_context_copy(
    root: &std::path::Path,
    source: &LoadoutContext,
    label: &str,
    retained_reduction: Option<u32>,
    profile_delta: Option<i64>,
) -> LoadoutContext {
    use crate::context::{sha256, GroupOperation, SourceKind, SourceLayer};
    let mut manifest = source.manifest().clone();
    let mut inventory = source.inventory().to_vec();
    let bank = manifest.character.active as usize * 0xF70;
    for slot in 4..13 {
        inventory[bank + slot * crate::engine::ITEM..bank + (slot + 1) * crate::engine::ITEM]
            .fill(0);
    }
    let mut retained = vec![(GPU_SET_FREE_EFFECT, 1)];
    if let Some(value) = retained_reduction {
        retained.push((GPU_SET_REDUCTION_EFFECT, value));
    }
    let row = gpu_set_record(0x117E, &retained);
    inventory[bank + 9 * crate::engine::ITEM..bank + 10 * crate::engine::ITEM]
        .copy_from_slice(&row);
    for map in &mut manifest.character.core_maps {
        map.retain(|entry| entry.key != SET_REDUCTION_GROUP);
    }
    manifest.provenance.capture_id = format!("gpu-set-audit-{label}");
    manifest.provenance.description = format!("Controlled native-set CUDA audit derived from {}; preserves captured scene and actual weapons; synthetic equipment and profile delta {profile_delta:?}", source.digest());
    manifest.provenance.source_sha256 = sha256(
        format!(
            "{}:{label}:{retained_reduction:?}:{profile_delta:?}",
            source.digest()
        )
        .as_bytes(),
    );
    manifest.inventory_sha256 = sha256(&inventory);
    if let Some(value) = profile_delta {
        manifest.non_gear.layers.push(SourceLayer {
            name: format!("gpu_set_audit_profile_{label}"),
            kind: SourceKind::DerivedNamed,
            provenance: manifest.provenance.clone(),
            operations: vec![GroupOperation {
                group: SET_REDUCTION_GROUP,
                value,
            }],
        });
    }
    let derived = source.rebind_character_inputs(manifest, inventory).unwrap();
    let target = root.join("set-contexts").join(label);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("context.json"),
        serde_json::to_vec_pretty(derived.manifest()).unwrap(),
    )
    .unwrap();
    std::fs::write(target.join("inventory.bin"), derived.inventory()).unwrap();
    std::fs::write(target.join("frame.bin"), derived.frame()).unwrap();
    for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
        let original = root.join("held-context").join(name);
        let destination = target.join(name);
        if !destination.is_file() && std::fs::hard_link(&original, &destination).is_err() {
            std::fs::copy(&original, &destination).unwrap();
        }
        assert_eq!(
            sha256(&std::fs::read(&destination).unwrap()),
            sha256(&std::fs::read(&original).unwrap())
        );
    }
    let loaded = LoadoutContext::load(&target).unwrap();
    assert_eq!(loaded.digest(), derived.digest());
    loaded
}

fn gpu_set_compare_cases(
    label: &str,
    engine: &Engine,
    pack: &Backpack,
    cases: &[(&str, Candidate)],
    rules: &[(&str, Vec<SetRule>)],
) -> serde_json::Value {
    let search = Search::new(engine, pack);
    let native: Vec<_> = cases.iter().map(|(name, candidate)| serde_json::json!({
        "name":name,"candidate":gpu_set_identity(candidate),
        "native":gpu_set_state_json(engine.equipment_set_state(candidate, pack, &mut engine.work())),
    })).collect();
    let list: Vec<_> = cases.iter().map(|(_, candidate)| *candidate).collect();
    let mut guard = engine.kernel();
    let card = match &mut *guard {
        Kernel::Ready(card) => card,
        _ => panic!("actual CUDA unavailable for {label}"),
    };
    assert!(
        card.supports_set_constraints(),
        "new kernel must expose every native-set entry point"
    );
    assert!(card.supports_observed_search());
    card.use_pool(&pack.pool).unwrap();
    let mut receipts = Vec::new();
    let mut observed = [0u64; 4];
    for (rule_name, sets) in rules {
        let request = gpu_set_request(sets.clone());
        let actual = card
            .cases_with_sets(&list, &request.limits, &request.sets)
            .unwrap();
        let mut work = engine.work();
        let expected: Vec<_> = list
            .iter()
            .map(|candidate| search.candidate_outcome(candidate, &request, &mut work))
            .collect();
        let mut counts = Counts::default();
        for (index, (cpu, gpu)) in expected.iter().zip(&actual).enumerate() {
            assert!(
                agree(cpu, gpu),
                "{label}/{rule_name}/{}: CPU {cpu:?}, CUDA {gpu:?}",
                cases[index].0
            );
            if let Outcome::Damage(bits) = cpu {
                assert_eq!(*bits, 0, "fixture must execute actual zero-motion MAIN");
            }
            let _ = Search::tally_outcome(*cpu, &mut counts);
        }
        for (total, count) in observed.iter_mut().zip(gpu_set_counts(&counts)) {
            *total += count;
        }
        receipts.push(serde_json::json!({
            "rule":rule_name,
            "sets":sets.iter().map(|rule| serde_json::json!({"at_least":rule.at_least,"options":rule.options.iter().map(|o|serde_json::json!({"key":o.key,"pieces":o.pieces})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "counts":gpu_set_counts(&counts),
            "cpu":expected.iter().map(gpu_set_outcome_json).collect::<Vec<_>>(),
            "card":actual.iter().map(|outcome|format!("{outcome:?}")).collect::<Vec<_>>(),
        }));
    }
    assert!(
        observed[0] > 0 && observed[1] > 0 && observed[3] > 0,
        "audit needs refusal, numeric, and set-exclusion controls"
    );
    let encoded = engine.encoded_context().unwrap();
    let setup = std::str::from_utf8(encoded.file("gpu_setup.txt").unwrap()).unwrap();
    let ops: usize = setup.split_whitespace().nth(2).unwrap().parse().unwrap();
    assert!(
        ops > 0,
        "exercise the empty-op admission followed by profile-op scoring pipeline"
    );
    serde_json::json!({"variant":label,"context_digest":engine.context_digest(),"input_digest":encoded.input_digest(),"bundle_input_digest":engine.bundle_input_digest(),"profile_operations":ops,"native_cases":native,"rules":receipts,"cpu_main_diagnostics":format!("{:?}",engine.cpu_diagnostics())})
}

fn gpu_set_complete_late_domain(
    engine: &Engine,
    explicit_weapon: bool,
    weapon: [u8; crate::engine::ITEM],
) -> serde_json::Value {
    let neutral = gpu_set_record(0x0404, &[]);
    let qualifying = gpu_set_record(0x0404, &[(GPU_SET_FREE_EFFECT, 1)]);
    let mut items: Vec<_> = (0..801).map(|_| (4, neutral)).collect();
    items.extend((0..401).map(|_| (4, qualifying)));
    if explicit_weapon {
        items.push((0, weapon));
    }
    let pack = Backpack::from_slots(&items).unwrap();
    let search = Search::new(engine, &pack);
    let unconstrained = gpu_set_request(Vec::new());
    let plan = search.build_plan(&unconstrained, Some(2000));
    assert!(plan.complete);
    assert_eq!(
        plan.size(),
        1202,
        "reference domain must retain every repeated record identity"
    );
    let baseline = gpu_set_candidate(engine, &pack, &[(4, 0)]);
    let late = gpu_set_candidate(engine, &pack, &[(4, 801)]);
    let native = engine
        .equipment_set_state(&baseline, &pack, &mut engine.work())
        .unwrap();
    let late_native = engine
        .equipment_set_state(&late, &pack, &mut engine.work())
        .unwrap();
    let threshold = gpu_set_active(&native, GPU_SET_FREE) + 1;
    assert!(gpu_set_active(&late_native, GPU_SET_FREE) >= threshold);
    let request = gpu_set_request(vec![gpu_set_rule(&[(GPU_SET_FREE, threshold)], 1)]);
    let mut expected = Best::default();
    let mut expected_counts = Counts::default();
    let mut work = engine.work();
    for candidate in plan.candidates(engine.start_attrs) {
        if let Some(bits) = Search::tally_outcome(
            search.candidate_outcome(&candidate, &request, &mut work),
            &mut expected_counts,
        ) {
            assert_eq!(bits, 0);
            expected.offer(Scored { bits, candidate });
        }
    }
    let expected = expected.sorted();
    assert_eq!(gpu_set_counts(&expected_counts), [0, 401, 0, 801]);
    assert_eq!(expected.len(), KEEP);
    for (index, score) in expected.iter().enumerate() {
        let row = score.candidate.items[0].unwrap() as usize;
        assert_eq!(
            pack.origin[row],
            801 + index,
            "every winner follows the first 801 ineligible identities"
        );
        assert_eq!(score.candidate.items[9].is_some(), explicit_weapon);
    }
    let mut guard = engine.kernel();
    let card = match &mut *guard {
        Kernel::Ready(card) => card,
        _ => panic!("actual CUDA unavailable"),
    };
    card.use_pool(&pack.pool).unwrap();
    search
        .verify(card, &plan, engine.start_attrs, &request)
        .unwrap();
    let (actual, counts, missed) = search
        .sweep_card(card, &plan, engine.start_attrs, &request, 73)
        .unwrap();
    assert_eq!(gpu_set_counts(&counts), gpu_set_counts(&expected_counts));
    assert_eq!(
        actual
            .iter()
            .map(|s| (s.bits, s.candidate))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|s| (s.bits, s.candidate))
            .collect::<Vec<_>>()
    );
    assert!(missed.is_empty());
    assert!(
        counts.card_overflows > 0 && counts.card_batches > 1,
        "401 legal zero ties must exceed the tiny first-sweep buffer"
    );
    let (spaces, lists) = Plan::card_spaces(&plan.spaces);
    let mut cancellation_progress = Vec::new();
    let cancelled = card.sweep_observed_with_sets(
        &spaces,
        &lists,
        &engine.start_attrs,
        &Limits::NONE,
        &request.sets,
        0.0,
        73,
        4,
        &mut |completed, total| {
            cancellation_progress.push([completed, total]);
            completed == 0
        },
    );
    assert!(
        matches!(cancelled, Err(crate::gpu::GpuSweepError::Cancelled)),
        "cancelled constrained call must not return a partial ranking"
    );
    assert!(cancellation_progress.iter().any(|progress| progress[0] > 0));
    let mut retry_progress = Vec::new();
    let retry = card
        .sweep_observed_with_sets(
            &spaces,
            &lists,
            &engine.start_attrs,
            &Limits::NONE,
            &request.sets,
            0.0,
            1202,
            4,
            &mut |completed, total| {
                retry_progress.push([completed, total]);
                true
            },
        )
        .unwrap();
    assert_eq!(retry.counts, gpu_set_counts(&expected_counts));
    assert_eq!(retry.hit_total, 401);
    assert!(retry.missed.is_empty());
    assert_eq!(retry_progress.last(), Some(&[1202, 1202]));
    let mut retried: Vec<_> = retry
        .hits
        .iter()
        .map(|hit| Scored {
            bits: hit.bits,
            candidate: plan.spaces[hit.space as usize].candidate(hit.index, engine.start_attrs),
        })
        .collect();
    retried.sort_by_key(rank);
    retried.truncate(KEEP);
    assert_eq!(
        retried
            .iter()
            .map(|s| (s.bits, s.candidate))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|s| (s.bits, s.candidate))
            .collect::<Vec<_>>()
    );
    serde_json::json!({"explicit_same_current_weapon":explicit_weapon,"weapon_seed_expected":!explicit_weapon,"domain":1202,"early_excluded":801,"late_legal_zero_ties":401,"capacity":73,"threshold":threshold,"counts":gpu_set_counts(&counts),"overflows":counts.card_overflows,"batches":counts.card_batches,"top":actual.iter().map(|score|serde_json::json!({"bits":score.bits,"candidate":gpu_set_identity(&score.candidate)})).collect::<Vec<_>>(),"cancelled_without_ranking":true,"cancel_progress":cancellation_progress,"retry_progress":retry_progress,"retry_hit_total":retry.hit_total})
}

#[test]
#[ignore = "Requires NIOH3_GPU_SET_AUDIT_ROOT with current captured context, new CUDA kernel, and accepted legacy DLL"]
fn actual_device_native_set_constraints_match_cpu_cover_late_ties_and_cancel() {
    use crate::context::sha256;
    let root = std::path::PathBuf::from(
        std::env::var_os("NIOH3_GPU_SET_AUDIT_ROOT").expect("native-set hardware root"),
    );
    let source = LoadoutContext::load(&root.join("held-context")).unwrap();
    let source_fields: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("CURRENT_CANDIDATE_FIELDS.json")).unwrap())
            .unwrap();
    assert!(source_fields["pool_rows"]
        .as_array()
        .is_some_and(|rows| !rows.is_empty()));
    let bank = source.manifest().character.active as usize * 0xF70;
    let weapon: [u8; crate::engine::ITEM] = source.inventory()[bank..bank + crate::engine::ITEM]
        .try_into()
        .unwrap();
    assert_ne!(
        u16::from_le_bytes(weapon[..2].try_into().unwrap()),
        0,
        "preserve an actual equipped weapon"
    );
    let mut refused = gpu_set_record(0x0024, &[(GPU_SET_FREE_EFFECT, 1)]);
    refused[0x18..0x1C].copy_from_slice(&0x40000u32.to_le_bytes());
    let pack = Backpack::from_slots(&[
        (4, gpu_set_record(0x0404, &[])),
        (4, gpu_set_record(0x0404, &[(GPU_SET_FREE_EFFECT, 1)])),
        (4, gpu_set_record(0x0404, &[(GPU_SET_FREE_EFFECT, 1); 3])),
        (
            4,
            gpu_set_record(
                0x0404,
                &[(GPU_SET_FREE_EFFECT, 1), (GPU_SET_REDUCTION_EFFECT, 0)],
            ),
        ),
        (6, refused),
        (9, gpu_set_record(0xF5BB, &[(GPU_SET_FIXED_EFFECT, 1)])),
        (9, gpu_set_record(0xF5BB, &[(GPU_SET_FIXED_EFFECT, 1)])),
        (9, gpu_set_record(0x117E, &[])),
        (0, weapon),
    ])
    .unwrap();
    let context = gpu_set_context_copy(&root, &source, "native_without_reduction", None, None);
    let engine = Engine::open_with_context(&root.join("zero-bundle"), &context).unwrap();
    let cases = vec![
        ("retained_only", gpu_set_candidate(&engine, &pack, &[])),
        ("neutral_head", gpu_set_candidate(&engine, &pack, &[(4, 0)])),
        (
            "one_free_head",
            gpu_set_candidate(&engine, &pack, &[(4, 1)]),
        ),
        (
            "three_repeated_free_sources",
            gpu_set_candidate(&engine, &pack, &[(4, 2)]),
        ),
        (
            "zero_valued_reduction",
            gpu_set_candidate(&engine, &pack, &[(4, 3)]),
        ),
        (
            "neutral_accessories",
            gpu_set_candidate(&engine, &pack, &[(9, 7), (10, 7)]),
        ),
        (
            "one_fixed_piece",
            gpu_set_candidate(&engine, &pack, &[(9, 5), (10, 7)]),
        ),
        (
            "duplicate_fixed_piece",
            gpu_set_candidate(&engine, &pack, &[(9, 5), (10, 6)]),
        ),
        (
            "explicit_same_current_weapon",
            gpu_set_candidate(&engine, &pack, &[(4, 1), (0, 8)]),
        ),
        (
            "native_admission_refusal",
            gpu_set_candidate(&engine, &pack, &[(6, 4)]),
        ),
    ];
    let states: Vec<_> = cases[..9]
        .iter()
        .map(|(_, candidate)| {
            engine
                .equipment_set_state(candidate, &pack, &mut engine.work())
                .unwrap()
        })
        .collect();
    assert!(
        !states[0].reduction,
        "controlled baseline must not contain a native reduction source"
    );
    let free = |state: &crate::pipeline::SetState| {
        state
            .pieces
            .iter()
            .find(|entry| entry.0 == GPU_SET_FREE)
            .map_or(0, |entry| entry.1)
    };
    let fixed = |state: &crate::pipeline::SetState| {
        state
            .pieces
            .iter()
            .find(|entry| entry.0 == GPU_SET_FIXED)
            .map_or(0, |entry| entry.1)
    };
    assert!(
        free(&states[0]) >= 1,
        "retained accessory source must survive head-only replacement"
    );
    assert_eq!(free(&states[2]), free(&states[1]) + 1);
    assert_eq!(free(&states[3]), free(&states[1]) + 3);
    assert!(
        states[4].reduction,
        "native zero-valued source is still present"
    );
    assert_eq!(fixed(&states[6]), fixed(&states[5]) + 1);
    assert_eq!(
        fixed(&states[7]),
        fixed(&states[6]),
        "native fixed member identity must deduplicate two copies"
    );
    let mut moved = cases[7].1;
    // Direct-case qualification control: keep Heart and the captured Ki seed
    // fixed. This synthetic attribute vector is not a respec-budget claim.
    moved.attrs[5] = 5;
    let moved_state = engine
        .equipment_set_state(&moved, &pack, &mut engine.work())
        .unwrap();
    assert!(
        fixed(&moved_state) < fixed(&states[7]),
        "attribute qualification must remove the fixed piece"
    );
    let mut cases = cases;
    cases.push(("attribute_unqualified_fixed_piece", moved));
    let threshold = gpu_set_active(&states[1], GPU_SET_FREE) + 1;
    let fixed_threshold = gpu_set_active(&states[6], GPU_SET_FIXED);
    let missing = u32::MAX;
    let rules = vec![
        ("legacy_empty_rules", vec![]),
        (
            "free_native_threshold",
            vec![gpu_set_rule(&[(GPU_SET_FREE, threshold)], 1)],
        ),
        (
            "all_rules",
            vec![
                gpu_set_rule(&[(GPU_SET_FREE, threshold)], 1),
                gpu_set_rule(&[(GPU_SET_FIXED, fixed_threshold)], 1),
            ],
        ),
        (
            "any_options",
            vec![gpu_set_rule(
                &[(GPU_SET_FREE, threshold), (GPU_SET_FIXED, fixed_threshold)],
                1,
            )],
        ),
        (
            "all_options",
            vec![gpu_set_rule(
                &[(GPU_SET_FREE, threshold), (GPU_SET_FIXED, fixed_threshold)],
                2,
            )],
        ),
        (
            "duplicate_options_count_twice",
            vec![gpu_set_rule(
                &[(GPU_SET_FREE, threshold), (GPU_SET_FREE, threshold)],
                2,
            )],
        ),
        (
            "missing_key_zero_pieces",
            vec![gpu_set_rule(&[(missing, 0)], 1)],
        ),
        (
            "missing_key_one_piece",
            vec![gpu_set_rule(&[(missing, 1)], 1)],
        ),
        ("empty_options_zero_at_least", vec![gpu_set_rule(&[], 0)]),
        ("empty_options_impossible", vec![gpu_set_rule(&[], 1)]),
        (
            "nonempty_options_zero_at_least",
            vec![gpu_set_rule(&[(missing, usize::MAX)], 0)],
        ),
        (
            "at_least_exceeds_option_count",
            vec![gpu_set_rule(&[(GPU_SET_FREE, 0), (GPU_SET_FREE, 0)], 3)],
        ),
        (
            "wide_piece_threshold",
            vec![gpu_set_rule(&[(GPU_SET_FREE, usize::MAX)], 1)],
        ),
        (
            "wide_at_least_threshold",
            vec![gpu_set_rule(&[(GPU_SET_FREE, 0)], usize::MAX)],
        ),
    ];
    let mut variants = vec![gpu_set_compare_cases(
        "native_without_reduction",
        &engine,
        &pack,
        &cases,
        &rules,
    )];
    for (label, retained_reduction, delta) in [
        ("profile_add_cannot_create_native_reduction", None, Some(1)),
        (
            "profile_remove_cannot_erase_native_reduction",
            Some(1),
            Some(-1),
        ),
        ("retained_zero_valued_native_reduction", Some(0), None),
    ] {
        let changed = gpu_set_context_copy(&root, &source, label, retained_reduction, delta);
        let changed_engine =
            Engine::open_with_context(&root.join("zero-bundle"), &changed).unwrap();
        let retained = changed_engine
            .equipment_set_state(&cases[0].1, &pack, &mut changed_engine.work())
            .unwrap();
        assert_eq!(
            retained.reduction,
            retained_reduction.is_some(),
            "profile delta must not replace the native reduction snapshot"
        );
        variants.push(gpu_set_compare_cases(
            label,
            &changed_engine,
            &pack,
            &cases,
            &rules,
        ));
    }
    let full_domains = [
        gpu_set_complete_late_domain(&engine, false, weapon),
        gpu_set_complete_late_domain(&engine, true, weapon),
    ];
    let legacy = Engine::open_with_context(&root.join("legacy-zero-bundle"), &context).unwrap();
    let legacy_search = Search::new(&legacy, &pack);
    let mut legacy_guard = legacy.kernel();
    let legacy_card = match &mut *legacy_guard {
        Kernel::Ready(card) => card,
        _ => panic!("accepted legacy CUDA kernel unavailable"),
    };
    assert!(!legacy_card.supports_set_constraints());
    legacy_card.use_pool(&pack.pool).unwrap();
    let list: Vec<_> = cases.iter().map(|(_, candidate)| *candidate).collect();
    let plain = legacy_card.cases(&list, &Limits::NONE).unwrap();
    let empty = legacy_card
        .cases_with_sets(&list, &Limits::NONE, &[])
        .unwrap();
    assert_eq!(plain, empty, "empty rules must retain legacy case behavior");
    let empty_request = gpu_set_request(vec![]);
    for (candidate, actual) in list.iter().zip(&empty) {
        let expected =
            legacy_search.candidate_outcome(candidate, &empty_request, &mut legacy.work());
        assert!(agree(&expected, actual));
    }
    let constrained_error = legacy_card
        .cases_with_sets(&list, &Limits::NONE, &rules[1].1)
        .unwrap_err();
    assert!(constrained_error.contains("native set-predicate capability"));
    let small = [Space::new(vec![(
        0,
        vec![pack.origin.iter().position(|&origin| origin == 1).unwrap() as u32],
    )])];
    let (spaces, lists) = Plan::card_spaces(&small);
    assert!(legacy_card
        .sweep_with_sets(
            &spaces,
            &lists,
            &engine.start_attrs,
            &Limits::NONE,
            &rules[1].1,
            0.0,
            1,
            1
        )
        .err()
        .unwrap()
        .contains("native set-predicate capability"));
    assert!(matches!(
        legacy_card.sweep_observed_with_sets(
            &spaces,
            &lists,
            &engine.start_attrs,
            &Limits::NONE,
            &rules[1].1,
            0.0,
            1,
            1,
            &mut |_, _| true
        ),
        Err(crate::gpu::GpuSweepError::Failure(_))
    ));
    let old_sweep = legacy_card
        .sweep(
            &spaces,
            &lists,
            &engine.start_attrs,
            &Limits::NONE,
            0.0,
            1,
            1,
        )
        .unwrap();
    let empty_sweep = legacy_card
        .sweep_with_sets(
            &spaces,
            &lists,
            &engine.start_attrs,
            &Limits::NONE,
            &[],
            0.0,
            1,
            1,
        )
        .unwrap();
    assert_eq!(old_sweep.counts, empty_sweep.counts);
    assert_eq!(old_sweep.hit_total, empty_sweep.hit_total);
    assert_eq!(
        old_sweep
            .hits
            .iter()
            .map(|hit| (hit.bits, hit.space, hit.index))
            .collect::<Vec<_>>(),
        empty_sweep
            .hits
            .iter()
            .map(|hit| (hit.bits, hit.space, hit.index))
            .collect::<Vec<_>>()
    );
    let identities: Vec<_> = [
        "held-context/context.json",
        "held-context/inventory.bin",
        "held-context/frame.bin",
        "held-context/image.bin",
        "held-context/image_known.bin",
        "held-context/scene_entry.bin",
        "zero-bundle/kernel.dll",
        "legacy-zero-bundle/kernel.dll",
        "zero-bundle/tables.bin",
        "zero-bundle/hits.txt",
        "CURRENT_CANDIDATE_FIELDS.json",
    ]
    .into_iter()
    .map(|file| {
        let bytes = std::fs::read(root.join(file)).unwrap();
        serde_json::json!({"file":file,"bytes":bytes.len(),"sha256":sha256(&bytes)})
    })
    .collect();
    let receipt = serde_json::json!({"schema":"nioh3-native-set-gpu-e2e/v1","passed":true,"actual_device":true,"test":"search::hardware_audit_tests::actual_device_native_set_constraints_match_cpu_cover_late_ties_and_cancel","fixture_env":"NIOH3_GPU_SET_AUDIT_ROOT","scope":"Actual translated CPU/CUDA MAIN with controlled zero-motion, native-table synthetic records, immutable derived captured contexts, and the same current weapon only. This does not establish new weapon support, natural item legality, live-game behavior, or release acceptance.","source_identities":identities,"compiled_sources":[{"file":"search_gpu_audit_tests.rs","sha256":sha256(include_bytes!("search_gpu_audit_tests.rs"))},{"file":"gpu.rs","sha256":sha256(include_bytes!("gpu.rs"))}],"variants":variants,"complete_domains":full_domains,"legacy":{"supports_native_sets":false,"constrained_cases_error":constrained_error,"all_constrained_entrypoints_refused":true,"empty_rules_unchanged":true}});
    std::fs::write(
        root.join("ACTUAL_GPU_NATIVE_SET_CONSTRAINTS.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
