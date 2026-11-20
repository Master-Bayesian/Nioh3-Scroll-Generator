//! Product discovery/context/search over installed templates and actual MAIN.
//! Uses in-memory save records and retained scenes, never a game or real save.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::loadout_live::SnapshotUse;
use crate::loadout_observation::ObservationOrigin;
use nioh3_runtime::combat_context::{ChangeReason, NotLoadedAt};
use nioh3_save::character::{
    equipment_fields, equipment_offset, EQUIPMENT_RECORD_BYTES, EQUIPMENT_SET_OFFSETS,
};
use std::fs;

fn wire_result(method: &str, result: Value, contract: &crate::Contract) {
    let jobs = crate::ProtectedJobs::new();
    let started = jobs.start(method, false, move |_| Ok(result)).unwrap();
    let id = started["job_id"].as_str().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let job = loop {
        let job = jobs.snapshot(id).unwrap();
        if job["state"] == "completed" {
            break job;
        }
        assert!(
            std::time::Instant::now() < until,
            "protected job did not complete"
        );
        std::thread::yield_now();
    };
    jobs.join();
    assert!(
        contract.response_valid(&json!({"protocol":1,"id":"product-e2e","ok":true,"result":job})),
        "{method} failed the actual wire contract: {job}"
    );
}

fn fixture_save(
    context: &LoadoutContext,
    data: &Path,
    bundle: &Value,
) -> (Vec<u8>, Vec<Value>, Vec<Value>) {
    let minimum =
        nioh3_save::layout::slot_offset(nioh3_save::layout::SCROLL_SLOT_COUNT - 1).unwrap() + 0xE8;
    let mut plain = vec![0u8; nioh3_save::character::EQUIPMENT_REGION_END.max(minimum) + 0x1000];
    let bank = context.manifest().character.active as usize * 0xF70;
    let mut candidates = Vec::new();
    let mut equipment = Vec::new();
    for slot in 0..17usize {
        let record = &context.inventory()[bank + slot * 0xE8..bank + (slot + 1) * 0xE8];
        if equipment_slot_is_empty(record) {
            continue;
        }
        if slot == 15 {
            let at = nioh3_save::layout::slot_offset(40).unwrap();
            plain[at..at + 0xE8].copy_from_slice(record);
            continue;
        }
        let index = 100 + slot;
        let at = equipment_offset(index).unwrap();
        plain[at..at + 0xE8].copy_from_slice(record);
        for offset in EQUIPMENT_SET_OFFSETS {
            plain[at + offset..at + offset + 4].fill(0xFF);
        }
        let fields = equipment_fields(&plain[at..at + EQUIPMENT_RECORD_BYTES]).unwrap();
        let class = crate::item_kinds::type_class(data, fields.item_id);
        let target = match class {
            Some(24..=38) => class.map(|class| (class - 24) / 3 + 4),
            Some(39) if bundle["school"] == "samurai" => Some(9),
            Some(40) if bundle["school"] == "ninja" => Some(9),
            Some(class) if bundle["weapon_class"].as_u64() == Some(class as u64) => Some(0),
            _ => None,
        };
        if let Some(target) = target {
            if fields.rarity <= 5 {
                candidates.push(json!({"slot_index":index,"slot":target}));
            }
        }
        equipment.push(json!({"slot_index":index,"item_id":fields.item_id,"appearance_id":fields.item_id,
            "quantity":1,"level":fields.level,"level_before_forge":fields.level,"plus":fields.plus,
            "familiarity":0,"inventory_key":0,"seed":fields.seed,"rarity":fields.rarity,"type_class":class,"effects":[]}));
    }
    (plain, candidates, equipment)
}

#[test]
fn fresh_profiles_choose_data_templates_and_do_not_confuse_buff_assets() {
    let root = std::env::temp_dir().join(format!(
        "nioh3-template-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let state = root.join("state");
    let data = root.join("data");
    let shipped = data.join("loadout/katana-deflect/context");
    let legacy = state.join("loadout-contexts/katana-deflect");
    fs::create_dir_all(&shipped).unwrap();
    fs::write(shipped.join("context.json"), b"{}").unwrap();
    assert_eq!(
        template_directory(&state, &data, "katana-deflect"),
        Some(shipped.clone())
    );
    assert!(
        !state.exists(),
        "scenario discovery created a private capture prerequisite"
    );
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("context.json"), b"{}").unwrap();
    assert_eq!(
        template_directory(&state, &data, "katana-deflect"),
        Some(shipped)
    );
    assert_eq!(
        template_directory(&state, &data, "katana-current"),
        None,
        "buff aliases borrowed another template"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "Requires explicitly installed product scene templates, exact native MAIN, versioned game data and an external output directory"]
fn installed_scenes_use_actual_protected_context_search_and_wire_contracts() {
    let data = PathBuf::from(
        std::env::var_os("NIOH3_PRODUCT_APP_DATA_ROOT").expect("installed data root"),
    );
    let output = PathBuf::from(
        std::env::var_os("NIOH3_PRODUCT_APP_OUTPUT").expect("external output directory"),
    );
    let original_path = PathBuf::from(
        std::env::var_os("NIOH3_PRODUCT_APP_ORIGINAL_MATRIX")
            .expect("independent original-code matrix"),
    );
    let original: Value = serde_json::from_slice(&fs::read(&original_path).unwrap()).unwrap();
    assert_eq!(
        original["success"], true,
        "independent original-code matrix must have passed"
    );
    fs::create_dir_all(&output).unwrap();
    let state = output.join("fresh-state");
    assert!(
        !state.exists(),
        "the bootstrap gate requires a fresh private profile"
    );
    let contract = crate::Contract::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts"),
    )
    .unwrap();
    let discovery = crate::loadout::list_with_query(&data, &json!({})).unwrap();
    wire_result("save.loadout_bundles", discovery.clone(), &contract);
    let bundles = discovery["bundles"].as_array().unwrap();
    assert!(
        bundles.len() >= 18,
        "registered scenarios disappeared from discovery"
    );
    let mut rows = Vec::new();
    for bundle in bundles {
        if bundle["scene"].is_null() {
            continue;
        }
        let id = bundle["id"].as_str().unwrap();
        let Some(template) = template_directory(&state, &data, id) else {
            rows.push(json!({"id":id,"passed":false,"stage":"template","error":"installed selection has no scenario template"}));
            continue;
        };
        let context = LoadoutContext::load(&template).unwrap();
        let (plain, items, equipment) = fixture_save(&context, &data, bundle);
        let source = nioh3_save::save::sha256_hex(&plain);
        let params = json!({"save_id":"a".repeat(64),"source_sha256":source,"bundle":id});
        let mut contexts = Contexts::default();
        let mut observed = false;
        let absent = contexts
            .read_with_snapshot_source(
                &state,
                &data,
                &plain,
                &params,
                ObservationOrigin::RetainedCaptureReplay,
                SnapshotUse::CurrentProcess,
                || {
                    observed = true;
                    Ok(None)
                },
            )
            .unwrap();
        assert!(observed, "{id} did not try to read the current role");
        assert_eq!(
            absent["reason"], "LOADOUT_CONTEXT_CHARACTER_NOT_LOADED",
            "{id} substituted a retained role"
        );
        for (snapshot, reason) in [
            (
                SnapshotOutcome::NotLoaded(NotLoadedAt::Player),
                "LOADOUT_CONTEXT_CHARACTER_NOT_LOADED",
            ),
            (
                SnapshotOutcome::Changed(ChangeReason::LogicalInputs),
                "LOADOUT_CONTEXT_CHANGED",
            ),
        ] {
            let rejected = contexts
                .read_with_snapshot_source(
                    &state,
                    &data,
                    &plain,
                    &params,
                    ObservationOrigin::RetainedCaptureReplay,
                    SnapshotUse::CurrentProcess,
                    || Ok(Some(snapshot)),
                )
                .unwrap();
            assert_eq!(rejected["reason"], reason, "{id}");
        }
        let ready = contexts.read_with_snapshot_source(
            &state,
            &data,
            &plain,
            &params,
            ObservationOrigin::RetainedCaptureReplay,
            SnapshotUse::RetainedDiagnostic,
            || Ok(None),
        );
        let ready = match ready {
            Ok(value) if value["state"] == "ready" => value,
            other => {
                rows.push(
                    json!({"id":id,"passed":false,"stage":"context","error":format!("{other:?}")}),
                );
                continue;
            }
        };
        assert_eq!(ready["source"], "captured");
        wire_result("save.loadout_context", ready.clone(), &contract);
        let request = json!({"save_id":params["save_id"],"source_sha256":source,"bundle":id,
            "context_id":ready["context_id"],"client_request_id":format!("product-{id}"),"items":items,
            "sets":[],"limits":{},"move_points":false,"enhanced":false});
        let engine = contexts.engine(&request).unwrap();
        let sets = crate::loadout::sets(&data, &plain, &request, &engine).unwrap();
        wire_result("save.loadout_sets", sets.clone(), &contract);
        let search = crate::loadout::search(&data, &plain, &request, &engine);
        let expected = original["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|scene| scene["id"] == id);
        let expected_damage = expected
            .filter(|scene| scene["success"] == true)
            .and_then(|scene| scene["engine_outcome"]["damage"].as_f64());
        let passed = search.as_ref().is_ok_and(|value| {
            value["complete"] == true
                && value["current"]["status"] == "answered"
                && value["searched"].as_u64().is_some_and(|count| count > 0)
                && value["no_path"] == 0
                && value["refused"] == 0
                && value["best"]
                    .as_array()
                    .is_some_and(|rows| !rows.is_empty())
                && expected_damage.is_some()
                && value["current"]["damage"].as_f64() == expected_damage
        });
        if let Ok(value) = &search {
            wire_result("save.loadout_search", value.clone(), &contract);
        }
        let output_row = json!({"id":id,"passed":passed,"diagnostic":bundle["diagnostic"],"bundle":bundle,
            "template_digest":context.digest(),"template_provenance":context.manifest().provenance,
            "template_scenario":context.manifest().scenario,"template_frame_sha256":context.manifest().frame_sha256,
            "independent_expected_damage":expected_damage,"original_matrix":original_path,
            "context":ready,"sets":sets,"request":request,"search":search.as_ref().ok(),"error":search.as_ref().err().map(ToString::to_string),
            "character":{"save_id":params["save_id"],"source_sha256":source,"currencies":{"amrita":null,"gold":null},"equipment_slots":2500,"equipment":equipment}});
        // A failed refresh must retire a formerly usable binding for every buff.
        contexts
            .read_with_snapshot_source(
                &state,
                &data,
                &plain,
                &params,
                ObservationOrigin::RetainedCaptureReplay,
                SnapshotUse::CurrentProcess,
                || Ok(None),
            )
            .unwrap();
        assert!(
            contexts.engine(&request).is_err(),
            "failed refresh retained old authority for {id}"
        );
        rows.push(output_row);
        println!(
            "product scene {id}: {}",
            if passed { "passed" } else { "failed" }
        );
        fs::write(
            output.join("PROTECTED_SCENES_PROGRESS.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
    assert!(
        !state.exists(),
        "retained test unexpectedly wrote a private role snapshot"
    );
    let passed = rows.len() >= 18 && rows.iter().all(|row| row["passed"] == true);
    fs::write(output.join("PROTECTED_SCENES.json"),serde_json::to_vec_pretty(&json!({"passed":passed,
        "scope":"Production protected context/Engine/search/Contract/Jobs with installed templates and in-memory save records, compared with an independent original-code expected matrix; retained diagnostics, no current game read or save write",
        "discovery":discovery,"rows":rows})).unwrap()).unwrap();
    assert!(
        passed,
        "scene product integration has failures; inspect PROTECTED_SCENES.json"
    );
}
