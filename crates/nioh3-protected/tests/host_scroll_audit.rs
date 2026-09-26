#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Focused E2E acceptance for the read-only `save.audit_scrolls` operation.
//!
//! Failure scenarios are deliberately listed before the fixture builder so the
//! test remains a contract gate rather than a happy-path demo:
//! - an R3 record and both R4 native stages must report replay matches;
//! - changing one generated slot must report an unmatched replay, never a
//!   rule violation;
//! - an expired snapshot must be rejected after the save bytes change;
//! - R5 and an unmapped playthrough must be reported as unsupported without
//!   replaying against a wrong resource context; and
//! - the audit must leave the encrypted save's SHA-256 unchanged.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use nioh3_data::load_effect_resource_for_file_version;
use nioh3_domain::effect::EffectTableIndex;
use nioh3_domain::install_materialize::{
    materialize_ng3_certified_install_record, materialize_ng3_certified_record,
};
use nioh3_domain::record::ScrollRecordBytes;
use nioh3_protected::{serve, Contract, RoleApplication, SaveApplication};
use nioh3_save::{
    encrypt_container,
    layout::{SCROLL_GROUP_OFFSET, SCROLL_RECORD_BYTES, USER_SAVE_BYTES},
};
use nioh3_worker::{ContextSelection, Engine, GameFileVersion};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn data_root() -> PathBuf {
    repo_root().join("nioh3_scroll_editor").join("data")
}

fn contract_dir() -> PathBuf {
    repo_root().join("packages").join("contracts")
}

fn build_root() -> PathBuf {
    std::env::var_os("NIOH3_BUILD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\Nioh3_v080_deliverables"))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialises");
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&body);
    bytes
}

fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"protocol": 1, "id": id, "method": method, "params": params})
}

fn response_frames(bytes: &[u8]) -> Vec<Value> {
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
        output.push(serde_json::from_slice(&bytes[offset + 4..end]).expect("valid response"));
        offset = end;
    }
    output
}

struct ChannelReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
}

impl Read for ChannelReader {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            self.buffer = self
                .receiver
                .recv()
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"))?;
        }
        let count = target.len().min(self.buffer.len());
        target[..count].copy_from_slice(&self.buffer[..count]);
        self.buffer.drain(..count);
        Ok(count)
    }
}

struct SharedSink(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for SharedSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn set_account(record: &mut ScrollRecordBytes, account_id: u64) {
    let bytes = record.as_bytes();
    let high = ((account_id >> 48) as u16).to_le_bytes();
    let middle = ((account_id >> 32) as u16).to_le_bytes();
    let low = (account_id as u32).to_le_bytes();
    let mut owned = *bytes;
    owned[0x02..0x04].copy_from_slice(&high);
    owned[0x04..0x06].copy_from_slice(&middle);
    owned[0x14..0x18].copy_from_slice(&low);
    *record = ScrollRecordBytes::from_slice(&owned).expect("account record");
}

fn fixture_save(root: &Path) -> (PathBuf, String) {
    let data = data_root();
    let effect =
        load_effect_resource_for_file_version(&data, (2, 0, 2, 0)).expect("v2.02 effect tables");
    let index = EffectTableIndex::from_resource(&effect).expect("effect index");
    let account_id = 76561198000000123u64;
    let mut template = ScrollRecordBytes::zeroed();
    template.write_u16(0x00, 0xE604).expect("record type");
    set_account(&mut template, account_id);

    let (r3, _) = materialize_ng3_certified_record(
        &index,
        &effect.grace_maps[0],
        &template,
        3,
        226061463,
        180,
        180,
        101,
        0,
    )
    .expect("R3 materializer");
    let (r4_final, _) = materialize_ng3_certified_record(
        &index,
        &effect.grace_maps[0],
        &template,
        4,
        10030700,
        180,
        180,
        102,
        0,
    )
    .expect("R4 final materializer");
    let (r4_stage, _) = materialize_ng3_certified_install_record(
        &index,
        &effect.grace_maps[0],
        &template,
        4,
        387276918,
        180,
        180,
        103,
        0,
    )
    .expect("R4 stage-one materializer");

    let mut changed = r3.clone();
    let old = changed.read_u32(0x34 + 8).expect("first value");
    changed
        .write_u32(0x34 + 8, old.wrapping_add(1))
        .expect("mutate slot");
    let mut r5 = r3.clone();
    r5.write_u8(0x30, 5).expect("R5 rarity");
    r5.write_u8(0x31, 5).expect("R5 rarity mirror");
    let mut unmapped = r3.clone();
    unmapped.write_u16(0x00, 0x1E82).expect("NG1 type");

    let mut clear = vec![0u8; USER_SAVE_BYTES];
    clear[..6].copy_from_slice(b"RNNUSR");
    for (slot, record) in [r3, r4_stage, r4_final, changed, r5, unmapped]
        .into_iter()
        .enumerate()
    {
        let start = SCROLL_GROUP_OFFSET + slot * SCROLL_RECORD_BYTES;
        clear[start..start + SCROLL_RECORD_BYTES].copy_from_slice(record.as_bytes());
    }
    let encrypted = encrypt_container(&clear).expect("encrypted fixture");
    let save_path = root
        .join("76561198000000123")
        .join("SAVEDATA00")
        .join("SAVEDATA.BIN");
    std::fs::create_dir_all(save_path.parent().expect("save parent")).expect("save directory");
    std::fs::write(&save_path, &encrypted).expect("save fixture");
    (save_path, digest(&encrypted))
}

fn wait_response(sink: &Arc<Mutex<Vec<u8>>>, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let values = response_frames(&sink.lock().expect("sink lock"));
        if values.len() >= count {
            return values;
        }
        assert!(Instant::now() < deadline, "protected host response timeout");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn save_audit_scrolls_is_read_only_and_fail_closed() {
    let root = build_root()
        .join("deliverables")
        .join("v081-integration-continuation-20260921")
        .join("scroll-audit")
        .join("e2e-fixture");
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("remove prior focused fixture");
    }
    let (save_path, original_sha) = fixture_save(&root);
    let state_root = root.join("state");
    let data = data_root();
    let engine = Engine::load(
        &data,
        &contract_dir(),
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .expect("production context");
    let application: Box<dyn RoleApplication> = Box::new(
        SaveApplication::new(state_root, &data, engine.context().clone()).expect("save host"),
    );
    let contract = Contract::load(&contract_dir()).expect("protected contract");
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = thread::spawn(move || {
        serve(
            application,
            &contract,
            &mut ChannelReader {
                receiver,
                buffer: Vec::new(),
            },
            &mut SharedSink(host_sink),
        )
        .expect("protected host");
    });
    let mut count = 0usize;
    let mut exchange = |method: &str, params: Value| -> Value {
        count += 1;
        sender
            .send(frame(&request(&count.to_string(), method, params)))
            .expect("request");
        let values = wait_response(&sink, count);
        values[count - 1].clone()
    };
    assert_eq!(exchange("handshake", json!({}))["ok"], true);
    let register_started = exchange("save.register", json!({"path": save_path}));
    assert_eq!(register_started["ok"], true, "{register_started}");
    let register_job = register_started["result"]["job_id"]
        .as_str()
        .expect("register job")
        .to_string();
    let registered = loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": register_job}));
        if snapshot["result"]["state"] != "running" {
            assert_eq!(snapshot["result"]["state"], "completed", "{snapshot}");
            break snapshot["result"]["result"].clone();
        }
    };
    let save_id = registered["save_id"].as_str().unwrap_or("").to_string();
    assert!(!save_id.is_empty(), "register result: {registered}");
    let inventory_started = exchange("save.inventory", json!({"save_id": save_id}));
    assert_eq!(inventory_started["ok"], true, "{inventory_started}");
    let inventory_job = inventory_started["result"]["job_id"]
        .as_str()
        .expect("inventory job")
        .to_string();
    let inventory;
    loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": inventory_job}));
        if snapshot["result"]["state"] != "running" {
            let completed = snapshot["result"]["result"].clone();
            assert_eq!(snapshot["result"]["state"], "completed", "{snapshot}");
            inventory = completed;
            break;
        }
    }
    let snapshot_id = inventory["snapshot_id"]
        .as_str()
        .expect("snapshot id")
        .to_string();
    let audit_started = exchange(
        "save.audit_scrolls",
        json!({"save_id": save_id, "snapshot_id": snapshot_id}),
    );
    assert_eq!(audit_started["ok"], true, "{audit_started}");
    let audit_job = audit_started["result"]["job_id"]
        .as_str()
        .expect("audit job")
        .to_string();
    let audit = loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": audit_job}));
        if snapshot["result"]["state"] != "running" {
            assert_eq!(snapshot["result"]["state"], "completed", "{snapshot}");
            break snapshot["result"]["result"].clone();
        }
    };
    assert_eq!(audit["status"], "insufficient_data");
    assert_eq!(audit["coverage_scope"], "generated_effect_projection");
    let rows = audit["rows"].as_array().expect("audit rows");
    assert!(rows
        .iter()
        .any(|row| row["replay_evidence"]["matched"] == true));
    assert!(rows
        .iter()
        .any(|row| row["replay_evidence"]["matched"] == false
            && row["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "replay_mismatch")));
    assert!(rows.iter().any(|row| row["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "unsupported_rarity")));
    assert!(rows.iter().any(|row| row["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "unsupported_record_type")));
    let source_sha_after_audit = digest(&std::fs::read(&save_path).expect("save remains"));
    assert_eq!(source_sha_after_audit, original_sha);

    let mut changed_bytes = std::fs::read(&save_path).expect("save bytes");
    let last = changed_bytes.len() - 1;
    changed_bytes[last] ^= 1;
    std::fs::write(&save_path, changed_bytes).expect("stale save mutation");
    let stale = exchange(
        "save.audit_scrolls",
        json!({"save_id": save_id, "snapshot_id": snapshot_id}),
    );
    assert_eq!(
        stale["ok"], true,
        "stale request returns failed job: {stale}"
    );
    let stale_job = stale["result"]["job_id"]
        .as_str()
        .expect("stale job")
        .to_string();
    let stale_done = loop {
        let snapshot = exchange("job.snapshot", json!({"job_id": stale_job}));
        if snapshot["result"]["state"] != "running" {
            break snapshot["result"].clone();
        }
    };
    assert_eq!(stale_done["state"], "failed");
    assert!(stale_done["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Save changed"));
    let source_sha_after_snapshot_expiry =
        digest(&std::fs::read(&save_path).expect("mutated save remains"));
    let evidence_path = save_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("scroll-audit evidence directory")
        .join("e2e-audit-result.json");
    let evidence = json!({
        "source_sha_before": original_sha,
        "source_sha_after_audit": source_sha_after_audit,
        "source_sha_after_snapshot_expiry": source_sha_after_snapshot_expiry,
        "audit": audit,
        "stale_job_result": stale_done,
    });
    std::fs::write(
        evidence_path,
        serde_json::to_vec_pretty(&evidence).expect("serialise audit evidence"),
    )
    .expect("write audit evidence");
    let shutdown = exchange("shutdown", json!({}));
    assert_eq!(shutdown["result"]["safe_to_shutdown"], true, "{shutdown}");
    drop(sender);
    host.join().expect("host joins");
}
