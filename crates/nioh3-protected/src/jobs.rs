//! Port of `nioh3_scroll_editor/protected_jobs.ProtectedJobs`.
//!
//! A protected operation is a single owner at a time. Its action runs on its own
//! thread so the read loop keeps answering `job.snapshot`, `job.cancel` and a
//! concurrent `runtime.status`; the terminal fields are published together after
//! the action's own cleanup, and the previous owner is only ever joined after it
//! reports a terminal state. The killable read-only search worker is a different
//! process and is not modelled here.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::error::HostError;

/// The cancel/progress handles the host hands to a running action.
pub struct JobContext {
    cancel: Arc<AtomicBool>,
    progress: Arc<dyn Fn(Value) + Send + Sync>,
}

impl JobContext {
    /// `cancelled.is_set()` for the cancellable runtime loops.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// `progress(value)` from the shipped generation/search loops.
    pub fn progress(&self, value: Value) {
        (self.progress)(value);
    }
}

struct JobRecord {
    job_id: String,
    kind: String,
    state: &'static str,
    sequence: u64,
    cancellable: bool,
    progress: Option<Value>,
    result: Option<Value>,
    error: Option<Value>,
}

impl JobRecord {
    fn to_json(&self) -> Value {
        json!({
            "job_id": self.job_id,
            "kind": self.kind,
            "state": self.state,
            "sequence": self.sequence,
            "cancellable": self.cancellable,
            "progress": self.progress,
            "result": self.result,
            "error": self.error,
        })
    }
}

/// The single protected owner behind `job.*`.
pub struct ProtectedJobs {
    cancel: Arc<AtomicBool>,
    closing: AtomicBool,
    job: Arc<Mutex<Option<JobRecord>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    seed: Mutex<u128>,
}

impl Default for ProtectedJobs {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtectedJobs {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            closing: AtomicBool::new(false),
            job: Arc::new(Mutex::new(None)),
            handle: Mutex::new(None),
            seed: Mutex::new(0),
        }
    }

    /// Join the previous owner only once it reports a terminal state.
    fn join_terminal_owner(&self) {
        let terminal = self
            .job
            .lock()
            .ok()
            .map(|guard| {
                guard
                    .as_ref()
                    .is_some_and(|record| record.state == "completed" || record.state == "failed")
            })
            .unwrap_or(false);
        if !terminal {
            return;
        }
        let handle = self.handle.lock().ok().and_then(|mut guard| guard.take());
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    fn next_job_id(&self) -> String {
        let mut seed = match self.seed.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *seed = seed.wrapping_add(1);
        let mut digest = Sha256::new();
        digest.update(std::process::id().to_le_bytes());
        digest.update(seed.to_le_bytes());
        digest.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        digest.update(uuid_salt().as_bytes());
        let bytes = digest.finalize();
        let mut rendered = String::with_capacity(32);
        for byte in bytes.iter().take(16) {
            rendered.push_str(&format!("{byte:02x}"));
        }
        rendered
    }

    /// `ProtectedJobs.start`: refuse while another action is live, then own it.
    pub fn start<F>(&self, kind: &str, cancellable: bool, action: F) -> Result<Value, HostError>
    where
        F: FnOnce(&JobContext) -> Result<Value, HostError> + Send + 'static,
    {
        if self.closing.load(Ordering::SeqCst) {
            return Err(HostError::rejected(
                "CLOSING: protected host is finalizing ownership",
            ));
        }
        self.join_terminal_owner();
        let mut handle_guard = match self.handle.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(handle) = handle_guard.as_ref() {
            if !handle.is_finished() {
                return Err(HostError::rejected(
                    "BUSY: protected operation is still running",
                ));
            }
        }
        *handle_guard = None;
        self.cancel.store(false, Ordering::SeqCst);

        let record = JobRecord {
            job_id: self.next_job_id(),
            kind: kind.to_string(),
            state: "running",
            sequence: 0,
            cancellable,
            progress: None,
            result: None,
            error: None,
        };
        let initial = record.to_json();
        match self.job.lock() {
            Ok(mut guard) => *guard = Some(record),
            Err(poisoned) => *poisoned.into_inner() = Some(record),
        }

        let shared = Arc::clone(&self.job);
        let cancel = Arc::clone(&self.cancel);
        let progress_shared = Arc::clone(&self.job);
        let context = JobContext {
            cancel: Arc::clone(&cancel),
            progress: Arc::new(move |value: Value| {
                if let Ok(mut guard) = progress_shared.lock() {
                    if let Some(record) = guard.as_mut() {
                        record.progress = Some(value);
                        record.sequence += 1;
                    }
                }
            }),
        };
        let name = format!("protected-{kind}");
        let spawn = std::thread::Builder::new().name(name).spawn(move || {
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(&context)));
            let (state, result, error) = match outcome {
                Ok(Ok(value)) => ("completed", Some(value), None),
                Ok(Err(failure)) => (
                    "failed",
                    None,
                    Some(json!({"code": failure.job_code(), "message": failure.message})),
                ),
                Err(_) => (
                    "failed",
                    None,
                    Some(json!({
                        "code": "OPERATION_REJECTED",
                        "message": "protected operation failed before recording an outcome"
                    })),
                ),
            };
            if let Ok(mut guard) = shared.lock() {
                if let Some(record) = guard.as_mut() {
                    record.state = state;
                    record.result = result;
                    record.error = error;
                    record.sequence += 1;
                }
            }
        });
        match spawn {
            Ok(handle) => {
                *handle_guard = Some(handle);
                Ok(initial)
            }
            Err(error) => {
                let mut guard = match self.job.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                if let Some(record) = guard.as_mut() {
                    record.state = "failed";
                    record.error = Some(json!({
                        "code": "OPERATION_REJECTED",
                        "message": format!("protected operation thread could not start: {error}"),
                    }));
                    record.sequence += 1;
                }
                Err(HostError::rejected(format!(
                    "protected operation thread could not start: {error}"
                )))
            }
        }
    }

    /// `ProtectedJobs.snapshot`, refusing an unknown id with the shipped text.
    pub fn snapshot(&self, job_id: &str) -> Result<Value, HostError> {
        let guard = match self.job.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.as_ref() {
            Some(record) if record.job_id == job_id => Ok(record.to_json()),
            _ => Err(HostError::rejected("Unknown operation job")),
        }
    }

    /// `ProtectedJobs.current`: `{'job': job_or_null}` recovery shape.
    pub fn current(&self) -> Value {
        let guard = match self.job.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        json!({"job": guard.as_ref().map(JobRecord::to_json)})
    }

    /// `ProtectedJobs.cancel`.
    pub fn cancel(&self, job_id: &str) -> Result<Value, HostError> {
        let mut guard = match self.job.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let record = match guard.as_mut() {
            Some(record) if record.job_id == job_id => record,
            _ => return Err(HostError::rejected("Unknown operation job")),
        };
        if !record.cancellable {
            return Err(HostError::rejected(
                "This operation cannot be cancelled; wait for its commit outcome",
            ));
        }
        self.cancel.store(true, Ordering::SeqCst);
        if record.state == "running" {
            record.state = "cancel_requested";
            record.sequence += 1;
        }
        Ok(record.to_json())
    }

    /// `ProtectedJobs.idle`.
    pub fn idle(&self) -> bool {
        self.join_terminal_owner();
        let guard = match self.handle.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.as_ref() {
            Some(handle) => handle.is_finished(),
            None => true,
        }
    }

    /// Signal cancellation to whatever is running; used on shutdown and EOF.
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        let mut guard = match self.job.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(record) = guard.as_mut() {
            if record.state == "running" {
                record.state = "cancel_requested";
                record.sequence += 1;
            }
        }
    }

    /// Enter the one-way host closing state. Once set, no new owner can start.
    pub fn begin_closing(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }

    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }

    /// `ProtectedJobs.join`: wait for the owning action to finish its cleanup.
    pub fn join(&self) {
        let handle = match self.handle.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

/// A per-process salt so job ids differ across restarts of the same second.
fn uuid_salt() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let address = &now as *const u128 as usize;
    format!("{address:032x}{now:032x}")
}
