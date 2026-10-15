#![allow(clippy::unwrap_used, clippy::expect_used)]
//! No game discovery: wire contracts, exact-build admission and receipt routing.
use nioh3_protected::compatibility::{validate_feature, ExecutableIdentity};
use nioh3_protected::{Contract, RoleApplication, RuntimeApplication};
use nioh3_runtime::prestige::{snapshot, test_fixture::Memory};
use serde_json::json;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
#[test]
fn prestige_contract_rejects_unknown_categories_addresses_and_unconfirmed_writes() {
    let contract = Contract::load(&root().join("packages/contracts")).unwrap();
    let request = json!({"protocol":1,"id":"points","method":"runtime.prestige_edit","params":{
        "operation_id":"11111111-1111-4111-8111-111111111111","process_id":42,"process_birth":"77",
        "snapshot_id":"a".repeat(64),"category_id":50152,"available":100,"confirmed":true}});
    assert!(contract.request_valid(&request));
    for (key, value) in [
        ("category_id", json!(1)),
        ("available", json!(-1)),
        ("available", json!(2001)),
        ("confirmed", json!(false)),
        ("address", json!(123)),
    ] {
        let mut invalid = request.clone();
        invalid["params"][key] = value;
        assert!(!contract.request_valid(&invalid));
    }
    assert!(
        contract.response_valid(&json!({"protocol":1,"id":"snapshot","ok":true,
        "result":snapshot(&Memory::fixture()).unwrap().json()}))
    );
}
#[test]
fn only_the_exact_v202_reference_gets_prestige_capability() {
    let mut identity = ExecutableIdentity {
        pid: 42,
        creation_filetime: 77,
        path: "D:/synthetic/Nioh3.exe".into(),
        version: "2.0.2.0".into(),
        sha256: nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.into(),
    };
    assert!(validate_feature(&identity, "live_prestige").is_ok());
    identity.version = "2.0.1.0".into();
    assert!(validate_feature(&identity, "live_prestige").is_err());
    identity.version = "2.0.2.0".into();
    identity.sha256 = "0".repeat(64);
    assert!(validate_feature(&identity, "live_prestige").is_err());
}
#[test]
fn production_receipt_route_needs_no_game_or_generation_resources() {
    let state = std::env::temp_dir().join(format!("nioh3-prestige-route-{}", std::process::id()));
    std::fs::create_dir_all(&state).unwrap();
    let repo = root();
    let contracts = repo.join("packages/contracts");
    let mut runtime = RuntimeApplication::deferred(
        state.clone(),
        &repo.join("not-present"),
        &contracts,
        None,
        None,
    )
    .unwrap();
    let result = runtime
        .direct("runtime.prestige_status", &json!({}))
        .unwrap();
    assert_eq!(
        result["prestige_edit"]["operation_id"],
        serde_json::Value::Null
    );
    let contract = Contract::load(&contracts).unwrap();
    assert!(
        contract.response_valid(&json!({"protocol":1,"id":"receipt","ok":true,"result":result}))
    );
    drop(runtime);
    if state.parent() == Some(std::env::temp_dir().as_path()) {
        std::fs::remove_dir_all(state).unwrap();
    }
}
