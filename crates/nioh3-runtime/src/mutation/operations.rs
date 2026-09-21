//! Durable at-most-once dispatch ownership for live additions.
//!
//! Port of `live_add_operations.LiveAddOperations`: plan, claim, cancel,
//! complete, unresolved scan and the receipt state machine, over an exclusive
//! receipt directory. An interrupted dispatched operation is `uncertain`:
//! reading its receipt is safe and replaying it is not.
//!
//! No game address, no numerical generation and no transport lives here.

use crate::error::RuntimeError;
use crate::mutation::count::{
    canonical_json, exclusive_json, is_canonical_uuid, read_json, sha256_hex,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn rejected(detail: &str) -> RuntimeError {
    RuntimeError::ReceiptConflict {
        detail: detail.to_string(),
    }
}

/// The dispatch states one operation can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Prepared,
    /// A durable, non-dispatchable preview child. It owns no claim, so no
    /// admission can turn it into an insertion.
    PreparingPreview,
    /// A preview child whose native preview reached a terminal, non-rejecting
    /// result: it matched, or its own explicit zero-redirect idle miss settled.
    PreviewCompleted,
    Cancelled,
    Uncertain,
    Verified,
    RejectedBeforeDispatch,
    /// A preview child that ran its native dispatch, was reviewed as a mismatch
    /// and left the complete, unchanged-inventory terminal proof.
    RejectedAfterPreview,
}

impl OperationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::PreparingPreview => "preparing_preview",
            Self::PreviewCompleted => "preview_completed",
            Self::Cancelled => "cancelled",
            Self::Uncertain => "uncertain",
            Self::Verified => "verified",
            Self::RejectedBeforeDispatch => "rejected_before_dispatch",
            Self::RejectedAfterPreview => "rejected_after_preview",
        }
    }
}

/// The durable record that makes one preview child discoverable before the
/// native side effect, without publishing a dispatchable plan.
const PREVIEW_CHILD_FILE: &str = "preview-child.json";

/// The receipt states a preview child can record.
const PREVIEW_CHILD_STATES: [&str; 4] = [
    "preview_completed",
    "preview_no_dispatch",
    "preview_uncertain",
    "rejected_after_preview",
];

/// `live_add_operations.LiveAddOperations.snapshot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSnapshot {
    pub operation_id: String,
    pub plan_digest: String,
    pub state: OperationState,
    pub can_dispatch: bool,
    pub can_cancel: bool,
    pub receipt: Option<Value>,
}

impl OperationSnapshot {
    pub fn to_json(&self) -> Value {
        json!({
            "operation_id": self.operation_id,
            "plan_digest": self.plan_digest,
            "state": self.state.as_str(),
            "can_dispatch": self.can_dispatch,
            "can_cancel": self.can_cancel,
            "receipt": self.receipt,
        })
    }
}

/// One receipt directory per operation, under `root/live-add`.
pub struct LiveAddOperations {
    root: PathBuf,
}

impl LiveAddOperations {
    pub fn new(root: &Path) -> Result<Self, RuntimeError> {
        fs::create_dir_all(root).map_err(|error| RuntimeError::Io {
            path: root.display().to_string(),
            detail: error.to_string(),
        })?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn directory(&self, operation_id: &str) -> Result<PathBuf, RuntimeError> {
        if !is_canonical_uuid(operation_id) {
            return Err(rejected("Use a canonical operation UUID"));
        }
        Ok(self.root.join(operation_id))
    }

    /// Port of `prepare`: exclusive create, digest over canonical JSON.
    pub fn prepare(
        &self,
        operation_id: &str,
        plan: &Value,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let directory = self.directory(operation_id)?;
        if plan.get("operation_id").and_then(Value::as_str) != Some(operation_id) {
            return Err(rejected("Plan operation identity differs"));
        }
        fs::create_dir(&directory).map_err(|error| RuntimeError::Io {
            path: directory.display().to_string(),
            detail: error.to_string(),
        })?;
        let digest = sha256_hex(&canonical_json(plan).into_bytes());
        exclusive_json(
            &directory.join("plan.json"),
            &json!({"digest": digest, "plan": plan}),
        )?;
        self.snapshot(operation_id)
    }

    /// Read one preview child's durable record.
    pub fn preview_child(&self, child_id: &str) -> Result<Value, RuntimeError> {
        let envelope = read_json(&self.directory(child_id)?.join(PREVIEW_CHILD_FILE))?;
        let record = envelope
            .get("child")
            .cloned()
            .ok_or_else(|| rejected("Stored preview child changed"))?;
        let digest = envelope
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored preview child changed"))?;
        if sha256_hex(&canonical_json(&record).into_bytes()) != digest {
            return Err(rejected("Stored preview child changed"));
        }
        Ok(record)
    }

    /// Whether one operation id already owns a durable preview child record.
    pub fn preview_child_known(&self, child_id: &str) -> Result<bool, RuntimeError> {
        Ok(self.directory(child_id)?.join(PREVIEW_CHILD_FILE).is_file())
    }

    /// Register a durable, non-dispatchable preview child before the native side
    /// effect.
    ///
    /// There is deliberately no `plan.json`: no claim can name this operation, so
    /// it can never be dispatched. Re-registering the same child returns its
    /// existing record instead of creating a second identity.
    pub fn register_preview_child(
        &self,
        child_id: &str,
        record: &Value,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let directory = self.directory(child_id)?;
        if record
            .get("parent_operation_id")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(rejected("Preview child needs a parent operation identity"));
        }
        if !directory.join(PREVIEW_CHILD_FILE).is_file() {
            fs::create_dir(&directory).map_err(|error| RuntimeError::Io {
                path: directory.display().to_string(),
                detail: error.to_string(),
            })?;
            exclusive_json(
                &directory.join(PREVIEW_CHILD_FILE),
                &json!({
                    "digest": sha256_hex(&canonical_json(record).into_bytes()),
                    "child": record,
                }),
            )?;
        }
        self.snapshot(child_id)
    }

    /// Record what a preview child's native run actually was.
    ///
    /// The child stays non-dispatchable in every case. A `rejected_after_preview`
    /// record must prove it dispatched (a preview with `redirect_count` of one),
    /// so a preview can never be classified as `rejected_before_dispatch`.
    pub fn record_preview_outcome(
        &self,
        child_id: &str,
        receipt: &Value,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let directory = self.directory(child_id)?;
        if !directory.join(PREVIEW_CHILD_FILE).is_file() {
            return Err(rejected("Unknown preview child"));
        }
        if directory.join("claim.json").is_file() {
            return Err(rejected("Preview child was claimed for dispatch"));
        }
        let state = receipt
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Preview child receipt has no terminal state"))?;
        if !PREVIEW_CHILD_STATES.contains(&state)
            || receipt.get("operation_id").and_then(Value::as_str) != Some(child_id)
        {
            return Err(rejected("Preview child receipt identity or state differs"));
        }
        if receipt.get("mode").and_then(Value::as_str) != Some("preview") {
            return Err(rejected("Preview child receipt is not a preview"));
        }
        if state == "rejected_after_preview"
            && receipt.get("redirect_count").and_then(Value::as_u64) != Some(1)
        {
            return Err(rejected(
                "Preview rejection does not establish that the dispatch ran",
            ));
        }
        exclusive_json(
            &directory.join("receipt.json"),
            &json!({
                "digest": sha256_hex(&canonical_json(receipt).into_bytes()),
                "receipt": receipt,
            }),
        )?;
        self.snapshot(child_id)
    }

    /// The child snapshot one durable preview record and its receipt resolve.
    fn preview_child_snapshot(
        &self,
        operation_id: &str,
        directory: &Path,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let record = self.preview_child(operation_id)?;
        let digest = sha256_hex(&canonical_json(&record).into_bytes());
        let receipt_path = directory.join("receipt.json");
        if !receipt_path.is_file() {
            return Ok(OperationSnapshot {
                operation_id: operation_id.to_string(),
                plan_digest: digest,
                state: OperationState::PreparingPreview,
                can_dispatch: false,
                can_cancel: false,
                receipt: None,
            });
        }
        let envelope = read_json(&receipt_path)?;
        let stored = envelope
            .get("receipt")
            .cloned()
            .ok_or_else(|| rejected("Stored operation receipt changed"))?;
        if envelope.get("digest").and_then(Value::as_str)
            != Some(sha256_hex(&canonical_json(&stored).into_bytes()).as_str())
            || stored.get("operation_id").and_then(Value::as_str) != Some(operation_id)
        {
            return Err(rejected("Stored operation receipt changed"));
        }
        let state = match stored.get("state").and_then(Value::as_str) {
            Some("rejected_after_preview") => OperationState::RejectedAfterPreview,
            Some("preview_completed") | Some("preview_no_dispatch") => {
                OperationState::PreviewCompleted
            }
            Some("preview_uncertain") => OperationState::Uncertain,
            _ => return Err(rejected("Stored operation receipt changed")),
        };
        Ok(OperationSnapshot {
            operation_id: operation_id.to_string(),
            plan_digest: digest,
            state,
            can_dispatch: false,
            can_cancel: false,
            receipt: Some(stored),
        })
    }

    /// Port of `plan`: the stored digest must still match its content.
    pub fn plan(&self, operation_id: &str) -> Result<(String, Value), RuntimeError> {
        let envelope = read_json(&self.directory(operation_id)?.join("plan.json"))?;
        let plan = envelope
            .get("plan")
            .cloned()
            .ok_or_else(|| rejected("Stored plan content changed"))?;
        let digest = envelope
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| rejected("Stored plan content changed"))?
            .to_string();
        if sha256_hex(&canonical_json(&plan).into_bytes()) != digest {
            return Err(rejected("Stored plan content changed"));
        }
        Ok((digest, plan))
    }

    /// Port of `claim`: the exclusive create is the cross-process arbiter.
    pub fn claim(&self, operation_id: &str, expected_digest: &str) -> Result<Value, RuntimeError> {
        let (digest, plan) = self.plan(operation_id)?;
        if digest != expected_digest {
            return Err(rejected("Reviewed plan digest differs"));
        }
        exclusive_json(
            &self.directory(operation_id)?.join("claim.json"),
            &json!({"action": "dispatch", "digest": expected_digest}),
        )?;
        Ok(plan)
    }

    /// Port of `cancel`. Cancellation claims the same file as dispatch, so it
    /// cannot race with it and report a false cancellation.
    pub fn cancel(&self, operation_id: &str) -> Result<OperationSnapshot, RuntimeError> {
        let (digest, _plan) = self.plan(operation_id)?;
        exclusive_json(
            &self.directory(operation_id)?.join("claim.json"),
            &json!({"action": "cancel", "digest": digest}),
        )?;
        self.snapshot(operation_id)
    }

    /// Port of `complete`: only an independently verified success or a proven
    /// pre-dispatch rejection can close a claimed operation.
    pub fn complete(
        &self,
        operation_id: &str,
        receipt: &Value,
    ) -> Result<OperationSnapshot, RuntimeError> {
        let directory = self.directory(operation_id)?;
        let claim = read_json(&directory.join("claim.json"))?;
        let (digest, _plan) = self.plan(operation_id)?;
        if claim.get("action").and_then(Value::as_str) != Some("dispatch")
            || claim.get("digest").and_then(Value::as_str) != Some(digest.as_str())
        {
            return Err(rejected("No matching dispatch claim"));
        }
        let state = receipt.get("state").and_then(Value::as_str);
        if receipt.get("operation_id").and_then(Value::as_str) != Some(operation_id)
            || !matches!(state, Some("verified") | Some("rejected_before_dispatch"))
        {
            return Err(rejected("Receipt identity or terminal state differs"));
        }
        let flagged = |key: &str| receipt.get(key).and_then(Value::as_bool) == Some(true);
        if state == Some("verified")
            && !(flagged("full_container_and_native_index_verified")
                && flagged("dispatch_and_cleanup_verified"))
        {
            return Err(rejected(
                "Successful receipt lacks independent verification",
            ));
        }
        if state == Some("rejected_before_dispatch")
            && receipt.get("redirect_count").and_then(Value::as_u64) != Some(0)
        {
            return Err(rejected("Rejection does not establish absence of dispatch"));
        }
        exclusive_json(
            &directory.join("receipt.json"),
            &json!({
                "digest": sha256_hex(&canonical_json(receipt).into_bytes()),
                "receipt": receipt,
            }),
        )?;
        self.snapshot(operation_id)
    }

    /// Port of `unresolved_ids`: only dispatched, unacknowledged plans block a
    /// new insertion.
    pub fn unresolved_ids(&self) -> Result<Vec<String>, RuntimeError> {
        let entries = fs::read_dir(&self.root).map_err(|error| RuntimeError::Io {
            path: self.root.display().to_string(),
            detail: error.to_string(),
        })?;
        let mut unresolved = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| RuntimeError::Io {
                path: self.root.display().to_string(),
                detail: error.to_string(),
            })?;
            let directory = entry.path();
            if !directory.join("claim.json").is_file()
                && !directory.join(PREVIEW_CHILD_FILE).is_file()
            {
                continue;
            }
            let Some(operation_id) = directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if operation_id == "batches" || operation_id == "native-executor" {
                continue;
            }
            if matches!(
                self.snapshot(&operation_id)?.state,
                OperationState::Uncertain | OperationState::PreparingPreview
            ) {
                unresolved.push(operation_id);
            }
        }
        unresolved.sort();
        Ok(unresolved)
    }

    /// Port of `snapshot`.
    pub fn snapshot(&self, operation_id: &str) -> Result<OperationSnapshot, RuntimeError> {
        let directory = self.directory(operation_id)?;
        if directory.join(PREVIEW_CHILD_FILE).is_file() {
            return self.preview_child_snapshot(operation_id, &directory);
        }
        let (digest, _plan) = self.plan(operation_id)?;
        let claim_path = directory.join("claim.json");
        let receipt_path = directory.join("receipt.json");
        let mut state = OperationState::Prepared;
        let mut receipt: Option<Value> = None;
        if claim_path.is_file() {
            let claim = read_json(&claim_path)?;
            let action = claim.get("action").and_then(Value::as_str);
            if claim.get("digest").and_then(Value::as_str) != Some(digest.as_str())
                || !matches!(action, Some("cancel") | Some("dispatch"))
            {
                return Err(rejected("Invalid operation claim"));
            }
            state = if action == Some("cancel") {
                OperationState::Cancelled
            } else {
                OperationState::Uncertain
            };
        }
        if receipt_path.is_file() {
            let envelope = read_json(&receipt_path)?;
            let stored = envelope
                .get("receipt")
                .cloned()
                .ok_or_else(|| rejected("Stored operation receipt changed"))?;
            if envelope.get("digest").and_then(Value::as_str)
                != Some(sha256_hex(&canonical_json(&stored).into_bytes()).as_str())
            {
                return Err(rejected("Stored operation receipt changed"));
            }
            if state != OperationState::Uncertain
                || stored.get("operation_id").and_then(Value::as_str) != Some(operation_id)
            {
                return Err(rejected("Receipt has no matching dispatch"));
            }
            state = match stored.get("state").and_then(Value::as_str) {
                Some("verified") => OperationState::Verified,
                Some("rejected_before_dispatch") => OperationState::RejectedBeforeDispatch,
                _ => {
                    return Err(rejected("Stored operation receipt changed"));
                }
            };
            receipt = Some(stored);
        }
        Ok(OperationSnapshot {
            operation_id: operation_id.to_string(),
            plan_digest: digest,
            state,
            can_dispatch: state == OperationState::Prepared,
            can_cancel: state == OperationState::Prepared,
            receipt,
        })
    }
}
