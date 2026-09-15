//! Sequential reviewed batches; every native insertion keeps its own receipt.
//!
//! Port of `live_add_batch.LiveAddBatch`. No batch rollback is claimed, a
//! claimed batch is never replayed even after an error, and reconciliation reads
//! the durable child receipts instead of dispatching anything again.

use crate::error::RuntimeError;
use crate::mutation::count::{
    canonical_json, exclusive_json, is_canonical_uuid, new_operation_id, read_json, sha256_hex,
};
use crate::mutation::inventory::CAPACITY;
use crate::mutation::live_add::LiveAddApplication;
use crate::mutation::operations::{OperationSnapshot, OperationState};
use serde_json::{json, Value};
use std::path::Path;

fn rejected(detail: &str) -> RuntimeError {
    RuntimeError::BatchRejected {
        detail: detail.to_string(),
    }
}

/// `live_add_batch.LiveAddBatch`, expressed over the shared application.
pub struct LiveAddBatch;

impl LiveAddBatch {
    /// `application.operations.root / 'batches'`.
    pub fn root(application: &LiveAddApplication) -> std::path::PathBuf {
        application.operations().root().join("batches")
    }

    pub fn directory(
        application: &LiveAddApplication,
        batch_id: &str,
    ) -> Result<std::path::PathBuf, RuntimeError> {
        if !is_canonical_uuid(batch_id) {
            return Err(rejected("Expected canonical batch UUID"));
        }
        Ok(Self::root(application).join(batch_id))
    }

    /// `LiveAddBatch.prepare`.
    pub fn prepare(
        application: &mut LiveAddApplication,
        candidates: &[Value],
        save_path: &Path,
    ) -> Result<Value, RuntimeError> {
        if candidates.is_empty() || candidates.len() > 200 {
            return Err(rejected("Batch size must be 1-200"));
        }
        let mut ids = Vec::new();
        for candidate in candidates {
            let id = candidate
                .get("candidate_id")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Duplicate candidate identity"))?;
            if ids.contains(&id.to_string()) {
                return Err(rejected("Duplicate candidate identity"));
            }
            ids.push(id.to_string());
        }
        for candidate in candidates {
            application.validate_candidate(candidate)?;
        }
        let first = application.prepare(&candidates[0], save_path, None)?;
        if first.count_before + candidates.len() > CAPACITY as usize {
            application.cancel(first.snapshot.operation_id.as_str())?;
            return Err(rejected("Insufficient scroll capacity"));
        }
        let batch_id = new_operation_id()?;
        let resolved = save_path.canonicalize().map_err(|error| RuntimeError::Io {
            path: save_path.display().to_string(),
            detail: error.to_string(),
        })?;
        let plan = json!({
            "batch_id": batch_id,
            "candidates": candidates,
            "save_path": resolved.display().to_string(),
            "first": first.to_json(),
        });
        let digest = sha256_hex(&canonical_json(&plan).into_bytes());
        std::fs::create_dir_all(Self::root(application)).map_err(|error| RuntimeError::Io {
            path: Self::root(application).display().to_string(),
            detail: error.to_string(),
        })?;
        let directory = Self::directory(application, &batch_id)?;
        std::fs::create_dir(&directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        exclusive_json(
            &directory.join("plan.json"),
            &json!({"plan": plan, "digest": digest}),
        )?;
        Ok(json!({
            "batch_id": batch_id,
            "plan_digest": digest,
            "count": candidates.len(),
            "state": "prepared",
        }))
    }

    /// `LiveAddBatch.execute`.
    ///
    /// `cancelled` is checked between items only; a claimed batch is never
    /// replayed, and the durable claim is the cross-process replay guard.
    pub fn execute(
        application: &mut LiveAddApplication,
        batch_id: &str,
        plan_digest: &str,
        cancelled: &mut dyn FnMut() -> bool,
        progress: &mut dyn FnMut(Value),
    ) -> Result<Value, RuntimeError> {
        let directory = Self::directory(application, batch_id)?;
        let stored = read_json(&directory.join("plan.json"))?;
        let plan = stored
            .get("plan")
            .cloned()
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        if stored.get("digest").and_then(Value::as_str) != Some(plan_digest)
            || sha256_hex(&canonical_json(&plan).into_bytes()) != plan_digest
        {
            return Err(rejected("Batch review digest differs"));
        }
        exclusive_json(
            &directory.join("claim.json"),
            &json!({"digest": plan_digest}),
        )?;
        let candidates = plan
            .get("candidates")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        let first = plan
            .get("first")
            .cloned()
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        let save_path = plan
            .get("save_path")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        progress(json!({"completed": 0, "total": candidates.len()}));
        let mut results: Vec<OperationSnapshot> = Vec::new();
        let mut previous: Option<String> = None;
        for (index, candidate) in candidates.iter().enumerate() {
            if cancelled() {
                if index == 0 {
                    let first_id = first
                        .get("operation_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| rejected("Batch review digest differs"))?;
                    application.cancel(first_id)?;
                }
                break;
            }
            let child = if index == 0 {
                first.clone()
            } else {
                application
                    .prepare(candidate, Path::new(save_path), previous.as_deref())?
                    .to_json()
            };
            exclusive_json(&directory.join(format!("child-{index:03}.json")), &child)?;
            let operation_id = child
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Batch review digest differs"))?
                .to_string();
            let child_digest = child
                .get("plan_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Batch review digest differs"))?
                .to_string();
            let result = application.execute(&operation_id, &child_digest)?;
            let verified = result.state == OperationState::Verified;
            results.push(result);
            progress(json!({
                "completed": results
                    .iter()
                    .filter(|result| result.state == OperationState::Verified)
                    .count(),
                "total": candidates.len(),
            }));
            if !verified {
                break;
            }
            previous = Some(operation_id);
        }
        let verified_count = results
            .iter()
            .filter(|result| result.state == OperationState::Verified)
            .count();
        let complete = results.len() == candidates.len() && verified_count == candidates.len();
        let receipt = json!({
            "batch_id": batch_id,
            "state": if complete { "complete" } else { "partial" },
            "verified_count": verified_count,
            "results": results
                .iter()
                .map(OperationSnapshot::to_json)
                .collect::<Vec<_>>(),
        });
        exclusive_json(&directory.join("receipt.json"), &receipt)?;
        Ok(receipt)
    }

    /// `LiveAddBatch.cancel`.
    pub fn cancel(
        application: &mut LiveAddApplication,
        batch_id: &str,
    ) -> Result<Value, RuntimeError> {
        let directory = Self::directory(application, batch_id)?;
        let stored = read_json(&directory.join("plan.json"))?;
        let digest = stored
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Batch review digest differs"))?
            .to_string();
        exclusive_json(
            &directory.join("claim.json"),
            &json!({"digest": digest, "cancelled": true}),
        )?;
        let first_id = stored
            .get("plan")
            .and_then(|plan| plan.get("first"))
            .and_then(|first| first.get("operation_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Batch review digest differs"))?
            .to_string();
        application.cancel(&first_id)?;
        exclusive_json(
            &directory.join("receipt.json"),
            &json!({
                "batch_id": batch_id,
                "state": "cancelled",
                "verified_count": 0,
                "results": [],
            }),
        )?;
        Self::status(application, batch_id)
    }

    /// `LiveAddBatch.status`: reconciliation only, never a replay.
    pub fn status(application: &LiveAddApplication, batch_id: &str) -> Result<Value, RuntimeError> {
        let directory = Self::directory(application, batch_id)?;
        let stored = read_json(&directory.join("plan.json"))?;
        let plan = stored
            .get("plan")
            .cloned()
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        let mut children: Vec<OperationSnapshot> = Vec::new();
        let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&directory)
            .map_err(|error| RuntimeError::Io {
                path: directory.display().to_string(),
                detail: error.to_string(),
            })?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("child-") && name.ends_with(".json"))
            })
            .collect();
        paths.sort();
        for path in paths {
            let child = read_json(&path)?;
            let operation_id = child
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| rejected("Batch review digest differs"))?;
            children.push(application.status(operation_id)?);
        }
        let receipt_path = directory.join("receipt.json");
        let receipt = if receipt_path.is_file() {
            read_json(&receipt_path)?
        } else {
            Value::Null
        };
        let claimed = directory.join("claim.json").is_file();
        let requested = plan
            .get("candidates")
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| rejected("Batch review digest differs"))?;
        let mut state = receipt
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("prepared")
            .to_string();
        if claimed && receipt.is_null() {
            // A child plan can exist before its journal entry is written. No
            // recorded child is insufficient evidence to clear uncertainty.
            if children.is_empty()
                || children
                    .iter()
                    .any(|child| child.state == OperationState::Uncertain)
            {
                state = "uncertain".to_string();
            } else if children.len() == requested
                && children
                    .iter()
                    .all(|child| child.state == OperationState::Verified)
            {
                state = "complete".to_string();
            } else {
                state = "partial".to_string();
            }
        }
        Ok(json!({
            "batch_id": batch_id,
            "plan_digest": stored.get("digest").cloned().unwrap_or(Value::Null),
            "state": state,
            "claimed": claimed,
            "requested_count": requested,
            "children": children
                .iter()
                .map(OperationSnapshot::to_json)
                .collect::<Vec<_>>(),
        }))
    }
}
