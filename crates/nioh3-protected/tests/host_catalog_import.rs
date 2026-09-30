#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

//! Focused E2E acceptance for the pure `catalog.import_names` protected route.
//!
//! Failure cases are listed before the fixture helpers so the acceptance stays
//! explicit: malformed base64, unsupported CT/trainer roles, oversized bytes,
//! malformed keys, byte-order inversion, and conflicting names must never
//! become a silently selected display label. A valid `items_little_endian.json`
//! source must name `0xF6E8` from stored key `E8F6`, while `F6E8` must remain the
//! distinct inverted id `0xE8F6`. The fixture application has no save, game, or
//! filesystem dependency; the host's catalog branch is the only code exercised.
//!
//! Set `NIOH3_LOCAL_CATALOG_E2E_OUT` to retain exact command and response JSONL
//! files for a repeatable handoff artifact.

use std::io::Cursor;
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use nioh3_protected::{serve, Contract, HostError, JobContext, Role, RoleApplication};
use serde_json::{json, Value};

struct CatalogFixture;

impl RoleApplication for CatalogFixture {
    fn role(&self) -> Role {
        Role::Save
    }

    fn context_payload(&self) -> Value {
        json!({
            "product_version": "0.8.3",
            "game_profile": "catalog-import-fixture",
            "resources_digest": "fixture",
            "algorithm_version": "fixture",
            "policy_version": "fixture",
            "context_digest": "fixture",
            "seed_accelerator_abi": 0,
            "seed_accelerator_build_id": "fixture",
        })
    }

    fn direct(&mut self, method: &str, _params: &Value) -> Result<Value, HostError> {
        Err(HostError::rejected(format!(
            "unexpected direct method in catalog fixture: {method}"
        )))
    }

    fn run(
        &mut self,
        operation: &str,
        _params: Value,
        _ctx: &JobContext,
    ) -> Result<Value, HostError> {
        Err(HostError::rejected(format!(
            "unexpected job method in catalog fixture: {operation}"
        )))
    }

    fn shutdown(&mut self) -> Result<Value, HostError> {
        Ok(json!({"safe_to_shutdown": true}))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn contract_dir() -> PathBuf {
    repo_root().join("packages").join("contracts")
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialises");
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&body);
    bytes
}

fn responses(bytes: &[u8]) -> Vec<Value> {
    let mut offset = 0usize;
    let mut output = Vec::new();
    while offset + 4 <= bytes.len() {
        let size = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let end = offset + 4 + size;
        output.push(serde_json::from_slice(&bytes[offset + 4..end]).expect("response JSON"));
        offset = end;
    }
    output
}

fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"protocol": 1, "id": id, "method": method, "params": params})
}

fn params(role: &str, content: &[u8]) -> Value {
    json!({
        "role": role,
        "content_base64": STANDARD.encode(content),
        "source_label": "items_little_endian.json",
        "declared_version": "user-declared-test-source",
        "locale": "zh-CN",
    })
}

fn drive(case: &str, params: Value) -> Vec<Value> {
    let contract = Contract::load(&contract_dir()).expect("protected contract loads");
    let import_request = request("import", "catalog.import_names", params.clone());
    let input = [
        frame(&request("handshake", "handshake", json!({}))),
        frame(&import_request),
    ]
    .concat();
    let mut source = Cursor::new(input);
    let mut sink = Vec::new();
    serve(Box::new(CatalogFixture), &contract, &mut source, &mut sink)
        .expect("catalog host serves");
    let output = responses(&sink);
    if let Some(directory) = std::env::var_os("NIOH3_LOCAL_CATALOG_E2E_OUT") {
        let directory = PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("artifact directory");
        std::fs::write(
            directory.join(format!("{case}-commands.json")),
            serde_json::to_vec_pretty(&json!({
                "handshake": request("handshake", "handshake", json!({})),
                "import": import_request,
            }))
            .expect("command JSON"),
        )
        .expect("commands retained");
        std::fs::write(
            directory.join(format!("{case}-responses.json")),
            serde_json::to_vec_pretty(&output).expect("response JSON"),
        )
        .expect("responses retained");
    }
    output
}

fn import_result(output: &[Value]) -> &Value {
    assert_eq!(output.len(), 2, "handshake + import: {output:?}");
    assert_eq!(output[0]["ok"], true, "handshake: {}", output[0]);
    &output[1]
}

#[test]
fn catalog_import_names_is_read_only_and_fail_closed() {
    let valid = br#"{"E8F6":{"name":"Birdflight Cross Spear","type":null}}"#;
    let valid_output = drive("valid_little_endian", params("save_active_items", valid));
    let valid_result = import_result(&valid_output);
    assert_eq!(valid_result["ok"], true, "valid import: {valid_result}");
    assert_eq!(valid_result["result"]["source"]["bytes"], valid.len());
    assert_eq!(valid_result["result"]["rows"][0]["raw_key"], "E8F6");
    assert_eq!(valid_result["result"]["rows"][0]["id"], 0xF6E8);
    assert_eq!(valid_result["result"]["rows"][0]["display_id"], 0xF6E8);
    assert_eq!(valid_result["result"]["rows"][0]["displayable"], true);
    assert_eq!(valid_result["result"]["counts"]["display_rows"], 1);
    assert!(
        valid_result["result"]["source"]["sha256"]
            .as_str()
            .unwrap()
            .len()
            == 64
    );

    let inverted = drive(
        "byte_order_inversion",
        params(
            "save_active_items",
            br#"{"F6E8":{"name":"Inverted byte order"}}"#,
        ),
    );
    let inverted_result = import_result(&inverted);
    assert_eq!(
        inverted_result["ok"], true,
        "inverted import: {inverted_result}"
    );
    assert_eq!(inverted_result["result"]["rows"][0]["display_id"], 0xE8F6);
    assert_ne!(inverted_result["result"]["rows"][0]["display_id"], 0xF6E8);

    let malformed = drive(
        "malformed_key_quarantine",
        params(
            "save_active_items",
            br#"{"E8F6":{"name":"Good"},"not-a-key":{"name":"Never display"}}"#,
        ),
    );
    let malformed_result = import_result(&malformed);
    assert_eq!(
        malformed_result["ok"], true,
        "malformed row is quarantined: {malformed_result}"
    );
    assert_eq!(malformed_result["result"]["counts"]["quarantined_rows"], 1);
    let quarantine = malformed_result["result"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["raw_key"] == "not-a-key")
        .expect("quarantined row retained");
    assert_eq!(quarantine["id"], Value::Null);
    assert_eq!(quarantine["display_id"], Value::Null);
    assert_eq!(quarantine["displayable"], false);

    let collision = drive(
        "duplicate_collision",
        params(
            "save_active_items",
            br#"{"E8F6":{"name":"Name A"},"e8f6":{"name":"Name B"}}"#,
        ),
    );
    let collision_result = import_result(&collision);
    assert_eq!(
        collision_result["ok"], true,
        "collision is explicit: {collision_result}"
    );
    assert_eq!(collision_result["result"]["counts"]["conflict_ids"], 1);
    assert_eq!(collision_result["result"]["counts"]["display_rows"], 0);
    assert_eq!(collision_result["result"]["conflicts"][0]["id"], 0xF6E8);
    assert_eq!(
        collision_result["result"]["conflicts"][0]["names"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let unsupported = drive(
        "unsupported_ct_role",
        params("ct_equipment", br#"00 00: sentinel"#),
    );
    let unsupported_result = import_result(&unsupported);
    assert_eq!(unsupported_result["ok"], false);
    assert!(unsupported_result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("CATALOG_ROLE_UNSUPPORTED"));

    let malformed_base64 = drive(
        "malformed_base64",
        json!({
            "role": "save_active_items",
            "content_base64": "not-base64",
            "source_label": "items_little_endian.json",
            "declared_version": "user-declared-test-source",
            "locale": "zh-CN",
        }),
    );
    let malformed_base64_result = import_result(&malformed_base64);
    assert_eq!(malformed_base64_result["ok"], false);
    assert!(malformed_base64_result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("CATALOG_INVALID_CONTENT"));

    // MAX_CATALOG_BYTES + 1 is still within the request schema's base64 bound,
    // so the handler (not schema validation) proves the decoded-byte limit.
    let oversized = drive(
        "oversized_rejected",
        params(
            "save_active_items",
            &vec![0u8; nioh3_data::equipment_catalog::MAX_CATALOG_BYTES + 1],
        ),
    );
    let oversized_result = import_result(&oversized);
    assert_eq!(oversized_result["ok"], false);
    assert!(oversized_result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("CATALOG_INPUT_TOO_LARGE"));
}
