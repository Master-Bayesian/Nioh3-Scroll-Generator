//! The protected read loop, ported from `protected_worker.main`.
//!
//! Framing is reused from the read-only worker crate (`nioh3_worker::transport`)
//! rather than reimplemented, so the protected process speaks the identical
//! four-byte little-endian length-prefixed UTF-8 JSON protocol.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::Duration;

use nioh3_worker::transport::{read_frame, write_frame};

use crate::app::{FinalizePlan, FinalizeStep, Role, RoleApplication};
use crate::contract::Contract;
use crate::error::HostError;
use crate::jobs::ProtectedJobs;

#[derive(Clone)]
struct ControlPlane {
    runtime_status: Arc<Mutex<Value>>,
}

impl ControlPlane {
    fn new() -> Self {
        Self {
            runtime_status: Arc::new(Mutex::new(Self::unavailable_status())),
        }
    }

    /// The degraded terminal state the bounded schedule publishes while the
    /// process stays retained: the runtime is gone, and the ownership reason is
    /// the explicit, observable record the operator otherwise has no way to
    /// read from a host whose read loop has already left.
    fn finalization_degraded_status(attempts: u32, reason: &str) -> Value {
        json!({
            "override_state": "unknown",
            "hit_count": 0,
            "pending_remote_calls": 1,
            "safe_to_shutdown": false,
            "finalization": {
                "state": "retained",
                "remaining_runtime_ownership": true,
                "attempts": attempts,
                "reason": reason,
            },
            "error": reason,
        })
    }

    fn unavailable_status() -> Value {
        json!({
            "override_state": "unknown",
            "hit_count": 0,
            "pending_remote_calls": 1,
            "safe_to_shutdown": false,
            "error": "Runtime ownership status is unavailable",
        })
    }

    fn publish(&self, status: Value) {
        match self.runtime_status.lock() {
            Ok(mut guard) => *guard = status,
            Err(poisoned) => *poisoned.into_inner() = status,
        }
    }

    fn mark_job_running(&self) {
        let mut guard = match self.runtime_status.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(object) = guard.as_object_mut() {
            object.insert("safe_to_shutdown".to_string(), Value::Bool(false));
            object.insert(
                "error".to_string(),
                Value::String(
                    "Protected operation is running; ownership status is cached".to_string(),
                ),
            );
        } else {
            *guard = Self::unavailable_status();
        }
    }

    fn snapshot(&self) -> Value {
        match self.runtime_status.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

/// Serve the protected contract on one framed stdio pair until EOF or an agreed
/// shutdown.
///
/// Frame faults and contained application panics are returned only after the
/// closing sequence has cancelled/joined the job and proven final ownership
/// released. They are fatal rather than being answered on a compromised pipe.
pub fn serve<R: Read, W: Write>(
    application: Box<dyn RoleApplication>,
    contract: &Contract,
    source: &mut R,
    sink: &mut W,
) -> Result<(), String> {
    serve_with_plan(application, contract, source, sink, FinalizePlan::DEFAULT)
}

/// [`serve`] with an explicit bounded finalization schedule.
///
/// The schedule is the only seam between the process and its release policy, so
/// a test can drive an unresolvable owner into the degraded terminal state
/// without waiting out the shipped bound.
pub fn serve_with_plan<R: Read, W: Write>(
    application: Box<dyn RoleApplication>,
    contract: &Contract,
    source: &mut R,
    sink: &mut W,
    plan: FinalizePlan,
) -> Result<(), String> {
    serve_with_plan_and_degraded_poll(
        application,
        contract,
        source,
        sink,
        plan,
        crate::app::FINALIZE_DEGRADED_POLL,
    )
}

/// [`serve_with_plan`] with an explicit degraded-retained poll cadence.
///
/// The degraded cadence is the retained host's watch interval, so a test can
/// observe the retained state without waiting out the shipped one-second poll.
pub fn serve_with_plan_and_degraded_poll<R: Read, W: Write>(
    application: Box<dyn RoleApplication>,
    contract: &Contract,
    source: &mut R,
    sink: &mut W,
    plan: FinalizePlan,
    degraded_poll: Duration,
) -> Result<(), String> {
    let application = Arc::new(Mutex::new(application));
    let jobs = ProtectedJobs::new();
    let control = ControlPlane::new();
    let role = lock(&application).map(|guard| guard.role()).ok();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        serve_loop(&application, &jobs, &control, contract, source, sink)
    }));
    // Every exit, including a transport/application panic, enters the same
    // explicit closing sequence. No read/write `?` can bypass this block.
    jobs.begin_closing();
    jobs.request_cancel();
    jobs.join();
    let attempts =
        finalize_until_released(&application, plan, degraded_poll, |attempts, reason| {
            // The retained state is the degraded terminal record: the runtime is
            // gone and ownership is unresolved, so the host stays observable instead
            // of exiting. It is written once when the active phase ends.
            report_retained(role, attempts, reason);
            if let Some(role) = role {
                publish_retained_status(&control, role, attempts, reason);
            }
        });
    if let Some(role) = role {
        eprintln!(
            "protected {} host: finalization released ownership after {attempts} attempt(s)",
            role.label()
        );
    }

    match outcome {
        Ok(result) => result,
        Err(_) => Err("protected host panicked; ownership was finalized before exit".to_string()),
    }
}

fn serve_loop<R: Read, W: Write>(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    jobs: &ProtectedJobs,
    control: &ControlPlane,
    contract: &Contract,
    source: &mut R,
    sink: &mut W,
) -> Result<(), String> {
    let role = lock(application)?.role();
    if role == Role::Runtime {
        refresh_runtime_status(application, control);
    }
    let mut negotiated = false;

    loop {
        let payload = match read_frame(source) {
            Ok(Some(payload)) => payload,
            Ok(None) => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        let (response, stop) = respond(
            application,
            jobs,
            control,
            contract,
            role,
            &mut negotiated,
            &payload,
        );
        if let Err(error) = write_frame(sink, &response) {
            return Err(error.to_string());
        }
        if stop {
            return Ok(());
        }
    }
}

/// One finalization cycle after the read loop has left.
///
/// The *active* phase is bounded by `plan.max_attempts`, and its job is to make
/// the reason observable: released wins immediately, an unrecoverable step ends
/// the phase at once, and an ordinary `Retained` ends it after the bound. A
/// bounded active phase is not permission to stop working, so once it ends the
/// host keeps polling at `degraded_poll` and keeps the retained state
/// published. Only a release proof returns; nothing here can approve an exit
/// while ownership is unresolved.
///
/// Returns the attempt count on which ownership was proven released.
fn finalize_until_released(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    plan: FinalizePlan,
    degraded_poll: Duration,
    publish_retained: impl Fn(u32, &str),
) -> u32 {
    let mut attempts: u32 = 0;
    let mut attempts_exhausted = false;
    loop {
        attempts += 1;
        let reason = match attempt_finalize(application) {
            FinishAttempt::Released => return attempts,
            FinishAttempt::Retained { reason } => reason,
            FinishAttempt::Terminal { reason } => {
                attempts_exhausted = true;
                reason
            }
        };
        if attempts_exhausted || attempts >= plan.max_attempts {
            // The active phase has ended without an exit proof. Report it once,
            // then keep the retained state published while polling slower.
            publish_retained(attempts, &reason);
            std::thread::sleep(degraded_poll);
            continue;
        }
        std::thread::sleep(plan.retry_interval);
    }
}

/// One panic-contained `finalize` call.
///
/// `Ok(Released)` is an exit proof; `Err(reason)` and `Ok(Terminal)` are not.
fn attempt_finalize(application: &Arc<Mutex<Box<dyn RoleApplication>>>) -> FinishAttempt {
    let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = match application.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.finalize()
    }));
    match step {
        Ok(FinalizeStep::Released) => FinishAttempt::Released,
        Ok(FinalizeStep::Terminal { reason }) => FinishAttempt::Terminal { reason },
        Ok(FinalizeStep::Retained { reason }) => FinishAttempt::Retained { reason },
        Err(_) => FinishAttempt::Retained {
            reason: "protected finalization panicked; ownership is still unresolved".to_string(),
        },
    }
}

/// The internal, panic-flattened result of one `finalize` attempt.
enum FinishAttempt {
    Released,
    Retained { reason: String },
    Terminal { reason: String },
}

fn refresh_runtime_status(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    control: &ControlPlane,
) {
    let result = lock(application).and_then(|mut guard| {
        guard
            .direct("runtime.status", &Value::Null)
            .map_err(|error| error.message)
    });
    match result {
        Ok(status) => control.publish(status),
        Err(_) => control.publish(ControlPlane::unavailable_status()),
    }
}

fn try_refresh_runtime_status(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    control: &ControlPlane,
) {
    let result = match application.try_lock() {
        Ok(mut guard) => guard.direct("runtime.status", &Value::Null),
        Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => return,
    };
    if let Ok(status) = result {
        control.publish(status);
    }
}

/// Report the retained outcome on the worker's only pre-exit channel.
///
/// The protected worker speaks framed JSON on stdout and diagnostics on stderr,
/// and the read loop has already left, so a line on stderr is the one thing an
/// operator or broker sees for a process that is still held open by an
/// unresolved owner.
fn report_retained(role: Option<Role>, attempts: u32, reason: &str) {
    match role {
        Some(role) => eprintln!(
            "protected {} host: bounded finalization ended after {attempts} attempt(s) with \
             ownership unresolved; process retained and observable, not exiting: {reason}",
            role.label()
        ),
        None => eprintln!(
            "protected host: bounded finalization ended after {attempts} attempt(s) with \
             ownership unresolved; process retained and observable, not exiting: {reason}"
        ),
    }
}

/// Publish the retained outcome into the cached control plane.
///
/// The cache normally answers `runtime.status`; once the read loop has left it
/// is the host's own durable record of why the process is still alive.
fn publish_retained_status(control: &ControlPlane, role: Role, attempts: u32, reason: &str) {
    if role != Role::Runtime {
        return;
    }
    control.publish(ControlPlane::finalization_degraded_status(attempts, reason));
}

fn lock(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
) -> Result<std::sync::MutexGuard<'_, Box<dyn RoleApplication>>, String> {
    application
        .lock()
        .map_err(|_| "protected application lock poisoned".to_string())
}

fn respond(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    jobs: &ProtectedJobs,
    control: &ControlPlane,
    contract: &Contract,
    role: Role,
    negotiated: &mut bool,
    payload: &Value,
) -> (Value, bool) {
    let request_id = payload.get("id").cloned().unwrap_or(Value::Null);
    match dispatch(
        application,
        jobs,
        control,
        contract,
        role,
        negotiated,
        payload,
    ) {
        Ok((result, stop)) => {
            let response = json!({
                "protocol": 1,
                "id": &request_id,
                "ok": true,
                "result": result,
            });
            if contract.response_valid(&response) {
                (response, stop)
            } else {
                // The shipped worker raises here, which the outer handler turns
                // into an error frame with the default code.
                (
                    error_frame(&request_id, &HostError::invalid_result()),
                    false,
                )
            }
        }
        Err(error) => (error_frame(&request_id, &error), false),
    }
}

fn error_frame(request_id: &Value, error: &HostError) -> Value {
    json!({
        "protocol": 1,
        "id": request_id,
        "ok": false,
        "error": {"code": error.inline_code(), "message": error.message},
    })
}

fn dispatch(
    application: &Arc<Mutex<Box<dyn RoleApplication>>>,
    jobs: &ProtectedJobs,
    control: &ControlPlane,
    contract: &Contract,
    role: Role,
    negotiated: &mut bool,
    payload: &Value,
) -> Result<(Value, bool), HostError> {
    if !contract.request_valid(payload) {
        return Err(HostError::invalid_request());
    }
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(HostError::invalid_request)?;
    let mut params = payload.get("params").cloned().unwrap_or(Value::Null);

    if method == "handshake" {
        *negotiated = true;
        let guard = lock(application).map_err(HostError::rejected)?;
        return Ok((
            json!({
                "role": role.label(),
                "protocol": 1,
                "contract_digest": contract.digest(),
                "context": guard.context_payload(),
                "kill_safe": false,
            }),
            false,
        ));
    }
    if !*negotiated {
        return Err(HostError::handshake_required());
    }

    match method {
        "job.snapshot" => {
            let job_id = params
                .get("job_id")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?;
            Ok((jobs.snapshot(job_id)?, false))
        }
        "job.current" => Ok((jobs.current(), false)),
        "job.cancel" => {
            let job_id = params
                .get("job_id")
                .and_then(Value::as_str)
                .ok_or_else(HostError::invalid_request)?;
            Ok((jobs.cancel(job_id)?, false))
        }
        "shutdown" => {
            if !jobs.idle() {
                return Ok((
                    json!({"safe_to_shutdown": false, "error": "Operation still running"}),
                    false,
                ));
            }
            let result = match role {
                Role::Runtime => lock(application).map_err(HostError::rejected)?.shutdown()?,
                Role::Save => json!({"safe_to_shutdown": true}),
            };
            let stop = result.get("safe_to_shutdown") == Some(&Value::Bool(true));
            Ok((result, stop))
        }
        _ => {
            if !method.starts_with(role.prefix()) {
                return Err(HostError::role_mismatch());
            }
            if method == "runtime.status" {
                if jobs.idle() {
                    try_refresh_runtime_status(application, control);
                }
                return Ok((control.snapshot(), false));
            }
            let operation = method
                .split_once('.')
                .map(|(_, suffix)| suffix.to_string())
                .unwrap_or_else(|| method.to_string());
            if let Some(object) = params.as_object_mut() {
                // The shipped host drops the UI acknowledgement before dispatch.
                object.remove("title_screen_confirmed");
            }
            let cancellable = matches!(
                method,
                "runtime.generate"
                    | "runtime.search"
                    | "runtime.capture_grace"
                    | "runtime.live_batch_execute"
            );
            let shared = Arc::clone(application);
            let job_control = control.clone();
            if role == Role::Runtime {
                control.mark_job_running();
            }
            let job = jobs.start(method, cancellable, move |context| {
                let mut guard = shared
                    .lock()
                    .map_err(|_| HostError::rejected("protected application lock poisoned"))?;
                let outcome = guard.run(&operation, params, context);
                if role == Role::Runtime {
                    if let Ok(status) = guard.direct("runtime.status", &Value::Null) {
                        job_control.publish(status);
                    }
                }
                outcome
            });
            let job = match job {
                Ok(job) => job,
                Err(error) => {
                    if role == Role::Runtime {
                        try_refresh_runtime_status(application, control);
                    }
                    return Err(error);
                }
            };
            Ok((job, false))
        }
    }
}
