#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! In-process acceptance for the protected role host.
//!
//! These drive the real `serve` loop over in-memory framed streams with a
//! scripted role application, so the request contract, the handshake gate, the
//! role prefix, the single-owner job machine and the EOF ownership rule are all
//! exercised without a game process or a save file. Methods are real
//! `protected-request` methods so the request validator is genuinely applied.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use nioh3_protected::{serve, Contract, HostError, JobContext, Role, RoleApplication};

struct Scripted {
    role: Role,
    saw_cancel: Arc<AtomicBool>,
}

impl RoleApplication for Scripted {
    fn role(&self) -> Role {
        self.role
    }

    fn context_payload(&self) -> Value {
        json!({
            "product_version": "0.7.5",
            "game_profile": "pc-v2.00.02-v2.01",
            "resources_digest": "r",
            "algorithm_version": "a",
            "policy_version": "p",
            "context_digest": "c",
            "seed_accelerator_abi": 2,
            "seed_accelerator_build_id": "b",
        })
    }

    fn direct(&mut self, method: &str, _params: &Value) -> Result<Value, HostError> {
        if method == "runtime.status" {
            return Ok(json!({
                "override_state": "stopped",
                "hit_count": 0,
                "pending_remote_calls": 0,
                "safe_to_shutdown": true,
                "error": null,
            }));
        }
        Err(HostError::rejected("unexpected inline method"))
    }

    fn run(
        &mut self,
        operation: &str,
        params: Value,
        ctx: &JobContext,
    ) -> Result<Value, HostError> {
        match operation {
            // A contract-valid `SaveReference`, the shape `save.register` answers.
            "register" => Ok(json!({
                "save_id": "0".repeat(64),
                "path": params.get("path").cloned().unwrap_or(Value::Null),
                "account_id": "1",
                "save_slot": 0,
            })),
            // Blocks until the owner is cancelled, proving EOF cancels and joins.
            "discover" => {
                let deadline = Instant::now() + Duration::from_secs(30);
                while !ctx.cancelled() {
                    if Instant::now() > deadline {
                        return Err(HostError::rejected("wait timed out"));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                self.saw_cancel.store(true, Ordering::SeqCst);
                Ok(json!({"cancelled": true}))
            }
            other => Err(HostError::rejected(format!("EXPECTED_FAILURE {other}"))),
        }
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

fn frames(values: &[Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend_from_slice(&frame(value));
    }
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

fn drive(input: Vec<Value>, role: Role) -> (Vec<Value>, Arc<AtomicBool>) {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let application: Box<dyn RoleApplication> = Box::new(Scripted {
        role,
        saw_cancel: Arc::clone(&saw_cancel),
    });
    let mut source = Cursor::new(frames(&input));
    let mut sink = Vec::new();
    serve(application, &contract, &mut source, &mut sink).expect("serve succeeds");
    (responses(&sink), saw_cancel)
}

#[test]
fn handshake_is_contract_valid() {
    let (out, _) = drive(vec![request("1", "handshake", json!({}))], Role::Save);
    let handshake = out.first().expect("one response");
    assert_eq!(handshake["ok"], true, "handshake refused: {handshake}");
    assert_eq!(handshake["result"]["role"], "save");
    assert_eq!(handshake["result"]["protocol"], 1);
    assert_eq!(handshake["result"]["kill_safe"], false);
    assert_eq!(
        handshake["result"]["contract_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn handshake_then_job_current_and_shutdown() {
    // The action runs on its own thread, so poll `job.current` until the owner
    // reports a terminal state instead of assuming it finished in one step.
    let mut input = vec![
        request("1", "handshake", json!({})),
        request("2", "save.register", json!({"path": "SAVEDATA.BIN"})),
    ];
    for index in 0..200 {
        input.push(request(&format!("p{index}"), "job.current", json!({})));
    }
    let (out, _) = drive(input, Role::Save);
    assert_eq!(out.len(), 202);
    let started = &out[1];
    assert_eq!(started["ok"], true, "job start refused: {started}");
    assert_eq!(started["result"]["kind"], "save.register");
    assert_eq!(started["result"]["state"], "running");
    assert_eq!(started["result"]["sequence"], 0);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_string();
    assert!(
        job_id.len() == 32 && job_id.chars().all(|c| c.is_ascii_hexdigit()),
        "job id {job_id} is not 32 lowercase hex characters"
    );

    let last = out.last().expect("a response per request");
    let job = &last["result"]["job"];
    assert_eq!(job["job_id"], job_id);
    assert_eq!(job["state"], "completed", "job never completed: {job}");
    assert_eq!(job["result"]["path"], "SAVEDATA.BIN");
    assert!(job["sequence"].as_u64().unwrap() >= 1);

    // `job.current` never mutates the owner, so a shutdown is still accepted.
    let shutdown = drive(
        vec![
            request("1", "handshake", json!({})),
            request("2", "save.register", json!({"path": "SAVEDATA.BIN"})),
            request("3", "shutdown", json!({})),
        ],
        Role::Save,
    )
    .0;
    assert_eq!(shutdown[2]["result"]["safe_to_shutdown"], true);
}

#[test]
fn handshake_is_required_before_any_role_method() {
    let (out, _) = drive(
        vec![request(
            "1",
            "save.register",
            json!({"path": "SAVEDATA.BIN"}),
        )],
        Role::Save,
    );
    assert_eq!(out[0]["ok"], false);
    assert_eq!(out[0]["error"]["code"], "OPERATION_REJECTED");
    assert_eq!(out[0]["error"]["message"], "HANDSHAKE_REQUIRED");
}

#[test]
fn an_unknown_method_is_an_invalid_request() {
    let (out, _) = drive(
        vec![
            request("1", "handshake", json!({})),
            request("2", "save.not_a_method", json!({})),
        ],
        Role::Save,
    );
    assert_eq!(out[1]["ok"], false);
    assert_eq!(out[1]["error"]["code"], "OPERATION_REJECTED");
    assert_eq!(
        out[1]["error"]["message"],
        "INVALID_REQUEST: protected contract validation failed"
    );
}

#[test]
fn a_wrong_role_prefix_is_refused() {
    let (out, _) = drive(
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.status", json!({})),
        ],
        Role::Save,
    );
    assert_eq!(out[1]["ok"], false);
    assert_eq!(out[1]["error"]["message"], "ROLE_MISMATCH");
}

#[test]
fn runtime_status_is_answered_inline() {
    let (out, _) = drive(
        vec![
            request("1", "handshake", json!({})),
            request("2", "runtime.status", json!({})),
        ],
        Role::Runtime,
    );
    assert_eq!(out[1]["ok"], true, "status refused: {}", out[1]);
    assert_eq!(out[1]["result"]["override_state"], "stopped");
    assert_eq!(out[1]["result"]["safe_to_shutdown"], true);
}

#[test]
fn a_failed_job_reports_its_own_error() {
    let mut input = vec![
        request("1", "handshake", json!({})),
        request("2", "save.inventory", json!({"save_id": "0".repeat(32)})),
    ];
    for index in 0..200 {
        input.push(request(&format!("p{index}"), "job.current", json!({})));
    }
    let (out, _) = drive(input, Role::Save);
    let job = &out.last().expect("a response per request")["result"]["job"];
    assert_eq!(job["state"], "failed");
    // `protected_jobs.py` answers a failed job with `OPERATION_FAILED`.
    assert_eq!(job["error"]["code"], "OPERATION_FAILED");
    assert!(job["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("EXPECTED_FAILURE"));
}

#[test]
fn cancelling_an_unknown_job_uses_the_shipped_text() {
    let (out, _) = drive(
        vec![
            request("1", "handshake", json!({})),
            request("2", "save.register", json!({"path": "SAVEDATA.BIN"})),
            request("3", "job.cancel", json!({"job_id": "0".repeat(32)})),
        ],
        Role::Save,
    );
    assert_eq!(out[2]["ok"], false);
    assert_eq!(out[2]["error"]["message"], "Unknown operation job");
}

#[test]
fn eof_never_discards_in_flight_ownership() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let saw_cancel = Arc::new(AtomicBool::new(false));
    let application: Box<dyn RoleApplication> = Box::new(Scripted {
        role: Role::Save,
        saw_cancel: Arc::clone(&saw_cancel),
    });
    // handshake + start a blocking job, then close the stream: EOF must cancel
    // and join the owner before the host returns.
    let input = frames(&[
        request("1", "handshake", json!({})),
        request("2", "save.discover", json!({})),
    ]);
    let mut source = Cursor::new(input);
    let mut sink = Vec::new();
    serve(application, &contract, &mut source, &mut sink).expect("serve returns on EOF");
    assert!(
        saw_cancel.load(Ordering::SeqCst),
        "the in-flight protected owner must observe cancellation on EOF"
    );
}

#[test]
fn a_malformed_request_frame_is_answered_with_a_null_id() {
    let (out, _) = drive(
        vec![json!({"protocol": 1, "method": "handshake", "params": {}})],
        Role::Save,
    );
    assert_eq!(out[0]["ok"], false);
    assert_eq!(out[0]["id"], Value::Null);
    assert_eq!(
        out[0]["error"]["message"],
        "INVALID_REQUEST: protected contract validation failed"
    );
}
