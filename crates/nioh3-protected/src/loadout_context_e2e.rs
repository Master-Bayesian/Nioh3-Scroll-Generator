//! Actual decoder -> context refresh -> private journal -> replay -> Engine,with retained current-character bytes.
//! The synthetic plain save is only a slot-membership fixture;no source save or game is opened.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::loadout_observation::{self, ObservationOrigin};
use nioh3_loadout::engine::{Backpack, Candidate, Limits, Outcome, SLOTS};
use nioh3_runtime::{
    combat_context::{self, CombatSnapshot},
    FixtureMemory,
};
use std::{fs, path::PathBuf};

fn score(engine: &Engine, attrs: [i32; 7]) -> u32 {
    match engine.evaluate(
        &Candidate {
            attrs,
            items: [None; SLOTS],
        },
        &Backpack {
            pool: Vec::new(),
            ranges: Vec::new(),
            origin: Vec::new(),
        },
        &Limits::NONE,
        &mut engine.work(),
    ) {
        Outcome::Damage(bits) => bits,
        other @ (Outcome::Refused(_) | Outcome::NoPath | Outcome::OutsideLimits) => panic!("Unanswered current outfit: {other:?}"),
    }
}
fn capsule(directory: &Path, id: &str) -> PathBuf {
    directory
        .join("source-observations")
        .join(format!("{id}.json"))
}
fn observe(
    contexts: &mut Contexts,
    state: &Path,
    data: &Path,
    plain: &[u8],
    params: &Value,
    snapshot: &CombatSnapshot,
) -> Result<Value, HostError> {
    let result=contexts.read_with_snapshot_source(
        state,
        data,
        plain,
        params,
        ObservationOrigin::RetainedCaptureReplay,
        crate::loadout_live::SnapshotUse::RetainedDiagnostic,
        || Ok(Some(SnapshotOutcome::Ready(Box::new(snapshot.clone())))),
    );
    if let Ok(value)=&result {
        let contract=crate::Contract::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts")).unwrap();
        let jobs=crate::ProtectedJobs::new();
        let result_value=value.clone();
        let accepted=jobs.start("save.loadout_context",false,move |_|Ok(result_value)).unwrap();
        let id=accepted["job_id"].as_str().unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(5);
        let completed=loop {
            let job=jobs.snapshot(id).unwrap();
            if job["state"]=="completed" { break job; }
            assert!(std::time::Instant::now()<deadline,"Public job envelope did not complete");
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        jobs.join();
        assert!(contract.response_valid(&json!({"protocol":1,"id":"retained-live-refresh","ok":true,"result":completed})),"Live context job violates the actual protected wire schema");
    }
    result
}

#[test]
#[ignore = "Requires the explicit current retained Reaper capture,binding export and evaluator bundle"]
fn retained_reaper_refresh_journal_replay_and_failure_outcomes() {
    let root = PathBuf::from(std::env::var_os("NIOH3_REFRESH_AUDIT_ROOT").expect("refresh root"));
    let data =
        PathBuf::from(std::env::var_os("NIOH3_REFRESH_DATA_ROOT").expect("current data root"));
    let state = root.join(format!(
        "host-refresh-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let directory = state.join("loadout-contexts/kusarigama-reaper-held");
    fs::create_dir_all(&directory).unwrap();
    for name in [
        "context.json",
        "profile.json",
        "inventory.bin",
        "frame.bin",
        "image.bin",
        "image_known.bin",
        "scene_entry.bin",
    ] {
        fs::copy(
            root.join("held-live-context").join(name),
            directory.join(name),
        )
        .unwrap();
    }
    let fixture: Value =
        serde_json::from_slice(&fs::read(root.join("reaper-memory.json")).unwrap()).unwrap();
    let decoded =
        combat_context::read_combat_snapshot(&FixtureMemory::from_json(&fixture).unwrap()).unwrap();
    let SnapshotOutcome::Ready(snapshot) = decoded else {
        panic!("Retained decoder is not ready: {decoded:?}")
    };
    assert_eq!(snapshot.inputs.mode, 1);
    assert_eq!(snapshot.inputs.stats.primary_attack, 2179);
    assert_eq!(snapshot.inputs.stats.defense, 1436);
    let template = LoadoutContext::load(&directory).unwrap();
    let baseline_profile = fs::read(directory.join("profile.json")).unwrap();
    let mut plain = vec![0; nioh3_save::layout::USER_SAVE_BYTES];
    // Populate only the observed equipped E8 bodies at independent synthetic save slots.
    let mut index = 0;
    for bank in 0..2 {
        for slot in 0..16 {
            let start = bank * 0xF70 + slot * 0xE8;
            let record = &snapshot.inputs.inventory[start..start + 0xE8];
            if equipment_slot_is_empty(record) {
                continue;
            }
            let offset = if slot == 15 {
                nioh3_save::layout::slot_offset(bank).unwrap()
            } else {
                let offset = nioh3_save::character::equipment_offset(index).unwrap();
                index += 1;
                offset
            };
            plain[offset..offset + 0xE8].copy_from_slice(record);
        }
    }
    let params = json!({"save_id":"retained-owned-copy","source_sha256":nioh3_loadout::context::sha256(&plain),"bundle":"kusarigama-reaper-held"});
    let bundle = crate::loadout::find_bundle(&data, &params)
        .unwrap()
        .directory(&data);
    let expected = score(
        &Engine::open_with_context(&bundle, &template).unwrap(),
        snapshot.inputs.raw_attrs,
    );
    assert_eq!(f32::from_bits(expected), 10049.0);
    let mut contexts = Contexts::default();
    let mut checks = Vec::new();
    let first = observe(&mut contexts, &state, &data, &plain, &params, &snapshot).unwrap();
    assert_eq!(first["state"], "ready");
    assert_eq!(first["source"], "live");
    assert_eq!(first["school"], "ninja");
    assert_eq!(first["source_recording"]["state"], "recorded");
    let first_id = first["source_recording"]["observation_id"]
        .as_str()
        .unwrap();
    let first_path = capsule(&directory, first_id);
    let first_bytes = fs::read(&first_path).unwrap();
    let recorded: loadout_observation::SourceObservation =
        serde_json::from_slice(&first_bytes).unwrap();
    assert_eq!(
        recorded.snapshot.inputs.inventory,
        snapshot.inputs.inventory
    );
    assert_eq!(recorded.snapshot.source_raw, snapshot.source_raw);
    assert_eq!(recorded.snapshot.core_source_raw, snapshot.core_source_raw);
    assert_eq!(recorded.snapshot.inputs.configured_skills.len(), 839);
    let (replay, refreshed) = loadout_observation::replay(&first_path, &bundle).unwrap();
    assert_eq!(replay["matched"], true);
    assert_eq!(replay["score"]["bits"], expected);
    assert_eq!(first["source_check"], replay["source_check"]);
    let self_contained = loadout_observation::replay(&first_path, Path::new("@recorded"))
        .unwrap()
        .0;
    assert_eq!(self_contained["score"]["bits"], expected);
    assert_eq!(self_contained["context_sha256"], replay["context_sha256"]);
    let mut bound = params.clone();
    bound["context_id"] = first["context_id"].clone();
    assert_eq!(
        score(&contexts.engine(&bound).unwrap(), snapshot.inputs.raw_attrs),
        expected
    );
    let replay_out = state.join("replayed-held-context");
    loadout_observation::export_replayed_context(&first_path, &bundle, &replay_out).unwrap();
    assert_eq!(
        LoadoutContext::load(&replay_out).unwrap().digest(),
        refreshed.digest()
    );
    fs::write(
        root.join("REPLAYED_CONTEXT_LOCATION.json"),
        serde_json::to_vec(&json!({"context":replay_out,"capsule":first_path})).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("REFRESHED_CURRENT.json"),
        serde_json::to_vec_pretty(&replay).unwrap(),
    )
    .unwrap();
    checks.push(json!({"case":"retained_native_decoder_refresh_log_replay_exact_current","passed":true,"source_check":first["source_check"],"damage":f32::from_bits(expected),"observation":first_path}));
    // This is the second real retained state,not a synthetic change to a buff key.
    let consumed: CombatSnapshot =
        serde_json::from_slice(&fs::read(root.join("reaper-after-snapshot.json")).unwrap())
            .unwrap();
    let consumed_result =
        observe(&mut contexts, &state, &data, &plain, &params, &consumed).unwrap();
    assert_eq!(consumed_result["state"], "ready");
    assert!(consumed_result["buffs"].as_array().unwrap().is_empty());
    let consumed_path = capsule(
        &directory,
        consumed_result["source_recording"]["observation_id"]
            .as_str()
            .unwrap(),
    );
    let consumed_replay = loadout_observation::replay(&consumed_path, Path::new("@recorded"))
        .unwrap()
        .0;
    assert_eq!(consumed_replay["score"]["damage"], 10028.0);
    assert_eq!(
        consumed_result["source_reference"],
        first["source_reference"]
    );
    checks.push(json!({"case":"real_consumed_buff_snapshot_records_and_self_replays_without_frozen_bonus","passed":true,"damage":10028,"capsule":consumed_path}));
    // Gain-only changes do not get hidden by an empty group-difference list.
    let mut gain = snapshot.as_ref().clone();
    gain.inputs.stats.primary_attack += 3;
    if let Some(secondary) = &mut gain.inputs.stats.secondary_attack {
        *secondary += 3;
    }
    let changed = contexts
        .read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            ObservationOrigin::ControlledRetainedInput,
            crate::loadout_live::SnapshotUse::RetainedDiagnostic,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(gain.clone())))),
        )
        .unwrap();
    assert_eq!(changed["source_check"]["state"], "reobserved");
    assert_eq!(
        changed["source_check"]["groups"].as_array().unwrap().len(),
        0
    );
    assert_eq!(
        changed["source_check"]["attack_gain"]["now"]
            .as_u64()
            .unwrap(),
        changed["source_check"]["attack_gain"]["stored"]
            .as_u64()
            .unwrap()
            + 3
    );
    assert_eq!(changed["source_reference"], first["source_reference"]);
    assert_eq!(
        fs::read(directory.join("profile.json")).unwrap(),
        baseline_profile
    );
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    let changed_path = capsule(
        &directory,
        changed["source_recording"]["observation_id"]
            .as_str()
            .unwrap(),
    );
    assert!(loadout_observation::replay(&changed_path, &bundle)
        .unwrap()
        .0["matched"]
        .as_bool()
        .unwrap());
    checks.push(json!({"case":"gain_only_change_keeps_immutable_reference_and_old_capsule","passed":true,"source_check":changed["source_check"]}));
    // Index failure preserves a sealed replayable capsule and does not turn a valid read into failure.
    let index_path = directory.join("source-checks.jsonl");
    fs::rename(&index_path, directory.join("retained-source-checks.jsonl")).unwrap();
    fs::create_dir(&index_path).unwrap();
    let snapshot_only = observe(&mut contexts, &state, &data, &plain, &params, &snapshot).unwrap();
    assert_eq!(snapshot_only["state"], "ready");
    assert_eq!(snapshot_only["source_recording"]["state"], "snapshot_only");
    let saved = capsule(
        &directory,
        snapshot_only["source_recording"]["observation_id"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(
        loadout_observation::replay(&saved, &bundle).unwrap().0["matched"],
        true
    );
    fs::remove_dir(&index_path).unwrap();
    checks.push(json!({"case":"index_IO_failure_keeps_capsule_and_ready_result","passed":true}));
    // Journal failure is explicit and nonfatal. Previous records are untouched.
    let journal = directory.join("source-observations");
    fs::rename(&journal, directory.join("retained-source-observations")).unwrap();
    fs::write(&journal, b"controlled journal obstacle").unwrap();
    let not_recorded = observe(&mut contexts, &state, &data, &plain, &params, &snapshot).unwrap();
    assert_eq!(not_recorded["state"], "ready");
    assert_eq!(not_recorded["source_recording"]["state"], "unavailable");
    fs::remove_file(&journal).unwrap();
    fs::rename(directory.join("retained-source-observations"), &journal).unwrap();
    checks.push(json!({"case":"journal_IO_failure_preserves_valid_context","passed":true}));
    // A fixed-weapon search still requires the live weapon TYPE to match the captured attack.
    // Null bundle.weapon_class disables searching weapon records;it must not disable this identity check.
    let mut wrong_weapon = snapshot.as_ref().clone();
    let active = wrong_weapon.inputs.active_bank as usize * 0xF70;
    let other = (1 - wrong_weapon.inputs.active_bank as usize) * 0xF70;
    let other_weapon = wrong_weapon.inputs.inventory[other..other + 0xE8].to_vec();
    wrong_weapon.inputs.inventory[active..active + 0xE8].copy_from_slice(&other_weapon);
    let weapon_error = contexts
        .read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            ObservationOrigin::ControlledRetainedInput,
            crate::loadout_live::SnapshotUse::RetainedDiagnostic,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(wrong_weapon.clone())))),
        )
        .unwrap_err();
    assert!(weapon_error.to_string().contains("WEAPON_MISMATCH"));
    checks.push(
        json!({"case":"fixed_search_scene_still_requires_the_live_weapon_type","passed":true}),
    );
    // Rejected refreshes retain the actual snapshot and primary error.
    let mut wrong = snapshot.as_ref().clone();
    wrong.inputs.mode = 0;
    let error = contexts
        .read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            ObservationOrigin::ControlledRetainedInput,
            crate::loadout_live::SnapshotUse::RetainedDiagnostic,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(wrong.clone())))),
        )
        .unwrap_err();
    assert!(error.to_string().contains("SCHOOL_MISMATCH"));
    assert!(
        contexts.engine(&bound).is_err(),
        "Failed refresh must retire old authority"
    );
    let rejected = fs::read_dir(&journal)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|s| s == "json"))
        .find(|entry| {
            let v: Value = serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
            v["terminal"]["state"] == "rejected"
                && v["terminal"]["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("SCHOOL_MISMATCH"))
        })
        .unwrap()
        .path();
    let rejected_replay = loadout_observation::replay(&rejected, &bundle).unwrap().0;
    assert_eq!(rejected_replay["terminal"], "rejected");
    checks.push(json!({"case":"rejected_refresh_keeps_primary_error_and_replay","passed":true}));
    // A deliberate input edit cannot pass the capsule's content identity.
    let tamper = state.join("tamper");
    fs::create_dir(&tamper).unwrap();
    let tampered = tamper.join(first_path.file_name().unwrap());
    let mut changed_json: Value = serde_json::from_slice(&first_bytes).unwrap();
    changed_json["snapshot"]["inputs"]["stats"]["primary_attack"] = json!(1);
    fs::write(&tampered, serde_json::to_vec(&changed_json).unwrap()).unwrap();
    assert!(loadout_observation::replay(&tampered, &bundle)
        .unwrap_err()
        .to_string()
        .contains("content identity differs"));
    checks.push(json!({"case":"deliberate_capsule_input_corruption_rejected","passed":true}));
    let report = json!({"passed":true,"checks":checks,"reference_context":template.digest(),"new_context":refreshed.digest(),"raw_snapshot_origin":"retained first Ninja capture plus declared same-process source traversal","live_game_accessed":false,"source_save_written":false,"state":state,"scope":"Actual production decoder,Contexts refresh,Engine,journal and replay. Synthetic save-slot membership only. Controlled gain and I/O/error variants are not natural game observations."});
    fs::write(
        root.join("REFRESH_JOURNAL_E2E.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}

#[test]
#[ignore = "Requires both retained Reaper memory states, the complete captured context and evaluator bundle; no game access"]
fn retained_current_admission_records_and_replays_the_exact_policy() {
    use crate::loadout_live::SnapshotUse;
    use crate::loadout_observation::{
        ObservationReference, ObservationTerminal, SourceObservation,
    };

    let root = PathBuf::from(std::env::var_os("NIOH3_REFRESH_AUDIT_ROOT").expect("refresh root"));
    let data =
        PathBuf::from(std::env::var_os("NIOH3_REFRESH_DATA_ROOT").expect("current data root"));
    let state = root.join(format!(
        "current-admission-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let directory = state.join("loadout-contexts/kusarigama-reaper-held");
    fs::create_dir_all(&directory).unwrap();
    let baseline_files: Vec<_> = [
        "context.json",
        "profile.json",
        "inventory.bin",
        "frame.bin",
        "image.bin",
        "image_known.bin",
        "scene_entry.bin",
    ]
    .into_iter()
    .map(|name| {
        let bytes = fs::read(root.join("held-live-context").join(name)).unwrap();
        fs::write(directory.join(name), &bytes).unwrap();
        (name, bytes)
    })
    .collect();
    // Decode the retained memory again. The historical typed JSON intentionally
    // lacks owner, secondary-channel and raw graph evidence required by this policy.
    let decode = |stem: &str| {
        let fixture: Value =
            serde_json::from_slice(&fs::read(root.join(format!("{stem}-memory.json"))).unwrap())
                .unwrap();
        let decoded =
            combat_context::read_combat_snapshot(&FixtureMemory::from_json(&fixture).unwrap())
                .unwrap();
        let SnapshotOutcome::Ready(snapshot) = decoded else {
            panic!("Retained production decoder refused {stem}: {decoded:?}")
        };
        snapshot
    };
    let first_snapshot = decode("reaper");
    let after_snapshot = decode("reaper-after");
    assert_eq!(first_snapshot.inputs.stats.secondary_attack, Some(2034));
    assert_eq!(first_snapshot.inputs.owner_flags_188, Some(463036225));
    assert!(!first_snapshot.inputs.status_keys.is_empty());
    assert_eq!(
        first_snapshot
            .status_tree_raw
            .as_ref()
            .unwrap()
            .objects
            .len(),
        1
    );
    assert!(after_snapshot.inputs.status_keys.is_empty());
    assert!(after_snapshot
        .status_tree_raw
        .as_ref()
        .unwrap()
        .nodes
        .is_empty());
    let template = LoadoutContext::load(&directory).unwrap();
    let template_profile: Value =
        serde_json::from_slice(&fs::read(directory.join("profile.json")).unwrap()).unwrap();
    let reference = ObservationReference::new(&template, &template_profile).unwrap();
    let mut plain = vec![0; nioh3_save::layout::USER_SAVE_BYTES];
    let mut index = 0;
    for bank in 0..2 {
        for slot in 0..16 {
            let start = bank * 0xF70 + slot * 0xE8;
            let record = &first_snapshot.inputs.inventory[start..start + 0xE8];
            if equipment_slot_is_empty(record) {
                continue;
            }
            let offset = if slot == 15 {
                nioh3_save::layout::slot_offset(bank).unwrap()
            } else {
                let offset = nioh3_save::character::equipment_offset(index).unwrap();
                index += 1;
                offset
            };
            plain[offset..offset + 0xE8].copy_from_slice(record);
        }
    }
    let params = json!({"save_id":"retained-current-policy-owned-copy",
        "source_sha256":nioh3_loadout::context::sha256(&plain),"bundle":"kusarigama-reaper-held"});
    let current = |contexts: &mut Contexts, snapshot: &CombatSnapshot| {
        contexts.read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            ObservationOrigin::ControlledRetainedInput,
            SnapshotUse::CurrentProcess,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(snapshot.clone())))),
        )
    };
    let mut contexts = Contexts::default();
    let mut checks = Vec::new();
    let mut sealed = Vec::new();
    let mut sealed_reference = Vec::new();
    for (name, snapshot, damage) in [
        ("first", first_snapshot.as_ref(), 10049.0f32),
        ("after", after_snapshot.as_ref(), 10028.0f32),
    ] {
        let result = current(&mut contexts, snapshot).unwrap();
        assert_eq!(result["state"], "ready");
        assert_eq!(result["source_recording"]["state"], "recorded");
        assert_eq!(
            result["source_reference"],
            serde_json::to_value(&reference).unwrap()
        );
        let path = capsule(
            &directory,
            result["source_recording"]["observation_id"]
                .as_str()
                .unwrap(),
        );
        let bytes = fs::read(&path).unwrap();
        let recorded: SourceObservation = serde_json::from_slice(&bytes).unwrap();
        if sealed_reference.is_empty() {
            for (name, _) in &baseline_files {
                let baseline = directory
                    .join("source-observations/baselines")
                    .join(&reference.id)
                    .join(name);
                sealed_reference.push((baseline.clone(), fs::read(baseline).unwrap()));
            }
        }
        assert!(matches!(
            recorded.origin,
            ObservationOrigin::ControlledRetainedInput
        ));
        assert_eq!(recorded.snapshot_use, Some(SnapshotUse::CurrentProcess));
        assert_eq!(recorded.reference, reference);
        assert_eq!(
            serde_json::to_value(&recorded.snapshot).unwrap(),
            serde_json::to_value(snapshot).unwrap()
        );
        let (receipt, replayed) =
            loadout_observation::replay(&path, Path::new("@recorded")).unwrap();
        assert_eq!(receipt["matched"], true);
        assert_eq!(receipt["terminal"], "ready");
        assert_eq!(receipt["snapshot_use"], "current_process");
        assert_eq!(receipt["snapshot_use_binding"], "recorded_policy");
        assert_eq!(receipt["recorded_origin"], "controlled_retained_input");
        assert_eq!(receipt["execution_origin"], "retained_capture_replay");
        assert_eq!(receipt["live_process_accessed"], false);
        assert_eq!(receipt["score"]["bits"], damage.to_bits());
        assert_eq!(receipt["source_check"], result["source_check"]);
        let ObservationTerminal::Ready { context_sha256, .. } = &recorded.terminal else {
            panic!("Ready admission recorded a rejected terminal")
        };
        assert_eq!(replayed.digest(), context_sha256);
        let mut bound = params.clone();
        bound["context_id"] = result["context_id"].clone();
        assert_eq!(
            score(&contexts.engine(&bound).unwrap(), snapshot.inputs.raw_attrs),
            damage.to_bits()
        );
        fs::write(
            state.join(format!("{name}-replay.json")),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        checks.push(json!({"case":name,"passed":true,"damage":damage,"capsule":path,
            "context_sha256":replayed.digest(),"snapshot_use":"current_process","origin":"controlled_retained_input"}));
        sealed.push((path, bytes));
    }

    let mut secondary = first_snapshot.as_ref().clone();
    *secondary.inputs.stats.secondary_attack.as_mut().unwrap() += 1;
    let mut owner = first_snapshot.as_ref().clone();
    *owner.inputs.owner_flags_188.as_mut().unwrap() ^= 2;
    let mut object = first_snapshot.as_ref().clone();
    object.status_tree_raw.as_mut().unwrap().objects[0]
        .bytes
        .as_mut()
        .unwrap()[0xA8] ^= 1;
    assert_eq!(object.inputs.status_keys, first_snapshot.inputs.status_keys);
    let mut missing_status = first_snapshot.as_ref().clone();
    missing_status.status_tree_raw = None;
    let mut missing_owner = first_snapshot.as_ref().clone();
    missing_owner.inputs.owner_flags_188 = None;
    for (name, snapshot, expected_error) in [
        (
            "secondary_only_change",
            secondary,
            "LOADOUT_CONTEXT_SECONDARY_CHANNEL_UNSUPPORTED",
        ),
        (
            "owner_branch_change",
            owner,
            "LOADOUT_CONTEXT_RESOURCE_BRANCH_UNSUPPORTED",
        ),
        (
            "same_key_type54_payload_change",
            object,
            "LOADOUT_CONTEXT_STATUS_OBJECT_UNSUPPORTED",
        ),
        (
            "nonempty_status_missing_raw",
            missing_status,
            "LOADOUT_CONTEXT_STATUS_PROVENANCE_MISSING",
        ),
        (
            "owner_branch_missing_provenance",
            missing_owner,
            "LOADOUT_CONTEXT_OWNER_BRANCH_PROVENANCE_MISSING",
        ),
    ] {
        snapshot.validate().unwrap();
        // Arm actual context authority before each refusal, then verify the
        // production replacement path retires it even when admission fails.
        let armed = current(&mut contexts, &first_snapshot).unwrap();
        let mut bound = params.clone();
        bound["context_id"] = armed["context_id"].clone();
        assert!(contexts.engine(&bound).is_ok());
        let before = fs::read_to_string(directory.join("source-checks.jsonl"))
            .unwrap()
            .lines()
            .count();
        let error = current(&mut contexts, &snapshot).unwrap_err();
        assert_eq!(error.to_string(), expected_error);
        assert_eq!(
            contexts
                .engine(&bound)
                .err()
                .expect("retired context")
                .to_string(),
            "OPTIMIZER_CONTEXT_STALE"
        );
        let journal = fs::read_to_string(directory.join("source-checks.jsonl")).unwrap();
        assert_eq!(journal.lines().count(), before + 1);
        let entry: Value = serde_json::from_str(journal.lines().last().unwrap()).unwrap();
        let path = capsule(&directory, entry["observation_id"].as_str().unwrap());
        let bytes = fs::read(&path).unwrap();
        let recorded: SourceObservation = serde_json::from_slice(&bytes).unwrap();
        assert!(matches!(
            recorded.origin,
            ObservationOrigin::ControlledRetainedInput
        ));
        assert_eq!(recorded.snapshot_use, Some(SnapshotUse::CurrentProcess));
        assert_eq!(recorded.reference, reference);
        assert_eq!(
            serde_json::to_value(&recorded.snapshot).unwrap(),
            serde_json::to_value(&snapshot).unwrap()
        );
        let ObservationTerminal::Rejected { message } = &recorded.terminal else {
            panic!("Rejected admission recorded a ready terminal")
        };
        assert_eq!(message, expected_error);
        let receipt = loadout_observation::replay(&path, Path::new("@recorded"))
            .unwrap()
            .0;
        assert_eq!(receipt["matched"], true);
        assert_eq!(receipt["terminal"], "rejected");
        assert_eq!(receipt["message"], expected_error);
        assert_eq!(receipt["snapshot_use"], "current_process");
        assert_eq!(receipt["snapshot_use_binding"], "recorded_policy");
        assert_eq!(receipt["recorded_origin"], "controlled_retained_input");
        assert_eq!(receipt["execution_origin"], "retained_capture_replay");
        assert_eq!(receipt["live_process_accessed"], false);
        fs::write(
            state.join(format!("{name}-replay.json")),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        checks.push(
            json!({"case":name,"passed":true,"rejection":expected_error,"capsule":path,
            "exact_rejected_replay":true,"retired_previous_context":true}),
        );
        sealed.push((path, bytes));
    }
    // Changing only the admission policy cannot preserve the content identity.
    let tamper = state.join("tampered-policy");
    fs::create_dir(&tamper).unwrap();
    let mut changed: Value = serde_json::from_slice(&sealed[0].1).unwrap();
    changed["snapshot_use"] = json!("retained_diagnostic");
    let tampered = tamper.join(sealed[0].0.file_name().unwrap());
    fs::write(&tampered, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        loadout_observation::replay(&tampered, Path::new("@recorded"))
            .unwrap_err()
            .to_string()
            .contains("source capsule content identity differs")
    );
    checks.push(json!({"case":"policy_content_identity_tamper","passed":true}));
    for line in fs::read_to_string(directory.join("source-checks.jsonl"))
        .unwrap()
        .lines()
    {
        let entry: Value = serde_json::from_str(line).unwrap();
        let bytes = fs::read(capsule(
            &directory,
            entry["observation_id"].as_str().unwrap(),
        ))
        .unwrap();
        assert_eq!(
            entry["capsule_sha256"],
            nioh3_loadout::context::sha256(&bytes)
        );
        assert_eq!(entry["reference_id"], reference.id);
    }
    for (name, bytes) in &baseline_files {
        assert_eq!(
            fs::read(directory.join(name)).unwrap(),
            *bytes,
            "Mutable reference file: {name}"
        );
    }
    for (path, bytes) in &sealed_reference {
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (path, bytes) in &sealed {
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    let report = json!({"passed":true,"checks":checks,"reference":reference,"state":state,
        "live_process_accessed":false,"source_save_written":false,
        "scope":"Production memory decoder, Contexts current admission, immutable journal, exact self-contained replay and host Engine. Retained controlled inputs and synthetic save membership; no new game read or source-causality acceptance."});
    fs::write(
        root.join("CURRENT_ADMISSION_POLICY_E2E.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}

#[cfg(windows)]
#[test]
#[ignore = "Requires the retained Ninja snapshot and original held-MAIN replay resources"]
fn refreshed_status_words_match_original_machine_code() {
    use nioh3_loadout::native::Native;
    let root = PathBuf::from(std::env::var_os("NIOH3_REFRESH_AUDIT_ROOT").unwrap());
    let template = LoadoutContext::load(&root.join("held-live-context")).unwrap();
    let snapshot: CombatSnapshot =
        serde_json::from_slice(&fs::read(root.join("reaper-snapshot.json")).unwrap()).unwrap();
    let profile: Value =
        serde_json::from_slice(&fs::read(root.join("held-live-context/profile.json")).unwrap())
            .unwrap();
    let owner = profile["bindings"]["canonical_source_owner"]
        .as_u64()
        .unwrap();
    let bundle =
        Path::new("F:/Nioh3_r16_build/deliverables/optimizer-continuation-20261114/held-bundle");
    let resource:Value=serde_json::from_slice(&fs::read("F:/Nioh3_r16_build/deliverables/ki-write-live-20261113/MATERIALIZER_CURRENT_INPUTS.json").unwrap()).unwrap();
    let stat_base: Vec<u8> = serde_json::from_value(resource["stat"].clone()).unwrap();
    let d_base: Vec<u8> = serde_json::from_value(resource["d_base"].clone()).unwrap();
    let addresses: Vec<u64> = serde_json::from_value(resource["addresses"].clone()).unwrap();
    let mut native = Native::open(&root.join("native-status-control")).unwrap();
    let pack = Backpack {
        pool: Vec::new(),
        ranges: Vec::new(),
        origin: Vec::new(),
    };
    let candidate = Candidate {
        attrs: snapshot.inputs.raw_attrs,
        items: [None; SLOTS],
    };
    let mut results = Vec::new();
    for (name, class0, class1) in [
        (
            "captured",
            snapshot.inputs.status_class0_bits,
            snapshot.inputs.status_class1_bits,
        ),
        (
            "class0_control",
            1.1f32.to_bits(),
            snapshot.inputs.status_class1_bits,
        ),
        (
            "class1_control",
            snapshot.inputs.status_class0_bits,
            1.1f32.to_bits(),
        ),
        ("both_control", 1.1f32.to_bits(), 1.1f32.to_bits()),
    ] {
        let mut input = snapshot.inputs.clone();
        input.status_class0_bits = class0;
        input.status_class1_bits = class1;
        let mut metadata = profile.clone();
        let refreshed =
            crate::loadout_live::refresh(&template, bundle, &mut metadata, &input).unwrap();
        let engine = Engine::open_with_context(bundle, &refreshed.context).unwrap();
        let cpu = score(&engine, candidate.attrs);
        let mut work = engine.work();
        let (words, hits) = engine.native_words(&candidate, &pack, &mut work).unwrap();
        let mut stat = stat_base.clone();
        let mut d = d_base.clone();
        for &(address, value) in words {
            for (bytes, start) in [(&mut stat, addresses[0]), (&mut d, addresses[1])] {
                if address >= start && address + 4 <= start + bytes.len() as u64 {
                    let offset = (address - start) as usize;
                    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
        }
        let ki = nioh3_loadout::sustained::reaper_ki_after_hits(&stat, &d).unwrap();
        let mut total = 0.0f32;
        let mut segments = Vec::new();
        for (step, hit) in hits.iter().enumerate() {
            native.reset();
            assert_eq!(native.write_words(words), words.len() * 4);
            assert_eq!(
                native.write_words(&[(owner + 0x2E3C, class0), (owner + 0x2E40, class1)]),
                8
            );
            assert_eq!(native.write_words(hit), hit.len() * 4);
            assert_eq!(
                native.write_words(&[(resource["hp_owner"].as_u64().unwrap() + 0xAC, ki[step])]),
                4
            );
            let bits = native.run().unwrap();
            segments.push(f32::from_bits(bits));
            total += f32::from_bits(bits);
        }
        results.push(json!({"case":name,"class0_bits":class0,"class1_bits":class1,"cpu":f32::from_bits(cpu),"original":total,"matched":cpu==total.to_bits(),"segments":segments}));
    }
    let passed = results.iter().all(|row| row["matched"] == true);
    let report = json!({"passed":passed,"results":results,"scope":"Controlled input-propagation sensitivity,not a claim that these scalar changes alone describe a natural buff. Actual refresh/Engine versus independent original owner-word writes;ordered full sequence,no game access."});
    fs::write(
        root.join("STATUS_WORD_NATIVE_PARITY.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
    assert!(
        passed,
        "Current status input propagation differs from original execution"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "Requires both retained Reaper states and original replay;optional GPU flag is explicit"]
fn consumed_single_use_buff_rebinds_first_hit_without_frozen_bonus() {
    use nioh3_loadout::{context::SceneBytes, engine::Kernel, gpu::CardOutcome, native::Native};
    use nioh3_runtime::InventoryMemory;
    use std::sync::Arc;
    let root = PathBuf::from(std::env::var_os("NIOH3_REFRESH_AUDIT_ROOT").unwrap());
    let template = LoadoutContext::load(&root.join("held-live-context")).unwrap();
    let input: CombatSnapshot =
        serde_json::from_slice(&fs::read(root.join("reaper-after-snapshot.json")).unwrap())
            .unwrap();
    assert!(input.inputs.status_keys.is_empty());
    assert_eq!(input.inputs.status_single_use_bits, [1.0f32.to_bits(); 2]);
    let mut metadata: Value =
        serde_json::from_slice(&fs::read(root.join("held-live-context/profile.json")).unwrap())
            .unwrap();
    let owner = metadata["bindings"]["canonical_source_owner"]
        .as_u64()
        .unwrap();
    let bundle =
        Path::new("F:/Nioh3_r16_build/deliverables/optimizer-continuation-20261114/held-bundle");
    let current = crate::loadout_live::refresh(&template, bundle, &mut metadata, &input.inputs)
        .unwrap()
        .context;
    let encoded = current.encode(&[]);
    assert!(
        encoded.file("hits.txt").is_some(),
        "Consumed buff must replace captured first-hit values"
    );
    let engine = Engine::open_with_context(bundle, &current).unwrap();
    let candidate = Candidate {
        attrs: input.inputs.raw_attrs,
        items: [None; SLOTS],
    };
    let pack = Backpack {
        pool: Vec::new(),
        ranges: Vec::new(),
        origin: Vec::new(),
    };
    let cpu = score(&engine, candidate.attrs);
    let mut work = engine.work();
    let (words, hits) = engine.native_words(&candidate, &pack, &mut work).unwrap();
    let resource:Value=serde_json::from_slice(&fs::read("F:/Nioh3_r16_build/deliverables/ki-write-live-20261113/MATERIALIZER_CURRENT_INPUTS.json").unwrap()).unwrap();
    let mut stat: Vec<u8> = serde_json::from_value(resource["stat"].clone()).unwrap();
    let mut d: Vec<u8> = serde_json::from_value(resource["d_base"].clone()).unwrap();
    let addresses: Vec<u64> = serde_json::from_value(resource["addresses"].clone()).unwrap();
    for &(address, value) in words {
        for (bytes, start) in [(&mut stat, addresses[0]), (&mut d, addresses[1])] {
            if address >= start && address + 4 <= start + bytes.len() as u64 {
                let offset = (address - start) as usize;
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    let ki = nioh3_loadout::sustained::reaper_ki_after_hits(&stat, &d).unwrap();
    let memory: Value =
        serde_json::from_slice(&fs::read(root.join("reaper-after-memory.json")).unwrap()).unwrap();
    let memory = FixtureMemory::from_json(&memory).unwrap();
    let head = u64::from_le_bytes(memory.read(owner + 0x3108, 8).unwrap().try_into().unwrap());
    let head_bytes = memory.read(head, 0x30).unwrap();
    let mut independent = vec![
        (owner + 0x2E34, 1.0f32.to_bits()),
        (owner + 0x2E38, 1.0f32.to_bits()),
        (owner + 0x3110, 0),
    ];
    for (offset, word) in head_bytes.as_chunks::<4>().0.iter().enumerate() {
        independent.push((
            head + offset as u64 * 4,
            u32::from_le_bytes(*word),
        ));
    }
    let mut native = Native::open(Path::new(
        "F:/Nioh3_r16_build/deliverables/sustained-actions-20261111/reaper-held-native",
    ))
    .unwrap();
    let mut total = 0.0f32;
    let mut segments = Vec::new();
    for (step, hit) in hits.iter().enumerate() {
        native.reset();
        assert_eq!(native.write_words(words), words.len() * 4);
        assert_eq!(native.write_words(hit), hit.len() * 4);
        // Independent values come from the actual second captured state,not the new runtime hit table.
        assert_eq!(native.write_words(&independent), independent.len() * 4);
        assert_eq!(native.write_words(&[(owner + 0xAC, ki[step])]), 4);
        let value = f32::from_bits(native.run().unwrap());
        total += value;
        segments.push(value);
    }
    assert_eq!(
        cpu,
        total.to_bits(),
        "Current no-buff refresh differs from original captured-empty-state writes"
    );
    // Deliberately omit the runtime hit buffer:the old frozen1.06 entry must be detected.
    let mut old_manifest = current.manifest().clone();
    old_manifest.frame_sha256 = nioh3_loadout::context::sha256(template.frame());
    let legacy = LoadoutContext::new_with_scene(
        old_manifest,
        current.inventory().to_vec(),
        template.frame().to_vec(),
        SceneBytes::CapturedScene {
            image: Arc::from(encoded.file("image.bin").unwrap()),
            image_known: Arc::from(encoded.file("image_known.bin").unwrap()),
            scene_entry: Arc::from(encoded.file("scene_entry.bin").unwrap()),
        },
    )
    .unwrap();
    let frozen = score(
        &Engine::open_with_context(bundle, &legacy).unwrap(),
        candidate.attrs,
    );
    assert_ne!(
        frozen, cpu,
        "Negative control did not detect a frozen first-hit buff"
    );
    let gpu = if std::env::var("NIOH3_REFRESH_CHECK_GPU").ok().as_deref() == Some("1") {
        std::env::set_var("NIOH3_LOADOUT_GPU", "1");
        let mut kernel = engine.kernel();
        let Kernel::Ready(card) = &mut *kernel else {
            panic!("Explicit GPU control unavailable")
        };
        card.use_pool(&[]).unwrap();
        let values = card.cases(&[candidate], &Limits::NONE).unwrap();
        let CardOutcome::Damage(bits) = values[0] else {
            panic!("GPU did not answer no-buff control")
        };
        assert_eq!(bits, cpu);
        Some(f32::from_bits(bits))
    } else {
        None
    };
    let output = root.join(format!("no-buff-context-{}", std::process::id()));
    fs::create_dir(&output).unwrap();
    fs::write(
        output.join("context.json"),
        serde_json::to_vec_pretty(current.manifest()).unwrap(),
    )
    .unwrap();
    fs::write(output.join("frame.bin"), current.frame()).unwrap();
    fs::write(output.join("inventory.bin"), current.inventory()).unwrap();
    for name in ["image.bin", "image_known.bin", "scene_entry.bin"] {
        fs::write(output.join(name), encoded.file(name).unwrap()).unwrap();
    }
    fs::write(
        output.join("profile.json"),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
    let report = json!({"passed":true,"cpu":f32::from_bits(cpu),"original":total,"GPU":gpu,"legacy_frozen_bonus":f32::from_bits(frozen),"segments":segments,"context":output,"scope":"Actual retained consumed-buff snapshot. Production refresh compared with original code after independent captured scalar/tree writes. Missing-hit-buffer mutant fails. No game access;explicit optional real GPU."});
    fs::write(
        root.join("CONSUMED_BUFF_NATIVE_PARITY.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{report}");
}
