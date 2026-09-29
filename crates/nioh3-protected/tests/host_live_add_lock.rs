#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Protected-host acceptance for the live-add admission lock recovery.
//!
//! A live add whose worker process died used to leave
//! `live-add/native-executor/admission.lock` behind, and every later live add
//! failed with "Another native executor is admitting an operation" until the
//! player deleted the file by hand. Checked here through the real wire
//! contract:
//! - a lock file nobody holds is cleared when the runtime role starts;
//! - `runtime.reset_live_add_lock` reports `absent`, `cleared` or `held`, and
//!   never removes a lock a running executor holds or any receipt.

use std::io::Read;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use nioh3_protected::{serve, Contract, RoleApplication, RuntimeApplication};
use nioh3_runtime::mutation::native_executor::AdmissionLock;
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

#[test]
fn a_stale_live_add_lock_is_cleared_at_start_and_on_request() {
    let root = build_root()
        .join("tmp")
        .join(format!("nioh3-live-add-lock-{}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let state = root.join("state");
    let directory = state.join("live-add").join("native-executor");
    std::fs::create_dir_all(&directory).unwrap();
    let lock = directory.join("admission.lock");
    let receipt = directory.join("3f2a8c1e-0d4b-4c6a-9f1e-5b7d2a9c4e10.json");
    std::fs::write(&lock, b"").unwrap();
    std::fs::write(&receipt, b"{}").unwrap();

    let data = repo_root().join("nioh3_scroll_editor").join("data");
    let contracts = repo_root().join("packages").join("contracts");
    let engine = Engine::load(
        &data,
        &contracts,
        None,
        ContextSelection::Production(GameFileVersion(2, 0, 2, 0)),
    )
    .unwrap();
    let application: Box<dyn RoleApplication> =
        Box::new(RuntimeApplication::new(state.clone(), &data, engine.context().clone()).unwrap());
    assert!(
        !lock.exists(),
        "starting the runtime role clears a lock nobody holds"
    );
    assert!(receipt.exists(), "a receipt is never touched");

    let contract = Contract::load(&contracts).unwrap();
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = thread::spawn(move || {
        // Closing the request stream ends the loop with a stream error.
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
    let mut count = 0usize;
    let mut exchange = |method: &str, params: Value| -> Value {
        count += 1;
        let request =
            json!({"protocol": 1, "id": count.to_string(), "method": method, "params": params});
        sender.send(frame(&request)).unwrap();
        wait_response(&sink, count)[count - 1].clone()
    };
    let reset = |exchange: &mut dyn FnMut(&str, Value) -> Value| -> Value {
        let response = exchange("runtime.reset_live_add_lock", json!({}));
        assert_eq!(response["ok"], true, "{response}");
        response["result"]["state"].clone()
    };
    assert_eq!(exchange("handshake", json!({}))["ok"], true);

    assert_eq!(reset(&mut exchange), "absent");
    std::fs::write(&lock, b"").unwrap();
    assert_eq!(reset(&mut exchange), "cleared");
    assert!(!lock.exists());

    let held = AdmissionLock::acquire(&directory).unwrap();
    assert_eq!(reset(&mut exchange), "held");
    assert!(lock.exists(), "a held lock is left in place");
    drop(held);
    assert_eq!(reset(&mut exchange), "absent");
    assert!(receipt.exists(), "a receipt is never touched");

    drop(sender);
    host.join().unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}
