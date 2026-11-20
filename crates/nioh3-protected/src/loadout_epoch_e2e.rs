#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Normal source binding and native damage checks over one real source epoch.

use super::*;
use nioh3_loadout::{
    damage::DamageDetails,
    engine::{Backpack, Candidate, SLOTS},
};
use nioh3_save::character::{equipment_offset, EQUIPMENT_REGION_END};
use std::fs;

fn owned_fixture(context: &LoadoutContext) -> Vec<u8> {
    let scroll_end = nioh3_save::layout::slot_offset(nioh3_save::layout::SCROLL_SLOT_COUNT - 1)
        .expect("scroll at bounded slot")
        + 0xe8;
    let mut plain = vec![0; EQUIPMENT_REGION_END.max(scroll_end)];
    let bank = context.manifest().character.active as usize * 0xf70;
    for slot in [0usize, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15] {
        let native = bank + slot * 0xe8;
        let at = if slot == 15 {
            nioh3_save::layout::slot_offset(0).unwrap()
        } else {
            equipment_offset(slot).unwrap()
        };
        plain[at..at + 0xe8].copy_from_slice(&context.inventory()[native..native + 0xe8]);
    }
    plain
}

#[test]
#[ignore = "Requires the current source-epoch app fixture and freshly built Windows CPU library"]
fn retained_source_epoch_refresh_publishes_only_verified_bundle() {
    let root = PathBuf::from(
        std::env::var_os("NIOH3_RETAINED_SOURCE_REFRESH_FIXTURE")
            .expect("current source-epoch fixture dir"),
    );
    let data = root.join("data");
    let capture = LoadoutContext::load(&data.join("loadout/kusarigama-strong/context")).unwrap();
    let input = root.join("EPOCH_INPUT.json");
    let raw = fs::read(&input).unwrap();
    let receipt: Value = serde_json::from_slice(&raw).unwrap();
    let prior = root.join("prior");
    let previous_data = prior.join("data");
    let previous =
        LoadoutContext::load(&previous_data.join("loadout/kusarigama-strong/context")).unwrap();
    let mut plain = owned_fixture(&capture);
    let glove = capture.manifest().character.active as usize * 0xF70 + 6 * 0xE8;
    let save_slot = equipment_offset(120).unwrap();
    plain[save_slot..save_slot + 0xE8].copy_from_slice(&previous.inventory()[glove..glove + 0xE8]);
    let params = json!({"save_id":"a".repeat(64),
        "source_sha256":nioh3_loadout::context::sha256(&plain),"bundle":"kusarigama-strong"});
    let mut contexts = Contexts::default();
    let earlier = contexts
        .read_replayed_epoch(&previous_data, &plain, &params, || {
            Ok(prior.join("EPOCH_INPUT.json"))
        })
        .unwrap();
    let mut earlier_search = params.clone();
    earlier_search["context_id"] = earlier["context_id"].clone();
    let earlier_engine = contexts.engine(&earlier_search).unwrap();
    let earlier_candidate = Candidate {
        attrs: earlier_engine.start_attrs,
        items: [None; SLOTS],
    };
    let empty_pool = Backpack::from_slots(&[]).unwrap();
    let older_damage =
        earlier_engine.damage_details(&earlier_candidate, &empty_pool, &mut earlier_engine.work());
    assert!(
        matches!(older_damage, DamageDetails::Available{total, physical, elemental, ..}
        if (total, physical, elemental) == (633.0, 614.0, 19.0))
    );
    let newer = contexts
        .read_replayed_epoch(&data, &plain, &params, || Ok(input.clone()))
        .unwrap();
    assert_eq!(newer["state"], "ready");
    assert_ne!(newer["context_id"], earlier["context_id"]);
    assert!(
        contexts.engine(&earlier_search).is_err(),
        "successful new source import kept an old handle"
    );

    let mut controls = vec![json!({
        "case":"actual_earlier_to_latest_epoch",
        "earlier_score":633, "latest_expected_score":634, "previous_retired":true,
    })];
    for (name, pointer, alternative, error) in [
        (
            "wrong_generation",
            "/generation",
            json!("different-generation"),
            "compiler generation and frame differ",
        ),
        (
            "wrong_capture",
            "/capture_sha256",
            json!("b".repeat(64)),
            "different captures",
        ),
        (
            "wrong_prefix",
            "/native_stages/prefix/0/base",
            json!(u32::MAX),
            "replayed source map differs",
        ),
        (
            "wrong_native_damage",
            "/damage/2",
            json!((18.0f32).to_bits()),
            "original damage evidence",
        ),
    ] {
        let ready = contexts
            .read_replayed_epoch(&data, &plain, &params, || Ok(input.clone()))
            .unwrap();
        assert_eq!(ready["state"], "ready");
        assert_eq!(ready["source"], "captured");
        let mut search = params.clone();
        search["context_id"] = ready["context_id"].clone();
        let engine = contexts.engine(&search).unwrap();
        let candidate = Candidate {
            attrs: engine.start_attrs,
            items: [None; SLOTS],
        };
        let pack = Backpack::from_slots(&[]).unwrap();
        let damage = engine.damage_details(&candidate, &pack, &mut engine.work());
        assert!(
            matches!(damage, DamageDetails::Available{total, physical, elemental, ..}
            if (total, physical, elemental) == (634.0, 615.0, 19.0))
        );
        let mut bad = receipt.clone();
        *bad.pointer_mut(pointer).expect("mutated evidence field") = alternative;
        let path = root.join(format!("{name}.json"));
        fs::write(&path, serde_json::to_vec_pretty(&bad).unwrap()).unwrap();
        let result = contexts.read_replayed_epoch(&data, &plain, &params, || Ok(path));
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains(error),
            "unexpected {name} error: {message}"
        );
        assert!(
            contexts.engine(&search).is_err(),
            "failed import kept the old generation"
        );
        controls.push(json!({"case":name,"rejected":true,"previous_retired":true}));
    }

    let ready = contexts
        .read_replayed_epoch(&data, &plain, &params, || Ok(input.clone()))
        .unwrap();
    let mut search = params.clone();
    search["context_id"] = ready["context_id"].clone();
    let not_registered = root.join("unregistered-data");
    let miss = contexts.read_replayed_epoch(&not_registered, &plain, &params, || Ok(input.clone()));
    assert!(miss.is_err());
    assert!(
        contexts.engine(&search).is_err(),
        "missing bundle kept its old generation"
    );
    controls.push(json!({"case":"missing_bundle","rejected":true,"previous_retired":true}));

    let ready = contexts
        .read_replayed_epoch(&data, &plain, &params, || Ok(input))
        .unwrap();
    assert_eq!(ready["state"], "ready");
    fs::write(
        root.join("APP_EPOCH_IMPORT_E2E.json"),
        serde_json::to_vec_pretty(&json!({
            "passed":true,"fresh_verified_capture_ready":true,"native_channels":[634,615,19],
            "controls":controls,"game_access":false,"runtime_import_ready":false,
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "Requires the post-rebuild Windows app fixture and current-scene byte evidence"]
fn production_live_epoch_receiver_requires_current_process_and_scene() {
    use crate::loadout_live::SnapshotUse;
    use crate::loadout_observation::ObservationOrigin;
    use nioh3_runtime::combat_context::{
        CombatSnapshot, CompiledSourceObservation, SnapshotOutcome,
    };

    let test_root = PathBuf::from(
        std::env::var_os("NIOH3_LIVE_EPOCH_RECEIVER_FIXTURE")
            .expect("prepared live source receiver fixture"),
    );
    let active = test_root.join("identities.json");
    let info: Value = serde_json::from_slice(&fs::read(active).unwrap()).unwrap();
    let cap = info["capture_sha256"].as_str().unwrap();
    // Older fixtures kept the binaries and the private observation together.
    // Exercise the current product shape without modifying the input fixture.
    let original = test_root.join("state/loadout-live-epochs/kusarigama-strong");
    let input = original.join(cap);
    let run = test_root.join(format!("installed-scorer-{}", std::process::id()));
    let data = run.join("data");
    let installed = data.join("loadout/kusarigama-strong");
    let state = run.join("state");
    let epoch = state.join("loadout-live-epochs/kusarigama-strong");
    let generation = epoch.join(cap);
    let bundle_path = generation.join("bundle");
    let private = bundle_path.join("context");
    fs::create_dir_all(&private).unwrap();
    fs::create_dir_all(&installed).unwrap();
    fs::copy(
        input.join("EPOCH_INPUT.json"),
        generation.join("EPOCH_INPUT.json"),
    )
    .unwrap();
    fs::copy(
        test_root.join("data/loadout/kusarigama-strong/bundle.json"),
        installed.join("bundle.json"),
    )
    .unwrap();
    for name in [
        "arena.bin",
        "frame.bin",
        "gpu_setup.txt",
        "hits.txt",
        "live_groups.txt",
        "main_cpu.dll",
        "mapped.bin",
        "programs.txt",
        "survival_groups.txt",
        "tables.bin",
    ] {
        fs::copy(input.join("bundle").join(name), installed.join(name)).unwrap();
    }
    for entry in fs::read_dir(input.join("bundle/context")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), private.join(entry.file_name())).unwrap();
    }
    assert!(!bundle_path.join("main_cpu.dll").exists());
    let receipt_path = generation.join("EPOCH_INPUT.json");
    let mut snapshot: CombatSnapshot =
        serde_json::from_slice(&fs::read(test_root.join("SNAPSHOT_FIXTURE.json")).unwrap())
            .unwrap();
    snapshot.inputs.compiled_source_observation = Some(
        CompiledSourceObservation::from_sources(&snapshot.core_source_raw, &snapshot.source_raw)
            .unwrap(),
    );
    let prep = crate::loadout_epoch::PreparedReplayEpoch::load(
        "kusarigama-strong",
        &bundle_path,
        &receipt_path,
    )
    .unwrap();
    fs::write(
        generation.join("LIVE_BINDING.json"),
        prep.binding_bytes(&snapshot).unwrap(),
    )
    .unwrap();
    let owned = owned_fixture(&prep.context);
    let params = json!({"save_id":"b".repeat(64),"source_sha256":nioh3_loadout::context::sha256(&owned),
        "bundle":"kusarigama-strong"});
    let mut contexts = Contexts::default();
    let bind = |contexts: &mut Contexts, source: &CombatSnapshot, origin| {
        contexts.read_with_snapshot_source(
            &state,
            &data,
            &owned,
            &params,
            origin,
            SnapshotUse::CurrentProcess,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(source.clone())))),
        )
    };
    let cold = bind(
        &mut contexts,
        &snapshot,
        ObservationOrigin::VerifiedProcessRead,
    )
    .unwrap();
    fs::copy(original.join("active.json"), epoch.join("active.json")).unwrap();
    let ready = bind(
        &mut contexts,
        &snapshot,
        ObservationOrigin::VerifiedProcessRead,
    )
    .unwrap();
    assert_eq!(ready["state"], "ready");
    assert_eq!(ready["source"], "live");
    let mut current = params.clone();
    current["context_id"] = ready["context_id"].clone();
    let (engine, authority) = contexts.search_binding(&current).unwrap();
    assert!(matches!(authority, SearchAuthority::VerifiedNativeEpoch));
    let candidate = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let details = engine.damage_details(
        &candidate,
        &Backpack::from_slots(&[]).unwrap(),
        &mut engine.work(),
    );
    assert!(
        matches!(details, DamageDetails::Available{total, physical, elemental, ..}
        if (total, physical, elemental) == (634.0, 615.0, 19.0))
    );

    let mut recovered = snapshot.clone();
    let [original_resource, maximum] = recovered.inputs.owner_resource_bits.unwrap();
    let current_resource = (f32::from_bits(original_resource) + 1.0).to_bits();
    recovered.inputs.owner_resource_bits = Some([current_resource, maximum]);
    let refreshed = bind(
        &mut contexts,
        &recovered,
        ObservationOrigin::VerifiedProcessRead,
    )
    .unwrap();
    assert_eq!(refreshed["source"], "live");
    assert_ne!(refreshed["context_id"], current["context_id"]);
    let mut refreshed_params = params.clone();
    refreshed_params["context_id"] = refreshed["context_id"].clone();
    assert!(contexts.engine(&current).is_err());
    let current_engine = contexts.engine(&refreshed_params).unwrap();
    let encoded = current_engine.encoded_context().unwrap();
    let observed_resource = crate::loadout_live::scene_word(
        encoded.file("image.bin").unwrap(),
        encoded.file("image_known.bin").unwrap(),
        snapshot.roots.owner + 0x15C,
    )
    .unwrap();
    assert_eq!(observed_resource, current_resource);
    let refreshed_damage = current_engine.damage_details(
        &candidate,
        &Backpack::from_slots(&[]).unwrap(),
        &mut current_engine.work(),
    );
    fs::write(
        test_root.join("VERIFIED_SNAPSHOT_FIXTURE.json"),
        serde_json::to_vec_pretty(&snapshot).unwrap(),
    )
    .unwrap();
    fs::write(
        test_root.join("RESOURCE_REFRESH_COMPONENTS.json"),
        serde_json::to_vec_pretty(&json!({
            "owner_resource_address": snapshot.roots.owner + 0x15C,
            "recorded_bits": original_resource, "updated_bits": current_resource,
            "translated": refreshed_damage, "game_access": false
        }))
        .unwrap(),
    )
    .unwrap();
    let mut changed_max = recovered.clone();
    changed_max.inputs.owner_resource_bits = Some([current_resource, maximum + 1]);
    assert!(bind(
        &mut contexts,
        &changed_max,
        ObservationOrigin::VerifiedProcessRead
    )
    .unwrap_err()
    .to_string()
    .contains("CURRENT_INPUT_CHANGED"));
    assert!(contexts.engine(&refreshed_params).is_err());
    let admitted = bind(
        &mut contexts,
        &snapshot,
        ObservationOrigin::VerifiedProcessRead,
    )
    .unwrap();
    assert_eq!(admitted["context_id"], current["context_id"]);

    let mut earlier = snapshot.clone();
    earlier.process.creation_filetime += 1;
    assert!(bind(
        &mut contexts,
        &earlier,
        ObservationOrigin::VerifiedProcessRead
    )
    .unwrap_err()
    .to_string()
    .contains("CURRENT_INPUT_CHANGED"));
    assert!(contexts.engine(&current).is_err());

    let mut other_outfit = snapshot.clone();
    other_outfit.inputs.inventory[0xF70 + 6 * 0xE8 + 6] ^= 1;
    let unchanged_scene = crate::loadout_epoch::PreparedReplayEpoch::load(
        "kusarigama-strong",
        &bundle_path,
        &receipt_path,
    )
    .unwrap();
    assert!(
        unchanged_scene.binding_bytes(&other_outfit).is_err(),
        "rebinding a changed outfit against a stale scene must fail before publication"
    );

    let replay = bind(
        &mut contexts,
        &snapshot,
        ObservationOrigin::RetainedCaptureReplay,
    );
    assert!(!replay.is_ok_and(|result| result["source"] == "live"));
    let result = json!({"passed":true,"production_receiver_reached":true,"current_snapshot_is_file_backed":true,
        "exact_scene_channels":[634,615,19],"wrong_birth_retired_old_binding":true,
        "changed_outfit_cannot_rebind_stale_scene":true,"retained_origin_cannot_acquire_live_authority":true,
        "current_resource_republished":true,"changed_maximum_rejected":true,"actual_game_read_verified":false,
        "before_generation_activation":cold,"private_generation_contains_scorer":false});
    fs::write(
        test_root.join("LIVE_EPOCH_RECEIVER_E2E.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}

fn product_fixture() -> (PathBuf, PathBuf, Vec<u8>, Value, Value) {
    let root = PathBuf::from(
        std::env::var_os("NIOH3_PRODUCT_EPOCH_PREPARED").expect("prepared product generation"),
    );
    let data = root.join("data");
    let reference = data.join("loadout/kusarigama-strong/context");
    let plain = owned_fixture(&LoadoutContext::load(&reference).unwrap());
    let params = json!({
        "save_id": "c".repeat(64),
        "source_sha256": nioh3_loadout::context::sha256(&plain),
        "bundle": "kusarigama-strong",
        "items": [],
    });
    let expected =
        serde_json::from_slice(&fs::read(root.join("IMPORT_EXPECTATIONS.json")).unwrap()).unwrap();
    (root, data, plain, params, expected)
}

fn product_fixture_score(
    contexts: &Contexts,
    data: &Path,
    plain: &[u8],
    mut params: Value,
    ready: &Value,
    expected: &Value,
    live_authority: bool,
) -> Value {
    assert_eq!(ready["state"], "ready");
    params["context_id"] = ready["context_id"].clone();
    let (engine, authority) = contexts.search_binding(&params).unwrap();
    assert_eq!(
        matches!(authority, SearchAuthority::VerifiedNativeEpoch),
        live_authority,
    );
    // This fixture compares one retained outfit with its independent native
    // receipt. Submit the same set constraint required by the product UI.
    let retained = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let empty = Backpack::from_slots(&[]).unwrap();
    let sets = engine
        .equipment_set_state(&retained, &empty, &mut engine.work())
        .unwrap();
    let &(key, _) = sets.pieces.iter().max_by_key(|entry| entry.1).unwrap();
    params["sets"] = json!([{"options":[{"key":key,"pieces":1}],"at_least":1}]);
    let score = crate::loadout::search(data, plain, &params, &engine).unwrap();
    assert_eq!(score["complete"], true);
    assert_eq!(score["answered"], 1);
    assert_eq!(score["best"][0]["damage"], expected["damage"]["total"]);
    let details = engine.damage_details(&retained, &empty, &mut engine.work());
    let decoded = serde_json::to_value(details).unwrap();
    assert_eq!(decoded["physical"], expected["damage"]["physical"]);
    assert_eq!(decoded["elemental"], expected["damage"]["wind"]);
    decoded
}

#[test]
#[ignore = "Uses the November 18 captured source and explicitly isolated reader controls"]
fn product_epoch_import_current_capture_file_controls() {
    use crate::loadout_live::SnapshotUse;
    use crate::loadout_observation::ObservationOrigin;
    use nioh3_runtime::combat_context::{CombatSnapshot, SnapshotOutcome};

    let (root, data, plain, params, expected) = product_fixture();
    let state = PathBuf::from(std::env::var_os("NIOH3_PRODUCT_EPOCH_STATE").unwrap());
    let snapshot_path =
        PathBuf::from(std::env::var_os("NIOH3_PRODUCT_EPOCH_AUDIT_SNAPSHOT").unwrap());
    let snapshot: CombatSnapshot =
        serde_json::from_slice(&fs::read(snapshot_path).unwrap()).unwrap();
    let mut contexts = Contexts::default();
    let captured = contexts
        .read_replayed_epoch(&data, &plain, &params, || Ok(root.join("EPOCH_INPUT.json")))
        .unwrap();
    assert_eq!(captured["source"], "captured");
    let score = product_fixture_score(
        &contexts,
        &data,
        &plain,
        params.clone(),
        &captured,
        &expected,
        false,
    );

    let bind = |contexts: &mut Contexts, origin, snapshot: CombatSnapshot| {
        contexts.read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            origin,
            SnapshotUse::CurrentProcess,
            || Ok(Some(SnapshotOutcome::Ready(Box::new(snapshot)))),
        )
    };
    let admitted = bind(
        &mut contexts,
        ObservationOrigin::VerifiedProcessRead,
        snapshot.clone(),
    )
    .unwrap();
    assert_eq!(admitted["source"], "live");
    let controlled_score = product_fixture_score(
        &contexts,
        &data,
        &plain,
        params.clone(),
        &admitted,
        &expected,
        true,
    );

    let mut changed = snapshot.clone();
    changed.inputs.style ^= 1;
    assert!(bind(
        &mut contexts,
        ObservationOrigin::VerifiedProcessRead,
        changed
    )
    .is_err());
    let mut stale = params.clone();
    stale["context_id"] = admitted["context_id"].clone();
    assert!(contexts.search_binding(&stale).is_err());

    let file_origin = bind(
        &mut contexts,
        ObservationOrigin::RetainedCaptureReplay,
        snapshot,
    );
    assert!(!file_origin.is_ok_and(|ready| ready["source"] == "live"));
    let receipt = json!({
        "passed": true,
        "captured_original_channels": score,
        "controlled_live_branch_channels": controlled_score,
        "retained_origin_not_authorized": true,
        "changed_style_retires_previous": true,
        "game_accessed": false,
        "current_snapshot_has_synthetic_status_head_and_style": true,
    });
    fs::write(
        root.join("OFFLINE_PRODUCT_IMPORT_E2E.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "Only for a newly staged live generation while its authorized game still runs"]
fn product_epoch_import_uses_current_game_process() {
    assert_eq!(
        std::env::var("NIOH3_PRODUCT_EPOCH_LIVE").as_deref(),
        Ok("1")
    );
    assert!(!crate::offline_e2e::enabled());
    let (root, data, plain, params, expected) = product_fixture();
    let state = PathBuf::from(std::env::var_os("NIOH3_PRODUCT_EPOCH_STATE").unwrap());
    let mut contexts = Contexts::default();
    let ready = contexts.read(&state, &data, &plain, &params).unwrap();
    assert_eq!(ready["source"], "live");
    let score = product_fixture_score(&contexts, &data, &plain, params, &ready, &expected, true);
    fs::write(
        root.join("LIVE_PRODUCT_IMPORT_E2E.json"),
        serde_json::to_vec_pretty(&json!({
            "passed": true,
            "current_game_read_and_bound": true,
            "native_channels": score,
            "actual_save_not_opened_by_test": true,
        }))
        .unwrap(),
    )
    .unwrap();
}
