//! Real Engine contexts over a captured corpus, with an independent native oracle.
use nioh3_loadout::context::{
    sha256, CharacterInputs, ContextManifest, CoreEntry, GroupOperation, LoadoutContext,
    NonGearSources, Provenance, Scenario, SceneBytes, SourceKind, SourceLayer, CONTEXT_SCHEMA,
};
use nioh3_loadout::engine::{Backpack, Candidate, Engine, Limits, Outcome, SLOTS};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn bundle() -> PathBuf {
    std::env::var_os("NIOH3_LOADOUT_TEST_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/loadout/katana-light")
        })
}
fn artifact(name: &str) -> PathBuf {
    let base = std::env::var_os("NIOH3_CONTEXT_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("NIOH3_BUILD_ROOT")
                .map(PathBuf::from)
                .expect("F: build root must be configured")
                .join("deliverables/context-engine")
        });
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = base.join(format!("{name}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}
fn sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut output = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let count = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        let name = std::str::from_utf8(&bytes[at..at + count])
            .unwrap()
            .to_string();
        at += count;
        let size = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        output.push((name, bytes[at..at + size].to_vec()));
        at += size;
    }
    output
}
fn encode_sections(values: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut output = Vec::new();
    for (name, bytes) in values {
        output.extend_from_slice(&(name.len() as u32).to_le_bytes());
        output.extend_from_slice(name.as_bytes());
        output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        output.extend_from_slice(bytes);
    }
    output
}
fn unhex(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
        .collect()
}
fn captured() -> (ContextManifest, Vec<u8>, Vec<u8>, Value) {
    let proof: Value = serde_json::from_str(include_str!("fixtures/context-native.json")).unwrap();
    let bundle = bundle();
    let frame = fs::read(bundle.join("frame.bin")).unwrap();
    let characters = fs::read(bundle.join("characters.bin")).unwrap();
    assert_eq!(sha256(&frame), proof["frame_sha256"]);
    assert_eq!(sha256(&characters), proof["characters_sha256"]);
    let character = sections(&characters)
        .into_iter()
        .find(|s| s.0 == "char0")
        .unwrap()
        .1;
    let word = |i: usize| u32::from_le_bytes(character[i * 4..i * 4 + 4].try_into().unwrap());
    let mut inventory = character[40..40 + 0x2018].to_vec();
    for item in proof["items"].as_array().unwrap() {
        let slot = item["slot"].as_u64().unwrap() as usize;
        let record = unhex(item["record_hex"].as_str().unwrap());
        let at = word(1) as usize * 0xF70 + slot * 0xE8;
        inventory[at..at + 0xE8].copy_from_slice(&record);
    }
    let mut entries = [Vec::new(), Vec::new()];
    let mut at = 40 + 0x2018;
    for (bank, map) in entries.iter_mut().enumerate() {
        for _ in 0..word(8 + bank) {
            map.push(CoreEntry {
                key: u16::from_le_bytes(character[at..at + 2].try_into().unwrap()),
                dup: u16::from_le_bytes(character[at + 2..at + 4].try_into().unwrap()),
                base: u32::from_le_bytes(character[at + 4..at + 8].try_into().unwrap()),
            });
            at += 8;
        }
    }
    let setup = fs::read_to_string(bundle.join("gpu_setup.txt")).unwrap();
    let mut lines = setup.lines();
    let head: Vec<u32> = lines
        .next()
        .unwrap()
        .split_ascii_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    let operations = lines
        .by_ref()
        .take(head[2] as usize)
        .map(|line| {
            let mut values = line.split_ascii_whitespace();
            GroupOperation {
                group: values.next().unwrap().parse().unwrap(),
                value: values.next().unwrap().parse().unwrap(),
            }
        })
        .collect();
    let provenance = Provenance { capture_id: proof["evidence"].as_str().unwrap().into(), source_sha256: proof["evidence_sha256"].as_str().unwrap().into(), game_version: [2,0,2,0], description: "Previously verified native MAIN corpus; captured katana-light action/target and fixed non-equipment profile".into() };
    let manifest = ContextManifest {
        schema: CONTEXT_SCHEMA.into(),
        scenario: Scenario::RecordedScenario,
        provenance: provenance.clone(),
        attrs: serde_json::from_value(proof["attrs"].clone()).unwrap(),
        character: CharacterInputs {
            mode: word(0) as i32,
            active: word(1),
            cap: word(2),
            plus_cap: word(3),
            style: word(4) as i32,
            unarmed: word(5),
            level: word(6),
            core_mode: word(7) as i32,
            core_maps: entries,
        },
        non_gear: NonGearSources {
            attack_gain: head[0],
            defense: head[1],
            layers: vec![SourceLayer {
                name: "captured-non-equipment-profile".into(),
                kind: SourceKind::ObservedNonEquipment,
                provenance: Provenance {
                    source_sha256: sha256(setup.as_bytes()),
                    ..provenance
                },
                operations,
            }],
        },
        inventory_sha256: sha256(&inventory),
        frame_sha256: sha256(&frame),
        native_scalars: None,
    };
    (manifest, inventory, frame, proof)
}
fn write_context(
    path: &Path,
    manifest: &ContextManifest,
    inventory: &[u8],
    frame: &[u8],
) -> LoadoutContext {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("context.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
    fs::write(path.join("inventory.bin"), inventory).unwrap();
    fs::write(path.join("frame.bin"), frame).unwrap();
    LoadoutContext::load(path).unwrap()
}
fn candidate(engine: &Engine) -> Candidate {
    Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    }
}
fn changed(mut manifest: ContextManifest) -> ContextManifest {
    manifest.provenance.capture_id = "synthetic-context-B".into();
    manifest.provenance.source_sha256 = sha256(b"synthetic-context-B");
    manifest.provenance.description =
        "Synthetic perturbation of the native corpus for context isolation; not a live character"
            .into();
    manifest.attrs[0] += 10;
    manifest.non_gear.attack_gain += 1;
    manifest.non_gear.defense += 77;
    manifest.non_gear.layers.push(SourceLayer {
        name: "synthetic-attack-group-delta".into(),
        kind: SourceKind::DerivedNamed,
        provenance: manifest.provenance.clone(),
        operations: vec![GroupOperation {
            group: 0x6A7C,
            value: 1,
        }],
    });
    manifest
}
fn uncovered(manifest: ContextManifest) -> ContextManifest {
    let mut large = changed(manifest);
    large.provenance.capture_id = "synthetic-large-context".into();
    large.provenance.description = "Synthetic context outside the captured CPU paths; must remain NoPath without a sample fallback".into();
    large.non_gear.attack_gain += 99;
    large.non_gear.layers.last_mut().unwrap().operations[0].value = 25;
    large
}

fn damage(outcome: Outcome) -> u32 {
    match outcome {
        Outcome::Damage(bits) => bits,
        other => panic!("expected answered corpus outfit: {other:?}"),
    }
}

#[test]
fn cpu_contexts_a_b_a_keep_inputs_and_native_corpus_identity() {
    let root = artifact("cpu");
    let (manifest, inventory, frame, proof) = captured();
    let a = write_context(&root.join("a"), &manifest, &inventory, &frame);
    let b = write_context(
        &root.join("b"),
        &changed(manifest.clone()),
        &inventory,
        &frame,
    );
    assert_ne!(a.digest(), b.digest());
    let engine_a = Engine::open_with_context(&bundle(), &a).unwrap();
    let engine_b = Engine::open_with_context(&bundle(), &b).unwrap();
    let pack = Backpack::from_slots(&[]).unwrap();
    let mut facts = Vec::new();
    let mut bits = Vec::new();
    for (index, engine) in [&engine_a, &engine_b, &engine_a].into_iter().enumerate() {
        let mut work = engine.work();
        let c = candidate(engine);
        let outcome = engine.evaluate(&c, &pack, &Limits::NONE, &mut work);
        println!("Context {index}: {outcome:?}");
        bits.push(damage(outcome));
        let f = engine.facts(&c, &pack, &mut work).unwrap();
        facts.push(f);
        let encoded = engine.encoded_context().unwrap();
        for name in ["characters.bin", "gpu_setup.txt", "frame.bin"] {
            fs::write(
                root.join(format!("{index}-{name}")),
                encoded.file(name).unwrap(),
            )
            .unwrap();
        }
    }
    assert_eq!(
        bits[0] as u64,
        proof["expected_native_bits"].as_u64().unwrap(),
        "frozen independent native corpus"
    );
    assert_eq!(bits[2], bits[0]);
    assert_eq!(facts[2], facts[0]);
    assert!(f32::from_bits(bits[1]) > f32::from_bits(bits[0]));
    assert_eq!(
        facts[1].defense,
        facts[0].defense + 77,
        "explicit non-gear defense replaces the sample contribution"
    );
    assert!(
        facts[1].health > facts[0].health,
        "player attributes reach the health pipeline"
    );
    assert_eq!(engine_a.context_digest(), Some(a.digest()));
    assert_eq!(engine_b.context_digest(), Some(b.digest()));
    let large = write_context(
        &root.join("uncovered"),
        &uncovered(manifest.clone()),
        &inventory,
        &frame,
    );
    let engine_large = Engine::open_with_context(&bundle(), &large).unwrap();
    let mut large_work = engine_large.work();
    let large_candidate = candidate(&engine_large);
    assert_eq!(
        engine_large.evaluate(&large_candidate, &pack, &Limits::NONE, &mut large_work),
        Outcome::NoPath
    );
    let large_facts = engine_large
        .facts(&large_candidate, &pack, &mut large_work)
        .unwrap();
    assert_eq!(
        large_facts, facts[1],
        "uncovered damage does not discard valid context facts"
    );
    // No candidate item replaces this slot: its absence must come from the
    // explicit complete inventory, not fall back to the bundle's helmet.
    let mut without_head = inventory.clone();
    let head = manifest.character.active as usize * 0xF70 + 4 * 0xE8;
    without_head[head..head + 0xE8].fill(0);
    let mut no_head_manifest = manifest;
    no_head_manifest.inventory_sha256 = sha256(&without_head);
    let no_head = LoadoutContext::new(no_head_manifest, without_head, frame).unwrap();
    let engine_empty = Engine::open_with_context(&bundle(), &no_head).unwrap();
    let empty_facts = engine_empty
        .facts(&candidate(&engine_empty), &pack, &mut engine_empty.work())
        .unwrap();
    assert!(
        empty_facts.defense < facts[0].defense,
        "unsearched gear must not be inherited from the sample"
    );
    let summary = json!({"passed": true, "scope": "real CPU Engine; A pinned to an independent recorded Native MAIN answer; B is synthetic", "a_context": a.digest(), "b_context": b.digest(), "a_input": engine_a.encoded_context().unwrap().input_digest(), "b_input": engine_b.encoded_context().unwrap().input_digest(), "damage_bits_a_b_a": bits, "defense_a_b_a": facts.iter().map(|f| f.defense).collect::<Vec<_>>(), "health_a_b_a": facts.iter().map(|f| f.health).collect::<Vec<_>>(), "missing_helmet_defense": empty_facts.defense, "uncovered_context": large.digest(), "uncovered_cpu_outcome": "NoPath"});
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    println!("Context CPU evidence: {}", root.display());
}

#[test]
fn context_loading_refuses_missing_drifted_and_foreign_frame_inputs() {
    let root = artifact("refusals");
    let (manifest, inventory, frame, _) = captured();
    write_context(&root, &manifest, &inventory, &frame);
    let mut missing = serde_json::to_value(&manifest).unwrap();
    missing.as_object_mut().unwrap().remove("attrs");
    fs::write(
        root.join("context.json"),
        serde_json::to_vec(&missing).unwrap(),
    )
    .unwrap();
    assert!(LoadoutContext::load(&root)
        .unwrap_err()
        .to_string()
        .contains("attrs"));
    fs::write(
        root.join("context.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let mut drifted = inventory.clone();
    drifted[0] ^= 1;
    fs::write(root.join("inventory.bin"), &drifted).unwrap();
    assert!(LoadoutContext::load(&root)
        .unwrap_err()
        .to_string()
        .contains("inventory digest"));
    let mut foreign = sections(&frame);
    foreign.iter_mut().find(|s| s.0 == "addresses").unwrap().1[0] ^= 8;
    let foreign_frame = encode_sections(&foreign);
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.frame_sha256 = sha256(&foreign_frame);
    let context = LoadoutContext::new(foreign_manifest, inventory.clone(), foreign_frame).unwrap();
    assert!(Engine::open_with_context(&bundle(), &context)
        .err()
        .unwrap()
        .reason
        .contains("address layout"));
    let mut duplicate = frame.clone();
    duplicate.extend_from_slice(&frame);
    let mut duplicate_manifest = manifest;
    duplicate_manifest.frame_sha256 = sha256(&duplicate);
    assert!(
        LoadoutContext::new(duplicate_manifest, inventory, duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"passed": true, "refused": ["missing attributes", "inventory digest drift", "foreign attack address layout", "duplicate frame sections"]})).unwrap()).unwrap();
    println!("Context refusal evidence: {}", root.display());
}

#[cfg(windows)]
#[test]
#[ignore = "Requires an explicitly coordinated GPU/native execution window"]
fn gpu_native_contexts_a_b_a_are_bit_exact() {
    use nioh3_loadout::{engine::Kernel, gpu::CardOutcome, native::Native};
    assert_eq!(
        std::env::var("NIOH3_CONTEXT_GPU").ok().as_deref(),
        Some("1"),
        "coordinate GPU access explicitly"
    );
    let root = artifact("gpu-native");
    let (manifest, inventory, frame, proof) = captured();
    let a = write_context(&root.join("a"), &manifest, &inventory, &frame);
    let b = write_context(
        &root.join("b"),
        &changed(manifest.clone()),
        &inventory,
        &frame,
    );
    let large = write_context(
        &root.join("uncovered"),
        &uncovered(manifest),
        &inventory,
        &frame,
    );
    let engine_a = Engine::open_with_context(&bundle(), &a).unwrap();
    let engine_b = Engine::open_with_context(&bundle(), &b).unwrap();
    let engine_large = Engine::open_with_context(&bundle(), &large).unwrap();
    let native_path = std::env::var_os("NIOH3_CONTEXT_NATIVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("F:/Nioh3_r16_build/work/native/ka"));
    let mut native = Native::open(&native_path).unwrap();
    let pack =
        Backpack::from_slots(&[(4, inventory[4 * 0xE8..5 * 0xE8].try_into().unwrap())]).unwrap();
    let mut results = Vec::new();
    for (index, engine) in [&engine_a, &engine_b, &engine_large, &engine_a]
        .into_iter()
        .enumerate()
    {
        let c = candidate(engine);
        let mut work = engine.work();
        let cpu = engine.evaluate(&c, &pack, &Limits::NONE, &mut work);
        let (outfit, hits) = engine.native_words(&c, &pack, &mut work).unwrap();
        let mut total = 0.0f32;
        let mut native_hits = Vec::new();
        for hit in &hits {
            native.reset();
            native.write_words(outfit);
            native.write_words(hit);
            let value = native.run().unwrap();
            total += f32::from_bits(value);
            native_hits.push(value);
        }
        let mut kernel = engine.kernel();
        let card = match &mut *kernel {
            Kernel::Ready(card) => card,
            Kernel::Unavailable(reason) => panic!("context GPU unavailable: {reason}"),
            Kernel::Untried => panic!("kernel was not initialized"),
        };
        card.use_pool(&pack.pool).unwrap();
        let result = card.cases(&[c], &Limits::NONE).unwrap();
        assert_eq!(result, vec![CardOutcome::Damage(total.to_bits())]);
        match cpu {
            Outcome::Damage(bits) => assert_eq!(bits, total.to_bits()),
            Outcome::NoPath if index == 2 => {}
            other => panic!("unexpected CPU outcome: {other:?}"),
        }
        if index == 0 || index == 3 {
            assert_eq!(
                total.to_bits() as u64,
                proof["expected_native_bits"].as_u64().unwrap()
            );
        }
        let record = json!({"index": index, "context_digest": engine.context_digest(), "input_digest": engine.encoded_context().unwrap().input_digest(), "attrs": c.attrs, "outfit_words": outfit, "hits": hits, "native_hit_bits": native_hits, "native_bits": total.to_bits(), "cpu_outcome": format!("{cpu:?}"), "gpu_bits": total.to_bits(), "bit_exact": true});
        fs::write(
            root.join(format!("proof-{index}.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        results.push(record);
    }
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"passed": true, "scope": "captured attack scene; explicit context A/B/A with independent Native MAIN; not arbitrary buff/target parity", "results": results})).unwrap()).unwrap();
    println!("Context GPU/native evidence: {}", root.display());
}

fn final_stat_word(engine: &Engine, frame: &[u8], offset: usize) -> u32 {
    let sections = sections(frame);
    let addresses = &sections.iter().find(|s| s.0 == "addresses").unwrap().1;
    let stat_address = u64::from_le_bytes(addresses[..8].try_into().unwrap());
    let stat = &sections.iter().find(|s| s.0 == "stat").unwrap().1;
    let pack = Backpack::from_slots(&[]).unwrap();
    let mut work = engine.work();
    let (writes, _) = engine
        .native_words(&candidate(engine), &pack, &mut work)
        .unwrap();
    writes
        .iter()
        .rev()
        .find(|(address, _)| *address == stat_address + offset as u64)
        .map_or_else(
            || u32::from_le_bytes(stat[offset..offset + 4].try_into().unwrap()),
            |(_, value)| *value,
        )
}

#[test]
fn materializer_sources_replace_sample_scalar_constants_once() {
    let root = artifact("native-writers");
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/context-native-writers.json")).unwrap();
    let observed = &oracle["observed"];
    assert_eq!(observed["STAT1C4"]["unassigned_observed_residual_i64"], 0);
    assert_eq!(
        observed["record_plus8"]["first_two_records_match_native_consumer"],
        true
    );
    let (manifest, inventory, frame, corpus) = captured();
    // The live writer proof and this earlier corpus have different attribute
    // seeds. Pin the earlier Native input, then apply only the independently
    // proven ADD32 deltas; do not transplant the live player's 624 seed.
    let seed = corpus["native_input_stat_words"]["0x1c4"].as_u64().unwrap() as u32
        - observed["STAT1C4"]["materializer_group_value_u32"]
            .as_u64()
            .unwrap() as u32;
    let base_attack = corpus["native_input_stat_words"]["0x12c"].as_u64().unwrap() as u32;
    let mut rows = Vec::new();
    for (name, group, delta, expected_capacity, expected_plus8) in [
        ("baseline", 0x9BEC, 0, seed + 11, base_attack + 35),
        ("capacity_removed", 0x9BEC, -11, seed, base_attack + 35),
        ("capacity_raised", 0x9BEC, 7, seed + 18, base_attack + 35),
        ("owner_bonus_removed", 0xF919, -35, seed + 11, base_attack),
        ("owner_bonus_raised", 0xF919, 5, seed + 11, base_attack + 40),
        ("other_mode_control", 0x052D, 9, seed + 11, base_attack + 35),
    ] {
        let mut variant = manifest.clone();
        variant.non_gear.layers.push(SourceLayer { name: name.into(), kind: SourceKind::DerivedNamed, provenance: Provenance { capture_id: oracle["evidence"].as_str().unwrap().into(), source_sha256: oracle["evidence_sha256"].as_str().unwrap().into(), game_version: [2,0,2,0], description: "Synthetic group perturbation checked against the independently replayed native ADD32/mode consumers".into() }, operations: vec![GroupOperation { group, value: delta }] });
        let context = write_context(&root.join(name), &variant, &inventory, &frame);
        let engine = Engine::open_with_context(&bundle(), &context).unwrap();
        let capacity = final_stat_word(&engine, &frame, 0x1C4);
        let first = final_stat_word(&engine, &frame, 0x2C);
        let second = final_stat_word(&engine, &frame, 0x6C);
        assert_eq!(
            capacity, expected_capacity,
            "{name}: capacity source must be applied once"
        );
        assert_eq!(
            [first, second],
            [expected_plus8; 2],
            "{name}: mode-specific source must be applied once"
        );
        assert_eq!(
            final_stat_word(&engine, &frame, 0x12C),
            base_attack,
            "owner bonus is not the base attack group"
        );
        rows.push(json!({"case": name, "group": group, "delta": delta, "capacity": capacity, "record_plus8": [first, second], "context": context.digest()}));
    }
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"passed": true, "independent_native_writer_evidence": oracle, "cases": rows}),
        )
        .unwrap(),
    )
    .unwrap();
    println!("Native writer context evidence: {}", root.display());
}

fn synthetic_scene(
    manifest: &mut ContextManifest,
    inventory: &mut [u8],
    frame: &[u8],
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let parts = sections(frame);
    let addresses = &parts.iter().find(|s| s.0 == "addresses").unwrap().1;
    let main = u64::from_le_bytes(addresses[16..24].try_into().unwrap());
    let base = main & !0xFFFF;
    let flags_at = (main + 0x298 - base) as usize;
    let mut image = base.to_le_bytes().to_vec();
    image.resize(8 + 65_536, 0);
    let flags = 0xCC01_5AA5u32;
    image[8 + flags_at..8 + flags_at + 4].copy_from_slice(&flags.to_le_bytes());
    let mut known = base.to_le_bytes().to_vec();
    known.resize(8 + 8192, 255);
    let mut entry = vec![0u8; 416];
    entry[..8].copy_from_slice(b"NKENTRY1");
    entry[8..12].copy_from_slice(&1u32.to_le_bytes());
    entry[12..16].copy_from_slice(&400u32.to_le_bytes());
    entry[24..32].copy_from_slice(&main.to_le_bytes());
    entry[408..412].copy_from_slice(&0x1F80u32.to_le_bytes());
    inventory[manifest.character.active as usize * 0xF70 + 0xF68] = 1;
    manifest.inventory_sha256 = sha256(inventory);
    manifest.provenance.capture_id = "synthetic-scene-control".into();
    manifest.provenance.description = "Synthetic known MAIN chunk for structural refusal and writer tests only; never executes captured MAIN".into();
    manifest.scenario = Scenario::CapturedScene {
        image_sha256: sha256(&image),
        image_known_sha256: sha256(&known),
        scene_entry_sha256: sha256(&entry),
        attack_weapon_slot: 0,
        main_flags_298: flags,
    };
    (image, known, entry)
}
fn scene_bytes(image: &[u8], known: &[u8], entry: &[u8]) -> SceneBytes {
    SceneBytes::CapturedScene {
        image: image.to_vec().into(),
        image_known: known.to_vec().into(),
        scene_entry: entry.to_vec().into(),
    }
}

#[test]
fn captured_scene_requires_files_and_never_falls_back_to_recorded_paths() {
    let root = artifact("captured-refusal");
    let (mut manifest, mut inventory, frame, _) = captured();
    let (image, known, entry) = synthetic_scene(&mut manifest, &mut inventory, &frame);
    assert!(
        LoadoutContext::new(manifest.clone(), inventory.clone(), frame.clone()).is_err(),
        "a captured tag cannot use the recorded constructor"
    );
    let context_path = root.join("context");
    fs::create_dir_all(&context_path).unwrap();
    for (name, data) in [
        ("inventory.bin", &inventory),
        ("frame.bin", &frame),
        ("image.bin", &image),
    ] {
        fs::write(context_path.join(name), data).unwrap();
    }
    fs::write(
        context_path.join("context.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    assert!(LoadoutContext::load(&context_path)
        .unwrap_err()
        .to_string()
        .contains("image_known.bin"));
    fs::write(context_path.join("image_known.bin"), &known).unwrap();
    assert!(LoadoutContext::load(&context_path)
        .unwrap_err()
        .to_string()
        .contains("scene_entry.bin"));
    fs::write(context_path.join("scene_entry.bin"), &entry).unwrap();
    let context = LoadoutContext::load(&context_path).unwrap();
    let encoded = context.encode(&[]);
    assert!(encoded.is_captured_scene());
    assert_eq!(encoded.file("image.bin"), Some(image.as_slice()));
    assert_eq!(encoded.file("image_known.bin"), Some(known.as_slice()));
    assert_eq!(encoded.file("scene_metadata.bin").unwrap().len(), 16);
    let mut bad_entry = entry.clone();
    bad_entry[8] = 2;
    let mut bad_manifest = manifest.clone();
    if let Scenario::CapturedScene {
        scene_entry_sha256, ..
    } = &mut bad_manifest.scenario
    {
        *scene_entry_sha256 = sha256(&bad_entry);
    }
    assert!(LoadoutContext::new_with_scene(
        bad_manifest,
        inventory.clone(),
        frame.clone(),
        scene_bytes(&image, &known, &bad_entry)
    )
    .is_err());
    let mut unknown = known.clone();
    unknown[8..].fill(0);
    let mut unknown_manifest = manifest;
    if let Scenario::CapturedScene {
        image_known_sha256, ..
    } = &mut unknown_manifest.scenario
    {
        *image_known_sha256 = sha256(&unknown);
    }
    assert!(LoadoutContext::new_with_scene(
        unknown_manifest,
        inventory,
        frame,
        scene_bytes(&image, &unknown, &entry)
    )
    .unwrap_err()
    .to_string()
    .contains("unknown bytes"));
    let attack = root.join("attack-without-library");
    fs::create_dir_all(&attack).unwrap();
    for name in [
        "programs.txt",
        "arena.bin",
        "mapped.bin",
        "tables.bin",
        "frame.bin",
        "characters.bin",
        "gpu_setup.txt",
        "hits.txt",
        "live_groups.txt",
        "survival_groups.txt",
    ] {
        fs::copy(bundle().join(name), attack.join(name)).unwrap();
    }
    let engine = Engine::open_with_context(&attack, &context).unwrap();
    assert!(matches!(
        engine.cpu_diagnostics(),
        nioh3_loadout::engine::CpuDiagnostics::CapturedScenePending
    ));
    let pack = Backpack::from_slots(&[]).unwrap();
    let result = engine.evaluate(
        &candidate(&engine),
        &pack,
        &Limits::NONE,
        &mut engine.work(),
    );
    assert!(
        matches!(result, Outcome::Refused(_)),
        "no recorded-path fallback: {result:?}"
    );
    assert!(matches!(
        engine.cpu_diagnostics(),
        nioh3_loadout::engine::CpuDiagnostics::CapturedSceneUnavailable { .. }
    ));
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"passed": true, "outcome": format!("{result:?}"), "diagnostics": engine.cpu_diagnostics(), "missing_known_mask_refused": true, "missing_entry_refused": true, "unknown_flags_refused": true, "unsupported_entry_version_refused": true, "recorded_fallback_used": false, "scope": "synthetic scene refusal; no full MAIN or GPU execution"})).unwrap()).unwrap();
    println!("Captured scene refusal evidence: {}", root.display());
}

#[test]
fn captured_hell_flag_tracks_each_candidate_and_preserves_other_bytes() {
    let root = artifact("hell-writer");
    let (mut manifest, mut inventory, frame, _) = captured();
    let (image, known, entry) = synthetic_scene(&mut manifest, &mut inventory, &frame);
    let weapon_at = manifest.character.active as usize * 0xF70;
    inventory[weapon_at + 0x1A] |= 0x10;
    manifest.inventory_sha256 = sha256(&inventory);
    let context = LoadoutContext::new_with_scene(
        manifest.clone(),
        inventory.clone(),
        frame.clone(),
        scene_bytes(&image, &known, &entry),
    )
    .unwrap();
    let engine = Engine::open_with_context(&bundle(), &context).unwrap();
    let parts = sections(&frame);
    let addresses = &parts.iter().find(|s| s.0 == "addresses").unwrap().1;
    let target = u64::from_le_bytes(addresses[16..24].try_into().unwrap()) + 0x298;
    let mut ordinary: [u8; 0xE8] = inventory[weapon_at..weapon_at + 0xE8].try_into().unwrap();
    ordinary[0x1A] &= !0x10;
    let mut empty = [0u8; 0xE8];
    empty[0x1A] = 0x10;
    let pack = Backpack::from_slots(&[(0, ordinary), (0, empty)]).unwrap();
    let mut proof = Vec::new();
    for (name, item, expected) in [
        ("retained_hell", None, 0xCC01_5AA5u32),
        ("ordinary_candidate", Some(0), 0xCC00_5AA5),
    ] {
        let mut candidate = candidate(&engine);
        candidate.items[9] = item;
        let mut work = engine.work();
        let (words, _) = engine.native_words(&candidate, &pack, &mut work).unwrap();
        let values: Vec<_> = words.iter().filter(|w| w.0 == target).collect();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].1, expected);
        proof.push(json!({"case": name, "main_address": target, "word": values[0].1, "preserved_other_bytes": values[0].1 & 0xFF00_FFFF == 0xCC00_5AA5}));
    }
    // Removing the equipped weapon changes its shared sources. The legacy
    // context must refuse that candidate; exercise the empty writer with a
    // separate context whose baseline already has the empty slot.
    let mut removed = candidate(&engine);
    removed.items[9] = Some(1);
    assert_eq!(
        engine
            .native_words(&removed, &pack, &mut engine.work())
            .unwrap_err(),
        "legacy shared-source dependencies changed; fresh source rebuild required"
    );
    inventory[weapon_at..weapon_at + 0xE8].copy_from_slice(&empty);
    manifest.inventory_sha256 = sha256(&inventory);
    let empty_context = LoadoutContext::new_with_scene(
        manifest,
        inventory,
        frame,
        scene_bytes(&image, &known, &entry),
    )
    .unwrap();
    let empty_engine = Engine::open_with_context(&bundle(), &empty_context).unwrap();
    let mut work = empty_engine.work();
    let (words, _) = empty_engine
        .native_words(&candidate(&empty_engine), &pack, &mut work)
        .unwrap();
    let flags: Vec<_> = words.iter().filter(|word| word.0 == target).collect();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0].1, 0xCC00_5AA5);
    proof.push(
        json!({"case":"empty_baseline","word":flags[0].1,"removal_requires_source_rebuild":true}),
    );
    let legacy = Engine::open(&bundle()).unwrap();
    let mut work = legacy.work();
    let (words, _) = legacy
        .native_words(&candidate(&legacy), &pack, &mut work)
        .unwrap();
    assert!(
        !words.iter().any(|w| w.0 == target),
        "legacy path does not assume the new writer admission"
    );
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"passed": true, "native_writer": "0x327277 active bank; 0x32728F F68 selector; 0x3272A5 record bit20; 0x3272AE MAIN29A", "scope": "captured admitted normal direct-actor attack only; no MAIN execution", "cases": proof, "legacy_unchanged": true})).unwrap()).unwrap();
    println!("Candidate hell writer evidence: {}", root.display());
}

#[test]
fn equipment_groups_exclude_non_gear_and_work_rebinds_across_contexts() {
    let root = artifact("equipment-groups");
    let (manifest, inventory, frame, proof) = captured();
    let a = LoadoutContext::new(manifest.clone(), inventory.clone(), frame.clone()).unwrap();
    let mut other = changed(manifest.clone());
    other.attrs = manifest.attrs;
    let b = LoadoutContext::new(other, inventory.clone(), frame.clone()).unwrap();
    let ea = Engine::open_with_context(&bundle(), &a).unwrap();
    let eb = Engine::open_with_context(&bundle(), &b).unwrap();
    let pack = Backpack::from_slots(&[]).unwrap();
    let mut shared_work = ea.work();
    let ga = ea
        .equipment_groups(&candidate(&ea), &pack, &mut shared_work)
        .unwrap();
    let gb = eb
        .equipment_groups(&candidate(&eb), &pack, &mut shared_work)
        .unwrap();
    assert!(!ga[0].is_empty());
    assert_eq!(
        ga, gb,
        "fixed non-gear layers must not leak into equipment source extraction"
    );
    assert!(!ga[0].iter().any(|e| e.key == 0xF919));
    let mut altered = manifest;
    altered.character.level += 1;
    altered.character.cap = 20;
    altered.character.plus_cap = 23;
    let altered = LoadoutContext::new(altered, inventory, frame).unwrap();
    let ec = Engine::open_with_context(&bundle(), &altered).unwrap();
    let fa = ea.facts(&candidate(&ea), &pack, &mut shared_work).unwrap();
    let fc = ec.facts(&candidate(&ec), &pack, &mut shared_work).unwrap();
    let fresh_c = ec.facts(&candidate(&ec), &pack, &mut ec.work()).unwrap();
    assert_eq!(
        fc, fresh_c,
        "all character scalars are rebound, not just inventory"
    );
    assert!(fc.health > fa.health);
    assert!(fc.defense < fa.defense);
    assert_eq!(
        ea.facts(&candidate(&ea), &pack, &mut shared_work).unwrap(),
        fa
    );
    let restored = damage(ea.evaluate(&candidate(&ea), &pack, &Limits::NONE, &mut shared_work));
    assert_eq!(
        restored as u64,
        proof["expected_native_bits"].as_u64().unwrap()
    );
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"passed": true, "equipment_groups": ga, "non_gear_excluded": true, "same_work_rebound_a_c_a": true, "a_health": fa.health, "c_health": fc.health, "a_defense": fa.defense, "c_defense": fc.defense, "restored_native_bits": restored})).unwrap()).unwrap();
    println!("Equipment source/rebinding evidence: {}", root.display());
}

#[test]
fn context_baseline_defense_and_captured_weight_do_not_leak_into_rebuilds() {
    let root = artifact("baseline-source-contract");
    let (manifest, inventory, original_frame, _) = captured();
    let original =
        LoadoutContext::new(manifest.clone(), inventory.clone(), original_frame.clone()).unwrap();
    let original_engine = Engine::open_with_context(&bundle(), &original).unwrap();
    let expected_weight = final_stat_word(&original_engine, &original_frame, 0x7B8);

    let mut parts = sections(&original_frame);
    let stat = &mut parts.iter_mut().find(|(name, _)| name == "stat").unwrap().1;
    let captured_weight = u32::from_le_bytes(stat[0x7B8..0x7BC].try_into().unwrap());
    stat[0x7B8..0x7BC].copy_from_slice(&captured_weight.wrapping_add(16).to_le_bytes());
    let weighted_frame = encode_sections(&parts);
    let mut weighted_manifest = manifest.clone();
    weighted_manifest.frame_sha256 = sha256(&weighted_frame);
    let weighted_context =
        LoadoutContext::new(weighted_manifest, inventory.clone(), weighted_frame.clone()).unwrap();
    let weighted_engine = Engine::open_with_context(&bundle(), &weighted_context).unwrap();
    assert_eq!(
        final_stat_word(&weighted_engine, &weighted_frame, 0x7B8),
        expected_weight,
        "captured armor weight must not be added again during a candidate rebuild"
    );

    let mut current_manifest = manifest.clone();
    current_manifest.non_gear.defense = 340;
    let legacy_context = LoadoutContext::new(
        current_manifest.clone(),
        inventory.clone(),
        original_frame.clone(),
    )
    .unwrap();
    let legacy_engine = Engine::open_with_context(&bundle(), &legacy_context).unwrap();
    let legacy_defense = final_stat_word(&legacy_engine, &original_frame, 0x1C8);
    let mut corrected_parts = sections(&original_frame);
    corrected_parts.retain(|(name, _)| name != "baseline_sources");
    corrected_parts.push(("baseline_sources".into(), 340u32.to_le_bytes().to_vec()));
    let corrected_frame = encode_sections(&corrected_parts);
    current_manifest.frame_sha256 = sha256(&corrected_frame);
    let corrected_context = LoadoutContext::new(
        current_manifest.clone(),
        inventory.clone(),
        corrected_frame.clone(),
    )
    .unwrap();
    let corrected_engine = Engine::open_with_context(&bundle(), &corrected_context).unwrap();
    let corrected_defense = final_stat_word(&corrected_engine, &corrected_frame, 0x1C8);
    assert_eq!(
        corrected_defense as i64 - legacy_defense as i64,
        378 - 340,
        "explicit baseline must replace the legacy canonical source value"
    );
    corrected_parts
        .iter_mut()
        .find(|(name, _)| name == "baseline_sources")
        .unwrap()
        .1
        .pop();
    let malformed = encode_sections(&corrected_parts);
    current_manifest.frame_sha256 = sha256(&malformed);
    assert!(LoadoutContext::new(current_manifest, inventory, malformed).is_err());
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({
        "passed": true, "rebuilt_weight": expected_weight, "injected_captured_weight": 16,
        "legacy_defense": legacy_defense, "corrected_defense": corrected_defense,
        "malformed_baseline_refused": true,
        "scope": "Real context loader and candidate preparation over the versioned compatibility corpus; current-player original parity is recorded separately"
    })).unwrap()).unwrap();
}

#[test]
fn fixed_attributes_keep_all_candidates_above_the_old_screening_threshold() {
    use nioh3_loadout::search::{Request, Search};
    let root = artifact("complete-fixed-plan");
    let (manifest, inventory, frame, _) = captured();
    let active = manifest.character.active as usize;
    let context = LoadoutContext::new(manifest, inventory.clone(), frame).unwrap();
    let engine = Engine::open_with_context(&bundle(), &context).unwrap();
    let mut items = Vec::new();
    for slot in 4..=8usize {
        let at = active * 0xF70 + slot * 0xE8;
        let item: [u8; 0xE8] = inventory[at..at + 0xE8].try_into().unwrap();
        // Distinct pool entries are distinct owned-item choices even when their stats tie.
        items.extend((0..10).map(|_| (slot as u32, item)));
    }
    let pack = Backpack::from_slots(&items).unwrap();
    let request = Request {
        sets: vec![],
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    };
    let plan = Search::new(&engine, &pack).plan(&request);
    assert!(
        plan.is_complete(),
        "fixed attributes must not switch to heuristic screening"
    );
    assert_eq!(plan.full_size, 100_000);
    assert_eq!(plan.size(), 100_000);
    assert!(plan.kept_counts().iter().all(|(_, kept, all)| kept == all));
    assert_eq!(plan.candidates(engine.start_attrs).count(), 100_000);
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({
        "passed": true, "fixed_attribute_candidates": plan.size(),
        "all_items_retained": true, "scope": "Production planner above the former32768 threshold; no native or GPU execution"
    })).unwrap()).unwrap();
}
