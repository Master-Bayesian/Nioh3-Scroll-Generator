#![allow(clippy::unwrap_used, clippy::expect_used)]
use nioh3_protected::compatibility::{validate_feature, ExecutableIdentity};
use nioh3_protected::{Contract, RoleApplication, RuntimeApplication};
use nioh3_runtime::player_resources::{
    snapshot_at,
    test_fixture::{Memory, NOW},
};
use serde_json::json;
fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
#[test]
fn resources_have_pointer_free_decimal_contracts_and_exact_build_gates() {
    let contract = Contract::load(&root().join("packages/contracts")).unwrap();
    let request = json!({"protocol":1,"id":"resources","method":"runtime.player_edit","params":{
        "operation_id":"11111111-1111-4111-8111-111111111111","process_id":42,"process_birth":"77",
        "snapshot_id":"a".repeat(64),"field":"ingot","clan_id":0,"value":"9999999999999","confirmed":true}});
    assert!(contract.request_valid(&request));
    for (key, value) in [
        ("value", json!(9999999999999u64)),
        ("value", json!("-1")),
        ("value", json!("01")),
        ("field", json!("amrita")),
        ("address", json!(123)),
        ("confirmed", json!(false)),
        ("clan_id", json!(51)),
    ] {
        let mut bad = request.clone();
        bad["params"][key] = value;
        assert!(!contract.request_valid(&bad));
    }
    let result = snapshot_at(&Memory::fixture(), NOW).unwrap().json();
    assert!(contract.response_valid(&json!({"protocol":1,"id":"read","ok":true,"result":result})));
    let mut identity = ExecutableIdentity {
        pid: 42,
        creation_filetime: 77,
        path: "D:/synthetic/Nioh3.exe".into(),
        version: "2.0.2.0".into(),
        sha256: nioh3_runtime::INVENTORY_EXECUTABLE_SHA256.into(),
    };
    assert!(validate_feature(&identity, "live_player_resources").is_ok());
    identity.sha256 = "0".repeat(64);
    assert!(validate_feature(&identity, "live_player_resources").is_err());
}
#[test]
fn resource_receipt_discovery_requires_no_running_game() {
    let state = std::env::temp_dir().join(format!("nioh3-resource-route-{}", std::process::id()));
    std::fs::create_dir_all(&state).unwrap();
    let mut app = RuntimeApplication::deferred(
        state.clone(),
        &root().join("not-present"),
        &root().join("packages/contracts"),
        None,
        None,
    )
    .unwrap();
    let result = app.direct("runtime.player_status", &json!({})).unwrap();
    assert!(result["player_edit"]["operation_id"].is_null());
    let _ = std::fs::remove_dir_all(state);
}
