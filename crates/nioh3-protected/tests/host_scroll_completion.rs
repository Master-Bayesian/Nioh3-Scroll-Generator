#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Read-only completion prediction over retained native records.
//!
//! See docs/research/V083_MISSING_FEATURE_ACCEPTANCE.md for the prewritten
//! failure cases. This runs the real framed contract/host, never a game call.

use std::io::Read;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use nioh3_protected::{serve, Contract, RoleApplication, RuntimeApplication};

use nioh3_worker::{ContextSelection, Engine, GameFileVersion};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn build_root() -> PathBuf {
    std::env::var_os("NIOH3_BUILD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\Nioh3_v080_deliverables"))
}

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("serialises");
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&body);
    bytes
}

fn response_frames(bytes: &[u8]) -> Vec<Value> {
    let mut offset = 0usize;
    let mut output = Vec::new();
    while offset + 4 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
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

fn wait_response(sink: &Arc<Mutex<Vec<u8>>>, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let values = response_frames(&sink.lock().expect("sink lock"));
        if values.len() >= count {
            return values;
        }
        assert!(Instant::now() < deadline, "protected host response timeout");
        thread::sleep(Duration::from_millis(5));
    }
}

fn bytes(hex: &str) -> Vec<u8> {
    hex.trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn hex(raw: &[u8]) -> String {
    raw.iter().map(|v| format!("{v:02x}")).collect()
}

fn projection(record: &str) -> Value {
    let b = bytes(record);
    json!({"counter": u16::from_le_bytes([b[12],b[13]]), "pity":b[50], "attempts":b[51],
        "effects": (0..7).map(|slot| {
            let at=0x34+slot*0x18;
            json!({"id":u32::from_le_bytes(b[at+4..at+8].try_into().unwrap()),
                "value":i32::from_le_bytes(b[at+8..at+12].try_into().unwrap()),"roll":b[at+12]})
        }).collect::<Vec<_>>()})
}

#[test]
fn predicts_native_replacements_from_deferred_runtime_without_process_or_writes() {
    let root = build_root()
        .join("tmp")
        .join(format!("nioh3-completion-{}", std::process::id()));
    let state = root.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let data = repo_root().join("nioh3_scroll_editor/data");
    let contracts = repo_root().join("packages/contracts");
    let engine = Engine::load(
        &data,
        &contracts,
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .unwrap();
    let digest = engine.context().digest().to_owned();
    let application: Box<dyn RoleApplication> = Box::new(
        RuntimeApplication::deferred(state.clone(), &data, &contracts, None, None).unwrap(),
    );
    let contract = Contract::load(&contracts).unwrap();
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = thread::spawn(move || {
        let _ = serve(
            application,
            &contract,
            &mut ChannelReader {
                receiver,
                buffer: Vec::new(),
            },
            &mut SharedSink(host_sink),
        );
    });
    let mut count = 0;
    let mut exchanges = Vec::new();
    let mut exchange = |method: &str, params: Value| -> Value {
        count += 1;
        let request = json!({"protocol":1,"id":count.to_string(),"method":method,"params":params});
        sender.send(frame(&request)).unwrap();
        let response = wait_response(&sink, count)[count - 1].clone();
        exchanges.push(json!({"request":request,"response":response}));
        response
    };
    assert_eq!(exchange("handshake", json!({}))["ok"], true);
    let pre = include_str!("fixtures/scroll-completion/seed121723131_pre_c1.hex").trim();
    let post = include_str!("fixtures/scroll-completion/seed121723131_post_c2.hex").trim();
    let final_record = include_str!("fixtures/scroll-completion/seed121723131_final.hex").trim();
    let params = |record: &str| json!({"record_hex":record,"context_digest":digest});
    let response = exchange("runtime.scroll_completion_predict", params(pre));
    assert_eq!(response["ok"], true, "{response}");
    let result = &response["result"]["completion_prediction"];
    assert_eq!(result["ordinary_completion_only"], true);
    assert_eq!(result["painting"]["draw"], 5659);
    assert_eq!(result["painting"]["threshold"], 4500);
    assert_eq!(result["painting"]["success"], false);
    let candidates = result["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3, "primary and grace must not be offered");
    for (entry, expected) in candidates
        .iter()
        .zip([(0xAE5A, 150), (0x6CE3, 13), (0x2EFC, 75)])
    {
        assert_eq!(entry["effect_id"], expected.0);
        assert_eq!(entry["value"], expected.1);
    }
    assert_eq!(candidates[0]["roll"], 85);
    let branches = result["branches"].as_array().unwrap();
    let accepted = branches.iter().find(|v| v["choice"] == 1).unwrap();
    assert_eq!(
        projection(accepted["record_hex"].as_str().unwrap()),
        projection(post)
    );
    let next = exchange(
        "runtime.scroll_completion_predict",
        params(accepted["record_hex"].as_str().unwrap()),
    );
    assert_eq!(next["ok"], true, "{next}");
    let next = &next["result"]["completion_prediction"];
    assert_eq!(next["painting"]["draw"], 3388);
    assert_eq!(next["painting"]["threshold"], 5999);
    assert_eq!(next["painting"]["success"], true);
    let declined = next["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["choice"].is_null())
        .unwrap();
    assert_eq!(declined["painting_effect"]["effect_id"], 0xB393);
    assert_eq!(declined["painting_effect"]["value"], 66);
    assert_eq!(declined["painting_effect"]["roll"], 90);
    assert_eq!(
        projection(declined["record_hex"].as_str().unwrap()),
        projection(final_record)
    );
    let fresh = include_str!("fixtures/scroll-completion/seed47878870_after_reveal.hex").trim();
    let response = exchange("runtime.scroll_completion_predict", params(fresh));
    assert_eq!(response["ok"], true, "{response}");
    let result = &response["result"]["completion_prediction"];
    assert_eq!(result["reveal_status"], "unknown");
    assert_eq!(result["painting"]["draw"], 416);
    assert_eq!(result["painting"]["threshold"], 2999);
    let decline = &result["branches"][0];
    assert_eq!(decline["painting_effect"]["effect_id"], 0xDB20);
    assert_eq!(decline["painting_effect"]["value"], 57);
    assert_eq!(decline["painting_effect"]["roll"], 93);
    for (pity, threshold) in [2999, 4500, 5999, 7500, 9000, 10000, 10000]
        .iter()
        .enumerate()
    {
        let mut raw = bytes(pre);
        raw[50] = pity as u8;
        let response = exchange("runtime.scroll_completion_predict", params(&hex(&raw)));
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(
            response["result"]["completion_prediction"]["painting"]["threshold"],
            *threshold
        );
    }
    assert_eq!(
        exchange("runtime.scroll_completion_predict", params(pre))["result"],
        exchange("runtime.scroll_completion_predict", params(pre))["result"],
        "idempotent"
    );
    for (at, value) in [(0, 0), (0x30, 5), (0x33, 0), (0x4C + 4, 0), (0x4C, 0)] {
        let mut raw = bytes(pre);
        raw[at] = value;
        assert_eq!(
            exchange("runtime.scroll_completion_predict", params(&hex(&raw)))["ok"],
            false,
            "reject corrupted/unsupported record {at}"
        );
    }
    assert_eq!(
        exchange(
            "runtime.scroll_completion_predict",
            json!({"record_hex":pre,"context_digest":"0".repeat(64)})
        )["ok"],
        false
    );
    assert_eq!(
        exchange("runtime.scroll_completion_predict", params(final_record))["ok"],
        false
    );
    // Equipment request guards run through the same production host/contract.
    // These controls never submit a valid native preparation or open a game.
    let unknown_id = "66666666-6666-4666-8666-666666666666";
    for method in [
        "runtime.equipment_add_status",
        "runtime.equipment_add_recover",
        "runtime.equipment_add_cancel",
    ] {
        let response = exchange(method, json!({"operation_id":unknown_id}));
        assert_eq!(response["ok"], true, "{response}");
        let job_id = response["result"]["job_id"].as_str().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let job = loop {
            let snapshot = exchange("job.snapshot", json!({"job_id":job_id}));
            assert_eq!(snapshot["ok"], true, "{snapshot}");
            if snapshot["result"]["state"] != "running" {
                break snapshot["result"].clone();
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(100));
        };
        assert_eq!(job["state"], "completed", "{job}");
        assert_eq!(
            job["result"]["equipment_add"]["state"],
            "rejected_before_dispatch"
        );
        assert!(job["result"]["equipment_add"]["process_id"].is_null());
    }
    assert_eq!(
        exchange(
            "runtime.equipment_add_execute",
            json!({"operation_id":unknown_id,"plan_digest":"0".repeat(64),"confirmed":false})
        )["ok"],
        false
    );
    assert_eq!(
        exchange(
            "runtime.equipment_add_prepare",
            json!({"operation_id":unknown_id,"item_id":0,"level":180,"plus":20,"rarity":4,"seed":65536})
        )["ok"],
        false
    );
    drop(sender);
    host.join().unwrap();
    let output = build_root().join("deliverables/codex-v083-missing-features-20260930/backend");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("completion-host-e2e.json"),serde_json::to_vec_pretty(&json!({"pass":true,"boundary":"framed Rust host; retained game records; no process/save writes","exchanges":exchanges})).unwrap()).unwrap();
}
