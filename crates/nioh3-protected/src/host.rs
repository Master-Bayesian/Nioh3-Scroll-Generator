//! The protected read loop, ported from `protected_worker.main`.
//!
//! Framing is reused from the read-only worker crate (`nioh3_worker::transport`)
//! rather than reimplemented, so the protected process speaks the identical
//! four-byte little-endian length-prefixed UTF-8 JSON protocol.

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use nioh3_worker::transport::{read_frame, write_frame};

use crate::app::{Role, RoleApplication};
use crate::contract::Contract;
use crate::error::HostError;
use crate::jobs::ProtectedJobs;

/// Serve the protected contract on one framed stdio pair until EOF or an agreed
/// shutdown.
///
/// The returned error is only ever a transport failure: frame-level faults are
/// fatal in the shipped worker too, so they terminate the process rather than
/// being answered.
pub fn serve<R: Read, W: Write>(
    application: Box<dyn RoleApplication>,
    contract: &Contract,
    source: &mut R,
    sink: &mut W,
) -> Result<(), String> {
    let application = Arc::new(Mutex::new(application));
    let jobs = ProtectedJobs::new();
    let role = lock(&application)?.role();
    let mut negotiated = false;

    loop {
        let payload = match read_frame(source) {
            Ok(Some(payload)) => payload,
            Ok(None) => break,
            Err(error) => return Err(error.to_string()),
        };
        let request_id = payload.get("id").cloned().unwrap_or(Value::Null);
        let (response, stop) = respond(
            &application,
            &jobs,
            contract,
            role,
            &mut negotiated,
            &payload,
            &request_id,
        );
        write_frame(sink, &response).map_err(|error| error.to_string())?;
        if stop {
            break;
        }
    }

    // A broker crash or a closed pipe is not permission to abandon ownership.
    jobs.request_cancel();
    jobs.join();
    lock(&application)?.finalize();
    Ok(())
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
    contract: &Contract,
    role: Role,
    negotiated: &mut bool,
    payload: &Value,
    request_id: &Value,
) -> (Value, bool) {
    match dispatch(application, jobs, contract, role, negotiated, payload) {
        Ok((result, stop)) => {
            let response = json!({
                "protocol": 1,
                "id": request_id,
                "ok": true,
                "result": result,
            });
            if contract.response_valid(&response) {
                (response, stop)
            } else {
                // The shipped worker raises here, which the outer handler turns
                // into an error frame with the default code.
                (error_frame(request_id, &HostError::invalid_result()), false)
            }
        }
        Err(error) => (error_frame(request_id, &error), false),
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
                let mut guard = lock(application).map_err(HostError::rejected)?;
                return Ok((guard.direct(method, &params)?, false));
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
            let job = jobs.start(method, cancellable, move |context| {
                let mut guard = shared
                    .lock()
                    .map_err(|_| HostError::rejected("protected application lock poisoned"))?;
                guard.run(&operation, params, context)
            })?;
            Ok((job, false))
        }
    }
}
