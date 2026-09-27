#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! End-to-end acceptance for `runtime.inventory_snapshot` over the real
//! protected wire contract.
//!
//! The real `serve` loop runs with the real `protected-*.schema.json` loaded by
//! `Contract::load`, so the request validator, the inline dispatch and the
//! response validator are all genuinely applied: a response that no longer
//! matches the published contract is reported as `INVALID_RESULT` instead of
//! passing. Only the memory source is declared: a fixture region map stands in
//! for the game process, so the chain walk, the paging, the decode and the
//! consistency rules all run for real with no game and no Cheat Engine.
//!
//! Set `NIOH3_INVENTORY_E2E_OUT` to a directory to retain the exact frames, one
//! `<case>.jsonl` file per case.

use std::io::Cursor;
use std::path::PathBuf;

use serde_json::{json, Value};

use nioh3_protected::{serve, Contract, HostError, JobContext, Role, RoleApplication};
use nioh3_runtime::inventory::{snapshot, FixtureMemory, InventoryRequest};

/// The four retained-runtime sites, written here independently of the module so
/// an accidental edit to either side is caught.
const SITE_SIGNATURES: [(&str, u64, &str); 4] = [
    ("global_ref_0x23E61F0", 0x23E61F0, "4c 8b 0d 39 b3 36 02"),
    ("global_ref_0x3EEDA3", 0x3EEDA3, "48 8b 0d 86 27 36 04"),
    (
        "container_leaf_0x553038",
        0x553038,
        "4c 8b 01 49 83 c0 10 8b c2 49 3b 80 c0 27 09 00",
    ),
    (
        "standalone_getter_0xF464C",
        0xF464C,
        "8b c2 48 3b 81 c0 27 09 00 73 0b 48 69 c0 f0 00 00 00",
    ),
];

const GLOBAL_SLOT_RVA: u64 = 0x4751530;
const COUNT_DISPLACEMENT: u64 = 0x927C0;
const RECORD_SIZE: usize = 0xF0;

struct FixtureRuntime {
    memory: FixtureMemory,
}

impl FixtureRuntime {
    fn new(fixture: &Value) -> Self {
        Self {
            memory: FixtureMemory::from_json(fixture).expect("fixture parses"),
        }
    }
}

impl RoleApplication for FixtureRuntime {
    fn role(&self) -> Role {
        Role::Runtime
    }

    fn context_payload(&self) -> Value {
        json!({
            "product_version": "0.8.2",
            "game_profile": "pc-v2.00.02-v2.01",
            "resources_digest": "r",
            "algorithm_version": "a",
            "policy_version": "p",
            "context_digest": "c",
            "seed_accelerator_abi": 2,
            "seed_accelerator_build_id": "b",
        })
    }

    fn direct(&mut self, method: &str, params: &Value) -> Result<Value, HostError> {
        match method {
            // The real inline route the host uses for the read-only methods.
            "runtime.status" => Ok(json!({
                "override_state": "stopped",
                "hit_count": 0,
                "pending_remote_calls": 0,
                "safe_to_shutdown": true,
                "error": Value::Null,
            })),
            "runtime.inventory_snapshot" => {
                let request =
                    InventoryRequest::from_json(params).map_err(HostError::from_runtime)?;
                snapshot(&self.memory, &request).map_err(HostError::from_runtime)
            }
            other => Err(HostError::rejected(format!(
                "INVALID_REQUEST: {other} is not an inline protected method"
            ))),
        }
    }

    fn run(
        &mut self,
        operation: &str,
        _params: Value,
        _ctx: &JobContext,
    ) -> Result<Value, HostError> {
        Err(HostError::rejected(format!(
            "INVALID_REQUEST: runtime.{operation} is not a job method here"
        )))
    }

    fn shutdown(&mut self) -> Result<Value, HostError> {
        Ok(json!({"safe_to_shutdown": true}))
    }
}

fn contract_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("packages")
        .join("contracts")
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialises");
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&body);
    bytes
}

fn responses(bytes: &[u8]) -> Vec<Value> {
    let mut offset = 0usize;
    let mut out = Vec::new();
    while offset + 4 <= bytes.len() {
        let size = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let body = &bytes[offset + 4..offset + 4 + size];
        out.push(serde_json::from_slice(body).expect("valid json"));
        offset += 4 + size;
    }
    out
}

fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"protocol": 1, "id": id, "method": method, "params": params})
}

/// Drive the real host over one framed request batch with one fixture.
fn drive(case: &str, fixture: &Value, frames: Vec<Value>) -> Vec<Value> {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let application: Box<dyn RoleApplication> = Box::new(FixtureRuntime::new(fixture));
    let mut input = Vec::new();
    for value in &frames {
        input.extend_from_slice(&frame(value));
    }
    let mut source = Cursor::new(input);
    let mut sink = Vec::new();
    serve(application, &contract, &mut source, &mut sink).expect("serve succeeds");
    let out = responses(&sink);
    if let Some(directory) = std::env::var_os("NIOH3_INVENTORY_E2E_OUT") {
        let mut retained = String::new();
        for value in &out {
            retained.push_str(&value.to_string());
            retained.push('\n');
        }
        std::fs::create_dir_all(&directory).expect("retained frame directory");
        std::fs::write(
            PathBuf::from(directory).join(format!("{case}.jsonl")),
            retained,
        )
        .expect("retained frames written");
    }
    out
}

fn hex_bytes(text: &str) -> Vec<u8> {
    let digits: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let high = pair[0].to_digit(16).expect("hex");
            let low = pair[1].to_digit(16).expect("hex");
            (high * 16 + low) as u8
        })
        .collect()
}

fn hex_text(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<String>>()
        .join("")
}

fn synthetic_record(
    item_id: u16,
    quantity: u16,
    level: u16,
    plus: u16,
    rarity: u8,
    effects: &[(u16, u32)],
) -> Vec<u8> {
    let mut record = vec![0u8; RECORD_SIZE];
    record[0x00..0x02].copy_from_slice(&item_id.to_le_bytes());
    record[0x04..0x06].copy_from_slice(&quantity.to_le_bytes());
    record[0x06..0x08].copy_from_slice(&level.to_le_bytes());
    record[0x0A..0x0C].copy_from_slice(&plus.to_le_bytes());
    record[0x30] = rarity;
    for (index, (id, raw)) in effects.iter().enumerate() {
        let base = 0x38 + index * 0x18;
        record[base..base + 2].copy_from_slice(&id.to_le_bytes());
        record[base + 4..base + 8].copy_from_slice(&raw.to_le_bytes());
    }
    record
}

/// Both sentinels, so the decoder cannot silently filter either one.
fn sentinel_effects() -> Vec<(u16, u32)> {
    vec![
        (0x1111, 80),
        (0x0000, 0),
        (0xFFFF, 0),
        (0x2222, 33),
        (0xFFFF, 0),
        (0xFFFF, 0),
        (0xFFFF, 0),
    ]
}

/// One fixture: four sites, a global slot, a container and its count.
fn fixture(records: &[Vec<u8>], count: u64, break_site: bool) -> Value {
    let module_base: u64 = 0x7FF7_0000_0000;
    let slot_value: u64 = 0x2000_0000_1000;
    let container: u64 = 0x02A6_770D_1290;
    let mut regions: Vec<Value> = Vec::new();
    for (index, (_, rva, hex)) in SITE_SIGNATURES.iter().enumerate() {
        let mut bytes = hex_bytes(hex);
        if break_site && index == 3 {
            bytes[0] ^= 0xFF;
        }
        regions.push(json!({"address": module_base + rva, "bytes": hex_text(&bytes)}));
    }
    regions.push(json!({
        "address": module_base + GLOBAL_SLOT_RVA,
        "bytes": hex_text(&slot_value.to_le_bytes()),
    }));
    regions.push(json!({
        "address": slot_value,
        "bytes": hex_text(&(container - 0x10).to_le_bytes()),
    }));
    if !records.is_empty() {
        let mut bytes = Vec::new();
        for record in records {
            bytes.extend_from_slice(record);
        }
        regions.push(json!({"address": container, "bytes": hex_text(&bytes)}));
    }
    regions.push(json!({
        "address": container + COUNT_DISPLACEMENT,
        "bytes": hex_text(&count.to_le_bytes()),
    }));
    json!({
        "module_base": module_base,
        "pid": 22000,
        "creation_filetime": "134344988244393971",
        "regions": regions,
    })
}

fn three_records() -> Vec<Vec<u8>> {
    vec![
        synthetic_record(0x1234, 1, 300, 7, 3, &sentinel_effects()),
        // 0x0200 = 512, above the u8 range: the raw u16 read must survive.
        synthetic_record(0x5678, 2, 512, 0, 5, &sentinel_effects()),
        synthetic_record(0x9ABC, 3, 180, 20, 4, &sentinel_effects()),
    ]
}

#[test]
fn a_page_is_observed_and_contract_valid() {
    let fixture = fixture(&three_records(), 3, false);
    let out = drive(
        "page_observed",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request(
                "2",
                "runtime.inventory_snapshot",
                json!({"start": 0, "limit": 4}),
            ),
        ],
    );
    assert_eq!(out.len(), 2, "{out:?}");
    assert_eq!(out[0]["ok"], true, "handshake refused: {}", out[0]);
    let page = &out[1];
    assert_eq!(page["ok"], true, "page refused: {page}");
    let result = &page["result"];
    assert_eq!(result["status"], "observed");
    assert_eq!(result["game_version"], "2.0.2.0");
    assert_eq!(result["read_only"], true);
    assert_eq!(result["consistency"], "reread_equal");
    assert_eq!(result["process"]["pid"], 22000);
    assert_eq!(result["process"]["creation_filetime"], "134344988244393971");
    assert_eq!(result["start"], 0);
    assert_eq!(result["limit"], 4);
    assert_eq!(result["observed_slot_count"], 3);
    assert_eq!(result["next_start"], Value::Null);

    let rows = result["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["slot"], 0);
    assert_eq!(rows[0]["item_id"], 0x1234);
    assert_eq!(rows[0]["quantity_raw"], 1);
    assert_eq!(rows[0]["level_raw"], 300, "the u16 level read is raw");
    assert_eq!(rows[0]["plus_raw"], 7);
    assert_eq!(rows[0]["rarity_raw"], 3);
    assert_eq!(rows[0]["record_sha256"].as_str().expect("digest").len(), 64);
    let effects = rows[0]["effects"].as_array().expect("effects");
    assert_eq!(effects.len(), 7, "all seven entries are preserved");
    assert_eq!(effects[0]["id"], 0x1111);
    assert_eq!(effects[0]["raw_value"], 80);
    assert_eq!(effects[1]["id"], 0, "the zero sentinel is preserved");
    assert_eq!(effects[2]["id"], 0xFFFF, "the FFFF sentinel is preserved");
    assert_eq!(effects[3]["id"], 0x2222);
    assert_eq!(rows[1]["level_raw"], 512, "a level above 255 stays raw");
    assert_eq!(rows[2]["plus_raw"], 20);
}

#[test]
fn a_start_past_the_observed_count_is_an_empty_page() {
    let fixture = fixture(&three_records(), 3, false);
    let out = drive(
        "start_past_count",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request(
                "2",
                "runtime.inventory_snapshot",
                json!({"start": 5, "limit": 4}),
            ),
        ],
    );
    let page = &out[1];
    assert_eq!(page["ok"], true, "{page}");
    assert_eq!(page["result"]["rows"].as_array().expect("rows").len(), 0);
    assert_eq!(page["result"]["next_start"], Value::Null);
    assert_eq!(page["result"]["observed_slot_count"], 3);
}

#[test]
fn a_zero_count_is_an_observed_empty_page() {
    let fixture = fixture(&[], 0, false);
    let out = drive(
        "zero_count",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.inventory_snapshot", json!({})),
        ],
    );
    let page = &out[1];
    assert_eq!(page["ok"], true, "{page}");
    assert_eq!(page["result"]["status"], "observed");
    assert_eq!(page["result"]["observed_slot_count"], 0);
    assert_eq!(
        page["result"]["limit"], 64,
        "the server applies the default"
    );
}

#[test]
fn a_page_beyond_the_first_is_continued_by_next_start() {
    let fixture = fixture(&three_records(), 3, false);
    let out = drive(
        "next_start",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request(
                "2",
                "runtime.inventory_snapshot",
                json!({"start": 1, "limit": 1}),
            ),
        ],
    );
    let result = &out[1]["result"];
    assert_eq!(out[1]["ok"], true, "{}", out[1]);
    assert_eq!(result["start"], 1);
    assert_eq!(result["next_start"], 2);
    assert_eq!(result["rows"].as_array().expect("rows").len(), 1);
    assert_eq!(result["rows"][0]["slot"], 1);
}

#[test]
fn a_client_cannot_override_the_fixed_inputs() {
    let fixture = fixture(&three_records(), 3, false);
    let out = drive(
        "override_refused",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request(
                "2",
                "runtime.inventory_snapshot",
                json!({"pid": 4, "sha256": "00", "start": 0}),
            ),
            request("3", "runtime.inventory_snapshot", json!({"limit": 65})),
            request("4", "runtime.inventory_snapshot", json!({"start": 2500})),
        ],
    );
    for (index, response) in out.iter().enumerate().skip(1) {
        assert_eq!(
            response["ok"], false,
            "case {index} was accepted: {response}"
        );
        let message = response["error"]["message"].as_str().expect("message");
        assert!(
            message.starts_with("INVALID_REQUEST") || message.contains("runtime contract"),
            "case {index} did not name the request: {message}"
        );
    }
}

#[test]
fn a_count_above_the_safety_ceiling_is_refused() {
    let fixture = fixture(&three_records(), 2501, false);
    let out = drive(
        "count_above_ceiling",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.inventory_snapshot", json!({})),
        ],
    );
    let response = &out[1];
    assert_eq!(response["ok"], false, "{response}");
    let message = response["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("2500") && message.contains("safety ceiling"),
        "{message}"
    );
    assert!(message.contains("not capacity"), "{message}");
}

#[test]
fn a_site_mismatch_is_refused_before_any_chain_read() {
    let fixture = fixture(&three_records(), 3, true);
    let out = drive(
        "site_mismatch",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.inventory_snapshot", json!({})),
        ],
    );
    let response = &out[1];
    assert_eq!(response["ok"], false, "{response}");
    let message = response["error"]["message"].as_str().expect("message");
    assert!(message.contains("standalone_getter_0xF464C"), "{message}");
}

#[test]
fn a_region_that_does_not_cover_the_read_is_refused() {
    // The counts live 0x927C0 bytes away from the records, so a fixture that
    // declares only the record region cannot answer the count read: the reader
    // must refuse instead of widening the range.
    let mut fixture = fixture(&three_records(), 3, false);
    let regions = fixture["regions"].as_array_mut().expect("regions");
    regions
        .retain(|region| region["address"].as_u64() != Some(0x02A6_770D_1290 + COUNT_DISPLACEMENT));
    let out = drive(
        "region_not_covered",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.inventory_snapshot", json!({})),
        ],
    );
    let response = &out[1];
    assert_eq!(response["ok"], false, "{response}");
    let message = response["error"]["message"].as_str().expect("message");
    assert!(message.contains("committed, readable region"), "{message}");
}

#[test]
fn a_snapshot_is_not_a_job_and_does_not_occupy_the_owner() {
    let fixture = fixture(&three_records(), 3, false);
    let out = drive(
        "inline_not_a_job",
        &fixture,
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.inventory_snapshot", json!({})),
            request("3", "runtime.status", json!({})),
        ],
    );
    // The snapshot answers inline, so the value is the page itself and not a
    // `ProtectedJob` envelope.
    assert_eq!(out[1]["result"]["status"], "observed");
    assert!(out[1]["result"]["job_id"].is_null(), "{}", out[1]);
    assert_eq!(out[2]["result"]["override_state"], "stopped");
}
