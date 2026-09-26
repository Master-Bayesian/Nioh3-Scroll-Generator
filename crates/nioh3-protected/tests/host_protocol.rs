#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! In-process acceptance for the protected role host.
//!
//! These drive the real `serve` loop over in-memory framed streams with a
//! scripted role application, so the request contract, the handshake gate, the
//! role prefix, the single-owner job machine and the EOF ownership rule are all
//! exercised without a game process or a save file. Methods are real
//! `protected-request` methods so the request validator is genuinely applied.

use std::io::{self, BufRead, Cursor, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use nioh3_protected::{
    serve, serve_with_plan, serve_with_plan_and_degraded_poll, Contract, FinalizePlan,
    FinalizeStep, HostError, JobContext, Role, RoleApplication,
};

struct Scripted {
    role: Role,
    signals: Signals,
    panic_job: bool,
}

#[derive(Clone)]
struct Signals {
    saw_cancel: Arc<AtomicBool>,
    finalized: Arc<AtomicBool>,
    allow_finalize: Arc<AtomicBool>,
    ownership_unknown: Arc<AtomicBool>,
    panic_status_once: Arc<AtomicBool>,
    /// How many `finalize` attempts the host made before it stopped.
    finalize_attempts: Arc<AtomicU32>,
    /// The maximum number of attempts this application allows before it reports
    /// an unrecoverable owner instead of an ordinary retry.
    finalize_gate: Arc<AtomicU32>,
    /// When true, `finalize` panics instead of answering. The panic must be
    /// contained and turned into a retained attempt, never an exit proof.
    panic_finalize: Arc<AtomicBool>,
    /// Deterministic panic injection for the first N finalization attempts.
    panic_finalize_until: Arc<AtomicU32>,
    /// When true, `finalize` answers an unretainable step instead of retrying.
    finalize_terminal: Arc<AtomicBool>,
}

impl Signals {
    fn new() -> Self {
        Self {
            saw_cancel: Arc::new(AtomicBool::new(false)),
            finalized: Arc::new(AtomicBool::new(false)),
            allow_finalize: Arc::new(AtomicBool::new(true)),
            ownership_unknown: Arc::new(AtomicBool::new(false)),
            panic_status_once: Arc::new(AtomicBool::new(false)),
            finalize_attempts: Arc::new(AtomicU32::new(0)),
            finalize_gate: Arc::new(AtomicU32::new(u32::MAX)),
            panic_finalize: Arc::new(AtomicBool::new(false)),
            panic_finalize_until: Arc::new(AtomicU32::new(0)),
            finalize_terminal: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl RoleApplication for Scripted {
    fn role(&self) -> Role {
        self.role
    }

    fn context_payload(&self) -> Value {
        json!({
            "product_version": "0.8.1",
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
            if self.signals.panic_status_once.swap(false, Ordering::SeqCst) {
                panic!("injected application panic");
            }
            let unknown = self.signals.ownership_unknown.load(Ordering::SeqCst);
            return Ok(json!({
                "override_state": if unknown { "unknown" } else { "stopped" },
                "hit_count": 0,
                "pending_remote_calls": if unknown { 1 } else { 0 },
                "safe_to_shutdown": !unknown,
                "error": if unknown { Value::String("cleanup ownership is unknown".to_string()) } else { Value::Null },
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
        if self.panic_job {
            panic!("injected protected job panic");
        }
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
                self.signals.saw_cancel.store(true, Ordering::SeqCst);
                Ok(json!({"cancelled": true}))
            }
            "live_batch_execute" => {
                let deadline = Instant::now() + Duration::from_secs(30);
                while !ctx.cancelled() {
                    if Instant::now() > deadline {
                        return Err(HostError::rejected("wait timed out"));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                self.signals.saw_cancel.store(true, Ordering::SeqCst);
                Ok(json!({
                    "live_batch": {
                        "batch_id": "00000000-0000-0000-0000-000000000001",
                        "plan_digest": "1".repeat(64),
                        "state": "cancelled",
                        "count": 1,
                        "verified_count": 0,
                        "child_operation_ids": [],
                    }
                }))
            }
            "live_add_status" => {
                self.signals.ownership_unknown.store(true, Ordering::SeqCst);
                Ok(json!({
                    "live_add": {
                        "operation_id": "00000000-0000-0000-0000-000000000001",
                        "plan_digest": "1".repeat(64),
                        "state": "uncertain",
                        "can_dispatch": false,
                        "can_cancel": false,
                        "receipt": {
                            "business_outcome": "committed",
                            "remote_execution": "quiescent",
                            "allocation_state": "retained",
                            "debugger_state": "attached",
                            "thread_cleanup": {"71:1": {"cleanup_state": "unknown"}},
                            "released": false,
                        }
                    }
                }))
            }
            other => Err(HostError::rejected(format!("EXPECTED_FAILURE {other}"))),
        }
    }

    fn shutdown(&mut self) -> Result<Value, HostError> {
        Ok(json!({"safe_to_shutdown": true}))
    }

    fn finalize(&mut self) -> FinalizeStep {
        self.signals.finalized.store(true, Ordering::SeqCst);
        let attempts = self
            .signals
            .finalize_attempts
            .fetch_add(1, Ordering::SeqCst)
            + 1;
        if self.signals.panic_finalize.load(Ordering::SeqCst)
            || attempts <= self.signals.panic_finalize_until.load(Ordering::SeqCst)
        {
            panic!("injected finalization panic");
        }
        if self.signals.allow_finalize.load(Ordering::SeqCst) {
            return FinalizeStep::Released;
        }
        if self.signals.finalize_terminal.load(Ordering::SeqCst) {
            return FinalizeStep::Terminal {
                reason: "scripted owner can never be resolved in process".to_string(),
            };
        }
        if attempts >= self.signals.finalize_gate.load(Ordering::SeqCst) {
            return FinalizeStep::Terminal {
                reason: "scripted owner can never be resolved in process".to_string(),
            };
        }
        FinalizeStep::Retained {
            reason: "scripted owner still owns remote cleanup".to_string(),
        }
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
    let signals = Signals::new();
    let application: Box<dyn RoleApplication> = Box::new(Scripted {
        role,
        signals: signals.clone(),
        panic_job: false,
    });
    // A panicking job must not also panic `finalize`; only the job handler is
    // under test here.
    signals.panic_finalize.store(false, Ordering::SeqCst);
    let mut source = Cursor::new(frames(&input));
    let mut sink = Vec::new();
    serve(application, &contract, &mut source, &mut sink).expect("serve succeeds");
    (responses(&sink), signals.saw_cancel)
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
    // Failure modes: a queued burst can finish before the job is scheduled;
    // terminal state can precede thread exit; an invalid response or a genuinely
    // stuck job must still fail. Drive actual responses with bounded deadlines,
    // keeping the same host alive through registration, observation and shutdown.
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let application = scripted_application(Role::Save, &signals, false);
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = std::thread::spawn(move || {
        serve(
            application,
            &contract,
            &mut ChannelReader::new(receiver),
            &mut SharedSink(host_sink),
        )
        .expect("serve succeeds");
    });
    let mut count = 0;
    let mut exchange = |method: &str, params: Value| {
        count += 1;
        let id = count.to_string();
        sender
            .send(frame(&request(&id, method, params)))
            .expect("send a framed request");
        let out = wait_for_responses(&sink, count);
        assert_eq!(out.len(), count, "one response per request");
        let response = out.last().expect("response arrives").clone();
        assert_eq!(response["id"], id);
        assert_eq!(response["ok"], true, "request refused: {response}");
        response
    };
    let handshake = exchange("handshake", json!({}));
    assert_eq!(handshake["result"]["role"], "save");
    let started = exchange("save.register", json!({"path": "SAVEDATA.BIN"}));
    assert_eq!(started["ok"], true, "job start refused: {started}");
    assert_eq!(started["result"]["kind"], "save.register");
    assert_eq!(started["result"]["state"], "running");
    assert_eq!(started["result"]["sequence"], 0);
    let job_id = started["result"]["job_id"].as_str().unwrap().to_string();
    assert!(
        job_id.len() == 32 && job_id.chars().all(|c| c.is_ascii_hexdigit()),
        "job id {job_id} is not 32 lowercase hex characters"
    );

    let deadline = Instant::now() + Duration::from_secs(30);
    let job = loop {
        let current = exchange("job.current", json!({}));
        let job = current["result"]["job"].clone();
        assert_eq!(job["job_id"], job_id);
        if job["state"] != "running" {
            break job;
        }
        assert!(Instant::now() < deadline, "job never completed: {job}");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(job["job_id"], job_id);
    assert_eq!(job["state"], "completed", "job never completed: {job}");
    assert_eq!(job["result"]["path"], "SAVEDATA.BIN");
    assert!(job["sequence"].as_u64().unwrap() >= 1);

    // A terminal record does not itself prove that its thread has exited.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let shutdown = exchange("shutdown", json!({}));
        if shutdown["result"]["safe_to_shutdown"] == true {
            break;
        }
        assert_eq!(shutdown["result"]["error"], "Operation still running");
        assert!(Instant::now() < deadline, "shutdown never became safe");
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(sender);
    host.join().expect("the host exits after safe shutdown");
    assert!(signals.finalized.load(Ordering::SeqCst));
    println!(
        "HOST_PROTOCOL_E2E_OK {}",
        serde_json::to_string(&complete_responses(&sink.lock().expect("sink lock")))
            .expect("serialize the repeatable protocol transcript")
    );
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
    let input = vec![
        request("1", "handshake", json!({})),
        request("2", "save.inventory", json!({"save_id": "0".repeat(32)})),
    ];
    let out = drive_completed_job(input, Vec::new(), Role::Save, &Signals::new(), false);
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
    let signals = Signals::new();
    let application: Box<dyn RoleApplication> = Box::new(Scripted {
        role: Role::Save,
        signals: signals.clone(),
        panic_job: false,
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
        signals.saw_cancel.load(Ordering::SeqCst),
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

struct BrokenPipe;

impl Write for BrokenPipe {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "injected pipe close",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "injected pipe close",
        ))
    }
}

struct BreakAfterHandshake {
    writes: usize,
}

impl Write for BreakAfterHandshake {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.writes >= 2 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected pipe close after handshake",
            ));
        }
        self.writes += 1;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ChannelReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    chunk: Vec<u8>,
    offset: usize,
}

impl ChannelReader {
    fn new(receiver: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            receiver,
            chunk: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        while self.offset == self.chunk.len() {
            match self.receiver.recv() {
                Ok(chunk) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let count = output.len().min(self.chunk.len() - self.offset);
        output[..count].copy_from_slice(&self.chunk[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

#[derive(Clone)]
struct SharedSink(Arc<Mutex<Vec<u8>>>);

impl Write for SharedSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn complete_responses(bytes: &[u8]) -> Vec<Value> {
    let mut offset = 0usize;
    let mut out = Vec::new();
    while offset + 4 <= bytes.len() {
        let size = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        if offset + 4 + size > bytes.len() {
            break;
        }
        out.push(
            serde_json::from_slice(&bytes[offset + 4..offset + 4 + size])
                .expect("complete response json"),
        );
        offset += 4 + size;
    }
    out
}

fn wait_for_responses(sink: &Arc<Mutex<Vec<u8>>>, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let values = complete_responses(&sink.lock().expect("sink lock"));
        if values.len() >= count {
            return values;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for response {count}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn scripted_application(
    role: Role,
    signals: &Signals,
    panic_job: bool,
) -> Box<dyn RoleApplication> {
    Box::new(Scripted {
        role,
        signals: signals.clone(),
        panic_job,
    })
}

// A burst of queued polls is not elapsed time and may starve the job entirely.
// Keep each response observable, stop only on a terminal record, and preserve
// failure/panic/retained-cleanup payloads for the caller's original assertions.
fn drive_completed_job(
    before: Vec<Value>,
    after: Vec<Value>,
    role: Role,
    signals: &Signals,
    panic_job: bool,
) -> Vec<Value> {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let application = scripted_application(role, signals, panic_job);
    let (sender, receiver) = mpsc::channel();
    let sink = Arc::new(Mutex::new(Vec::new()));
    let host_sink = Arc::clone(&sink);
    let host = std::thread::spawn(move || {
        serve(
            application,
            &contract,
            &mut ChannelReader::new(receiver),
            &mut SharedSink(host_sink),
        )
        .expect("interactive protocol host");
    });
    let mut count = 0;
    let mut exchange = |value: Value| {
        count += 1;
        sender.send(frame(&value)).expect("send framed request");
        let out = wait_for_responses(&sink, count);
        assert_eq!(out.len(), count);
        let response = out.last().expect("response").clone();
        assert_eq!(response["id"], value["id"]);
        assert_eq!(response["ok"], true, "request refused: {response}");
        response
    };
    for value in before {
        exchange(value);
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let response = exchange(request("poll", "job.current", json!({})));
        let job = &response["result"]["job"];
        assert!(!job.is_null(), "the started job must remain observable");
        if job["state"] != "running" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "job never became terminal: {job}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for value in after {
        exchange(value);
    }
    drop(sender);
    host.join().expect("EOF finalizes the host");
    let out = complete_responses(&sink.lock().expect("sink lock"));
    println!(
        "HOST_PROTOCOL_E2E_OK {}",
        serde_json::to_string(&out).unwrap()
    );
    out
}

#[test]
fn normal_eof_enters_finalization() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(Vec::<u8>::new());
    let mut sink = Vec::new();
    serve(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect("clean EOF");
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn truncated_frame_enters_finalization_before_returning_error() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(vec![5, 0, 0, 0, b'{']);
    let mut sink = Vec::new();
    let error = serve(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect_err("truncated frame is fatal");
    assert_eq!(error, "Truncated frame");
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn malformed_frame_enters_finalization_before_returning_error() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let body = b"{not-json}";
    let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(body);
    let mut source = Cursor::new(bytes);
    let mut sink = Vec::new();
    let error = serve(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect_err("malformed frame is fatal");
    assert!(error.starts_with("Invalid frame JSON:"));
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn broken_pipe_enters_finalization_before_returning_error() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(frame(&request("1", "handshake", json!({}))));
    let mut sink = BrokenPipe;
    let error = serve(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect_err("broken pipe is fatal");
    assert!(error.contains("injected pipe close"));
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn broken_pipe_cancels_and_joins_an_in_flight_owner_before_exit() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(frames(&[
        request("1", "handshake", json!({})),
        request(
            "2",
            "runtime.live_batch_execute",
            json!({
                "batch_id": "00000000-0000-0000-0000-000000000001",
                "plan_digest": "1".repeat(64),
            }),
        ),
    ]));
    let mut sink = BreakAfterHandshake { writes: 0 };
    let error = serve(
        scripted_application(Role::Runtime, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect_err("job response pipe closes");
    assert!(error.contains("injected pipe close after handshake"));
    assert!(signals.saw_cancel.load(Ordering::SeqCst));
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn application_panic_crosses_the_boundary_only_after_finalization() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    signals.panic_status_once.store(true, Ordering::SeqCst);
    let mut source = Cursor::new(Vec::<u8>::new());
    let mut sink = Vec::new();
    let error = serve(
        scripted_application(Role::Runtime, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect_err("application panic is contained");
    assert!(error.contains("panicked"));
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn job_panic_is_terminal_and_host_finalization_still_runs() {
    let signals = Signals::new();
    let input = vec![
        request("1", "handshake", json!({})),
        request("2", "save.register", json!({"path": "SAVEDATA.BIN"})),
    ];
    let out = drive_completed_job(input, Vec::new(), Role::Save, &signals, true);
    let job = &out.last().expect("current job response")["result"]["job"];
    assert_eq!(job["state"], "failed");
    assert_eq!(job["error"]["code"], "OPERATION_REJECTED");
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn unresolved_owner_keeps_the_host_alive_until_release_is_proven() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    signals.allow_finalize.store(false, Ordering::SeqCst);
    signals.finalize_gate.store(u32::MAX, Ordering::SeqCst);
    let thread_signals = signals.clone();
    let handle = std::thread::spawn(move || {
        let mut source = Cursor::new(Vec::<u8>::new());
        let mut sink = Vec::new();
        serve(
            scripted_application(Role::Save, &thread_signals, false),
            &contract,
            &mut source,
            &mut sink,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !signals.finalized.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(signals.finalized.load(Ordering::SeqCst));
    assert!(
        !handle.is_finished(),
        "unknown ownership must keep its host alive"
    );
    signals.allow_finalize.store(true, Ordering::SeqCst);
    // The bounded schedule has already ended by now, so a host that only
    // re-checks inside its budget would never notice the release. The retained
    // state must stay re-checkable by the operator-driven release path, which
    // this proves: the retain is expressed by the live process, never by a spin
    // inside the exhausted loop.
    handle
        .join()
        .expect("host thread")
        .expect("release permits exit");
}

#[test]
fn bounded_finalization_reports_a_degraded_state_instead_of_spinning_forever() {
    accept_retained_process(
        "bounded_finalization_reports_a_degraded_state_instead_of_spinning_forever",
        3,
        u32::MAX,
        3,
    );
}

#[test]
fn a_terminal_finalize_step_ends_the_active_phase_without_waiting_out_the_bound() {
    accept_retained_process(
        "a_terminal_finalize_step_ends_the_active_phase_without_waiting_out_the_bound",
        8,
        1,
        1,
    );
}

// Failure modes: a missing/wrong first retained report, exiting with unresolved
// ownership, and failing to exit after release. Observe the actual host report
// in a child process: sampling a shared counter within a 5 ms window confuses
// valid degraded polls with active retries when the parent is descheduled.
fn accept_retained_process(name: &str, max_attempts: u32, terminal_at: u32, expected: u32) {
    const CHILD_KEY: &str = "NIOH3_FINALIZATION_ACCEPTANCE_CHILD";
    if std::env::var(CHILD_KEY).ok().as_deref() == Some(name) {
        let contract = Contract::load(&contract_dir()).expect("contract loads");
        let signals = Signals::new();
        signals.allow_finalize.store(false, Ordering::SeqCst);
        signals.finalize_gate.store(terminal_at, Ordering::SeqCst);
        let release_signals = signals.clone();
        std::thread::spawn(move || {
            let mut release = String::new();
            io::stdin().read_line(&mut release).expect("release input");
            assert_eq!(release.trim(), "release");
            release_signals.allow_finalize.store(true, Ordering::SeqCst);
        });
        serve_with_plan_and_degraded_poll(
            scripted_application(Role::Runtime, &signals, false),
            &contract,
            &mut Cursor::new(Vec::<u8>::new()),
            &mut Vec::new(),
            FinalizePlan {
                max_attempts,
                retry_interval: Duration::from_millis(1),
            },
            Duration::from_millis(5),
        )
        .expect("release permits exit");
        return;
    }
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", name, "--nocapture"])
            .env(CHILD_KEY, name)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start finalization acceptance child"),
    );
    let stderr = child.0.stderr.take().expect("child stderr");
    let (reports, received) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if line.contains("bounded finalization ended after") {
                let _ = reports.send(line);
            }
        }
    });
    let report = received
        .recv_timeout(Duration::from_secs(10))
        .expect("retained report");
    assert!(
        report.contains(&format!("ended after {expected} attempt(s)")),
        "the first retained report must name the exact active-phase bound: {report}"
    );
    assert!(
        child.0.try_wait().expect("child status").is_none(),
        "unresolved ownership must retain the process"
    );
    writeln!(child.0.stdin.take().expect("child stdin"), "release").expect("release owner");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.0.try_wait().expect("released child status") {
            assert!(
                status.success(),
                "released host must exit successfully: {status}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "released host did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "FINALIZATION_E2E_OK {}",
        json!({"test":name,"firstRetainedAttempts":expected,"retainedReport":report,"releasedExitCode":0})
    );
}

#[test]
fn a_released_owner_exits_on_the_first_finalization_attempt() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(Vec::<u8>::new());
    let mut sink = Vec::new();
    serve_with_plan(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
        FinalizePlan {
            max_attempts: 8,
            retry_interval: Duration::from_millis(250),
        },
    )
    .expect("clean EOF");
    assert_eq!(
        signals.finalize_attempts.load(Ordering::SeqCst),
        1,
        "an ordinary release must not pay the retry schedule"
    );
}

#[test]
fn a_panicking_finalize_is_reported_and_never_becomes_an_exit_proof() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    // Inject exactly two panics; a separately scheduled releaser can miss the
    // second attempt and incorrectly make the expected count timing-dependent.
    signals.panic_finalize_until.store(2, Ordering::SeqCst);
    let mut source = Cursor::new(Vec::<u8>::new());
    let mut sink = Vec::new();
    serve_with_plan_and_degraded_poll(
        scripted_application(Role::Runtime, &signals, false),
        &contract,
        &mut source,
        &mut sink,
        FinalizePlan {
            max_attempts: 2,
            retry_interval: Duration::from_millis(1),
        },
        Duration::from_millis(5),
    )
    .expect("clean EOF is not a transport error");
    assert_eq!(
        signals.finalize_attempts.load(Ordering::SeqCst),
        3,
        "a contained panic is retained, retried within the bound, then released"
    );
}

#[test]
fn shutdown_enters_finalization() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let mut source = Cursor::new(frames(&[
        request("1", "handshake", json!({})),
        request("2", "shutdown", json!({})),
    ]));
    let mut sink = Vec::new();
    serve(
        scripted_application(Role::Save, &signals, false),
        &contract,
        &mut source,
        &mut sink,
    )
    .expect("shutdown");
    assert_eq!(responses(&sink)[1]["result"]["safe_to_shutdown"], true);
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn blocking_native_job_keeps_status_and_cancel_responsive() {
    let contract = Contract::load(&contract_dir()).expect("contract loads");
    let signals = Signals::new();
    let (sender, receiver) = mpsc::channel();
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let thread_bytes = Arc::clone(&bytes);
    let thread_signals = signals.clone();
    let handle = std::thread::spawn(move || {
        let mut source = ChannelReader::new(receiver);
        let mut sink = SharedSink(thread_bytes);
        serve(
            scripted_application(Role::Runtime, &thread_signals, false),
            &contract,
            &mut source,
            &mut sink,
        )
    });

    sender
        .send(frame(&request("1", "handshake", json!({}))))
        .expect("handshake send");
    wait_for_responses(&bytes, 1);
    sender
        .send(frame(&request(
            "2",
            "runtime.live_batch_execute",
            json!({
                "batch_id": "00000000-0000-0000-0000-000000000001",
                "plan_digest": "1".repeat(64),
            }),
        )))
        .expect("job send");
    let out = wait_for_responses(&bytes, 2);
    let job_id = out[1]["result"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();

    let started = Instant::now();
    sender
        .send(frame(&request("3", "runtime.status", json!({}))))
        .expect("status send");
    let out = wait_for_responses(&bytes, 3);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(out[2]["result"]["safe_to_shutdown"], false);

    sender
        .send(frame(&request(
            "4",
            "job.cancel",
            json!({"job_id": job_id}),
        )))
        .expect("cancel send");
    let out = wait_for_responses(&bytes, 4);
    assert_eq!(out[3]["result"]["state"], "cancel_requested");
    drop(sender);
    handle.join().expect("host thread").expect("controlled EOF");
    assert!(signals.saw_cancel.load(Ordering::SeqCst));
    assert!(signals.finalized.load(Ordering::SeqCst));
}

#[test]
fn business_success_with_unknown_cleanup_keeps_both_facts() {
    let signals = Signals::new();
    let input = vec![
        request("1", "handshake", json!({})),
        request(
            "2",
            "runtime.live_add_status",
            json!({"operation_id": "00000000-0000-0000-0000-000000000001"}),
        ),
    ];
    let out = drive_completed_job(
        input,
        vec![request("status", "runtime.status", json!({}))],
        Role::Runtime,
        &signals,
        false,
    );
    let job = out
        .iter()
        .rev()
        .find_map(|response| response["result"].get("job"))
        .expect("job projection");
    let receipt = &job["result"]["live_add"]["receipt"];
    assert_eq!(receipt["business_outcome"], "committed");
    assert_eq!(receipt["remote_execution"], "quiescent");
    assert_eq!(receipt["allocation_state"], "retained");
    assert_eq!(receipt["debugger_state"], "attached");
    assert_eq!(receipt["released"], false);
    let status = out.last().expect("runtime status");
    assert_eq!(status["result"]["safe_to_shutdown"], false);
}
